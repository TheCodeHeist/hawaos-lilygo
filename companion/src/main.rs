use gpui_kit::assets::{AllAssets, IconName};
use gpui_kit::component::{
    Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    scroll::ScrollableElement,
    switch::Switch,
    theme::{Theme, ThemeMode as GpuiThemeMode},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use image::GenericImageView;
use ksni::blocking::TrayMethods;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet, VecDeque};
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, UdpSocket};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{Networks, System};

const DEFAULT_DEVICE_IP: &str = "192.168.0.98";
const UDP_PORT: u16 = 7777;
const ART_HTTP_PORT: u16 = 7778;
const ART_DIM: u32 = 128;
const USB_BAUD: u32 = 115_200;

static LOGO_PNG_BYTES: &[u8] = include_bytes!("../../assets/logo.png");

// ============================================================================
// Persistent Companion + Device Configuration
// ============================================================================

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct CompanionConfig {
    pub target_ip: String,
    pub usb_port: String,
    pub stream_via_usb: bool,
    pub auto_sync_settings: bool,
    pub autostart_enabled: bool,
    pub close_to_tray: bool,
    pub companion_dark_mode: bool,

    // Device configuration fields (DESIGN.md Section 11.3)
    pub config_version: u32,
    pub device_theme: String, // "dark" | "light" | "auto"
    pub brightness: u8,       // 20 | 40 | 60 | 80 | 100
    pub aod_enabled: bool,    // true | false
    pub aod_timeout_s: u16,   // 15 | 30 | 60 | 120 | 300
    pub reduce_motion: bool,  // true | false
    pub wallpaper: String,    // "gradient" | "solid"
    pub use_24h: bool,        // true | false
    pub notify_popup: bool,   // true | false
    pub wifi_ssid: String,
}

impl Default for CompanionConfig {
    fn default() -> Self {
        Self {
            target_ip: DEFAULT_DEVICE_IP.to_string(),
            usb_port: "/dev/ttyACM0".to_string(),
            stream_via_usb: false,
            auto_sync_settings: true,
            autostart_enabled: is_autostart_installed(),
            close_to_tray: true,
            companion_dark_mode: true,
            config_version: 1,
            device_theme: "dark".to_string(),
            brightness: 80,
            aod_enabled: true,
            aod_timeout_s: 30,
            reduce_motion: false,
            wallpaper: "gradient".to_string(),
            use_24h: true,
            notify_popup: true,
            wifi_ssid: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
struct DiscoveredWifi {
    pub ssid: String,
    pub signal: u8,
    pub security: String,
    pub in_use: bool,
}

fn scan_wifi_networks() -> Vec<DiscoveredWifi> {
    let out = Command::new("nmcli")
        .args([
            "-t",
            "-f",
            "IN-USE,SSID,SIGNAL,SECURITY,FREQ",
            "dev",
            "wifi",
            "list",
            "--rescan",
            "no",
        ])
        .output();
    let mut list = Vec::new();
    if let Ok(output) = out {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let mut seen = HashSet::new();
            for line in text.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 5 {
                    let in_use = parts[0].trim() == "*";
                    let ssid = parts[1].trim();
                    let signal: u8 = parts[2].trim().parse().unwrap_or(0);
                    let security = parts[3].trim().to_string();
                    let freq = parts[4].trim();

                    if ssid.is_empty() {
                        continue;
                    }
                    // Filter for 2.4 GHz frequencies (e.g. 2412..2484 MHz) supported by ESP32-S3
                    let is_24ghz = freq.starts_with("24") || freq.contains("2.4");
                    if !is_24ghz {
                        continue;
                    }
                    if seen.insert(ssid.to_string()) {
                        list.push(DiscoveredWifi {
                            ssid: ssid.to_string(),
                            signal,
                            security,
                            in_use,
                        });
                    }
                }
            }
        }
    }
    list.sort_by(|a, b| b.signal.cmp(&a.signal));
    list.truncate(10);
    list
}

fn config_file_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("hawaos-companion")
        .join("config.json")
}

fn autostart_desktop_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("autostart")
        .join("hawaos-companion.desktop")
}

fn is_autostart_installed() -> bool {
    autostart_desktop_path().exists()
}

fn set_autostart_installed(enable: bool) -> std::io::Result<()> {
    let path = autostart_desktop_path();
    if enable {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let exe_path = env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("hawaos-companion"))
            .display()
            .to_string();
        let desktop_entry = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=HawaOS Companion\n\
             Comment=HawaOS Desktop Companion & Tray Daemon for LilyGo T-Display-S3\n\
             Exec=\"{}\" --minimized\n\
             Terminal=false\n\
             Categories=Utility;System;\n\
             StartupNotify=false\n\
             X-GNOME-Autostart-enabled=true\n",
            exe_path
        );
        fs::write(path, desktop_entry)?;
    } else if path.exists() {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

fn sanitize_ip_str(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(idx) = trimmed.find(|c: char| c.is_whitespace() || c == '|' || c == ',' || c == ';') {
        trimmed[..idx].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn get_local_lan_ip() -> String {
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    String::new()
}

fn load_config() -> CompanionConfig {
    let path = config_file_path();
    if let Ok(bytes) = fs::read(&path) {
        if let Ok(mut cfg) = serde_json::from_slice::<CompanionConfig>(&bytes) {
            cfg.target_ip = sanitize_ip_str(&cfg.target_ip);
            cfg.autostart_enabled = is_autostart_installed();
            return cfg;
        }
    }
    CompanionConfig::default()
}

fn save_config(cfg: &CompanionConfig) {
    let mut clean_cfg = cfg.clone();
    clean_cfg.target_ip = sanitize_ip_str(&clean_cfg.target_ip);
    let path = config_file_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(&clean_cfg) {
        let _ = fs::write(path, json);
    }
}

// ============================================================================
// Wire Protocol Messages (DESIGN.md Section 11)
// ============================================================================

#[derive(Serialize)]
struct StatsMsg<'a> {
    #[serde(rename = "type")]
    msg_type: &'a str,
    ts: u64,
    cpu: u8,
    ram: u8,
    net_up_kbps: u32,
    net_down_kbps: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    host_ip: Option<&'a str>,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
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
    #[serde(skip_serializing_if = "String::is_empty", default)]
    host_ip: String,
}

impl Default for MediaMsg {
    fn default() -> Self {
        Self {
            msg_type: "media".to_string(),
            state: "stopped".to_string(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            pos_ms: 0,
            dur_ms: 0,
            ts: 0,
            art: String::new(),
            bg_top: "#071115".to_string(),
            bg_bottom: "#05090b".to_string(),
            ink: "light".to_string(),
            host_ip: String::new(),
        }
    }
}

#[derive(Deserialize, Debug)]
struct IncomingPacket {
    #[serde(rename = "type")]
    msg_type: String,
    cmd: Option<String>,
    v: Option<u32>,
    ip: Option<String>,
    wifi: Option<bool>,
    ssid: Option<String>,
    theme: Option<String>,
    brightness: Option<u8>,
    aod: Option<bool>,
    aod_timeout_s: Option<u16>,
    reduce_motion: Option<bool>,
    wallpaper: Option<String>,
    use_24h: Option<bool>,
    notify_popup: Option<bool>,
}

struct ArtEntry {
    rgb565_le: Vec<u8>,
    png_bytes: Vec<u8>,
    bg_top: String,
    bg_bottom: String,
    ink: String,
}

type SharedArtCache = Arc<Mutex<HashMap<String, ArtEntry>>>;

#[derive(Clone, Debug)]
enum OutboundAction {
    PushConfig {
        wifi_ssid: Option<String>,
        wifi_pass: Option<String>,
        forget_wifi: bool,
        force_usb: bool,
    },
    SendNotification {
        title: String,
        body: String,
    },
    QueryDevice {
        via_usb: bool,
    },
}

struct SharedDaemonState {
    config: CompanionConfig,
    cpu_pct: u8,
    ram_pct: u8,
    used_ram_gb: f32,
    total_ram_gb: f32,
    net_up_kbps: u32,
    net_down_kbps: u32,
    cpu_history: VecDeque<u8>,
    media: MediaMsg,
    active_player: Option<String>,
    current_art_png: Option<Arc<Image>>,
    last_ack_version: Option<u32>,
    last_ack_instant: Option<Instant>,
    device_reported_ip: Option<String>,
    device_wifi_connected: Option<bool>,
    available_ports: Vec<String>,
    discovered_ssids: Vec<DiscoveredWifi>,
    outbound_queue: VecDeque<OutboundAction>,
    logs: VecDeque<String>,
    open_window_requested: bool,
    quit_requested: bool,
}

impl SharedDaemonState {
    fn new(config: CompanionConfig) -> Self {
        let mut hist = VecDeque::with_capacity(30);
        for _ in 0..30 {
            hist.push_back(0);
        }
        let ports = scan_serial_ports();
        let ssids = scan_wifi_networks();
        Self {
            config,
            cpu_pct: 0,
            ram_pct: 0,
            used_ram_gb: 0.0,
            total_ram_gb: 0.0,
            net_up_kbps: 0,
            net_down_kbps: 0,
            cpu_history: hist,
            media: MediaMsg::default(),
            active_player: None,
            current_art_png: None,
            last_ack_version: None,
            last_ack_instant: None,
            device_reported_ip: None,
            device_wifi_connected: None,
            available_ports: ports,
            discovered_ssids: ssids,
            outbound_queue: VecDeque::new(),
            logs: VecDeque::with_capacity(40),
            open_window_requested: false,
            quit_requested: false,
        }
    }

    fn push_log(&mut self, msg: impl Into<String>) {
        if self.logs.len() >= 35 {
            self.logs.pop_front();
        }
        self.logs.push_back(msg.into());
    }
}

type SharedState = Arc<Mutex<SharedDaemonState>>;

fn scan_serial_ports() -> Vec<String> {
    let mut list = Vec::new();
    if let Ok(ports) = serialport::available_ports() {
        for p in ports {
            if p.port_name.contains("ttyACM") || p.port_name.contains("ttyUSB") {
                list.push(p.port_name);
            }
        }
    }
    if list.is_empty() && PathBuf::from("/dev/ttyACM0").exists() {
        list.push("/dev/ttyACM0".to_string());
    }
    list
}

// ============================================================================
// Album Art + MPRIS Media Engine
// ============================================================================

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

fn compute_tinted_palette(img: &image::DynamicImage) -> (String, String, String) {
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

fn load_and_convert_art(
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

fn query_mpris_media(
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
                        host_ip: get_local_lan_ip(),
                    },
                    art_img,
                    Some(player_name.clone()),
                );
            }
        }
    }

    (MediaMsg::default(), None, None)
}

