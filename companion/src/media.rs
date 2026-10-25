use crate::config::get_local_lan_ip;
use gpui_kit::gpui::ImageFormat;
use image::GenericImageView;
use sha1::{Digest, Sha1};
use std::fs;
use std::io::{BufRead, Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gpui_kit::gpui::Image;
use crate::config::{ART_DIM, ART_HTTP_PORT};
use crate::models::{ArtEntry, MediaMsg, SharedArtCache};
use crate::protocol::sanitize_for_font;

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let s = if max == 0.0 { 0.0 } else { d / max };
    let v = max;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        (((g - b) / d) % 6.0) / 6.0
    } else if max == g {
        (((b - r) / d) + 2.0) / 6.0
    } else {
        (((r - g) / d) + 4.0) / 6.0
    };
    let h = if h < 0.0 { h + 1.0 } else { h };
    (h, s, v)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let h_deg = (h * 6.0).floor() as u32;
    let (r_prime, g_prime, b_prime) = match h_deg % 6 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (r_prime + m, g_prime + m, b_prime + m)
}

pub fn compute_tinted_palette(img: &image::DynamicImage) -> (String, String, String) {
    let small = img.resize_exact(32, 32, image::imageops::FilterType::Triangle);
    let mut weighted_r = 0.0f32;
    let mut weighted_g = 0.0f32;
    let mut weighted_b = 0.0f32;
    let mut total_w = 0.0f32;

    for (_x, _y, px) in small.pixels() {
        let [r, g, b, a] = px.0;
        if a < 64 {
            continue;
        }
        let rf = r as f32 / 255.0;
        let gf = g as f32 / 255.0;
        let bf = b as f32 / 255.0;
        let (_h, s, v) = rgb_to_hsv(rf, gf, bf);
        if s > 0.12 && v > 0.12 {
            let w = s.powf(1.5) * v.powf(0.5);
            weighted_r += rf * w;
            weighted_g += gf * w;
            weighted_b += bf * w;
            total_w += w;
        }
    }

    if total_w <= 0.0001 {
        return (
            "#0f3a44".to_string(),
            "#05090b".to_string(),
            "light".to_string(),
        );
    }

    let avg_r = weighted_r / total_w;
    let avg_g = weighted_g / total_w;
    let avg_b = weighted_b / total_w;
    let (h, s, _v) = rgb_to_hsv(avg_r, avg_g, avg_b);

    // s_top clamped to [0.40, 0.90], v_top = 0.42 (rich tone, high contrast against white text)
    let s_top = (s * 1.3).clamp(0.40, 0.90);
    let v_top = 0.42;
    let (r_top, g_top, b_top) = hsv_to_rgb(h, s_top, v_top);

    // s_bot clamped to [0.30, 0.80], v_bot = 0.12 (deep shadow gradient)
    let s_bot = (s * 1.0).clamp(0.30, 0.80);
    let v_bot = 0.12;
    let (r_bot, g_bot, b_bot) = hsv_to_rgb(h, s_bot, v_bot);

    (
        format!(
            "#{:02x}{:02x}{:02x}",
            (r_top * 255.0).round().clamp(0.0, 255.0) as u8,
            (g_top * 255.0).round().clamp(0.0, 255.0) as u8,
            (b_top * 255.0).round().clamp(0.0, 255.0) as u8
        ),
        format!(
            "#{:02x}{:02x}{:02x}",
            (r_bot * 255.0).round().clamp(0.0, 255.0) as u8,
            (g_bot * 255.0).round().clamp(0.0, 255.0) as u8,
            (b_bot * 255.0).round().clamp(0.0, 255.0) as u8
        ),
        "light".to_string(),
    )
}

