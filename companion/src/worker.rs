use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, UdpSocket};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{Networks, System};

use crate::config::CompanionConfig;
use crate::media::query_mpris_media;
use crate::config::{get_local_lan_ip, ART_HTTP_PORT, DEFAULT_DEVICE_IP, UDP_PORT, USB_BAUD};
use crate::models::{
    MediaMsg, OtaProgress, OutboundAction, SharedArtCache, SharedState, StatsMsg, WeatherMsg,
};
use crate::config::sanitize_ip_str;
use crate::protocol::{
    fit_bytes, handle_incoming_json_line, sanitize_for_font, send_usb_line,
};
use crate::weather::trigger_weather_update;

pub fn spawn_daemon_worker(state: SharedState, art_cache: SharedArtCache) {
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
        let mut last_weather_fetch = Instant::now();
        let mut last_weather_send = Instant::now();
        let mut prev_media: Option<MediaMsg> = None;
        let mut prev_weather: Option<WeatherMsg> = None;
        let mut rx_buf = [0u8; 1460];

        trigger_weather_update(&state);

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
            let actions: Vec<(OutboundAction, CompanionConfig, Option<String>)> = if let Ok(mut g) = state.lock() {
                let cfg = g.config.clone();
                let dev_ip = g.device_reported_ip.clone();
                let mut drained = Vec::new();
                while let Some(act) = g.outbound_queue.pop_front() {
                    drained.push((act, cfg.clone(), dev_ip.clone()));
                }
                drained
            } else {
                Vec::new()
            };

            for (act, cfg, dev_ip) in actions {
                let target_ip_str = dev_ip.unwrap_or_else(|| sanitize_ip_str(&cfg.target_ip));
                let target_addr: Option<SocketAddr> =
                    format!("{}:{}", target_ip_str, UDP_PORT)
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
                            "accent": cfg.accent,
                            "brightness": cfg.brightness,
                            "aod": cfg.aod_enabled,
                            "aod_timeout_s": cfg.aod_timeout_s,
                            "reduce_motion": cfg.reduce_motion,
                            "wallpaper": cfg.wallpaper,
                            "use_24h": cfg.use_24h,
                            "notify_popup": cfg.notify_popup,
                            "orientation": cfg.orientation,
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
                                let _ = send_udp(&udp, addr, &json_str, &cfg.device_token);
                            }
                        }
                    }
                    OutboundAction::SendNotification { app, title, body } => {
                        let is_clock_running = if let Ok(g) = state.lock() {
                            g.stopwatch_state == "running" || g.timer_state == "running"
                        } else {
                            false
                        };
                        if is_clock_running {
                            eprintln!("[Daemon] Suppressing notification '{}' because timer/stopwatch is active", title);
                            continue;
                        }
                        let ts = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let pkt = serde_json::json!({
                            "type": "notify",
                            "app": fit_bytes(sanitize_for_font(&app), 31),
                            "title": fit_bytes(sanitize_for_font(&title), 47),
                            "body": fit_bytes(sanitize_for_font(&body), 95),
                            "ts": ts
                        });
                        if let Ok(json_str) = serde_json::to_string(&pkt) {
                            eprintln!("[Daemon] Dispatching notification to {:?}: {}", target_addr, json_str);
                            if let Some(addr) = target_addr {
                                let _ = send_udp(&udp, addr, &json_str, &cfg.device_token);
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
                            let _ = send_udp(&udp, addr, pkt, &cfg.device_token);
                        }
                    }
                    OutboundAction::RefreshWeather => {
                        trigger_weather_update(&state);
                    }
                    OutboundAction::StartOta { path } => {
                        let res = start_ota(&path, &cfg, target_addr, &udp, &state);
                        if let Ok(mut g) = state.lock() {
                            g.ota = Some(match res {
                                Ok(via) => {
                                    g.push_log(format!("Firmware update sent over {via}"));
                                    OtaProgress::new("starting", 0, None)
                                }
                                Err(e) => OtaProgress::new("failed", 0, Some(e)),
                            });
                        }
                    }
                    OutboundAction::SendClockAction { action, duration_sec, duration_ms } => {
                        let pkt = serde_json::json!({
                            "type": "clock",
                            "action": action,
                            "duration": duration_sec,
                            "duration_ms": duration_ms
                        });
                        if let Ok(json_str) = serde_json::to_string(&pkt) {
                            eprintln!("[Daemon] Dispatching clock action to {:?}: {}", target_addr, json_str);
                            if let Some(addr) = target_addr {
                                let _ = send_udp(&udp, addr, &json_str, &cfg.device_token);
                            }
                            if cfg.stream_via_usb {
                                let _ = send_usb_line(&cfg.usb_port, &json_str, &state);
                            }
                        }
                    }
                }
            }

            // 3. 1 Hz Telemetry + Media tick
            if last_stats_tick.elapsed() >= Duration::from_secs(1) {
                if let Ok(mut g) = state.lock() {
                    g.tick_clock();
                    // After an update, ask until the restarted device answers;
                    // its config_ack clears the status (protocol.rs).
                    if let Some(since) = g.ota.as_ref().filter(|o| o.state == "done").map(|o| o.at.elapsed()) {
                        if since > Duration::from_secs(60) {
                            g.ota = None;
                        } else if since > Duration::from_secs(3) {
                            let via_usb = std::path::Path::new(&g.config.usb_port).exists();
                            g.outbound_queue.push_back(OutboundAction::QueryDevice { via_usb });
                        }
                    }
                }
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

                let (target_ip, stream_usb, usb_port, token) = if let Ok(mut g) = state.lock() {
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
                        g.config.device_token.clone(),
                    )
                } else {
                    (DEFAULT_DEVICE_IP.to_string(), false, String::new(), String::new())
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
                        let _ = send_udp(&udp, addr, &json_str, &token);
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
                            let _ = send_udp(&udp, addr, &json_str, &token);
                        }
                        if stream_usb {
                            let _ = send_usb_line(&usb_port, &json_str, &state);
                        }
                    }
                    prev_media = Some(cur_media);
                    last_media_heartbeat = Instant::now();
                }

                let clock_pkt = if let Ok(g) = state.lock() {
                    if g.stopwatch_state == "running" || g.timer_state == "running" || g.timer_state == "paused" || g.stopwatch_state == "paused" {
                        Some(serde_json::json!({
                            "type": "clock",
                            "stopwatch_state": g.stopwatch_state,
                            "stopwatch_ms": g.stopwatch_elapsed_ms,
                            "timer_state": g.timer_state,
                            "timer_duration_sec": g.timer_duration_sec,
                            "timer_remaining_sec": g.timer_remaining_sec
                        }))
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(pkt) = clock_pkt {
                    if let Ok(json_str) = serde_json::to_string(&pkt) {
                        if let Some(addr) = target_addr {
                            let _ = send_udp(&udp, addr, &json_str, &token);
                        }
                        if stream_usb {
                            let _ = send_usb_line(&usb_port, &json_str, &state);
                        }
                    }
                }

                if last_weather_fetch.elapsed() >= Duration::from_secs(900) {
                    last_weather_fetch = Instant::now();
                    trigger_weather_update(&state);
                }

                let cur_weather = if let Ok(g) = state.lock() {
                    g.weather.clone()
                } else {
                    None
                };

                if let Some(w) = cur_weather {
                    let weather_changed = match &prev_weather {
                        Some(prev) => prev != &w,
                        None => true,
                    };
                    if weather_changed || last_weather_send.elapsed() >= Duration::from_secs(60) {
                        if let Ok(json_str) = serde_json::to_string(&w) {
                            if let Some(addr) = target_addr {
                                let _ = send_udp(&udp, addr, &json_str, &token);
                            }
                            if stream_usb {
                                let _ = send_usb_line(&usb_port, &json_str, &state);
                            }
                        }
                        prev_weather = Some(w);
                        last_weather_send = Instant::now();
                    }
                }
            }

            thread::sleep(Duration::from_millis(50));
        }
    });
}