fn handle_device_command(cmd: &str, active_player: Option<&str>) {
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

fn spawn_art_http_server(cache: SharedArtCache) {
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

struct HawaTray {
    state: SharedState,
    icon_pixmap: Vec<ksni::Icon>,
}

fn build_tray_icons() -> Vec<ksni::Icon> {
    let Ok(img) = image::load_from_memory_with_format(LOGO_PNG_BYTES, image::ImageFormat::Png)
    else {
        return Vec::new();
    };
    let mut icons = Vec::new();
    for dim in [32u32, 64u32] {
        let resized = img.resize_exact(dim, dim, image::imageops::FilterType::Lanczos3);
        let mut data = resized.into_rgba8().into_vec();
        for px in data.chunks_exact_mut(4) {
            px.rotate_right(1); // RGBA -> ARGB
        }
        icons.push(ksni::Icon {
            width: dim as i32,
            height: dim as i32,
            data,
        });
    }
    icons
}

impl ksni::Tray for HawaTray {
    fn id(&self) -> String {
        "hawaos-companion".into()
    }

    fn title(&self) -> String {
        "HawaOS Companion".into()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Hardware
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icon_pixmap.clone()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let (ip, cpu, ram, track) = if let Ok(g) = self.state.lock() {
            let t = if g.media.title.is_empty() {
                "Idle".to_string()
            } else {
                format!("{} — {}", g.media.title, g.media.artist)
            };
            (g.config.target_ip.clone(), g.cpu_pct, g.ram_pct, t)
        } else {
            (DEFAULT_DEVICE_IP.to_string(), 0, 0, "Idle".to_string())
        };

        ksni::ToolTip {
            icon_name: String::new(),
            icon_pixmap: self.icon_pixmap.clone(),
            title: format!("HawaOS Companion ({})", ip),
            description: format!("CPU {}% · RAM {}% · {}", cpu, ram, track),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        if let Ok(mut g) = self.state.lock() {
            g.open_window_requested = true;
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let (autostart, device_theme, track_summary, active_player) =
            if let Ok(g) = self.state.lock() {
                let summary = if g.media.title.is_empty() {
                    "Media: No active track".to_string()
                } else {
                    format!("♪ {} — {}", g.media.title, g.media.artist)
                };
                (
                    g.config.autostart_enabled,
                    g.config.device_theme.clone(),
                    summary,
                    g.active_player.clone(),
                )
            } else {
                (false, "dark".to_string(), "Media: Idle".to_string(), None)
            };

        let play_target = active_player.clone();
        let next_target = active_player.clone();

        vec![
            StandardItem {
                label: "Open HawaOS Companion".into(),
                activate: Box::new(|this: &mut Self| {
                    if let Ok(mut g) = this.state.lock() {
                        g.open_window_requested = true;
                    }
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: track_summary,
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Play / Pause Media".into(),
                activate: Box::new(move |_| {
                    handle_device_command("play_pause", play_target.as_deref())
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next Track".into(),
                activate: Box::new(move |_| {
                    handle_device_command("next", next_target.as_deref())
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: format!(
                    "Switch Device Theme (Current: {})",
                    device_theme.to_uppercase()
                ),
                activate: Box::new(|this: &mut Self| {
                    if let Ok(mut g) = this.state.lock() {
                        g.config.device_theme = if g.config.device_theme == "dark" {
                            "light".to_string()
                        } else {
                            "dark".to_string()
                        };
                        g.config.config_version += 1;
                        save_config(&g.config);
                        g.outbound_queue.push_back(OutboundAction::PushConfig {
                            wifi_ssid: None,
                            wifi_pass: None,
                            forget_wifi: false,
                            force_usb: false,
                        });
                    }
                }),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Start Companion with System".into(),
                checked: autostart,
                activate: Box::new(|this: &mut Self| {
                    if let Ok(mut g) = this.state.lock() {
                        let next = !g.config.autostart_enabled;
                        if set_autostart_installed(next).is_ok() {
                            g.config.autostart_enabled = next;
                            save_config(&g.config);
                        }
                    }
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit HawaOS Daemon".into(),
                activate: Box::new(|this: &mut Self| {
                    if let Ok(mut g) = this.state.lock() {
                        g.quit_requested = true;
                    }
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn spawn_system_tray(state: SharedState) {
    let tray = HawaTray {
        state,
        icon_pixmap: build_tray_icons(),
    };
    let _ = tray.assume_sni_available(true).spawn();
}

// ============================================================================
// Background Telemetry + UDP + USB Serial Worker
// ============================================================================

fn handle_incoming_json_line(line: &str, state: &SharedState) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }

    // Check if serial log line reports WiFi connection IP
    if let Some(idx) = trimmed.find("Connected! IP=") {
        let ip_part = trimmed[idx + "Connected! IP=".len()..].trim();
        let ip_clean = sanitize_ip_str(ip_part);
        if !ip_clean.is_empty() && ip_clean != "0.0.0.0" {
            if let Ok(mut g) = state.lock() {
                g.device_reported_ip = Some(ip_clean.clone());
                g.device_wifi_connected = Some(true);
                g.config.target_ip = ip_clean.clone();
                save_config(&g.config);
                g.push_log(format!(
                    "Device connected to WiFi! Assigned IP: {}",
                    ip_clean
                ));
            }
        }
        return;
    }

    if !trimmed.starts_with('{') {
        return;
    }

    if let Ok(pkt) = serde_json::from_str::<IncomingPacket>(trimmed) {
        if pkt.msg_type == "cmd" {
            if let Some(cmd) = pkt.cmd {
                let player_opt = state.lock().ok().and_then(|g| g.active_player.clone());
                handle_device_command(&cmd, player_opt.as_deref());
                if let Ok(mut g) = state.lock() {
                    let focused = g.active_player.clone();
                    g.push_log(format!(
                        "Executed media cmd '{}' (focused: {:?})",
                        cmd, focused
                    ));
                }
            }
        } else if pkt.msg_type == "config_ack" {
            if let Ok(mut g) = state.lock() {
                g.last_ack_version = pkt.v;
                g.last_ack_instant = Some(Instant::now());
                if let Some(ip) = pkt.ip {
                    let ip_clean = sanitize_ip_str(&ip);
                    if !ip_clean.is_empty() && ip_clean != "0.0.0.0" {
                        g.device_reported_ip = Some(ip_clean.clone());
                        g.config.target_ip = ip_clean;
                        save_config(&g.config);
                    }
                }
                if let Some(w) = pkt.wifi {
                    g.device_wifi_connected = Some(w);
                }
                if let Some(ssid) = pkt.ssid {
                    if !ssid.is_empty() {
                        g.config.wifi_ssid = ssid;
                    }
                }
                if let Some(th) = pkt.theme {
                    g.config.device_theme = th;
                }
                if let Some(br) = pkt.brightness {
                    g.config.brightness = br;
                }
                if let Some(aod) = pkt.aod {
                    g.config.aod_enabled = aod;
                }
                if let Some(timeout) = pkt.aod_timeout_s {
                    g.config.aod_timeout_s = timeout;
                }
                if let Some(rm) = pkt.reduce_motion {
                    g.config.reduce_motion = rm;
                }
                if let Some(wp) = pkt.wallpaper {
                    g.config.wallpaper = wp;
                }
                if let Some(u24) = pkt.use_24h {
                    g.config.use_24h = u24;
                }
                if let Some(np) = pkt.notify_popup {
                    g.config.notify_popup = np;
                }
                let ack_ip = g
                    .device_reported_ip
                    .clone()
                    .unwrap_or_else(|| g.config.target_ip.clone());
                let log_msg = format!(
                    "Device ACK v={} (IP: {}, theme: {}, brightness: {}%)",
                    pkt.v.unwrap_or(0),
                    ack_ip,
                    g.config.device_theme,
                    g.config.brightness
                );
                g.push_log(log_msg);
            }
        }
    }
}

fn send_usb_line(port_name: &str, json_line: &str, state: &SharedState) -> bool {
    match serialport::new(port_name, USB_BAUD)
        .timeout(Duration::from_millis(350))
        .open()
    {
        Ok(mut port) => {
            let payload = format!("{}\n", json_line);
            if port.write_all(payload.as_bytes()).is_ok() {
                let _ = port.flush();
                let mut reader = BufReader::new(port);
                let start = Instant::now();
                while start.elapsed() < Duration::from_millis(350) {
                    let mut line = String::new();
                    if let Ok(n) = reader.read_line(&mut line) {
                        if n > 0 {
                            handle_incoming_json_line(&line, state);
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                true
            } else {
                false
            }
        }
        Err(e) => {
            if let Ok(mut g) = state.lock() {
                g.push_log(format!("USB serial {} error: {}", port_name, e));
            }
            false
        }
    }
}

fn spawn_daemon_worker(state: SharedState, art_cache: SharedArtCache) {
    thread::spawn(move || {
        let udp = UdpSocket::bind("0.0.0.0:0").expect("Failed to bind local UDP socket");
        let _ = udp.set_nonblocking(true);

        if let Ok(mut g) = state.lock() {
            g.outbound_queue
                .push_back(OutboundAction::QueryDevice { via_usb: false });
            let msg = format!(
                "Daemon started · UDP -> {}:{} · Art HTTP :{}",
                g.config.target_ip, UDP_PORT, ART_HTTP_PORT
            );
            g.push_log(msg);
        }

        let mut sys = System::new_all();
        let mut networks = Networks::new_with_refreshed_list();
        let mut last_stats_tick = Instant::now() - Duration::from_secs(1);
        let mut last_media_heartbeat = Instant::now() - Duration::from_secs(10);
        let mut prev_media: Option<MediaMsg> = None;
        let mut rx_buf = [0u8; 1460];

        loop {
            // 1. Poll incoming UDP packets from T-Display-S3
            while let Ok((len, src)) = udp.recv_from(&mut rx_buf) {
                if let Ok(s) = std::str::from_utf8(&rx_buf[..len]) {
                    if let Ok(mut g) = state.lock() {
                        if g.device_reported_ip.is_none() {
                            g.device_reported_ip = Some(src.ip().to_string());
                        }
                    }
                    handle_incoming_json_line(s, &state);
                }
            }

            // 2. Process queued outbound actions
            let actions: Vec<(OutboundAction, CompanionConfig)> = if let Ok(mut g) = state.lock() {
                let cfg = g.config.clone();
                let mut drained = Vec::new();
                while let Some(act) = g.outbound_queue.pop_front() {
                    drained.push((act, cfg.clone()));
                }
                drained
            } else {
                Vec::new()
            };

            for (act, cfg) in actions {
                let clean_ip = sanitize_ip_str(&cfg.target_ip);
                let target_addr: Option<SocketAddr> =
                    format!("{}:{}", clean_ip, UDP_PORT)
                        .parse()
                        .ok();

                match act {
                    OutboundAction::PushConfig {
                        wifi_ssid,
                        wifi_pass,
                        forget_wifi,
                        force_usb,
                    } => {
                        let mut obj = serde_json::json!({
                            "type": "config",
                            "v": cfg.config_version,
                            "theme": cfg.device_theme,
                            "brightness": cfg.brightness,
                            "aod": cfg.aod_enabled,
                            "aod_timeout_s": cfg.aod_timeout_s,
                            "reduce_motion": cfg.reduce_motion,
                            "wallpaper": cfg.wallpaper,
                            "use_24h": cfg.use_24h,
                            "notify_popup": cfg.notify_popup,
                        });
                        if forget_wifi {
                            obj["forget_wifi"] = serde_json::Value::Bool(true);
                        } else if let (Some(ssid), Some(pass)) = (wifi_ssid.as_ref(), wifi_pass.as_ref()) {
                            if !ssid.is_empty() {
                                obj["wifi_ssid"] = serde_json::Value::String(ssid.clone());
                                obj["wifi_pass"] = serde_json::Value::String(pass.clone());
                            }
                        }
                        if let Ok(json_str) = serde_json::to_string(&obj) {
                            if force_usb || cfg.stream_via_usb {
                                let ok = send_usb_line(&cfg.usb_port, &json_str, &state);
                                if ok && wifi_ssid.is_some() {
                                    if let Ok(port) = serialport::new(&cfg.usb_port, USB_BAUD)
                                        .timeout(Duration::from_millis(500))
                                        .open()
                                    {
                                        let mut reader = BufReader::new(port);
                                        let wait_start = Instant::now();
                                        while wait_start.elapsed() < Duration::from_secs(6) {
                                            let mut line = String::new();
                                            if let Ok(n) = reader.read_line(&mut line) {
                                                if n > 0 {
                                                    handle_incoming_json_line(&line, &state);
                                                    if line.contains("Connected! IP=") {
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if let Some(addr) = target_addr {
                                let _ = udp.send_to(json_str.as_bytes(), addr);
                            }
                        }
                    }
                    OutboundAction::SendNotification { title, body } => {
                        let ts = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let pkt = serde_json::json!({
                            "type": "notify",
                            "title": sanitize_for_font(&title),
                            "body": sanitize_for_font(&body),
                            "ts": ts
                        });
                        if let Ok(json_str) = serde_json::to_string(&pkt) {
                            if let Some(addr) = target_addr {
                                let _ = udp.send_to(json_str.as_bytes(), addr);
                            }
                            if cfg.stream_via_usb {
                                let _ = send_usb_line(&cfg.usb_port, &json_str, &state);
                            }
                        }
                    }
                    OutboundAction::QueryDevice { via_usb } => {
                        let pkt = r#"{"type":"query"}"#;
                        if via_usb {
                            let _ = send_usb_line(&cfg.usb_port, pkt, &state);
                        } else if let Some(addr) = target_addr {
                            let _ = udp.send_to(pkt.as_bytes(), addr);
                        }
                    }
                }
            }

            // 3. 1 Hz Telemetry + Media tick
            if last_stats_tick.elapsed() >= Duration::from_secs(1) {
                let dt = last_stats_tick.elapsed().as_secs_f64().max(0.1);
                last_stats_tick = Instant::now();

                sys.refresh_cpu_usage();
                sys.refresh_memory();
                networks.refresh(true);

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

                let (cur_media, cur_art_img, cur_player) = query_mpris_media(&art_cache);

                let (target_ip, stream_usb, usb_port) = if let Ok(mut g) = state.lock() {
                    g.cpu_pct = cpu_pct;
                    g.ram_pct = ram_pct;
                    g.used_ram_gb = (used_mem as f32) / (1024.0 * 1024.0 * 1024.0);
                    g.total_ram_gb = (total_mem as f32) / (1024.0 * 1024.0 * 1024.0);
                    g.net_up_kbps = net_up_kbps;
                    g.net_down_kbps = net_down_kbps;
                    if g.cpu_history.len() >= 30 {
                        g.cpu_history.pop_front();
                    }
                    g.cpu_history.push_back(cpu_pct);
                    g.media = cur_media.clone();
                    g.current_art_png = cur_art_img;
                    g.active_player = cur_player;
                    (
                        g.config.target_ip.clone(),
                        g.config.stream_via_usb,
                        g.config.usb_port.clone(),
                    )
                } else {
                    (DEFAULT_DEVICE_IP.to_string(), false, String::new())
                };

                let clean_ip = sanitize_ip_str(&target_ip);
                let target_addr: Option<SocketAddr> =
                    format!("{}:{}", clean_ip, UDP_PORT).parse().ok();

                let local_ip_str = get_local_lan_ip();
                let stats_msg = StatsMsg {
                    msg_type: "stats",
                    ts,
                    cpu: cpu_pct,
                    ram: ram_pct,
                    net_up_kbps,
                    net_down_kbps,
                    host_ip: if local_ip_str.is_empty() {
                        None
                    } else {
                        Some(&local_ip_str)
                    },
                };

                if let Ok(json_str) = serde_json::to_string(&stats_msg) {
                    if let Some(addr) = target_addr {
                        let _ = udp.send_to(json_str.as_bytes(), addr);
                    }
                    if stream_usb {
                        let _ = send_usb_line(&usb_port, &json_str, &state);
                    }
                }

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
                    if let Ok(json_str) = serde_json::to_string(&cur_media) {
                        if let Some(addr) = target_addr {
                            let _ = udp.send_to(json_str.as_bytes(), addr);
                        }
                        if stream_usb {
                            let _ = send_usb_line(&usb_port, &json_str, &state);
                        }
                    }
                    prev_media = Some(cur_media);
                    last_media_heartbeat = Instant::now();
                }
            }

            thread::sleep(Duration::from_millis(50));
        }
    });
}

// ============================================================================
// Official HawaOS Theme Tokens (DESIGN.md Section 3)
// ============================================================================

#[derive(Clone, Copy)]
struct HawaPalette {
    bg: Rgba,
    surface: Rgba,
    surface2: Rgba,
    border: Rgba,
    text: Rgba,
    text_dim: Rgba,
    text_muted: Rgba,
    primary: Rgba,
    accent: Rgba,
    on_primary: Rgba,
    success: Rgba,
    warn: Rgba,
    danger: Rgba,
}

impl HawaPalette {
    fn dark() -> Self {
        Self {
            bg: rgb(0x071115),
            surface: rgb(0x101d22),
            surface2: rgb(0x1b292f),
            border: rgb(0x28353b),
            text: rgb(0xebf4f6),
            text_dim: rgb(0x98a8ad),
            text_muted: rgb(0x5b6d73),
            primary: rgb(0x00b6d4),
            accent: rgb(0x00d1ec),
            on_primary: rgb(0x031b20),
            success: rgb(0x22c55e),
            warn: rgb(0xf59e0b),
            danger: rgb(0xef4444),
        }
    }

    fn light() -> Self {
        Self {
            bg: rgb(0xf2f8f9),
            surface: rgb(0xfafcfd),
            surface2: rgb(0xe3eff2),
            border: rgb(0xcbdce0),
            text: rgb(0x0b171b),
            text_dim: rgb(0x4a5d63),
            text_muted: rgb(0x7e939a),
            primary: rgb(0x008da6),
            accent: rgb(0x00b6d4),
            on_primary: rgb(0xfafcfd),
            success: rgb(0x16a34a),
            warn: rgb(0xd97706),
            danger: rgb(0xdc2626),
        }
    }
}

fn apply_hawaos_gpui_theme(dark: bool, cx: &mut App) {
    let mode = if dark {
        GpuiThemeMode::Dark
    } else {
        GpuiThemeMode::Light
    };
    Theme::change(mode, None, cx);
    let pal = if dark {
        HawaPalette::dark()
    } else {
        HawaPalette::light()
    };

    Theme::update(cx, |theme| {
        theme.radius = px(10.);
        theme.colors.background = pal.bg.into();
        theme.colors.foreground = pal.text.into();
        theme.colors.border = pal.border.into();
        theme.colors.input = pal.border.into();
        theme.colors.ring = pal.accent.into();
        theme.colors.caret = pal.accent.into();

        theme.colors.primary = pal.primary.into();
        theme.colors.primary_hover = pal.accent.into();
        theme.colors.primary_active = pal.primary.into();
        theme.colors.primary_foreground = pal.on_primary.into();

        theme.colors.secondary = pal.surface2.into();
        theme.colors.secondary_hover = pal.border.into();
        theme.colors.secondary_active = pal.surface.into();
        theme.colors.secondary_foreground = pal.text.into();

        theme.colors.muted = pal.surface2.into();
        theme.colors.muted_foreground = pal.text_dim.into();

        theme.colors.accent = pal.surface2.into();
        theme.colors.accent_foreground = pal.text.into();

        theme.colors.sidebar = pal.surface.into();
        theme.colors.sidebar_foreground = pal.text.into();
        theme.colors.sidebar_border = pal.border.into();
        theme.colors.sidebar_primary = pal.primary.into();
        theme.colors.sidebar_primary_foreground = pal.on_primary.into();

        theme.colors.switch = pal.surface2.into();
        theme.colors.slider_bar = pal.primary.into();
        theme.colors.progress_bar = pal.primary.into();

        theme.colors.success = pal.success.into();
        theme.colors.warning = pal.warn.into();
        theme.colors.danger = pal.danger.into();
    });
}

// ============================================================================
// GPUI-Kit Desktop Application View
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum NavSection {
    Overview,
    DeviceSettings,
    InitialSetup,
    DaemonSettings,
}

struct CompanionWindowView {
    state: SharedState,
    active_section: NavSection,
    logo_image: Arc<Image>,
    ip_input: Entity<InputState>,
    ssid_input: Entity<InputState>,
    pass_input: Entity<InputState>,
    notify_title_input: Entity<InputState>,
    notify_body_input: Entity<InputState>,
    status_banner: Option<String>,
}

impl CompanionWindowView {
    fn new(window: &mut Window, cx: &mut Context<Self>, state: SharedState) -> Self {
        let (target_ip, wifi_ssid) = if let Ok(g) = state.lock() {
            (g.config.target_ip.clone(), g.config.wifi_ssid.clone())
        } else {
            (DEFAULT_DEVICE_IP.to_string(), String::new())
        };

        let ip_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("e.g. 192.168.0.98")
                .default_value(target_ip)
        });
        let ssid_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("WiFi Network SSID (2.4 GHz)")
                .default_value(wifi_ssid)
        });
        let pass_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("WiFi Password")
                .masked(true)
        });
        let notify_title_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Notification title")
                .default_value("HawaOS Desktop")
        });
        let notify_body_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Message to display on T-Display-S3...")
                .default_value("Breezy Elegance, Materialised.")
        });

        // Pre-resize logo to 96x96 via Lanczos3 for crisp rendering without pixelation/aliasing
        let logo_image = if let Ok(img) =
            image::load_from_memory_with_format(LOGO_PNG_BYTES, image::ImageFormat::Png)
        {
            let resized = img.resize_exact(96, 96, image::imageops::FilterType::Lanczos3);
            let mut buf = Vec::new();
            let _ = resized.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png);
            Arc::new(Image::from_bytes(ImageFormat::Png, buf))
        } else {
            Arc::new(Image::from_bytes(ImageFormat::Png, LOGO_PNG_BYTES.to_vec()))
        };

        Self {
            state,
            active_section: NavSection::Overview,
            logo_image,
            ip_input,
            ssid_input,
            pass_input,
            notify_title_input,
            notify_body_input,
            status_banner: None,
        }
    }

    fn mutate_config_and_sync(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut CompanionConfig),
    ) {
        if let Ok(mut g) = self.state.lock() {
            f(&mut g.config);
            g.config.config_version += 1;
            save_config(&g.config);
            if g.config.auto_sync_settings {
                g.outbound_queue.push_back(OutboundAction::PushConfig {
                    wifi_ssid: None,
                    wifi_pass: None,
                    forget_wifi: false,
                    force_usb: false,
                });
            }
        }
        cx.notify();
    }

    fn render_sidebar(&mut self, pal: HawaPalette, cx: &mut Context<Self>) -> impl IntoElement {
        let (target_ip, ack_ver, ack_recent, stream_usb, usb_port, dark_ui) =
            if let Ok(g) = self.state.lock() {
                let recent = g
                    .last_ack_instant
                    .map(|t| t.elapsed() < Duration::from_secs(120))
                    .unwrap_or(false);
                (
                    g.config.target_ip.clone(),
                    g.last_ack_version,
                    recent,
                    g.config.stream_via_usb,
                    g.config.usb_port.clone(),
                    g.config.companion_dark_mode,
                )
            } else {
                (
                    DEFAULT_DEVICE_IP.to_string(),
                    None,
                    false,
                    false,
                    "/dev/ttyACM0".to_string(),
                    true,
                )
            };

        let nav_item = |id: &'static str,
                        icon: IconName,
                        label: &'static str,
                        section: NavSection,
                        active: NavSection,
                        cx: &mut Context<Self>| {
            let is_active = active == section;
            let bg_color = if is_active {
                pal.primary.opacity(0.16)
            } else {
                pal.primary.opacity(0.0)
            };
            let fg_color = if is_active { pal.accent } else { pal.text_dim };
            let border_color = if is_active {
                pal.primary.opacity(0.45)
            } else {
                pal.primary.opacity(0.0)
            };

            div()
                .id(id)
                .cursor_pointer()
                .w_full()
                .px_3()
                .py_2p5()
                .rounded(px(10.))
                .bg(bg_color)
                .border_1()
                .border_color(border_color)
                .hover(|s| s.bg(pal.surface2))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active_section = section;
                    cx.notify();
                }))
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(div().text_color(fg_color).child(icon))
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(if is_active { pal.text } else { pal.text_dim })
                                .child(label),
                        ),
                )
        };

        let active = self.active_section;

        v_flex()
            .w(px(252.))
            .h_full()
            .flex_shrink_0()
            .bg(pal.surface)
            .border_r_1()
            .border_color(pal.border)
            .p_4()
            .justify_between()
            .child(
                v_flex()
                    .gap_5()
                    .child(
                        // Brand Header
                        h_flex()
                            .gap_3()
                            .items_center()
                            .p_2()
                            .rounded(px(12.))
                            .bg(pal.surface2.opacity(0.6))
                            .border_1()
                            .border_color(pal.border)
                            .child(
                                img(Arc::clone(&self.logo_image))
                                    .w(px(40.))
                                    .h(px(40.))
                                    .rounded(px(8.)),
                            )
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_base()
                                                    .font_bold()
                                                    .text_color(pal.text)
                                                    .child("HawaOS"),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded(px(6.))
                                                    .bg(pal.primary.opacity(0.2))
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.accent)
                                                    .child("v0.4"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Breezy Elegance"),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(
                                div()
                                    .px_2()
                                    .pb_1()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(pal.text_muted)
                                    .child("COMPANION MENU"),
                            )
                            .child(nav_item(
                                "nav-overview",
                                IconName::Activity,
                                "Overview & Live",
                                NavSection::Overview,
                                active,
                                cx,
                            ))
                            .child(nav_item(
                                "nav-device-settings",
                                IconName::SlidersHorizontal,
                                "Device Settings",
                                NavSection::DeviceSettings,
                                active,
                                cx,
                            ))
                            .child(nav_item(
                                "nav-initial-setup",
                                IconName::Usb,
                                "Initial Setup & WiFi",
                                NavSection::InitialSetup,
                                active,
                                cx,
                            ))
                            .child(nav_item(
                                "nav-daemon-settings",
                                IconName::Settings,
                                "System Tray & Startup",
                                NavSection::DaemonSettings,
                                active,
                                cx,
                            )),
                    ),
            )
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        // Device Connection Status Card
                        v_flex()
                            .gap_2()
                            .p_3()
                            .rounded(px(12.))
                            .bg(pal.bg)
                            .border_1()
                            .border_color(pal.border)
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(
                                                if ack_recent { pal.success } else { pal.primary },
                                            ))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.text)
                                                    .child("T-Display-S3 Link"),
                                            ),
                                    )
                                    .child(div().text_xs().text_color(pal.accent).child(
                                        match ack_ver {
                                            Some(v) => format!("ACK v{}", v),
                                            None => "UDP Active".to_string(),
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child(format!("WiFi UDP: {}:{}", target_ip, UDP_PORT)),
                            )
                            .child(div().text_xs().text_color(pal.text_muted).child(
                                if stream_usb {
                                    format!("USB Serial: {} (On)", usb_port)
                                } else {
                                    format!("Art Server: :{}", ART_HTTP_PORT)
                                },
                            )),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .px_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child("App Appearance"),
                            )
                            .child(
                                Button::new("toggle-companion-theme")
                                    .small()
                                    .secondary()
                                    .icon(if dark_ui {
                                        IconName::Moon
                                    } else {
                                        IconName::Sun
                                    })
                                    .label(if dark_ui { "Dark" } else { "Light" })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let next_dark = if let Ok(mut g) = this.state.lock() {
                                            g.config.companion_dark_mode =
                                                !g.config.companion_dark_mode;
                                            save_config(&g.config);
                                            g.config.companion_dark_mode
                                        } else {
                                            true
                                        };
                                        apply_hawaos_gpui_theme(next_dark, cx);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }

    fn render_overview(&mut self, pal: HawaPalette, cx: &mut Context<Self>) -> impl IntoElement {
        let (
            cpu,
            ram,
            used_gb,
            total_gb,
            up_kbps,
            down_kbps,
            hist,
            media,
            art_img,
            cfg,
            reported_ip,
            active_player,
        ) = if let Ok(g) = self.state.lock() {
            (
                g.cpu_pct,
                g.ram_pct,
                g.used_ram_gb,
                g.total_ram_gb,
                g.net_up_kbps,
                g.net_down_kbps,
                g.cpu_history.iter().copied().collect::<Vec<u8>>(),
                g.media.clone(),
                g.current_art_png.clone(),
                g.config.clone(),
                g.device_reported_ip.clone(),
                g.active_player.clone(),
            )
        } else {
            (
                0,
                0,
                0.0,
                1.0,
                0,
                0,
                vec![0; 30],
                MediaMsg::default(),
                None,
                CompanionConfig::default(),
                None,
                None,
            )
        };

        let progress_pct = if media.dur_ms > 0 {
            ((media.pos_ms as f32 / media.dur_ms as f32) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        let fmt_ms = |ms: u32| {
            let total_s = ms / 1000;
            format!("{}:{:02}", total_s / 60, total_s % 60)
        };

        let player_label = match &active_player {
            Some(p) => format!("NOW PLAYING ({} -> DEVICE)", p.to_uppercase()),
            None => "NOW PLAYING (MPRIS -> DEVICE)".to_string(),
        };

        let p_prev = active_player.clone();
        let p_play = active_player.clone();
        let p_next = active_player.clone();

        v_flex()
            .gap_5()
            .w_full()
            .min_w_0()
            .child(
                // Top Page Title Row
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xl()
                                    .font_bold()
                                    .text_color(pal.text)
                                    .child("Live Telemetry & Media Dashboard"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(pal.text_dim)
                                    .child("Streaming real-time PC metrics, MPRIS album art & notifications to your LilyGo T-Display-S3"),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("overview-ping-btn")
                                    .secondary()
                                    .icon(IconName::RefreshCw)
                                    .label("Sync Status")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            g.outbound_queue.push_back(
                                                OutboundAction::QueryDevice { via_usb: false },
                                            );
                                        }
                                        this.status_banner =
                                            Some("Sent status query to T-Display-S3.".into());
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("overview-push-cfg-btn")
                                    .primary()
                                    .icon(IconName::Send)
                                    .label("Push Config")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            g.config.config_version += 1;
                                            save_config(&g.config);
                                            g.outbound_queue.push_back(
                                                OutboundAction::PushConfig {
                                                    wifi_ssid: None,
                                                    wifi_pass: None,
                                                    forget_wifi: false,
                                                    force_usb: false,
                                                },
                                            );
                                        }
                                        this.status_banner =
                                            Some("Pushed active configuration to device.".into());
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // 3 Metric Cards Row (CPU, RAM, Network)
            .child(
                h_flex()
                    .gap_4()
                    .w_full()
                    .child(
                        // CPU Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .gap_3()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_color(pal.accent)
                                                    .child(IconName::Cpu),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.text_dim)
                                                    .child("CPU LOAD"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child(format!("{}%", cpu)),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(8.))
                                    .rounded_full()
                                    .bg(pal.surface2)
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative((cpu as f32) / 100.0))
                                            .rounded_full()
                                            .bg(if cpu > 85 {
                                                pal.danger
                                            } else if cpu > 65 {
                                                pal.warn
                                            } else {
                                                pal.primary
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .h(px(28.))
                                    .items_end()
                                    .children(hist.into_iter().map(|val| {
                                        let h_frac = ((val as f32) / 100.0).clamp(0.12, 1.0);
                                        div()
                                            .flex_1()
                                            .h(relative(h_frac))
                                            .rounded(px(2.))
                                            .bg(pal.primary.opacity(0.65))
                                    })),
                            ),
                    )
                    .child(
                        // RAM Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .gap_3()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_color(pal.accent)
                                                    .child(IconName::HardDrive),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.text_dim)
                                                    .child("MEMORY"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child(format!("{}%", ram)),
                                    ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .h(px(8.))
                                    .rounded_full()
                                    .bg(pal.surface2)
                                    .overflow_hidden()
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative((ram as f32) / 100.0))
                                            .rounded_full()
                                            .bg(pal.accent),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child(format!(
                                        "{:.1} GB / {:.1} GB used",
                                        used_gb, total_gb
                                    )),
                            ),
                    )
                    .child(
                        // Network Throughput Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .gap_3()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_color(pal.accent)
                                                    .child(IconName::Wifi),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.text_dim)
                                                    .child("NETWORK I/O"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded(px(6.))
                                            .bg(pal.primary.opacity(0.18))
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(pal.accent)
                                            .child("1 Hz UDP"),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_color(pal.success)
                                                    .child(IconName::ArrowDown),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .text_color(pal.text)
                                                    .child(format!("{} KB/s", down_kbps)),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_color(pal.accent)
                                                    .child(IconName::ArrowUp),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .text_color(pal.text)
                                                    .child(format!("{} KB/s", up_kbps)),
                                            ),
                                    ),
                            )
                            .child(
                                div().text_xs().text_color(pal.text_dim).child(format!(
                                    "Device IP: {}",
                                    reported_ip.unwrap_or_else(|| cfg.target_ip.clone())
                                )),
                            ),
                    ),
            )
            // Middle Row: Now Playing Card + Quick Notification Dispatcher
            .child(
                h_flex()
                    .gap_4()
                    .w_full()
                    .items_stretch()
                    .child(
                        // Spotify-style Now Playing Preview Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .justify_between()
                            .gap_4()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_color(pal.accent)
                                                    .child(IconName::Music),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.text_dim)
                                                    .child(player_label),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded(px(6.))
                                            .bg(pal.surface2)
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child(media.state.to_uppercase()),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_4()
                                    .items_center()
                                    .child(if let Some(art) = art_img {
                                        div()
                                            .w(px(76.))
                                            .h(px(76.))
                                            .rounded(px(12.))
                                            .overflow_hidden()
                                            .border_1()
                                            .border_color(pal.border)
                                            .child(img(art).w(px(76.)).h(px(76.)))
                                            .into_any_element()
                                    } else {
                                        h_flex()
                                            .w(px(76.))
                                            .h(px(76.))
                                            .rounded(px(12.))
                                            .bg(pal.surface2)
                                            .border_1()
                                            .border_color(pal.border)
                                            .items_center()
                                            .justify_center()
                                            .text_color(pal.accent)
                                            .child(IconName::Music)
                                            .into_any_element()
                                    })
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .gap_1()
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .text_base()
                                                    .font_bold()
                                                    .text_color(pal.text)
                                                    .child(if media.title.is_empty() {
                                                        "No Active Track".to_string()
                                                    } else {
                                                        media.title.clone()
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(pal.text_dim)
                                                    .child(if media.artist.is_empty() {
                                                        "Play audio in Spotify, Sonora, Firefox, or any MPRIS player".to_string()
                                                    } else {
                                                        media.artist.clone()
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(pal.text_muted)
                                                    .child(format!(
                                                        "Tint: {} -> {} · Art hash: {}",
                                                        media.bg_top,
                                                        media.bg_bottom,
                                                        if media.art.is_empty() {
                                                            "none"
                                                        } else {
                                                            &media.art
                                                        }
                                                    )),
                                            ),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w_full()
                                            .h(px(6.))
                                            .rounded_full()
                                            .bg(pal.surface2)
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .h_full()
                                                    .w(relative(progress_pct / 100.0))
                                                    .rounded_full()
                                                    .bg(pal.primary),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(pal.text_dim)
                                                    .child(fmt_ms(media.pos_ms)),
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .child(
                                                        Button::new("media-prev")
                                                            .small()
                                                            .secondary()
                                                            .icon(IconName::SkipBack)
                                                            .on_click(move |_, _, _| {
                                                                handle_device_command("prev", p_prev.as_deref())
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new("media-play-pause")
                                                            .small()
                                                            .primary()
                                                            .icon(if media.state == "playing" {
                                                                IconName::Pause
                                                            } else {
                                                                IconName::Play
                                                            })
                                                            .label(if media.state == "playing" {
                                                                "Pause"
                                                            } else {
                                                                "Play"
                                                            })
                                                            .on_click(move |_, _, _| {
                                                                handle_device_command("play_pause", p_play.as_deref())
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new("media-next")
                                                            .small()
                                                            .secondary()
                                                            .icon(IconName::SkipForward)
                                                            .on_click(move |_, _, _| {
                                                                handle_device_command("next", p_next.as_deref())
                                                            }),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(pal.text_dim)
                                                    .child(fmt_ms(media.dur_ms)),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        // Push Notification to T-Display-S3 Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .justify_between()
                            .gap_3()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Bell))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(pal.text_dim)
                                            .child("PUSH NOTIFICATION TO DEVICE"),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Title"),
                                    )
                                    .child(Input::new(&self.notify_title_input).cleanable(true))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Message Body"),
                                    )
                                    .child(Input::new(&self.notify_body_input).cleanable(true)),
                            )
                            .child(
                                h_flex()
                                    .justify_end()
                                    .child(
                                        Button::new("send-notify-btn")
                                            .primary()
                                            .icon(IconName::Send)
                                            .label("Send to T-Display-S3")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let title =
                                                    this.notify_title_input.read(cx).value().to_string();
                                                let body =
                                                    this.notify_body_input.read(cx).value().to_string();
                                                if !title.trim().is_empty() {
                                                    if let Ok(mut g) = this.state.lock() {
                                                        g.outbound_queue.push_back(
                                                            OutboundAction::SendNotification {
                                                                title: title.clone(),
                                                                body,
                                                            },
                                                        );
                                                        g.push_log(format!(
                                                            "Sent notification '{}' to device",
                                                            title
                                                        ));
                                                    }
                                                    this.status_banner = Some(format!(
                                                        "Notification '{}' dispatched to device.",
                                                        title
                                                    ));
                                                    cx.notify();
                                                }
                                            })),
                                    ),
                            ),
                    ),
            )
            // Bottom Row: Quick Device Controls Summary
            .child(
                h_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .justify_between()
                    .items_center()
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(pal.text)
                                    .child("Quick Device Toggles"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child(format!(
                                        "Theme: {} · Brightness: {}% · AOD: {} ({}s) · Clock: {} · Popups: {}",
                                        cfg.device_theme.to_uppercase(),
                                        cfg.brightness,
                                        if cfg.aod_enabled { "On" } else { "Off" },
                                        cfg.aod_timeout_s,
                                        if cfg.use_24h { "24h" } else { "12h AM/PM" },
                                        if cfg.notify_popup { "On" } else { "Off" }
                                    )),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("quick-theme-dark")
                                    .small()
                                    .with_variant(if cfg.device_theme == "dark" {
                                        gpui_kit::component::button::ButtonVariant::Primary
                                    } else {
                                        gpui_kit::component::button::ButtonVariant::Secondary
                                    })
                                    .icon(IconName::Moon)
                                    .label("Dark")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| {
                                            c.device_theme = "dark".into();
                                        });
                                    })),
                            )
                            .child(
                                Button::new("quick-theme-light")
                                    .small()
                                    .with_variant(if cfg.device_theme == "light" {
                                        gpui_kit::component::button::ButtonVariant::Primary
                                    } else {
                                        gpui_kit::component::button::ButtonVariant::Secondary
                                    })
                                    .icon(IconName::Sun)
                                    .label("Light")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| {
                                            c.device_theme = "light".into();
                                        });
                                    })),
                            )
                            .child(
                                Button::new("quick-clock-fmt")
                                    .small()
                                    .secondary()
                                    .icon(IconName::Clock)
                                    .label(if cfg.use_24h { "24-Hour" } else { "12-Hour" })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| {
                                            c.use_24h = !c.use_24h;
                                        });
                                    })),
                            ),
                    ),
            )
    }

    fn render_device_settings(
        &mut self,
        pal: HawaPalette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let cfg = if let Ok(g) = self.state.lock() {
            g.config.clone()
        } else {
            CompanionConfig::default()
        };

        let brightness_steps: [u8; 5] = [20, 40, 60, 80, 100];
        let aod_timeouts: [u16; 5] = [15, 30, 60, 120, 300];

        v_flex()
            .gap_4()
            .w_full()
            .min_w_0()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xl()
                                    .font_bold()
                                    .text_color(pal.text)
                                    .child("Device Appearance & Behavior"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(pal.text_dim)
                                    .child("Configure your LilyGo T-Display-S3 in real time (persisted to ESP32-S3 NVS Flash)"),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("pull-device-cfg")
                                    .secondary()
                                    .icon(IconName::RefreshCw)
                                    .label("Read from Device")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            let via_usb = g.config.stream_via_usb;
                                            g.outbound_queue.push_back(
                                                OutboundAction::QueryDevice { via_usb },
                                            );
                                        }
                                        this.status_banner = Some(
                                            "Requested current configuration from T-Display-S3."
                                                .into(),
                                        );
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("push-device-cfg-main")
                                    .primary()
                                    .icon(IconName::Check)
                                    .label("Apply & Push to Device")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            g.config.config_version += 1;
                                            save_config(&g.config);
                                            g.outbound_queue.push_back(
                                                OutboundAction::PushConfig {
                                                    wifi_ssid: None,
                                                    wifi_pass: None,
                                                    forget_wifi: false,
                                                    force_usb: false,
                                                },
                                            );
                                        }
                                        this.status_banner = Some(
                                            "Configuration pushed to T-Display-S3.".into(),
                                        );
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Theme + Wallpaper Row
            .child(
                h_flex()
                    .gap_4()
                    .w_full()
                    .child(
                        // Device Theme Mode Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .gap_3()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Palette))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("HawaOS Device Theme"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child("Switch between Deep Cyan-Slate Dark, Crisp Daylight Light, or Auto"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("theme-dark-btn")
                                            .with_variant(if cfg.device_theme == "dark" {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .icon(IconName::Moon)
                                            .label("Dark")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.device_theme = "dark".into();
                                                });
                                            })),
                                    )
                                    .child(
                                        Button::new("theme-light-btn")
                                            .with_variant(if cfg.device_theme == "light" {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .icon(IconName::Sun)
                                            .label("Light")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.device_theme = "light".into();
                                                });
                                            })),
                                    )
                                    .child(
                                        Button::new("theme-auto-btn")
                                            .with_variant(if cfg.device_theme == "auto" {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .icon(IconName::Sparkles)
                                            .label("Auto")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.device_theme = "auto".into();
                                                });
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        // Wallpaper & Clock Format Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .gap_3()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Monitor))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Backdrop & Clock Format"),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Home & Launcher Backdrop"),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("wp-gradient")
                                                    .small()
                                                    .with_variant(if cfg.wallpaper == "gradient" {
                                                        gpui_kit::component::button::ButtonVariant::Primary
                                                    } else {
                                                        gpui_kit::component::button::ButtonVariant::Secondary
                                                    })
                                                    .label("Cyan Gradient")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.mutate_config_and_sync(cx, |c| {
                                                            c.wallpaper = "gradient".into();
                                                        });
                                                    })),
                                            )
                                            .child(
                                                Button::new("wp-solid")
                                                    .small()
                                                    .with_variant(if cfg.wallpaper == "solid" {
                                                        gpui_kit::component::button::ButtonVariant::Primary
                                                    } else {
                                                        gpui_kit::component::button::ButtonVariant::Secondary
                                                    })
                                                    .label("Solid Slate")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.mutate_config_and_sync(cx, |c| {
                                                            c.wallpaper = "solid".into();
                                                        });
                                                    })),
                                            ),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Time Format (Home, Status Bar & AOD)"),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("clk-24h")
                                                    .small()
                                                    .with_variant(if cfg.use_24h {
                                                        gpui_kit::component::button::ButtonVariant::Primary
                                                    } else {
                                                        gpui_kit::component::button::ButtonVariant::Secondary
                                                    })
                                                    .label("24-Hour")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.mutate_config_and_sync(cx, |c| {
                                                            c.use_24h = true;
                                                        });
                                                    })),
                                            )
                                            .child(
                                                Button::new("clk-12h")
                                                    .small()
                                                    .with_variant(if !cfg.use_24h {
                                                        gpui_kit::component::button::ButtonVariant::Primary
                                                    } else {
                                                        gpui_kit::component::button::ButtonVariant::Secondary
                                                    })
                                                    .label("12-Hour (AM/PM)")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.mutate_config_and_sync(cx, |c| {
                                                            c.use_24h = false;
                                                        });
                                                    })),
                                            ),
                                    ),
                            ),
                    ),
            )
            // Backlight Brightness Card
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Sun))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("ST7789V Backlight Brightness (PWM)"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .font_bold()
                                    .text_color(pal.accent)
                                    .child(format!("{}%", cfg.brightness)),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(8.))
                            .rounded_full()
                            .bg(pal.surface2)
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(relative((cfg.brightness as f32) / 100.0))
                                    .rounded_full()
                                    .bg(pal.primary),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .children(brightness_steps.into_iter().map(|step| {
                                let selected = cfg.brightness == step;
                                Button::new(("brightness-step", step as usize))
                                    .with_variant(if selected {
                                        gpui_kit::component::button::ButtonVariant::Primary
                                    } else {
                                        gpui_kit::component::button::ButtonVariant::Secondary
                                    })
                                    .label(format!("{}%", step))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| {
                                            c.brightness = step;
                                        });
                                    }))
                            })),
                    ),
            )
            // AOD & Motion Row
            .child(
                h_flex()
                    .gap_4()
                    .w_full()
                    .child(
                        // Always-On Display Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .gap_3()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div().text_color(pal.accent).child(IconName::Eye),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_bold()
                                                    .text_color(pal.text)
                                                    .child("Always-On Display (AOD)"),
                                            ),
                                    )
                                    .child(
                                        Switch::new("aod-enable-switch")
                                            .checked(cfg.aod_enabled)
                                            .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                                let val = *checked;
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.aod_enabled = val;
                                                });
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child("Dims backlight to 12% after inactivity and shows pitch-black clock, date & marquee media strip"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(pal.text_dim)
                                            .child("Idle Timeout:"),
                                    )
                                    .children(aod_timeouts.into_iter().map(|secs| {
                                        let selected = cfg.aod_timeout_s == secs;
                                        Button::new(("aod-timeout", secs as usize))
                                            .small()
                                            .with_variant(if selected {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .label(if secs >= 60 {
                                                format!("{}m", secs / 60)
                                            } else {
                                                format!("{}s", secs)
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.aod_timeout_s = secs;
                                                });
                                            }))
                                    })),
                            ),
                    )
                    .child(
                        // Motion & Live Sync Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .gap_3()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div().text_color(pal.accent).child(IconName::Zap),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_bold()
                                                    .text_color(pal.text)
                                                    .child("Reduce Motion on Device"),
                                            ),
                                    )
                                    .child(
                                        Switch::new("reduce-motion-switch")
                                            .checked(cfg.reduce_motion)
                                            .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                                let val = *checked;
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.reduce_motion = val;
                                                });
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child("Replaces 180-220 ms screen/launcher slide animations with instant switches"),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .pt_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(pal.text)
                                            .child("Auto-push changes immediately"),
                                    )
                                    .child(
                                        Switch::new("auto-sync-switch")
                                            .checked(cfg.auto_sync_settings)
                                            .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                                let val = *checked;
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.config.auto_sync_settings = val;
                                                    save_config(&g.config);
                                                }
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
            )
            // Notification Banner Popup Card
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Bell))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Notification Popups on Screen"),
                                    ),
                            )
                            .child(
                                Switch::new("notify-popup-switch")
                                    .checked(cfg.notify_popup)
                                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                        let val = *checked;
                                        this.mutate_config_and_sync(cx, |c| {
                                            c.notify_popup = val;
                                        });
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(pal.text_dim)
                            .child("When enabled, incoming notifications slide in as a 54 px floating banner on top of the active screen with 4.5s auto-dismiss and wake on AOD"),
                    ),
            )
    }

    fn render_initial_setup(
        &mut self,
        pal: HawaPalette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (cfg, ports, reported_ip, wifi_connected, ssids) = if let Ok(g) = self.state.lock() {
            (
                g.config.clone(),
                g.available_ports.clone(),
                g.device_reported_ip.clone(),
                g.device_wifi_connected,
                g.discovered_ssids.clone(),
            )
        } else {
            (CompanionConfig::default(), Vec::new(), None, None, Vec::new())
        };

        v_flex()
            .gap_4()
            .w_full()
            .min_w_0()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xl()
                            .font_bold()
                            .text_color(pal.text)
                            .child("Initial Device Setup & WiFi Provisioning"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(pal.text_dim)
                            .child("Set up a brand-new LilyGo T-Display-S3 over USB-C Serial (/dev/ttyACM0) or update WiFi & network link settings"),
                    ),
            )
            // Step 1: USB Serial Provisioning & First-Time Setup
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Usb))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Step 1 · USB-C Serial Link (115200 Baud)"),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("scan-usb-ports")
                                            .small()
                                            .secondary()
                                            .icon(IconName::RefreshCw)
                                            .label("Scan USB Ports")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let found = scan_serial_ports();
                                                if let Ok(mut g) = this.state.lock() {
                                                    if let Some(first) = found.first() {
                                                        g.config.usb_port = first.clone();
                                                    }
                                                    g.available_ports = found.clone();
                                                }
                                                this.status_banner = Some(format!(
                                                    "Detected {} USB serial port(s).",
                                                    found.len()
                                                ));
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("query-usb-btn")
                                            .small()
                                            .secondary()
                                            .icon(IconName::Cable)
                                            .label("Detect Device via USB")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.outbound_queue.push_back(
                                                        OutboundAction::QueryDevice {
                                                            via_usb: true,
                                                        },
                                                    );
                                                }
                                                this.status_banner = Some(
                                                    "Queried T-Display-S3 over USB Serial.".into(),
                                                );
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(pal.text_dim)
                                    .child("Active Port:"),
                            )
                            .children(if ports.is_empty() {
                                vec![
                                    div()
                                        .text_xs()
                                        .text_color(pal.warn)
                                        .child("No /dev/ttyACM* detected — plug in T-Display-S3 via USB-C")
                                        .into_any_element(),
                                ]
                            } else {
                                ports
                                    .into_iter()
                                    .enumerate()
                                    .map(|(idx, port_str)| {
                                        let selected = cfg.usb_port == port_str;
                                        let p_clone = port_str.clone();
                                        Button::new(("usb-port-select", idx))
                                            .small()
                                            .with_variant(if selected {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .label(port_str)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.config.usb_port = p_clone.clone();
                                                    save_config(&g.config);
                                                }
                                                cx.notify();
                                            }))
                                            .into_any_element()
                                    })
                                    .collect()
                            }),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .pt_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child("Also stream 1 Hz PC stats & media over USB Serial (useful without WiFi)"),
                            )
                            .child(
                                Switch::new("stream-usb-switch")
                                    .checked(cfg.stream_via_usb)
                                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                        let val = *checked;
                                        if let Ok(mut g) = this.state.lock() {
                                            g.config.stream_via_usb = val;
                                            save_config(&g.config);
                                        }
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Step 2: WiFi Credentials Provisioning Card
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Wifi))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Step 2 · Provision 2.4 GHz WiFi Credentials to Device NVS"),
                                    ),
                            )
                            .child(
                                Button::new("refresh-wifi-scan-btn")
                                    .small()
                                    .secondary()
                                    .icon(IconName::RefreshCw)
                                    .label("Scan 2.4 GHz Networks")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let scanned = scan_wifi_networks();
                                        if let Ok(mut g) = this.state.lock() {
                                            g.discovered_ssids = scanned.clone();
                                        }
                                        this.status_banner = Some(format!(
                                            "Discovered {} nearby 2.4 GHz WiFi network(s).",
                                            scanned.len()
                                        ));
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(pal.text_dim)
                            .child("Click a discovered network below to auto-fill the SSID, or type your own credentials:"),
                    )
                    // Discovered WiFi network chips
                    .when(!ssids.is_empty(), |this| {
                        this.child(
                            h_flex()
                                .gap_2()
                                .flex_wrap()
                                .children(ssids.into_iter().enumerate().map(|(idx, item)| {
                                    let ssid_str = item.ssid.clone();
                                    let btn_label = if item.in_use {
                                        format!("★ {} ({}%)", item.ssid, item.signal)
                                    } else {
                                        format!("{} ({}%)", item.ssid, item.signal)
                                    };
                                    Button::new(("wifi-chip", idx))
                                        .small()
                                        .secondary()
                                        .icon(IconName::Wifi)
                                        .label(btn_label)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            let s = ssid_str.clone();
                                            this.ssid_input.update(cx, |input, cx| {
                                                input.set_value(s.clone(), window, cx);
                                            });
                                            if let Ok(mut g) = this.state.lock() {
                                                g.config.wifi_ssid = s;
                                                save_config(&g.config);
                                            }
                                            cx.notify();
                                        }))
                                        .into_any_element()
                                })),
                        )
                    })
                    .child(
                        h_flex()
                            .gap_3()
                            .w_full()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(pal.text_dim)
                                            .child("WiFi SSID"),
                                    )
                                    .child(Input::new(&self.ssid_input).cleanable(true)),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(pal.text_dim)
                                            .child("WiFi Password"),
                                    )
                                    .child(
                                        Input::new(&self.pass_input)
                                            .mask_toggle()
                                            .cleanable(true),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .pt_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.accent)
                                    .child(match (wifi_connected, reported_ip) {
                                        (Some(true), Some(ip)) => {
                                            format!("Device WiFi Connected · IP: {}", ip)
                                        }
                                        (Some(false), _) => "Device WiFi Disconnected".to_string(),
                                        _ => format!("Current Target IP: {}", cfg.target_ip),
                                    }),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("forget-device-wifi-btn")
                                            .danger()
                                            .small()
                                            .icon(IconName::Trash)
                                            .label("Forget WiFi on Device")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Ok(mut g) = this.state.lock() {
                                                    let force_usb = g.config.stream_via_usb;
                                                    g.config.wifi_ssid = String::new();
                                                    g.config.config_version += 1;
                                                    save_config(&g.config);
                                                    g.outbound_queue.push_back(
                                                        OutboundAction::PushConfig {
                                                            wifi_ssid: None,
                                                            wifi_pass: None,
                                                            forget_wifi: true,
                                                            force_usb,
                                                        },
                                                    );
                                                    g.push_log("Sent 'forget_wifi' command to device.");
                                                }
                                                this.status_banner = Some(
                                                    "Instructed device to forget stored WiFi credentials."
                                                        .into(),
                                                );
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("provision-usb-btn")
                                            .primary()
                                            .icon(IconName::Usb)
                                            .label("Provision via USB Serial (Initial Setup)")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let ssid = this
                                                    .ssid_input
                                                    .read(cx)
                                                    .value()
                                                    .trim()
                                                    .to_string();
                                                let pass =
                                                    this.pass_input.read(cx).value().to_string();
                                                if ssid.is_empty() {
                                                    this.status_banner = Some(
                                                        "Please enter a 2.4 GHz WiFi SSID first."
                                                            .into(),
                                                    );
                                                    cx.notify();
                                                    return;
                                                }
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.config.wifi_ssid = ssid.clone();
                                                    g.config.config_version += 1;
                                                    save_config(&g.config);
                                                    g.outbound_queue.push_back(
                                                        OutboundAction::PushConfig {
                                                            wifi_ssid: Some(ssid.clone()),
                                                            wifi_pass: Some(pass),
                                                            forget_wifi: false,
                                                            force_usb: true,
                                                        },
                                                    );
                                                    g.push_log(format!(
                                                        "Provisioning WiFi '{}' over USB Serial...",
                                                        ssid
                                                    ));
                                                }
                                                this.status_banner = Some(format!(
                                                    "Sent WiFi '{}' + config over USB Serial.",
                                                    ssid
                                                ));
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("provision-udp-btn")
                                            .secondary()
                                            .icon(IconName::Wifi)
                                            .label("Update via WiFi UDP")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let ssid = this
                                                    .ssid_input
                                                    .read(cx)
                                                    .value()
                                                    .trim()
                                                    .to_string();
                                                let pass =
                                                    this.pass_input.read(cx).value().to_string();
                                                if ssid.is_empty() {
                                                    this.status_banner = Some(
                                                        "Please enter a 2.4 GHz WiFi SSID first."
                                                            .into(),
                                                    );
                                                    cx.notify();
                                                    return;
                                                }
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.config.wifi_ssid = ssid.clone();
                                                    g.config.config_version += 1;
                                                    save_config(&g.config);
                                                    g.outbound_queue.push_back(
                                                        OutboundAction::PushConfig {
                                                            wifi_ssid: Some(ssid.clone()),
                                                            wifi_pass: Some(pass),
                                                            forget_wifi: false,
                                                            force_usb: false,
                                                        },
                                                    );
                                                }
                                                this.status_banner = Some(format!(
                                                    "Sent WiFi '{}' over UDP.",
                                                    ssid
                                                ));
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
            )
            // Step 3: Target WiFi IP Address Card
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_3()
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().text_color(pal.accent).child(IconName::Radio))
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(pal.text)
                                    .child("Step 3 · Wireless UDP Target IP (Port 7777)"),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(div().flex_1().child(Input::new(&self.ip_input).cleanable(true)))
                            .child(
                                Button::new("save-target-ip-btn")
                                    .primary()
                                    .icon(IconName::Check)
                                    .label("Save Target IP & Connect")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let new_ip = sanitize_ip_str(
                                            this.ip_input.read(cx).value().trim(),
                                        );
                                        if !new_ip.is_empty() {
                                            if let Ok(mut g) = this.state.lock() {
                                                g.config.target_ip = new_ip.clone();
                                                save_config(&g.config);
                                                g.outbound_queue.push_back(
                                                    OutboundAction::QueryDevice { via_usb: false },
                                                );
                                                g.push_log(format!(
                                                    "Updated target UDP IP to {}:{}",
                                                    new_ip, UDP_PORT
                                                ));
                                            }
                                            this.status_banner = Some(format!(
                                                "Target IP set to {}:{}.",
                                                new_ip, UDP_PORT
                                            ));
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ),
            )
    }

    fn render_daemon_settings(
        &mut self,
        pal: HawaPalette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (cfg, logs) = if let Ok(g) = self.state.lock() {
            (
                g.config.clone(),
                g.logs.iter().cloned().collect::<Vec<String>>(),
            )
        } else {
            (CompanionConfig::default(), Vec::new())
        };

        v_flex()
            .gap_4()
            .w_full()
            .min_w_0()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xl()
                            .font_bold()
                            .text_color(pal.text)
                            .child("System Tray Daemon & Startup Settings"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(pal.text_dim)
                            .child("Configure background tray behavior, XDG autostart on login, and inspect live daemon events"),
                    ),
            )
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_4()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Start HawaOS Companion Daemon with System"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child(format!(
                                                "Installs XDG autostart entry ({}) with --minimized tray mode",
                                                autostart_desktop_path().display()
                                            )),
                                    ),
                            )
                            .child(
                                Switch::new("autostart-switch")
                                    .checked(cfg.autostart_enabled)
                                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                        let enable = *checked;
                                        match set_autostart_installed(enable) {
                                            Ok(()) => {
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.config.autostart_enabled = enable;
                                                    save_config(&g.config);
                                                    g.push_log(if enable {
                                                        "Enabled system autostart (--minimized)"
                                                    } else {
                                                        "Disabled system autostart"
                                                    });
                                                }
                                                this.status_banner = Some(if enable {
                                                    "Autostart enabled (~/.config/autostart/hawaos-companion.desktop)."
                                                        .into()
                                                } else {
                                                    "Autostart entry removed.".into()
                                                });
                                            }
                                            Err(e) => {
                                                this.status_banner = Some(format!(
                                                    "Failed to update autostart: {}",
                                                    e
                                                ));
                                            }
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Keep Daemon Running in System Tray When Closed"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Closing this window leaves the tray icon and 1 Hz telemetry stream active in the background"),
                                    ),
                            )
                            .child(
                                Switch::new("close-to-tray-switch")
                                    .checked(cfg.close_to_tray)
                                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                        let val = *checked;
                                        if let Ok(mut g) = this.state.lock() {
                                            g.config.close_to_tray = val;
                                            save_config(&g.config);
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .pt_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_muted)
                                    .child(format!(
                                        "Config file: {}",
                                        config_file_path().display()
                                    )),
                            )
                            .child(
                                Button::new("quit-daemon-btn")
                                    .danger()
                                    .small()
                                    .icon(IconName::Power)
                                    .label("Quit Companion Daemon")
                                    .on_click(|_, _, cx| {
                                        cx.quit();
                                    }),
                            ),
                    ),
            )
            // Live Daemon Activity Log
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .text_color(pal.text_dim)
                            .child("RECENT DAEMON & DEVICE LOGS"),
                    )
                    .child(
                        v_flex()
                            .p_3()
                            .rounded(px(10.))
                            .bg(pal.bg)
                            .border_1()
                            .border_color(pal.border)
                            .gap_1()
                            .children(logs.into_iter().rev().take(10).map(|line| {
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child(format!("• {}", line))
                            })),
                    ),
            )
    }
}

