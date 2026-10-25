use crate::media::handle_device_command;
use crate::models::MediaMsg;
use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, NumberInput};
use gpui_kit::component::{h_flex, v_flex, Sizable, StyledExt};
use gpui_kit::prelude::FluentBuilder as _;

use crate::config::save_config;
use crate::models::{CompanionConfig, OutboundAction};
use crate::ui::theme::HawaPalette;
use crate::ui::CompanionWindowView;

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
            reported_ip,
            active_player,
            sw_state,
            sw_elapsed_ms,
            tm_state,
            tm_duration_sec,
            tm_remaining_sec,
            tm_remaining_ms,
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
                g.device_reported_ip.clone(),
                g.active_player.clone(),
                g.stopwatch_state.clone(),
                g.stopwatch_elapsed_ms,
                g.timer_state.clone(),
                g.timer_duration_sec,
                g.timer_remaining_sec,
                g.timer_remaining_ms,
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
                None,
                "stopped".to_string(),
                0,
                "stopped".to_string(),
                300,
                300,
                300_000,
            )
        };

        let tm_progress_pct = if tm_duration_sec > 0 {
            ((tm_remaining_sec as f32 / tm_duration_sec as f32) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        let progress_pct = if media.dur_ms > 0 {
            ((media.pos_ms as f32 / media.dur_ms as f32) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        let fmt_ms = |ms: u32| {
            let total_s = ms / 1000;
            format!("{}:{:02}", total_s / 60, total_s % 60)
        };

        let player_label = match &active_player {
            Some(p) => format!("NOW PLAYING ({} -> DEVICE)", p.to_uppercase()),
            None => "NOW PLAYING (MPRIS -> DEVICE)".to_string(),
        };

        let p_prev = active_player.clone();
        let p_play = active_player.clone();
        let p_next = active_player.clone();

        v_flex()
            .gap_5()
            .w_full()
            .min_w_0()
            .child(
                // Top Page Title Row
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
                                    .child("Live Telemetry & Media Dashboard"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(pal.text_dim)
                                    .child("Streaming real-time PC metrics, MPRIS album art & notifications to your LilyGo T-Display-S3"),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("overview-ping-btn")
                                    .secondary()
                                    .icon(IconName::RefreshCw)
                                    .label("Sync Status")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            g.outbound_queue.push_back(
                                                OutboundAction::QueryDevice { via_usb: false },
                                            );
                                        }
                                        this.status_banner =
                                            Some("Sent status query to T-Display-S3.".into());
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("overview-push-cfg-btn")
                                    .primary()
                                    .icon(IconName::Send)
                                    .label("Push Config")
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
                                        this.status_banner =
                                            Some("Pushed active configuration to device.".into());
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // 3 Metric Cards Row (CPU, RAM, Network)
            .child(
                h_flex()
                    .gap_4()
                    .w_full()
                    .child(
                        // CPU Card
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
                                                div()
                                                    .text_color(pal.accent)
                                                    .child(IconName::Cpu),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.text_dim)
                                                    .child("CPU LOAD"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child(format!("{}%", cpu)),
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
                                            .w(relative((cpu as f32) / 100.0))
                                            .rounded_full()
                                            .bg(if cpu > 85 {
                                                pal.danger
                                            } else if cpu > 65 {
                                                pal.warn
                                            } else {
                                                pal.primary
                                            }),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .h(px(28.))
                                    .items_end()
                                    .children(hist.into_iter().map(|val| {
                                        let h_frac = ((val as f32) / 100.0).clamp(0.12, 1.0);
                                        div()
                                            .flex_1()
                                            .h(relative(h_frac))
                                            .rounded(px(2.))
                                            .bg(pal.primary.opacity(0.65))
                                    })),
                            ),
                    )
                    .child(
                        // RAM Card
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
                                                div()
                                                    .text_color(pal.accent)
                                                    .child(IconName::HardDrive),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.text_dim)
                                                    .child("MEMORY"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child(format!("{}%", ram)),
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
                                            .w(relative((ram as f32) / 100.0))
                                            .rounded_full()
                                            .bg(pal.accent),
                                    ),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child(format!(
                                        "{:.1} GB / {:.1} GB used",
                                        used_gb, total_gb
                                    )),
                            ),
                    )
                    .child(
                        // Network Throughput Card
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
                                                div()
                                                    .text_color(pal.accent)
                                                    .child(IconName::Wifi),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.text_dim)
                                                    .child("NETWORK I/O"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded(px(6.))
                                            .bg(pal.primary.opacity(0.18))
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(pal.accent)
                                            .child("1 Hz UDP"),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_color(pal.success)
                                                    .child(IconName::ArrowDown),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .text_color(pal.text)
                                                    .child(format!("{} KB/s", down_kbps)),
                                            ),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_color(pal.accent)
                                                    .child(IconName::ArrowUp),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .text_color(pal.text)
                                                    .child(format!("{} KB/s", up_kbps)),
                                            ),
                                    ),
                            )
                            .child(
                                div().text_xs().text_color(pal.text_dim).child(format!(
                                    "Device IP: {}",
                                    reported_ip.unwrap_or_else(|| cfg.target_ip.clone())
                                )),
                            ),
                    ),
            )
            // Middle Row: Now Playing Card + Quick Notification Dispatcher
            .child(
                h_flex()
                    .gap_4()
                    .w_full()
                    .items_stretch()
                    .child(
                        // Spotify-style Now Playing Preview Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .justify_between()
                            .gap_4()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_color(pal.accent)
                                                    .child(IconName::Music),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.text_dim)
                                                    .child(player_label),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded(px(6.))
                                            .bg(pal.surface2)
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child(media.state.to_uppercase()),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_4()
                                    .items_center()
                                    .child(if let Some(art) = art_img {
                                        div()
                                            .w(px(76.))
                                            .h(px(76.))
                                            .rounded(px(12.))
                                            .overflow_hidden()
                                            .border_1()
                                            .border_color(pal.border)
                                            .child(img(art).w(px(76.)).h(px(76.)))
                                            .into_any_element()
                                    } else {
                                        h_flex()
                                            .w(px(76.))
                                            .h(px(76.))
                                            .rounded(px(12.))
                                            .bg(pal.surface2)
                                            .border_1()
                                            .border_color(pal.border)
                                            .items_center()
                                            .justify_center()
                                            .text_color(pal.accent)
                                            .child(IconName::Music)
                                            .into_any_element()
                                    })
                                    .child(
                                        v_flex()
                                            .flex_1()
                                            .gap_1()
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .text_base()
                                                    .font_bold()
                                                    .text_color(pal.text)
                                                    .child(if media.title.is_empty() {
                                                        "No Active Track".to_string()
                                                    } else {
                                                        media.title.clone()
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .text_color(pal.text_dim)
                                                    .child(if media.artist.is_empty() {
                                                        "Play audio in Spotify, Sonora, Firefox, or any MPRIS player".to_string()
                                                    } else {
                                                        media.artist.clone()
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(pal.text_muted)
                                                    .child(format!(
                                                        "Tint: {} -> {} · Art hash: {}",
                                                        media.bg_top,
                                                        media.bg_bottom,
                                                        if media.art.is_empty() {
                                                            "none"
                                                        } else {
                                                            &media.art
                                                        }
                                                    )),
                                            ),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w_full()
                                            .h(px(6.))
                                            .rounded_full()
                                            .bg(pal.surface2)
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .h_full()
                                                    .w(relative(progress_pct / 100.0))
                                                    .rounded_full()
                                                    .bg(pal.primary),
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
                                                    .child(fmt_ms(media.pos_ms)),
                                            )
                                            .child(
                                                h_flex()
                                                    .gap_2()
                                                    .child(
                                                        Button::new("media-prev")
                                                            .small()
                                                            .secondary()
                                                            .icon(IconName::SkipBack)
                                                            .on_click(move |_, _, _| {
                                                                handle_device_command("prev", p_prev.as_deref())
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new("media-play-pause")
                                                            .small()
                                                            .primary()
                                                            .icon(if media.state == "playing" {
                                                                IconName::Pause
                                                            } else {
                                                                IconName::Play
                                                            })
                                                            .label(if media.state == "playing" {
                                                                "Pause"
                                                            } else {
                                                                "Play"
                                                            })
                                                            .on_click(move |_, _, _| {
                                                                handle_device_command("play_pause", p_play.as_deref())
                                                            }),
                                                    )
                                                    .child(
                                                        Button::new("media-next")
                                                            .small()
                                                            .secondary()
                                                            .icon(IconName::SkipForward)
                                                            .on_click(move |_, _, _| {
                                                                handle_device_command("next", p_next.as_deref())
                                                            }),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(pal.text_dim)
                                                    .child(fmt_ms(media.dur_ms)),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        // Push Notification to T-Display-S3 Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .justify_between()
                            .gap_3()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(div().text_color(pal.accent).child(IconName::Bell))
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(pal.text_dim)
                                            .child("PUSH NOTIFICATION TO DEVICE"),
                                    ),
                            )
                            .child(
                                v_flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Title"),
                                    )
                                    .child(Input::new(&self.notify_title_input).cleanable(true))
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Message Body"),
                                    )
                                    .child(Input::new(&self.notify_body_input).cleanable(true)),
                            )
                            .child(
                                h_flex()
                                    .justify_end()
                                    .child(
                                        Button::new("send-notify-btn")
                                            .primary()
                                            .icon(IconName::Send)
                                            .label("Send to T-Display-S3")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let title =
                                                    this.notify_title_input.read(cx).value().to_string();
                                                let body =
                                                    this.notify_body_input.read(cx).value().to_string();
                                                if !title.trim().is_empty() {
                                                    if let Ok(mut g) = this.state.lock() {
                                                        g.outbound_queue.push_back(
                                                            OutboundAction::SendNotification {
                                                                app: title.clone(),
                                                                title: title.clone(),
                                                                body,
                                                            },
                                                        );
                                                        g.push_log(format!(
                                                            "Sent notification '{}' to device",
                                                            title
                                                        ));
                                                    }
                                                    this.status_banner = Some(format!(
                                                        "Notification '{}' dispatched to device.",
                                                        title
                                                    ));
                                                    cx.notify();
                                                }
                                            })),
                                    ),
                            ),
                    ),
            )
            // Clock Tools Card: Live Stopwatch & Countdown Timer
            .child(
                h_flex()
                    .gap_4()
                    .w_full()
                    .items_stretch()
                    .child(
                        // Stopwatch Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .justify_between()
                            .gap_4()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(div().text_color(pal.accent).child(IconName::Timer))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.text_dim)
                                                    .child("STOPWATCH"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded(px(6.))
                                            .bg(if sw_state == "running" {
                                                pal.primary.opacity(0.18)
                                            } else if sw_state == "paused" {
                                                pal.warn.opacity(0.18)
                                            } else {
                                                pal.surface2
                                            })
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(if sw_state == "running" {
                                                pal.accent
                                            } else if sw_state == "paused" {
                                                pal.warn
                                            } else {
                                                pal.text_dim
                                            })
                                            .child(sw_state.to_uppercase()),
                                    ),
                            )
                            .child({
                                let total_s = sw_elapsed_ms / 1000;
                                let msec = sw_elapsed_ms % 1000;
                                let h = total_s / 3600;
                                let m = (total_s / 60) % 60;
                                let s = total_s % 60;
                                let sw_hms = format!("{:02}:{:02}:{:02}", h, m, s);
                                let sw_ms = format!(".{:03}", msec);

                                v_flex()
                                    .gap_1()
                                    .items_center()
                                    .py_2()
                                    .child(
                                        h_flex()
                                            .items_baseline()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_3xl()
                                                    .font_bold()
                                                    .text_color(pal.text)
                                                    .child(sw_hms),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .text_color(pal.text_dim)
                                                    .child(sw_ms),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(pal.text_dim)
                                            .child("Live stopwatch synced to LilyGo AOD & Clock app"),
                                    )
                            })
                            .child(
                                h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        Button::new("sw-toggle-btn")
                                            .small()
                                            .with_variant(if sw_state == "running" {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            })
                                            .icon(if sw_state == "running" {
                                                IconName::Pause
                                            } else {
                                                IconName::Play
                                            })
                                            .label(if sw_state == "running" {
                                                "Pause"
                                            } else {
                                                "Start"
                                            })
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.toggle_stopwatch();
                                                }
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("sw-reset-btn")
                                            .small()
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
                    .child(
                        // Countdown Timer Card
                        v_flex()
                            .flex_1()
                            .p_4()
                            .rounded(px(14.))
                            .bg(pal.surface)
                            .border_1()
                            .border_color(pal.border)
                            .justify_between()
                            .gap_4()
                            .child(
                                h_flex()
                                    .justify_between()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(div().text_color(pal.accent).child(IconName::Clock))
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .font_semibold()
                                                    .text_color(pal.text_dim)
                                                    .child("COUNTDOWN TIMER"),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px_2()
                                            .py_0p5()
                                            .rounded(px(6.))
                                            .bg(if tm_state == "running" {
                                                pal.primary.opacity(0.18)
                                            } else if tm_state == "paused" {
                                                pal.warn.opacity(0.18)
                                            } else {
                                                pal.surface2
                                            })
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(if tm_state == "running" {
                                                pal.accent
                                            } else if tm_state == "paused" {
                                                pal.warn
                                            } else {
                                                pal.text_dim
                                            })
                                            .child(tm_state.to_uppercase()),
                                    ),
                            )
                            .child({
                                let total_s = tm_remaining_ms / 1000;
                                let msec = tm_remaining_ms % 1000;
                                let h = total_s / 3600;
                                let m = (total_s / 60) % 60;
                                let s = total_s % 60;
                                let tm_hms = format!("{:02}:{:02}:{:02}", h, m, s);
                                let tm_ms = format!(".{:03}", msec);

                                v_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        h_flex()
                                            .items_baseline()
                                            .gap_0p5()
                                            .child(
                                                div()
                                                    .text_3xl()
                                                    .font_bold()
                                                    .text_color(pal.text)
                                                    .child(tm_hms),
                                            )
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_semibold()
                                                    .text_color(pal.text_dim)
                                                    .child(tm_ms),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .w_full()
                                            .h(px(6.))
                                            .rounded_full()
                                            .bg(pal.surface2)
                                            .overflow_hidden()
                                            .child(
                                                div()
                                                    .h_full()
                                                    .w(relative(tm_progress_pct / 100.0))
                                                    .rounded_full()
                                                    .bg(pal.accent),
                                            ),
                                    )
                                    .child(
                                        // Arbitrary Time Selector (up to 59:99:99.999 max) using GPUI NumberInput
                                        h_flex()
                                            .w_full()
                                            .items_center()
                                            .justify_between()
                                            .gap_2()
                                            .p_2()
                                            .rounded(px(10.))
                                            .bg(pal.surface2)
                                            .child(
                                                h_flex()
                                                    .items_center()
                                                    .gap_1()
                                                    .child(
                                                        v_flex()
                                                            .items_center()
                                                            .child(div().text_xs().text_color(pal.text_dim).child("HH"))
                                                            .child(
                                                                div()
                                                                    .w(px(48.))
                                                                    .child(NumberInput::new(&self.timer_h_input).placeholder("00"))
                                                            )
                                                    )
                                                    .child(div().text_sm().font_bold().text_color(pal.text_dim).child(":"))
                                                    .child(
                                                        v_flex()
                                                            .items_center()
                                                            .child(div().text_xs().text_color(pal.text_dim).child("MM"))
                                                            .child(
                                                                div()
                                                                    .w(px(48.))
                                                                    .child(NumberInput::new(&self.timer_m_input).placeholder("05"))
                                                            )
                                                    )
                                                    .child(div().text_sm().font_bold().text_color(pal.text_dim).child(":"))
                                                    .child(
                                                        v_flex()
                                                            .items_center()
                                                            .child(div().text_xs().text_color(pal.text_dim).child("SS"))
                                                            .child(
                                                                div()
                                                                    .w(px(48.))
                                                                    .child(NumberInput::new(&self.timer_s_input).placeholder("00"))
                                                            )
                                                    )
                                                    .child(div().text_sm().font_bold().text_color(pal.text_dim).child("."))
                                                    .child(
                                                        v_flex()
                                                            .items_center()
                                                            .child(div().text_xs().text_color(pal.text_dim).child("MS"))
                                                            .child(
                                                                div()
                                                                    .w(px(56.))
                                                                    .child(NumberInput::new(&self.timer_ms_input).placeholder("000"))
                                                            )
                                                    )
                                            )
                                            .child(
                                                Button::new("set-timer-val-btn")
                                                    .primary()
                                                    .small()
                                                    .icon(IconName::Clock)
                                                    .label("Set")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        let h = this.timer_h_input.read(cx).value().trim().parse::<u32>().unwrap_or(0).min(59);
                                                        let m = this.timer_m_input.read(cx).value().trim().parse::<u32>().unwrap_or(0).min(99);
                                                        let s = this.timer_s_input.read(cx).value().trim().parse::<u32>().unwrap_or(0).min(99);
                                                        let ms = this.timer_ms_input.read(cx).value().trim().parse::<u32>().unwrap_or(0).min(999);
                                                        let total_ms = (h * 3600 + m * 60 + s) * 1000 + ms;
                                                        if total_ms > 0 {
                                                            if let Ok(mut g) = this.state.lock() {
                                                                g.start_timer_ms(total_ms);
                                                            }
                                                            this.status_banner = Some(format!("Timer set to {:02}:{:02}:{:02}.{:03}", h, m, s, ms));
                                                            cx.notify();
                                                        }
                                                    }))
                                            )
                                    )
                                    .child(
                                        h_flex()
                                            .gap_1p5()
                                            .items_center()
                                            .child(
                                                Button::new("tm-pre-1m")
                                                    .small()
                                                    .secondary()
                                                    .label("1m")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        if let Ok(mut g) = this.state.lock() {
                                                            g.start_timer(60);
                                                        }
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                Button::new("tm-pre-3m")
                                                    .small()
                                                    .secondary()
                                                    .label("3m")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        if let Ok(mut g) = this.state.lock() {
                                                            g.start_timer(180);
                                                        }
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                Button::new("tm-pre-5m")
                                                    .small()
                                                    .secondary()
                                                    .label("5m")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        if let Ok(mut g) = this.state.lock() {
                                                            g.start_timer(300);
                                                        }
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                Button::new("tm-pre-10m")
                                                    .small()
                                                    .secondary()
                                                    .label("10m")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        if let Ok(mut g) = this.state.lock() {
                                                            g.start_timer(600);
                                                        }
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                Button::new("tm-pre-15m")
                                                    .small()
                                                    .secondary()
                                                    .label("15m")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        if let Ok(mut g) = this.state.lock() {
                                                            g.start_timer(900);
                                                        }
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                Button::new("tm-pre-25m")
                                                    .small()
                                                    .secondary()
                                                    .label("25m")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        if let Ok(mut g) = this.state.lock() {
                                                            g.start_timer(1500);
                                                        }
                                                        cx.notify();
                                                    })),
                                            ),
                                    )
                            })
                            .child(
                                h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        Button::new("tm-toggle-btn")
                                            .small()
                                            .with_variant(if tm_state == "running" {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            })
                                            .icon(if tm_state == "running" {
                                                IconName::Pause
                                            } else {
                                                IconName::Play
                                            })
                                            .label(if tm_state == "running" {
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
                                            .small()
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
                            ),
                    ),
            )
            // Live Weather Card
            .child({
                let weather_opt = if let Ok(g) = self.state.lock() {
                    g.weather.clone()
                } else {
                    None
                };

                if let Some(w) = weather_opt {
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
                                                .child("Live Weather"),
                                        )
                                        .child(
                                            div()
                                                .px_2()
                                                .py_0p5()
                                                .rounded(px(6.))
                                                .bg(pal.primary.opacity(0.18))
                                                .text_xs()
                                                .font_medium()
                                                .text_color(pal.accent)
                                                .child(format!("{} · {}", w.city, w.condition)),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .child(
                                            Button::new("ov-weather-unit-btn")
                                                .small()
                                                .secondary()
                                                .label(if cfg.weather_unit == "f" { "Unit: °F" } else { "Unit: °C" })
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
                                                .secondary()
                                                .icon(IconName::RefreshCw)
                                                .label("Refresh")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    if let Ok(mut g) = this.state.lock() {
                                                        g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                                    }
                                                    this.status_banner = Some("Refreshing weather forecast...".into());
                                                    cx.notify();
                                                })),
                                        ),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_4()
                                .items_center()
                                .child(
                                    v_flex()
                                        .gap_1()
                                        .child(
                                            div()
                                                .text_2xl()
                                                .font_bold()
                                                .text_color(pal.text)
                                                .child(format!("{}°{}", w.temp, w.unit.to_uppercase())),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(pal.text_dim)
                                                .child(format!(
                                                    "Feels like {}° · H: {}° L: {}° · {}% hum · {} {}",
                                                    w.feels_like,
                                                    w.temp_max,
                                                    w.temp_min,
                                                    w.humidity,
                                                    w.wind_speed,
                                                    if w.unit == "f" { "mph" } else { "km/h" }
                                                )),
                                        ),
                                )
                                .child(
                                    h_flex()
                                        .gap_2()
                                        .flex_1()
                                        .justify_end()
                                        .children(w.forecast.iter().map(|day| {
                                            v_flex()
                                                .p_2p5()
                                                .rounded(px(10.))
                                                .bg(pal.surface2)
                                                .border_1()
                                                .border_color(pal.border)
                                                .items_center()
                                                .gap_1()
                                                .min_w(px(72.))
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_semibold()
                                                        .text_color(pal.text)
                                                        .child(day.day.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .font_bold()
                                                        .text_color(pal.accent)
                                                        .child(format!("{}° / {}°", day.min, day.max)),
                                                )
                                        })),
                                ),
                        )
                } else {
                    v_flex()
                        .p_4()
                        .rounded(px(14.))
                        .bg(pal.surface)
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
                                        .child(div().text_color(pal.accent).child(IconName::Sun))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_bold()
                                                .text_color(pal.text)
                                                .child("Live Weather Forecast"),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(pal.text_dim)
                                                .child("Waiting for Open-Meteo weather sync..."),
                                        ),
                                )
                                .child(
                                    Button::new("ov-weather-sync-btn")
                                        .small()
                                        .primary()
                                        .icon(IconName::RefreshCw)
                                        .label("Sync Weather")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Ok(mut g) = this.state.lock() {
                                                g.outbound_queue.push_back(OutboundAction::RefreshWeather);
                                            }
                                            this.status_banner = Some("Fetching weather from Open-Meteo...".into());
                                            cx.notify();
                                        })),
                                ),
                        )
                }
            })
            // Bottom Row: Quick Device Controls Summary
            .child(
                h_flex()
                    .p_4()
                    .rounded(px(14.))
                    .bg(pal.surface)
                    .border_1()
                    .border_color(pal.border)
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
                                    .child("Quick Device Toggles"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child(format!(
                                        "Theme: {} · Brightness: {}% · AOD: {} ({}s) · Clock: {} · Popups: {}",
                                        cfg.device_theme.to_uppercase(),
                                        cfg.brightness,
                                        if cfg.aod_enabled { "On" } else { "Off" },
                                        cfg.aod_timeout_s,
                                        if cfg.use_24h { "24h" } else { "12h AM/PM" },
                                        if cfg.notify_popup { "On" } else { "Off" }
                                    )),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("quick-theme-dark")
                                    .small()
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
                                Button::new("quick-theme-light")
                                    .small()
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
                                Button::new("quick-clock-fmt")
                                    .small()
                                    .secondary()
                                    .icon(IconName::Clock)
                                    .label(if cfg.use_24h { "24-Hour" } else { "12-Hour" })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.mutate_config_and_sync(cx, |c| {
                                            c.use_24h = !c.use_24h;
                                        });
                                    })),
                            ),
                    ),
            )
    }
}
