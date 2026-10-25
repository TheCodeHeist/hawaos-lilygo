//! Shared glass building blocks so every page is laid out the same way.
//!
//! Responsiveness rules used across pages:
//! - Cards sit in a `card_row`, which wraps; each card has a min width, so a
//!   narrow window turns a row of cards into a column instead of overflowing.
//! - Text columns are `min_w_0` so they can shrink; long single-line values
//!   (titles, IPs, paths) are `truncate`d, descriptions wrap.
//! - Controls (`setting_row`) never shrink; their label column does.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{ButtonCustomVariant, ButtonVariant};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex, StyledExt};
use gpui_kit::gpui::prelude::*;
use gpui_kit::gpui::*;

use crate::ui::accents::Accent;
use crate::ui::theme::HawaPalette;

pub const CARD_RADIUS: f32 = 16.;

/// Frosted glass panel.
pub fn glass_card(pal: HawaPalette) -> Div {
    v_flex()
        .min_w_0()
        .p_4()
        .gap_3()
        .rounded(px(CARD_RADIUS))
        .bg(pal.glass)
        .border_1()
        .border_color(pal.glass_border)
        .when(!pal.dark, |this| this.shadow_sm())
}

/// A glass card that takes an equal share of a `card_row`, wrapping below `min_w`.
pub fn row_card(pal: HawaPalette, min_w: f32) -> Div {
    glass_card(pal).flex_1().min_w(px(min_w))
}

/// Horizontal group of cards that wraps into a column when space runs out.
pub fn card_row() -> Div {
    h_flex().w_full().flex_wrap().items_stretch().gap_4()
}

/// Card title row: icon + title on the left; append a trailing element with `.child()`.
pub fn card_header(pal: HawaPalette, icon: IconName, title: impl Into<SharedString>) -> Div {
    h_flex()
        .w_full()
        .gap_2()
        .justify_between()
        .items_center()
        .child(
            h_flex()
                .gap_2()
                .min_w_0()
                .items_center()
                .child(div().flex_shrink_0().text_color(pal.accent).child(icon))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_sm()
                        .font_semibold()
                        .text_color(pal.text)
                        .child(title.into()),
                ),
        )
}

/// Page title + one-line subtitle; append action buttons with `.child()`.
/// Actions wrap below the title on narrow windows.
pub fn page_header(
    pal: HawaPalette,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
) -> Div {
    h_flex()
        .w_full()
        .flex_wrap()
        .gap_3()
        .justify_between()
        .items_center()
        .child(
            v_flex()
                .gap_0p5()
                .min_w(px(220.))
                .flex_1()
                .child(
                    div()
                        .text_2xl()
                        .font_bold()
                        .text_color(pal.text)
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(pal.text_dim)
                        .child(subtitle.into()),
                ),
        )
}

/// Label (+ optional wrapping description) on the left, a fixed control on the right.
pub fn setting_row(
    pal: HawaPalette,
    title: impl Into<SharedString>,
    desc: Option<&'static str>,
    control: impl IntoElement,
) -> Div {
    h_flex()
        .w_full()
        .gap_4()
        .justify_between()
        .items_center()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_color(pal.text)
                        .child(title.into()),
                )
                .when_some(desc, |this, d| {
                    this.child(div().text_xs().text_color(pal.text_dim).child(d))
                }),
        )
        .child(div().flex_shrink_0().child(control))
}

/// Hairline separator between rows inside a card.
pub fn divider(pal: HawaPalette) -> Div {
    div().w_full().h(px(1.)).bg(pal.glass_border)
}

/// Small rounded status chip.
pub fn pill(text: impl Into<SharedString>, fg: Rgba, bg: impl Into<Hsla>) -> Div {
    div()
        .flex_shrink_0()
        .px_2()
        .py_0p5()
        .rounded_full()
        .bg(Into::<Hsla>::into(bg))
        .text_xs()
        .font_semibold()
        .text_color(fg)
        .child(text.into())
}

/// Pill for running / paused / stopped states.
pub fn state_pill(pal: HawaPalette, state: &str) -> Div {
    let (fg, bg): (Rgba, Hsla) = match state {
        "running" | "playing" => (pal.accent, pal.primary.opacity(0.18).into()),
        "paused" => (pal.warn, pal.warn.opacity(0.16).into()),
        _ => (pal.text_dim, pal.well.into()),
    };
    let mut label = state.to_string();
    if let Some(first) = label.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    pill(label, fg, bg)
}

