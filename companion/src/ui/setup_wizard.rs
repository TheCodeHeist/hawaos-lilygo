//! "Add device" wizard: Connect -> (Pair) -> Wi-Fi -> Personalize -> Done.
//! The backend lives in `crate::setup`; this file only renders its state.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{Disableable, Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;

use crate::setup::link::Endpoint;
use crate::setup::{Cmd, FoundDevice, Method, Net, Step, WizardState};
use crate::ui::accents::ACCENTS;
use crate::ui::components::{
    accent_swatch, button_row, card_row, choice, divider, glass_card, page_header, pill, row_card,
};
use crate::ui::theme::HawaPalette;
use crate::ui::{CompanionWindowView, NavSection};

const WIZARD_MAX_W: f32 = 760.;

impl CompanionWindowView {
    pub(crate) fn render_setup_wizard(
        &mut self,
        pal: HawaPalette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let w = self.wizard.snapshot();
        let body = match w.step {
            Step::Welcome => self.wizard_welcome(pal, cx).into_any_element(),
            Step::Searching => self.wizard_searching(pal, &w, cx).into_any_element(),
            Step::Connecting => busy_card(
                pal,
                format!("Connecting to {}", w.device.as_ref().map(|d| d.name.as_str()).unwrap_or("HawaOS")),
                "This only takes a moment.",
            )
            .into_any_element(),
            Step::Pairing => self.wizard_pairing(pal, &w, cx).into_any_element(),
            Step::Wifi => self.wizard_wifi(pal, &w, cx).into_any_element(),
            Step::Joining => busy_card(
                pal,
                format!("Joining {}", w.joining_ssid),
                "Your HawaOS is connecting. This can take up to 20 seconds.",
            )
            .into_any_element(),
            Step::Personalize => self.wizard_personalize(pal, &w, cx).into_any_element(),
            Step::Done => self.wizard_done(pal, &w, cx).into_any_element(),
        };

        v_flex()
            .w_full()
            .gap_4()
            .child(page_header(pal, "Add a device", "Set up a HawaOS display in a few steps"))
            .when(w.step != Step::Welcome, |this| this.child(stepper(pal, &w)))
            .child(div().w_full().max_w(px(WIZARD_MAX_W)).child(body))
            .when(w.step == Step::Welcome, |this| {
                this.child(
                    h_flex().child(
                        Button::new("toggle-manual")
                            .ghost()
                            .small()
                            .icon(if self.show_manual_setup { IconName::ChevronUp } else { IconName::ChevronDown })
                            .label("Manual connection")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.show_manual_setup = !this.show_manual_setup;
                                cx.notify();
                            })),
                    ),
                )
                .when(self.show_manual_setup, |this| this.child(self.render_initial_setup(pal, cx)))
            })
    }

    fn wizard_welcome(&mut self, pal: HawaPalette, cx: &mut Context<Self>) -> impl IntoElement {
        let method_card = |id: &'static str,
                           icon: IconName,
                           title: &'static str,
                           desc: &'static str,
                           badge: Option<&'static str>,
                           method: Method,
                           cx: &mut Context<Self>| {
            row_card(pal, 240.)
                .id(id)
                .cursor_pointer()
                .hover(|s| s.bg(pal.glass_strong))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.wizard.send(Cmd::Search(method));
                    cx.notify();
                }))
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(icon_tile(pal, icon))
                        .when_some(badge, |this, b| {
                            this.child(pill(b, pal.accent, pal.primary.opacity(0.16)))
                        }),
                )
                .child(div().text_base().font_semibold().text_color(pal.text).child(title))
                .child(div().text_sm().text_color(pal.text_dim).child(desc))
        };

        glass_card(pal)
            .gap_5()
            .child(
                h_flex()
                    .gap_4()
                    .items_center()
                    // The artwork is already rounded: never clip it.
                    .child(img(super::logo(pal.accent)).size(px(56.)).flex_shrink_0())
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_1()
                            .min_w_0()
                            .child(div().text_lg().font_semibold().text_color(pal.text).child("Let's meet your HawaOS"))
                            .child(div().text_sm().text_color(pal.text_dim).child(
                                "Turn it on. A new device shows a Setup screen with a pairing code. How is it connected?",
                            )),
                    ),
            )
            .child(
                card_row()
                    .child(method_card(
                        "method-usb",
                        IconName::Usb,
                        "USB cable",
                        "Fastest. Plug it into this computer with a USB-C data cable.",
                        Some("Recommended"),
                        Method::Usb,
                        cx,
                    ))
                    .child(method_card(
                        "method-ble",
                        IconName::Bluetooth,
                        "Bluetooth",
                        "No cable needed. Keep the device within a few meters.",
                        None,
                        Method::Ble,
                        cx,
                    )),
            )
    }

    fn wizard_searching(&mut self, pal: HawaPalette, w: &WizardState, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, hint) = match w.method {
            Method::Usb => ("Looking for devices on USB", "Plug in your HawaOS with a USB-C data cable."),
            Method::Ble => ("Looking for devices nearby", "Make sure the device is on and showing its Setup screen."),
        };
        glass_card(pal)
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(if w.error.is_none() {
                        Spinner::new().color(pal.accent.into()).into_any_element()
                    } else {
                        div().text_color(pal.danger).child(IconName::CircleAlert).into_any_element()
                    })
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_base().font_semibold().text_color(pal.text).child(title))
                            .child(div().text_sm().text_color(pal.text_dim).child(hint)),
                    ),
            )
            .when_some(w.error.clone(), |this, e| this.child(error_line(pal, e)))
            .when(!w.found.is_empty(), |this| {
                this.child(divider(pal)).children(
                    w.found
                        .iter()
                        .enumerate()
                        .map(|(i, d)| device_row(pal, i, d, cx.listener(move |this, _, _, cx| {
                            this.wizard.send(Cmd::Choose(i));
                            cx.notify();
                        }))),
                )
            })
            .child(footer().child(back_button(cx)))
    }

    fn wizard_pairing(&mut self, pal: HawaPalette, w: &WizardState, cx: &mut Context<Self>) -> impl IntoElement {
        let code: String = self.code_input.read(cx).value().chars().filter(char::is_ascii_digit).collect();
        let ready = code.len() == 6 && !w.busy;
        glass_card(pal)
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(icon_tile(pal, IconName::ShieldCheck))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_base().font_semibold().text_color(pal.text).child("Enter the pairing code"))
                            .child(div().text_sm().text_color(pal.text_dim).child(
                                "Type the 6 digits shown on the device. They prove you are next to it, and encrypt your Wi-Fi password.",
                            )),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(div().w(px(220.)).child(Input::new(&self.code_input).large()))
                    .child(
                        Button::new("pair-btn")
                            .primary()
                            .large()
                            .loading(w.busy)
                            .disabled(!ready)
                            .label("Pair")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let code: String =
                                    this.code_input.read(cx).value().chars().filter(char::is_ascii_digit).collect();
                                this.wizard.send(Cmd::SubmitCode(code));
                                cx.notify();
                            })),
                    ),
            )
            .when_some(w.error.clone(), |this, e| this.child(error_line(pal, e)))
            .child(footer().child(back_button(cx)))
    }

    fn wizard_wifi(&mut self, pal: HawaPalette, w: &WizardState, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.wizard_ssid.clone();
        let manual = self.wizard_manual_ssid;
        let chosen_secure = selected
            .as_ref()
            .and_then(|s| w.nets.iter().find(|n| &n.ssid == s))
            .map(|n| n.secure)
            .unwrap_or(true);
        let ssid_value = if manual {
            self.ssid_input.read(cx).value().trim().to_string()
        } else {
            selected.clone().unwrap_or_default()
        };
        let can_join = !ssid_value.is_empty();
        let usb = w.method == Method::Usb;

        glass_card(pal)
            .gap_4()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .gap_3()
                    .child(
                        h_flex()
                            .flex_1()
                            .gap_3()
                            .items_center()
                            .min_w_0()
                            .child(icon_tile(pal, IconName::Wifi))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().text_base().font_semibold().text_color(pal.text).child("Choose a Wi-Fi network"))
                                    .child(div().text_sm().text_color(pal.text_dim).child("Networks your HawaOS can see (2.4 GHz).")),
                            ),
                    )
                    .child(
                        Button::new("rescan-nets")
                            .ghost()
                            .small()
                            .loading(w.scanning_nets)
                            .icon(IconName::RefreshCw)
                            .label("Rescan")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.wizard.send(Cmd::ScanNets);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .children(w.nets.iter().enumerate().map(|(i, n)| {
                        let is_sel = !manual && selected.as_deref() == Some(n.ssid.as_str());
                        let ssid = n.ssid.clone();
                        net_row(pal, i, n, is_sel).on_click(cx.listener(move |this, _, _, cx| {
                            this.wizard_ssid = Some(ssid.clone());
                            this.wizard_manual_ssid = false;
                            cx.notify();
                        }))
                    }))
                    .when(w.nets.is_empty() && w.scanning_nets, |this| {
                        this.child(div().text_sm().text_color(pal.text_dim).py_2().child("Scanning..."))
                    })
                    .child(
                        list_row(pal, "net-other", manual)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.wizard_manual_ssid = true;
                                this.wizard_ssid = None;
                                cx.notify();
                            }))
                            .child(div().text_color(pal.text_dim).child(IconName::Plus))
                            .child(div().text_sm().text_color(pal.text).child("Other network")),
                    ),
            )
            .when(manual, |this| {
                this.child(labeled(pal, "Network name", Input::new(&self.ssid_input).cleanable(true)))
            })
            .when(manual || (selected.is_some() && chosen_secure), |this| {
                this.child(labeled(pal, "Password", Input::new(&self.pass_input).mask_toggle()))
            })
            .when_some(w.error.clone(), |this, e| this.child(error_line(pal, e)))
            .child(
                footer()
                    .child(back_button(cx))
                    .child(
                        button_row()
                            .child(
                                Button::new("use-ble")
                                    .ghost()
                                    .icon(IconName::Bluetooth)
                                    .label("Use Bluetooth")
                                    .tooltip("No network needed; Wi-Fi only for firmware updates")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.wizard.send(Cmd::UseBluetooth);
                                        cx.notify();
                                    })),
                            )
                            .when(usb, |this| {
                                this.child(
                                    Button::new("skip-wifi")
                                        .ghost()
                                        .label("Skip, use USB only")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.wizard.send(Cmd::SkipWifi);
                                            cx.notify();
                                        })),
                                )
                            })
                            .child(
                                Button::new("join-wifi")
                                    .primary()
                                    .disabled(!can_join)
                                    .label("Connect")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let pass = this.pass_input.read(cx).value().to_string();
                                        this.wizard.send(Cmd::Join { ssid: ssid_value.clone(), pass });
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
    }

    fn wizard_personalize(&mut self, pal: HawaPalette, _w: &WizardState, cx: &mut Context<Self>) -> impl IntoElement {
        let (accent, theme, use_24h) = self
            .state
            .lock()
            .map(|g| (g.config.accent.clone(), g.config.device_theme.clone(), g.config.use_24h))
            .unwrap_or_default();
        let send = move |this: &mut Self, accent: String, theme: String, use_24h: bool| {
            this.wizard.send(Cmd::Personalize { accent, theme, use_24h });
        };

        glass_card(pal)
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .items_center()
                    .child(icon_tile(pal, IconName::Palette))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_base().font_semibold().text_color(pal.text).child("Make it yours"))
                            .child(div().text_sm().text_color(pal.text_dim).child("Changes show up on the device right away.")),
                    ),
            )
            .child(section_label(pal, "Accent"))
            .child(h_flex().flex_wrap().gap(px(6.)).children(ACCENTS.iter().map(|a| {
                let (t, h) = (theme.clone(), use_24h);
                accent_swatch(pal, a, a.id == accent).on_click(cx.listener(move |this, _, _, cx| {
                    send(this, a.id.to_string(), t.clone(), h);
                    cx.notify();
                }))
            })))
            .child(divider(pal))
            .child(
                card_row()
                    .child(
                        v_flex()
                            .gap_2()
                            .child(section_label(pal, "Theme"))
                            .child(button_row().children(["dark", "light"].into_iter().map(|t| {
                                let (a, h) = (accent.clone(), use_24h);
                                Button::new(SharedString::from(format!("theme-{t}")))
                                    .small()
                                    .with_variant(choice(pal, theme == t, cx))
                                    .icon(if t == "dark" { IconName::Moon } else { IconName::Sun })
                                    .label(if t == "dark" { "Dark" } else { "Light" })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        send(this, a.clone(), t.to_string(), h);
                                        cx.notify();
                                    }))
                            }))),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(section_label(pal, "Clock"))
                            .child(button_row().children([false, true].into_iter().map(|h24| {
                                let (a, t) = (accent.clone(), theme.clone());
                                Button::new(SharedString::from(format!("clock-{h24}")))
                                    .small()
                                    .with_variant(choice(pal, use_24h == h24, cx))
                                    .label(if h24 { "24-hour" } else { "12-hour" })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        send(this, a.clone(), t.clone(), h24);
                                        cx.notify();
                                    }))
                            }))),
                    ),
            )
            .child(
                footer().child(div()).child(
                    Button::new("finish-setup")
                        .primary()
                        .icon(IconName::Check)
                        .label("Finish")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.wizard.send(Cmd::Finish);
                            cx.notify();
                        })),
                ),
            )
    }

    fn wizard_done(&mut self, pal: HawaPalette, w: &WizardState, cx: &mut Context<Self>) -> impl IntoElement {
        let name = w.device.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| "Your HawaOS".into());
        let detail = match (&w.device_ip, w.usb_only) {
            _ if w.use_ble => format!("{name} will talk to this computer over Bluetooth."),
            (Some(ip), false) => format!("{name} is online at {ip}."),
            _ => format!("{name} is connected over USB."),
        };
        glass_card(pal)
            .gap_4()
            .items_center()
            .py_8()
            .child(
                div()
                    .size(px(64.))
                    .rounded_full()
                    .bg(pal.success.opacity(0.16))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(pal.success)
                    .text_2xl()
                    .child(IconName::Check),
            )
            .child(div().text_xl().font_bold().text_color(pal.text).child("You're all set"))
            .child(div().text_sm().text_color(pal.text_dim).child(detail))
            .child(
                button_row()
                    .child(
                        Button::new("setup-another")
                            .ghost()
                            .label("Add another device")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.wizard.send(Cmd::Cancel);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("open-dashboard")
                            .primary()
                            .icon(IconName::LayoutDashboard)
                            .label("Open dashboard")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.wizard.send(Cmd::Cancel);
                                this.active_section = NavSection::Overview;
                                cx.notify();
                            })),
                    ),
            )
    }
}

