//! One companion per user. A second launch (autostart while it's already
//! running, or the app menu again) hands over to the running one and exits:
//! two copies would fight over the device's ports and links.
//!
//! The running companion listens on a local socket named for the user (an
//! abstract Unix socket on Linux, shared with the Flatpak; a named pipe on
//! Windows). Both vanish when the process does, so a crash leaves nothing
//! stale behind.

use std::io::{BufRead, BufReader, Write};
use std::thread;

use interprocess::local_socket::{prelude::*, GenericNamespaced, ListenerOptions, Name, Stream};

use crate::models::SharedState;

/// What a second launch asks of the running companion.
const OPEN: &str = "open";

fn name() -> std::io::Result<Name<'static>> {
    format!("dev.is_a.thecodeheist.HawaOSCompanion.{}", user()).to_ns_name::<GenericNamespaced>()
}

/// Names the user, so each user on a machine gets their own companion.
fn user() -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata("/proc/self").map(|m| m.uid().to_string()).unwrap_or_default()
    }
    #[cfg(windows)]
    {
        std::env::var("USERNAME").unwrap_or_default()
    }
}

/// This launch is a second one: tells the running companion (to show its
/// window unless `quietly`) and returns true, and the caller exits.
fn hand_over(quietly: bool) -> bool {
    let Ok(name) = name() else { return false };
    let Ok(mut stream) = Stream::connect(name) else { return false };
    if !quietly {
        let _ = stream.write_all(format!("{OPEN}\n").as_bytes());
    }
    true
}

/// The single-instance guard. Returns false if another companion is already
/// running (after handing over to it): the caller should exit. Otherwise
/// this one listens for later launches until it exits.
///
/// `minimized` launches (autostart) don't bring up the running one's window.
pub fn claim(minimized: bool, state: &SharedState) -> bool {
    if hand_over(minimized) {
        return false;
    }
    let listener = match name().and_then(|n| ListenerOptions::new().name(n).create_sync()) {
        Ok(l) => l,
        // Another launch claimed it a moment ago.
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => return !hand_over(minimized),
        // Without the guard the companion still works; it just can't tell.
        Err(e) => {
            eprintln!("[companion] Single-instance guard unavailable: {e}");
            return true;
        }
    };
    let state = std::sync::Arc::clone(state);
    thread::spawn(move || {
        for conn in listener.incoming().filter_map(Result::ok) {
            let mut line = String::new();
            if BufReader::new(conn).read_line(&mut line).is_ok() && line.trim() == OPEN {
                if let Ok(mut g) = state.lock() {
                    g.open_window_requested = true;
                }
            }
        }
    });
    true
}
