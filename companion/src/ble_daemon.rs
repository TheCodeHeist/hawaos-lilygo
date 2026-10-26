//! Everyday Bluetooth link (DESIGN.md Section 11.7): when the device runs in
//! Bluetooth mode, this keeps a connection to it and carries the same sealed
//! envelopes the worker would send over UDP. It steps aside while the setup
//! wizard runs, and reconnects whenever the link drops.

use std::sync::Mutex;
use std::time::Duration;

use btleplug::api::Peripheral as _;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::models::SharedState;
use crate::protocol::handle_incoming_json_line;
use crate::setup::link::{BleLink, Link, ble_adapter, ble_devices, ble_start_scan};

/// Lines for the device while connected; `None` otherwise.
static TX: Mutex<Option<UnboundedSender<String>>> = Mutex::new(None);

/// Queues a line for the device. Callers seal it first (except art_data,
/// which carries an already-encrypted blob). false when not connected.
pub fn send_line(line: String) -> bool {
    TX.lock().ok().and_then(|g| g.as_ref().map(|tx| tx.send(line).is_ok())).unwrap_or(false)
}

pub fn spawn(state: SharedState) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build();
        match rt {
            Ok(rt) => rt.block_on(run(state)),
            Err(e) => eprintln!("[BleLink] No runtime: {e}"),
        }
    });
}

/// How quiet the device must be in Wi-Fi mode before we look for it here:
/// it may have switched to Bluetooth with its ack lost (DESIGN.md 11.7).
const PROBE_AFTER: Duration = Duration::from_secs(45);

/// The device name to look for, or None while Bluetooth isn't wanted. The
/// flag says whether this is only a probe from Wi-Fi mode.
fn wanted(state: &SharedState) -> Option<(String, bool)> {
    let g = state.lock().ok()?;
    let c = &g.config;
    if g.wizard_active || c.device_id.is_empty() || c.device_token.is_empty() {
        return None;
    }
    let probe = !c.use_ble && g.device_silence() > PROBE_AFTER;
    (c.use_ble || probe).then(|| (format!("HawaOS-{}", c.device_id), probe))
}

fn set_connected(state: &SharedState, connected: bool, msg: &str) {
    if let Ok(mut g) = state.lock() {
        if g.ble_connected != connected {
            g.ble_connected = connected;
            g.push_log(msg.to_string());
        }
    }
}

async fn run(state: SharedState) {
    loop {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let Some((name, probe)) = wanted(&state) else { continue };
        if probe {
            // A device in Wi-Fi mode doesn't advertise; don't scan nonstop.
            tokio::time::sleep(Duration::from_secs(28)).await;
            if wanted(&state).is_none() {
                continue;
            }
        }
        match connect(&name).await {
            Ok(link) => {
                set_connected(&state, true, &format!("Bluetooth link to {name} up"));
                serve(link, &state, &name).await;
                set_connected(&state, false, "Bluetooth link down");
            }
            Err(e) => {
                if let Ok(mut g) = state.lock() {
                    g.ble_connected = false;
                    if g.logs.back().map(|l| l != &e).unwrap_or(true) {
                        g.push_log(e);
                    }
                }
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
}

async fn connect(name: &str) -> Result<Link, String> {
    let adapter = ble_adapter().await?;
    ble_start_scan(&adapter).await?;
    for _ in 0..16 {
        // A device in setup belongs to the wizard, not the everyday link.
        let found = ble_devices(&adapter)
            .await?
            .into_iter()
            .find(|(d, _)| d.name == name && d.needs_setup != Some(true));
        if let Some((_, p)) = found {
            if p.is_connected().await.unwrap_or(false) {
                let _ = p.disconnect().await; // stale connection from an earlier run
            }
            return BleLink::connect(p).await.map(|l| Link::Ble(l.fast()));
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    Err(format!("Bluetooth: {name} not in range"))
}

async fn serve(mut link: Link, state: &SharedState, name: &str) {
    let (tx, mut rx) = unbounded_channel::<String>();
    // Ask for an ack right away so the window shows the device. Queued like
    // everything else, so envelopes leave in the order they were sealed.
    if let Some(q) = token(state).and_then(|t| crate::auth::seal(&t, r#"{"type":"query"}"#)) {
        let _ = tx.send(q);
    }
    if let Ok(mut g) = TX.lock() {
        *g = Some(tx);
    }
    let mut check = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {
            out = rx.recv() => {
                let Some(line) = out else { break };
                if let Err(e) = link.send_raw(&line).await {
                    if let Ok(mut g) = state.lock() { g.push_log(e); }
                    break;
                }
            }
            line = link.recv() => {
                let Some(line) = line else { break };
                if let Some(plain) = open(&line, state) {
                    handle_incoming_json_line(&plain, state);
                }
            }
            _ = check.tick() => {
                if wanted(state).map(|(n, _)| n).as_deref() != Some(name) {
                    break; // switched to Wi-Fi, or the wizard needs the radio
                }
            }
        }
    }
    if let Ok(mut g) = TX.lock() {
        *g = None;
    }
    link.close().await;
}

fn token(state: &SharedState) -> Option<String> {
    state.lock().ok().map(|g| g.config.device_token.clone()).filter(|t| !t.is_empty())
}

/// Same rule as UDP: sealed messages, or a plaintext refusal.
fn open(line: &str, state: &SharedState) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    match v["type"].as_str()? {
        "sealed" => crate::auth::open_from_device(&token(state)?, &v),
        "auth_error" => Some(line.to_string()),
        _ => None,
    }
}
