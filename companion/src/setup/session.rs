use std::time::Duration;

use btleplug::api::Central;
use btleplug::platform::{Adapter, Peripheral};
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedReceiver;

use super::crypto::{KeyExchange, Session, VERIFY_TEXT};
use super::link::{self, BleLink, Endpoint, Link, Method, UsbLink};
use super::{Cmd, Net, SharedWizard, Step, WizardState};
use crate::config::save_config;
use crate::models::{OutboundAction, SharedState};
use crate::protocol::handle_incoming_json_line;

const SEARCH_EVERY: Duration = Duration::from_millis(1500);

struct Ctx {
    daemon: SharedState,
    wiz: SharedWizard,
    link: Option<Link>,
    adapter: Option<Adapter>,
    ble_peripherals: Vec<Peripheral>,
    searching: Option<Method>,
    ble_scanning: bool,
    pending_kx: Option<(KeyExchange, String)>,
    session: Option<Session>,
}

impl Ctx {
    fn update(&self, f: impl FnOnce(&mut WizardState)) {
        let active = if let Ok(mut g) = self.wiz.lock() {
            f(&mut g);
            !matches!(g.step, Step::Welcome | Step::Done)
        } else {
            return;
        };
        // Mid-wizard the window redraws on every tick, not once a second.
        if let Ok(mut d) = self.daemon.lock() {
            d.wizard_active = active;
        }
    }

    fn fail(&self, msg: impl Into<String>) {
        let msg = msg.into();
        self.update(|w| {
            w.error = Some(msg);
            w.busy = false;
        });
    }

    /// The daemon's own USB traffic would collide with ours on the port.
    fn hold_usb(&self, hold: bool) {
        if let Ok(mut g) = self.daemon.lock() {
            g.setup_holds_usb = hold;
        }
    }

    async fn send(&mut self, v: Value) {
        let res = match self.link.as_mut() {
            Some(l) => l.send(&v).await,
            None => Err("Not connected to a device.".into()),
        };
        if let Err(e) = res {
            self.fail(e);
        }
    }

    async fn drop_link(&mut self) {
        if let Some(l) = self.link.take() {
            l.close().await;
        }
        self.session = None;
        self.pending_kx = None;
    }

    async fn stop_search(&mut self) {
        self.searching = None;
        if self.ble_scanning {
            self.ble_scanning = false;
            if let Some(a) = &self.adapter {
                let _ = a.stop_scan().await;
            }
        }
    }

    async fn reset(&mut self) {
        self.stop_search().await;
        self.drop_link().await;
        self.hold_usb(false);
        let method = self.wiz.lock().map(|w| w.method).unwrap_or(Method::Usb);
        self.update(|w| {
            *w = WizardState::default();
            w.method = method;
        });
    }
}

fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "your computer".into())
}

async fn next_line(link: &mut Option<Link>) -> Option<String> {
    match link {
        Some(l) => l.recv().await,
        None => std::future::pending().await,
    }
}

