use serde::{Deserialize, Serialize};
use std::fs;
use std::net::UdpSocket;

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
    pub aod_off_min: u16,     // backlight off after this long in AOD: 0 (never) | 5 | 15 | 30 | 60
    pub reduce_motion: bool,  // true | false
    pub wallpaper: String,    // "gradient" | "solid" | "image"
    pub wallpaper_hash: String, // the picture for "image" (see wallpaper.rs)
    pub rollback_seen: String,  // last rolled_back id we told the user about
    pub use_24h: bool,        // true | false
    pub notify_popup: bool,   // true | false
    pub wifi_ssid: String,

    // Weather configuration fields
    pub weather_auto_location: bool,
    pub weather_city: String,
    pub weather_unit: String, // "c" | "f"
    pub orientation: String,  // "normal" | "inverted"

    /// Device added with the setup wizard ("" until one is).
    pub device_id: String,
    pub device_name: String,
    /// Secret given to the device at setup; keys the encrypted network link.
    pub device_token: String,
    /// Everyday link is Bluetooth instead of Wi-Fi (DESIGN.md Section 11.7).
    pub use_ble: bool,
}

impl Default for CompanionConfig {
    fn default() -> Self {
        Self {
            target_ip: DEFAULT_DEVICE_IP.to_string(),
            usb_port: "/dev/ttyACM0".to_string(),
            stream_via_usb: false,
            auto_sync_settings: true,
            autostart_enabled: crate::platform::is_autostart_installed(),
            close_to_tray: true,
            companion_dark_mode: true,
            window_transparency: "auto".to_string(),
            config_version: 1,
            device_theme: "dark".to_string(),
            accent: crate::ui::accents::DEFAULT_ACCENT.to_string(),
            brightness: 80,
            aod_enabled: true,
            aod_timeout_s: 30,
            aod_off_min: 0,
            reduce_motion: false,
            wallpaper: "gradient".to_string(),
            wallpaper_hash: String::new(),
            rollback_seen: String::new(),
            use_24h: true,
            notify_popup: true,
            wifi_ssid: String::new(),
            weather_auto_location: true,
            weather_city: String::new(),
            weather_unit: "c".to_string(),
            orientation: "normal".to_string(),
            device_id: String::new(),
            device_name: String::new(),
            device_token: String::new(),
            use_ble: false,
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

/// Reverse-DNS id: the Flatpak id, the desktop file's name, and the window's
/// Wayland app_id, so the compositor shows the right name and icon.
pub const APP_ID: &str = "dev.is_a.thecodeheist.HawaOSCompanion";

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
    let path = crate::platform::config_file_path();
    if let Ok(bytes) = fs::read(&path) {
        if let Ok(mut cfg) = serde_json::from_slice::<CompanionConfig>(&bytes) {
            cfg.target_ip = sanitize_ip_str(&cfg.target_ip);
            cfg.autostart_enabled = crate::platform::is_autostart_installed();
            return cfg;
        }
    }
    CompanionConfig::default()
}

pub fn save_config(cfg: &CompanionConfig) {
    let mut clean_cfg = cfg.clone();
    clean_cfg.target_ip = sanitize_ip_str(&clean_cfg.target_ip);
    let path = crate::platform::config_file_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(&clean_cfg) {
        let _ = fs::write(&path, json);
        // Holds the device token that keys the encrypted link: owner only.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
    }
}
