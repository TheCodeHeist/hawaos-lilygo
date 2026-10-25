use crate::config::save_config;
use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{h_flex, v_flex, Sizable, StyledExt};
use gpui_kit::prelude::FluentBuilder as _;

use crate::models::{CompanionConfig, OutboundAction};
use crate::ui::theme::HawaPalette;
use crate::ui::CompanionWindowView;

impl CompanionWindowView {
    pub(crate) fn render_device_settings(
        &mut self,
        pal: HawaPalette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let cfg = if let Ok(g) = self.state.lock() {
            g.config.clone()
        } else {
            CompanionConfig::default()
        };

        let brightness_steps: [u8; 5] = [20, 40, 60, 80, 100];
        let aod_timeouts: [u16; 5] = [15, 30, 60, 120, 300];

        v_flex()
            .gap_4()
            .w_full()
            .min_w_0()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_xl()
                                    .font_bold()
                                    .text_color(pal.text)
                                    .child("Device Appearance & Behavior"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(pal.text_dim)
                                    .child("Configure your LilyGo T-Display-S3 in real time (persisted to ESP32-S3 NVS Flash)"),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("pull-device-cfg")
                                    .secondary()
                                    .icon(IconName::RefreshCw)
                                    .label("Read from Device")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            let via_usb = g.config.stream_via_usb;
                                            g.outbound_queue.push_back(
                                                OutboundAction::QueryDevice { via_usb },
                                            );
                                        }
                                        this.status_banner = Some(
                                            "Requested current configuration from T-Display-S3."
                                                .into(),
                                        );
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("push-device-cfg-main")
                                    .primary()
                                    .icon(IconName::Check)
                                    .label("Apply & Push to Device")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            g.config.config_version += 1;
                                            save_config(&g.config);
                                            g.outbound_queue.push_back(
                                                OutboundAction::PushConfig {
                                                    wifi_ssid: None,
                                                    wifi_pass: None,
                                                    forget_wifi: false,
                                                    force_usb: false,
                                                },
                                            );
                                        }
                                        this.status_banner = Some(
                                            "Configuration pushed to T-Display-S3.".into(),
                                        );
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Theme + Wallpaper Row
            .child(
                h_flex()
                    .gap_4()
                    .w_full()
                    .child(
                        // Device Theme Mode Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .gap_3()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Palette))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("HawaOS Device Theme"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child("Switch between Deep Cyan-Slate Dark, Crisp Daylight Light, or Auto"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("theme-dark-btn")
                                            .with_variant(if cfg.device_theme == "dark" {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .icon(IconName::Moon)
                                            .label("Dark")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.device_theme = "dark".into();
                                                });
                                            })),
                                    )
                                    .child(
                                        Button::new("theme-light-btn")
                                            .with_variant(if cfg.device_theme == "light" {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .icon(IconName::Sun)
                                            .label("Light")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.device_theme = "light".into();
                                                });
                                            })),
                                    )
                                    .child(
                                        Button::new("theme-auto-btn")
                                            .with_variant(if cfg.device_theme == "auto" {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .icon(IconName::Sparkles)
                                            .label("Auto")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.device_theme = "auto".into();
                                                });
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        // Wallpaper & Clock Format Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .gap_3()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Monitor))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Backdrop & Clock Format"),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Home & Launcher Backdrop"),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("wp-gradient")
                                                    .small()
                                                    .with_variant(if cfg.wallpaper == "gradient" {
                                                        gpui_kit::component::button::ButtonVariant::Primary
                                                    } else {
                                                        gpui_kit::component::button::ButtonVariant::Secondary
                                                    })
                                                    .label("Cyan Gradient")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.mutate_config_and_sync(cx, |c| {
                                                            c.wallpaper = "gradient".into();
                                                        });
                                                    })),
                                            )
                                            .child(
                                                Button::new("wp-solid")
                                                    .small()
                                                    .with_variant(if cfg.wallpaper == "solid" {
                                                        gpui_kit::component::button::ButtonVariant::Primary
                                                    } else {
                                                        gpui_kit::component::button::ButtonVariant::Secondary
                                                    })
                                                    .label("Solid Slate")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.mutate_config_and_sync(cx, |c| {
                                                            c.wallpaper = "solid".into();
                                                        });
                                                    })),
                                            ),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Time Format (Home, Status Bar & AOD)"),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("clk-24h")
                                                    .small()
                                                    .with_variant(if cfg.use_24h {
                                                        gpui_kit::component::button::ButtonVariant::Primary
                                                    } else {
                                                        gpui_kit::component::button::ButtonVariant::Secondary
                                                    })
                                                    .label("24-Hour")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.mutate_config_and_sync(cx, |c| {
                                                            c.use_24h = true;
                                                        });
                                                    })),
                                            )
                                            .child(
                                                Button::new("clk-12h")
                                                    .small()
                                                    .with_variant(if !cfg.use_24h {
                                                        gpui_kit::component::button::ButtonVariant::Primary
                                                    } else {
                                                        gpui_kit::component::button::ButtonVariant::Secondary
                                                    })
                                                    .label("12-Hour (AM/PM)")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.mutate_config_and_sync(cx, |c| {
                                                            c.use_24h = false;
                                                        });
                                                    })),
                                            ),
                                    ),
                            ),
                    ),
            )
            // Backlight Brightness Card
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Sun))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("ST7789V Backlight Brightness (PWM)"),
                                    ),
                            )
                            .child(
                                div()
                                    .text_base()
                                    .font_bold()
                                    .text_color(pal.accent)
                                    .child(format!("{}%", cfg.brightness)),
                            ),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(8.))
                            .rounded_full()
                            .bg(pal.surface2)
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(relative((cfg.brightness as f32) / 100.0))
                                    .rounded_full()
                                    .bg(pal.primary),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .children(brightness_steps.into_iter().map(|step| {
                                let selected = cfg.brightness == step;
                                Button::new(("brightness-step", step as usize))
                                    .with_variant(if selected {
                                        gpui_kit::component::button::ButtonVariant::Primary
                                    } else {
                                        gpui_kit::component::button::ButtonVariant::Secondary
                                    })
                                    .label(format!("{}%", step))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| {
                                            c.brightness = step;
                                        });
                                    }))
                            })),
                    ),
            )
            // AOD & Motion Row
            .child(
                h_flex()
                    .gap_4()
                    .w_full()
                    .child(
                        // Always-On Display Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .gap_3()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div().text_color(pal.accent).child(IconName::Eye),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_bold()
                                                    .text_color(pal.text)
                                                    .child("Always-On Display (AOD)"),
                                            ),
                                    )
                                    .child(
                                        Switch::new("aod-enable-switch")
                                            .checked(cfg.aod_enabled)
                                            .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                                let val = *checked;
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.aod_enabled = val;
                                                });
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child("Dims backlight to 12% after inactivity and shows pitch-black clock, date & marquee media strip"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(pal.text_dim)
                                            .child("Idle Timeout:"),
                                    )
                                    .children(aod_timeouts.into_iter().map(|secs| {
                                        let selected = cfg.aod_timeout_s == secs;
                                        Button::new(("aod-timeout", secs as usize))
                                            .small()
                                            .with_variant(if selected {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .label(if secs >= 60 {
                                                format!("{}m", secs / 60)
                                            } else {
                                                format!("{}s", secs)
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.aod_timeout_s = secs;
                                                });
                                            }))
                                    })),
                            ),
                    )
                    .child(
                        // Motion & Live Sync Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .gap_3()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div().text_color(pal.accent).child(IconName::Zap),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_bold()
                                                    .text_color(pal.text)
                                                    .child("Reduce Motion on Device"),
                                            ),
                                    )
                                    .child(
                                        Switch::new("reduce-motion-switch")
                                            .checked(cfg.reduce_motion)
                                            .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                                let val = *checked;
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.reduce_motion = val;
                                                });
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child("Replaces 180-220 ms screen/launcher slide animations with instant switches"),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .pt_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(pal.text)
                                            .child("Auto-push changes immediately"),
                                    )
                                    .child(
                                        Switch::new("auto-sync-switch")
                                            .checked(cfg.auto_sync_settings)
                                            .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                                let val = *checked;
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.config.auto_sync_settings = val;
                                                    save_config(&g.config);
                                                }
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
            )
            // Notification Banner Popup Card
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Bell))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Notification Popups on Screen"),
                                    ),
                            )
                            .child(
                                Switch::new("notify-popup-switch")
                                    .checked(cfg.notify_popup)
                                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                        let val = *checked;
                                        this.mutate_config_and_sync(cx, |c| {
                                            c.notify_popup = val;
                                        });
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(pal.text_dim)
                            .child("When enabled, incoming notifications slide in as a 54 px floating banner on top of the active screen with 4.5s auto-dismiss and wake on AOD"),
                    ),
            )
            // Screen Orientation Card
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::RefreshCw))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Screen Orientation"),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("orient-normal-btn")
                                            .with_variant(if cfg.orientation != "inverted" {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .label("Normal (USB-C Left)")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.orientation = "normal".into();
                                                });
                                            })),
                                    )
                                    .child(
                                        Button::new("orient-inverted-btn")
                                            .with_variant(if cfg.orientation == "inverted" {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .label("Inverted 180° (USB-C Right)")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.orientation = "inverted".into();
                                                });
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(pal.text_dim)
                            .child("Flips the display 180° and automatically swaps on-board button roles so the physical top button is always Select and bottom is Next"),
                    ),
            )
            // Weather & Location Configuration Card
            .child(
                v_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
                    .gap_3()
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Sun))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Weather & Geolocation Configuration"),
                                    ),
                            )
                            .child(
                                Button::new("weather-fetch-now-btn")
                                    .small()
                                    .secondary()
                                    .icon(IconName::RefreshCw)
                                    .label("Fetch & Push Now")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                        }
                                        this.status_banner = Some("Triggered weather update.".into());
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
                                            .font_semibold()
                                            .text_color(pal.text)
                                            .child("Automatic IP Geolocation"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Auto-detect city and coordinates via IP address; uncheck to specify custom city"),
                                    ),
                            )
                            .child(
                                Switch::new("weather-autoloc-switch")
                                    .checked(cfg.weather_auto_location)
                                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                        let val = *checked;
                                        this.mutate_config_and_sync(cx, |c| {
                                            c.weather_auto_location = val;
                                        });
                                        if let Ok(mut g) = this.state.lock() {
                                            g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                        }
                                    })),
                            ),
                    )
                    .when(!cfg.weather_auto_location, |el| {
                        el.child(
                            v_flex()
                                .gap_1()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_semibold()
                                        .text_color(pal.text_dim)
                                        .child("Custom City / Location Override"),
                                )
                                .child(
                                    h_flex()
                                        .gap_3()
                                        .items_center()
                                        .child(div().flex_1().child(Input::new(&self.weather_city_input).cleanable(true)))
                                        .child(
                                            Button::new("save-weather-city-btn")
                                                .primary()
                                                .icon(IconName::Check)
                                                .label("Save City")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    let new_city = this.weather_city_input.read(cx).value().trim().to_string();
                                                    this.mutate_config_and_sync(cx, |c| {
                                                        c.weather_city = new_city.clone();
                                                    });
                                                    if let Ok(mut g) = this.state.lock() {
                                                        g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                                    }
                                                    this.status_banner = Some(format!("Custom city set to '{}'.", new_city));
                                                    cx.notify();
                                                })),
                                        ),
                                ),
                        )
                    })
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
                                            .font_semibold()
                                            .text_color(pal.text)
                                            .child("Temperature Unit"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Choose Celsius (°C) or Fahrenheit (°F) for weather display on device"),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("weather-unit-c-btn")
                                            .small()
                                            .with_variant(if cfg.weather_unit != "f" {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .label("Celsius (°C)")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.weather_unit = "c".into();
                                                });
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                                }
                                            })),
                                    )
                                    .child(
                                        Button::new("weather-unit-f-btn")
                                            .small()
                                            .with_variant(if cfg.weather_unit == "f" {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .label("Fahrenheit (°F)")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| {
                                                    c.weather_unit = "f".into();
                                                });
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                                }
                                            })),
                                    ),
                            ),
                    ),
            )
    }
}
