//! Device page: how the companion reaches the device day to day.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Disableable, Sizable};
use gpui_kit::gpui::prelude::*;

use crate::models::OutboundAction;
use crate::ui::CompanionWindowView;
use crate::ui::components::{button_row, card_header, choice, glass_card, pill, setting_row};
use crate::ui::theme::HawaPalette;

impl CompanionWindowView {
    pub(crate) fn render_connection_card(&mut self, pal: HawaPalette, cx: &mut Context<Self>) -> impl IntoElement {
        let (use_ble, paired, ble_up, recent_ack, has_network) = self
            .state
            .lock()
            .map(|g| {
                let recent = g.last_ack_instant.is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(120));
                let net = g.device_has_network != Some(false);
                (g.config.use_ble, !g.config.device_token.is_empty(), g.ble_connected, recent, net)
            })
            .unwrap_or_default();
        // The switch travels over the current link, so it needs the device.
        let reachable = if use_ble { ble_up } else { recent_ack };

        let status = if use_ble {
            if ble_up {
                pill("Linked", pal.accent, pal.primary.opacity(0.16))
            } else {
                pill("Searching", pal.text_dim, pal.well)
            }
        } else {
            pill("Wi-Fi", pal.text_dim, pal.well)
        };

        glass_card(pal)
            .child(card_header(pal, IconName::Radio, "Connection").child(status))
            .child(setting_row(
                pal,
                "Link",
                Some("Bluetooth needs no network; Wi-Fi turns on only for firmware updates."),
                button_row()
                    .child(
                        Button::new("link-wifi")
                            .small()
                            .with_variant(choice(pal, !use_ble, cx))
                            .icon(IconName::Wifi)
                            .label("Wi-Fi")
                            // Without a saved network the device would only be
                            // reachable over USB; it refuses that switch too.
                            .disabled(!reachable || (use_ble && !has_network))
                            .tooltip("Needs a Wi-Fi network saved on the device (Add device)")
                            .on_click(cx.listener(|this, _, _, cx| this.switch_link(false, cx))),
                    )
                    .child(
                        Button::new("link-ble")
                            .small()
                            .with_variant(choice(pal, use_ble, cx))
                            .icon(IconName::Bluetooth)
                            .label("Bluetooth")
                            .disabled(!paired || !reachable)
                            .tooltip("Needs a device added with Add device")
                            .on_click(cx.listener(|this, _, _, cx| this.switch_link(true, cx))),
                    ),
            ))
    }

    fn switch_link(&mut self, ble: bool, cx: &mut Context<Self>) {
        if let Ok(mut g) = self.state.lock() {
            g.outbound_queue.push_back(OutboundAction::SetLink { ble, since: std::time::Instant::now() });
        }
        self.status_banner = Some(if ble {
            "Switching to Bluetooth. The device turns Wi-Fi off and waits for this computer.".into()
        } else {
            "Switching to Wi-Fi. The device rejoins its saved network.".into()
        });
        cx.notify();
    }
}
