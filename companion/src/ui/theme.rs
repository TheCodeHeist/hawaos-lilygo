use gpui_kit::gpui::{px, rgb, rgba, App, Rgba, WindowBackgroundAppearance};
use gpui_kit::component::theme::{Theme, ThemeMode as GpuiThemeMode};

/// Glass design tokens for the companion window.
///
/// The window is created with a blurred background (real compositor blur on
/// KDE/macOS/Windows), so the root paints a *translucent* tinted gradient
/// (`bg_top` -> `bg_bottom`) and panels are frosted layers on top of it
/// (`glass` fill + `glass_border` hairline). Alphas are high enough that text
/// stays legible when the compositor cannot blur.
#[derive(Clone, Copy)]
pub struct HawaPalette {
    /// Window gradient (translucent).
    pub bg_top: Rgba,
    pub bg_bottom: Rgba,
    /// Frosted panel fill and its hairline border.
    pub glass: Rgba,
    pub glass_border: Rgba,
    /// Stronger frosted fill for the sidebar and hover states.
    pub glass_strong: Rgba,
    /// Inset wells: progress tracks, chips, inner groups.
    pub well: Rgba,
    pub text: Rgba,
    pub text_dim: Rgba,
    pub text_muted: Rgba,
    pub primary: Rgba,
    pub accent: Rgba,
    pub on_primary: Rgba,
    pub success: Rgba,
    pub warn: Rgba,
    pub danger: Rgba,
    /// Opaque surface for popovers/menus, where translucency hurts legibility.
    pub popover: Rgba,
    pub dark: bool,
}

impl HawaPalette {
    pub fn dark() -> Self {
        Self {
            bg_top: rgba(0x0b2c36d6),
            bg_bottom: rgba(0x050c0fe6),
            glass: rgba(0xffffff0f),
            glass_border: rgba(0xffffff1f),
            glass_strong: rgba(0xffffff17),
            well: rgba(0xffffff12),
            text: rgb(0xebf4f6),
            text_dim: rgb(0x9fb1b6),
            text_muted: rgb(0x6a7f86),
            primary: rgb(0x00b6d4),
            accent: rgb(0x22d3ee),
            on_primary: rgb(0x031b20),
            success: rgb(0x34d399),
            warn: rgb(0xfbbf24),
            danger: rgb(0xf87171),
            popover: rgb(0x0e1c22),
            dark: true,
        }
    }

    pub fn light() -> Self {
        Self {
            bg_top: rgba(0xf3fbfceb),
            bg_bottom: rgba(0xd6eaf0f2),
            glass: rgba(0xffffffbf),
            glass_border: rgba(0xffffffff),
            glass_strong: rgba(0xffffffd9),
            well: rgba(0x0b171b0f),
            text: rgb(0x0b171b),
            text_dim: rgb(0x47595f),
            text_muted: rgb(0x7a8f96),
            primary: rgb(0x0891b2),
            accent: rgb(0x0e7490),
            on_primary: rgb(0xffffff),
            success: rgb(0x059669),
            warn: rgb(0xb45309),
            danger: rgb(0xdc2626),
            popover: rgb(0xf8fcfd),
            dark: false,
        }
    }

    /// Palette for the theme; with `translucent == false` the window gradient is
    /// painted fully opaque (the cards stay frosted over it).
    pub fn new(dark: bool, translucent: bool) -> Self {
        let mut pal = if dark { Self::dark() } else { Self::light() };
        if !translucent {
            pal.bg_top.a = 1.0;
            pal.bg_bottom.a = 1.0;
        }
        pal
    }
}

/// Whether the window background should be blurred-translucent, given the
/// `window_transparency` setting ("auto" | "on" | "off").
///
/// gpui only blurs on macOS, Windows, and Wayland compositors exposing KWin's
/// `org_kde_kwin_blur` (i.e. KDE Plasma). Elsewhere translucency would show the
/// desktop unblurred (or black without a compositor), so "auto" goes opaque.
/// gpui can't report whether blur took effect, so this is a best guess; "on"
/// covers compositors that blur on their own (e.g. Hyprland window rules).
pub fn use_translucent_window(setting: &str) -> bool {
    match setting {
        "on" => true,
        "off" => false,
        _ => blur_supported(),
    }
}

fn blur_supported() -> bool {
    if cfg!(any(target_os = "macos", target_os = "windows")) {
        return true;
    }
    let env = |k: &str| std::env::var(k).unwrap_or_default().to_ascii_lowercase();
    let wayland = env("XDG_SESSION_TYPE") == "wayland" || !env("WAYLAND_DISPLAY").is_empty();
    let kde = env("XDG_CURRENT_DESKTOP").split(':').any(|d| d == "kde")
        || env("KDE_FULL_SESSION") == "true";
    wayland && kde
}