/// The page's stepper: done steps get a check, the current one a ring.
fn stepper(pal: HawaPalette, w: &WizardState) -> impl IntoElement {
    let ble = w.method == Method::Ble;
    let steps: &[(&str, usize)] = if ble {
        &[("Connect", 0), ("Pair", 1), ("Wi-Fi", 2), ("Personalize", 3), ("Done", 4)]
    } else {
        &[("Connect", 0), ("Wi-Fi", 2), ("Personalize", 3), ("Done", 4)]
    };
    let current = w.step.stage();
    h_flex()
        .w_full()
        .max_w(px(WIZARD_MAX_W))
        .gap_2()
        .items_center()
        .children(steps.iter().enumerate().flat_map(|(i, (label, stage))| {
            let done = *stage < current || w.step == Step::Done;
            let active = *stage == current && w.step != Step::Done;
            let dot = div()
                .size(px(24.))
                .flex_shrink_0()
                .rounded_full()
                .flex()
                .items_center()
                .justify_center()
                .text_xs()
                .font_semibold()
                .when(done, |d| d.bg(pal.primary).text_color(pal.on_primary).child(IconName::Check))
                .when(active, |d| {
                    d.border_2().border_color(pal.accent).text_color(pal.accent).child(format!("{}", i + 1))
                })
                .when(!done && !active, |d| d.bg(pal.well).text_color(pal.text_muted).child(format!("{}", i + 1)));
            let item = h_flex()
                .gap_2()
                .items_center()
                .flex_shrink_0()
                .child(dot)
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_color(if active { pal.text } else { pal.text_dim })
                        .child(*label),
                );
            let line = (i + 1 < steps.len()).then(|| {
                div()
                    .flex_1()
                    .min_w(px(12.))
                    .h(px(2.))
                    .rounded_full()
                    .bg(if done { pal.primary.opacity(0.6) } else { pal.well })
            });
            std::iter::once(item.into_any_element()).chain(line.map(|l| l.into_any_element()))
        }))
}

