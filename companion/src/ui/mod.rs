pub mod accents;
pub mod components;
pub mod daemon_settings;
pub mod device_settings;
pub mod connection;
pub mod firmware;
pub mod initial_setup;
pub mod overview;
pub mod setup_wizard;
pub mod sidebar;
pub mod theme;

use std::sync::Arc;
use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::InputState;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::{h_flex, v_flex, Sizable, StyledExt};

/// Below this window width the sidebar collapses to icons only.
const COMPACT_SIDEBAR_BELOW: f32 = 920.;
/// Content column stops growing past this width so lines stay readable.
const CONTENT_MAX_W: f32 = 1240.;

pub use theme::{apply_hawaos_gpui_theme, use_translucent_window, window_background, HawaPalette};

use crate::config::{save_config, DEFAULT_DEVICE_IP, LOGO_PNG_BYTES};
use crate::models::{CompanionConfig, OutboundAction, SharedState};
use crate::setup::{self, Wizard};

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
    pub(crate) wizard: Wizard,
    pub(crate) code_input: Entity<InputState>,
    /// Network picked in the wizard's list (None with "Other network").
    pub(crate) wizard_ssid: Option<String>,
    pub(crate) wizard_manual_ssid: bool,
    pub(crate) show_manual_setup: bool,
}

impl CompanionWindowView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, state: SharedState) -> Self {
        let (target_ip, wifi_ssid, weather_city, has_device) = if let Ok(g) = state.lock() {
            (
                g.config.target_ip.clone(),
                g.config.wifi_ssid.clone(),
                g.config.weather_city.clone(),
                !g.config.device_id.is_empty() || !g.config.wifi_ssid.is_empty(),
            )
        } else {
            (DEFAULT_DEVICE_IP.to_string(), String::new(), String::new(), true)
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
        // Setting min/max/step makes NumberInput apply -/+ itself (clamped);
        // without them it only emits Step events and the value never changes.
        let timer_h_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("0")
                .default_value("0")
                .min(0.)
                .max(59.)
                .step(1.)
        });
        let timer_m_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("0")
                .default_value("5")
                .min(0.)
                .max(99.)
                .step(1.)
        });
        let timer_s_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("0")
                .default_value("0")
                .min(0.)
                .max(99.)
                .step(1.)
        });
        let timer_ms_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("0")
                .default_value("0")
                .min(0.)
                .max(999.)
                .step(100.)
        });

        let code_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("000000")
        });

        Self {
            wizard: Wizard::spawn(Arc::clone(&state)),
            code_input,
            wizard_ssid: None,
            wizard_manual_ssid: false,
            show_manual_setup: false,
            // First run (no device yet): open straight into the wizard.
            active_section: dev_start_section()
                .unwrap_or(if has_device { NavSection::Overview } else { NavSection::InitialSetup }),
            state,
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
        sync_config(&self.state, f);
        cx.notify();
    }
}

/// Changes the config, saves it, and sends it to the device if auto-sync is
/// on. For callers without the view, such as a file dialog's callback.
/// The logo mark in `color` (the theme's accent). The artwork is one colour,
/// so only its shape is kept and it takes the accent, like on the device.
/// 96 px (Lanczos, for crisp edges at the sizes shown); one image per colour.
pub(crate) fn logo(color: Rgba) -> Arc<Image> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static SHAPE: OnceLock<Option<image::GrayImage>> = OnceLock::new();
    static TINTED: OnceLock<Mutex<HashMap<u32, Arc<Image>>>> = OnceLock::new();
    let to8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    let (r, g, b) = (to8(color.r), to8(color.g), to8(color.b));
    let key = u32::from_be_bytes([0, r, g, b]);
    let tinted = TINTED.get_or_init(Default::default);
    if let Some(hit) = tinted.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return hit;
    }
    let shape = SHAPE.get_or_init(|| {
        let img = image::load_from_memory_with_format(LOGO_PNG_BYTES, image::ImageFormat::Png).ok()?;
        let small = img.resize_exact(96, 96, image::imageops::FilterType::Lanczos3).into_rgba8();
        Some(image::GrayImage::from_fn(96, 96, |x, y| image::Luma([small.get_pixel(x, y)[3]])))
    });
    let mut buf = Vec::new();
    if let Some(shape) = shape {
        let out = image::RgbaImage::from_fn(96, 96, |x, y| image::Rgba([r, g, b, shape.get_pixel(x, y)[0]]));
        let _ = out.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png);
    }
    let logo = Arc::new(Image::from_bytes(ImageFormat::Png, buf));
    if let Ok(mut m) = tinted.lock() {
        m.insert(key, Arc::clone(&logo));
    }
    logo
}

