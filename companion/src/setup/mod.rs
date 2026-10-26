//! "Add device" wizard backend (DESIGN.md Section 11.4). A session thread
//! owns one live link (USB or BLE) for the whole wizard and drives the device
//! through discovery, pairing, WiFi and personalization; the UI sends
//! [`Cmd`]s and renders the shared [`WizardState`].

mod crypto;
pub mod link;
mod session;

use std::sync::{Arc, Mutex};

use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

pub use link::{FoundDevice, Method};

use crate::models::SharedState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Welcome,
    Searching,
    Connecting,
    Pairing,
    Wifi,
    Joining,
    Personalize,
    Done,
}

impl Step {
    /// Position in the stepper (Connect, Pair, Wi-Fi, Personalize, Done).
    pub fn stage(self) -> usize {
        match self {
            Step::Welcome | Step::Searching | Step::Connecting => 0,
            Step::Pairing => 1,
            Step::Wifi | Step::Joining => 2,
            Step::Personalize => 3,
            Step::Done => 4,
        }
    }
}

/// A network as the device sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct Net {
    pub ssid: String,
    pub rssi: i32,
    pub secure: bool,
}

#[derive(Clone, Debug)]
pub struct WizardState {
    pub step: Step,
    pub method: Method,
    pub found: Vec<FoundDevice>,
    pub device: Option<FoundDevice>,
    pub nets: Vec<Net>,
    pub scanning_nets: bool,
    pub joining_ssid: String,
    pub device_ip: Option<String>,
    pub usb_only: bool,
    /// Everyday link over Bluetooth instead of Wi-Fi (DESIGN.md 11.7).
    pub use_ble: bool,
    pub busy: bool,
    pub error: Option<String>,
}

impl Default for WizardState {
    fn default() -> Self {
        Self {
            step: Step::Welcome,
            method: Method::Usb,
            found: Vec::new(),
            device: None,
            nets: Vec::new(),
            scanning_nets: false,
            joining_ssid: String::new(),
            device_ip: None,
            usb_only: false,
            use_ble: false,
            busy: false,
            error: None,
        }
    }
}

#[derive(Debug)]
pub enum Cmd {
    Search(Method),
    Choose(usize),
    SubmitCode(String),
    ScanNets,
    Join { ssid: String, pass: String },
    SkipWifi,
    UseBluetooth,
    Personalize { accent: String, theme: String, use_24h: bool },
    Finish,
    Cancel,
}

pub type SharedWizard = Arc<Mutex<WizardState>>;

/// Handle kept by the window: send commands, read state.
#[derive(Clone)]
pub struct Wizard {
    pub state: SharedWizard,
    cmds: UnboundedSender<Cmd>,
}

impl Wizard {
    pub fn spawn(daemon: SharedState) -> Self {
        let state: SharedWizard = Arc::new(Mutex::new(WizardState::default()));
        let (cmds, rx) = unbounded_channel();
        let st = Arc::clone(&state);
        std::thread::Builder::new()
            .name("setup-wizard".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime");
                rt.block_on(session::run(daemon, st, rx));
            })
            .expect("spawn setup thread");
        Self { state, cmds }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.cmds.send(cmd);
    }

    pub fn snapshot(&self) -> WizardState {
        self.state.lock().map(|g| g.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
mod hardware_tests {
    //! Needs a HawaOS on USB: `cargo test -- --ignored usb_only_setup`
    //! (or `usb_setup_bluetooth`). Both give the device a new token.
    use super::*;
    use crate::models::{CompanionConfig, SharedDaemonState};
    use std::time::{Duration, Instant};

    fn wait_for(w: &Wizard, what: &str, mut f: impl FnMut(&WizardState) -> bool) -> WizardState {
        let start = Instant::now();
        loop {
            let s = w.snapshot();
            if f(&s) {
                return s;
            }
            assert!(start.elapsed() < Duration::from_secs(20), "timed out waiting for {what}: {s:?}");
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    #[test]
    #[ignore]
    fn usb_only_setup() {
        let home = std::env::temp_dir().join(format!("hawaos-hil-{}", std::process::id()));
        unsafe { std::env::set_var("HOME", &home) };
        let daemon = Arc::new(Mutex::new(SharedDaemonState::new(CompanionConfig::default())));
        let w = Wizard::spawn(Arc::clone(&daemon));

        w.send(Cmd::Search(Method::Usb));
        let s = wait_for(&w, "a USB device", |s| !s.found.is_empty());
        eprintln!("found: {:?}", s.found);
        w.send(Cmd::Choose(0));
        let s = wait_for(&w, "wifi list", |s| s.step == Step::Wifi && !s.nets.is_empty());
        eprintln!("device sees {} networks", s.nets.len());

        w.send(Cmd::SkipWifi);
        wait_for(&w, "personalize", |s| s.step == Step::Personalize);
        w.send(Cmd::Personalize { accent: "emerald".into(), theme: "dark".into(), use_24h: false });
        std::thread::sleep(Duration::from_millis(800));
        w.send(Cmd::Finish);
        let s = wait_for(&w, "done", |s| s.step == Step::Done);
        let cfg = daemon.lock().unwrap().config.clone();
        eprintln!("done: device={:?} usb_only={} cfg.device_id={} stream_usb={}",
            s.device.as_ref().map(|d| &d.name), s.usb_only, cfg.device_id, cfg.stream_via_usb);
        assert!(!cfg.device_id.is_empty());
        assert!(cfg.stream_via_usb);
        assert_eq!(cfg.accent, "emerald");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    #[ignore]
    fn usb_setup_bluetooth() {
        let home = std::env::temp_dir().join(format!("hawaos-hil-ble-{}", std::process::id()));
        unsafe { std::env::set_var("HOME", &home) };
        let daemon = Arc::new(Mutex::new(SharedDaemonState::new(CompanionConfig::default())));
        let w = Wizard::spawn(Arc::clone(&daemon));

        w.send(Cmd::Search(Method::Usb));
        wait_for(&w, "a USB device", |s| !s.found.is_empty());
        w.send(Cmd::Choose(0));
        wait_for(&w, "wifi list", |s| s.step == Step::Wifi);
        w.send(Cmd::UseBluetooth);
        wait_for(&w, "personalize", |s| s.step == Step::Personalize);
        w.send(Cmd::Personalize { accent: "cyan".into(), theme: "dark".into(), use_24h: false });
        std::thread::sleep(Duration::from_millis(800));
        w.send(Cmd::Finish);
        let s = wait_for(&w, "done", |s| s.step == Step::Done);
        let cfg = daemon.lock().unwrap().config.clone();
        eprintln!("done: use_ble={} usb_only={} device={}", cfg.use_ble, s.usb_only, cfg.device_id);
        assert!(cfg.use_ble && !cfg.stream_via_usb && !cfg.device_token.is_empty());
        let _ = std::fs::remove_dir_all(home);
    }
}
