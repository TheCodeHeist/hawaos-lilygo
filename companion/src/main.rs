use image::GenericImageView;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, UdpSocket};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{Networks, System};

const DEFAULT_DEVICE_IP: &str = "192.168.0.98";
const UDP_PORT: u16 = 7777;
const ART_HTTP_PORT: u16 = 7778;
const ART_DIM: u32 = 128;

#[derive(Serialize)]
struct StatsMsg<'a> {
    #[serde(rename = "type")]
    msg_type: &'a str,
    ts: u64,
    cpu: u8,
    ram: u8,
    net_up_kbps: u32,
    net_down_kbps: u32,
}

#[derive(Serialize, Clone, PartialEq)]
struct MediaMsg {
    #[serde(rename = "type")]
    msg_type: String,
    state: String,
    title: String,
    artist: String,
    album: String,
    pos_ms: u32,
    dur_ms: u32,
    ts: u64,
    art: String,
    bg_top: String,
    bg_bottom: String,
    ink: String,
}

#[derive(Deserialize)]
struct IncomingPacket {
    #[serde(rename = "type")]
    msg_type: String,
    cmd: Option<String>,
    v: Option<u32>,
}

struct ArtEntry {
    rgb565_le: Vec<u8>,
    bg_top: String,
    bg_bottom: String,
    ink: String,
}

type SharedArtCache = Arc<Mutex<HashMap<String, ArtEntry>>>;

fn sanitize_for_font(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\u{2018}' | '\u{2019}' => out.push('\''),
            '\u{201C}' | '\u{201D}' => out.push('"'),
            '\u{2013}' => out.push('-'),
            '\u{2014}' => out.push_str(" - "),
            '\u{2026}' => out.push_str("..."),
            '\u{00B7}' | '\u{00D7}' | '\u{00B0}' => out.push(ch),
            c if (' '..='~').contains(&c) => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn compute_tinted_palette(img: &image::DynamicImage) -> (String, String, String) {
    let small = img.resize_exact(16, 16, image::imageops::FilterType::Triangle);
    let mut r_sum: u64 = 0;
    let mut g_sum: u64 = 0;
    let mut b_sum: u64 = 0;
    let mut weight_sum: u64 = 0;

    for (_x, _y, px) in small.pixels() {
        let [r, g, b, a] = px.0;
        if a < 64 {
            continue;
        }
        let max_c = r.max(g).max(b) as u64;
        let min_c = r.min(g).min(b) as u64;
        let sat_weight = 1 + (max_c - min_c) / 16;
        r_sum += (r as u64) * sat_weight;
        g_sum += (g as u64) * sat_weight;
        b_sum += (b as u64) * sat_weight;
        weight_sum += sat_weight;
    }

    if weight_sum == 0 {
        return (
            "#0f3a44".to_string(),
            "#05090b".to_string(),
            "light".to_string(),
        );
    }

    let dom_r = (r_sum / weight_sum) as f32;
    let dom_g = (g_sum / weight_sum) as f32;
    let dom_b = (b_sum / weight_sum) as f32;

    // Clamp brightness so bg_top is a deep ambient tint (~28% of dominant color,
    // blended with deep cyan-slate) and bg_bottom is ~10%
    let top_r = ((dom_r * 0.28) + 6.0).clamp(6.0, 72.0) as u8;
    let top_g = ((dom_g * 0.28) + 14.0).clamp(12.0, 84.0) as u8;
    let top_b = ((dom_b * 0.28) + 18.0).clamp(16.0, 96.0) as u8;

    let bot_r = ((dom_r * 0.10) + 3.0).clamp(3.0, 28.0) as u8;
    let bot_g = ((dom_g * 0.10) + 7.0).clamp(6.0, 32.0) as u8;
    let bot_b = ((dom_b * 0.10) + 9.0).clamp(8.0, 38.0) as u8;

    (
        format!("#{:02x}{:02x}{:02x}", top_r, top_g, top_b),
        format!("#{:02x}{:02x}{:02x}", bot_r, bot_g, bot_b),
        "light".to_string(),
    )
}

