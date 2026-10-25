use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::Command;

pub const DEFAULT_DEVICE_IP: &str = "192.168.0.98";
pub const UDP_PORT: u16 = 7777;
pub const ART_HTTP_PORT: u16 = 7778;
pub const ART_DIM: u32 = 128;
pub const USB_BAUD: u32 = 115_200;

pub static LOGO_PNG_BYTES: &[u8] = include_bytes!("../../assets/logo.png");

// ============================================================================
// Persistent Companion + Device Configuration
// ============================================================================

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct CompanionConfig {
    pub target_ip: String,
    pub usb_port: String,
    pub stream_via_usb: bool,
    pub auto_sync_settings: bool,
    pub autostart_enabled: bool,
    pub close_to_tray: bool,
    pub companion_dark_mode: bool,
    pub window_transparency: String, // "auto" | "on" | "off"

    // Device configuration fields (DESIGN.md Section 11.3)
    pub config_version: u32,
    pub device_theme: String, // "dark" | "light" | "auto"
    pub accent: String,       // Tailwind color id (see ui/accents.rs); shared by device + companion
    pub brightness: u8,       // 20 | 40 | 60 | 80 | 100
    pub aod_enabled: bool,    // true | false
    pub aod_timeout_s: u16,   // 15 | 30 | 60 | 120 | 300
    pub reduce_motion: bool,  // true | false
    pub wallpaper: String,    // "gradient" | "solid"
    pub use_24h: bool,        // true | false
    pub notify_popup: bool,   // true | false
    pub wifi_ssid: String,

    // Weather configuration fields
    pub weather_auto_location: bool,
    pub weather_city: String,
    pub weather_unit: String, // "c" | "f"
    pub orientation: String,  // "normal" | "inverted"
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
            window_transparency: "auto".to_string(),
            config_version: 1,
            device_theme: "dark".to_string(),
            accent: crate::ui::accents::DEFAULT_ACCENT.to_string(),
            brightness: 80,
            aod_enabled: true,
            aod_timeout_s: 30,
            reduce_motion: false,
            wallpaper: "gradient".to_string(),
            use_24h: true,
            notify_popup: true,
            wifi_ssid: String::new(),
            weather_auto_location: true,
            weather_city: String::new(),
            weather_unit: "c".to_string(),
            orientation: "normal".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DiscoveredWifi {
    pub ssid: String,
    pub signal: u8,
    pub security: String,
    pub in_use: bool,
}

pub fn scan_wifi_networks() -> Vec<DiscoveredWifi> {
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

pub fn config_file_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("hawaos-companion")
        .join("config.json")
}

pub fn autostart_desktop_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("autostart")
        .join("hawaos-companion.desktop")
}

pub fn is_autostart_installed() -> bool {
    autostart_desktop_path().exists()
}

pub fn set_autostart_installed(enable: bool) -> std::io::Result<()> {
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

pub fn sanitize_ip_str(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(idx) = trimmed.find(|c: char| c.is_whitespace() || c == '|' || c == ',' || c == ';') {
        trimmed[..idx].trim().to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn get_local_lan_ip() -> String {
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    String::new()
}

pub fn load_config() -> CompanionConfig {
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

pub fn save_config(cfg: &CompanionConfig) {
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