pub async fn run(daemon: SharedState, wiz: SharedWizard, mut cmds: UnboundedReceiver<Cmd>) {
    let mut cx = Ctx {
        daemon,
        wiz,
        link: None,
        adapter: None,
        ble_peripherals: Vec::new(),
        searching: None,
        ble_scanning: false,
        pending_kx: None,
        session: None,
    };
    let mut tick = tokio::time::interval(SEARCH_EVERY);

    // Dev aid for screenshots: `HAWAOS_DEV_WIZARD=usb|ble[:choose[:skip]]`
    // starts discovery, picks the first device, and skips Wi-Fi (USB).
    let dev = std::env::var("HAWAOS_DEV_WIZARD").unwrap_or_default();
    let mut auto_choose = dev.contains(":choose");
    let mut auto_skip = dev.ends_with(":skip");
    match dev.split(':').next() {
        Some("usb") => on_cmd(&mut cx, Cmd::Search(Method::Usb)).await,
        Some("ble") => on_cmd(&mut cx, Cmd::Search(Method::Ble)).await,
        _ => {}
    }

    loop {
        if auto_choose && cx.wiz.lock().map(|w| !w.found.is_empty()).unwrap_or(false) {
            auto_choose = false;
            on_cmd(&mut cx, Cmd::Choose(0)).await;
        }
        if auto_skip && cx.wiz.lock().map(|w| w.step == Step::Wifi).unwrap_or(false) {
            auto_skip = false;
            on_cmd(&mut cx, Cmd::SkipWifi).await;
        }
        tokio::select! {
            cmd = cmds.recv() => match cmd {
                Some(c) => on_cmd(&mut cx, c).await,
                None => break,
            },
            _ = tick.tick(), if cx.searching.is_some() => search_once(&mut cx).await,
            line = next_line(&mut cx.link) => match line {
                Some(l) => on_line(&mut cx, &l).await,
                None => {
                    let done = cx.wiz.lock().map(|w| w.step == Step::Done).unwrap_or(true);
                    cx.drop_link().await;
                    if !done {
                        cx.update(|w| {
                            w.step = Step::Searching;
                            w.busy = false;
                            w.error = Some("The device disconnected. Reconnect it to continue.".into());
                        });
                        cx.searching = Some(cx.wiz.lock().map(|w| w.method).unwrap_or(Method::Usb));
                    }
                }
            },
        }
    }
    cx.reset().await;
}

async fn search_once(cx: &mut Ctx) {
    match cx.searching {
        Some(Method::Usb) => {
            let found = tokio::task::spawn_blocking(link::probe_all_usb).await.unwrap_or_default();
            cx.update(|w| w.found = found);
        }
        Some(Method::Ble) => {
            if cx.adapter.is_none() {
                match link::ble_adapter().await {
                    Ok(a) => cx.adapter = Some(a),
                    Err(e) => {
                        cx.searching = None;
                        cx.fail(e);
                        return;
                    }
                }
            }
            let adapter = cx.adapter.as_ref().expect("adapter");
            if !cx.ble_scanning {
                if let Err(e) = link::ble_start_scan(adapter).await {
                    cx.searching = None;
                    cx.fail(e);
                    return;
                }
                cx.ble_scanning = true;
            }
            match link::ble_devices(adapter).await {
                Ok(list) => {
                    let (found, periph): (Vec<_>, Vec<_>) = list.into_iter().unzip();
                    cx.ble_peripherals = periph;
                    cx.update(|w| w.found = found);
                }
                Err(e) => {
                    cx.searching = None;
                    cx.fail(e);
                }
            }
        }
        None => {}
    }
}