pub(crate) fn sync_config(state: &SharedState, f: impl FnOnce(&mut CompanionConfig)) {
    if let Ok(mut g) = state.lock() {
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
}

impl Render for CompanionWindowView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (dark_mode, translucent, accent, needs_setup, update) = if let Ok(mut g) = self.state.lock() {
            if std::mem::take(&mut g.open_setup_requested) {
                self.active_section = NavSection::InitialSetup;
            }
            if let Some(notice) = g.notice.take() {
                self.status_banner = Some(notice);
            }
            (
                g.config.companion_dark_mode,
                use_translucent_window(&g.config.window_transparency),
                g.config.accent.clone(),
                g.device_needs_setup,
                crate::tray_actions::update_to_show(&g),
            )
        } else {
            (true, false, accents::DEFAULT_ACCENT.to_string(), false, None)
        };
        let setup_prompt = needs_setup && self.active_section != NavSection::InitialSetup;
        // Leaving the page mid-wizard cancels it, so it can't keep the USB
        // port (and the daemon's USB streaming) tied up in the background.
        if self.active_section != NavSection::InitialSetup
            && !matches!(self.wizard.snapshot().step, setup::Step::Welcome | setup::Step::Done)
        {
            self.wizard.send(setup::Cmd::Cancel);
        }
        // Same frame as the palette below, so gpui-kit widgets and our own
        // elements always switch accent together (no-op when unchanged).
        apply_hawaos_gpui_theme(dark_mode, &accent, cx);
        let pal = HawaPalette::new(dark_mode, translucent, &accent);
        let compact = window.viewport_size().width < px(COMPACT_SIDEBAR_BELOW);

        let content = match self.active_section {
            NavSection::Overview => self.render_overview(pal, cx).into_any_element(),
            NavSection::DeviceSettings => self.render_device_settings(pal, cx).into_any_element(),
            NavSection::InitialSetup => self.render_setup_wizard(pal, cx).into_any_element(),
            NavSection::DaemonSettings => self.render_daemon_settings(pal, cx).into_any_element(),
        };

        let banner = self.status_banner.clone();

        // Translucent tinted gradient over the compositor-blurred window = the glass backdrop.
        h_flex()
            .size_full()
            .bg(linear_gradient(
                160.,
                linear_color_stop(pal.bg_top, 0.),
                linear_color_stop(pal.bg_bottom, 1.),
            ))
            .text_color(pal.text)
            .p_3()
            .gap_3()
            .child(self.render_sidebar(pal, compact, cx))
            .child(
                v_flex()
                    .id("content-scroll")
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .overflow_y_scrollbar()
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(CONTENT_MAX_W))
                            .mx_auto()
                            .px_3()
                            .py_3()
                            .gap_4()
                            .when_some(banner, |this, msg| {
                                this.child(
                                    h_flex()
                                        .w_full()
                                        .gap_3()
                                        .pl_4()
                                        .pr_2()
                                        .py_1p5()
                                        .rounded_full()
                                        .bg(pal.primary.opacity(0.14))
                                        .border_1()
                                        .border_color(pal.primary.opacity(0.35))
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .min_w_0()
                                                .items_center()
                                                .child(
                                                    div()
                                                        .flex_shrink_0()
                                                        .text_color(pal.accent)
                                                        .child(IconName::CircleCheck),
                                                )
                                                .child(
                                                    div()
                                                        .min_w_0()
                                                        .truncate()
                                                        .text_sm()
                                                        .text_color(pal.text)
                                                        .child(msg),
                                                ),
                                        )
                                        .child(
                                            Button::new("dismiss-banner")
                                                .small()
                                                .ghost()
                                                .icon(IconName::Close)
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.status_banner = None;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                            })
                            .when_some(update, |this, version| {
                                this.child(
                                    h_flex()
                                        .w_full()
                                        .gap_3()
                                        .pl_4()
                                        .pr_2()
                                        .py_2()
                                        .rounded(px(components::CARD_RADIUS))
                                        .bg(pal.primary.opacity(0.14))
                                        .border_1()
                                        .border_color(pal.primary.opacity(0.35))
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            // flex_1: the text takes what the buttons leave and
                                            // wraps there (without it, it shrinks to nothing).
                                            h_flex()
                                                .flex_1()
                                                .gap_3()
                                                .min_w_0()
                                                .items_center()
                                                .child(div().flex_shrink_0().text_color(pal.accent).child(IconName::ArrowUp))
                                                .child(
                                                    v_flex()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_semibold()
                                                                .text_color(pal.text)
                                                                .child(format!("HawaOS {version} is available")),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(pal.text_dim)
                                                                .child(crate::update_check::how_to_update()),
                                                        ),
                                                ),
                                        )
                                        .child(
                                            h_flex()
                                                .flex_shrink_0()
                                                .gap_1()
                                                .child(
                                                    Button::new("update-whats-new")
                                                        .small()
                                                        .primary()
                                                        .label("What's new")
                                                        .on_click(|_, _, cx| cx.open_url(crate::update_check::RELEASES_URL)),
                                                )
                                                .child(
                                                    Button::new("update-dismiss")
                                                        .small()
                                                        .ghost()
                                                        .icon(IconName::Close)
                                                        .on_click(cx.listener(move |this, _, _, cx| {
                                                            if let Ok(mut g) = this.state.lock() {
                                                                g.config.update_dismissed = version.clone();
                                                                save_config(&g.config);
                                                            }
                                                            cx.notify();
                                                        })),
                                                ),
                                        ),
                                )
                            })
                            .when(setup_prompt, |this| {
                                this.child(
                                    h_flex()
                                        .w_full()
                                        .gap_3()
                                        .pl_4()
                                        .pr_2()
                                        .py_1p5()
                                        .rounded_full()
                                        .bg(pal.primary.opacity(0.14))
                                        .border_1()
                                        .border_color(pal.primary.opacity(0.35))
                                        .justify_between()
                                        .items_center()
                                        .child(
                                            h_flex()
                                                .gap_2()
                                                .min_w_0()
                                                .items_center()
                                                .child(div().flex_shrink_0().text_color(pal.accent).child(IconName::Sparkles))
                                                .child(
                                                    div()
                                                        .min_w_0()
                                                        .truncate()
                                                        .text_sm()
                                                        .text_color(pal.text)
                                                        .child("A new HawaOS is connected and waiting to be set up."),
                                                ),
                                        )
                                        .child(
                                            Button::new("setup-prompt")
                                                .small()
                                                .primary()
                                                .label("Set up")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.active_section = NavSection::InitialSetup;
                                                    cx.notify();
                                                })),
                                        ),
                                )
                            })
                            .child(content),
                    ),
            )
    }
}

