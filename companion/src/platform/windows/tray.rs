//! The notification-area icon and its menu (the same menu as Linux:
//! tray_actions.rs). Windows delivers tray clicks as window messages, so the
//! icon lives on its own thread, which pumps messages and refreshes the
//! labels when what they show changes.

use std::thread;
use std::time::Duration;

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE};

use crate::config::LOGO_PNG_BYTES;
use crate::media::handle_device_command;
use crate::models::SharedState;
use crate::tray_actions::{self, TrayView};
use crate::ui::accents::ACCENTS;

fn icon() -> Option<Icon> {
    let img = image::load_from_memory_with_format(LOGO_PNG_BYTES, image::ImageFormat::Png).ok()?;
    let small = img.resize_exact(32, 32, image::imageops::FilterType::Lanczos3).into_rgba8();
    Icon::from_rgba(small.into_raw(), 32, 32).ok()
}

/// Windows limits tooltips to 127 characters.
fn tooltip(view: &TrayView) -> String {
    view.tooltip.chars().take(127).collect()
}

pub fn spawn_system_tray(state: SharedState) {
    thread::spawn(move || {
        let Some(view) = tray_actions::view(&state) else { return };

        let open = MenuItem::new("Open HawaOS Companion", true, None);
        let add = MenuItem::new("Add Device...", true, None);
        let track = MenuItem::new(&view.track, false, None);
        let play = MenuItem::new("Play / Pause Media", true, None);
        let next = MenuItem::new("Next Track", true, None);
        let theme = MenuItem::new(tray_actions::theme_label(&view), true, None);
        let accents: Vec<CheckMenuItem> =
            ACCENTS.iter().enumerate().map(|(i, a)| CheckMenuItem::new(a.label, true, i == view.accent, None)).collect();
        let accent_menu = Submenu::new("Accent Color", true);
        for item in &accents {
            let _ = accent_menu.append(item);
        }
        let autostart = CheckMenuItem::new("Start Companion with System", true, view.autostart, None);
        let quit = MenuItem::new("Quit HawaOS Daemon", true, None);
        // At the top, while a newer release is out (tray_actions::update_to_show).
        let update = MenuItem::new("", true, None);
        let update_sep = PredefinedMenuItem::separator();

        let menu = Menu::new();
        let sep = PredefinedMenuItem::separator;
        for item in [
            &open as &dyn tray_icon::menu::IsMenuItem,
            &add,
            &sep(),
            &track,
            &play,
            &next,
            &sep(),
            &theme,
            &accent_menu,
            &autostart,
            &sep(),
            &quit,
        ] {
            let _ = menu.append(item);
        }

        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(false)
            .with_tooltip(tooltip(&view));
        if let Some(icon) = icon() {
            builder = builder.with_icon(icon);
        }
        let Ok(tray) = builder.build() else { return };

        let show_update = |menu: &Menu, version: Option<&String>| match version {
            Some(v) => {
                update.set_text(tray_actions::update_label(v));
                let _ = menu.prepend_items(&[&update, &update_sep]);
            }
            None => {
                let _ = menu.remove(&update);
                let _ = menu.remove(&update_sep);
            }
        };
        show_update(&menu, view.update.as_ref());
        let mut shown = view;
        let mut since_refresh = Duration::ZERO;
        let tick = Duration::from_millis(50);
        loop {
            unsafe {
                let mut msg = MSG::default();
                while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            // A left click opens the window; the right click shows the menu.
            while let Ok(event) = TrayIconEvent::receiver().try_recv() {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } =
                    event
                {
                    tray_actions::open_window(&state);
                }
            }
            while let Ok(event) = MenuEvent::receiver().try_recv() {
                let id = &event.id;
                if id == open.id() {
                    tray_actions::open_window(&state);
                } else if id == add.id() {
                    tray_actions::add_device(&state);
                } else if id == play.id() {
                    handle_device_command("play_pause", shown.active_player.as_deref());
                } else if id == next.id() {
                    handle_device_command("next", shown.active_player.as_deref());
                } else if id == theme.id() {
                    tray_actions::toggle_device_theme(&state);
                } else if id == autostart.id() {
                    tray_actions::toggle_autostart(&state);
                } else if id == update.id() {
                    tray_actions::open_release_page(&state);
                } else if id == quit.id() {
                    tray_actions::quit(&state);
                } else if let Some(i) = accents.iter().position(|a| a.id() == id) {
                    tray_actions::set_accent(&state, i);
                }
                since_refresh = Duration::from_secs(1); // show the result now
            }

            // Settings also change from the window and the device.
            since_refresh += tick;
            if since_refresh >= Duration::from_millis(500) {
                since_refresh = Duration::ZERO;
                if let Some(now) = tray_actions::view(&state) {
                    if now != shown {
                        track.set_text(&now.track);
                        theme.set_text(tray_actions::theme_label(&now));
                        for (i, item) in accents.iter().enumerate() {
                            item.set_checked(i == now.accent);
                        }
                        autostart.set_checked(now.autostart);
                        if now.update != shown.update {
                            show_update(&menu, None);
                            show_update(&menu, now.update.as_ref());
                        }
                        let _ = tray.set_tooltip(Some(tooltip(&now)));
                        shown = now;
                    }
                }
            }
            thread::sleep(tick);
        }
    });
}