fn busy_card(pal: HawaPalette, title: String, desc: &'static str) -> Div {
    glass_card(pal).py_8().items_center().gap_3().child(Spinner::new().large().color(pal.accent.into())).child(
        v_flex()
            .items_center()
            .gap_1()
            .child(div().text_base().font_semibold().text_color(pal.text).child(title))
            .child(div().text_sm().text_color(pal.text_dim).child(desc)),
    )
}

fn icon_tile(pal: HawaPalette, icon: IconName) -> Div {
    div()
        .size(px(40.))
        .flex_shrink_0()
        .rounded(px(12.))
        .bg(pal.primary.opacity(0.16))
        .text_color(pal.accent)
        .flex()
        .items_center()
        .justify_center()
        .child(icon)
}

fn section_label(pal: HawaPalette, text: &'static str) -> Div {
    div().text_xs().font_semibold().text_color(pal.text_dim).child(text)
}

fn error_line(pal: HawaPalette, msg: String) -> Div {
    h_flex()
        .gap_2()
        .items_center()
        .px_3()
        .py_2()
        .rounded(px(10.))
        .bg(pal.danger.opacity(0.12))
        .child(div().flex_shrink_0().text_color(pal.danger).child(IconName::CircleAlert))
        .child(div().min_w_0().text_sm().text_color(pal.text).child(msg))
}

