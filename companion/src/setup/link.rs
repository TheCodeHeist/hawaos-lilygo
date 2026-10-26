//! Transports for the setup wizard. Both carry the same newline-delimited JSON
//! as the daemon's USB/UDP links; BLE is only offered by devices in setup mode.

use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use btleplug::api::{
    Central, CharPropFlags, Characteristic, Manager as _, Peripheral as _, ScanFilter, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use uuid::Uuid;

use crate::config::USB_BAUD;
use crate::protocol::scan_serial_ports;

pub const SERVICE_UUID: Uuid = Uuid::from_u128(0x48415741_4f53_4000_8000_000000000001);
const RX_UUID: Uuid = Uuid::from_u128(0x48415741_4f53_4000_8000_000000000002);
const TX_UUID: Uuid = Uuid::from_u128(0x48415741_4f53_4000_8000_000000000003);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Usb,
    Ble,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Endpoint {
    Usb(String),
    Ble(String), // btleplug peripheral id, as text
}

/// A HawaOS device seen during discovery.
#[derive(Clone, Debug, PartialEq)]
pub struct FoundDevice {
    pub id: String,   // "CF69"
    pub name: String, // "HawaOS-CF69"
    pub fw: Option<String>,
    pub needs_setup: Option<bool>,
    pub rssi: Option<i16>,
    pub endpoint: Endpoint,
}

impl FoundDevice {
    fn from_ack(ack: &Value, endpoint: Endpoint) -> Self {
        let id = ack["id"].as_str().unwrap_or("").to_string();
        let name = if id.is_empty() { "HawaOS".to_string() } else { format!("HawaOS-{id}") };
        Self {
            id,
            name,
            fw: ack["fw"].as_str().map(str::to_string),
            needs_setup: ack["setup"].as_bool(),
            rssi: None,
            endpoint,
        }
    }
}

// ---------------------------------------------------------------- USB ----

/// Asks one serial port whether a HawaOS device is behind it (blocking).
pub fn probe_usb(port: &str) -> Option<FoundDevice> {
    let mut p = serialport::new(port, USB_BAUD)
        .timeout(Duration::from_millis(150))
        .open()
        .ok()?;
    p.write_all(b"{\"type\":\"query\"}\n").ok()?;
    let _ = p.flush();
    let mut reader = BufReader::new(p);
    let deadline = Instant::now() + Duration::from_millis(1500);
    let mut line = String::new();
    while Instant::now() < deadline {
        line.clear();
        if reader.read_line(&mut line).is_ok() {
            if let Ok(v) = serde_json::from_str::<Value>(line.trim()) {
                if v["type"] == "config_ack" {
                    return Some(FoundDevice::from_ack(&v, Endpoint::Usb(port.to_string())));
                }
            }
        }
    }
    None
}

pub fn probe_all_usb() -> Vec<FoundDevice> {
    scan_serial_ports().iter().filter_map(|p| probe_usb(p)).collect()
}

pub struct UsbLink {
    writer: Box<dyn serialport::SerialPort>,
    lines: UnboundedReceiver<String>,
    stop: Arc<AtomicBool>,
}

impl UsbLink {
    pub fn open(port: &str) -> Result<Self, String> {
        let writer = serialport::new(port, USB_BAUD)
            .timeout(Duration::from_millis(200))
            .open()
            .map_err(|e| format!("Couldn't open {port}: {e}"))?;
        let reader_port = writer.try_clone().map_err(|e| e.to_string())?;
        let (tx, lines) = unbounded_channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_r = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(reader_port);
            let mut line = String::new();
            while !stop_r.load(Ordering::Relaxed) {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(n) if n > 0 => {
                        if tx.send(line.trim().to_string()).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(_) => break, // unplugged
                }
            }
        });
        Ok(Self { writer, lines, stop })
    }
}