/// Dev aid for screenshots: `HAWAOS_DEV_SECTION=device|firmware|setup|background`.
fn dev_start_section() -> Option<NavSection> {
    match std::env::var("HAWAOS_DEV_SECTION").as_deref() {
        Ok("device" | "firmware") => Some(NavSection::DeviceSettings),
        Ok("setup") => Some(NavSection::InitialSetup),
        Ok("background") => Some(NavSection::DaemonSettings),
        Ok("overview") => Some(NavSection::Overview),
        _ => None,
    }
}

/// Dev aid for layout checks: `HAWAOS_DEV_SIZE=760x560`.
fn dev_window_size() -> Option<Size<Pixels>> {
    let v = std::env::var("HAWAOS_DEV_SIZE").ok()?;
    let (w, h) = v.split_once('x')?;
    Some(size(px(w.parse().ok()?), px(h.parse().ok()?)))
}

pub fn open_companion_window(cx: &mut App, state: SharedState) {
    let translucent = if let Ok(mut g) = state.lock() {
        let setting = g.config.window_transparency.clone();
        let translucent = use_translucent_window(&setting);
        let msg = format!(
            "Window transparency '{}': {}",
            setting,
            if translucent { "blurred glass" } else { "solid backdrop" }
        );
        eprintln!("[companion] {}", msg);
        g.push_log(msg);
        // Dev aid for screenshots: `HAWAOS_DEV_OTA=path/to/firmware.bin`
        // starts a real update as the window opens.
        if let Ok(path) = std::env::var("HAWAOS_DEV_OTA") {
            g.ota = Some(crate::models::OtaProgress::new("starting", 0, None));
            g.outbound_queue.push_back(OutboundAction::StartOta { path: path.into() });
        }
        // Dev aid for tests: `HAWAOS_DEV_LINK=ble|wifi` presses the Connection
        // card's switch.
        if let Ok(link) = std::env::var("HAWAOS_DEV_LINK") {
            g.outbound_queue.push_back(OutboundAction::SetLink { ble: link == "ble", since: std::time::Instant::now() });
        }
        translucent
    } else {
        false
    };
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            dev_window_size().unwrap_or(size(px(1120.), px(760.))),
            cx,
        ))),
        // Below this the card grid can no longer wrap cleanly.
        window_min_size: Some(size(px(760.), px(560.))),
        // Blurred glass where gpui can blur (see `use_translucent_window`), else opaque.
        window_background: window_background(translucent),
        titlebar: Some(TitlebarOptions {
            title: Some("HawaOS Companion".into()),
            ..Default::default()
        }),
        app_id: Some(crate::config::APP_ID.into()),
        ..Default::default()
    };

    let _ = gpui_kit::open_window(options, cx, move |window, cx| {
        cx.new(|cx| CompanionWindowView::new(window, cx, Arc::clone(&state)))
    });
}
