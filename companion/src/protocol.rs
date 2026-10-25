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
                if let Some(orient) = pkt.orientation {
                    g.config.orientation = orient;
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

pub fn send_usb_line(port_name: &str, json_line: &str, state: &SharedState) -> bool {
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
