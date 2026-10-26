//! Device page: firmware version and over-the-air updates.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Disableable, StyledExt, h_flex, v_flex};
use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;

use crate::models::{OtaProgress, OutboundAction};
use crate::ui::CompanionWindowView;
use crate::ui::components::{card_header, glass_card, pill, progress_bar};
use crate::ui::theme::HawaPalette;

/// Longer than the device's 8 s read timeout, so a stalled download reports
/// its own error first.
const OTA_SILENCE: std::time::Duration = std::time::Duration::from_secs(20);

impl CompanionWindowView {
    pub(crate) fn render_firmware_card(&mut self, pal: HawaPalette, cx: &mut Context<Self>) -> impl IntoElement {
        let (fw, ota) = self
            .state
            .lock()
            .map(|mut g| {
                // A device that went quiet mid-update rebooted into its old firmware.
                if g.ota.as_ref().is_some_and(|o| o.busy() && o.at.elapsed() > OTA_SILENCE) {
                    g.ota = Some(OtaProgress::new("failed", 0, Some("no_response".into())));
                }
                (g.device_fw.clone(), g.ota.clone())
            })
            .unwrap_or_default();
        let busy = ota.as_ref().is_some_and(OtaProgress::busy);

        let status: Option<AnyElement> = ota.as_ref().map(|o| match o.state.as_str() {
            "failed" => h_flex()
                .gap_2()
                .items_center()
                .child(div().flex_shrink_0().text_color(pal.danger).child(IconName::CircleAlert))
                .child(div().text_sm().text_color(pal.text).child(friendly_error(o.error.as_deref())))
                .into_any_element(),
            "done" => h_flex()
                .gap_2()
                .items_center()
                .child(div().flex_shrink_0().text_color(pal.success).child(IconName::CircleCheck))
                .child(div().text_sm().text_color(pal.text).child("Installed. The device is restarting."))
                .into_any_element(),
            st => v_flex()
                .gap_2()
                .child(
                    h_flex()
                        .justify_between()
                        .child(div().text_sm().text_color(pal.text).child(match st {
                            "verifying" => "Checking the update",
                            "starting" => "Waiting for the device",
                            _ => "Downloading to the device",
                        }))
                        .child(div().text_sm().font_semibold().text_color(pal.accent).child(format!("{}%", o.pct))),
                )
                .child(progress_bar(pal, o.pct as f32 / 100., pal.primary, 6.))
                .into_any_element(),
        });

        glass_card(pal)
            .child(card_header(pal, IconName::HardDriveDownload, "Firmware").child(match &fw {
                Some(v) => pill(format!("v{v} on device"), pal.accent, pal.primary.opacity(0.16)),
                None => pill("Unknown", pal.text_dim, pal.well),
            }))
            .child(div().text_xs().text_color(pal.text_dim).child(
                "Install a firmware.bin built with PlatformIO. The device downloads it over WiFi and checks it before switching.",
            ))
            .children(status)
            .child(
                h_flex().justify_end().child(
                    Button::new("install-firmware")
                        .secondary()
                        .icon(IconName::Upload)
                        .label("Install update...")
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let picked = cx.prompt_for_paths(PathPromptOptions {
                                files: true,
                                directories: false,
                                multiple: false,
                                prompt: Some("Install".into()),
                            });
                            let state = this.state.clone();
                            cx.spawn(async move |_, _| {
                                if let Ok(Ok(Some(paths))) = picked.await {
                                    if let (Some(path), Ok(mut g)) = (paths.into_iter().next(), state.lock()) {
                                        g.ota = Some(OtaProgress::new("starting", 0, None));
                                        g.outbound_queue.push_back(OutboundAction::StartOta { path });
                                    }
                                }
                            })
                            .detach();
                        })),
                ),
            )
    }
}

fn friendly_error(code: Option<&str>) -> String {
    match code.unwrap_or("") {
        "unauthorized" => "The device didn't accept this computer. Run Add device again to re-pair it.".into(),
        "no_response" => "The device stopped answering. It keeps its current firmware; try again.".into(),
        "rejected" => "The device is busy or the request was invalid.".into(),
        "no_wifi" => "The device needs a Wi-Fi network to download updates. Run Add device and pick one.".into(),
        "http_error" | "bad_url" => "The device couldn't download from this computer. Check your firewall (port 7778).".into(),
        "checksum" | "incomplete" | "size_mismatch" => "The download was damaged. Try again.".into(),
        "bad_image" => "That file isn't HawaOS firmware.".into(),
        "no_space" => "That image is too large for the device.".into(),
        "" => "The update failed. Nothing changed on the device.".into(),
        other if other.contains(' ') => other.to_string(), // already a sentence (local errors)
        _ => "The update failed. Nothing changed on the device.".into(),
    }
}