pub fn load_and_convert_art(
    art_url: &str,
    cache: &SharedArtCache,
) -> Option<(String, String, String, String, Vec<u8>)> {
    if art_url.is_empty() {
        return None;
    }

    let mut hasher = Sha1::new();
    hasher.update(art_url.as_bytes());
    let digest = hasher.finalize();
    let short_hash = format!(
        "{:02x}{:02x}{:02x}{:02x}",
        digest[0], digest[1], digest[2], digest[3]
    );

    if let Ok(guard) = cache.lock() {
        if let Some(existing) = guard.get(&short_hash) {
            return Some((
                short_hash,
                existing.bg_top.clone(),
                existing.bg_bottom.clone(),
                existing.ink.clone(),
                existing.png_bytes.clone(),
            ));
        }
    }

    let raw_bytes = if let Some(file_path) = art_url.strip_prefix("file://") {
        let decoded_path = file_path.replace("%20", " ");
        fs::read(decoded_path).ok()?
    } else if art_url.starts_with("http://") || art_url.starts_with("https://") {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_millis(1500)))
            .build();
        let agent: ureq::Agent = config.into();
        let mut resp = agent.get(art_url).call().ok()?;
        resp.body_mut().read_to_vec().ok()?
    } else {
        return None;
    };

    let img = image::load_from_memory(&raw_bytes).ok()?;
    let (bg_top, bg_bottom, ink) = compute_tinted_palette(&img);

    let (w, h) = img.dimensions();
    let side = w.min(h);
    let cropped = img.crop_imm((w - side) / 2, (h - side) / 2, side, side);
    let resized_dyn = cropped.resize_exact(ART_DIM, ART_DIM, image::imageops::FilterType::Lanczos3);
    let resized = resized_dyn.to_rgb8();

    let mut png_bytes = Vec::new();
    let _ = resized_dyn.write_to(
        &mut std::io::Cursor::new(&mut png_bytes),
        image::ImageFormat::Png,
    );

    let mut rgb565_le = Vec::with_capacity((ART_DIM * ART_DIM * 2) as usize);
    for px in resized.pixels() {
        let [r, g, b] = px.0;
        let c565: u16 = (((r as u16) & 0xF8) << 8) | (((g as u16) & 0xFC) << 3) | ((b as u16) >> 3);
        rgb565_le.extend_from_slice(&c565.to_le_bytes());
    }

    if let Ok(mut guard) = cache.lock() {
        if guard.len() > 16 {
            guard.clear();
        }
        guard.insert(
            short_hash.clone(),
            ArtEntry {
                rgb565_le,
                png_bytes: png_bytes.clone(),
                bg_top: bg_top.clone(),
                bg_bottom: bg_bottom.clone(),
                ink: ink.clone(),
            },
        );
    }

    Some((short_hash, bg_top, bg_bottom, ink, png_bytes))
}

