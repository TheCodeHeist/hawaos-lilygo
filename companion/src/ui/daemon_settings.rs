use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::assets::IconName;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{h_flex, v_flex, Sizable, StyledExt};
use gpui_kit::prelude::FluentBuilder as _;

use crate::config::{autostart_desktop_path, config_file_path, save_config, set_autostart_installed, CompanionConfig};
use crate::ui::theme::HawaPalette;
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

        v_flex()
            .gap_4()
            .w_full()
            .min_w_0()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_xl()
                            .font_bold()
                            .text_color(pal.text)
                            .child("System Tray Daemon & Startup Settings"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(pal.text_dim)
                            .child("Configure background tray behavior, XDG autostart on login, and inspect live daemon events"),
                    ),
            )
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_4()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Start HawaOS Companion Daemon with System"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child(format!(
                                                "Installs XDG autostart entry ({}) with --minimized tray mode",
                                                autostart_desktop_path().display()
                                            )),
                                    ),
                            )
                            .child(
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
                                                    "Autostart enabled (~/.config/autostart/hawaos-companion.desktop)."
                                                        .into()
                                                } else {
                                                    "Autostart entry removed.".into()
                                                });
                                            }
                                            Err(e) => {
                                                this.status_banner = Some(format!(
                                                    "Failed to update autostart: {}",
                                                    e
                                                ));
                                            }
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Keep Daemon Running in System Tray When Closed"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Closing this window leaves the tray icon and 1 Hz telemetry stream active in the background"),
                                    ),
                            )
                            .child(
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
                            ),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .pt_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_muted)
                                    .child(format!(
                                        "Config file: {}",
                                        config_file_path().display()
                                    )),
                            )
                            .child(
                                Button::new("quit-daemon-btn")
                                    .danger()
                                    .small()
                                    .icon(IconName::Power)
                                    .label("Quit Companion Daemon")
                                    .on_click(|_, _, cx| {
                                        cx.quit();
                                    }),
                            ),
                    ),
            )
            // Live Daemon Activity Log
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .text_color(pal.text_dim)
                            .child("RECENT DAEMON & DEVICE LOGS"),
                    )
                    .child(
                        v_flex()
                            .p_3()
                            .rounded(px(10.))
                            .bg(pal.bg)
                            .border_1()
                            .border_color(pal.border)
                            .gap_1()
                            .children(logs.into_iter().rev().take(10).map(|line| {
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child(format!("• {}", line))
                            })),
                    ),
            )
    }
}
