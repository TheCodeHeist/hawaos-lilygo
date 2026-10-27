//! Text in scripts the device's fonts lack (DESIGN.md Section 11.9).
//!
//! The device's fonts cover Latin, Greek and Cyrillic. For anything else (CJK,
//! Arabic, Hebrew, Bengali and other Indic scripts, emoji) the companion shapes
//! the text with the system's fonts, the way a desktop would draw it, and
//! sends each shaped cluster as a small 4-bit glyph image. In the text itself
//! a cluster becomes one private-use character; the device's fallback fonts
//! draw those from the glyphs it was sent. Shaping happens here, so the device
//! never needs to know how Bengali conjuncts or Arabic joining work: it draws
//! left to right what arrives in display order.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent};
use sha1::{Digest, Sha1};
use unicode_normalization::UnicodeNormalization;

use crate::notifications::{EMPH_OFF, EMPH_ON};
use crate::protocol::{device_has_glyph, fit_bytes};

/// The device's text sizes (font_inter_{12,14,18}); each glyph goes at all of
/// them since the companion doesn't know which label shows a field.
pub const SIZES: [u8; 3] = [12, 14, 18];
/// Shaped once at this size and scaled for the others.
const SHAPE_PX: f32 = 14.0;
/// Private-use characters stand for clusters. U+F8FF is left alone: LVGL
/// treats it specially.
const PUA_FIRST: u32 = 0xE000;
const PUA_LAST: u32 = 0xF8FE;
/// The device's staging buffer (GlyphCache::kPackBytes).
pub const MAX_PACK: usize = 32 * 1024;
const PACKS_KEPT: usize = 32;
const TEXTS_KEPT: usize = 256;

/// One cluster at one size, ready for the device.
#[derive(Clone)]
struct GlyphImage {
    w: u8,
    h: u8,
    ofs_x: i8,
    ofs_y: i8,
    adv: u8,
    a4: Vec<u8>,
}

struct Engine {
    fonts: FontSystem,
    swash: SwashCache,
    /// Cluster identity (font and glyph ids, with their offsets) -> character.
    clusters: HashMap<String, u32>,
    images: HashMap<u32, Vec<GlyphImage>>,
    next: u32,
    /// Tells the device when characters were handed out afresh, so it drops
    /// glyphs a previous run of the companion gave the same characters.
    session: u32,
    texts: HashMap<String, (String, Vec<u32>)>,
    packs: VecDeque<(String, Arc<Vec<u8>>)>,
}

fn engine() -> &'static Mutex<Engine> {
    static ENGINE: OnceLock<Mutex<Engine>> = OnceLock::new();
    ENGINE.get_or_init(|| {
        Mutex::new(Engine {
            fonts: FontSystem::new(),
            swash: SwashCache::new(),
            clusters: HashMap::new(),
            images: HashMap::new(),
            next: PUA_FIRST,
            session: fresh_session(),
            texts: HashMap::new(),
            packs: VecDeque::new(),
        })
    })
}

fn fresh_session() -> u32 {
    let mut b = [0u8; 4];
    let _ = getrandom::fill(&mut b);
    u32::from_le_bytes(b) | 1
}

/// Fields as the device should get them, and the glyphs they need.
pub struct DeviceText {
    pub fields: Vec<String>,
    /// (hash, bytes) of the glyph pack, when any field needs one.
    pub pack: Option<(String, usize)>,
}

/// `fields` are (text, the device's byte limit for it). With `glyphs` false
/// (firmware without a glyph cache) text is only cleaned up for the fonts.
pub fn for_device(fields: &[(&str, usize)], glyphs: bool) -> DeviceText {
    if !glyphs {
        return DeviceText {
            fields: fields.iter().map(|(t, max)| fit_bytes(crate::protocol::sanitize_for_font(t), *max)).collect(),
            pack: None,
        };
    }
    let Ok(mut e) = engine().lock() else {
        return for_device(fields, false);
    };
    let mut out = Vec::with_capacity(fields.len());
    let mut wanted: Vec<u32> = Vec::new();
    for (text, max) in fields {
        let (converted, _) = e.convert(text);
        let fitted = fit_bytes(converted, *max);
        for c in fitted.chars() {
            let cp = c as u32;
            if (PUA_FIRST..=PUA_LAST).contains(&cp) && !wanted.contains(&cp) {
                wanted.push(cp);
            }
        }
        out.push(fitted);
    }
    let pack = (!wanted.is_empty()).then(|| e.pack(&wanted));
    DeviceText { fields: out, pack }
}

