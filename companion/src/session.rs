//! What the computer is doing that the device mirrors: whether the session
//! is locked (the device turns its screen off or dims, DESIGN.md 10) and the
//! microphone (the device shows whether a call can hear you, and mutes it).
//! Polled once a second; the worker sends both with the stats.

use std::thread;
use std::time::Duration;

use crate::models::SharedState;
use crate::platform;

pub fn spawn(state: SharedState) {
    thread::spawn(move || loop {
        let locked = platform::session_locked().unwrap_or(false);
        let mic = platform::mic();
        if let Ok(mut g) = state.lock() {
            if g.session_locked != locked {
                g.push_log(if locked { "Computer locked" } else { "Computer unlocked" }.to_string());
            }
            g.session_locked = locked;
            g.mic = mic;
        }
        thread::sleep(Duration::from_secs(1));
    });
}

/// The stats message's `lock` field.
pub fn lock_field(g: &crate::models::SharedDaemonState) -> Option<&'static str> {
    if !g.session_locked {
        return None;
    }
    match g.config.lock_screen.as_str() {
        "aod" => Some("aod"),
        "none" => None,
        _ => Some("off"),
    }
}


/// The device's mute button. Updates the state at once so the next stats
/// message already carries the new state.
pub fn toggle_mic(state: &SharedState) {
    platform::toggle_mic();
    let mic = platform::mic();
    if let Ok(mut g) = state.lock() {
        g.mic = mic;
        let what = match mic {
            Some(m) if m.muted => "Microphone muted from the device",
            Some(_) => "Microphone unmuted from the device",
            None => "No microphone to mute",
        };
        g.push_log(what.to_string());
    }
}
