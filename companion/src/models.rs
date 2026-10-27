use gpui_kit::gpui::Image;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub use crate::config::{CompanionConfig, DiscoveredWifi};
use crate::platform::{scan_wifi_networks, serial_ports as scan_serial_ports};

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
    pub id: Option<String>,
    pub setup: Option<bool>,
    pub fw: Option<String>,
    pub state: Option<String>,
    pub pct: Option<u8>,
    pub error: Option<String>,
    pub reason: Option<String>,
    pub hash: Option<String>,
    pub link: Option<String>,
    pub theme: Option<String>,
    pub accent: Option<String>,
    pub brightness: Option<u8>,
    pub aod: Option<bool>,
    pub aod_timeout_s: Option<u16>,
    pub aod_off_min: Option<u16>,
    /// Set when a firmware update failed its start-up check and the device
    /// went back to the firmware it had (DESIGN.md 11.5).
    pub rolled_back: Option<String>,
    pub gl: Option<u8>,
    pub fmt: Option<u8>,
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
    /// Send a firmware image (path to firmware.bin) to the device.
    StartOta {
        path: std::path::PathBuf,
    },
    /// Switch the device's everyday link (DESIGN.md Section 11.7).
    SetLink {
        ble: bool,
        /// When it was asked for: waits up to 15 s for a reconnecting link.
        since: std::time::Instant,
    },
    /// Push album art to a device that can't pull it over HTTP (art_req).
    SendArt {
        hash: String,
    },
    /// Push the wallpaper to a device that can't pull it over HTTP (wp_req).
    SendWallpaper {
        hash: String,
    },
    /// Push a glyph pack to a device that can't pull it over HTTP (gl_req).
    SendGlyphs {
        hash: String,
    },
    SendClockAction {
        action: String,
        duration_sec: u32,
        duration_ms: u32,
    },
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
    /// When this process started (device_silence before the first ack).
    pub started: Instant,
    pub device_reported_ip: Option<String>,
    pub device_wifi_connected: Option<bool>,
    pub available_ports: Vec<String>,
    pub discovered_ssids: Vec<DiscoveredWifi>,
    pub outbound_queue: VecDeque<OutboundAction>,
    pub logs: VecDeque<String>,
    pub open_window_requested: bool,
    /// Tray "Add device...": show the setup wizard when the window opens.
    pub open_setup_requested: bool,
    pub quit_requested: bool,
    pub weather: Option<WeatherMsg>,
    pub stopwatch_state: String,
    pub stopwatch_elapsed_ms: u32,
    pub stopwatch_last_start: Option<Instant>,
    pub timer_state: String,
    pub timer_duration_sec: u32,
    pub timer_remaining_sec: u32,
    pub timer_remaining_ms: u32,
    pub timer_target_instant: Option<Instant>,
    /// The setup wizard owns the USB port; the worker stays off it.
    pub setup_holds_usb: bool,
    /// Setup wizard running: the window refreshes faster.
    pub wizard_active: bool,
    /// A device on our link reports it still needs first-run setup.
    pub device_needs_setup: bool,
    /// Firmware version the device reports (`fw` in config_ack).
    pub device_fw: Option<String>,
    /// The device's firmware has a glyph cache (`gl` in config_ack), so text
    /// in other scripts goes as glyphs (glyphs.rs) instead of being dropped.
    pub device_glyphs: bool,
    /// The device formats notification bodies (`fmt` in config_ack): line
    /// breaks and emphasis (notifications.rs).
    pub device_markup: bool,
    pub ota: Option<OtaProgress>,
    /// One-shot message for the window's banner (taken when shown).
    pub notice: Option<String>,
    pub last_auth_notice: Option<std::time::Instant>,
    /// The everyday Bluetooth link is up (ble_daemon.rs).
    pub ble_connected: bool,
    /// The device has a saved Wi-Fi network (from its last ack's `ssid`).
    pub device_has_network: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OtaProgress {
    /// "starting" | "downloading" | "verifying" | "done" | "failed"
    pub state: String,
    pub pct: u8,
    pub error: Option<String>,
    /// When this state was last reported, to time out a silent device.
    pub at: std::time::Instant,
}