async fn on_cmd(cx: &mut Ctx, cmd: Cmd) {
    match cmd {
        Cmd::Search(method) => {
            cx.reset().await;
            cx.searching = Some(method);
            cx.hold_usb(method == Method::Usb);
            cx.update(|w| {
                w.method = method;
                w.step = Step::Searching;
            });
            search_once(cx).await;
        }
        Cmd::Choose(idx) => {
            let Some(dev) = cx.wiz.lock().ok().and_then(|w| w.found.get(idx).cloned()) else {
                return;
            };
            cx.stop_search().await;
            cx.update(|w| {
                w.step = Step::Connecting;
                w.busy = true;
                w.error = None;
                w.device = Some(dev.clone());
            });
            let opened = match &dev.endpoint {
                Endpoint::Usb(port) => UsbLink::open(port).map(Link::Usb),
                Endpoint::Ble(_) => match cx.ble_peripherals.get(idx).cloned() {
                    Some(p) => BleLink::connect(p).await.map(Link::Ble),
                    None => Err("That device is gone. Search again.".into()),
                },
            };
            match opened {
                Ok(l) => cx.link = Some(l),
                Err(e) => {
                    cx.searching = cx.wiz.lock().ok().map(|w| w.method);
                    cx.update(|w| w.step = Step::Searching);
                    cx.fail(e);
                    return;
                }
            }
            cx.send(json!({"type": "setup", "stage": "begin", "host": hostname()})).await;
            cx.send(json!({"type": "query"})).await;
            let ble = matches!(dev.endpoint, Endpoint::Ble(_));
            cx.update(|w| {
                w.busy = false;
                w.step = if ble { Step::Pairing } else { Step::Wifi };
            });
            if !ble {
                request_scan(cx).await;
            }
        }
        Cmd::SubmitCode(code) => {
            let kx = KeyExchange::new();
            let public = kx.public_b64().to_string();
            cx.pending_kx = Some((kx, code));
            cx.update(|w| {
                w.busy = true;
                w.error = None;
            });
            cx.send(json!({"type": "setup_key", "pub": public})).await;
        }
        Cmd::ScanNets => request_scan(cx).await,
        Cmd::Join { ssid, pass } => {
            let msg = match (&cx.link, &cx.session) {
                (Some(Link::Ble(_)), Some(s)) => {
                    let (nonce, ct) = s.seal(&json!({"ssid": ssid, "pass": pass}).to_string());
                    json!({"type": "wifi_creds", "nonce": nonce, "ct": ct})
                }
                (Some(Link::Ble(_)), None) => {
                    cx.fail("Pair with the device first.");
                    return;
                }
                _ => json!({"type": "wifi_join", "ssid": ssid, "pass": pass}),
            };
            cx.update(|w| {
                w.step = Step::Joining;
                w.joining_ssid = ssid.clone();
                w.error = None;
            });
            cx.send(msg).await;
        }
        Cmd::SkipWifi => cx.update(|w| {
            w.usb_only = true;
            w.step = Step::Personalize;
            w.error = None;
        }),
        Cmd::Personalize { accent, theme, use_24h } => {
            let v = if let Ok(mut g) = cx.daemon.lock() {
                g.config.accent = accent.clone();
                g.config.device_theme = theme.clone();
                g.config.use_24h = use_24h;
                g.config.config_version += 1;
                save_config(&g.config);
                g.config.config_version
            } else {
                return;
            };
            cx.send(json!({
                "type": "config", "v": v, "accent": accent, "theme": theme, "use_24h": use_24h,
            }))
            .await;
        }
        Cmd::Finish => finish(cx).await,
        Cmd::Cancel => cx.reset().await,
    }
}

async fn request_scan(cx: &mut Ctx) {
    cx.update(|w| w.scanning_nets = true);
    cx.send(json!({"type": "wifi_scan"})).await;
}

async fn finish(cx: &mut Ctx) {
    // Token that authorizes later firmware updates over WiFi; sealed with the
    // pairing key over BLE, plain over USB (physical access).
    let token = crate::ota::new_token();
    let done = match (&cx.link, &cx.session) {
        (Some(Link::Ble(_)), Some(s)) => {
            let (nonce, ct) = s.seal(&token);
            json!({"type": "setup", "stage": "done", "token_nonce": nonce, "token_ct": ct})
        }
        _ => json!({"type": "setup", "stage": "done", "token": token}),
    };
    cx.send(done).await;
    // The device answers with a config_ack (setup: false); give it a moment.
    let _ = tokio::time::timeout(Duration::from_secs(2), async {
        while let Some(l) = next_line(&mut cx.link).await {
            let done = l.contains("config_ack");
            on_line(cx, &l).await;
            if done {
                break;
            }
        }
    })
    .await;

    let w = cx.wiz.lock().map(|w| w.clone()).unwrap_or_default();
    if let (Ok(mut g), Some(dev)) = (cx.daemon.lock(), w.device.as_ref()) {
        g.config.device_id = dev.id.clone();
        g.config.device_name = dev.name.clone();
        if let Endpoint::Usb(port) = &dev.endpoint {
            g.config.usb_port = port.clone();
        }
        g.config.stream_via_usb = w.usb_only;
        g.config.device_token = token;
        if let Some(ip) = &w.device_ip {
            g.config.target_ip = ip.clone();
            g.device_reported_ip = Some(ip.clone());
        }
        save_config(&g.config);
        g.push_log(format!("Set up {} ({})", dev.name, w.device_ip.as_deref().unwrap_or("USB only")));
        g.outbound_queue.push_back(OutboundAction::QueryDevice { via_usb: w.usb_only });
    }
    cx.drop_link().await;
    cx.hold_usb(false);
    cx.update(|w| {
        w.step = Step::Done;
        w.busy = false;
    });
}

