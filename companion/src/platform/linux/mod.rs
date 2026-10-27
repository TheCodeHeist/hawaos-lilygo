//! Linux: MPRIS media players through playerctl, notifications from the
//! session bus, XDG autostart and config folders, NetworkManager's Wi-Fi scan.

mod notifications;

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub use notifications::spawn_notification_listener;

use super::{Art, MediaCommand, Player};
use crate::config::{DiscoveredWifi, APP_ID};

pub use crate::tray::spawn_system_tray;

/// Every MPRIS player, through playerctl.
pub fn players() -> Vec<Player> {
    let Ok(out) = Command::new("playerctl")
        .args([
            "-a",
            "metadata",
            "--format",
            "{{playerName}}\t{{status}}\t{{title}}\t{{artist}}\t{{album}}\t{{position}}\t{{mpris:length}}\t{{mpris:artUrl}}",
        ])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.trim().split('\t').collect();
            if parts.len() < 7 {
                return None;
            }
            let art = parts.get(7).map(|u| u.trim()).unwrap_or("");
            Some(Player {
                name: parts[0].trim().to_string(),
                status: parts[1].trim().to_lowercase(),
                // As the player gave them; the worker fits them to the
                // device (glyphs.rs).
                title: parts[2].trim().to_string(),
                artist: parts[3].trim().to_string(),
                album: parts[4].trim().to_string(),
                pos_us: parts[5].trim().parse().unwrap_or(0),
                dur_us: parts[6].trim().parse().unwrap_or(0),
                art: if art.is_empty() { Art::None } else { Art::Url(art.to_string()) },
            })
        })
        .collect()
}

pub fn media_command(cmd: MediaCommand, player: Option<&str>) {
    let arg = match cmd {
        MediaCommand::PlayPause => "play-pause",
        MediaCommand::Next => "next",
        MediaCommand::Previous => "previous",
    };
    let mut command = Command::new("playerctl");
    if let Some(player) = player.filter(|p| !p.is_empty()) {
        command.args(["-p", player]);
    }
    command.arg(arg);
    let _ = command.spawn();
}

/// In the Flatpak, a cover outside the folders it may read looks missing:
/// say how to allow the folder it's in.
pub fn art_access_hint(path: &str) -> String {
    if !std::path::Path::new("/.flatpak-info").exists() {
        return String::new();
    }
    let dir = std::path::Path::new(path).parent().map(|d| d.display().to_string()).unwrap_or_default();
    format!(". The Flatpak may not see that folder; to allow it: flatpak override --user --filesystem={dir}:ro {APP_ID}")
}

/// `~/.config/hawaos-companion`, shared by the Flatpak and a source build.
pub fn config_dir() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".config").join("hawaos-companion")
}

fn autostart_desktop_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("autostart")
        .join("hawaos-companion.desktop")
}

pub fn is_autostart_installed() -> bool {
    autostart_desktop_path().exists()
}

pub fn set_autostart_installed(enable: bool) -> std::io::Result<()> {
    let path = autostart_desktop_path();
    if enable {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Inside a Flatpak the binary's path only exists in the sandbox.
        let exec = match env::var("FLATPAK_ID") {
            Ok(id) if !id.is_empty() => format!("flatpak run {id}"),
            _ => format!(
                "\"{}\"",
                env::current_exe()
                    .unwrap_or_else(|_| PathBuf::from("hawaos-companion"))
                    .display()
            ),
        };
        let desktop_entry = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=HawaOS Companion\n\
             Comment=HawaOS Desktop Companion & Tray Daemon for LilyGo T-Display-S3\n\
             Exec={} --minimized\n\
             Icon={}\n\
             Terminal=false\n\
             Categories=Utility;System;\n\
             StartupNotify=false\n\
             X-GNOME-Autostart-enabled=true\n",
            exec, APP_ID
        );
        fs::write(path, desktop_entry)?;
    } else if path.exists() {
        let _ = fs::remove_file(path);
    }
    Ok(())
}


pub fn scan_wifi_networks() -> Vec<DiscoveredWifi> {
    let out = Command::new("nmcli")
        .args([
            "-t",
            "-f",
            "IN-USE,SSID,SIGNAL,SECURITY,FREQ",
            "dev",
            "wifi",
            "list",
            "--rescan",
            "no",
        ])
        .output();
    let mut list = Vec::new();
    if let Ok(output) = out {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let mut seen = HashSet::new();
            for line in text.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 5 {
                    let in_use = parts[0].trim() == "*";
                    let ssid = parts[1].trim();
                    let signal: u8 = parts[2].trim().parse().unwrap_or(0);
                    let security = parts[3].trim().to_string();
                    let freq = parts[4].trim();

                    if ssid.is_empty() {
                        continue;
                    }
                    // Filter for 2.4 GHz frequencies (e.g. 2412..2484 MHz) supported by ESP32-S3
                    let is_24ghz = freq.starts_with("24") || freq.contains("2.4");
                    if !is_24ghz {
                        continue;
                    }
                    if seen.insert(ssid.to_string()) {
                        list.push(DiscoveredWifi {
                            ssid: ssid.to_string(),
                            signal,
                            security,
                            in_use,
                        });
                    }
                }
            }
        }
    }
    list.sort_by(|a, b| b.signal.cmp(&a.signal));
    list.truncate(10);
    list
}


pub fn serial_ports() -> Vec<String> {
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