fn labeled(pal: HawaPalette, label: &'static str, input: Input) -> Div {
    v_flex().gap_1().max_w(px(420.)).child(section_label(pal, label)).child(input)
}

fn footer() -> Div {
    h_flex().w_full().justify_between().items_center().gap_2().pt_1()
}

fn back_button(cx: &mut Context<CompanionWindowView>) -> Button {
    Button::new("wizard-back")
        .ghost()
        .icon(IconName::ArrowLeft)
        .label("Back")
        .on_click(cx.listener(|this, _, _, cx| {
            this.wizard.send(Cmd::Cancel);
            cx.notify();
        }))
}

/// A selectable row in a list (networks, "Other network").
fn list_row(pal: HawaPalette, id: impl Into<ElementId>, selected: bool) -> Stateful<Div> {
    h_flex()
        .id(id)
        .w_full()
        .gap_3()
        .items_center()
        .px_3()
        .py_2()
        .rounded(px(10.))
        .cursor_pointer()
        .when(selected, |r| r.bg(pal.primary.opacity(0.16)))
        .when(!selected, |r| r.hover(|s| s.bg(pal.glass_strong)))
}

fn net_row(pal: HawaPalette, i: usize, n: &Net, selected: bool) -> Stateful<Div> {
    list_row(pal, ("net", i), selected)
        .child(signal_bars(pal, n.rssi, selected))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_sm()
                .font_medium()
                .text_color(pal.text)
                .child(n.ssid.clone()),
        )
        .when(n.secure, |r| r.child(div().text_color(pal.text_dim).text_sm().child(IconName::Lock)))
}