async fn on_line(cx: &mut Ctx, line: &str) {
    let Ok(msg) = serde_json::from_str::<Value>(line) else {
        return;
    };
    match msg["type"].as_str().unwrap_or("") {
        "config_ack" => {
            handle_incoming_json_line(line, &cx.daemon); // keeps the daemon's view in sync
            let fw = msg["fw"].as_str().map(str::to_string);
            let id = msg["id"].as_str().map(str::to_string);
            let ip = msg["ip"].as_str().filter(|ip| *ip != "0.0.0.0" && msg["wifi"] == true);
            let ip = ip.map(str::to_string);
            cx.update(|w| {
                if let Some(d) = w.device.as_mut() {
                    if fw.is_some() {
                        d.fw = fw;
                    }
                    if let Some(id) = id.filter(|s| !s.is_empty()) {
                        d.name = format!("HawaOS-{id}");
                        d.id = id;
                    }
                }
                if ip.is_some() {
                    w.device_ip = ip;
                }
            });
        }
        "setup_key" => {
            let Some((kx, code)) = cx.pending_kx.take() else { return };
            let Some(session) = msg["pub"].as_str().and_then(|p| kx.finish(p, &code)) else {
                cx.fail("The device rejected the pairing request. Try again.");
                return;
            };
            let (nonce, ct) = session.seal(VERIFY_TEXT);
            cx.session = Some(session);
            cx.send(json!({"type": "setup_verify", "nonce": nonce, "ct": ct})).await;
        }
        "setup_verify" => {
            if msg["ok"] == true {
                cx.update(|w| {
                    w.busy = false;
                    w.error = None;
                    w.step = Step::Wifi;
                });
                request_scan(cx).await;
            } else {
                cx.session = None;
                let left = msg["attempts_left"].as_u64().unwrap_or(0);
                cx.fail(if left >= 5 {
                    "Too many wrong codes. The device is showing a new one.".to_string()
                } else {
                    format!("That code didn't match. {left} tries left.")
                });
            }
        }
        "wifi_scan" => {
            let mut nets: Vec<Net> = msg["nets"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|n| {
                            Some(Net {
                                ssid: n["ssid"].as_str()?.to_string(),
                                rssi: n["rssi"].as_i64().unwrap_or(-100) as i32,
                                secure: n["secure"].as_bool().unwrap_or(true),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            nets.sort_by_key(|n| std::cmp::Reverse(n.rssi));
            cx.update(|w| {
                w.nets = nets;
                w.scanning_nets = false;
            });
        }
        "wifi_status" => {
            let ssid = msg["ssid"].as_str().unwrap_or("").to_string();
            match msg["state"].as_str().unwrap_or("") {
                "connected" => {
                    let ip = msg["ip"].as_str().map(str::to_string);
                    if let (Some(ip), Ok(mut g)) = (ip.as_ref(), cx.daemon.lock()) {
                        g.config.wifi_ssid = ssid.clone();
                        g.device_reported_ip = Some(ip.clone());
                        g.device_wifi_connected = Some(true);
                    }
                    cx.update(|w| {
                        w.device_ip = ip;
                        w.usb_only = false;
                        w.step = Step::Personalize;
                        w.error = None;
                    });
                }
                "failed" => {
                    let err = match msg["reason"].as_str().unwrap_or("") {
                        "auth" => format!("Wrong password for {ssid}."),
                        "no_ap" => format!("The device can't see {ssid}. Move it closer or pick another network."),
                        "decrypt" => "The pairing expired. Cancel and start again.".to_string(),
                        _ => format!("{ssid} didn't respond. Try again."),
                    };
                    cx.update(|w| {
                        w.step = Step::Wifi;
                        w.error = Some(err);
                    });
                }
                _ => {}
            }
        }
        _ => {}
    }
}
