//! Whether the session is locked, and the default microphone.
//!
//! The lock comes from the screen saver interface on the session bus (KDE and
//! most desktops: org.freedesktop.ScreenSaver; GNOME: org.gnome.ScreenSaver),
//! then logind's LockedHint on the system bus. The microphone comes from
//! `pactl`, which talks to PipeWire (pipewire-pulse) and PulseAudio alike.

use std::process::Command;
use std::sync::OnceLock;

use zbus::blocking::Connection;

use crate::platform::Mic;

fn session_bus() -> Option<&'static Connection> {
    static BUS: OnceLock<Option<Connection>> = OnceLock::new();
    BUS.get_or_init(|| Connection::session().ok()).as_ref()
}

fn system_bus() -> Option<&'static Connection> {
    static BUS: OnceLock<Option<Connection>> = OnceLock::new();
    BUS.get_or_init(|| Connection::system().ok()).as_ref()
}

fn screen_saver_active(bus: &Connection, dest: &str, path: &str, iface: &str) -> Option<bool> {
    let reply = bus.call_method(Some(dest), path, Some(iface), "GetActive", &()).ok()?;
    reply.body().deserialize::<bool>().ok()
}

/// None when no desktop interface answers.
pub fn session_locked() -> Option<bool> {
    let mut answered = None;
    if let Some(bus) = session_bus() {
        for (dest, path) in [
            ("org.freedesktop.ScreenSaver", "/org/freedesktop/ScreenSaver"),
            ("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver"),
        ] {
            match screen_saver_active(bus, dest, path, dest) {
                Some(true) => return Some(true),
                Some(false) => answered = Some(false),
                None => {}
            }
        }
    }
    if let Some(bus) = system_bus() {
        let hint = zbus::blocking::fdo::PropertiesProxy::builder(bus)
            .destination("org.freedesktop.login1")
            .and_then(|b| b.path("/org/freedesktop/login1/session/auto"))
            .and_then(|b| b.build())
            .and_then(|p| {
                p.get(
                    zbus::names::InterfaceName::from_static_str_unchecked("org.freedesktop.login1.Session"),
                    "LockedHint",
                )
                .map_err(Into::into)
            })
            .ok()
            .and_then(|v| bool::try_from(v).ok());
        if hint.is_some() {
            return hint.map(|h| h || answered == Some(true));
        }
    }
    answered
}

fn pactl(args: &[&str]) -> Option<String> {
    let out = Command::new("pactl").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// None without a sound server (or pactl).
pub fn mic() -> Option<Mic> {
    let muted = pactl(&["get-source-mute", "@DEFAULT_SOURCE@"])?.contains("yes");
    // Recording streams on a real input; monitors of an output (visualisers,
    // screen recorders capturing sound) don't count.
    let sources = pactl(&["list", "short", "sources"])?;
    let inputs: Vec<&str> = sources
        .lines()
        .filter_map(|l| {
            let mut cols = l.split('\t');
            let index = cols.next()?;
            let name = cols.next()?;
            (!name.ends_with(".monitor")).then_some(index)
        })
        .collect();
    let in_use = pactl(&["list", "short", "source-outputs"])?
        .lines()
        .filter_map(|l| l.split('\t').nth(1))
        .any(|source| inputs.contains(&source));
    Some(Mic { in_use, muted })
}

pub fn toggle_mic() {
    let _ = pactl(&["set-source-mute", "@DEFAULT_SOURCE@", "toggle"]);
}
