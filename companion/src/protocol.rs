use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::config::{save_config, sanitize_ip_str, USB_BAUD};
use crate::media::handle_device_command;
use crate::models::{IncomingPacket, SharedState};

pub fn scan_serial_ports() -> Vec<String> {
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

/// Cuts `s` to at most `max_bytes` on a char boundary: the device's field
/// size (DataStore.h, minus the terminator). Keeps signed packets in one
/// datagram however long a notification is.
pub fn fit_bytes(mut s: String, max_bytes: usize) -> String {
    if s.len() > max_bytes {
        let mut end = max_bytes;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

pub fn sanitize_for_font(input: &str) -> String {
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

pub fn handle_incoming_json_line(line: &str, state: &SharedState) {
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
        if pkt.msg_type == "clock_cmd" {
            if let Some(cmd) = pkt.cmd {
                if let Ok(mut g) = state.lock() {
                    g.handle_clock_cmd(&cmd);
                    g.push_log(format!("Device triggered clock command: '{}'", cmd));
                }
            }
        } else if pkt.msg_type == "cmd" {
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
        } else if pkt.msg_type == "ota_status" {
            if let Ok(mut g) = state.lock() {
                let st = pkt.state.unwrap_or_default();
                g.push_log(format!("Firmware update: {st} {}%", pkt.pct.unwrap_or(0)));
                g.ota = Some(crate::models::OtaProgress::new(&st, pkt.pct.unwrap_or(0), pkt.error));
            }
        } else if pkt.msg_type == "art_req" {
            // The device can't pull art over HTTP (Bluetooth or USB): push it.
            if let (Some(hash), Ok(mut g)) = (pkt.hash.clone(), state.lock()) {
                g.outbound_queue.push_back(crate::models::OutboundAction::SendArt { hash });
            }
        } else if pkt.msg_type == "auth_error" {
            if let Ok(mut g) = state.lock() {
                // Stats go out every second; one log line and banner a minute is plenty.
                let quiet = g.last_auth_notice.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(60));
                if !quiet {
                    let reason = pkt.reason.unwrap_or_default();
                    g.push_log(format!("Device refused a network message: {reason}"));
                    g.last_auth_notice = Some(std::time::Instant::now());
                    g.notice = Some(match reason.as_str() {
                        "stale" => "The device ignored an old message. Try again.".into(),
                        _ => "The device didn't accept this computer. Run Add device again to re-pair it, or use USB.".into(),
                    });
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
                    g.device_has_network = Some(!ssid.is_empty());
                    if !ssid.is_empty() {
                        g.config.wifi_ssid = ssid;
                    }
                }
                // Devices from before the wizard: remember the first one we hear.
                if let Some(id) = pkt.id.filter(|s| !s.is_empty()) {
                    if g.config.device_id.is_empty() {
                        g.config.device_name = format!("HawaOS-{id}");
                        g.config.device_id = id;
                        save_config(&g.config);
                    }
                }
                g.device_needs_setup = pkt.setup.unwrap_or(false);
                // Follow the device's link mode (Device page switch, wizard).
                if let Some(link) = pkt.link.as_deref() {
                    let ble = link == "ble";
                    if g.config.use_ble != ble {
                        g.config.use_ble = ble;
                        save_config(&g.config);
                        g.push_log(format!("Device link: {}", if ble { "Bluetooth" } else { "Wi-Fi" }));
                    }
                }
                if let Some(fw) = pkt.fw.filter(|s| !s.is_empty()) {
                    // The device answering again after "done" (a new version, or
                    // past its restart delay) means the update went through.
                    if g.ota.as_ref().is_some_and(|o| {
                        o.state == "done"
                            && (g.device_fw.as_deref() != Some(fw.as_str())
                                || o.at.elapsed() > std::time::Duration::from_secs(3))
                    }) {
                        g.ota = None;
                    }
                    g.device_fw = Some(fw);
                }
                if let Some(th) = pkt.theme {
                    g.config.device_theme = th;
                }
                // The device answers a push with `v` >= the pushed version, so an
                // ACK below our version predates our latest push. Taking its
                // accent would flash the previous color back after quick clicks.
                let ack_v = pkt.v.unwrap_or(0);
                let stale = ack_v < g.config.config_version;
                if ack_v > g.config.config_version {
                    g.config.config_version = ack_v;
                    save_config(&g.config);
                }
                // Accent can also be cycled on the device; adopt it so both UIs
                // match. The main loop re-themes the window when it changes.
                if let Some(acc) = pkt.accent {
                    if !stale
                        && acc != g.config.accent
                        && crate::ui::accents::ACCENTS.iter().any(|a| a.id == acc)
                    {
                        g.config.accent = acc;
                        save_config(&g.config);
                    }
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
                if let Some(orient) = pkt.orientation {
                    g.config.orientation = orient;
                }
                let ack_ip = g
                    .device_reported_ip
                    .clone()
                    .unwrap_or_else(|| g.config.target_ip.clone());
                let log_msg = format!(
                    "Device ACK v={} (IP: {}, theme: {}, accent: {}, brightness: {}%)",
                    pkt.v.unwrap_or(0),
                    ack_ip,
                    g.config.device_theme,
                    g.config.accent,
                    g.config.brightness
                );
                g.push_log(log_msg);
            }
        }
    }
}

/// Writes several lines on one open port (album art: ~65 lines), then reads replies
/// briefly like `send_usb_line`.
pub fn send_usb_lines(port_name: &str, lines: &[String], state: &SharedState) -> bool {
    if state.lock().map(|g| g.setup_holds_usb).unwrap_or(false) {
        return false;
    }
    let Ok(mut port) = serialport::new(port_name, USB_BAUD).timeout(Duration::from_millis(350)).open() else {
        return false;
    };
    for l in lines {
        if port.write_all(format!("{l}\n").as_bytes()).is_err() {
            return false;
        }
        // The device's USB driver drops what doesn't fit its buffer; pace
        // so its 25 ms poll keeps up (65 art lines: ~0.7 s).
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = port.flush();
    let mut reader = BufReader::new(port);
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(350) {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(n) if n > 0 => handle_incoming_json_line(&line, state),
            _ => break,
        }
    }
    true
}

pub fn send_usb_line(port_name: &str, json_line: &str, state: &SharedState) -> bool {
    if state.lock().map(|g| g.setup_holds_usb).unwrap_or(false) {
        return false; // the setup wizard has the port
    }
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

#[cfg(test)]
mod tests {
    use super::handle_incoming_json_line;
    use crate::models::{CompanionConfig, SharedDaemonState, SharedState};
    use std::sync::{Arc, Mutex};

    fn ack(v: u32, accent: &str) -> String {
        format!(r#"{{"type":"config_ack","v":{v},"accent":"{accent}"}}"#)
    }

    #[test]
    fn stale_ack_does_not_revert_accent() {
        // ACK handling saves the config; keep it away from the real one.
        let home = std::env::temp_dir().join(format!("hawaos-test-{}", std::process::id()));
        unsafe { std::env::set_var("HOME", &home) };

        let cfg = CompanionConfig {
            accent: "blue".into(),
            config_version: 10,
            ..Default::default()
        };
        let state: SharedState = Arc::new(Mutex::new(SharedDaemonState::new(cfg)));
        let accent = |s: &SharedState| s.lock().unwrap().config.accent.clone();

        // ACK for an older push (user clicked again since): ignored.
        handle_incoming_json_line(&ack(9, "red"), &state);
        assert_eq!(accent(&state), "blue");

        // Change made on the device itself (newer version): adopted.
        handle_incoming_json_line(&ack(11, "green"), &state);
        assert_eq!(accent(&state), "green");
        assert_eq!(state.lock().unwrap().config.config_version, 11);

        // Unknown ids are never adopted.
        handle_incoming_json_line(&ack(12, "chartreuse"), &state);
        assert_eq!(accent(&state), "green");

        let _ = std::fs::remove_dir_all(home);
    }
}

#[cfg(test)]
mod fit_tests {
    use super::fit_bytes;

    #[test]
    fn cuts_on_char_boundary() {
        assert_eq!(fit_bytes("héllo".into(), 2), "h"); // 'é' is 2 bytes
        assert_eq!(fit_bytes("hello".into(), 10), "hello");
        assert_eq!(fit_bytes("x".repeat(500), 95).len(), 95);
    }
}
