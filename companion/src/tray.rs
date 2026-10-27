use crate::media::handle_device_command;
use ksni::blocking::TrayMethods;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::config::LOGO_PNG_BYTES;
use crate::models::SharedState;
use crate::tray_actions;
use crate::ui::accents::ACCENTS;

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
        let tip = tray_actions::view(&self.state).map(|v| v.tooltip).unwrap_or_default();
        let (title, description) = tip.split_once('\n').unwrap_or((tip.as_str(), ""));
        ksni::ToolTip {
            icon_name: String::new(),
            icon_pixmap: self.icon_pixmap.clone(),
            title: title.to_string(),
            description: description.to_string(),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        tray_actions::open_window(&self.state);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let Some(view) = tray_actions::view(&self.state) else {
            return Vec::new();
        };
        let play_target = view.active_player.clone();
        let next_target = view.active_player.clone();

        let mut items: Vec<ksni::MenuItem<Self>> = Vec::new();
        if let Some(version) = &view.update {
            items.push(
                StandardItem {
                    label: tray_actions::update_label(version),
                    activate: Box::new(|this: &mut Self| tray_actions::open_release_page(&this.state)),
                    ..Default::default()
                }
                .into(),
            );
            items.push(MenuItem::Separator);
        }
        items.extend([
            StandardItem {
                label: "Open HawaOS Companion".into(),
                activate: Box::new(|this: &mut Self| tray_actions::open_window(&this.state)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Add Device...".into(),
                activate: Box::new(|this: &mut Self| tray_actions::add_device(&this.state)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem { label: view.track.clone(), enabled: false, ..Default::default() }.into(),
            StandardItem {
                label: "Play / Pause Media".into(),
                activate: Box::new(move |_| handle_device_command("play_pause", play_target.as_deref())),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next Track".into(),
                activate: Box::new(move |_| handle_device_command("next", next_target.as_deref())),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: tray_actions::theme_label(&view),
                activate: Box::new(|this: &mut Self| tray_actions::toggle_device_theme(&this.state)),
                ..Default::default()
            }
            .into(),
            SubMenu {
                label: "Accent Color".into(),
                submenu: vec![RadioGroup {
                    selected: view.accent,
                    select: Box::new(|this: &mut Self, idx| tray_actions::set_accent(&this.state, idx)),
                    options: ACCENTS.iter().map(|a| RadioItem { label: a.label.into(), ..Default::default() }).collect(),
                }
                .into()],
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: "Start Companion with System".into(),
                checked: view.autostart,
                activate: Box::new(|this: &mut Self| tray_actions::toggle_autostart(&this.state)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit HawaOS Daemon".into(),
                activate: Box::new(|this: &mut Self| tray_actions::quit(&this.state)),
                ..Default::default()
            }
            .into(),
        ]);
        items
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
        let mut shown: Option<(String, String, String, Option<String>)> = None;
        loop {
            thread::sleep(Duration::from_millis(500));
            let Some(v) = tray_actions::view(&state) else { continue };
            let now = (v.theme, v.accent.to_string(), v.track, v.update);
            if shown.as_ref() != Some(&now) {
                shown = Some(now);
                if handle.update(|_| {}).is_none() {
                    break; // tray service shut down
                }
            }
        }
    });
}

