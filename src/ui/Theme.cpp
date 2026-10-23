#include "ui/Theme.h"
#include "config.h"
#include "core/Settings.h"

namespace Theme {

const ThemeTokens::Palette &active() {
  return Settings::isDarkThemeEffective() ? ThemeTokens::kDarkPalette
                                          : ThemeTokens::kLightPalette;
}

const ThemeTokens::Palette &dark() { return ThemeTokens::kDarkPalette; }

bool isDark() { return Settings::isDarkThemeEffective(); }

void styleScreen(lv_obj_t *scr, bool withWallpaper, bool forceDark) {
  const ThemeTokens::Palette &p =
      forceDark ? ThemeTokens::kDarkPalette : active();

  lv_obj_set_style_bg_color(scr, lv_color_hex(p.bg), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(scr, LV_OPA_COVER, LV_PART_MAIN);

  if (withWallpaper &&
      Settings::getWallpaperMode() == WallpaperMode::Gradient) {
    lv_obj_set_style_bg_grad_color(scr, lv_color_hex(p.bgGradEnd),
                                   LV_PART_MAIN);
    lv_obj_set_style_bg_grad_dir(scr, LV_GRAD_DIR_VER, LV_PART_MAIN);
  } else {
    lv_obj_set_style_bg_grad_dir(scr, LV_GRAD_DIR_NONE, LV_PART_MAIN);
  }

  lv_obj_set_style_border_width(scr, 0, LV_PART_MAIN);
  lv_obj_set_style_pad_all(scr, 0, LV_PART_MAIN);
  lv_obj_clear_flag(scr, LV_OBJ_FLAG_SCROLLABLE);
}

lv_obj_t *createCard(lv_obj_t *parent, int16_t w, int16_t h, int16_t x,
                     int16_t y, uint8_t radius, bool forceDark) {
  const bool useDark = forceDark || isDark();
  const ThemeTokens::Palette &p =
      useDark ? ThemeTokens::kDarkPalette : ThemeTokens::kLightPalette;

  lv_obj_t *card = lv_obj_create(parent);
  lv_obj_set_pos(card, x, y);
  lv_obj_set_size(card, w, h);
  lv_obj_set_style_bg_color(card, lv_color_hex(p.surface), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(card, LV_OPA_COVER, LV_PART_MAIN);
  lv_obj_set_style_radius(card, radius, LV_PART_MAIN);
  lv_obj_set_style_pad_all(card, 8, LV_PART_MAIN);

  // DESIGN.md Section 2: 1 px border token in light theme; dark theme uses
  // surface contrast
  if (!useDark) {
    lv_obj_set_style_border_color(card, lv_color_hex(p.border), LV_PART_MAIN);
    lv_obj_set_style_border_width(card, 1, LV_PART_MAIN);
  } else {
    lv_obj_set_style_border_width(card, 0, LV_PART_MAIN);
  }

  lv_obj_clear_flag(card, LV_OBJ_FLAG_SCROLLABLE);
  return card;
}

void setFocusRing(lv_obj_t *obj, bool focused, bool forceDark) {
  if (obj == nullptr)
    return;
  const bool useDark = forceDark || isDark();
  const ThemeTokens::Palette &p =
      useDark ? ThemeTokens::kDarkPalette : ThemeTokens::kLightPalette;

  if (focused) {
    lv_obj_set_style_border_color(obj, lv_color_hex(p.accent), LV_PART_MAIN);
    lv_obj_set_style_border_width(obj, Config::FOCUS_RING_WIDTH, LV_PART_MAIN);
  } else {
    if (!useDark) {
      lv_obj_set_style_border_color(obj, lv_color_hex(p.border), LV_PART_MAIN);
      lv_obj_set_style_border_width(obj, 1, LV_PART_MAIN);
    } else {
      lv_obj_set_style_border_width(obj, 0, LV_PART_MAIN);
    }
  }
}

uint32_t animDuration(uint32_t normalMs) {
  return Settings::getReduceMotion() ? 0 : normalMs;
}

} // namespace Theme
