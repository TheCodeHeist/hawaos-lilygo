use crate::config::save_config;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;

use crate::models::{CompanionConfig, OutboundAction};
use crate::ui::CompanionWindowView;
use crate::ui::accents::ACCENTS;
use crate::ui::components::{
    accent_swatch, button_row, card_header, card_row, choice, divider, glass_card, page_header,
    pill, progress_bar, row_card, setting_row,
};
use crate::ui::theme::{accent_by_id, HawaPalette};

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
        let current_accent = accent_by_id(&cfg.accent);
        let aod_timeouts: [u16; 5] = [15, 30, 60, 120, 300];

        v_flex()
            .w_full()
            .gap_4()
            .child(
                page_header(pal, "Device", "Changes are saved on the device and survive reboots")
                    .child(
                        button_row()
                            .child(
                                Button::new("pull-device-cfg")
                                    .secondary()
                                    .icon(IconName::RefreshCw)
                                    .label("Read")
                                    .tooltip("Read current settings from the device")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            let via_usb = g.config.stream_via_usb;
                                            g.outbound_queue
                                                .push_back(OutboundAction::QueryDevice { via_usb });
                                        }
                                        this.status_banner = Some("Requested settings from the device.".into());
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("push-device-cfg-main")
                                    .primary()
                                    .icon(IconName::Check)
                                    .label("Apply")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            g.config.config_version += 1;
                                            save_config(&g.config);
                                            g.outbound_queue.push_back(OutboundAction::PushConfig {
                                                wifi_ssid: None,
                                                wifi_pass: None,
                                                forget_wifi: false,
                                                force_usb: false,
                                            });
                                        }
                                        this.status_banner = Some("Settings pushed to device.".into());
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Appearance
            .child(
                card_row()
                    .child(
                        row_card(pal, 300.)
                            .child(card_header(pal, IconName::SunMoon, "Theme"))
                            .child(
                                button_row()
                                    .child(
                                        Button::new("theme-dark-btn")
                                            .with_variant(choice(pal, cfg.device_theme == "dark", cx))
                                            .icon(IconName::Moon)
                                            .label("Dark")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| c.device_theme = "dark".into());
                                            })),
                                    )
                                    .child(
                                        Button::new("theme-light-btn")
                                            .with_variant(choice(pal, cfg.device_theme == "light", cx))
                                            .icon(IconName::Sun)
                                            .label("Light")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| c.device_theme = "light".into());
                                            })),
                                    )
                                    .child(
                                        Button::new("theme-auto-btn")
                                            .with_variant(choice(pal, cfg.device_theme == "auto", cx))
                                            .icon(IconName::Sparkles)
                                            .label("Auto")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| c.device_theme = "auto".into());
                                            })),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child("Auto follows the time of day."),
                            ),
                    )
                    .child(
                        row_card(pal, 300.)
                            .child(card_header(pal, IconName::Monitor, "Home screen"))
                            .child(setting_row(
                                pal,
                                "Backdrop",
                                None,
                                button_row()
                                    .child(
                                        Button::new("wp-gradient")
                                            .small()
                                            .with_variant(choice(pal, cfg.wallpaper == "gradient", cx))
                                            .label("Gradient")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| c.wallpaper = "gradient".into());
                                            })),
                                    )
                                    .child(
                                        Button::new("wp-solid")
                                            .small()
                                            .with_variant(choice(pal, cfg.wallpaper == "solid", cx))
                                            .label("Solid")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| c.wallpaper = "solid".into());
                                            })),
                                    ),
                            ))
                            .child(setting_row(
                                pal,
                                "Time format",
                                None,
                                button_row()
                                    .child(
                                        Button::new("clk-24h")
                                            .small()
                                            .with_variant(choice(pal, cfg.use_24h, cx))
                                            .label("24h")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| c.use_24h = true);
                                            })),
                                    )
                                    .child(
                                        Button::new("clk-12h")
                                            .small()
                                            .with_variant(choice(pal, !cfg.use_24h, cx))
                                            .label("12h")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.mutate_config_and_sync(cx, |c| c.use_24h = false);
                                            })),
                                    ),
                            )),
                    ),
            )
            // Accent color
            .child(
                glass_card(pal)
                    .child(
                        card_header(pal, IconName::Palette, "Accent color").child(pill(
                            current_accent.label,
                            pal.accent,
                            pal.primary.opacity(0.16),
                        )),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .gap_2()
                            .children(ACCENTS.iter().map(|accent| {
                                let id = accent.id;
                                accent_swatch(pal, accent, accent.id == current_accent.id)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| c.accent = id.into());
                                    }))
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(pal.text_dim)
                            .child("Recolors the device and this app, backgrounds included."),
                    ),
            )
            // Brightness
            .child(
                glass_card(pal)
                    .child(
                        card_header(pal, IconName::Sun, "Brightness").child(
                            div()
                                .text_base()
                                .font_bold()
                                .text_color(pal.accent)
                                .child(format!("{}%", cfg.brightness)),
                        ),
                    )
                    .child(progress_bar(pal, cfg.brightness as f32 / 100.0, pal.primary, 6.))
                    .child(button_row().children(brightness_steps.into_iter().map(|step| {
                        Button::new(("brightness-step", step as usize))
                            .small()
                            .with_variant(choice(pal, cfg.brightness == step, cx))
                            .label(format!("{}%", step))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.mutate_config_and_sync(cx, |c| c.brightness = step);
                            }))
                    }))),
            )
            // Behavior
            .child(
                glass_card(pal)
                    .child(card_header(pal, IconName::Eye, "Behavior"))
                    .child(setting_row(
                        pal,
                        "Always-on display",
                        Some("Dims the screen and shows the clock after a period of inactivity."),
                        Switch::new("aod-enable-switch")
                            .checked(cfg.aod_enabled)
                            .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                let val = *checked;
                                this.mutate_config_and_sync(cx, |c| c.aod_enabled = val);
                            })),
                    ))
                    .when(cfg.aod_enabled, |this| {
                        this.child(setting_row(
                            pal,
                            "Idle timeout",
                            None,
                            button_row().children(aod_timeouts.into_iter().map(|secs| {
                                Button::new(("aod-timeout", secs as usize))
                                    .small()
                                    .with_variant(choice(pal, cfg.aod_timeout_s == secs, cx))
                                    .label(if secs >= 60 {
                                        format!("{}m", secs / 60)
                                    } else {
                                        format!("{}s", secs)
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| c.aod_timeout_s = secs);
                                    }))
                            })),
                        ))
                    })
                    .child(divider(pal))
                    .child(setting_row(
                        pal,
                        "Notification popups",
                        Some("Show incoming notifications on top of the current screen."),
                        Switch::new("notify-popup-switch")
                            .checked(cfg.notify_popup)
                            .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                let val = *checked;
                                this.mutate_config_and_sync(cx, |c| c.notify_popup = val);
                            })),
                    ))
                    .child(divider(pal))
                    .child(setting_row(
                        pal,
                        "Reduce motion",
                        Some("Switch screens instantly instead of sliding."),
                        Switch::new("reduce-motion-switch")
                            .checked(cfg.reduce_motion)
                            .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                let val = *checked;
                                this.mutate_config_and_sync(cx, |c| c.reduce_motion = val);
                            })),
                    ))
                    .child(divider(pal))
                    .child(setting_row(
                        pal,
                        "Apply changes instantly",
                        Some("Otherwise, changes are sent when you press Apply."),
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
                    ))
                    .child(divider(pal))
                    .child(setting_row(
                        pal,
                        "Orientation",
                        Some("Flipping also swaps the button roles."),
                        button_row()
                            .child(
                                Button::new("orient-normal-btn")
                                    .small()
                                    .with_variant(choice(pal, cfg.orientation != "inverted", cx))
                                    .label("Normal")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| c.orientation = "normal".into());
                                    })),
                            )
                            .child(
                                Button::new("orient-inverted-btn")
                                    .small()
                                    .with_variant(choice(pal, cfg.orientation == "inverted", cx))
                                    .label("Flipped")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| c.orientation = "inverted".into());
                                    })),
                            ),
                    )),
            )
            // Weather
            .child(
                glass_card(pal)
                    .child(
                        card_header(pal, IconName::CloudSun, "Weather").child(
                            Button::new("weather-fetch-now-btn")
                                .small()
                                .ghost()
                                .icon(IconName::RefreshCw)
                                .label("Update")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Ok(mut g) = this.state.lock() {
                                        g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                    }
                                    this.status_banner = Some("Updating weather…".into());
                                    cx.notify();
                                })),
                        ),
                    )
                    .child(setting_row(
                        pal,
                        "Detect location automatically",
                        Some("Uses your IP address. Turn off to choose a city."),
                        Switch::new("weather-autoloc-switch")
                            .checked(cfg.weather_auto_location)
                            .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                let val = *checked;
                                this.mutate_config_and_sync(cx, |c| c.weather_auto_location = val);
                                if let Ok(mut g) = this.state.lock() {
                                    g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                }
                            })),
                    ))
                    .when(!cfg.weather_auto_location, |el| {
                        el.child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(div().flex_1().min_w_0().child(Input::new(&self.weather_city_input).cleanable(true)))
                                .child(
                                    Button::new("save-weather-city-btn")
                                        .primary()
                                        .icon(IconName::Check)
                                        .label("Save")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            let new_city = this.weather_city_input.read(cx).value().trim().to_string();
                                            this.mutate_config_and_sync(cx, |c| c.weather_city = new_city.clone());
                                            if let Ok(mut g) = this.state.lock() {
                                                g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                            }
                                            this.status_banner = Some(format!("City set to '{}'.", new_city));
                                            cx.notify();
                                        })),
                                ),
                        )
                    })
                    .child(divider(pal))
                    .child(setting_row(
                        pal,
                        "Temperature unit",
                        None,
                        button_row()
                            .child(
                                Button::new("weather-unit-c-btn")
                                    .small()
                                    .with_variant(choice(pal, cfg.weather_unit != "f", cx))
                                    .label("°C")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| c.weather_unit = "c".into());
                                        if let Ok(mut g) = this.state.lock() {
                                            g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                        }
                                    })),
                            )
                            .child(
                                Button::new("weather-unit-f-btn")
                                    .small()
                                    .with_variant(choice(pal, cfg.weather_unit == "f", cx))
                                    .label("°F")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| c.weather_unit = "f".into());
                                        if let Ok(mut g) = this.state.lock() {
                                            g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                        }
                                    })),
                            ),
                    )),
            )
    }
}
