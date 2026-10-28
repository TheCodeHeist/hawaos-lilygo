//! Whether the session is locked, and the default microphone.
//!
//! The lock comes from the screen saver interface on the session bus (KDE and
//! most desktops: org.freedesktop.ScreenSaver; GNOME: org.gnome.ScreenSaver),
//! then logind's LockedHint on the system bus. The microphone comes from
//! `pactl`, which talks to PipeWire (pipewire-pulse) and PulseAudio alike.

use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

use zbus::blocking::Connection;

use crate::platform::Mic;

/// zbus waits for an answer forever by default, and some names never answer
/// (on KDE, org.gnome.ScreenSaver can be claimed without replying): a stuck
/// call would stall the poll, and the microphone with it.
const CALL_TIMEOUT: Duration = Duration::from_secs(2);

fn session_bus() -> Option<&'static Connection> {
    static BUS: OnceLock<Option<Connection>> = OnceLock::new();
    BUS.get_or_init(|| {
        zbus::blocking::connection::Builder::session()
            .ok()?
            .method_timeout(CALL_TIMEOUT)
            .build()
            .ok()
    })
    .as_ref()
}

fn system_bus() -> Option<&'static Connection> {
    static BUS: OnceLock<Option<Connection>> = OnceLock::new();
    BUS.get_or_init(|| {
        zbus::blocking::connection::Builder::system()
            .ok()?
            .method_timeout(CALL_TIMEOUT)
            .build()
            .ok()
    })
    .as_ref()
}

/// Only asks a name something holds: calling an absent one would have the
/// bus try to start it.
fn screen_saver_active(bus: &Connection, dest: &str, path: &str) -> Option<bool> {
    let owned = bus
        .call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "NameHasOwner",
            &(dest,),
        )
        .ok()?
        .body()
        .deserialize::<bool>()
        .ok()?;
    if !owned {
        return None;
    }
    let reply = bus.call_method(Some(dest), path, Some(dest), "GetActive", &()).ok()?;
    reply.body().deserialize::<bool>().ok()
}

/// None when no desktop interface answers. The first screen saver that
/// answers decides (KDE and most desktops, then GNOME); logind's hint is
/// asked only when neither does, or when the screen saver says unlocked.
pub fn session_locked() -> Option<bool> {
    let mut answered = None;
    if let Some(bus) = session_bus() {
        for (dest, path) in [
            ("org.freedesktop.ScreenSaver", "/org/freedesktop/ScreenSaver"),
            ("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver"),
        ] {
            match screen_saver_active(bus, dest, path) {
                Some(true) => return Some(true),
                Some(false) => {
                    answered = Some(false);
                    break;
                }
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
