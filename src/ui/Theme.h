#pragma once

#include "theme_tokens.h"
#include "ui/icons.h"
#include <lvgl.h>
#include <stdint.h>

namespace Theme {

// Active palette accessor (Dark or Light per Settings::isDarkThemeEffective())
const ThemeTokens::Palette &active();

// Always-dark palette accessor (used by Media screen and AOD per DESIGN.md 3.3)
const ThemeTokens::Palette &dark();

bool isDark();

// Companion media tint sent when a track has no artwork (cyan, pre-accents),
// and the slate bottom paired with it (also the pre-accent dark `bg`).
constexpr uint32_t kLegacyMediaTint = 0x008DA6;
constexpr uint32_t kLegacyMediaBottom = 0x071115;

// Deep accent tint used as the media/AOD background top when a track has no
// artwork.
uint32_t mediaTint();

// Media background top color: an untinted value (0 or the legacy defaults)
// becomes mediaTint().
uint32_t resolveMediaTint(uint32_t bgTop);

// Media background bottom color: an untinted value becomes the accent-tinted
// dark `bg`.
uint32_t resolveMediaTintBottom(uint32_t bgBottom);

// Deep accent tints sit below one RGB565 step per channel and would band (or
// lose their hue) without dithering; error diffusion hides both.
constexpr lv_dither_mode_t kGradientDither = LV_DITHER_ERR_DIFF;
// Image wallpaper: share of the theme background laid over the picture.
constexpr lv_opa_t kWallpaperScrim = LV_OPA_50;

// Apply theme to root screen (solid `bg` or vertical gradient wallpaper if
// `withWallpaper` is true)
void styleScreen(lv_obj_t *scr, bool withWallpaper = false,
                 bool forceDark = false);

// Create a solid card (`surface` fill, optional 1px `border` in light mode,
// radius 8/12/16)
lv_obj_t *createCard(lv_obj_t *parent, int16_t w, int16_t h, int16_t x,
                     int16_t y, uint8_t radius = 12, bool forceDark = false);

// Apply or remove a 2px `accent` focus ring on a widget (never scales, per
// DESIGN.md Section 2)
void setFocusRing(lv_obj_t *obj, bool focused, bool forceDark = false);

// Helper to return animation duration honoring Settings::getReduceMotion()
// (DESIGN.md Section 7)
uint32_t animDuration(uint32_t normalMs);

// Color luminance and contrast helpers
uint8_t luminance(uint32_t rgb888);
bool isColorLight(uint32_t rgb888);
uint32_t tintAccent(uint32_t bgTop, uint32_t fallbackRgb);

// Companion text for a label (DESIGN.md Section 11.10): spaces collapsed,
// never cut inside a character. Line breaks stay (multiline) or become " · ".
// With `accent`, for a label with recolor on: emphasised runs (0x02..0x03)
// are drawn in that colour and '#' is escaped; without, the marks are dropped.
void formatText(const char *in, char *out, size_t outSize, bool multiline,
                const uint32_t *accent);
// One plain line (app names, titles).
void sanitizeText(const char *in, char *out, size_t outSize);

// Typography role helpers (DESIGN.md Section 4)
inline const lv_font_t *fontCaption() { return &font_inter_12_reg; }
inline const lv_font_t *fontBody() { return &font_inter_14_reg; }
inline const lv_font_t *fontBodySb() { return &font_inter_14_sb; }
inline const lv_font_t *fontTitle() { return &font_inter_18_sb; }
inline const lv_font_t *fontDisplay() { return &font_inter_28_sb; }
inline const lv_font_t *fontHero() { return &font_inter_48_sb; }

// Lucide icon font helpers (DESIGN.md Section 5)
inline const lv_font_t *iconFont20() { return &font_lucide_20; }
inline const lv_font_t *iconFont24() { return &font_lucide_24; }
inline const lv_font_t *iconFont32() { return &font_lucide_32; }
inline const lv_font_t *iconFont48() { return &font_lucide_48; }

} // namespace Theme
