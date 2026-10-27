use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::assets::IconName;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::theme::ActiveTheme;
use gpui_kit::component::{h_flex, v_flex, Sizable};

use crate::config::{save_config, CompanionConfig};
use crate::platform::{config_file_path, set_autostart_installed};
use crate::ui::components::{
    button_row, card_header, choice, divider, glass_card, page_header, setting_row,
};
use crate::ui::theme::{use_translucent_window, window_background, HawaPalette};
use crate::ui::CompanionWindowView;

impl CompanionWindowView {
    pub(crate) fn render_daemon_settings(
        &mut self,
        pal: HawaPalette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (cfg, logs) = if let Ok(g) = self.state.lock() {
            (
                g.config.clone(),
                g.logs.iter().cloned().collect::<Vec<String>>(),
            )
        } else {
            (CompanionConfig::default(), Vec::new())
        };
        let mono = cx.theme().mono_font_family.clone();

        v_flex()
            .w_full()
            .gap_4()
            .child(page_header(
                pal,
                "Background",
                "Tray, startup and window behavior",
            ))
            .child(
                glass_card(pal)
                    .child(setting_row(
                        pal,
                        "Start on login",
                        Some("Launches minimized to the system tray."),
                        Switch::new("autostart-switch")
                            .checked(cfg.autostart_enabled)
                            .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                let enable = *checked;
                                match set_autostart_installed(enable) {
                                    Ok(()) => {
                                        if let Ok(mut g) = this.state.lock() {
                                            g.config.autostart_enabled = enable;
                                            save_config(&g.config);
                                            g.push_log(if enable {
                                                "Enabled system autostart (--minimized)"
                                            } else {
                                                "Disabled system autostart"
                                            });
                                        }
                                        this.status_banner = Some(if enable {
                                            "Will start on login.".into()
                                        } else {
                                            "Won't start on login.".into()
                                        });
                                    }
                                    Err(e) => {
                                        this.status_banner =
                                            Some(format!("Couldn't update autostart: {}", e));
                                    }
                                }
                                cx.notify();
                            })),
                    ))
                    .child(divider(pal))
                    .child(setting_row(
                        pal,
                        "Keep running when closed",
                        Some("Closing the window leaves the tray icon and the device feed running."),
                        Switch::new("close-to-tray-switch")
                            .checked(cfg.close_to_tray)
                            .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                let val = *checked;
                                if let Ok(mut g) = this.state.lock() {
                                    g.config.close_to_tray = val;
                                    save_config(&g.config);
                                }
                                cx.notify();
                            })),
                    ))
                    .child(divider(pal))
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .gap_3()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .truncate()
                                    .text_xs()
                                    .text_color(pal.text_muted)
                                    .child(config_file_path().display().to_string()),
                            )
                            .child(
                                Button::new("quit-daemon-btn")
                                    .danger()
                                    .small()
                                    .icon(IconName::Power)
                                    .label("Quit companion")
                                    .on_click(|_, _, cx| {
                                        cx.quit();
                                    }),
                            ),
                    ),
            )
            .child(
                glass_card(pal)
                    .child(setting_row(
                        pal,
                        "Window transparency",
                        Some("Auto blurs the desktop behind the window where supported (KDE Plasma, macOS, Windows) and stays solid elsewhere."),
                        button_row().children([("auto", "Auto"), ("on", "On"), ("off", "Off")].map(
                            |(value, label)| {
                                Button::new(SharedString::from(format!("transparency-{value}")))
                                    .small()
                                    .with_variant(choice(pal, cfg.window_transparency == value, cx))
                                    .label(label)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            g.config.window_transparency = value.to_string();
                                            save_config(&g.config);
                                        }
                                        window.set_background_appearance(window_background(
                                            use_translucent_window(value),
                                        ));
                                        cx.notify();
                                    }))
                            },
                        )),
                    )),
            )
            .child(
                glass_card(pal)
                    .child(card_header(pal, IconName::ScrollText, "Activity"))
                    .child(
                        v_flex()
                            .p_3()
                            .gap_1()
                            .rounded(px(12.))
                            .bg(pal.well)
                            .font_family(mono)
                            .when(logs.is_empty(), |this| {
                                this.child(div().text_xs().text_color(pal.text_muted).child("No activity yet."))
                            })
                            .children(logs.into_iter().rev().take(12).map(|line| {
                                div().text_xs().text_color(pal.text_dim).child(line)
                            })),
                    ),
            )
    }
}