fn load_and_convert_art(
    art_url: &str,
    cache: &SharedArtCache,
) -> Option<(String, String, String, String)> {
    if art_url.is_empty() {
        return None;
    }

    let mut hasher = Sha1::new();
    hasher.update(art_url.as_bytes());
    let full_hash = format!("{:x}", hasher.finalize());
    let short_hash = full_hash[..8].to_string();

    if let Ok(guard) = cache.lock() {
        if let Some(existing) = guard.get(&short_hash) {
            return Some((
                short_hash,
                existing.bg_top.clone(),
                existing.bg_bottom.clone(),
                existing.ink.clone(),
            ));
        }
    }

    let raw_bytes = if let Some(file_path) = art_url.strip_prefix("file://") {
        let decoded_path = file_path.replace("%20", " ");
        fs::read(decoded_path).ok()?
    } else if art_url.starts_with("http://") || art_url.starts_with("https://") {
        let resp = ureq::get(art_url)
            .timeout(Duration::from_millis(1500))
            .call()
            .ok()?;
        let mut bytes = Vec::new();
        resp.into_reader()
            .take(5 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .ok()?;
        bytes
    } else {
        return None;
    };

    let img = image::load_from_memory(&raw_bytes).ok()?;
    let (bg_top, bg_bottom, ink) = compute_tinted_palette(&img);

    // Center-crop to square and resize to 128x128
    let (w, h) = img.dimensions();
    let side = w.min(h);
    let cropped = img.crop_imm((w - side) / 2, (h - side) / 2, side, side);
    let resized = cropped
        .resize_exact(ART_DIM, ART_DIM, image::imageops::FilterType::Lanczos3)
        .to_rgb8();

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
                bg_top: bg_top.clone(),
                bg_bottom: bg_bottom.clone(),
                ink: ink.clone(),
            },
        );
    }

    Some((short_hash, bg_top, bg_bottom, ink))
}

fn query_mpris_media(cache: &SharedArtCache) -> MediaMsg {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // Query playerctl if installed on Fedora Linux
    let out = Command::new("playerctl")
        .args([
            "metadata",
            "--format",
            "{{status}}\t{{title}}\t{{artist}}\t{{album}}\t{{position}}\t{{mpris:length}}\t{{mpris:artUrl}}",
        ])
        .output();

    if let Ok(output) = out {
        if output.status.success() {
            let line = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = line.trim().split('\t').collect();
            if parts.len() >= 6 {
                let raw_status = parts[0].to_lowercase();
                let state = match raw_status.as_str() {
                    "playing" => "playing",
                    "paused" => "paused",
                    _ => "stopped",
                };
                let title = sanitize_for_font(parts[1]);
                let artist = sanitize_for_font(parts[2]);
                let album = sanitize_for_font(parts[3]);
                let pos_us: u64 = parts[4].parse().unwrap_or(0);
                let dur_us: u64 = parts[5].parse().unwrap_or(0);
                let art_url = if parts.len() >= 7 { parts[6] } else { "" };

                let (art_hash, bg_top, bg_bottom, ink) = load_and_convert_art(art_url, cache)
                    .unwrap_or_else(|| {
                        (
                            String::new(),
                            "#0f3a44".to_string(),
                            "#05090b".to_string(),
                            "light".to_string(),
                        )
                    });

                return MediaMsg {
                    msg_type: "media".to_string(),
                    state: state.to_string(),
                    title,
                    artist,
                    album,
                    pos_ms: (pos_us / 1000) as u32,
                    dur_ms: (dur_us / 1000) as u32,
                    ts,
                    art: art_hash,
                    bg_top,
                    bg_bottom,
                    ink,
                };
            }
        }
    }

    MediaMsg {
        msg_type: "media".to_string(),
        state: "stopped".to_string(),
        title: String::new(),
        artist: String::new(),
        album: String::new(),
        pos_ms: 0,
        dur_ms: 0,
        ts,
        art: String::new(),
        bg_top: "#071115".to_string(),
        bg_bottom: "#05090b".to_string(),
        ink: "light".to_string(),
    }
}

fn handle_device_command(cmd: &str) {
    println!(
        "[Companion] Executing media command from T-Display-S3: {}",
        cmd
    );
    let arg = match cmd {
        "play_pause" => "play-pause",
        "next" => "next",
        "prev" => "previous",
        _ => return,
    };
    let _ = Command::new("playerctl").arg(arg).spawn();
}