/// Four rising bars, filled by signal strength.
fn signal_bars(pal: HawaPalette, rssi: i32, strong_color: bool) -> Div {
    let level = match rssi {
        r if r >= -55 => 4,
        r if r >= -67 => 3,
        r if r >= -78 => 2,
        _ => 1,
    };
    h_flex().gap(px(2.)).items_end().h(px(14.)).flex_shrink_0().children((0..4).map(|b| {
        div()
            .w(px(3.))
            .h(px(5. + b as f32 * 3.))
            .rounded(px(1.))
            .bg(if b < level {
                if strong_color { pal.accent } else { pal.text_dim }
            } else {
                pal.well
            })
    }))
}

fn device_row(
    pal: HawaPalette,
    i: usize,
    d: &FoundDevice,
    on_choose: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let mut detail = Vec::new();
    match &d.endpoint {
        Endpoint::Usb(port) => detail.push(port.clone()),
        Endpoint::Ble(_) => detail.push("Bluetooth".into()),
    }
    if let Some(fw) = &d.fw {
        detail.push(format!("v{fw}"));
    }
    let status = match d.needs_setup {
        Some(true) => Some(pill("New", pal.accent, pal.primary.opacity(0.16))),
        Some(false) => Some(pill("Set up", pal.text_dim, pal.well)),
        None => None,
    };
    h_flex()
        .w_full()
        .gap_3()
        .items_center()
        .child(icon_tile(pal, IconName::Monitor))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().truncate().text_sm().font_semibold().text_color(pal.text).child(d.name.clone()))
                        .children(status),
                )
                .child(div().truncate().text_xs().text_color(pal.text_dim).child(detail.join(" · "))),
        )
        .when_some(d.rssi, |r, rssi| r.child(signal_bars(pal, rssi as i32, false)))
        .child(
            Button::new(("choose-device", i))
                .primary()
                .small()
                .label("Set up")
                .on_click(on_choose),
        )
}
