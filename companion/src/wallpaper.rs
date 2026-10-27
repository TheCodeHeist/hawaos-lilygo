//! Image wallpapers (DESIGN.md Section 11.8): a picture the user picks,
//! cropped to the device's 320x170 screen and dithered to RGB565. It is kept
//! as `wallpaper-<hash>.png` next to the config, already in the device's
//! colors, so the preview shows what the device will, and the pixels the
//! device downloads come straight from it.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gpui_kit::gpui::{Image, ImageFormat};
use image::imageops::FilterType;
use image::{GenericImageView, RgbImage};
use sha1::{Digest, Sha1};

use crate::platform::config_file_path;

pub const WP_W: u32 = 320;
pub const WP_H: u32 = 170;

fn dir() -> PathBuf {
    config_file_path().parent().map(Path::to_path_buf).unwrap_or_default()
}

fn png_path(hash: &str) -> PathBuf {
    dir().join(format!("wallpaper-{hash}.png"))
}

/// Crops `src` to fill the screen, dithers it to RGB565 and saves it.
/// Returns the hash the device asks for it by.
pub fn import(src: &Path) -> Result<String, String> {
    let img = image::open(src).map_err(|e| format!("Couldn't open that picture: {e}"))?;
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("That picture is empty.".into());
    }
    // Cover: the largest centered crop with the screen's aspect ratio.
    let (cw, ch) = if u64::from(w) * u64::from(WP_H) > u64::from(h) * u64::from(WP_W) {
        ((u64::from(h) * u64::from(WP_W) / u64::from(WP_H)) as u32, h)
    } else {
        (w, (u64::from(w) * u64::from(WP_H) / u64::from(WP_W)) as u32)
    };
    let fitted = img
        .crop_imm((w - cw) / 2, (h - ch) / 2, cw.max(1), ch.max(1))
        .resize_exact(WP_W, WP_H, FilterType::Lanczos3)
        .to_rgb8();
    let shown = dither_565(&fitted);
    let hash = short_hash(&to_rgb565_le(&shown));

    let mut png = Vec::new();
    shown
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| format!("Couldn't save the wallpaper: {e}"))?;
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    std::fs::write(png_path(&hash), &png).map_err(|e| format!("Couldn't save the wallpaper: {e}"))?;
    // Only the current one is kept.
    if let Ok(entries) = std::fs::read_dir(dir()) {
        let keep = format!("wallpaper-{hash}.png");
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("wallpaper-") && name.ends_with(".png") && name != keep {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    Ok(hash)
}

/// The wallpaper's RGB565 pixels (little-endian, row-major), if it's saved
/// and still matches its hash.
pub fn pixels(hash: &str) -> Option<Vec<u8>> {
    if hash.is_empty() || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let img = image::open(png_path(hash)).ok()?.to_rgb8();
    if img.dimensions() != (WP_W, WP_H) {
        return None;
    }
    let px = to_rgb565_le(&img);
    (short_hash(&px) == hash).then_some(px)
}

/// The saved wallpaper for the settings page; decoded once per hash.
pub fn preview(hash: &str) -> Option<Arc<Image>> {
    static CACHE: Mutex<Option<(String, Arc<Image>)>> = Mutex::new(None);
    let mut c = CACHE.lock().ok()?;
    if let Some((h, img)) = c.as_ref() {
        if h == hash {
            return Some(Arc::clone(img));
        }
    }
    let bytes = std::fs::read(png_path(hash)).ok()?;
    let img = Arc::new(Image::from_bytes(ImageFormat::Png, bytes));
    *c = Some((hash.to_string(), Arc::clone(&img)));
    Some(img)
}

fn short_hash(data: &[u8]) -> String {
    let d = Sha1::digest(data);
    format!("{:02x}{:02x}{:02x}{:02x}", d[0], d[1], d[2], d[3])
}

/// Floyd-Steinberg to the 5-6-5 grid, so gradients in photos don't band.
/// Every output pixel is exactly representable in RGB565 (bit-replicated),
/// so to_rgb565_le() round-trips it.
fn dither_565(src: &RgbImage) -> RgbImage {
    let (w, h) = (src.width() as usize, src.height() as usize);
    let mut err = vec![[0f32; 3]; w * h];
    let mut out = RgbImage::new(src.width(), src.height());
    let bits = [5u32, 6, 5];
    for y in 0..h {
        for x in 0..w {
            let p = src.get_pixel(x as u32, y as u32).0;
            let mut q = [0u8; 3];
            for c in 0..3 {
                let v = (f32::from(p[c]) + err[y * w + x][c]).clamp(0., 255.);
                let levels = (1u32 << bits[c]) - 1;
                let level = (v * levels as f32 / 255.).round() as u32;
                // Bit replication: the 8-bit value the panel shows for this level.
                let shown = (level << (8 - bits[c])) | (level >> (2 * bits[c] - 8));
                q[c] = shown as u8;
                let e = v - shown as f32;
                let mut spread = |dx: isize, dy: usize, k: f32| {
                    let nx = x as isize + dx;
                    if nx >= 0 && (nx as usize) < w && y + dy < h {
                        err[(y + dy) * w + nx as usize][c] += e * k;
                    }
                };
                spread(1, 0, 7. / 16.);
                spread(-1, 1, 3. / 16.);
                spread(0, 1, 5. / 16.);
                spread(1, 1, 1. / 16.);
            }
            out.put_pixel(x as u32, y as u32, image::Rgb(q));
        }
    }
    out
}

fn to_rgb565_le(img: &RgbImage) -> Vec<u8> {
    let mut out = Vec::with_capacity((img.width() * img.height() * 2) as usize);
    for px in img.pixels() {
        let [r, g, b] = px.0;
        let c: u16 = ((u16::from(r) & 0xF8) << 8) | ((u16::from(g) & 0xFC) << 3) | (u16::from(b) >> 3);
        out.extend_from_slice(&c.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dithered_pixels_round_trip_exactly() {
        // A smooth gradient: the case dithering exists for.
        let src = RgbImage::from_fn(WP_W, WP_H, |x, y| image::Rgb([(x * 255 / WP_W) as u8, (y * 255 / WP_H) as u8, 90]));
        let shown = dither_565(&src);
        let px = to_rgb565_le(&shown);
        assert_eq!(px.len(), (WP_W * WP_H * 2) as usize);
        // Every shown pixel is the panel's value for its 565 code, so the PNG
        // the companion keeps decodes back to the same bytes and hash.
        for (p, c) in shown.pixels().zip(px.chunks(2)) {
            let v = u16::from_le_bytes([c[0], c[1]]);
            let (r, g, b) = ((v >> 11) as u8, ((v >> 5) & 0x3F) as u8, (v & 0x1F) as u8);
            assert_eq!(p.0, [(r << 3) | (r >> 2), (g << 2) | (g >> 4), (b << 3) | (b >> 2)]);
        }
    }

    #[test]
    fn import_crops_to_the_screen_and_serves_by_hash() {
        let home = std::env::temp_dir().join(format!("hawaos-wp-test-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        // SAFETY: the only test that touches HOME.
        unsafe { std::env::set_var("HOME", &home) };
        let src = home.join("tall.png");
        RgbImage::from_fn(300, 900, |_, y| image::Rgb([0, if y < 450 { 40 } else { 200 }, 0])).save(&src).unwrap();

        let hash = import(&src).unwrap();
        let px = pixels(&hash).expect("saved picture");
        assert_eq!(px.len(), (WP_W * WP_H * 2) as usize);
        assert!(pixels("00000000").is_none());
        // Re-importing replaces the old file rather than piling up copies.
        let wide = home.join("wide.png");
        RgbImage::from_pixel(1920, 1080, image::Rgb([10, 20, 30])).save(&wide).unwrap();
        let hash2 = import(&wide).unwrap();
        assert!(pixels(&hash).is_none() && pixels(&hash2).is_some());
        let _ = std::fs::remove_dir_all(&home);
    }
}