/// A pack the device asked for by hash (GET /gl/<hash>, gl_req).
pub fn pack(hash: &str) -> Option<Arc<Vec<u8>>> {
    let e = engine().lock().ok()?;
    e.packs.iter().find(|(h, _)| h == hash).map(|(_, p)| Arc::clone(p))
}

/// Line breaks and emphasis marks in notification bodies (notifications.rs),
/// which the device formats itself.
fn is_format(c: char) -> bool {
    matches!(c, '\n' | EMPH_ON | EMPH_OFF)
}

/// Composed, with tabs and other spacing as spaces and control characters
/// other than the format marks dropped.
fn tidy(text: &str) -> String {
    let mut s = String::with_capacity(text.len());
    for c in text.nfc() {
        if is_format(c) {
            s.push(c);
        } else if c.is_whitespace() {
            s.push(' ');
        } else if !c.is_control() {
            s.push(c);
        }
    }
    s.trim().to_string()
}

impl Engine {
    /// The text with every cluster the device can't draw replaced by a
    /// private-use character, in display order; and those characters.
    fn convert(&mut self, text: &str) -> (String, Vec<u32>) {
        if let Some(hit) = self.texts.get(text) {
            return hit.clone();
        }
        let tidy = tidy(text);
        let result = if tidy.chars().all(|c| device_has_glyph(c) || is_format(c)) {
            (tidy, Vec::new())
        } else {
            // Shaped between format marks, which pass through as they are.
            let mut out = String::with_capacity(tidy.len());
            let mut used = Vec::new();
            for (i, part) in tidy.split(is_format).enumerate() {
                if i > 0 {
                    let at: usize = tidy.split(is_format).take(i).map(|p| p.len() + 1).sum::<usize>() - 1;
                    out.push(tidy[at..].chars().next().unwrap_or(' '));
                }
                let (shaped, cps) = self.shape(part);
                out.push_str(&shaped);
                used.extend(cps);
            }
            (out, used)
        };
        if self.texts.len() >= TEXTS_KEPT {
            self.texts.clear();
        }
        self.texts.insert(text.to_string(), result.clone());
        result
    }

    fn shape(&mut self, text: &str) -> (String, Vec<u32>) {
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(SHAPE_PX, SHAPE_PX * 1.4));
        buffer.set_size(None, None);
        buffer.set_text(text, &Attrs::new(), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.fonts, false);

