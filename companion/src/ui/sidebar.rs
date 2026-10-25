use std::sync::Arc;
use gpui_kit::component::{Sizable, StyledExt};
use std::time::Duration;
use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};

use crate::config::{save_config, ART_HTTP_PORT, DEFAULT_DEVICE_IP, UDP_PORT};
use crate::ui::theme::{apply_hawaos_gpui_theme, HawaPalette};
use crate::ui::{CompanionWindowView, NavSection};

impl CompanionWindowView {
    pub(crate) fn render_sidebar(&mut self, pal: HawaPalette, cx: &mut Context<Self>) -> impl IntoElement {
        let (target_ip, ack_ver, ack_recent, stream_usb, usb_port, dark_ui) =
            if let Ok(g) = self.state.lock() {
                let recent = g
                    .last_ack_instant
                    .map(|t| t.elapsed() < Duration::from_secs(120))
                    .unwrap_or(false);
                (
                    g.config.target_ip.clone(),
                    g.last_ack_version,
                    recent,
                    g.config.stream_via_usb,
                    g.config.usb_port.clone(),
                    g.config.companion_dark_mode,
                )
            } else {
                (
                    DEFAULT_DEVICE_IP.to_string(),
                    None,
                    false,
                    false,
                    "/dev/ttyACM0".to_string(),
                    true,
                )
            };

        let nav_item = |id: &'static str,
                        icon: IconName,
                        label: &'static str,
                        section: NavSection,
                        active: NavSection,
                        cx: &mut Context<Self>| {
            let is_active = active == section;
            let bg_color = if is_active {
                pal.primary.opacity(0.16)
            } else {
                pal.primary.opacity(0.0)
            };
            let fg_color = if is_active { pal.accent } else { pal.text_dim };
            let border_color = if is_active {
                pal.primary.opacity(0.45)
            } else {
                pal.primary.opacity(0.0)
            };

            div()
                .id(id)
                .cursor_pointer()
                .w_full()
                .px_3()
                .py_2p5()
                .rounded(px(10.))
                .bg(bg_color)
                .border_1()
                .border_color(border_color)
                .hover(|s| s.bg(pal.surface2))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active_section = section;
                    cx.notify();
                }))
                .child(
                    h_flex()
                        .gap_3()
                        .items_center()
                        .child(div().text_color(fg_color).child(icon))
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(if is_active { pal.text } else { pal.text_dim })
                                .child(label),
                        ),
                )
        };

        let active = self.active_section;

        v_flex()
            .w(px(252.))
            .h_full()
            .flex_shrink_0()
            .bg(pal.surface)
            .border_r_1()
            .border_color(pal.border)
            .p_4()
            .justify_between()
            .child(
                v_flex()
                    .gap_5()
                    .child(
                        // Brand Header
                        h_flex()
                            .gap_3()
                            .items_center()
                            .p_2()
                            .rounded(px(12.))
                            .bg(pal.surface2.opacity(0.6))
                            .border_1()
                            .border_color(pal.border)
                            .child(
                                img(Arc::clone(&self.logo_image))
                                    .w(px(40.))
                                    .h(px(40.))
                                    .rounded(px(8.)),
                            )
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_base()
                                                    .font_bold()
                                                    .text_color(pal.text)
                                                    .child("HawaOS"),
                                            )
                                            .child(
                                                div()
                                                    .px_1p5()
                                                    .py_0p5()
                                                    .rounded(px(6.))
                                                    .bg(pal.primary.opacity(0.2))
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.accent)
                                                    .child("v0.4"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Breezy Elegance"),
                                    ),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_1p5()
                            .child(
                                div()
                                    .px_2()
                                    .pb_1()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(pal.text_muted)
                                    .child("COMPANION MENU"),
                            )
                            .child(nav_item(
                                "nav-overview",
                                IconName::Activity,
                                "Overview & Live",
                                NavSection::Overview,
                                active,
                                cx,
                            ))
                            .child(nav_item(
                                "nav-device-settings",
                                IconName::SlidersHorizontal,
                                "Device Settings",
                                NavSection::DeviceSettings,
                                active,
                                cx,
                            ))
                            .child(nav_item(
                                "nav-initial-setup",
                                IconName::Usb,
                                "Initial Setup & WiFi",
                                NavSection::InitialSetup,
                                active,
                                cx,
                            ))
                            .child(nav_item(
                                "nav-daemon-settings",
                                IconName::Settings,
                                "System Tray & Startup",
                                NavSection::DaemonSettings,
                                active,
                                cx,
                            )),
                    ),
            )
            .child(
                v_flex()
                    .gap_3()
                    .child(
                        // Device Connection Status Card
                        v_flex()
                            .gap_2()
                            .p_3()
                            .rounded(px(12.))
                            .bg(pal.bg)
                            .border_1()
                            .border_color(pal.border)
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(
                                                if ack_recent { pal.success } else { pal.primary },
                                            ))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.text)
                                                    .child("T-Display-S3 Link"),
                                            ),
                                    )
                                    .child(div().text_xs().text_color(pal.accent).child(
                                        match ack_ver {
                                            Some(v) => format!("ACK v{}", v),
                                            None => "UDP Active".to_string(),
                                        },
                                    )),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child(format!("WiFi UDP: {}:{}", target_ip, UDP_PORT)),
                            )
                            .child(div().text_xs().text_color(pal.text_muted).child(
                                if stream_usb {
                                    format!("USB Serial: {} (On)", usb_port)
                                } else {
                                    format!("Art Server: :{}", ART_HTTP_PORT)
                                },
                            )),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .px_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child("App Appearance"),
                            )
                            .child(
                                Button::new("toggle-companion-theme")
                                    .small()
                                    .secondary()
                                    .icon(if dark_ui {
                                        IconName::Moon
                                    } else {
                                        IconName::Sun
                                    })
                                    .label(if dark_ui { "Dark" } else { "Light" })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let next_dark = if let Ok(mut g) = this.state.lock() {
                                            g.config.companion_dark_mode =
                                                !g.config.companion_dark_mode;
                                            save_config(&g.config);
                                            g.config.companion_dark_mode
                                        } else {
                                            true
                                        };
                                        apply_hawaos_gpui_theme(next_dark, cx);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }

}
