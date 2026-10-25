use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{h_flex, v_flex, Sizable, StyledExt};
use gpui_kit::prelude::FluentBuilder as _;

use crate::config::{save_config, UDP_PORT};
use crate::models::OutboundAction;
use crate::config::{sanitize_ip_str, scan_wifi_networks, CompanionConfig};
use crate::protocol::scan_serial_ports;
use crate::ui::theme::HawaPalette;
use crate::ui::CompanionWindowView;

impl CompanionWindowView {
    pub(crate) fn render_initial_setup(
        &mut self,
        pal: HawaPalette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (cfg, ports, reported_ip, wifi_connected, ssids) = if let Ok(g) = self.state.lock() {
            (
                g.config.clone(),
                g.available_ports.clone(),
                g.device_reported_ip.clone(),
                g.device_wifi_connected,
                g.discovered_ssids.clone(),
            )
        } else {
            (CompanionConfig::default(), Vec::new(), None, None, Vec::new())
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
                            .child("Initial Device Setup & WiFi Provisioning"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(pal.text_dim)
                            .child("Set up a brand-new LilyGo T-Display-S3 over USB-C Serial (/dev/ttyACM0) or update WiFi & network link settings"),
                    ),
            )
            // Step 1: USB Serial Provisioning & First-Time Setup
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
                                    .child(div().text_color(pal.accent).child(IconName::Usb))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Step 1 · USB-C Serial Link (115200 Baud)"),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("scan-usb-ports")
                                            .small()
                                            .secondary()
                                            .icon(IconName::RefreshCw)
                                            .label("Scan USB Ports")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let found = scan_serial_ports();
                                                if let Ok(mut g) = this.state.lock() {
                                                    if let Some(first) = found.first() {
                                                        g.config.usb_port = first.clone();
                                                    }
                                                    g.available_ports = found.clone();
                                                }
                                                this.status_banner = Some(format!(
                                                    "Detected {} USB serial port(s).",
                                                    found.len()
                                                ));
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("query-usb-btn")
                                            .small()
                                            .secondary()
                                            .icon(IconName::Cable)
                                            .label("Detect Device via USB")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.outbound_queue.push_back(
                                                        OutboundAction::QueryDevice {
                                                            via_usb: true,
                                                        },
                                                    );
                                                }
                                                this.status_banner = Some(
                                                    "Queried T-Display-S3 over USB Serial.".into(),
                                                );
                                                cx.notify();
                                            })),
                                    ),
                            ),
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
                                    .child("Active Port:"),
                            )
                            .children(if ports.is_empty() {
                                vec![
                                    div()
                                        .text_xs()
                                        .text_color(pal.warn)
                                        .child("No /dev/ttyACM* detected — plug in T-Display-S3 via USB-C")
                                        .into_any_element(),
                                ]
                            } else {
                                ports
                                    .into_iter()
                                    .enumerate()
                                    .map(|(idx, port_str)| {
                                        let selected = cfg.usb_port == port_str;
                                        let p_clone = port_str.clone();
                                        Button::new(("usb-port-select", idx))
                                            .small()
                                            .with_variant(if selected {
                                                gpui_kit::component::button::ButtonVariant::Primary
                                            } else {
                                                gpui_kit::component::button::ButtonVariant::Secondary
                                            })
                                            .label(port_str)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.config.usb_port = p_clone.clone();
                                                    save_config(&g.config);
                                                }
                                                cx.notify();
                                            }))
                                            .into_any_element()
                                    })
                                    .collect()
                            }),
                    )
                    .child(
                        h_flex()
                            .justify_between()
                            .items_center()
                            .pt_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(pal.text_dim)
                                    .child("Also stream 1 Hz PC stats & media over USB Serial (useful without WiFi)"),
                            )
                            .child(
                                Switch::new("stream-usb-switch")
                                    .checked(cfg.stream_via_usb)
                                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                                        let val = *checked;
                                        if let Ok(mut g) = this.state.lock() {
                                            g.config.stream_via_usb = val;
                                            save_config(&g.config);
                                        }
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            // Step 2: WiFi Credentials Provisioning Card
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
                                    .child(div().text_color(pal.accent).child(IconName::Wifi))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_bold()
                                            .text_color(pal.text)
                                            .child("Step 2 · Provision 2.4 GHz WiFi Credentials to Device NVS"),
                                    ),
                            )
                            .child(
                                Button::new("refresh-wifi-scan-btn")
                                    .small()
                                    .secondary()
                                    .icon(IconName::RefreshCw)
                                    .label("Scan 2.4 GHz Networks")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let scanned = scan_wifi_networks();
                                        if let Ok(mut g) = this.state.lock() {
                                            g.discovered_ssids = scanned.clone();
                                        }
                                        this.status_banner = Some(format!(
                                            "Discovered {} nearby 2.4 GHz WiFi network(s).",
                                            scanned.len()
                                        ));
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(pal.text_dim)
                            .child("Click a discovered network below to auto-fill the SSID, or type your own credentials:"),
                    )
                    // Discovered WiFi network chips
                    .when(!ssids.is_empty(), |this| {
                        this.child(
                            h_flex()
                                .gap_2()
                                .flex_wrap()
                                .children(ssids.into_iter().enumerate().map(|(idx, item)| {
                                    let ssid_str = item.ssid.clone();
                                    let btn_label = if item.in_use {
                                        format!("★ {} ({}%)", item.ssid, item.signal)
                                    } else {
                                        format!("{} ({}%)", item.ssid, item.signal)
                                    };
                                    Button::new(("wifi-chip", idx))
                                        .small()
                                        .secondary()
                                        .icon(IconName::Wifi)
                                        .label(btn_label)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            let s = ssid_str.clone();
                                            this.ssid_input.update(cx, |input, cx| {
                                                input.set_value(s.clone(), window, cx);
                                            });
                                            if let Ok(mut g) = this.state.lock() {
                                                g.config.wifi_ssid = s;
                                                save_config(&g.config);
                                            }
                                            cx.notify();
                                        }))
                                        .into_any_element()
                                })),
                        )
                    })
                    .child(
                        h_flex()
                            .gap_3()
                            .w_full()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(pal.text_dim)
                                            .child("WiFi SSID"),
                                    )
                                    .child(Input::new(&self.ssid_input).cleanable(true)),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(pal.text_dim)
                                            .child("WiFi Password"),
                                    )
                                    .child(
                                        Input::new(&self.pass_input)
                                            .mask_toggle()
                                            .cleanable(true),
                                    ),
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
                                    .text_color(pal.accent)
                                    .child(match (wifi_connected, reported_ip) {
                                        (Some(true), Some(ip)) => {
                                            format!("Device WiFi Connected · IP: {}", ip)
                                        }
                                        (Some(false), _) => "Device WiFi Disconnected".to_string(),
                                        _ => format!("Current Target IP: {}", cfg.target_ip),
                                    }),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(
                                        Button::new("forget-device-wifi-btn")
                                            .danger()
                                            .small()
                                            .icon(IconName::Trash)
                                            .label("Forget WiFi on Device")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                if let Ok(mut g) = this.state.lock() {
                                                    let force_usb = g.config.stream_via_usb;
                                                    g.config.wifi_ssid = String::new();
                                                    g.config.config_version += 1;
                                                    save_config(&g.config);
                                                    g.outbound_queue.push_back(
                                                        OutboundAction::PushConfig {
                                                            wifi_ssid: None,
                                                            wifi_pass: None,
                                                            forget_wifi: true,
                                                            force_usb,
                                                        },
                                                    );
                                                    g.push_log("Sent 'forget_wifi' command to device.");
                                                }
                                                this.status_banner = Some(
                                                    "Instructed device to forget stored WiFi credentials."
                                                        .into(),
                                                );
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("provision-usb-btn")
                                            .primary()
                                            .icon(IconName::Usb)
                                            .label("Provision via USB Serial (Initial Setup)")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let ssid = this
                                                    .ssid_input
                                                    .read(cx)
                                                    .value()
                                                    .trim()
                                                    .to_string();
                                                let pass =
                                                    this.pass_input.read(cx).value().to_string();
                                                if ssid.is_empty() {
                                                    this.status_banner = Some(
                                                        "Please enter a 2.4 GHz WiFi SSID first."
                                                            .into(),
                                                    );
                                                    cx.notify();
                                                    return;
                                                }
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.config.wifi_ssid = ssid.clone();
                                                    g.config.config_version += 1;
                                                    save_config(&g.config);
                                                    g.outbound_queue.push_back(
                                                        OutboundAction::PushConfig {
                                                            wifi_ssid: Some(ssid.clone()),
                                                            wifi_pass: Some(pass),
                                                            forget_wifi: false,
                                                            force_usb: true,
                                                        },
                                                    );
                                                    g.push_log(format!(
                                                        "Provisioning WiFi '{}' over USB Serial...",
                                                        ssid
                                                    ));
                                                }
                                                this.status_banner = Some(format!(
                                                    "Sent WiFi '{}' + config over USB Serial.",
                                                    ssid
                                                ));
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("provision-udp-btn")
                                            .secondary()
                                            .icon(IconName::Wifi)
                                            .label("Update via WiFi UDP")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                let ssid = this
                                                    .ssid_input
                                                    .read(cx)
                                                    .value()
                                                    .trim()
                                                    .to_string();
                                                let pass =
                                                    this.pass_input.read(cx).value().to_string();
                                                if ssid.is_empty() {
                                                    this.status_banner = Some(
                                                        "Please enter a 2.4 GHz WiFi SSID first."
                                                            .into(),
                                                    );
                                                    cx.notify();
                                                    return;
                                                }
                                                if let Ok(mut g) = this.state.lock() {
                                                    g.config.wifi_ssid = ssid.clone();
                                                    g.config.config_version += 1;
                                                    save_config(&g.config);
                                                    g.outbound_queue.push_back(
                                                        OutboundAction::PushConfig {
                                                            wifi_ssid: Some(ssid.clone()),
                                                            wifi_pass: Some(pass),
                                                            forget_wifi: false,
                                                            force_usb: false,
                                                        },
                                                    );
                                                }
                                                this.status_banner = Some(format!(
                                                    "Sent WiFi '{}' over UDP.",
                                                    ssid
                                                ));
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
            )
            // Step 3: Target WiFi IP Address Card
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
                            .gap_2()
                            .items_center()
                            .child(div().text_color(pal.accent).child(IconName::Radio))
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .text_color(pal.text)
                                    .child("Step 3 · Wireless UDP Target IP (Port 7777)"),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(div().flex_1().child(Input::new(&self.ip_input).cleanable(true)))
                            .child(
                                Button::new("save-target-ip-btn")
                                    .primary()
                                    .icon(IconName::Check)
                                    .label("Save Target IP & Connect")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let new_ip = sanitize_ip_str(
                                            this.ip_input.read(cx).value().trim(),
                                        );
                                        if !new_ip.is_empty() {
                                            if let Ok(mut g) = this.state.lock() {
                                                g.config.target_ip = new_ip.clone();
                                                save_config(&g.config);
                                                g.outbound_queue.push_back(
                                                    OutboundAction::QueryDevice { via_usb: false },
                                                );
                                                g.push_log(format!(
                                                    "Updated target UDP IP to {}:{}",
                                                    new_ip, UDP_PORT
                                                ));
                                            }
                                            this.status_banner = Some(format!(
                                                "Target IP set to {}:{}.",
                                                new_ip, UDP_PORT
                                            ));
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ),
            )
    }
}
