mod config;
mod media;
mod models;
mod notifications;
mod auth;
mod ble_daemon;
mod ota;
mod protocol;
mod setup;
mod tray;
mod wallpaper;
mod ui;
mod weather;
mod worker;

use gpui_kit::assets::AllAssets;
use gpui_kit::*;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::config::{load_config, save_config};
use crate::media::spawn_art_http_server;
use crate::models::{SharedArtCache, SharedDaemonState, SharedState};
use crate::notifications::spawn_notification_listener;
use crate::tray::spawn_system_tray;
use crate::ui::{apply_hawaos_gpui_theme, open_companion_window};
use crate::worker::spawn_daemon_worker;

fn main() {
    let args: Vec<String> = env::args().collect();
    let start_minimized = args.iter().any(|a| a == "--minimized" || a == "--tray");

    let mut cfg = load_config();
    if let Some(ip_arg) = args.iter().skip(1).find(|a| !a.starts_with('-')) {
        cfg.target_ip = ip_arg.clone();
        save_config(&cfg);
    }

    let state: SharedState = Arc::new(Mutex::new(SharedDaemonState::new(cfg.clone())));
    let art_cache: SharedArtCache = Arc::new(Mutex::new(HashMap::new()));

    spawn_art_http_server(Arc::clone(&art_cache), Arc::clone(&state));
    spawn_daemon_worker(Arc::clone(&state), Arc::clone(&art_cache));
    ble_daemon::spawn(Arc::clone(&state));
    spawn_notification_listener(Arc::clone(&state));
    spawn_system_tray(Arc::clone(&state));

    gpui_kit::application()
        .with_assets(AllAssets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            cx.set_quit_mode(QuitMode::Explicit);
            apply_hawaos_gpui_theme(cfg.companion_dark_mode, &cfg.accent, cx);

            if !start_minimized {
                open_companion_window(cx, Arc::clone(&state));
            }

            let poll_state = Arc::clone(&state);
            cx.spawn(async move |cx: &mut AsyncApp| {
                let mut tick_count: u32 = 0;
                // Accent / mode last seen, to re-theme right away when the tray
                // or a device ACK changes it (not just on the 1 s refresh).
                let mut last_look: Option<(bool, String)> = None;
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(200))
                        .await;
                    tick_count = tick_count.wrapping_add(1);

                    let (want_open, want_quit, look, live) = if let Ok(mut g) = poll_state.lock() {
                        let open = g.open_window_requested;
                        g.open_window_requested = false;
                        let look = (g.config.companion_dark_mode, g.config.accent.clone());
                        (open, g.quit_requested, Some(look), g.wizard_active)
                    } else {
                        (false, false, None, false)
                    };
                    let look_changed = look.is_some() && look != last_look;
                    if look_changed {
                        last_look = look.clone();
                    }

                    let state_for_ui = Arc::clone(&poll_state);
                    let _ = cx.update(|cx: &mut App| {
                        if want_quit {
                            cx.quit();
                            return;
                        }
                        if let (true, Some((dark, accent))) = (look_changed, look.as_ref()) {
                            apply_hawaos_gpui_theme(*dark, accent, cx);
                        }
                        if want_open {
                            if let Some(handle) = cx.windows().first() {
                                let _ = handle.update(cx, |_, window, _| {
                                    window.activate_window();
                                });
                            } else {
                                open_companion_window(cx, state_for_ui);
                            }
                        } else if (live || tick_count.is_multiple_of(5)) && !cx.windows().is_empty() {
                            cx.refresh_windows();
                        }
                    });
                }
            })
            .detach();
        });
}