pub fn query_mpris_media(
    cache: &SharedArtCache,
) -> (MediaMsg, Option<Arc<Image>>, Option<String>) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let out = Command::new("playerctl")
        .args([
            "-a",
            "metadata",
            "--format",
            "{{playerName}}\t{{status}}\t{{title}}\t{{artist}}\t{{album}}\t{{position}}\t{{mpris:length}}\t{{mpris:artUrl}}",
        ])
        .output();

    if let Ok(output) = out {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let mut players: Vec<(String, String, String, String, String, u64, u64, String)> =
                Vec::new();

            for line in text.lines() {
                let parts: Vec<&str> = line.trim().split('\t').collect();
                if parts.len() >= 7 {
                    let player_name = parts[0].trim().to_string();
                    let raw_status = parts[1].trim().to_lowercase();
                    let title = sanitize_for_font(parts[2]);
                    let artist = sanitize_for_font(parts[3]);
                    let album = sanitize_for_font(parts[4]);
                    let pos_us: u64 = parts[5].trim().parse().unwrap_or(0);
                    let dur_us: u64 = parts[6].trim().parse().unwrap_or(0);
                    let art_url = if parts.len() >= 8 {
                        parts[7].trim().to_string()
                    } else {
                        String::new()
                    };
                    players.push((
                        player_name, raw_status, title, artist, album, pos_us, dur_us, art_url,
                    ));
                }
            }

            // Select active player:
            // 1. First player with status == "playing"
            // 2. First player with status == "paused" and non-empty title
            // 3. First available player
            let selected = players
                .iter()
                .find(|p| p.1 == "playing")
                .or_else(|| players.iter().find(|p| p.1 == "paused" && !p.2.is_empty()))
                .or_else(|| players.first());

            if let Some((
                player_name,
                raw_status,
                title,
                artist,
                album,
                pos_us,
                dur_us,
                art_url,
            )) = selected
            {
                let state = match raw_status.as_str() {
                    "playing" => "playing",
                    "paused" => "paused",
                    _ => "stopped",
                };

                let (art_hash, bg_top, bg_bottom, ink, art_img) =
                    match load_and_convert_art(art_url, cache) {
                        Some((h, t, b, i, png)) if !png.is_empty() => (
                            h,
                            t,
                            b,
                            i,
                            Some(Arc::new(Image::from_bytes(ImageFormat::Png, png))),
                        ),
                        Some((h, t, b, i, _)) => (h, t, b, i, None),
                        None => (
                            String::new(),
                            "#0f3a44".to_string(),
                            "#05090b".to_string(),
                            "light".to_string(),
                            None,
                        ),
                    };

                return (
                    MediaMsg {
                        msg_type: "media".to_string(),
                        state: state.to_string(),
                        title: title.clone(),
                        artist: artist.clone(),
                        album: album.clone(),
                        pos_ms: (pos_us / 1000) as u32,
                        dur_ms: (dur_us / 1000) as u32,
                        ts,
                        art: art_hash,
                        bg_top,
                        bg_bottom,
                        ink,
                        host_ip: Some(get_local_lan_ip()),
                    },
                    art_img,
                    Some(player_name.clone()),
                );
            }
        }
    }

    (MediaMsg::default(), None, None)
}

pub fn handle_device_command(cmd: &str, active_player: Option<&str>) {
    let arg = match cmd {
        "play_pause" => "play-pause",
        "next" => "next",
        "prev" => "previous",
        _ => return,
    };
    let mut command = Command::new("playerctl");
    if let Some(player) = active_player {
        if !player.is_empty() {
            command.args(["-p", player]);
        }
    }
    command.arg(arg);
    let _ = command.spawn();
}

pub fn spawn_art_http_server(cache: SharedArtCache) {
    thread::spawn(move || {
        let bind_addr = format!("0.0.0.0:{}", ART_HTTP_PORT);
        let listener = match TcpListener::bind(&bind_addr) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[ArtServer] Failed to bind {}: {}", bind_addr, e);
                return;
            }
        };

        for stream in listener.incoming().flatten() {
            let cache_ref = Arc::clone(&cache);
            thread::spawn(move || {
                let mut stream = stream;
                let _ = stream.set_read_timeout(Some(Duration::from_millis(1000)));
                let mut buf = [0u8; 512];
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    return;
                }
                let req = String::from_utf8_lossy(&buf[..n]);
                let first_line = req.lines().next().unwrap_or("");
                let parts: Vec<&str> = first_line.split_whitespace().collect();
                if parts.len() >= 2 && parts[0] == "GET" {
                    if let Some(hash) = parts[1].strip_prefix("/art/") {
                        let hash_clean = hash.trim();
                        let bytes_opt = cache_ref
                            .lock()
                            .ok()
                            .and_then(|g| g.get(hash_clean).map(|e| e.rgb565_le.clone()));

                        if let Some(bytes) = bytes_opt {
                            let header = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                bytes.len()
                            );
                            let _ = stream.write_all(header.as_bytes());
                            let _ = stream.write_all(&bytes);
                            return;
                        }
                    }
                }
                let _ = stream.write_all(
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            });
        }
    });
}

// ============================================================================
// System Tray Service (ksni StatusNotifierItem)
// ============================================================================
