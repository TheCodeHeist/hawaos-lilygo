use gpui_kit::gpui::{px, rgb, App, Rgba};
use gpui_kit::component::theme::{Theme, ThemeMode as GpuiThemeMode};

#[derive(Clone, Copy)]
pub struct HawaPalette {
    pub bg: Rgba,
    pub surface: Rgba,
    pub surface2: Rgba,
    pub border: Rgba,
    pub text: Rgba,
    pub text_dim: Rgba,
    pub text_muted: Rgba,
    pub primary: Rgba,
    pub accent: Rgba,
    pub on_primary: Rgba,
    pub success: Rgba,
    pub warn: Rgba,
    pub danger: Rgba,
}

impl HawaPalette {
    pub fn dark() -> Self {
        Self {
            bg: rgb(0x071115),
            surface: rgb(0x101d22),
            surface2: rgb(0x1b292f),
            border: rgb(0x28353b),
            text: rgb(0xebf4f6),
            text_dim: rgb(0x98a8ad),
            text_muted: rgb(0x5b6d73),
            primary: rgb(0x00b6d4),
            accent: rgb(0x00d1ec),
            on_primary: rgb(0x031b20),
            success: rgb(0x22c55e),
            warn: rgb(0xf59e0b),
            danger: rgb(0xef4444),
        }
    }

    pub fn light() -> Self {
        Self {
            bg: rgb(0xf2f8f9),
            surface: rgb(0xfafcfd),
            surface2: rgb(0xe3eff2),
            border: rgb(0xcbdce0),
            text: rgb(0x0b171b),
            text_dim: rgb(0x4a5d63),
            text_muted: rgb(0x7e939a),
            primary: rgb(0x008da6),
            accent: rgb(0x00b6d4),
            on_primary: rgb(0xfafcfd),
            success: rgb(0x16a34a),
            warn: rgb(0xd97706),
            danger: rgb(0xdc2626),
        }
    }
}

pub fn apply_hawaos_gpui_theme(dark: bool, cx: &mut App) {
    let mode = if dark {
        GpuiThemeMode::Dark
    } else {
        GpuiThemeMode::Light
    };
    Theme::change(mode, None, cx);
    let pal = if dark {
        HawaPalette::dark()
    } else {
        HawaPalette::light()
    };

    Theme::update(cx, |theme| {
        theme.radius = px(10.);
        theme.colors.background = pal.bg.into();
        theme.colors.foreground = pal.text.into();
        theme.colors.border = pal.border.into();
        theme.colors.input = pal.border.into();
        theme.colors.ring = pal.accent.into();
        theme.colors.caret = pal.accent.into();

        theme.colors.primary = pal.primary.into();
        theme.colors.primary_hover = pal.accent.into();
        theme.colors.primary_active = pal.primary.into();
        theme.colors.primary_foreground = pal.on_primary.into();

        theme.colors.secondary = pal.surface2.into();
        theme.colors.secondary_hover = pal.border.into();
        theme.colors.secondary_active = pal.surface.into();
        theme.colors.secondary_foreground = pal.text.into();

        theme.colors.muted = pal.surface2.into();
        theme.colors.muted_foreground = pal.text_dim.into();

        theme.colors.accent = pal.surface2.into();
        theme.colors.accent_foreground = pal.text.into();

        theme.colors.sidebar = pal.surface.into();
        theme.colors.sidebar_foreground = pal.text.into();
        theme.colors.sidebar_border = pal.border.into();
        theme.colors.sidebar_primary = pal.primary.into();
        theme.colors.sidebar_primary_foreground = pal.on_primary.into();

        theme.colors.switch = pal.surface2.into();
        theme.colors.slider_bar = pal.primary.into();
        theme.colors.progress_bar = pal.primary.into();

        theme.colors.success = pal.success.into();
        theme.colors.warning = pal.warn.into();
        theme.colors.danger = pal.danger.into();
    });
}