pub fn window_background(translucent: bool) -> WindowBackgroundAppearance {
    if translucent {
        WindowBackgroundAppearance::Blurred
    } else {
        WindowBackgroundAppearance::Opaque
    }
}

pub fn apply_hawaos_gpui_theme(dark: bool, cx: &mut App) {
    let mode = if dark {
        GpuiThemeMode::Dark
    } else {
        GpuiThemeMode::Light
    };
    Theme::change(mode, None, cx);
    // Only gpui-kit widget colors come from here; the backdrop doesn't matter.
    let pal = HawaPalette::new(dark, true);

    Theme::update(cx, |theme| {
        theme.radius = px(10.);
        // `background` doubles as the light-mode input fill and popover fill,
        // so it stays (nearly) opaque; the window itself paints the glass.
        theme.colors.background = pal.popover.into();
        theme.colors.popover = pal.popover.into();
        theme.colors.foreground = pal.text.into();
        theme.colors.border = pal.glass_border.into();
        theme.colors.input = pal.glass_border.into();
        theme.colors.ring = pal.accent.into();
        theme.colors.caret = pal.accent.into();

        theme.colors.primary = pal.primary.into();
        theme.colors.primary_hover = pal.accent.into();
        theme.colors.primary_active = pal.primary.into();
        theme.colors.primary_foreground = pal.on_primary.into();
        // gpui-kit 0.8 paints buttons from their own fields, not `primary`/`secondary`.
        theme.colors.button_primary = pal.primary.into();
        theme.colors.button_primary_hover = pal.accent.into();
        theme.colors.button_primary_active = pal.primary.into();
        theme.colors.button_primary_foreground = pal.on_primary.into();
        theme.colors.button_secondary = pal.glass_strong.into();
        theme.colors.button_secondary_hover = pal.glass_border.into();
        theme.colors.button_secondary_active = pal.glass.into();
        theme.colors.button_secondary_foreground = pal.text.into();

        // Secondary buttons are frosted chips.
        theme.colors.secondary = pal.glass_strong.into();
        theme.colors.secondary_hover = pal.glass_border.into();
        theme.colors.secondary_active = pal.glass.into();
        theme.colors.secondary_foreground = pal.text.into();

        theme.colors.muted = pal.well.into();
        theme.colors.muted_foreground = pal.text_dim.into();

        theme.colors.accent = pal.glass_strong.into();
        theme.colors.accent_foreground = pal.text.into();

        theme.colors.sidebar = pal.glass_strong.into();
        theme.colors.sidebar_foreground = pal.text.into();
        theme.colors.sidebar_border = pal.glass_border.into();
        theme.colors.sidebar_primary = pal.primary.into();
        theme.colors.sidebar_primary_foreground = pal.on_primary.into();

        theme.colors.switch = pal.well.into();
        theme.colors.slider_bar = pal.primary.into();
        theme.colors.progress_bar = pal.primary.into();

        theme.colors.success = pal.success.into();
        theme.colors.warning = pal.warn.into();
        theme.colors.danger = pal.danger.into();
    });
}

#[cfg(test)]
mod tests {
    use super::use_translucent_window;

    // One test so the env mutations can't race each other.
    #[test]
    fn transparency_setting_and_detection() {
        let set = |session: &str, display: &str, desktop: &str, kde_full: &str| unsafe {
            std::env::set_var("XDG_SESSION_TYPE", session);
            std::env::set_var("WAYLAND_DISPLAY", display);
            std::env::set_var("XDG_CURRENT_DESKTOP", desktop);
            std::env::set_var("KDE_FULL_SESSION", kde_full);
        };

        set("wayland", "wayland-0", "KDE", "true");
        assert!(use_translucent_window("auto"), "KDE Wayland blurs");
        assert!(!use_translucent_window("off"), "off always wins");

        set("wayland", "wayland-0", "GNOME", "");
        assert!(!use_translucent_window("auto"), "GNOME has no kwin blur");
        assert!(use_translucent_window("on"), "on always wins");

        set("wayland", "wayland-1", "Hyprland", "");
        assert!(!use_translucent_window("auto"));

        set("x11", "", "KDE", "true");
        assert!(!use_translucent_window("auto"), "gpui can't blur on X11");

        set("wayland", "wayland-0", "ubuntu:GNOME", "");
        assert!(!use_translucent_window("auto"));
    }
}
