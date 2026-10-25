use std::sync::Arc;
use std::time::Duration;
use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex, Sizable, StyledExt};

use crate::config::{save_config, DEFAULT_DEVICE_IP};
use crate::ui::theme::{apply_hawaos_gpui_theme, HawaPalette};
use crate::ui::{CompanionWindowView, NavSection};

const SIDEBAR_W: f32 = 232.;
const SIDEBAR_W_COMPACT: f32 = 64.;

impl CompanionWindowView {
    /// Floating glass sidebar. `compact` shows icons only (narrow windows).
    pub(crate) fn render_sidebar(
        &mut self,
        pal: HawaPalette,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (target_ip, connected, stream_usb, dark_ui) = if let Ok(g) = self.state.lock() {
            let recent = g
                .last_ack_instant
                .map(|t| t.elapsed() < Duration::from_secs(120))
                .unwrap_or(false);
            (
                g.device_reported_ip
                    .clone()
                    .unwrap_or_else(|| g.config.target_ip.clone()),
                recent,
                g.config.stream_via_usb,
                g.config.companion_dark_mode,
            )
        } else {
            (DEFAULT_DEVICE_IP.to_string(), false, false, true)
        };

        let active = self.active_section;
        let nav_item = |id: &'static str,
                        icon: IconName,
                        label: &'static str,
                        section: NavSection,
                        cx: &mut Context<Self>| {
            let is_active = active == section;
            h_flex()
                .id(id)
                .w_full()
                .gap_3()
                .items_center()
                .when(compact, |this| this.justify_center())
                .px_3()
                .py_2()
                .rounded(px(10.))
                .cursor_pointer()
                .when(is_active, |this| {
                    this.bg(pal.primary.opacity(0.16))
                        .text_color(pal.accent)
                })
                .when(!is_active, |this| {
                    this.text_color(pal.text_dim)
                        .hover(|s| s.bg(pal.glass_strong).text_color(pal.text))
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.active_section = section;
                    cx.notify();
                }))
                .child(div().flex_shrink_0().child(icon))
                .when(!compact, |this| {
                    this.child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .font_medium()
                            .text_color(if is_active { pal.text } else { pal.text_dim })
                            .child(label),
                    )
                })
        };

        let status_color = if connected { pal.success } else { pal.warn };
        let theme_btn = Button::new("toggle-companion-theme")
            .small()
            .ghost()
            .icon(if dark_ui { IconName::Moon } else { IconName::Sun })
            .tooltip(if dark_ui { "Switch to light" } else { "Switch to dark" })
            .on_click(cx.listener(|this, _, _, cx| {
                let next_dark = if let Ok(mut g) = this.state.lock() {
                    g.config.companion_dark_mode = !g.config.companion_dark_mode;
                    save_config(&g.config);
                    g.config.companion_dark_mode
                } else {
                    true
                };
                apply_hawaos_gpui_theme(next_dark, cx);
                cx.notify();
            }));

        v_flex()
            .w(px(if compact { SIDEBAR_W_COMPACT } else { SIDEBAR_W }))
            .h_full()
            .flex_shrink_0()
            .p_3()
            .gap_5()
            .rounded(px(20.))
            .bg(pal.glass_strong)
            .border_1()
            .border_color(pal.glass_border)
            .justify_between()
            .child(
                v_flex()
                    .gap_5()
                    .child(
                        // Brand. The logo artwork is already rounded, so no clipping radius here.
                        h_flex()
                            .gap_3()
                            .items_center()
                            .when(compact, |this| this.justify_center())
                            .when(!compact, |this| this.px_1())
                            .child(
                                img(Arc::clone(&self.logo_image))
                                    .flex_shrink_0()
                                    .w(px(36.))
                                    .h(px(36.)),
                            )
                            .when(!compact, |this| {
                                this.child(
                                    v_flex()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_base()
                                                .font_bold()
                                                .text_color(pal.text)
                                                .child("HawaOS"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(pal.text_dim)
                                                .child(concat!("Companion v", env!("CARGO_PKG_VERSION"))),
                                        ),
                                )
                            }),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(nav_item("nav-overview", IconName::LayoutDashboard, "Overview", NavSection::Overview, cx))
                            .child(nav_item("nav-device-settings", IconName::SlidersHorizontal, "Device", NavSection::DeviceSettings, cx))
                            .child(nav_item("nav-initial-setup", IconName::Usb, "Setup & WiFi", NavSection::InitialSetup, cx))
                            .child(nav_item("nav-daemon-settings", IconName::Settings, "Background", NavSection::DaemonSettings, cx)),
                    ),
            )
            .child(if compact {
                v_flex()
                    .gap_3()
                    .items_center()
                    .child(div().size(px(8.)).rounded_full().bg(status_color))
                    .child(theme_btn)
                    .into_any_element()
            } else {
                v_flex()
                    .gap_2()
                    .p_3()
                    .rounded(px(14.))
                    .bg(pal.glass)
                    .border_1()
                    .border_color(pal.glass_border)
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .min_w_0()
                                    .items_center()
                                    .child(div().flex_shrink_0().size(px(8.)).rounded_full().bg(status_color))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .truncate()
                                            .text_sm()
                                            .font_medium()
                                            .text_color(pal.text)
                                            .child(if connected { "Connected" } else { "Waiting for device" }),
                                    ),
                            )
                            .child(theme_btn),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_xs()
                            .text_color(pal.text_dim)
                            .child(format!(
                                "{} · {}",
                                target_ip,
                                if stream_usb { "WiFi + USB" } else { "WiFi" }
                            )),
                    )
                    .into_any_element()
            })
    }
}
