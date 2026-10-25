use crate::media::handle_device_command;
use crate::models::MediaMsg;
use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState, NumberInput};
use gpui_kit::component::{h_flex, v_flex, Sizable, StyledExt};

use crate::config::save_config;
use crate::models::{CompanionConfig, OutboundAction};
use crate::ui::components::{
    button_row, card_header, card_row, choice, glass_card, page_header, parse_hex, pill,
    progress_bar, row_card, state_pill, time_readout,
};
use crate::ui::theme::HawaPalette;
use crate::ui::CompanionWindowView;

const METRIC_MIN_W: f32 = 200.;
const WIDE_CARD_MIN_W: f32 = 340.;

impl CompanionWindowView {
    pub(crate) fn render_overview(&mut self, pal: HawaPalette, cx: &mut Context<Self>) -> impl IntoElement {
        let (
            cpu,
            ram,
            used_gb,
            total_gb,
            up_kbps,
            down_kbps,
            hist,
            media,
            art_img,
            cfg,
            active_player,
            sw_state,
            sw_elapsed_ms,
            tm_state,
            tm_duration_sec,
            tm_remaining_sec,
            tm_remaining_ms,
            weather,
        ) = if let Ok(g) = self.state.lock() {
            (
                g.cpu_pct,
                g.ram_pct,
                g.used_ram_gb,
                g.total_ram_gb,
                g.net_up_kbps,
                g.net_down_kbps,
                g.cpu_history.iter().copied().collect::<Vec<u8>>(),
                g.media.clone(),
                g.current_art_png.clone(),
                g.config.clone(),
                g.active_player.clone(),
                g.stopwatch_state.clone(),
                g.stopwatch_elapsed_ms,
                g.timer_state.clone(),
                g.timer_duration_sec,
                g.timer_remaining_sec,
                g.timer_remaining_ms,
                g.weather.clone(),
            )
        } else {
            (
                0,
                0,
                0.0,
                1.0,
                0,
                0,
                vec![0; 30],
                MediaMsg::default(),
                None,
                CompanionConfig::default(),
                None,
                "stopped".to_string(),
                0,
                "stopped".to_string(),
                300,
                300,
                300_000,
                None,
            )
        };

        let tm_frac = if tm_duration_sec > 0 {
            tm_remaining_sec as f32 / tm_duration_sec as f32
        } else {
            0.0
        };
        let media_frac = if media.dur_ms > 0 {
            media.pos_ms as f32 / media.dur_ms as f32
        } else {
            0.0
        };
        let fmt_ms = |ms: u32| {
            let total_s = ms / 1000;
            format!("{}:{:02}", total_s / 60, total_s % 60)
        };
        let fmt_rate = |kbps: u64| {
            if kbps >= 1024 {
                format!("{:.1} MB/s", kbps as f64 / 1024.0)
            } else {
                format!("{} KB/s", kbps)
            }
        };
        let load_color = |pct: u8| {
            if pct > 85 {
                pal.danger
            } else if pct > 65 {
                pal.warn
            } else {
                pal.primary
            }
        };

        // Scale the sparkline to its recent peak (at least 25%) so low load still shows a shape.
        let hist_peak = hist.iter().copied().max().unwrap_or(0).max(25) as f32;

        let p_prev = active_player.clone();
        let p_play = active_player.clone();
        let p_next = active_player.clone();
        let has_track = !media.title.is_empty();
        let is_playing = media.state == "playing";
        // Album tint washes the Now Playing card, like the device's media screen.
        let tint = if has_track { parse_hex(&media.bg_top) } else { None };

        v_flex()
            .w_full()
            .gap_4()
            .child(
                page_header(pal, "Overview", "Live PC stats, media and tools on your T-Display S3")
                    .child(
                        button_row()
                            .child(
                                Button::new("overview-ping-btn")
                                    .secondary()
                                    .icon(IconName::RefreshCw)
                                    .label("Sync")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            g.outbound_queue.push_back(
                                                OutboundAction::QueryDevice { via_usb: false },
                                            );
                                        }
                                        this.status_banner = Some("Asked the device for its status.".into());
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("overview-push-cfg-btn")
                                    .primary()
                                    .icon(IconName::Send)
                                    .label("Push config")
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
                                        this.status_banner = Some("Configuration pushed to device.".into());
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Metrics
            .child(
                card_row()
                    .child(
                        row_card(pal, METRIC_MIN_W)
                            .child(
                                card_header(pal, IconName::Cpu, "CPU")
                                    .child(div().text_2xl().font_bold().child(format!("{}%", cpu))),
                            )
                            .child(
                                // Sparkline of recent CPU load; each bar sits in a faint track
                                // so the chart keeps its shape even at low load.
                                h_flex()
                                    .gap(px(3.))
                                    .h(px(36.))
                                    .children(hist.into_iter().map(|val| {
                                        v_flex()
                                            .flex_1()
                                            .h_full()
                                            .justify_end()
                                            .rounded(px(2.))
                                            .bg(pal.well)
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .w_full()
                                                    .h(relative((val as f32 / hist_peak).clamp(0.06, 1.0)))
                                                    .rounded(px(2.))
                                                    .bg(load_color(val)),
                                            )
                                    })),
                            ),
                    )
                    .child(
                        row_card(pal, METRIC_MIN_W)
                            .child(
                                card_header(pal, IconName::MemoryStick, "Memory")
                                    .child(div().text_2xl().font_bold().child(format!("{}%", ram))),
                            )
                            .child(progress_bar(pal, ram as f32 / 100.0, load_color(ram), 8.))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child(format!("{:.1} of {:.1} GB", used_gb, total_gb)),
                            ),
                    )
                    .child(
                        row_card(pal, METRIC_MIN_W)
                            .child(card_header(pal, IconName::ArrowDownUp, "Network"))
                            .child(
                                v_flex()
                                    .gap_1p5()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(div().text_color(pal.success).child(IconName::ArrowDown))
                                            .child(div().text_lg().font_semibold().child(fmt_rate(down_kbps as u64))),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(div().text_color(pal.accent).child(IconName::ArrowUp))
                                            .child(div().text_lg().font_semibold().child(fmt_rate(up_kbps as u64))),
                                    ),
                            ),
                    ),
            )
            // Now Playing + Notification composer
            .child(
                card_row()
                    .child(
                        row_card(pal, WIDE_CARD_MIN_W)
                            .when_some(tint, |this, t| {
                                this.bg(linear_gradient(
                                    135.,
                                    linear_color_stop(t.opacity(if pal.dark { 0.45 } else { 0.3 }), 0.),
                                    linear_color_stop(pal.glass, 0.85),
                                ))
                            })
                            .justify_between()
                            .child(
                                card_header(pal, IconName::Music, "Now Playing").child(
                                    h_flex()
                                        .gap_1p5()
                                        .when_some(active_player.clone(), |this, p| {
                                            this.child(pill(p, pal.text_dim, pal.well))
                                        })
                                        .child(state_pill(pal, &media.state)),
                                ),
                            )
                            .child(
                                h_flex()
                                    .gap_4()
                                    .items_center()
                                    .child(if let Some(art) = art_img {
                                        img(art)
                                            .flex_shrink_0()
                                            .size(px(72.))
                                            .rounded(px(12.))
                                            .into_any_element()
                                    } else {
                                        h_flex()
                                            .flex_shrink_0()
                                            .size(px(72.))
                                            .rounded(px(12.))
                                            .bg(pal.well)
                                            .items_center()
                                            .justify_center()
                                            .text_color(pal.text_muted)
                                            .child(IconName::Disc)
                                            .into_any_element()
                                    })
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .min_w_0()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .truncate()
                                                    .text_base()
                                                    .font_bold()
                                                    .child(if has_track {
                                                        media.title.clone()
                                                    } else {
                                                        "Nothing playing".to_string()
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .truncate()
                                                    .text_sm()
                                                    .text_color(pal.text_dim)
                                                    .child(if has_track {
                                                        media.artist.clone()
                                                    } else {
                                                        "Start any MPRIS player".to_string()
                                                    }),
                                            ),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(progress_bar(pal, media_frac, tint.unwrap_or(pal.primary), 4.))
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .child(div().w(px(40.)).text_xs().text_color(pal.text_dim).child(fmt_ms(media.pos_ms)))
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .child(
                                                        Button::new("media-prev")
                                                            .small()
                                                            .ghost()
                                                            .icon(IconName::SkipBack)
                                                            .on_click(move |_, _, _| {
                                                                handle_device_command("prev", p_prev.as_deref())
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new("media-play-pause")
                                                            .primary()
                                                            .icon(if is_playing { IconName::Pause } else { IconName::Play })
                                                            .on_click(move |_, _, _| {
                                                                handle_device_command("play_pause", p_play.as_deref())
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new("media-next")
                                                            .small()
                                                            .ghost()
                                                            .icon(IconName::SkipForward)
                                                            .on_click(move |_, _, _| {
                                                                handle_device_command("next", p_next.as_deref())
                                                            }),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .w(px(40.))
                                                    .flex()
                                                    .justify_end()
                                                    .text_xs()
                                                    .text_color(pal.text_dim)
                                                    .child(fmt_ms(media.dur_ms)),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        row_card(pal, WIDE_CARD_MIN_W)
                            .justify_between()
                            .child(card_header(pal, IconName::Bell, "Send Notification"))
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(Input::new(&self.notify_title_input).cleanable(true))
                                    .child(Input::new(&self.notify_body_input).cleanable(true)),
                            )
                            .child(
                                h_flex().justify_end().child(
                                    Button::new("send-notify-btn")
                                        .primary()
                                        .icon(IconName::Send)
                                        .label("Send")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            let title = this.notify_title_input.read(cx).value().to_string();
                                            let body = this.notify_body_input.read(cx).value().to_string();
                                            if title.trim().is_empty() {
                                                return;
                                            }
                                            if let Ok(mut g) = this.state.lock() {
                                                g.outbound_queue.push_back(OutboundAction::SendNotification {
                                                    app: title.clone(),
                                                    title: title.clone(),
                                                    body,
                                                });
                                                g.push_log(format!("Sent notification '{}' to device", title));
                                            }
                                            this.status_banner = Some(format!("Sent '{}' to device.", title));
                                            cx.notify();
                                        })),
                                ),
                            ),
                    ),
            )
            // Clock tools
            .child(
                card_row()
                    .child(
                        row_card(pal, CARD_TOOL_MIN_W)
                            .justify_between()
                            .child(card_header(pal, IconName::Timer, "Stopwatch").child(state_pill(pal, &sw_state)))
                            .child(
                                h_flex()
                                    .justify_center()
                                    .py_4()
                                    .child(time_readout(pal, sw_elapsed_ms as u64)),
                            )
                            .child(
                                button_row()
                                    .justify_center()
                                    .child(
                                        Button::new("sw-toggle-btn")
                                            .with_variant(choice(sw_state != "running"))
                                            .icon(if sw_state == "running" { IconName::Pause } else { IconName::Play })
                                            .label(if sw_state == "running" { "Pause" } else { "Start" })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.toggle_stopwatch();
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("sw-reset-btn")
                                            .secondary()
                                            .icon(IconName::RotateCcw)
                                            .label("Reset")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.reset_stopwatch();
                                                }
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(self.render_timer_card(pal, &tm_state, tm_remaining_ms as u64, tm_frac, cx)),
            )
            // Weather
            .child(match weather {
                Some(w) => glass_card(pal)
                    .child(
                        card_header(pal, IconName::CloudSun, format!("{} · {}", w.city, w.condition)).child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Button::new("ov-weather-unit-btn")
                                        .small()
                                        .ghost()
                                        .label(if cfg.weather_unit == "f" { "°F" } else { "°C" })
                                        .tooltip("Toggle unit")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.mutate_config_and_sync(cx, |c| {
                                                c.weather_unit = if c.weather_unit == "f" { "c".into() } else { "f".into() };
                                            });
                                            if let Ok(mut g) = this.state.lock() {
                                                g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                            }
                                        })),
                                )
                                .child(
                                    Button::new("ov-weather-refresh-btn")
                                        .small()
                                        .ghost()
                                        .icon(IconName::RefreshCw)
                                        .tooltip("Refresh")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Ok(mut g) = this.state.lock() {
                                                g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                            }
                                            this.status_banner = Some("Refreshing weather…".into());
                                            cx.notify();
                                        })),
                                ),
                        ),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_4()
                            .items_center()
                            .justify_between()
                            .child(
                                v_flex()
                                    .min_w(px(180.))
                                    .gap_0p5()
                                    .child(div().text_3xl().font_bold().child(format!("{}°", w.temp)))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child(format!(
                                                "Feels {}° · {}% humidity · {} {}",
                                                w.feels_like,
                                                w.humidity,
                                                w.wind_speed,
                                                if w.unit == "f" { "mph" } else { "km/h" }
                                            )),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .gap_2()
                                    .children(w.forecast.iter().map(|day| {
                                        v_flex()
                                            .min_w(px(64.))
                                            .px_3()
                                            .py_2()
                                            .rounded(px(12.))
                                            .bg(pal.well)
                                            .items_center()
                                            .gap_0p5()
                                            .child(div().text_xs().text_color(pal.text_dim).child(day.day.clone()))
                                            .child(div().text_sm().font_semibold().child(format!("{}°", day.max)))
                                            .child(div().text_xs().text_color(pal.text_muted).child(format!("{}°", day.min)))
                                    })),
                            ),
                    ),
                None => glass_card(pal).child(
                    card_header(pal, IconName::CloudSun, "Weather").child(
                        Button::new("ov-weather-sync-btn")
                            .small()
                            .secondary()
                            .icon(IconName::RefreshCw)
                            .label("Fetch")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Ok(mut g) = this.state.lock() {
                                    g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                }
                                this.status_banner = Some("Fetching weather…".into());
                                cx.notify();
                            })),
                    ),
                ),
            })
            // Quick toggles
            .child(
                glass_card(pal).child(
                    card_header(pal, IconName::Zap, "Quick toggles").child(
                        button_row()
                            .child(
                                Button::new("quick-theme-dark")
                                    .small()
                                    .with_variant(choice(cfg.device_theme == "dark"))
                                    .icon(IconName::Moon)
                                    .label("Dark")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| c.device_theme = "dark".into());
                                    })),
                            )
                            .child(
                                Button::new("quick-theme-light")
                                    .small()
                                    .with_variant(choice(cfg.device_theme == "light"))
                                    .icon(IconName::Sun)
                                    .label("Light")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| c.device_theme = "light".into());
                                    })),
                            )
                            .child(
                                Button::new("quick-clock-fmt")
                                    .small()
                                    .secondary()
                                    .icon(IconName::Clock)
                                    .label(if cfg.use_24h { "24h" } else { "12h" })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| c.use_24h = !c.use_24h);
                                    })),
                            ),
                    ),
                ),
            )
    }

    fn render_timer_card(
        &mut self,
        pal: HawaPalette,
        tm_state: &str,
        remaining_ms: u64,
        frac: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let running = tm_state == "running";
        // Each field fits the -/+ buttons plus 3 digits. Fields wrap in pairs,
        // so the layout is one row of four or a 2x2 grid, never 3 + 1.
        let field = |label: &'static str, state: &Entity<InputState>| {
            v_flex()
                .flex_1()
                .min_w(px(92.))
                .gap_1()
                .child(div().text_xs().text_color(pal.text_dim).child(label))
                .child(NumberInput::new(state).small().w_full())
        };
        let pair = |a: Div, b: Div| h_flex().flex_1().min_w(px(192.)).gap_2().child(a).child(b);
        let preset = |id: &'static str, label: &'static str, secs: u32, cx: &mut Context<Self>| {
            Button::new(id)
                .small()
                .ghost()
                .label(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Ok(mut g) = this.state.lock() {
                        g.start_timer(secs);
                    }
                    cx.notify();
                }))
        };

        row_card(pal, CARD_TOOL_MIN_W)
            .child(card_header(pal, IconName::Hourglass, "Timer").child(state_pill(pal, tm_state)))
            .child(
                v_flex()
                    .gap_2()
                    .items_center()
                    .py_1()
                    .child(time_readout(pal, remaining_ms))
                    .child(progress_bar(pal, frac, pal.accent, 4.)),
            )
            .child(
                v_flex()
                    .gap_3()
                    .p_3()
                    .rounded(px(12.))
                    .bg(pal.well)
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .gap_2()
                            .child(pair(
                                field("Hours", &self.timer_h_input),
                                field("Minutes", &self.timer_m_input),
                            ))
                            .child(pair(
                                field("Seconds", &self.timer_s_input),
                                field("Millis", &self.timer_ms_input),
                            )),
                    )
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_1()
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .gap_0p5()
                                    .child(preset("tm-pre-1m", "1m", 60, cx))
                                    .child(preset("tm-pre-5m", "5m", 300, cx))
                                    .child(preset("tm-pre-10m", "10m", 600, cx))
                                    .child(preset("tm-pre-25m", "25m", 1500, cx)),
                            )
                            .child(
                                Button::new("set-timer-val-btn")
                                    .small()
                                    .primary()
                                    .icon(IconName::Check)
                                    .label("Set")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        // Parse as f64 so a typed "5.0" doesn't silently become 0.
                                        let read = |state: &Entity<InputState>, max: u32, cx: &App| {
                                            (state.read(cx).value().trim().parse::<f64>().unwrap_or(0.).max(0.) as u32).min(max)
                                        };
                                        let h = read(&this.timer_h_input, 59, cx);
                                        let m = read(&this.timer_m_input, 99, cx);
                                        let s = read(&this.timer_s_input, 99, cx);
                                        let ms = read(&this.timer_ms_input, 999, cx);
                                        let total_ms = (h * 3600 + m * 60 + s) * 1000 + ms;
                                        if total_ms > 0 {
                                            if let Ok(mut g) = this.state.lock() {
                                                g.start_timer_ms(total_ms);
                                            }
                                            this.status_banner =
                                                Some(format!("Timer set to {:02}:{:02}:{:02}.{:03}", h, m, s, ms));
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ),
            )
            .child(
                button_row()
                    .justify_center()
                    .child(
                        Button::new("tm-toggle-btn")
                            .with_variant(choice(!running))
                            .icon(if running { IconName::Pause } else { IconName::Play })
                            .label(if running {
                                "Pause"
                            } else if tm_state == "paused" {
                                "Resume"
                            } else {
                                "Start"
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Ok(mut g) = this.state.lock() {
                                    g.toggle_timer();
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("tm-reset-btn")
                            .secondary()
                            .icon(IconName::RotateCcw)
                            .label("Reset")
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Ok(mut g) = this.state.lock() {
                                    g.reset_timer();
                                }
                                cx.notify();
                            })),
                    ),
            )
    }
}

const CARD_TOOL_MIN_W: f32 = 320.;
