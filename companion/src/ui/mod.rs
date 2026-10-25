use gpui_kit::component::{Sizable, StyledExt};
pub mod daemon_settings;
pub mod device_settings;
pub mod initial_setup;
pub mod overview;
pub mod sidebar;
pub mod theme;

use std::sync::Arc;
use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::InputState;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;

pub use theme::{apply_hawaos_gpui_theme, HawaPalette};

use crate::config::{save_config, DEFAULT_DEVICE_IP, LOGO_PNG_BYTES};
use crate::models::{CompanionConfig, OutboundAction, SharedState};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NavSection {
    Overview,
    DeviceSettings,
    InitialSetup,
    DaemonSettings,
}

pub struct CompanionWindowView {
    pub(crate) state: SharedState,
    pub(crate) active_section: NavSection,
    pub(crate) logo_image: Arc<Image>,
    pub(crate) ip_input: Entity<InputState>,
    pub(crate) ssid_input: Entity<InputState>,
    pub(crate) pass_input: Entity<InputState>,
    pub(crate) notify_title_input: Entity<InputState>,
    pub(crate) notify_body_input: Entity<InputState>,
    pub(crate) weather_city_input: Entity<InputState>,
    pub(crate) timer_h_input: Entity<InputState>,
    pub(crate) timer_m_input: Entity<InputState>,
    pub(crate) timer_s_input: Entity<InputState>,
    pub(crate) timer_ms_input: Entity<InputState>,
    pub(crate) status_banner: Option<String>,
}

impl CompanionWindowView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, state: SharedState) -> Self {
        let (target_ip, wifi_ssid, weather_city) = if let Ok(g) = state.lock() {
            (
                g.config.target_ip.clone(),
                g.config.wifi_ssid.clone(),
                g.config.weather_city.clone(),
            )
        } else {
            (DEFAULT_DEVICE_IP.to_string(), String::new(), String::new())
        };

        let ip_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("e.g. 192.168.0.98")
                .default_value(target_ip)
        });
        let ssid_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("WiFi Network SSID (2.4 GHz)")
                .default_value(wifi_ssid)
        });
        let pass_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("WiFi Password")
                .masked(true)
        });
        let notify_title_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Notification title")
                .default_value("HawaOS Desktop")
        });
        let notify_body_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Message to display on T-Display-S3...")
                .default_value("Breezy Elegance, Materialised.")
        });
        let weather_city_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("e.g. London, Tokyo, New York")
                .default_value(weather_city)
        });
        let timer_h_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("00")
                .default_value("00")
        });
        let timer_m_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("05")
                .default_value("05")
        });
        let timer_s_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("00")
                .default_value("00")
        });
        let timer_ms_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("000")
                .default_value("000")
        });

        // Pre-resize logo to 96x96 via Lanczos3 for crisp rendering without pixelation/aliasing
        let logo_image = if let Ok(img) =
            image::load_from_memory_with_format(LOGO_PNG_BYTES, image::ImageFormat::Png)
        {
            let resized = img.resize_exact(96, 96, image::imageops::FilterType::Lanczos3);
            let mut buf = Vec::new();
            let _ = resized.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png);
            Arc::new(Image::from_bytes(ImageFormat::Png, buf))
        } else {
            Arc::new(Image::from_bytes(ImageFormat::Png, LOGO_PNG_BYTES.to_vec()))
        };

        Self {
            state,
            active_section: NavSection::Overview,
            logo_image,
            ip_input,
            ssid_input,
            pass_input,
            notify_title_input,
            notify_body_input,
            weather_city_input,
            timer_h_input,
            timer_m_input,
            timer_s_input,
            timer_ms_input,
            status_banner: None,
        }
    }

    pub(crate) fn mutate_config_and_sync(
        &mut self,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut CompanionConfig),
    ) {
        if let Ok(mut g) = self.state.lock() {
            f(&mut g.config);
            g.config.config_version += 1;
            save_config(&g.config);
            if g.config.auto_sync_settings {
                g.outbound_queue.push_back(OutboundAction::PushConfig {
                    wifi_ssid: None,
                    wifi_pass: None,
                    forget_wifi: false,
                    force_usb: false,
                });
            }
        }
        cx.notify();
    }
}

impl Render for CompanionWindowView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark_mode = if let Ok(g) = self.state.lock() {
            g.config.companion_dark_mode
        } else {
            true
        };
        let pal = if dark_mode {
            HawaPalette::dark()
        } else {
            HawaPalette::light()
        };

        let content = match self.active_section {
            NavSection::Overview => self.render_overview(pal, cx).into_any_element(),
            NavSection::DeviceSettings => self.render_device_settings(pal, cx).into_any_element(),
            NavSection::InitialSetup => self.render_initial_setup(pal, cx).into_any_element(),
            NavSection::DaemonSettings => self.render_daemon_settings(pal, cx).into_any_element(),
        };

        let banner = self.status_banner.clone();

        h_flex()
            .size_full()
            .bg(pal.bg)
            .text_color(pal.text)
            .child(self.render_sidebar(pal, cx))
            .child(
                v_flex()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .p_6()
                    .gap_4()
                    .overflow_y_scrollbar()
                    .when_some(banner, |this, msg| {
                        this.child(
                            h_flex()
                                .px_4()
                                .py_2()
                                .rounded(px(10.))
                                .bg(pal.primary.opacity(0.16))
                                .border_1()
                                .border_color(pal.primary.opacity(0.45))
                                .justify_between()
                                .items_center()
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .items_center()
                                        .child(
                                            div()
                                                .text_color(pal.accent)
                                                .child(IconName::CircleCheck),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_semibold()
                                                .text_color(pal.text)
                                                .child(msg),
                                        ),
                                )
                                .child(
                                    Button::new("dismiss-banner")
                                        .small()
                                        .ghost()
                                        .label("Dismiss")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.status_banner = None;
                                            cx.notify();
                                        })),
                                ),
                        )
                    })
                    .child(content),
            )
    }
}

pub fn open_companion_window(cx: &mut App, state: SharedState) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1040.), px(700.)),
            cx,
        ))),
        titlebar: Some(TitlebarOptions {
            title: Some("HawaOS Desktop Companion — Breezy Elegance, Materialised".into()),
            ..Default::default()
        }),
        app_id: Some("hawaos-companion".into()),
        ..Default::default()
    };

    let _ = gpui_kit::open_window(options, cx, move |window, cx| {
        cx.new(|cx| CompanionWindowView::new(window, cx, Arc::clone(&state)))
    });
}
