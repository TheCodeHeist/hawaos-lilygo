//! Windows: media sessions through the System Media Transport Controls
//! (WinRT), the WLAN API for the setup Wi-Fi scan, autostart in the
//! registry's Run key, and settings in %APPDATA%.
//!
//! Notification mirroring needs a packaged, signed app on Windows
//! (UserNotificationListener); until then it's off (DESIGN.md Section 16).

mod media;
mod tray;
mod wifi;

use std::path::PathBuf;
use std::sync::Once;

pub use media::{media_command, players};
pub use tray::spawn_system_tray;
pub use wifi::scan_wifi_networks;

use crate::models::SharedState;

/// Covers come as image bytes on Windows, never as files to allow.
pub fn art_access_hint(_path: &str) -> String {
    String::new()
}

/// `%APPDATA%\hawaos-companion`.
pub fn config_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hawaos-companion")
}

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "HawaOS Companion";

pub fn is_autostart_installed() -> bool {
    windows_registry::CURRENT_USER
        .open(RUN_KEY)
        .and_then(|k| k.get_string(RUN_VALUE))
        .is_ok()
}

pub fn set_autostart_installed(enable: bool) -> std::io::Result<()> {
    let key = windows_registry::CURRENT_USER.create(RUN_KEY).map_err(std::io::Error::other)?;
    if enable {
        let exe = std::env::current_exe()?;
        key.set_string(RUN_VALUE, format!("\"{}\" --minimized", exe.display()))
            .map_err(std::io::Error::other)
    } else {
        let _ = key.remove_value(RUN_VALUE);
        Ok(())
    }
}

/// The board's USB serial port (Espressif's USB vendor id), else every USB
/// serial port.
pub fn serial_ports() -> Vec<String> {
    const ESPRESSIF: u16 = 0x303A;
    let ports = serialport::available_ports().unwrap_or_default();
    let usb: Vec<(String, u16)> = ports
        .into_iter()
        .filter_map(|p| match p.port_type {
            serialport::SerialPortType::UsbPort(info) => Some((p.port_name, info.vid)),
            _ => None,
        })
        .collect();
    let board: Vec<String> = usb.iter().filter(|(_, vid)| *vid == ESPRESSIF).map(|(n, _)| n.clone()).collect();
    if board.is_empty() { usb.into_iter().map(|(n, _)| n).collect() } else { board }
}

/// Not available yet: Windows only lets packaged, signed apps read other
/// apps' notifications. Says so once in the log.
pub fn spawn_notification_listener(state: SharedState) {
    if let Ok(mut g) = state.lock() {
        g.push_log(
            "Notification mirroring isn't available on Windows yet: it needs a signed app package.".to_string(),
        );
    }
}

/// WinRT on the calling thread (multithreaded apartment), once per thread.
fn winrt_init() {
    thread_local! {
        static INIT: Once = const { Once::new() };
    }
    INIT.with(|once| {
        once.call_once(|| unsafe {
            let _ = windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED);
        })
    });
}