/// Thin rounded progress bar; `frac` is clamped to 0..=1.
pub fn progress_bar(pal: HawaPalette, frac: f32, color: impl Into<Hsla>, height: f32) -> Div {
    div()
        .w_full()
        .h(px(height))
        .rounded_full()
        .bg(pal.well)
        .overflow_hidden()
        .child(
            div()
                .h_full()
                .w(relative(frac.clamp(0.0, 1.0)))
                .rounded_full()
                .bg(Into::<Hsla>::into(color)),
        )
}

/// Large `HH:MM:SS` with a smaller `.mmm` on the same baseline.
pub fn time_readout(pal: HawaPalette, ms_total: u64) -> Div {
    let total_s = ms_total / 1000;
    h_flex()
        .items_baseline()
        .gap_0p5()
        .child(
            div()
                .text_3xl()
                .font_bold()
                .text_color(pal.text)
                .child(format!(
                    "{:02}:{:02}:{:02}",
                    total_s / 3600,
                    (total_s / 60) % 60,
                    total_s % 60
                )),
        )
        .child(
            div()
                .text_sm()
                .font_semibold()
                .text_color(pal.text_dim)
                .child(format!(".{:03}", ms_total % 1000)),
        )
}

/// Variant for one option of a mutually exclusive button group. The selected
/// option is a tinted chip (like the sidebar's active item) rather than a solid
/// primary fill, so solid fills stay reserved for real actions such as Apply.
pub fn choice(pal: HawaPalette, selected: bool, cx: &App) -> ButtonVariant {
    if selected {
        ButtonVariant::Custom(
            ButtonCustomVariant::new(cx)
                .color(pal.primary.opacity(0.18).into())
                .foreground(pal.accent.into())
                .hover(pal.primary.opacity(0.26).into())
                .active(pal.primary.opacity(0.32).into()),
        )
    } else {
        ButtonVariant::Secondary
    }
}

pub const SWATCH_SIZE: f32 = 36.;

/// Round accent swatch: the accent's 500 shade sweeping into its deeper 700
/// shade from top-left to bottom-right. The selected one gets a ring in its own
/// color (with a gap, so it reads on any backdrop) and a check mark.
pub fn accent_swatch(pal: HawaPalette, accent: &'static Accent, selected: bool) -> Stateful<Div> {
    let (from, to) = accent.swatch;
    div()
        .id(accent.id)
        .flex_shrink_0()
        .size(px(SWATCH_SIZE))
        .p(px(3.))
        .rounded_full()
        .border_2()
        .border_color(if selected { rgb(from) } else { rgba(0x00000000) })
        .cursor_pointer()
        .when(!selected, |this| this.hover(|s| s.border_color(pal.glass_border)))
        .tooltip(move |window, cx| Tooltip::new(accent.label).build(window, cx))
        .child(
            div()
                .size_full()
                .rounded_full()
                .bg(linear_gradient(
                    135.,
                    linear_color_stop(rgb(from), 0.),
                    linear_color_stop(rgb(to), 1.),
                ))
                // Soft light rim gives the disc a little depth on dark glass.
                .border_1()
                .border_color(rgba(0xffffff2e))
                .shadow_sm()
                .flex()
                .items_center()
                .justify_center()
                .when(selected, |this| {
                    // Light-mode ink is picked against the 600 shade, i.e. the
                    // middle of this gradient, so the check always reads.
                    this.child(
                        div()
                            .text_sm()
                            .text_color(rgb(accent.light.on_primary))
                            .child(IconName::Check),
                    )
                }),
        )
}

/// Wrapping row for a group of buttons.
pub fn button_row() -> Div {
    h_flex().flex_wrap().gap_2().items_center()
}

/// Media tint sent for tracks without artwork (see `MediaMsg::default`).
pub const NO_ART_TINT: &str = "#008DA6";

/// Parse `#RRGGBB` (as sent in media tints) into a color.
pub fn parse_hex(hex: &str) -> Option<Rgba> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    u32::from_str_radix(h, 16).ok().map(rgb)
}