impl Drop for UsbLink {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------- BLE ----

pub async fn ble_adapter() -> Result<Adapter, String> {
    let manager = Manager::new().await.map_err(|e| format!("Bluetooth unavailable: {e}"))?;
    manager
        .adapters()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .next()
        .ok_or_else(|| "No Bluetooth adapter found.".to_string())
}

/// Starts a scan filtered to the HawaOS service. A scan BlueZ already has
/// running (ours from earlier, or another app's) is fine.
pub async fn ble_start_scan(adapter: &Adapter) -> Result<(), String> {
    match adapter.start_scan(ScanFilter { services: vec![SERVICE_UUID] }).await {
        Ok(()) => Ok(()),
        Err(e) if e.to_string().contains("in progress") => Ok(()),
        Err(e) => Err(format!("Bluetooth scan failed: {e}")),
    }
}

/// HawaOS devices the running scan has seen so far.
pub async fn ble_devices(adapter: &Adapter) -> Result<Vec<(FoundDevice, Peripheral)>, String> {
    let mut out = Vec::new();
    for p in adapter.peripherals().await.map_err(|e| e.to_string())? {
        let Ok(Some(props)) = p.properties().await else { continue };
        if !props.services.contains(&SERVICE_UUID) {
            continue;
        }
        let name = props.local_name.clone().unwrap_or_else(|| "HawaOS".to_string());
        let id = name.strip_prefix("HawaOS-").unwrap_or("").to_string();
        out.push((
            FoundDevice {
                id,
                name,
                fw: None,
                needs_setup: Some(true), // only advertises while in setup
                rssi: props.rssi,
                endpoint: Endpoint::Ble(p.id().to_string()),
            },
            p,
        ));
    }
    out.sort_by_key(|(d, _)| std::cmp::Reverse(d.rssi.unwrap_or(-127)));
    Ok(out)
}

pub struct BleLink {
    peripheral: Peripheral,
    rx: Characteristic,
    lines: UnboundedReceiver<String>,
}

impl BleLink {
    pub async fn connect(peripheral: Peripheral) -> Result<Self, String> {
        peripheral
            .connect_with_timeout(Duration::from_secs(12))
            .await
            .map_err(|e| format!("Couldn't connect over Bluetooth: {e}"))?;
        peripheral
            .discover_services_with_timeout(Duration::from_secs(10))
            .await
            .map_err(|e| format!("Bluetooth service discovery failed: {e}"))?;
        let chars = peripheral.characteristics();
        let find = |uuid: Uuid| chars.iter().find(|c| c.uuid == uuid).cloned();
        let rx = find(RX_UUID).ok_or("This device doesn't speak the HawaOS setup protocol.")?;
        let tx = find(TX_UUID).ok_or("This device doesn't speak the HawaOS setup protocol.")?;
        if !tx.properties.contains(CharPropFlags::NOTIFY) {
            return Err("Unexpected HawaOS Bluetooth service.".into());
        }
        peripheral.subscribe(&tx).await.map_err(|e| e.to_string())?;

        let mut notes = peripheral.notifications().await.map_err(|e| e.to_string())?;
        let (line_tx, lines) = unbounded_channel();
        tokio::spawn(async move {
            // Notifications are MTU-sized pieces of newline-terminated JSON.
            let mut buf: Vec<u8> = Vec::new();
            while let Some(n) = notes.next().await {
                if n.uuid != TX_UUID {
                    continue;
                }
                buf.extend_from_slice(&n.value);
                while let Some(i) = buf.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = buf.drain(..=i).collect();
                    let text = String::from_utf8_lossy(&line).trim().to_string();
                    if !text.is_empty() && line_tx.send(text).is_err() {
                        return;
                    }
                }
            }
        });
        Ok(Self { peripheral, rx, lines })
    }
}

// --------------------------------------------------------------- Link ----

pub enum Link {
    Usb(UsbLink),
    Ble(BleLink),
}

impl Link {
    pub async fn send(&mut self, value: &Value) -> Result<(), String> {
        let mut data = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        data.push(b'\n');
        match self {
            Link::Usb(u) => {
                u.writer.write_all(&data).map_err(|e| format!("USB write failed: {e}"))?;
                u.writer.flush().map_err(|e| e.to_string())
            }
            Link::Ble(b) => {
                let chunk = (b.peripheral.mtu() as usize).saturating_sub(3).clamp(20, 240);
                for piece in data.chunks(chunk) {
                    b.peripheral
                        .write(&b.rx, piece, WriteType::WithResponse)
                        .await
                        .map_err(|e| format!("Bluetooth write failed: {e}"))?;
                }
                Ok(())
            }
        }
    }

    /// Next line from the device; `None` once the link is gone.
    pub async fn recv(&mut self) -> Option<String> {
        match self {
            Link::Usb(u) => u.lines.recv().await,
            Link::Ble(b) => b.lines.recv().await,
        }
    }

    pub async fn close(self) {
        if let Link::Ble(b) = self {
            let _ = b.peripheral.disconnect().await;
        }
    }
}
