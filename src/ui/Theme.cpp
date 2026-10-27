#include "ui/Theme.h"
#include "config.h"
#include "core/Settings.h"
#include "services/Wallpaper.h"
#include <stdio.h>
#include <string.h>

namespace Theme {

namespace {
// Palettes are precomputed per accent in flash, so switching accents is just
// an index change: nothing is mutated that a half-drawn frame could observe.
const ThemeTokens::Palette &palette(bool dark) {
  return ThemeTokens::kPalettes[Settings::getAccent()][dark ? 0 : 1];
}
} // namespace

const ThemeTokens::Palette &active() {
  return palette(Settings::isDarkThemeEffective());
}

const ThemeTokens::Palette &dark() { return palette(true); }

uint32_t mediaTint() { return ThemeTokens::kMediaTint[Settings::getAccent()]; }

uint32_t resolveMediaTint(uint32_t bgTop) {
  // 0 = no tint yet; kLegacyMediaTint = the companion's "no artwork" default,
  // which predates accents and is always cyan; kLegacyMediaBottom = the
  // DataStore boot default. All of them follow the accent instead.
  if (bgTop == 0 || bgTop == kLegacyMediaTint || bgTop == kLegacyMediaBottom) {
    return mediaTint();
  }
  return bgTop;
}

uint32_t resolveMediaTintBottom(uint32_t bgBottom) {
  if (bgBottom == 0 || bgBottom == kLegacyMediaBottom) {
    return dark().bg;
  }
  return bgBottom;
}

bool isDark() { return Settings::isDarkThemeEffective(); }

void styleScreen(lv_obj_t *scr, bool withWallpaper, bool forceDark) {
  const ThemeTokens::Palette &p = forceDark ? dark() : active();

  lv_obj_set_style_bg_color(scr, lv_color_hex(p.bg), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(scr, LV_OPA_COVER, LV_PART_MAIN);

  const WallpaperMode mode = Settings::getWallpaperMode();
  const lv_img_dsc_t *picture =
      (withWallpaper && mode == WallpaperMode::Image) ? Wallpaper::image()
                                                      : nullptr;
  lv_obj_set_style_bg_img_src(scr, picture, LV_PART_MAIN);
  if (picture != nullptr) {
    // Tinted toward the theme's background so text and cards stay legible.
    lv_obj_set_style_bg_img_recolor(scr, lv_color_hex(p.bg), LV_PART_MAIN);
    lv_obj_set_style_bg_img_recolor_opa(scr, kWallpaperScrim, LV_PART_MAIN);
    lv_obj_set_style_bg_grad_dir(scr, LV_GRAD_DIR_NONE, LV_PART_MAIN);
  } else if (withWallpaper && mode != WallpaperMode::Solid) {
    lv_obj_set_style_bg_grad_color(scr, lv_color_hex(p.bgGradEnd),
                                   LV_PART_MAIN);
    lv_obj_set_style_bg_grad_dir(scr, LV_GRAD_DIR_VER, LV_PART_MAIN);
    lv_obj_set_style_bg_dither_mode(scr, kGradientDither, LV_PART_MAIN);
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
  const ThemeTokens::Palette &p = palette(useDark);

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
  const ThemeTokens::Palette &p = palette(useDark);

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

uint8_t luminance(uint32_t rgb888) {
  const uint32_t r = (rgb888 >> 16) & 0xFF;
  const uint32_t g = (rgb888 >> 8) & 0xFF;
  const uint32_t b = rgb888 & 0xFF;
  return static_cast<uint8_t>((r * 2126U + g * 7152U + b * 722U) / 10000U);
}

bool isColorLight(uint32_t rgb888) { return luminance(rgb888) > 118U; }

uint32_t tintAccent(uint32_t bgTop, uint32_t fallbackRgb) {
  if (bgTop == 0 || bgTop == kLegacyMediaBottom || bgTop == 0x05090B ||
      bgTop == kLegacyMediaTint || bgTop == mediaTint() ||
      bgTop == dark().bg) {
    return fallbackRgb;
  }
  uint32_t r = (bgTop >> 16) & 0xFF;
  uint32_t g = (bgTop >> 8) & 0xFF;
  uint32_t b = bgTop & 0xFF;

  const uint32_t maxCh = (r > g) ? ((r > b) ? r : b) : ((g > b) ? g : b);
  if (maxCh > 0 && maxCh < 120) {
    const uint32_t factor = (160U * 255U) / maxCh;
    r = (r * factor) / 255U;
    g = (g * factor) / 255U;
    b = (b * factor) / 255U;
    if (r > 255)
      r = 255;
    if (g > 255)
      g = 255;
    if (b > 255)
      b = 255;
  }
  return (r << 16) | (g << 8) | b;
}

namespace {

// UTF-8 sequence length from its first byte.
size_t utf8Len(uint8_t b) {
  return (b >= 0xF0) ? 4 : (b >= 0xE0) ? 3 : (b >= 0xC0) ? 2 : 1;
}

} // namespace

void formatText(const char *in, char *out, size_t outSize, bool multiline,
                const uint32_t *accent) {
  if (out == nullptr || outSize == 0)
    return;
  out[0] = '\0';
  if (in == nullptr)
    return;

  char open[10] = "";
  if (accent != nullptr) {
    snprintf(open, sizeof(open), "#%06lX ",
             static_cast<unsigned long>(*accent & 0xFFFFFF));
  }
  size_t o = 0;
  bool emph = false;
  bool full = false;
  // Appends n bytes if they fit with room left to close a run and terminate.
  auto put = [&](const char *s, size_t n) {
    if (full || o + n + (emph ? 1 : 0) + 1 > outSize) {
      full = true;
      return false;
    }
    memcpy(out + o, s, n);
    o += n;
    return true;
  };
  // Pending separator: collapsed spaces, or a line break.
  enum { None, Space, Break } gap = None;
  auto flushGap = [&]() {
    if (o > 0 && gap == Space) {
      put(" ", 1);
    } else if (o > 0 && gap == Break) {
      if (multiline) {
        put("\n", 1);
      } else {
        put(" \xC2\xB7 ", 4); // " · "
      }
    }
    gap = None;
  };

  for (size_t i = 0; in[i] != '\0' && !full;) {
    const uint8_t c = static_cast<uint8_t>(in[i]);
    if (c == '\n') {
      gap = Break;
      ++i;
    } else if (c == ' ' || c == '\t' || c == '\r') {
      if (gap == None)
        gap = Space;
      ++i;
    } else if (c == 0x02 || c == 0x03) { // emphasis on / off (11.10)
      const bool on = (c == 0x02);
      if (accent != nullptr && on != emph) {
        if (on) {
          flushGap();
          emph = true; // so put() keeps room for the closing '#'
          if (!put(open, strlen(open)))
            emph = false;
        } else {
          emph = false;
          out[o++] = '#'; // room was kept for it
        }
      }
      ++i;
    } else {
      flushGap();
      if (c == '#' && accent != nullptr) {
        // Escaped as "##"; a run can't contain it, so close and reopen.
        if (emph) {
          emph = false;
          out[o++] = '#';
          if (put("##", 2)) {
            emph = true;
            if (!put(open, strlen(open)))
              emph = false;
          }
        } else {
          put("##", 2);
        }
        ++i;
        continue;
      }
      size_t n = utf8Len(c);
      for (size_t k = 1; k < n; ++k) {
        if (in[i + k] == '\0') {
          n = k;
          break;
        }
      }
      put(in + i, n); // whole characters only
      i += n;
    }
  }
  if (emph) {
    out[o++] = '#';
  }
  out[o] = '\0';
}

void sanitizeText(const char *in, char *out, size_t outSize) {
  formatText(in, out, outSize, false, nullptr);
}

} // namespace Theme