impl OtaProgress {
    pub fn new(state: &str, pct: u8, error: Option<String>) -> Self {
        Self { state: state.into(), pct, error, at: std::time::Instant::now() }
    }

    pub fn busy(&self) -> bool {
        matches!(self.state.as_str(), "starting" | "downloading" | "verifying")
    }
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
            started: Instant::now(),
            device_reported_ip: None,
            device_wifi_connected: None,
            available_ports: ports,
            discovered_ssids: ssids,
            outbound_queue: VecDeque::new(),
            logs: VecDeque::with_capacity(40),
            open_window_requested: false,
            open_setup_requested: false,
            quit_requested: false,
            weather: None,
            stopwatch_state: "stopped".to_string(),
            stopwatch_elapsed_ms: 0,
            stopwatch_last_start: None,
            timer_state: "stopped".to_string(),
            timer_duration_sec: 300,
            timer_remaining_sec: 300,
            timer_remaining_ms: 300_000,
            timer_target_instant: None,
            setup_holds_usb: false,
            wizard_active: false,
            device_needs_setup: false,
            device_fw: None,
            device_glyphs: false,
            device_markup: false,
            ota: None,
            notice: None,
            last_auth_notice: None,
            ble_connected: false,
            device_has_network: None,
        }
    }

    pub fn start_stopwatch(&mut self) {
        if self.stopwatch_state != "running" {
            self.stopwatch_state = "running".to_string();
            self.stopwatch_last_start = Some(Instant::now());
            self.outbound_queue.push_back(OutboundAction::SendClockAction {
                action: "stopwatch_start".to_string(),
                duration_sec: 0,
                duration_ms: 0,
            });
        }
    }

    pub fn pause_stopwatch(&mut self) {
        if self.stopwatch_state == "running" {
            if let Some(inst) = self.stopwatch_last_start.take() {
                self.stopwatch_elapsed_ms = self.stopwatch_elapsed_ms.saturating_add(inst.elapsed().as_millis() as u32);
            }
            self.stopwatch_state = "paused".to_string();
            self.outbound_queue.push_back(OutboundAction::SendClockAction {
                action: "stopwatch_pause".to_string(),
                duration_sec: 0,
                duration_ms: 0,
            });
        }
    }

    pub fn reset_stopwatch(&mut self) {
        self.stopwatch_state = "stopped".to_string();
        self.stopwatch_elapsed_ms = 0;
        self.stopwatch_last_start = None;
        self.outbound_queue.push_back(OutboundAction::SendClockAction {
            action: "stopwatch_reset".to_string(),
            duration_sec: 0,
            duration_ms: 0,
        });
    }

    pub fn toggle_stopwatch(&mut self) {
        if self.stopwatch_state == "running" {
            self.pause_stopwatch();
        } else {
            self.start_stopwatch();
        }
    }

    pub fn start_timer(&mut self, duration_sec: u32) {
        let dur = if duration_sec > 0 { duration_sec } else { 300 };
        self.timer_duration_sec = dur;
        self.timer_remaining_sec = dur;
        self.timer_remaining_ms = dur * 1000;
        self.timer_target_instant = Some(Instant::now() + std::time::Duration::from_secs(dur as u64));
        self.timer_state = "running".to_string();
        self.outbound_queue.push_back(OutboundAction::SendClockAction {
            action: "timer_start".to_string(),
            duration_sec: dur,
            duration_ms: dur * 1000,
        });
    }

    pub fn start_timer_ms(&mut self, duration_ms: u32) {
        let dur_ms = if duration_ms > 0 { duration_ms } else { 300_000 };
        self.timer_duration_sec = (dur_ms + 999) / 1000;
        self.timer_remaining_sec = (dur_ms + 999) / 1000;
        self.timer_remaining_ms = dur_ms;
        self.timer_target_instant = Some(Instant::now() + std::time::Duration::from_millis(dur_ms as u64));
        self.timer_state = "running".to_string();
        self.outbound_queue.push_back(OutboundAction::SendClockAction {
            action: "timer_start".to_string(),
            duration_sec: self.timer_duration_sec,
            duration_ms: dur_ms,
        });
    }

    pub fn pause_timer(&mut self) {
        if self.timer_state == "running" {
            if let Some(target) = self.timer_target_instant.take() {
                let now = Instant::now();
                if target > now {
                    let rem = target - now;
                    self.timer_remaining_sec = rem.as_secs() as u32;
                    self.timer_remaining_ms = rem.as_millis() as u32;
                } else {
                    self.timer_remaining_sec = 0;
                    self.timer_remaining_ms = 0;
                }
            }
            self.timer_state = "paused".to_string();
            self.outbound_queue.push_back(OutboundAction::SendClockAction {
                action: "timer_pause".to_string(),
                duration_sec: self.timer_duration_sec,
                duration_ms: self.timer_remaining_ms,
            });
        }
    }

    pub fn resume_timer(&mut self) {
        if self.timer_state == "paused" || self.timer_state == "stopped" {
            let rem_s = if self.timer_remaining_sec > 0 { self.timer_remaining_sec } else { self.timer_duration_sec };
            let rem_ms = if self.timer_remaining_ms > 0 { self.timer_remaining_ms } else { rem_s * 1000 };
            self.timer_remaining_sec = rem_s;
            self.timer_remaining_ms = rem_ms;
            self.timer_target_instant = Some(Instant::now() + std::time::Duration::from_millis(rem_ms as u64));
            self.timer_state = "running".to_string();
            self.outbound_queue.push_back(OutboundAction::SendClockAction {
                action: "timer_resume".to_string(),
                duration_sec: rem_s,
                duration_ms: rem_ms,
            });
        }
    }

    pub fn reset_timer(&mut self) {
        self.timer_state = "stopped".to_string();
        self.timer_remaining_sec = self.timer_duration_sec;
        self.timer_remaining_ms = self.timer_duration_sec * 1000;
        self.timer_target_instant = None;
        self.outbound_queue.push_back(OutboundAction::SendClockAction {
            action: "timer_reset".to_string(),
            duration_sec: self.timer_duration_sec,
            duration_ms: self.timer_duration_sec * 1000,
        });
    }

    pub fn toggle_timer(&mut self) {
        if self.timer_state == "running" {
            self.pause_timer();
        } else if self.timer_state == "paused" {
            self.resume_timer();
        } else {
            self.start_timer(self.timer_duration_sec);
        }
    }

    pub fn handle_clock_cmd(&mut self, cmd: &str) {
        match cmd {
            "timer_toggle" => self.toggle_timer(),
            "stopwatch_toggle" => self.toggle_stopwatch(),
            "timer_reset" => self.reset_timer(),
            "stopwatch_reset" => self.reset_stopwatch(),
            _ => {}
        }
    }

    pub fn tick_clock(&mut self) {
        if self.stopwatch_state == "running" {
            if let Some(inst) = self.stopwatch_last_start {
                self.stopwatch_elapsed_ms = self.stopwatch_elapsed_ms.saturating_add(inst.elapsed().as_millis() as u32);
                self.stopwatch_last_start = Some(Instant::now());
            }
        }
        if self.timer_state == "running" {
            if let Some(target) = self.timer_target_instant {
                let now = Instant::now();
                if now >= target {
                    self.timer_remaining_sec = 0;
                    self.timer_remaining_ms = 0;
                    self.timer_state = "finished".to_string();
                    self.timer_target_instant = None;
                } else {
                    let rem = target - now;
                    self.timer_remaining_sec = rem.as_secs() as u32;
                    self.timer_remaining_ms = rem.as_millis() as u32;
                }
            }
        }
    }

    /// How long the device has been quiet: since its last ack, or since we
    /// started. The worker queries every 20 s, so this stays small while the
    /// link works; past 45 s the other transport gets probed (DESIGN.md 11.7).
    pub fn device_silence(&self) -> std::time::Duration {
        self.last_ack_instant.unwrap_or(self.started).elapsed()
    }

    pub fn push_log(&mut self, msg: impl Into<String>) {
        if self.logs.len() >= 35 {
            self.logs.pop_front();
        }
        self.logs.push_back(msg.into());
    }
}

pub type SharedState = Arc<Mutex<SharedDaemonState>>;
