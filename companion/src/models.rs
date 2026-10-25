use gpui_kit::gpui::Image;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub use crate::config::{CompanionConfig, DiscoveredWifi};
use crate::config::scan_wifi_networks;
use crate::protocol::scan_serial_ports;

// ============================================================================
// Wire Protocol Messages (DESIGN.md Section 11)
// ============================================================================

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StatsMsg<'a> {
    #[serde(rename = "type")]
    pub msg_type: &'a str,
    pub ts: u64,
    pub cpu: u8,
    pub ram: u8,
    pub net_up_kbps: u32,
    pub net_down_kbps: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_ip: Option<&'a str>,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
pub struct MediaMsg {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub state: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub pos_ms: u32,
    pub dur_ms: u32,
    pub ts: u64,
    pub art: String,
    pub bg_top: String,
    pub bg_bottom: String,
    pub ink: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_ip: Option<String>,
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
            bg_top: "#008DA6".to_string(),
            bg_bottom: "#071115".to_string(),
            ink: "#FFFFFF".to_string(),
            host_ip: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct WeatherForecastDayMsg {
    pub day: String,
    pub icon: String,
    pub min: i16,
    pub max: i16,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct WeatherMsg {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub city: String,
    pub temp: i16,
    pub feels_like: i16,
    pub temp_min: i16,
    pub temp_max: i16,
    pub humidity: u8,
    pub wind_speed: u16,
    pub unit: String,
    pub condition: String,
    pub icon: String,
    pub forecast: Vec<WeatherForecastDayMsg>,
}

#[derive(Deserialize, Debug)]
pub struct IncomingPacket {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub cmd: Option<String>,
    pub v: Option<u32>,
    pub ip: Option<String>,
    pub wifi: Option<bool>,
    pub ssid: Option<String>,
    pub theme: Option<String>,
    pub brightness: Option<u8>,
    pub aod: Option<bool>,
    pub aod_timeout_s: Option<u16>,
    pub reduce_motion: Option<bool>,
    pub wallpaper: Option<String>,
    pub use_24h: Option<bool>,
    pub notify_popup: Option<bool>,
    pub orientation: Option<String>,
}

pub struct ArtEntry {
    pub rgb565_le: Vec<u8>,
    pub png_bytes: Vec<u8>,
    pub bg_top: String,
    pub bg_bottom: String,
    pub ink: String,
}

pub type SharedArtCache = Arc<Mutex<HashMap<String, ArtEntry>>>;

#[derive(Clone, Debug)]
pub enum OutboundAction {
    PushConfig {
        wifi_ssid: Option<String>,
        wifi_pass: Option<String>,
        forget_wifi: bool,
        force_usb: bool,
    },
    SendNotification {
        app: String,
        title: String,
        body: String,
    },
    QueryDevice {
        via_usb: bool,
    },
    RefreshWeather,
}

pub struct SharedDaemonState {
    pub config: CompanionConfig,
    pub cpu_pct: u8,
    pub ram_pct: u8,
    pub used_ram_gb: f32,
    pub total_ram_gb: f32,
    pub net_up_kbps: u32,
    pub net_down_kbps: u32,
    pub cpu_history: VecDeque<u8>,
    pub media: MediaMsg,
    pub active_player: Option<String>,
    pub current_art_png: Option<Arc<Image>>,
    pub last_ack_version: Option<u32>,
    pub last_ack_instant: Option<Instant>,
    pub device_reported_ip: Option<String>,
    pub device_wifi_connected: Option<bool>,
    pub available_ports: Vec<String>,
    pub discovered_ssids: Vec<DiscoveredWifi>,
    pub outbound_queue: VecDeque<OutboundAction>,
    pub logs: VecDeque<String>,
    pub open_window_requested: bool,
    pub quit_requested: bool,
    pub weather: Option<WeatherMsg>,
}

impl SharedDaemonState {
    pub fn new(config: CompanionConfig) -> Self {
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
            weather: None,
        }
    }

    pub fn push_log(&mut self, msg: impl Into<String>) {
        if self.logs.len() >= 35 {
            self.logs.pop_front();
        }
        self.logs.push_back(msg.into());
    }
}

pub type SharedState = Arc<Mutex<SharedDaemonState>>;