/// Stages `path` and asks the device to fetch it. Over USB physical access is
/// the authorization; over WiFi the request is signed with the setup token.
fn start_ota(
    path: &std::path::Path,
    cfg: &CompanionConfig,
    target: Option<SocketAddr>,
    udp: &UdpSocket,
    state: &SharedState,
) -> Result<&'static str, String> {
    let wifi = state.lock().ok().and_then(|g| g.device_wifi_connected);
    if wifi == Some(false) {
        return Err("The device needs WiFi to download updates.".into());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Couldn't read {}: {e}", path.display()))?;
    let (id, sha256, size) = crate::ota::stage(bytes)?;
    let url = format!("http://{}:{ART_HTTP_PORT}/fw/{id}.bin", get_local_lan_ip());
    let mut msg = serde_json::json!({"type": "ota", "url": url, "size": size, "sha256": sha256});

    if std::path::Path::new(&cfg.usb_port).exists()
        && send_usb_line(&cfg.usb_port, &msg.to_string(), state)
    {
        return Ok("USB");
    }
    if cfg.device_token.is_empty() {
        return Err("Wireless updates need a device added with Add device. Plug it in over USB, or run the wizard again.".into());
    }
    let mac = crate::ota::sign(&cfg.device_token, &sha256, size).ok_or("Bad device token.")?;
    msg["mac"] = serde_json::Value::String(mac);
    let addr = target.ok_or("No device address.")?;
    send_udp(udp, addr, &msg.to_string(), &cfg.device_token)
        .map_err(|e| format!("Couldn't reach the device: {e}"))?;
    Ok("WiFi")
}

/// Sends a JSON message over UDP, signed once the device has a token
/// (DESIGN.md Section 11.6).
fn send_udp(udp: &UdpSocket, addr: SocketAddr, json: &str, token: &str) -> std::io::Result<()> {
    let sealed;
    let pkt = if token.is_empty() {
        json
    } else {
        sealed = crate::auth::seal(token, json)
            .ok_or_else(|| std::io::Error::other("bad device token"))?;
        sealed.as_str()
    };
    udp.send_to(pkt.as_bytes(), addr).map(|_| ())
}