impl Render for CompanionWindowView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark_mode = if let Ok(g) = self.state.lock() {
            g.config.companion_dark_mode
        } else {
            true
        };
        let pal = if dark_mode {
            HawaPalette::dark()
        } else {
            HawaPalette::light()
        };

        let content = match self.active_section {
            NavSection::Overview => self.render_overview(pal, cx).into_any_element(),
            NavSection::DeviceSettings => self.render_device_settings(pal, cx).into_any_element(),
            NavSection::InitialSetup => self.render_initial_setup(pal, cx).into_any_element(),
            NavSection::DaemonSettings => self.render_daemon_settings(pal, cx).into_any_element(),
        };

        let banner = self.status_banner.clone();

        h_flex()
            .size_full()
            .bg(pal.bg)
            .text_color(pal.text)
            .child(self.render_sidebar(pal, cx))
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .p_6()
                    .gap_4()
                    .overflow_y_scrollbar()
                    .when_some(banner, |this, msg| {
                        this.child(
                            h_flex()
                                .px_4()
                                .py_2()
                                .rounded(px(10.))
                                .bg(pal.primary.opacity(0.16))
                                .border_1()
                                .border_color(pal.primary.opacity(0.45))
                                .justify_between()
                                .items_center()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_color(pal.accent)
                                                .child(IconName::CircleCheck),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_semibold()
                                                .text_color(pal.text)
                                                .child(msg),
                                        ),
                                )
                                .child(
                                    Button::new("dismiss-banner")
                                        .small()
                                        .ghost()
                                        .label("Dismiss")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.status_banner = None;
                                            cx.notify();
                                        })),
                                ),
                        )
                    })
                    .child(content),
            )
    }
}

