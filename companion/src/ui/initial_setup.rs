use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{h_flex, v_flex, Sizable, StyledExt};

use crate::config::{save_config, UDP_PORT};
use crate::models::OutboundAction;
use crate::config::{sanitize_ip_str, scan_wifi_networks, CompanionConfig};
use crate::protocol::scan_serial_ports;
use crate::ui::components::{
    button_row, card_header, choice, divider, glass_card, page_header, pill, setting_row,
};
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

        let labeled = |label: &'static str, input: Input| {
            v_flex()
                .flex_1()
                .min_w(px(220.))
                .gap_1()
                .child(div().text_xs().font_medium().text_color(pal.text_dim).child(label))
                .child(input)
        };
        let wifi_status = match (wifi_connected, reported_ip) {
            (Some(true), Some(ip)) => pill(format!("Online · {}", ip), pal.success, pal.success.opacity(0.14)),
            (Some(false), _) => pill("Offline", pal.warn, pal.warn.opacity(0.14)),
            _ => pill("Unknown", pal.text_dim, pal.well),
        };

        v_flex()
            .w_full()
            .gap_4()
            .child(page_header(
                pal,
                "Setup & WiFi",
                "Connect a new device over USB, then give it WiFi",
            ))
            // 1. USB
            .child(
                glass_card(pal)
                    .child(
                        card_header(pal, IconName::Usb, "1 · USB connection").child(
                            button_row()
                                .child(
                                    Button::new("scan-usb-ports")
                                        .small()
                                        .ghost()
                                        .icon(IconName::RefreshCw)
                                        .label("Scan")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            let found = scan_serial_ports();
                                            if let Ok(mut g) = this.state.lock() {
                                                if let Some(first) = found.first() {
                                                    g.config.usb_port = first.clone();
                                                }
                                                g.available_ports = found.clone();
                                            }
                                            this.status_banner =
                                                Some(format!("Found {} USB serial port(s).", found.len()));
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Button::new("query-usb-btn")
                                        .small()
                                        .secondary()
                                        .icon(IconName::Cable)
                                        .label("Detect device")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Ok(mut g) = this.state.lock() {
                                                g.outbound_queue
                                                    .push_back(OutboundAction::QueryDevice { via_usb: true });
                                            }
                                            this.status_banner = Some("Asked the device over USB.".into());
                                            cx.notify();
                                        })),
                                ),
                        ),
                    )
                    .child(if ports.is_empty() {
                        div()
                            .text_sm()
                            .text_color(pal.warn)
                            .child("No serial port found. Plug the device in over USB-C and press Scan.")
                            .into_any_element()
                    } else {
                        button_row()
                            .children(ports.into_iter().enumerate().map(|(idx, port_str)| {
                                let selected = cfg.usb_port == port_str;
                                let p_clone = port_str.clone();
                                Button::new(("usb-port-select", idx))
                                    .small()
                                    .with_variant(choice(pal, selected, cx))
                                    .label(port_str)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            g.config.usb_port = p_clone.clone();
                                            save_config(&g.config);
                                        }
                                        cx.notify();
                                    }))
                            }))
                            .into_any_element()
                    })
                    .child(divider(pal))
                    .child(setting_row(
                        pal,
                        "Also stream over USB",
                        Some("Sends stats and media over the cable too. Useful without WiFi."),
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
                    )),
            )
            // 2. WiFi
            .child(
                glass_card(pal)
                    .child(
                        card_header(pal, IconName::Wifi, "2 · WiFi (2.4 GHz)").child(
                            h_flex()
                                .gap_2()
                                .items_center()
                                .child(wifi_status)
                                .child(
                                    Button::new("refresh-wifi-scan-btn")
                                        .small()
                                        .ghost()
                                        .icon(IconName::RefreshCw)
                                        .label("Scan")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            let scanned = scan_wifi_networks();
                                            if let Ok(mut g) = this.state.lock() {
                                                g.discovered_ssids = scanned.clone();
                                            }
                                            this.status_banner =
                                                Some(format!("Found {} network(s).", scanned.len()));
                                            cx.notify();
                                        })),
                                ),
                        ),
                    )
                    .when(!ssids.is_empty(), |this| {
                        this.child(button_row().children(ssids.into_iter().enumerate().map(|(idx, item)| {
                            let ssid_str = item.ssid.clone();
                            Button::new(("wifi-chip", idx))
                                .small()
                                .with_variant(choice(pal, item.in_use, cx))
                                .icon(IconName::Wifi)
                                .label(format!("{} · {}%", item.ssid, item.signal))
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
                        })))
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .flex_wrap()
                            .gap_3()
                            .child(labeled("Network name", Input::new(&self.ssid_input).cleanable(true)))
                            .child(labeled(
                                "Password",
                                Input::new(&self.pass_input).mask_toggle().cleanable(true),
                            )),
                    )
                    .child(
                        button_row()
                            .justify_end()
                            .child(
                                Button::new("forget-device-wifi-btn")
                                    .ghost()
                                    .icon(IconName::Trash)
                                    .label("Forget")
                                    .tooltip("Make the device forget its saved WiFi")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Ok(mut g) = this.state.lock() {
                                            let force_usb = g.config.stream_via_usb;
                                            g.config.wifi_ssid = String::new();
                                            g.config.config_version += 1;
                                            save_config(&g.config);
                                            g.outbound_queue.push_back(OutboundAction::PushConfig {
                                                wifi_ssid: None,
                                                wifi_pass: None,
                                                forget_wifi: true,
                                                force_usb,
                                            });
                                            g.push_log("Sent 'forget_wifi' command to device.");
                                        }
                                        this.status_banner = Some("Device told to forget its WiFi.".into());
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("provision-udp-btn")
                                    .secondary()
                                    .icon(IconName::Wifi)
                                    .label("Send over WiFi")
                                    .on_click(cx.listener(|this, _, _, cx| this.send_wifi_credentials(false, cx))),
                            )
                            .child(
                                Button::new("provision-usb-btn")
                                    .primary()
                                    .icon(IconName::Usb)
                                    .label("Send over USB")
                                    .on_click(cx.listener(|this, _, _, cx| this.send_wifi_credentials(true, cx))),
                            ),
                    ),
            )
            // 3. Device address
            .child(
                glass_card(pal)
                    .child(card_header(pal, IconName::Radio, "3 · Device address"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(pal.text_dim)
                            .child(format!("The companion sends data to this IP on UDP port {}.", UDP_PORT)),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(div().flex_1().min_w_0().child(Input::new(&self.ip_input).cleanable(true)))
                            .child(
                                Button::new("save-target-ip-btn")
                                    .primary()
                                    .icon(IconName::Check)
                                    .label("Connect")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let new_ip = sanitize_ip_str(this.ip_input.read(cx).value().trim());
                                        if new_ip.is_empty() {
                                            return;
                                        }
                                        if let Ok(mut g) = this.state.lock() {
                                            g.config.target_ip = new_ip.clone();
                                            save_config(&g.config);
                                            g.outbound_queue
                                                .push_back(OutboundAction::QueryDevice { via_usb: false });
                                            g.push_log(format!("Updated target UDP IP to {}:{}", new_ip, UDP_PORT));
                                        }
                                        this.status_banner = Some(format!("Now sending to {}.", new_ip));
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }

    /// Push the SSID/password from the inputs to the device, over USB or WiFi.
    fn send_wifi_credentials(&mut self, via_usb: bool, cx: &mut Context<Self>) {
        let ssid = self.ssid_input.read(cx).value().trim().to_string();
        let pass = self.pass_input.read(cx).value().to_string();
        if ssid.is_empty() {
            self.status_banner = Some("Enter a WiFi network name first.".into());
            cx.notify();
            return;
        }
        if let Ok(mut g) = self.state.lock() {
            g.config.wifi_ssid = ssid.clone();
            g.config.config_version += 1;
            save_config(&g.config);
            g.outbound_queue.push_back(OutboundAction::PushConfig {
                wifi_ssid: Some(ssid.clone()),
                wifi_pass: Some(pass),
                forget_wifi: false,
                force_usb: via_usb,
            });
            if via_usb {
                g.push_log(format!("Provisioning WiFi '{}' over USB Serial...", ssid));
            }
        }
        self.status_banner = Some(format!(
            "Sent WiFi '{}' over {}.",
            ssid,
            if via_usb { "USB" } else { "WiFi" }
        ));
        cx.notify();
    }
}
