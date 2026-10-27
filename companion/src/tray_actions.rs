//! What the tray menu shows and does, shared by each platform's tray
//! (platform::spawn_system_tray), so the menus stay the same.

use crate::config::save_config;
use crate::models::{OutboundAction, SharedState};
use crate::platform::set_autostart_installed;
use crate::ui::accents::ACCENTS;
use crate::ui::theme::accent_by_id;

/// What the menu shows; a tray rebuilds its labels when this changes.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TrayView {
    pub track: String,
    pub theme: String,
    /// Index into ACCENTS.
    pub accent: usize,
    pub autostart: bool,
    pub active_player: Option<String>,
    /// The tooltip: device address, load, and the track.
    pub tooltip: String,
}

pub fn view(state: &SharedState) -> Option<TrayView> {
    let g = state.lock().ok()?;
    let track = if g.media.title.is_empty() {
        "Media: No active track".to_string()
    } else {
        format!("♪ {} — {}", g.media.title, g.media.artist)
    };
    let playing = if g.media.title.is_empty() {
        "Idle".to_string()
    } else {
        format!("{} — {}", g.media.title, g.media.artist)
    };
    Some(TrayView {
        track,
        theme: g.config.device_theme.clone(),
        accent: ACCENTS.iter().position(|a| a.id == accent_by_id(&g.config.accent).id).unwrap_or(0),
        autostart: g.config.autostart_enabled,
        active_player: g.active_player.clone(),
        tooltip: format!(
            "HawaOS Companion ({})\nCPU {}% · RAM {}% · {}",
            g.config.target_ip, g.cpu_pct, g.ram_pct, playing
        ),
    })
}

pub fn theme_label(view: &TrayView) -> String {
    format!("Switch Device Theme (Current: {})", view.theme.to_uppercase())
}

pub fn open_window(state: &SharedState) {
    if let Ok(mut g) = state.lock() {
        g.open_window_requested = true;
    }
}

pub fn add_device(state: &SharedState) {
    if let Ok(mut g) = state.lock() {
        g.open_window_requested = true;
        g.open_setup_requested = true;
    }
}

fn push_config(g: &mut crate::models::SharedDaemonState) {
    g.config.config_version += 1;
    save_config(&g.config);
    g.outbound_queue.push_back(OutboundAction::PushConfig {
        wifi_ssid: None,
        wifi_pass: None,
        forget_wifi: false,
        force_usb: false,
    });
}

pub fn toggle_device_theme(state: &SharedState) {
    if let Ok(mut g) = state.lock() {
        g.config.device_theme = if g.config.device_theme == "dark" { "light" } else { "dark" }.to_string();
        push_config(&mut g);
    }
}

pub fn set_accent(state: &SharedState, idx: usize) {
    let Some(picked) = ACCENTS.get(idx) else { return };
    if let Ok(mut g) = state.lock() {
        g.config.accent = picked.id.to_string();
        push_config(&mut g);
    }
}

pub fn toggle_autostart(state: &SharedState) {
    if let Ok(mut g) = state.lock() {
        let next = !g.config.autostart_enabled;
        if set_autostart_installed(next).is_ok() {
            g.config.autostart_enabled = next;
            save_config(&g.config);
        }
    }
}

pub fn quit(state: &SharedState) {
    if let Ok(mut g) = state.lock() {
        g.quit_requested = true;
    }
}