fn open_companion_window(cx: &mut App, state: SharedState) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1040.), px(700.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("HawaOS Desktop Companion — Breezy Elegance, Materialised".into()),
            ..Default::default()
        }),
        app_id: Some("hawaos-companion".into()),
        ..Default::default()
    };

    let _ = gpui_kit::open_window(options, cx, move |window, cx| {
        cx.new(|cx| CompanionWindowView::new(window, cx, Arc::clone(&state)))
    });
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let start_minimized = args.iter().any(|a| a == "--minimized" || a == "--tray");

    let mut cfg = load_config();
    if let Some(ip_arg) = args.iter().skip(1).find(|a| !a.starts_with('-')) {
        cfg.target_ip = ip_arg.clone();
        save_config(&cfg);
    }

    let state: SharedState = Arc::new(Mutex::new(SharedDaemonState::new(cfg.clone())));
    let art_cache: SharedArtCache = Arc::new(Mutex::new(HashMap::new()));

    spawn_art_http_server(Arc::clone(&art_cache));
    spawn_daemon_worker(Arc::clone(&state), Arc::clone(&art_cache));
    spawn_system_tray(Arc::clone(&state));

    gpui_kit::application()
        .with_assets(AllAssets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            cx.set_quit_mode(QuitMode::Explicit);
            apply_hawaos_gpui_theme(cfg.companion_dark_mode, cx);

            if !start_minimized {
                open_companion_window(cx, Arc::clone(&state));
            }

            let poll_state = Arc::clone(&state);
            cx.spawn(async move |cx: &mut AsyncApp| {
                let mut tick_count: u32 = 0;
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(200))
                        .await;
                    tick_count = tick_count.wrapping_add(1);

                    let (want_open, want_quit) = if let Ok(mut g) = poll_state.lock() {
                        let open = g.open_window_requested;
                        g.open_window_requested = false;
                        (open, g.quit_requested)
                    } else {
                        (false, false)
                    };

                    let state_for_ui = Arc::clone(&poll_state);
                    let _ = cx.update(|cx: &mut App| {
                        if want_quit {
                            cx.quit();
                            return;
                        }
                        if want_open {
                            if let Some(handle) = cx.windows().first() {
                                let _ = handle.update(cx, |_, window, _| {
                                    window.activate_window();
                                });
                            } else {
                                open_companion_window(cx, state_for_ui);
                            }
                        } else if tick_count.is_multiple_of(5) && !cx.windows().is_empty() {
                            cx.refresh_windows();
                        }
                    });
                }
            })
            .detach();
        });
}