        let mut out = String::with_capacity(text.len());
        let mut used = Vec::new();
        let runs: Vec<(String, Vec<cosmic_text::LayoutGlyph>)> =
            buffer.layout_runs().map(|r| (r.text.to_string(), r.glyphs.to_vec())).collect();
        for (line, glyphs) in runs {
            // A cluster's glyphs come together; right-to-left runs come in
            // reading order, so put clusters in display order by position.
            let mut clusters: Vec<&[cosmic_text::LayoutGlyph]> = Vec::new();
            let mut i = 0;
            while i < glyphs.len() {
                let (start, end) = (glyphs[i].start, glyphs[i].end);
                let mut j = i + 1;
                while j < glyphs.len() && glyphs[j].start == start && glyphs[j].end == end {
                    j += 1;
                }
                clusters.push(&glyphs[i..j]);
                i = j;
            }
            let left = |c: &&[cosmic_text::LayoutGlyph]| c.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
            clusters.sort_by(|a, b| left(a).total_cmp(&left(b)));
            for c in clusters {
                let text = &line[c[0].start..c[0].end];
                let mut chars = text.chars();
                match (chars.next(), chars.next()) {
                    (Some(ch), None) if device_has_glyph(ch) => out.push(ch),
                    _ => {
                        if let Some(cp) = self.cluster(c).or_else(|| self.monochrome(text)) {
                            out.push(char::from_u32(cp).unwrap_or(' '));
                            used.push(cp);
                        }
                    }
                }
            }
        }
        (out, used)
    }

    /// A cluster that drew nothing, again in a one-colour emoji font: colour
    /// emoji fonts (COLRv1) don't rasterise here.
    fn monochrome(&mut self, text: &str) -> Option<u32> {
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(SHAPE_PX, SHAPE_PX * 1.4));
        buffer.set_size(None, None);
        buffer.set_text(text, &Attrs::new().family(Family::Name("Noto Emoji")), Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut self.fonts, false);
        let glyphs: Vec<cosmic_text::LayoutGlyph> = buffer.layout_runs().flat_map(|r| r.glyphs.to_vec()).collect();
        if glyphs.is_empty() {
            return None;
        }
        self.cluster(&glyphs)
    }

    /// The character standing for these glyphs, drawn at every size; None
    /// when they draw nothing.
    fn cluster(&mut self, glyphs: &[cosmic_text::LayoutGlyph]) -> Option<u32> {
        let origin = glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
        let key = glyphs
            .iter()
            .map(|g| format!("{:?}:{}:{:.1}:{:.1}", g.font_id, g.glyph_id, g.x - origin, g.y_offset))
            .collect::<Vec<_>>()
            .join(",");
        if let Some(&cp) = self.clusters.get(&key) {
            return Some(cp);
        }
        let images: Vec<GlyphImage> =
            SIZES.iter().map(|&px| self.render(glyphs, origin, f32::from(px) / SHAPE_PX)).collect::<Option<_>>()?;
        if images.iter().all(|g| g.a4.is_empty()) {
            return None;
        }
        if self.next > PUA_LAST {
            // Out of characters: start over, and the device starts over too.
            self.clusters.clear();
            self.images.clear();
            self.texts.clear();
            self.next = PUA_FIRST;
            self.session = fresh_session();
        }
        let cp = self.next;
        self.next += 1;
        self.clusters.insert(key, cp);
        self.images.insert(cp, images);
        Some(cp)
    }

    fn render(&mut self, glyphs: &[cosmic_text::LayoutGlyph], origin: f32, scale: f32) -> Option<GlyphImage> {
        // Each glyph's coverage, placed with y growing down from the baseline.
        let mut parts = Vec::new();
        for g in glyphs {
            let pg = g.physical((-origin * scale, 0.0), scale);
            let Some(img) = self.swash.get_image_uncached(&mut self.fonts, pg.cache_key) else {
                continue;
            };
            let (w, h) = (img.placement.width as i32, img.placement.height as i32);
            let alpha: Vec<u8> = match img.content {
                SwashContent::Mask => img.data,
                // Colour emoji: darker parts read stronger, so faces and
                // outlines survive as one colour on the device.
                SwashContent::Color => img
                    .data
                    .chunks_exact(4)
                    .map(|p| {
                        let lum = (u32::from(p[0]) * 3 + u32::from(p[1]) * 6 + u32::from(p[2])) / 10;
                        (u32::from(p[3]) * (90 + (255 - lum) * 165 / 255) / 255) as u8
                    })
                    .collect(),
                SwashContent::SubpixelMask => img.data.chunks_exact(4).map(|p| p[1]).collect(),
            };
            parts.push((pg.x + img.placement.left, pg.y - img.placement.top, w, h, alpha));
        }
        let adv = (glyphs.iter().map(|g| g.w).sum::<f32>() * scale).round().clamp(0.0, 255.0) as u8;
        if parts.iter().all(|p| p.2 == 0 || p.3 == 0) {
            // Nothing to draw (a space-like cluster): keep the advance.
            return Some(GlyphImage { w: 0, h: 0, ofs_x: 0, ofs_y: 0, adv, a4: Vec::new() });
        }
        let x0 = parts.iter().map(|p| p.0).min()?;
        let y0 = parts.iter().map(|p| p.1).min()?;
        let x1 = parts.iter().map(|p| p.0 + p.2).max()?;
        let y1 = parts.iter().map(|p| p.1 + p.3).max()?;
        let (w, h) = ((x1 - x0).clamp(1, 255), (y1 - y0).clamp(1, 255));
        let mut cover = vec![0u8; (w * h) as usize];
        for (px, py, pw, ph, alpha) in &parts {
            for y in 0..*ph {
                for x in 0..*pw {
                    let (cx, cy) = (px - x0 + x, py - y0 + y);
                    if cx < w && cy < h {
                        let a = alpha[(y * pw + x) as usize];
                        let at = (cy * w + cx) as usize;
                        cover[at] = cover[at].max(a);
                    }
                }
            }
        }
        // 4 bits a pixel, first pixel in the high nibble, rows not padded
        // (LVGL's font bitmap layout).
        let mut a4 = vec![0u8; cover.len().div_ceil(2)];
        for (i, a) in cover.iter().enumerate() {
            let q = (u16::from(*a) + 8) / 17;
            a4[i / 2] |= (q as u8) << if i % 2 == 0 { 4 } else { 0 };
        }
        Some(GlyphImage {
            w: w as u8,
            h: h as u8,
            ofs_x: x0.clamp(-128, 127) as i8,
            // Bottom edge above the baseline.
            ofs_y: (-y1).clamp(-128, 127) as i8,
            adv,
            a4,
        })
    }

    /// Builds (or finds) the pack holding these characters at every size.
    fn pack(&mut self, cps: &[u32]) -> (String, usize) {
        // "HGL1", session, count, then per glyph: cp, px, w, h, ofs_x, ofs_y,
        // adv and the 4-bit bitmap.
        let mut body = Vec::new();
        let mut count: u16 = 0;
        'all: for &cp in cps {
            let Some(images) = self.images.get(&cp) else { continue };
            for (img, &px) in images.iter().zip(SIZES.iter()) {
                if 10 + body.len() + 11 + img.a4.len() > MAX_PACK {
                    break 'all;
                }
                body.extend_from_slice(&cp.to_le_bytes());
                body.extend_from_slice(&[px, img.w, img.h, img.ofs_x as u8, img.ofs_y as u8, img.adv]);
                body.push(0); // reserved
                body.extend_from_slice(&img.a4);
                count += 1;
            }
        }
        let mut pack = Vec::with_capacity(10 + body.len());
        pack.extend_from_slice(b"HGL1");
        pack.extend_from_slice(&self.session.to_le_bytes());
        pack.extend_from_slice(&count.to_le_bytes());
        pack.extend_from_slice(&body);
        let digest = Sha1::digest(&pack);
        let hash: String = digest[..4].iter().map(|b| format!("{b:02x}")).collect();
        let len = pack.len();
        if !self.packs.iter().any(|(h, _)| *h == hash) {
            if self.packs.len() >= PACKS_KEPT {
                self.packs.pop_front();
            }
            self.packs.push_back((hash.clone(), Arc::new(pack)));
        }
        (hash, len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_text_needs_no_glyphs() {
        let t = for_device(&[("Señorita", 63), ("Shawn Mendes", 47)], true);
        assert_eq!(t.fields, ["Señorita", "Shawn Mendes"]);
        assert!(t.pack.is_none());
    }

    #[test]
    fn format_marks_pass_through() {
        let body = "moved to \u{2}3 PM\u{3}\n東京 ok";
        let t = for_device(&[(body, 95)], true);
        let f: Vec<char> = t.fields[0].chars().collect();
        assert_eq!(f[..9].iter().collect::<String>(), "moved to ");
        assert_eq!(f[9], '\u{2}');
        assert_eq!(f[10..16].iter().collect::<String>(), "3 PM\u{3}\n");
        assert_eq!(f[18..].iter().collect::<String>(), " ok");
    }

    #[test]
    fn old_firmware_gets_cleaned_text() {
        let t = for_device(&[("東京 Tokyo", 63)], false);
        assert_eq!(t.fields, ["Tokyo"]);
        assert!(t.pack.is_none());
    }

    // Needs CJK and Bengali system fonts: cargo test -- --ignored
    #[test]
    #[ignore]
    fn other_scripts_become_glyphs() {
        let t = for_device(&[("東京 Tokyo", 63), ("আমার সোনার বাংলা", 63)], true);
        let (hash, len) = t.pack.expect("a pack");
        let p = pack(&hash).expect("kept for the device");
        assert_eq!(p.len(), len);
        assert_eq!(&p[..4], b"HGL1");
        // Two CJK characters, then the Latin as it was.
        let tokyo: Vec<char> = t.fields[0].chars().collect();
        assert!(tokyo[..2].iter().all(|c| (PUA_FIRST..=PUA_LAST).contains(&(*c as u32))));
        assert_eq!(tokyo[2..].iter().collect::<String>(), " Tokyo");
        // Bengali: fewer clusters than characters (conjuncts, vowel signs),
        // spaces kept.
        let bn = &t.fields[1];
        assert_eq!(bn.matches(' ').count(), 2);
        assert!(bn.chars().count() < "আমার সোনার বাংলা".chars().count());
        let count = u16::from_le_bytes([p[8], p[9]]) as usize;
        assert_eq!(count % SIZES.len(), 0);
    }
}