fn spawn_art_http_server(cache: SharedArtCache) {
    thread::spawn(move || {
        let bind_addr = format!("0.0.0.0:{}", ART_HTTP_PORT);
        let listener = match TcpListener::bind(&bind_addr) {
            Ok(l) => {
                println!(
                    "[ArtServer] Serving 128x128 RGB565 album art on http://{}",
                    bind_addr
                );
                l
            }
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

fn main() {
    let args: Vec<String> = env::args().collect();
    let target_ip = args.get(1).map(|s| s.as_str()).unwrap_or(DEFAULT_DEVICE_IP);
    let target_addr: SocketAddr = format!("{}:{}", target_ip, UDP_PORT)
        .parse()
        .expect("Invalid device IP address");

    println!("=====================================================");
    println!("  HawaOS Rust Companion Daemon v0.4.0");
    println!("  Target T-Display-S3 -> UDP {}", target_addr);
    println!("=====================================================");

    let art_cache: SharedArtCache = Arc::new(Mutex::new(HashMap::new()));
    spawn_art_http_server(Arc::clone(&art_cache));

    let udp = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind local UDP socket");
    let _ = udp.set_nonblocking(true);

    let now_sec = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let welcome_notify = serde_json::json!({
        "type": "notify",
        "title": "HawaOS Companion Connected",
        "body": "Streaming live Fedora PC telemetry & media over WiFi UDP.",
        "ts": now_sec
    });
    if let Ok(bytes) = serde_json::to_vec(&welcome_notify) {
        let _ = udp.send_to(&bytes, target_addr);
    }

    let mut sys = System::new_all();
    let mut networks = Networks::new_with_refreshed_list();
    let mut last_stats_tick = Instant::now() - Duration::from_secs(1);
    let mut last_media_heartbeat = Instant::now() - Duration::from_secs(10);
    let mut prev_media: Option<MediaMsg> = None;

    let mut rx_buf = [0u8; 1024];

    loop {
        while let Ok((len, _src)) = udp.recv_from(&mut rx_buf) {
            if let Ok(pkt) = serde_json::from_slice::<IncomingPacket>(&rx_buf[..len]) {
                if pkt.msg_type == "cmd" {
                    if let Some(cmd) = pkt.cmd {
                        handle_device_command(&cmd);
                        last_media_heartbeat = Instant::now() - Duration::from_secs(10);
                    }
                } else if pkt.msg_type == "config_ack" {
                    println!("[Companion] Device acknowledged config v={:?}", pkt.v);
                }
            }
        }

        if last_stats_tick.elapsed() >= Duration::from_secs(1) {
            let dt = last_stats_tick.elapsed().as_secs_f64().max(0.1);
            last_stats_tick = Instant::now();

            sys.refresh_cpu_usage();
            sys.refresh_memory();
            networks.refresh();

            let cpu_pct = sys.global_cpu_usage().round().clamp(0.0, 100.0) as u8;
            let total_mem = sys.total_memory().max(1);
            let used_mem = sys.used_memory();
            let ram_pct = (((used_mem as f64) * 100.0) / (total_mem as f64))
                .round()
                .clamp(0.0, 100.0) as u8;

            let mut rx_bytes: u64 = 0;
            let mut tx_bytes: u64 = 0;
            for (iface_name, data) in &networks {
                if iface_name == "lo" {
                    continue;
                }
                rx_bytes += data.received();
                tx_bytes += data.transmitted();
            }

            let net_down_kbps = ((rx_bytes as f64 / 1024.0) / dt).round() as u32;
            let net_up_kbps = ((tx_bytes as f64 / 1024.0) / dt).round() as u32;

            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let stats_msg = StatsMsg {
                msg_type: "stats",
                ts,
                cpu: cpu_pct,
                ram: ram_pct,
                net_up_kbps,
                net_down_kbps,
            };

            if let Ok(payload) = serde_json::to_vec(&stats_msg) {
                let _ = udp.send_to(&payload, target_addr);
            }

            let cur_media = query_mpris_media(&art_cache);
            let changed = match &prev_media {
                Some(prev) => {
                    prev.state != cur_media.state
                        || prev.title != cur_media.title
                        || prev.artist != cur_media.artist
                        || prev.art != cur_media.art
                }
                None => true,
            };

            if changed || last_media_heartbeat.elapsed() >= Duration::from_secs(5) {
                if let Ok(payload) = serde_json::to_vec(&cur_media) {
                    let _ = udp.send_to(&payload, target_addr);
                }
                prev_media = Some(cur_media);
                last_media_heartbeat = Instant::now();
            }
        }

        thread::sleep(Duration::from_millis(50));
    }
}
