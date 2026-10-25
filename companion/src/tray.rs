use crate::media::handle_device_command;
use ksni::blocking::TrayMethods;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::config::{save_config, set_autostart_installed, DEFAULT_DEVICE_IP, LOGO_PNG_BYTES};
use crate::models::{OutboundAction, SharedState};
use crate::ui::accents::ACCENTS;
use crate::ui::theme::accent_by_id;

struct HawaTray {
    state: SharedState,
    icon_pixmap: Vec<ksni::Icon>,
}

fn build_tray_icons() -> Vec<ksni::Icon> {
    let Ok(img) = image::load_from_memory_with_format(LOGO_PNG_BYTES, image::ImageFormat::Png)
    else {
        return Vec::new();
    };
    let mut icons = Vec::new();
    for dim in [32u32, 64u32] {
        let resized = img.resize_exact(dim, dim, image::imageops::FilterType::Lanczos3);
        let mut data = resized.into_rgba8().into_vec();
        for px in data.chunks_exact_mut(4) {
            px.rotate_right(1); // RGBA -> ARGB
        }
        icons.push(ksni::Icon {
            width: dim as i32,
            height: dim as i32,
            data,
        });
    }
    icons
}

impl ksni::Tray for HawaTray {
    fn id(&self) -> String {
        "hawaos-companion".into()
    }

    fn title(&self) -> String {
        "HawaOS Companion".into()
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::Hardware
    }

    fn status(&self) -> ksni::Status {
        ksni::Status::Active
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icon_pixmap.clone()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let (ip, cpu, ram, track) = if let Ok(g) = self.state.lock() {
            let t = if g.media.title.is_empty() {
                "Idle".to_string()
            } else {
                format!("{} — {}", g.media.title, g.media.artist)
            };
            (g.config.target_ip.clone(), g.cpu_pct, g.ram_pct, t)
        } else {
            (DEFAULT_DEVICE_IP.to_string(), 0, 0, "Idle".to_string())
        };

        ksni::ToolTip {
            icon_name: String::new(),
            icon_pixmap: self.icon_pixmap.clone(),
            title: format!("HawaOS Companion ({})", ip),
            description: format!("CPU {}% · RAM {}% · {}", cpu, ram, track),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        if let Ok(mut g) = self.state.lock() {
            g.open_window_requested = true;
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let (autostart, device_theme, accent, track_summary, active_player) =
            if let Ok(g) = self.state.lock() {
                let summary = if g.media.title.is_empty() {
                    "Media: No active track".to_string()
                } else {
                    format!("♪ {} — {}", g.media.title, g.media.artist)
                };
                (
                    g.config.autostart_enabled,
                    g.config.device_theme.clone(),
                    g.config.accent.clone(),
                    summary,
                    g.active_player.clone(),
                )
            } else {
                (false, "dark".to_string(), String::new(), "Media: Idle".to_string(), None)
            };

        let play_target = active_player.clone();
        let next_target = active_player.clone();

        vec![
            StandardItem {
                label: "Open HawaOS Companion".into(),
                activate: Box::new(|this: &mut Self| {
                    if let Ok(mut g) = this.state.lock() {
                        g.open_window_requested = true;
                    }
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: track_summary,
                enabled: false,
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Play / Pause Media".into(),
                activate: Box::new(move |_| {
                    handle_device_command("play_pause", play_target.as_deref())
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next Track".into(),
                activate: Box::new(move |_| {
                    handle_device_command("next", next_target.as_deref())
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: format!(
                    "Switch Device Theme (Current: {})",
                    device_theme.to_uppercase()
                ),
                activate: Box::new(|this: &mut Self| {
                    if let Ok(mut g) = this.state.lock() {
                        g.config.device_theme = if g.config.device_theme == "dark" {
                            "light".to_string()
                        } else {
                            "dark".to_string()
                        };
                        g.config.config_version += 1;
                        save_config(&g.config);
                        g.outbound_queue.push_back(OutboundAction::PushConfig {
                            wifi_ssid: None,
                            wifi_pass: None,
                            forget_wifi: false,
                            force_usb: false,
                        });
                    }
                }),
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: "Accent Color".into(),
                submenu: vec![RadioGroup {
                    selected: ACCENTS.iter().position(|a| a.id == accent_by_id(&accent).id).unwrap_or(0),
                    select: Box::new(|this: &mut Self, idx| {
                        let Some(picked) = ACCENTS.get(idx) else { return };
                        if let Ok(mut g) = this.state.lock() {
                            g.config.accent = picked.id.to_string();
                            g.config.config_version += 1;
                            save_config(&g.config);
                            g.outbound_queue.push_back(OutboundAction::PushConfig {
                                wifi_ssid: None,
                                wifi_pass: None,
                                forget_wifi: false,
                                force_usb: false,
                            });
                        }
                    }),
                    options: ACCENTS
                        .iter()
                        .map(|a| RadioItem {
                            label: a.label.into(),
                            ..Default::default()
                        })
                        .collect(),
                }
                .into()],
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Start Companion with System".into(),
                checked: autostart,
                activate: Box::new(|this: &mut Self| {
                    if let Ok(mut g) = this.state.lock() {
                        let next = !g.config.autostart_enabled;
                        if set_autostart_installed(next).is_ok() {
                            g.config.autostart_enabled = next;
                            save_config(&g.config);
                        }
                    }
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit HawaOS Daemon".into(),
                activate: Box::new(|this: &mut Self| {
                    if let Ok(mut g) = this.state.lock() {
                        g.quit_requested = true;
                    }
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn spawn_system_tray(state: SharedState) {
    let tray = HawaTray {
        state: Arc::clone(&state),
        icon_pixmap: build_tray_icons(),
    };
    let Ok(handle) = tray.assume_sni_available(true).spawn() else {
        return;
    };
    // ksni only rebuilds the menu on `update()`. Settings also change from the
    // window and from device ACKs, so re-publish whenever what the menu shows
    // (theme, accent, track) differs from the last published menu.
    thread::spawn(move || {
        let mut shown: Option<(String, String, String)> = None;
        loop {
            thread::sleep(Duration::from_millis(500));
            let now = match state.lock() {
                Ok(g) => (
                    g.config.device_theme.clone(),
                    g.config.accent.clone(),
                    g.media.title.clone(),
                ),
                Err(_) => continue,
            };
            if shown.as_ref() != Some(&now) {
                shown = Some(now);
                if handle.update(|_| {}).is_none() {
                    break; // tray service shut down
                }
            }
        }
    });
}

