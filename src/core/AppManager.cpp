#include "core/AppManager.h"
#include "config.h"
#include "core/DataStore.h"
#include "core/Kernel.h"
#include "core/Settings.h"
#include "drivers/Display.h"
#include "services/WifiService.h"
#include "services/CompanionLink.h"
#include "ui/StatusBar.h"
#include "ui/Theme.h"
#include "ui/icons.h"

#include <Arduino.h>
#include <esp_heap_caps.h>
#include <lvgl.h>
#include <string.h>
#include <time.h>

namespace AppManager {

namespace {

enum class ViewState : uint8_t {
  InApp,
  Launcher,
  ControlCenter,
  Aod,
};

App *s_apps[Config::MAX_REGISTERED_APPS] = {nullptr};
size_t s_appCount = 0;
int s_activeAppIdx = 0;      // Last/active app index
size_t s_launcherCursor = 0; // Focused app in Launcher carousel
uint8_t s_ccCursor = 0;      // 0..3 in Control Center 2x2 grid
ViewState s_viewState = ViewState::InApp;
ViewState s_preOverlayView = ViewState::InApp;
uint32_t s_lastInputMs = 0;
uint32_t s_switchCounter = 0;

// --- Launcher Widgets (valid when s_viewState == ViewState::Launcher) ---
lv_obj_t *s_slotTiles[3] = {nullptr};
lv_obj_t *s_slotIcons[3] = {nullptr};
lv_obj_t *s_launcherNameLbl = nullptr;
lv_obj_t *s_launcherDots[Config::MAX_REGISTERED_APPS] = {nullptr};

// --- Control Center Widgets (valid when s_viewState ==
// ViewState::ControlCenter) ---
lv_obj_t *s_ccTiles[4] = {nullptr};
lv_obj_t *s_ccIcons[4] = {nullptr};
lv_obj_t *s_ccTitleLbls[4] = {nullptr};
lv_obj_t *s_ccValLbls[4] = {nullptr};

// --- AOD Widgets (valid when s_viewState == ViewState::Aod) ---
lv_obj_t *s_aodScreen = nullptr;
lv_obj_t *s_aodClockLbl = nullptr;
lv_obj_t *s_aodAmPmLbl = nullptr;
lv_obj_t *s_aodDateLbl = nullptr;
lv_obj_t *s_aodArtBox = nullptr;
lv_obj_t *s_aodArtIcon = nullptr;
lv_obj_t *s_aodArtImg = nullptr;
lv_img_dsc_t s_aodArtDsc{};
lv_obj_t *s_aodTitleLbl = nullptr;
lv_obj_t *s_aodArtistLbl = nullptr;
lv_obj_t *s_aodProgBar = nullptr;
uint32_t s_aodFromTop = 0x000000;
uint32_t s_aodFromBot = 0x000000;
uint32_t s_aodCurTop = 0x000000;
uint32_t s_aodCurBot = 0x000000;
uint32_t s_aodToTop = 0x000000;
uint32_t s_aodToBot = 0x000000;
uint32_t s_aodLastSec = 0xFFFFFFFF;
uint32_t s_aodLastSubSecMs = 0;
bool s_aodWasPlaying = false;
uint32_t s_aodPausedSinceMs = 0;

// --- AOD Timer / Stopwatch & Media Footnote Elements ---
lv_obj_t *s_aodTimerTagLbl = nullptr;
lv_obj_t *s_aodTimerDigitsLbl = nullptr;
lv_obj_t *s_aodTimerMsLbl = nullptr;
lv_obj_t *s_aodTimerSubLbl = nullptr;
lv_obj_t *s_aodTimerProgBar = nullptr;
lv_obj_t *s_aodMediaFootnoteCard = nullptr;
lv_obj_t *s_aodFootnoteIcon = nullptr;
lv_obj_t *s_aodFootnoteText = nullptr;
lv_obj_t *s_aodFootnoteProgBar = nullptr;

// --- AOD Notification Alert Elements ---
lv_obj_t *s_aodAlertCard = nullptr;
lv_obj_t *s_aodAlertIcon = nullptr;
lv_obj_t *s_aodAlertHeaderLbl = nullptr;
lv_obj_t *s_aodAlertBadgeLbl = nullptr;
lv_obj_t *s_aodAlertTitleLbl = nullptr;
lv_obj_t *s_aodAlertBodyLbl = nullptr;
bool s_aodAlertActive = false;
uint32_t s_aodAlertStartMs = 0;
constexpr uint32_t kAodAlertDurationMs = 4500;

void setLabelTextIfChanged(lv_obj_t *lbl, const char *newText) {
  if (lbl == nullptr || newText == nullptr)
    return;
  const char *cur = lv_label_get_text(lbl);
  if (cur == nullptr || strcmp(cur, newText) != 0) {
    lv_label_set_text(lbl, newText);
  }
}

void clearViewPointers() {
  for (int i = 0; i < 3; ++i) {
    s_slotTiles[i] = nullptr;
    s_slotIcons[i] = nullptr;
  }
  s_launcherNameLbl = nullptr;
  for (size_t i = 0; i < Config::MAX_REGISTERED_APPS; ++i) {
    s_launcherDots[i] = nullptr;
  }
  for (int i = 0; i < 4; ++i) {
    s_ccTiles[i] = nullptr;
    s_ccIcons[i] = nullptr;
    s_ccTitleLbls[i] = nullptr;
    s_ccValLbls[i] = nullptr;
  }
  s_aodScreen = nullptr;
  s_aodClockLbl = nullptr;
  s_aodAmPmLbl = nullptr;
  s_aodDateLbl = nullptr;
  s_aodArtBox = nullptr;
  s_aodArtIcon = nullptr;
  s_aodArtImg = nullptr;
  s_aodTitleLbl = nullptr;
  s_aodArtistLbl = nullptr;
  s_aodProgBar = nullptr;
  s_aodTimerTagLbl = nullptr;
  s_aodTimerDigitsLbl = nullptr;
  s_aodTimerMsLbl = nullptr;
  s_aodTimerSubLbl = nullptr;
  s_aodTimerProgBar = nullptr;
  s_aodMediaFootnoteCard = nullptr;
  s_aodFootnoteIcon = nullptr;
  s_aodFootnoteText = nullptr;
  s_aodFootnoteProgBar = nullptr;
  s_aodAlertCard = nullptr;
  s_aodAlertIcon = nullptr;
  s_aodAlertHeaderLbl = nullptr;
  s_aodAlertBadgeLbl = nullptr;
  s_aodAlertTitleLbl = nullptr;
  s_aodAlertBodyLbl = nullptr;
  s_aodAlertActive = false;
}

lv_obj_t *createAndSwapScreen(bool withWallpaper, bool forceDark = false) {
  lv_obj_t *oldScr = lv_scr_act();
  lv_obj_t *newScr = lv_obj_create(nullptr);
  Theme::styleScreen(newScr, withWallpaper, forceDark);

  lv_scr_load(newScr);
  if (oldScr != nullptr && oldScr != newScr) {
    lv_obj_del(oldScr);
  }
  return newScr;
}

// =============================================================================
// Redmi-style Horizontal Carousel Launcher (DESIGN.md Section 9.2)
// =============================================================================
void refreshLauncherCarousel() {
  if (s_viewState != ViewState::Launcher || s_appCount == 0)
    return;
  const ThemeTokens::Palette &p = Theme::active();

  const size_t leftIdx = (s_launcherCursor + s_appCount - 1) % s_appCount;
  const size_t midIdx = s_launcherCursor;
  const size_t rightIdx = (s_launcherCursor + 1) % s_appCount;
  const size_t indices[3] = {leftIdx, midIdx, rightIdx};

  for (int slot = 0; slot < 3; ++slot) {
    if (s_slotTiles[slot] == nullptr || s_slotIcons[slot] == nullptr)
      continue;
    const bool isCenter = (slot == 1);
    const size_t appIdx = indices[slot];
    App *app = s_apps[appIdx];

    lv_label_set_text(s_slotIcons[slot], app->icon());
    Theme::setFocusRing(s_slotTiles[slot], isCenter);
    lv_obj_set_style_text_color(s_slotIcons[slot],
                                lv_color_hex(isCenter ? p.accent : p.textDim),
                                LV_PART_MAIN);
  }

  if (s_launcherNameLbl != nullptr) {
    lv_label_set_text(s_launcherNameLbl, s_apps[midIdx]->name());
    lv_obj_set_style_text_color(s_launcherNameLbl, lv_color_hex(p.text),
                                LV_PART_MAIN);
  }

  for (size_t i = 0; i < s_appCount; ++i) {
    if (s_launcherDots[i] == nullptr)
      continue;
    const bool activeDot = (i == midIdx);
    lv_obj_set_style_bg_color(s_launcherDots[i],
                              lv_color_hex(activeDot ? p.accent : p.border),
                              LV_PART_MAIN);
  }
}

void buildLauncherScreen(lv_obj_t *root) {
  StatusBar::setVisible(true);
  StatusBar::setTitle("Launcher");

  // 3 visible 88x88 tiles, radius 16, gap 16 -> x = 12, 116, 220; y = 28
  constexpr int16_t kTileXs[3] = {12, 116, 220};
  constexpr int16_t kTileY = 28;
  constexpr int16_t kTileSize = 88;

  for (int slot = 0; slot < 3; ++slot) {
    lv_obj_t *tile = Theme::createCard(
        root, kTileSize, kTileSize, kTileXs[slot], kTileY, Config::RADIUS_LG);
    lv_obj_t *iconLbl = lv_label_create(tile);
    lv_obj_set_style_text_font(iconLbl, Theme::iconFont48(), LV_PART_MAIN);
    lv_obj_center(iconLbl);

    s_slotTiles[slot] = tile;
    s_slotIcons[slot] = iconLbl;
  }

  // Focused App Name (title 18 SemiBold) centered beneath middle tile
  s_launcherNameLbl = lv_label_create(root);
  lv_obj_set_style_text_font(s_launcherNameLbl, Theme::fontTitle(),
                             LV_PART_MAIN);
  lv_obj_align(s_launcherNameLbl, LV_ALIGN_TOP_MID, 0, 122);

  // Page dots at bottom
  const int16_t dotSpacing = 12;
  const int16_t totalDotsW =
      static_cast<int16_t>((s_appCount - 1) * dotSpacing);
  const int16_t startDotX = (Config::LCD_HOR_RES / 2) - (totalDotsW / 2) - 3;

  for (size_t i = 0; i < s_appCount; ++i) {
    lv_obj_t *dot = lv_obj_create(root);
    lv_obj_set_size(dot, 6, 6);
    lv_obj_set_pos(dot, startDotX + static_cast<int16_t>(i * dotSpacing), 152);
    lv_obj_set_style_radius(dot, LV_RADIUS_CIRCLE, LV_PART_MAIN);
    lv_obj_set_style_border_width(dot, 0, LV_PART_MAIN);
    lv_obj_set_style_bg_opa(dot, LV_OPA_COVER, LV_PART_MAIN);
    lv_obj_clear_flag(dot, LV_OBJ_FLAG_SCROLLABLE);
    s_launcherDots[i] = dot;
  }

  refreshLauncherCarousel();
}

// =============================================================================
// Control Center 2x2 Full-Screen Tiles (DESIGN.md Section 9.7)
// =============================================================================
void refreshControlCenterTiles() {
  if (s_viewState != ViewState::ControlCenter)
    return;
  const ThemeTokens::Palette &p = Theme::active();
  const DataSnapshot d = DataStore::getSnapshot();

  // Tile 0: Brightness (20/40/60/80/100%)
  lv_label_set_text(s_ccIcons[0], ICON_SUN);
  lv_label_set_text(s_ccTitleLbls[0], "Brightness");
  lv_label_set_text_fmt(s_ccValLbls[0], "%u%%",
                        static_cast<unsigned>(Settings::getBrightnessPct()));

  // Tile 1: Theme (Dark / Light)
  const bool dark = Theme::isDark();
  lv_label_set_text(s_ccIcons[1], dark ? ICON_MOON : ICON_SUN);
  lv_label_set_text(s_ccTitleLbls[1], "Theme");
  lv_label_set_text(s_ccValLbls[1], dark ? "Dark" : "Light");

  // Tile 2: Connection (WiFi / USB state, Select = reconnect)
  lv_label_set_text(s_ccIcons[2], d.wifiConnected ? ICON_WIFI : ICON_WIFI_OFF);
  lv_label_set_text(s_ccTitleLbls[2], "Connection");
  lv_label_set_text(s_ccValLbls[2],
                    d.wifiConnected
                        ? "WiFi OK"
                        : (d.wifiSsid[0] ? "Reconnect" : "Use Companion"));

  // Tile 3: AOD (Enter AOD now)
  lv_label_set_text(s_ccIcons[3], ICON_CLOCK);
  lv_label_set_text(s_ccTitleLbls[3], "Ambient");
  lv_label_set_text(s_ccValLbls[3], "Enter AOD");

  for (int i = 0; i < 4; ++i) {
    const bool focused = (i == s_ccCursor);
    Theme::setFocusRing(s_ccTiles[i], focused);
    lv_obj_set_style_text_color(
        s_ccIcons[i], lv_color_hex(focused ? p.accent : p.text), LV_PART_MAIN);
  }
}

void buildControlCenterScreen(lv_obj_t *root) {
  StatusBar::setVisible(false);

  // 2x2 grid: tiles are 144x68, x = 12, 164; y = 12, 88
  constexpr int16_t kTileW = 144;
  constexpr int16_t kTileH = 68;
  constexpr int16_t kTileXs[4] = {12, 164, 12, 164};
  constexpr int16_t kTileYs[4] = {12, 12, 88, 88};

  for (int i = 0; i < 4; ++i) {
    lv_obj_t *tile = Theme::createCard(root, kTileW, kTileH, kTileXs[i],
                                       kTileYs[i], Config::RADIUS_MD);

    lv_obj_t *iconLbl = lv_label_create(tile);
    lv_obj_set_style_text_font(iconLbl, Theme::iconFont32(), LV_PART_MAIN);
    lv_obj_align(iconLbl, LV_ALIGN_LEFT_MID, 6, 0);

    lv_obj_t *titleLbl = lv_label_create(tile);
    lv_obj_set_style_text_font(titleLbl, Theme::fontCaption(), LV_PART_MAIN);
    lv_obj_set_style_text_color(titleLbl, lv_color_hex(Theme::active().textDim),
                                LV_PART_MAIN);
    lv_obj_align(titleLbl, LV_ALIGN_TOP_LEFT, 46, 12);

    lv_obj_t *valLbl = lv_label_create(tile);
    lv_obj_set_style_text_font(valLbl, Theme::fontBodySb(), LV_PART_MAIN);
    lv_obj_set_style_text_color(valLbl, lv_color_hex(Theme::active().text),
                                LV_PART_MAIN);
    lv_obj_align(valLbl, LV_ALIGN_TOP_LEFT, 46, 32);

    s_ccTiles[i] = tile;
    s_ccIcons[i] = iconLbl;
    s_ccTitleLbls[i] = titleLbl;
    s_ccValLbls[i] = valLbl;
  }

  refreshControlCenterTiles();
}

// =============================================================================
// Always-On Display (AOD) Full-Screen Ambient Clock (DESIGN.md Section 10)
// =============================================================================
uint32_t lerpRgb888(uint32_t fromRgb, uint32_t toRgb, uint8_t t) {
  const uint32_t inv = 255U - t;
  const uint32_t r =
      (((fromRgb >> 16) & 0xFFU) * inv + ((toRgb >> 16) & 0xFFU) * t) / 255U;
  const uint32_t g =
      (((fromRgb >> 8) & 0xFFU) * inv + ((toRgb >> 8) & 0xFFU) * t) / 255U;
  const uint32_t b = ((fromRgb & 0xFFU) * inv + (toRgb & 0xFFU) * t) / 255U;
  return (r << 16) | (g << 8) | b;
}

void aodAnimExecCb(void * /*var*/, int32_t v) {
  if (s_aodScreen == nullptr)
    return;
  const uint8_t mix = static_cast<uint8_t>(constrain(v, 0, 255));
  s_aodCurTop = lerpRgb888(s_aodFromTop, s_aodToTop, mix);
  s_aodCurBot = lerpRgb888(s_aodFromBot, s_aodToBot, mix);

  lv_obj_set_style_bg_color(s_aodScreen, lv_color_hex(s_aodCurTop),
                            LV_PART_MAIN);
  lv_obj_set_style_bg_grad_color(s_aodScreen, lv_color_hex(s_aodCurBot),
                                 LV_PART_MAIN);
  lv_obj_set_style_bg_grad_dir(s_aodScreen, LV_GRAD_DIR_VER, LV_PART_MAIN);
  lv_obj_invalidate(s_aodScreen);
}

void startAodTintTween(uint32_t toTop, uint32_t toBot, uint32_t durationMs) {
  if (toTop == s_aodToTop && toBot == s_aodToBot)
    return;

  s_aodFromTop = s_aodCurTop;
  s_aodFromBot = s_aodCurBot;
  s_aodToTop = toTop;
  s_aodToBot = toBot;

  lv_anim_del(s_aodScreen, aodAnimExecCb);

  const uint32_t dur = Theme::animDuration(durationMs);
  if (dur == 0) {
    aodAnimExecCb(nullptr, 255);
    return;
  }

  lv_anim_t a;
  lv_anim_init(&a);
  lv_anim_set_var(&a, s_aodScreen);
  lv_anim_set_values(&a, 0, 255);
  lv_anim_set_time(&a, dur);
  lv_anim_set_path_cb(&a, lv_anim_path_ease_out);
  lv_anim_set_exec_cb(&a, aodAnimExecCb);
  lv_anim_start(&a);
}

void refreshAodContent() {
  if (s_viewState != ViewState::Aod || s_aodClockLbl == nullptr ||
      s_aodAlertActive)
    return;

  const DataSnapshot d = DataStore::getSnapshot();
  const ThemeTokens::Palette &p = Theme::dark();

  const bool use24 = Settings::getUse24Hour();
  const uint32_t nowMs = millis();

  // Media active check
  bool showMedia = false;
  if (d.mediaTitle[0] != '\0') {
    if (d.mediaState == MediaPlayState::Playing) {
      showMedia = true;
      s_aodWasPlaying = true;
      s_aodPausedSinceMs = 0;
    } else if (d.mediaState == MediaPlayState::Paused && s_aodWasPlaying) {
      if (s_aodPausedSinceMs == 0) {
        s_aodPausedSinceMs = nowMs;
      }
      if ((nowMs - s_aodPausedSinceMs) < 30000UL) {
        showMedia = true;
      } else {
        s_aodWasPlaying = false;
      }
    } else {
      s_aodWasPlaying = false;
    }
  }

  const bool timerActive =
      (d.timer.state == TimerState::Running || d.timer.state == TimerState::Paused ||
       d.timer.state == TimerState::Finished);
  const bool stopwatchActive =
      (d.stopwatch.state == StopwatchState::Running || d.stopwatch.state == StopwatchState::Paused);
  const bool clockToolsActive = (timerActive || stopwatchActive);

  // Dynamic background gradient tween: keep tinted gradient if media is active
  const uint32_t targetTop =
      showMedia ? Theme::resolveMediaTint(d.mediaBgTop) : 0x000000;
  const uint32_t targetBot = showMedia ? d.mediaBgBottom : 0x000000;
  startAodTintTween(targetTop, targetBot, Config::ANIM_AOD_ENTER_MS);

  const uint32_t aodBgTop =
      showMedia ? Theme::resolveMediaTint(d.mediaBgTop) : 0x071115;
  const bool aodLightBg = Theme::isColorLight(aodBgTop) || (!d.mediaInkLight && showMedia);
  const uint32_t clockColor = aodLightBg ? 0x0A1014 : 0xFFFFFF;
  const uint32_t dateColor = aodLightBg ? 0x2D3E45 : 0xA0B4BA;
  const uint32_t titleColor = aodLightBg ? 0x0A1014 : 0xFFFFFF;
  const uint32_t artistColor = aodLightBg ? 0x243238 : 0xD0DCE0;
  const uint32_t aodProgColor = Theme::tintAccent(aodBgTop, p.primary);
  const uint32_t aodTrackColor = aodLightBg ? 0x000000 : 0xFFFFFF;
  const lv_opa_t aodTrackOpa = aodLightBg ? LV_OPA_20 : LV_OPA_30;

  const time_t now = time(nullptr);
  const bool validTime = (now > 1700000000);
  struct tm ti{};
  int hr = 0;
  int mn = 0;
  const char *ampmStr = "";
  char shortDateStr[32] = {0};

  if (validTime) {
    localtime_r(&now, &ti);
    hr = ti.tm_hour;
    mn = ti.tm_min;
    if (!use24) {
      ampmStr = (ti.tm_hour >= 12) ? "PM" : "AM";
      hr = ti.tm_hour % 12;
      if (hr == 0)
        hr = 12;
    }
    strftime(shortDateStr, sizeof(shortDateStr), "%a \u00b7 %b %d", &ti);
  } else {
    const uint32_t up = nowMs / 1000;
    hr = static_cast<int>((up / 60) % 60);
    mn = static_cast<int>(up % 60);
    snprintf(shortDateStr, sizeof(shortDateStr), "Syncing NTP...");
  }

  // =========================================================================
  // Case A: Stopwatch or Timer is Active in AOD
  // =========================================================================
  if (clockToolsActive) {
    // 1. Top bar: Main clock is made SMALLER (Theme::fontBodySb()) when timer/stopwatch is running
    lv_obj_set_style_text_font(s_aodClockLbl, Theme::fontBodySb(), LV_PART_MAIN);
    lv_obj_set_style_text_color(s_aodClockLbl, lv_color_hex(clockColor), LV_PART_MAIN);
    if (!use24 && validTime) {
      lv_label_set_text_fmt(s_aodClockLbl, "%02d:%02d %s", hr, mn, ampmStr);
    } else {
      lv_label_set_text_fmt(s_aodClockLbl, "%02d:%02d", hr, mn);
    }
    lv_obj_align(s_aodClockLbl, LV_ALIGN_TOP_LEFT, 20, 6);
    lv_obj_clear_flag(s_aodClockLbl, LV_OBJ_FLAG_HIDDEN);

    if (s_aodAmPmLbl != nullptr) {
      lv_obj_add_flag(s_aodAmPmLbl, LV_OBJ_FLAG_HIDDEN);
    }

    if (s_aodDateLbl != nullptr) {
      setLabelTextIfChanged(s_aodDateLbl, shortDateStr);
      lv_obj_set_style_text_font(s_aodDateLbl, Theme::fontCaption(), LV_PART_MAIN);
      lv_obj_set_style_text_color(s_aodDateLbl, lv_color_hex(dateColor), LV_PART_MAIN);
      lv_obj_align(s_aodDateLbl, LV_ALIGN_TOP_RIGHT, -20, 7);
      lv_obj_clear_flag(s_aodDateLbl, LV_OBJ_FLAG_HIDDEN);
    }

    // 2. Hide large media widgets
    lv_obj_add_flag(s_aodArtBox, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(s_aodTitleLbl, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(s_aodArtistLbl, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(s_aodProgBar, LV_OBJ_FLAG_HIDDEN);

    // 3. Middle Section: Timer or Stopwatch display with HOURS and smaller MILLISECONDS
    char digitsBuf[16] = {0};
    char msBuf[8] = {0};
    const char *tagStr = "";
    const char *subStr = "";
    int progPct = -1;

    if (timerActive) {
      tagStr = "TIMER";
      const uint32_t remMs = DataStore::getTimerRemainingMs(nowMs);
      const uint32_t totSec = remMs / 1000;
      const uint32_t remHr = totSec / 3600;
      const uint32_t remMin = (totSec / 60) % 60;
      const uint32_t remSec = totSec % 60;
      const uint32_t remMsFrac = remMs % 1000;
      snprintf(digitsBuf, sizeof(digitsBuf), "%02lu:%02lu:%02lu",
               static_cast<unsigned long>(remHr), static_cast<unsigned long>(remMin),
               static_cast<unsigned long>(remSec));
      snprintf(msBuf, sizeof(msBuf), ".%03lu", static_cast<unsigned long>(remMsFrac));

      if (d.timer.state == TimerState::Running) {
        subStr = "RUNNING";
      } else if (d.timer.state == TimerState::Paused) {
        subStr = "PAUSED";
      } else {
        subStr = "TIME'S UP!";
      }

      if (d.timer.durationSec > 0) {
        const uint32_t el = (d.timer.durationSec >= totSec) ? (d.timer.durationSec - totSec) : d.timer.durationSec;
        progPct = static_cast<int>((el * 100) / d.timer.durationSec);
      }
    } else { // stopwatchActive
      tagStr = "STOPWATCH";
      const uint32_t el = DataStore::getStopwatchElapsedMs(nowMs);
      const uint32_t totSec = el / 1000;
      const uint32_t elHr = totSec / 3600;
      const uint32_t elMin = (totSec / 60) % 60;
      const uint32_t elSec = totSec % 60;
      const uint32_t elMsFrac = el % 1000;
      snprintf(digitsBuf, sizeof(digitsBuf), "%02lu:%02lu:%02lu",
               static_cast<unsigned long>(elHr), static_cast<unsigned long>(elMin),
               static_cast<unsigned long>(elSec));
      snprintf(msBuf, sizeof(msBuf), ".%03lu", static_cast<unsigned long>(elMsFrac));
      subStr = (d.stopwatch.state == StopwatchState::Running) ? "RUNNING" : "PAUSED";
    }

    if (s_aodTimerTagLbl != nullptr) {
      setLabelTextIfChanged(s_aodTimerTagLbl, tagStr);
      lv_obj_set_style_text_color(s_aodTimerTagLbl, lv_color_hex(p.accent), LV_PART_MAIN);
      lv_obj_clear_flag(s_aodTimerTagLbl, LV_OBJ_FLAG_HIDDEN);
    }
    if (s_aodTimerDigitsLbl != nullptr) {
      setLabelTextIfChanged(s_aodTimerDigitsLbl, digitsBuf);
      lv_obj_set_style_text_font(s_aodTimerDigitsLbl, Theme::fontDisplay(), LV_PART_MAIN);
      lv_obj_set_style_text_color(s_aodTimerDigitsLbl, lv_color_hex(titleColor), LV_PART_MAIN);
      lv_obj_clear_flag(s_aodTimerDigitsLbl, LV_OBJ_FLAG_HIDDEN);
    }
    if (s_aodTimerMsLbl != nullptr) {
      setLabelTextIfChanged(s_aodTimerMsLbl, msBuf);
      lv_obj_set_style_text_font(s_aodTimerMsLbl, Theme::fontBody(), LV_PART_MAIN);
      lv_obj_set_style_text_color(s_aodTimerMsLbl, lv_color_hex(artistColor), LV_PART_MAIN);
      lv_obj_clear_flag(s_aodTimerMsLbl, LV_OBJ_FLAG_HIDDEN);
    }
    if (s_aodTimerSubLbl != nullptr) {
      setLabelTextIfChanged(s_aodTimerSubLbl, subStr);
      lv_obj_set_style_text_color(s_aodTimerSubLbl, lv_color_hex(p.textDim), LV_PART_MAIN);
      lv_obj_clear_flag(s_aodTimerSubLbl, LV_OBJ_FLAG_HIDDEN);
    }

    if (s_aodTimerProgBar != nullptr) {
      if (progPct >= 0) {
        lv_bar_set_value(s_aodTimerProgBar, progPct, LV_ANIM_OFF);
        lv_obj_clear_flag(s_aodTimerProgBar, LV_OBJ_FLAG_HIDDEN);
      } else {
        lv_obj_add_flag(s_aodTimerProgBar, LV_OBJ_FLAG_HIDDEN);
      }
    }

    // Positioning centered in available vertical space
    if (showMedia) {
      // Top bar at y=8..32, Footnote at y=128..160 -> Center region y=34..126
      if (s_aodTimerTagLbl != nullptr) lv_obj_align(s_aodTimerTagLbl, LV_ALIGN_CENTER, 0, -28);
      if (s_aodTimerDigitsLbl != nullptr) {
        lv_obj_align(s_aodTimerDigitsLbl, LV_ALIGN_CENTER, -16, -4);
        if (s_aodTimerMsLbl != nullptr) {
          lv_obj_align_to(s_aodTimerMsLbl, s_aodTimerDigitsLbl, LV_ALIGN_OUT_RIGHT_BOTTOM, 2, -3);
        }
      }
      if (s_aodTimerSubLbl != nullptr) lv_obj_align(s_aodTimerSubLbl, LV_ALIGN_CENTER, 0, 18);
      if (s_aodTimerProgBar != nullptr) lv_obj_align(s_aodTimerProgBar, LV_ALIGN_CENTER, 0, 30);

      // Footnote chip at the bottom
      if (s_aodMediaFootnoteCard != nullptr) {
        char fnText[128];
        snprintf(fnText, sizeof(fnText), "%s \u2014 %s", d.mediaTitle,
                 d.mediaArtist[0] ? d.mediaArtist : "Unknown");
        if (s_aodFootnoteText != nullptr) {
          setLabelTextIfChanged(s_aodFootnoteText, fnText);
          lv_obj_set_style_text_color(s_aodFootnoteText, lv_color_hex(artistColor), LV_PART_MAIN);
        }
        if (s_aodFootnoteProgBar != nullptr) {
          uint32_t curPos = d.mediaPosMs;
          if (d.mediaState == MediaPlayState::Playing && d.mediaRxMillis > 0) {
            curPos = min(d.mediaDurMs, d.mediaPosMs + (nowMs - d.mediaRxMillis));
          }
          const int mPct = (d.mediaDurMs > 0)
                              ? static_cast<int>(min<uint32_t>(100, (curPos * 100) / d.mediaDurMs))
                              : 0;
          lv_bar_set_value(s_aodFootnoteProgBar, mPct, LV_ANIM_OFF);
          lv_obj_set_style_bg_color(s_aodFootnoteProgBar, lv_color_hex(aodProgColor), LV_PART_INDICATOR);
        }
        lv_obj_clear_flag(s_aodMediaFootnoteCard, LV_OBJ_FLAG_HIDDEN);
      }
    } else {
      // Full screen available: Hero size digits!
      if (s_aodTimerTagLbl != nullptr) lv_obj_align(s_aodTimerTagLbl, LV_ALIGN_CENTER, 0, -32);
      if (s_aodTimerDigitsLbl != nullptr) {
        lv_obj_align(s_aodTimerDigitsLbl, LV_ALIGN_CENTER, -16, 2);
        if (s_aodTimerMsLbl != nullptr) {
          lv_obj_align_to(s_aodTimerMsLbl, s_aodTimerDigitsLbl, LV_ALIGN_OUT_RIGHT_BOTTOM, 2, -3);
        }
      }
      if (s_aodTimerSubLbl != nullptr) lv_obj_align(s_aodTimerSubLbl, LV_ALIGN_CENTER, 0, 36);
      if (s_aodTimerProgBar != nullptr) lv_obj_align(s_aodTimerProgBar, LV_ALIGN_CENTER, 0, 48);

      if (s_aodMediaFootnoteCard != nullptr) {
        lv_obj_add_flag(s_aodMediaFootnoteCard, LV_OBJ_FLAG_HIDDEN);
      }
    }
    return;
  }

  // If clock tools are NOT active, hide timer & footnote elements
  if (s_aodTimerTagLbl != nullptr) lv_obj_add_flag(s_aodTimerTagLbl, LV_OBJ_FLAG_HIDDEN);
  if (s_aodTimerDigitsLbl != nullptr) lv_obj_add_flag(s_aodTimerDigitsLbl, LV_OBJ_FLAG_HIDDEN);
  if (s_aodTimerMsLbl != nullptr) lv_obj_add_flag(s_aodTimerMsLbl, LV_OBJ_FLAG_HIDDEN);
  if (s_aodTimerSubLbl != nullptr) lv_obj_add_flag(s_aodTimerSubLbl, LV_OBJ_FLAG_HIDDEN);
  if (s_aodTimerProgBar != nullptr) lv_obj_add_flag(s_aodTimerProgBar, LV_OBJ_FLAG_HIDDEN);
  if (s_aodMediaFootnoteCard != nullptr) lv_obj_add_flag(s_aodMediaFootnoteCard, LV_OBJ_FLAG_HIDDEN);

  // =========================================================================
  // Case B: Media Playing AOD (No Timer/Stopwatch)
  // =========================================================================
  if (showMedia) {
    lv_obj_set_style_text_font(s_aodClockLbl, Theme::fontDisplay(), LV_PART_MAIN);
    lv_obj_set_style_text_color(s_aodClockLbl, lv_color_hex(clockColor), LV_PART_MAIN);
    if (!use24 && validTime) {
      lv_label_set_text_fmt(s_aodClockLbl, "%02d:%02d %s", hr, mn, ampmStr);
    } else {
      lv_label_set_text_fmt(s_aodClockLbl, "%02d:%02d", hr, mn);
    }
    lv_obj_align(s_aodClockLbl, LV_ALIGN_TOP_LEFT, 20, 8);
    lv_obj_clear_flag(s_aodClockLbl, LV_OBJ_FLAG_HIDDEN);

    if (s_aodAmPmLbl != nullptr) {
      lv_obj_add_flag(s_aodAmPmLbl, LV_OBJ_FLAG_HIDDEN);
    }

    if (s_aodDateLbl != nullptr) {
      setLabelTextIfChanged(s_aodDateLbl, shortDateStr);
      lv_obj_set_style_text_font(s_aodDateLbl, Theme::fontCaption(), LV_PART_MAIN);
      lv_obj_set_style_text_color(s_aodDateLbl, lv_color_hex(dateColor), LV_PART_MAIN);
      lv_obj_align(s_aodDateLbl, LV_ALIGN_TOP_RIGHT, -20, 16);
      lv_obj_clear_flag(s_aodDateLbl, LV_OBJ_FLAG_HIDDEN);
    }

    lv_obj_clear_flag(s_aodArtBox, LV_OBJ_FLAG_HIDDEN);
    lv_obj_clear_flag(s_aodTitleLbl, LV_OBJ_FLAG_HIDDEN);
    lv_obj_clear_flag(s_aodArtistLbl, LV_OBJ_FLAG_HIDDEN);
    lv_obj_clear_flag(s_aodProgBar, LV_OBJ_FLAG_HIDDEN);

    if (d.mediaArtReady && DataStore::albumArtBuffer() != nullptr) {
      lv_obj_clear_flag(s_aodArtImg, LV_OBJ_FLAG_HIDDEN);
      lv_obj_add_flag(s_aodArtIcon, LV_OBJ_FLAG_HIDDEN);
      lv_obj_invalidate(s_aodArtImg);
    } else {
      lv_obj_add_flag(s_aodArtImg, LV_OBJ_FLAG_HIDDEN);
      lv_obj_clear_flag(s_aodArtIcon, LV_OBJ_FLAG_HIDDEN);
    }

    setLabelTextIfChanged(s_aodTitleLbl, d.mediaTitle);
    lv_obj_set_style_text_color(s_aodTitleLbl, lv_color_hex(titleColor), LV_PART_MAIN);
    setLabelTextIfChanged(s_aodArtistLbl, d.mediaArtist[0] ? d.mediaArtist : "Unknown Artist");
    lv_obj_set_style_text_color(s_aodArtistLbl, lv_color_hex(artistColor), LV_PART_MAIN);

    lv_obj_set_style_bg_color(s_aodProgBar, lv_color_hex(aodProgColor), LV_PART_INDICATOR);
    lv_obj_set_style_bg_color(s_aodProgBar, lv_color_hex(aodTrackColor), LV_PART_MAIN);
    lv_obj_set_style_bg_opa(s_aodProgBar, aodTrackOpa, LV_PART_MAIN);

    uint32_t curPos = d.mediaPosMs;
    if (d.mediaState == MediaPlayState::Playing && d.mediaRxMillis > 0) {
      curPos = min(d.mediaDurMs, d.mediaPosMs + (nowMs - d.mediaRxMillis));
    }
    const int pct = (d.mediaDurMs > 0)
                        ? static_cast<int>(min<uint32_t>(100, (curPos * 100) / d.mediaDurMs))
                        : 0;
    lv_bar_set_value(s_aodProgBar, pct, LV_ANIM_OFF);
  } else {
    // =======================================================================
    // Case C: Standard Centered Ambient Clock (No Media, No Timer)
    // =======================================================================
    lv_obj_set_style_text_font(s_aodClockLbl, Theme::fontHero(), LV_PART_MAIN);
    lv_obj_set_style_text_color(s_aodClockLbl, lv_color_hex(p.text), LV_PART_MAIN);
    lv_label_set_text_fmt(s_aodClockLbl, "%02d:%02d", hr, mn);

    const int16_t clockXOffset = (!use24 && validTime) ? -16 : 0;
    lv_obj_align(s_aodClockLbl, LV_ALIGN_CENTER, clockXOffset, -14);
    lv_obj_clear_flag(s_aodClockLbl, LV_OBJ_FLAG_HIDDEN);

    if (s_aodAmPmLbl != nullptr) {
      if (!use24 && validTime) {
        setLabelTextIfChanged(s_aodAmPmLbl, ampmStr);
        lv_obj_set_style_text_color(s_aodAmPmLbl, lv_color_hex(p.accent), LV_PART_MAIN);
        lv_obj_clear_flag(s_aodAmPmLbl, LV_OBJ_FLAG_HIDDEN);
        lv_obj_align_to(s_aodAmPmLbl, s_aodClockLbl, LV_ALIGN_OUT_RIGHT_BOTTOM, 6, -8);
      } else {
        lv_obj_add_flag(s_aodAmPmLbl, LV_OBJ_FLAG_HIDDEN);
      }
    }

    if (s_aodDateLbl != nullptr) {
      setLabelTextIfChanged(s_aodDateLbl, shortDateStr);
      lv_obj_set_style_text_font(s_aodDateLbl, Theme::fontBody(), LV_PART_MAIN);
      lv_obj_set_style_text_color(s_aodDateLbl, lv_color_hex(p.textDim), LV_PART_MAIN);
      lv_obj_align(s_aodDateLbl, LV_ALIGN_CENTER, 0, 26);
      lv_obj_clear_flag(s_aodDateLbl, LV_OBJ_FLAG_HIDDEN);
    }

    lv_obj_add_flag(s_aodArtBox, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(s_aodTitleLbl, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(s_aodArtistLbl, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(s_aodProgBar, LV_OBJ_FLAG_HIDDEN);
  }
}

// `keepBackdrop`: rebuilding an AOD screen that is already showing (theme or
// accent change) keeps the current tint, so it eases to the new target instead
// of snapping back to `bg` and fading in again.
void buildAodScreen(lv_obj_t *root, bool keepBackdrop = false) {
  StatusBar::setVisible(false);
  s_aodScreen = root;

  const ThemeTokens::Palette &p = Theme::dark();
  const DataSnapshot d = DataStore::getSnapshot();

  if (!keepBackdrop) {
    s_aodCurTop = Theme::active().bg;
    s_aodCurBot = Theme::active().bg;
  }
  s_aodFromTop = s_aodCurTop;
  s_aodFromBot = s_aodCurBot;
  s_aodToTop = s_aodCurTop;
  s_aodToBot = s_aodCurBot;
  s_aodWasPlaying =
      (d.mediaState != MediaPlayState::Stopped && d.mediaTitle[0] != '\0');
  s_aodPausedSinceMs = (d.mediaState == MediaPlayState::Paused) ? millis() : 0;

  lv_obj_set_style_bg_grad_dir(s_aodScreen, LV_GRAD_DIR_VER, LV_PART_MAIN);

  s_aodClockLbl = lv_label_create(root);
  lv_obj_set_style_text_color(s_aodClockLbl, lv_color_hex(p.text),
                              LV_PART_MAIN);

  s_aodAmPmLbl = lv_label_create(root);
  lv_obj_set_style_text_font(s_aodAmPmLbl, Theme::fontTitle(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodAmPmLbl, lv_color_hex(p.accent),
                              LV_PART_MAIN);

  s_aodDateLbl = lv_label_create(root);
  lv_obj_set_style_text_font(s_aodDateLbl, Theme::fontBody(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodDateLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);

  // Media playing AOD elements (96px art + scrolling title & artist + 3px
  // progress bar)
  s_aodArtBox =
      Theme::createCard(root, 96, 96, 20, 44, Config::RADIUS_SM, true);
  lv_obj_set_style_pad_all(s_aodArtBox, 0, LV_PART_MAIN);
  lv_obj_set_style_clip_corner(s_aodArtBox, true, LV_PART_MAIN);

  s_aodArtIcon = lv_label_create(s_aodArtBox);
  lv_label_set_text(s_aodArtIcon, ICON_MUSIC);
  lv_obj_set_style_text_font(s_aodArtIcon, Theme::iconFont32(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodArtIcon, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_center(s_aodArtIcon);

  s_aodArtDsc.header.always_zero = 0;
  s_aodArtDsc.header.w = DataStore::ALBUM_ART_DIM;
  s_aodArtDsc.header.h = DataStore::ALBUM_ART_DIM;
  s_aodArtDsc.header.cf = LV_IMG_CF_TRUE_COLOR;
  s_aodArtDsc.data_size = DataStore::ALBUM_ART_BYTES;
  s_aodArtDsc.data =
      reinterpret_cast<const uint8_t *>(DataStore::albumArtBuffer());

  s_aodArtImg = lv_img_create(s_aodArtBox);
  lv_img_set_src(s_aodArtImg, &s_aodArtDsc);
  lv_img_set_zoom(s_aodArtImg, 192); // 128 * (192/256) = 96 px
  lv_obj_center(s_aodArtImg);
  lv_obj_add_flag(s_aodArtImg, LV_OBJ_FLAG_HIDDEN);

  s_aodTitleLbl = lv_label_create(root);
  lv_obj_set_style_text_font(s_aodTitleLbl, Theme::fontTitle(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodTitleLbl, lv_color_hex(p.text),
                              LV_PART_MAIN);
  lv_label_set_long_mode(s_aodTitleLbl, LV_LABEL_LONG_SCROLL_CIRCULAR);
  lv_obj_set_style_anim_speed(s_aodTitleLbl, 28, LV_PART_MAIN);
  lv_obj_set_width(s_aodTitleLbl, 170);
  lv_obj_set_pos(s_aodTitleLbl, 130, 62);

  s_aodArtistLbl = lv_label_create(root);
  lv_obj_set_style_text_font(s_aodArtistLbl, Theme::fontBody(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodArtistLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_label_set_long_mode(s_aodArtistLbl, LV_LABEL_LONG_SCROLL_CIRCULAR);
  lv_obj_set_style_anim_speed(s_aodArtistLbl, 24, LV_PART_MAIN);
  lv_obj_set_width(s_aodArtistLbl, 170);
  lv_obj_set_pos(s_aodArtistLbl, 130, 92);

  s_aodProgBar = lv_bar_create(root);
  lv_obj_set_size(s_aodProgBar, 280, 3);
  lv_obj_set_pos(s_aodProgBar, 20, 154);
  lv_bar_set_range(s_aodProgBar, 0, 100);
  lv_obj_set_style_bg_color(s_aodProgBar, lv_color_hex(0xFFFFFF), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(s_aodProgBar, LV_OPA_30, LV_PART_MAIN);
  lv_obj_set_style_bg_color(s_aodProgBar, lv_color_hex(p.primary),
                            LV_PART_INDICATOR);

  // --- AOD Timer / Stopwatch widgets ---
  s_aodTimerTagLbl = lv_label_create(root);
  lv_obj_set_style_text_font(s_aodTimerTagLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodTimerTagLbl, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_add_flag(s_aodTimerTagLbl, LV_OBJ_FLAG_HIDDEN);

  s_aodTimerDigitsLbl = lv_label_create(root);
  lv_obj_set_style_text_font(s_aodTimerDigitsLbl, Theme::fontDisplay(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodTimerDigitsLbl, lv_color_hex(p.text), LV_PART_MAIN);
  lv_obj_add_flag(s_aodTimerDigitsLbl, LV_OBJ_FLAG_HIDDEN);

  s_aodTimerMsLbl = lv_label_create(root);
  lv_obj_set_style_text_font(s_aodTimerMsLbl, Theme::fontBody(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodTimerMsLbl, lv_color_hex(p.textDim), LV_PART_MAIN);
  lv_obj_add_flag(s_aodTimerMsLbl, LV_OBJ_FLAG_HIDDEN);

  s_aodTimerSubLbl = lv_label_create(root);
  lv_obj_set_style_text_font(s_aodTimerSubLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodTimerSubLbl, lv_color_hex(p.textDim), LV_PART_MAIN);
  lv_obj_add_flag(s_aodTimerSubLbl, LV_OBJ_FLAG_HIDDEN);

  s_aodTimerProgBar = lv_bar_create(root);
  lv_obj_set_size(s_aodTimerProgBar, 180, 2);
  lv_bar_set_range(s_aodTimerProgBar, 0, 100);
  lv_obj_set_style_bg_color(s_aodTimerProgBar, lv_color_hex(0x22333B), LV_PART_MAIN);
  lv_obj_set_style_bg_color(s_aodTimerProgBar, lv_color_hex(p.accent), LV_PART_INDICATOR);
  lv_obj_add_flag(s_aodTimerProgBar, LV_OBJ_FLAG_HIDDEN);

  // --- AOD Media Footnote Card (shown when Timer/Stopwatch is active AND Media is playing) ---
  s_aodMediaFootnoteCard = Theme::createCard(root, 300, 32, 10, 128, Config::RADIUS_SM, true);
  lv_obj_set_style_bg_color(s_aodMediaFootnoteCard, lv_color_hex(0x0C1317), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(s_aodMediaFootnoteCard, LV_OPA_70, LV_PART_MAIN);
  lv_obj_set_style_pad_all(s_aodMediaFootnoteCard, 6, LV_PART_MAIN);
  lv_obj_add_flag(s_aodMediaFootnoteCard, LV_OBJ_FLAG_HIDDEN);

  s_aodFootnoteIcon = lv_label_create(s_aodMediaFootnoteCard);
  lv_label_set_text(s_aodFootnoteIcon, ICON_MUSIC);
  lv_obj_set_style_text_font(s_aodFootnoteIcon, Theme::iconFont20(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodFootnoteIcon, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_align(s_aodFootnoteIcon, LV_ALIGN_LEFT_MID, 2, 0);

  s_aodFootnoteText = lv_label_create(s_aodMediaFootnoteCard);
  lv_label_set_long_mode(s_aodFootnoteText, LV_LABEL_LONG_DOT);
  lv_obj_set_width(s_aodFootnoteText, 256);
  lv_obj_set_style_text_font(s_aodFootnoteText, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodFootnoteText, lv_color_hex(0xD0DCE0), LV_PART_MAIN);
  lv_obj_align(s_aodFootnoteText, LV_ALIGN_LEFT_MID, 28, 0);

  s_aodFootnoteProgBar = lv_bar_create(s_aodMediaFootnoteCard);
  lv_obj_set_size(s_aodFootnoteProgBar, 290, 2);
  lv_obj_align(s_aodFootnoteProgBar, LV_ALIGN_BOTTOM_MID, 0, 2);
  lv_bar_set_range(s_aodFootnoteProgBar, 0, 100);
  lv_obj_set_style_bg_color(s_aodFootnoteProgBar, lv_color_hex(0xFFFFFF), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(s_aodFootnoteProgBar, LV_OPA_20, LV_PART_MAIN);
  lv_obj_set_style_bg_color(s_aodFootnoteProgBar, lv_color_hex(p.primary), LV_PART_INDICATOR);

  // --- Notification Alert Card for AOD mode ---
  s_aodAlertCard = Theme::createCard(root, 300, 140, 10, 15, Config::RADIUS_MD, true);
  lv_obj_set_style_bg_color(s_aodAlertCard, lv_color_hex(0x0C1317), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(s_aodAlertCard, LV_OPA_COVER, LV_PART_MAIN);
  lv_obj_set_style_border_color(s_aodAlertCard, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_set_style_border_width(s_aodAlertCard, 2, LV_PART_MAIN);
  lv_obj_set_style_pad_all(s_aodAlertCard, 8, LV_PART_MAIN);
  lv_obj_set_style_clip_corner(s_aodAlertCard, true, LV_PART_MAIN);
  lv_obj_add_flag(s_aodAlertCard, LV_OBJ_FLAG_HIDDEN);

  s_aodAlertIcon = lv_label_create(s_aodAlertCard);
  lv_label_set_text(s_aodAlertIcon, ICON_BELL);
  lv_obj_set_style_text_font(s_aodAlertIcon, Theme::iconFont24(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodAlertIcon, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_set_pos(s_aodAlertIcon, 2, 0);

  s_aodAlertHeaderLbl = lv_label_create(s_aodAlertCard);
  lv_obj_set_style_text_font(s_aodAlertHeaderLbl, Theme::fontBodySb(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodAlertHeaderLbl, lv_color_hex(0xFFFFFF), LV_PART_MAIN);
  lv_label_set_long_mode(s_aodAlertHeaderLbl, LV_LABEL_LONG_DOT);
  lv_obj_set_width(s_aodAlertHeaderLbl, 206);
  lv_obj_set_pos(s_aodAlertHeaderLbl, 34, 1);

  s_aodAlertBadgeLbl = lv_label_create(s_aodAlertCard);
  lv_obj_set_style_text_font(s_aodAlertBadgeLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodAlertBadgeLbl, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_label_set_text(s_aodAlertBadgeLbl, "NOW");
  lv_obj_align(s_aodAlertBadgeLbl, LV_ALIGN_TOP_RIGHT, -2, 2);

  s_aodAlertTitleLbl = lv_label_create(s_aodAlertCard);
  lv_obj_set_style_text_font(s_aodAlertTitleLbl, Theme::fontBodySb(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodAlertTitleLbl, lv_color_hex(0xFFFFFF), LV_PART_MAIN);
  lv_label_set_long_mode(s_aodAlertTitleLbl, LV_LABEL_LONG_DOT);
  lv_obj_set_width(s_aodAlertTitleLbl, 276);
  lv_obj_set_pos(s_aodAlertTitleLbl, 4, 30);

  s_aodAlertBodyLbl = lv_label_create(s_aodAlertCard);
  lv_obj_set_style_text_font(s_aodAlertBodyLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(s_aodAlertBodyLbl, lv_color_hex(0xD0DCE0), LV_PART_MAIN);
  lv_label_set_long_mode(s_aodAlertBodyLbl, LV_LABEL_LONG_WRAP);
  lv_obj_set_width(s_aodAlertBodyLbl, 276);
  lv_obj_set_pos(s_aodAlertBodyLbl, 4, 54);

  s_aodLastSec = 0xFFFFFFFF;
  refreshAodContent();
}

} // namespace

void begin() {
  s_appCount = 0;
  s_activeAppIdx = 0;
  s_launcherCursor = 0;
  s_ccCursor = 0;
  s_viewState = ViewState::InApp;
  s_lastInputMs = millis();
  s_switchCounter = 0;
  clearViewPointers();
}

bool registerApp(App *app) {
  if (app == nullptr || s_appCount >= Config::MAX_REGISTERED_APPS) {
    return false;
  }
  s_apps[s_appCount++] = app;
  Serial.printf("[AppManager] Registered app [%u]: %s\n",
                static_cast<unsigned>(s_appCount - 1), app->name());
  return true;
}

void switchTo(size_t idx) {
  if (idx >= s_appCount) {
    return;
  }

  if (s_viewState == ViewState::InApp && s_activeAppIdx >= 0 &&
      static_cast<size_t>(s_activeAppIdx) < s_appCount) {
    s_apps[s_activeAppIdx]->onExit();
  }

  clearViewPointers();
  s_viewState = ViewState::InApp;
  s_activeAppIdx = static_cast<int>(idx);
  s_launcherCursor = idx;
  ++s_switchCounter;

  App *nextApp = s_apps[idx];
  StatusBar::setVisible(!nextApp->hidesStatusBar());
  StatusBar::setTitle(nextApp->name());

  lv_obj_t *newScr = createAndSwapScreen(nextApp->usesWallpaper(), false);
  nextApp->onEnter(newScr);
}

void showLauncher() {
  if (s_viewState == ViewState::InApp && s_activeAppIdx >= 0 &&
      static_cast<size_t>(s_activeAppIdx) < s_appCount) {
    s_apps[s_activeAppIdx]->onExit();
  }

  clearViewPointers();
  s_viewState = ViewState::Launcher;

  lv_obj_t *newScr = createAndSwapScreen(true, false);
  buildLauncherScreen(newScr);
  Serial.println("[AppManager] Entered Redmi-style Carousel Launcher");
}

void openControlCenter() {
  if (s_viewState == ViewState::ControlCenter) {
    closeControlCenter();
    return;
  }

  if (s_viewState == ViewState::InApp && s_activeAppIdx >= 0 &&
      static_cast<size_t>(s_activeAppIdx) < s_appCount) {
    s_apps[s_activeAppIdx]->onExit();
  }

  s_preOverlayView = (s_viewState == ViewState::Launcher) ? ViewState::Launcher
                                                          : ViewState::InApp;
  clearViewPointers();
  s_viewState = ViewState::ControlCenter;

  lv_obj_t *newScr = createAndSwapScreen(true, false);
  buildControlCenterScreen(newScr);
  Serial.println("[AppManager] Opened Control Center");
}

void closeControlCenter() {
  if (s_preOverlayView == ViewState::Launcher) {
    showLauncher();
  } else {
    switchTo(static_cast<size_t>(s_activeAppIdx >= 0 ? s_activeAppIdx : 0));
  }
}

void enterAod() {
  if (s_viewState == ViewState::Aod)
    return;

  StatusBar::dismissNotificationPopup();

  if (s_viewState == ViewState::InApp && s_activeAppIdx >= 0 &&
      static_cast<size_t>(s_activeAppIdx) < s_appCount) {
    s_apps[s_activeAppIdx]->onExit();
  }

  s_preOverlayView = (s_viewState == ViewState::Launcher) ? ViewState::Launcher
                                                          : ViewState::InApp;
  clearViewPointers();
  s_viewState = ViewState::Aod;

  lv_obj_t *newScr = createAndSwapScreen(false, true);
  buildAodScreen(newScr);
  Display::setBrightness(Config::AOD_BRIGHTNESS_STEP);
  Serial.println("[AppManager] Entered AOD mode");
}

void exitAod() {
  if (s_viewState != ViewState::Aod)
    return;
  dismissAodNotificationAlert();
  s_lastInputMs = millis();
  Display::setBrightness(Settings::getBrightnessStep());
  Serial.println("[AppManager] Woke from AOD mode");

  if (s_preOverlayView == ViewState::Launcher) {
    showLauncher();
  } else {
    switchTo(static_cast<size_t>(s_activeAppIdx >= 0 ? s_activeAppIdx : 0));
  }
}

bool isInAod() { return s_viewState == ViewState::Aod; }

void showAodNotificationAlert(const char *app, const char *title,
                              const char *body, uint8_t count) {
  if (s_viewState != ViewState::Aod || s_aodScreen == nullptr)
    return;

  s_aodAlertActive = true;
  s_aodAlertStartMs = millis();

  // Cancel any running background tint animation
  lv_anim_del(s_aodScreen, aodAnimExecCb);

  const ThemeTokens::Palette &p = Theme::dark();

  char cleanApp[32] = {0};
  char cleanTitle[64] = {0};
  char cleanBody[160] = {0};
  Theme::sanitizeText(app, cleanApp, sizeof(cleanApp));
  Theme::sanitizeText(title, cleanTitle, sizeof(cleanTitle));
  Theme::sanitizeText(body, cleanBody, sizeof(cleanBody));

  const char *appName =
      (cleanApp[0] != '\0')
          ? cleanApp
          : ((cleanTitle[0] != '\0') ? cleanTitle : "Notification");

  const char *iconGlyph = ICON_BELL;
  if ((cleanApp[0] != '\0' && (strstr(cleanApp, "Spotify") != nullptr ||
                               strstr(cleanApp, "Music") != nullptr)) ||
      (cleanTitle[0] != '\0' && (strstr(cleanTitle, "Spotify") != nullptr ||
                                 strstr(cleanTitle, "Music") != nullptr))) {
    iconGlyph = ICON_MUSIC;
  }

  if (s_aodAlertIcon != nullptr) {
    lv_label_set_text(s_aodAlertIcon, iconGlyph);
    lv_obj_set_style_text_color(s_aodAlertIcon, lv_color_hex(p.accent),
                                LV_PART_MAIN);
  }

  if (s_aodAlertHeaderLbl != nullptr) {
    char headerBuf[64];
    snprintf(headerBuf, sizeof(headerBuf), "(%u) New Notification \"%s\"",
             (count > 0 ? count : 1), appName);
    lv_label_set_text(s_aodAlertHeaderLbl, headerBuf);
  }

  const bool hasDistinctTitle =
      (cleanTitle[0] != '\0' && strcmp(cleanTitle, appName) != 0);

  if (hasDistinctTitle) {
    if (s_aodAlertTitleLbl != nullptr) {
      lv_label_set_text(s_aodAlertTitleLbl, cleanTitle);
      lv_obj_clear_flag(s_aodAlertTitleLbl, LV_OBJ_FLAG_HIDDEN);
    }
    if (s_aodAlertBodyLbl != nullptr) {
      if (cleanBody[0] != '\0') {
        lv_obj_set_pos(s_aodAlertBodyLbl, 4, 54);
        lv_label_set_text(s_aodAlertBodyLbl, cleanBody);
        lv_obj_clear_flag(s_aodAlertBodyLbl, LV_OBJ_FLAG_HIDDEN);
      } else {
        lv_obj_add_flag(s_aodAlertBodyLbl, LV_OBJ_FLAG_HIDDEN);
      }
    }
  } else {
    if (s_aodAlertTitleLbl != nullptr) {
      lv_obj_add_flag(s_aodAlertTitleLbl, LV_OBJ_FLAG_HIDDEN);
    }
    if (s_aodAlertBodyLbl != nullptr) {
      lv_obj_set_pos(s_aodAlertBodyLbl, 4, 32);
      if (cleanBody[0] != '\0') {
        lv_label_set_text(s_aodAlertBodyLbl, cleanBody);
      } else {
        lv_label_set_text(s_aodAlertBodyLbl, "New notification received");
      }
      lv_obj_clear_flag(s_aodAlertBodyLbl, LV_OBJ_FLAG_HIDDEN);
    }
  }

  // Hide all clock & media elements on AOD screen
  if (s_aodClockLbl != nullptr)
    lv_obj_add_flag(s_aodClockLbl, LV_OBJ_FLAG_HIDDEN);
  if (s_aodAmPmLbl != nullptr)
    lv_obj_add_flag(s_aodAmPmLbl, LV_OBJ_FLAG_HIDDEN);
  if (s_aodDateLbl != nullptr)
    lv_obj_add_flag(s_aodDateLbl, LV_OBJ_FLAG_HIDDEN);
  if (s_aodArtBox != nullptr)
    lv_obj_add_flag(s_aodArtBox, LV_OBJ_FLAG_HIDDEN);
  if (s_aodTitleLbl != nullptr)
    lv_obj_add_flag(s_aodTitleLbl, LV_OBJ_FLAG_HIDDEN);
  if (s_aodArtistLbl != nullptr)
    lv_obj_add_flag(s_aodArtistLbl, LV_OBJ_FLAG_HIDDEN);
  if (s_aodProgBar != nullptr)
    lv_obj_add_flag(s_aodProgBar, LV_OBJ_FLAG_HIDDEN);

  // Show alert card
  if (s_aodAlertCard != nullptr) {
    lv_obj_clear_flag(s_aodAlertCard, LV_OBJ_FLAG_HIDDEN);
    lv_obj_move_foreground(s_aodAlertCard);
  }

  // Maintain dim backlight for AOD
  Display::setBrightness(Config::AOD_BRIGHTNESS_STEP);
  lv_obj_set_style_bg_color(s_aodScreen, lv_color_hex(0x000000), LV_PART_MAIN);
  lv_obj_set_style_bg_grad_dir(s_aodScreen, LV_GRAD_DIR_NONE, LV_PART_MAIN);
  lv_obj_invalidate(s_aodScreen);
}

void dismissAodNotificationAlert() {
  if (!s_aodAlertActive)
    return;
  s_aodAlertActive = false;

  if (s_aodAlertCard != nullptr) {
    lv_obj_add_flag(s_aodAlertCard, LV_OBJ_FLAG_HIDDEN);
  }

  refreshAodContent();
}
bool isAodAlertActive() { return s_aodAlertActive; }

void resetActivity() { s_lastInputMs = millis(); }

void applyActiveTheme() {
  StatusBar::applyTheme();
  if (s_viewState == ViewState::ControlCenter) {
    lv_obj_t *newScr = createAndSwapScreen(true, false);
    buildControlCenterScreen(newScr);
  } else if (s_viewState == ViewState::Launcher) {
    showLauncher();
  } else if (s_viewState == ViewState::InApp) {
    switchTo(static_cast<size_t>(s_activeAppIdx >= 0 ? s_activeAppIdx : 0));
  } else if (s_viewState == ViewState::Aod) {
    if (s_aodScreen != nullptr) {
      // buildAodScreen() creates every widget, so drop the old ones first;
      // otherwise both sets stay on screen, overlapping.
      lv_obj_t *scr = s_aodScreen;
      lv_anim_del(scr, nullptr);
      lv_obj_clean(scr);
      clearViewPointers(); // also nulls s_aodScreen; rebuilt below
      buildAodScreen(scr, true);
    }
  }
}

bool activeAppWantsDouble(Button b) {
  if (s_viewState == ViewState::InApp && s_activeAppIdx >= 0 &&
      static_cast<size_t>(s_activeAppIdx) < s_appCount) {
    return s_apps[s_activeAppIdx]->wantsDouble(b);
  }
  return false;
}

void tick(uint32_t nowMs) {
  DataStore::tickClock(nowMs);
  if (s_lastInputMs == 0 || nowMs < s_lastInputMs) {
    s_lastInputMs = nowMs;
  }

  // Keep idle timer awake while notification popup is actively displayed
  if (StatusBar::isPopupVisible() || s_aodAlertActive) {
    s_lastInputMs = nowMs;
  }

  // Check AOD idle timeout (DESIGN.md Section 10)
  if (s_viewState != ViewState::Aod && Settings::getAodEnabled()) {
    const uint32_t timeoutMs =
        static_cast<uint32_t>(Settings::getAodTimeoutSec()) * 1000UL;
    if ((nowMs - s_lastInputMs) >= timeoutMs) {
      enterAod();
      return;
    }
  }

  if (s_viewState == ViewState::Aod) {
    if (s_aodAlertActive) {
      if (nowMs >= s_aodAlertStartMs &&
          (nowMs - s_aodAlertStartMs >= kAodAlertDurationMs)) {
        dismissAodNotificationAlert();
      }
      return; // Suspend clock/media rendering while notification alert is
              // displayed
    }
    const bool clockToolsRunning = DataStore::isClockToolRunning();
    if (clockToolsRunning) {
      if (nowMs - s_aodLastSubSecMs >= 50) {
        s_aodLastSubSecMs = nowMs;
        refreshAodContent();
      }
    } else {
      const uint32_t nowSec = nowMs / 1000;
      if (nowSec != s_aodLastSec) {
        s_aodLastSec = nowSec;
        refreshAodContent();
      }
    }
    return;
  }

  if (s_viewState == ViewState::InApp && s_activeAppIdx >= 0 &&
      static_cast<size_t>(s_activeAppIdx) < s_appCount) {
    s_apps[s_activeAppIdx]->onTick(nowMs);
  }
}

void handleButton(Button b, Press p) {
  s_lastInputMs = millis();

  // 1. Any press in AOD:
  // - If alert active, dismiss alert
  // - If timer/stopwatch is active and Button::Select pressed, toggle play/pause directly in AOD!
  // - Otherwise exit AOD
  if (s_viewState == ViewState::Aod) {
    if (s_aodAlertActive) {
      dismissAodNotificationAlert();
      return;
    }

    const DataSnapshot d = DataStore::getSnapshot();
    const bool timerActive =
        (d.timer.state == TimerState::Running || d.timer.state == TimerState::Paused);
    const bool stopwatchActive =
        (d.stopwatch.state == StopwatchState::Running || d.stopwatch.state == StopwatchState::Paused);

    if ((timerActive || stopwatchActive) && b == Button::Select && p == Press::Short) {
      if (timerActive) {
        DataStore::toggleTimer();
        CompanionLink::sendClockCommand("timer_toggle");
      } else {
        DataStore::toggleStopwatch();
        CompanionLink::sendClockCommand("stopwatch_toggle");
      }
      refreshAodContent();
      return; // Handled directly in AOD without waking or exiting!
    }

    exitAod();
    return;
  }

  // 2. Chord opens/closes Control Center from anywhere (DESIGN.md Section 8 &
  // 9.7)
  if (p == Press::Chord) {
    openControlCenter();
    return;
  }

  // 3. Control Center navigation
  if (s_viewState == ViewState::ControlCenter) {
    if (b == Button::Next && p == Press::Short) {
      s_ccCursor = (s_ccCursor + 1) % 4;
      refreshControlCenterTiles();
    } else if (b == Button::Select && p == Press::Short) {
      if (s_ccCursor == 0) {
        Settings::cycleBrightnessPct();
        Display::setBrightness(Settings::getBrightnessStep());
        refreshControlCenterTiles();
      } else if (s_ccCursor == 1) {
        Settings::toggleDarkLightTheme();
        applyActiveTheme();
      } else if (s_ccCursor == 2) {
        WifiService::forceReconnect();
        refreshControlCenterTiles();
      } else if (s_ccCursor == 3) {
        enterAod();
      }
    } else if ((b == Button::Select && p == Press::Long) ||
               (b == Button::Next && p == Press::Long)) {
      closeControlCenter();
    }
    return;
  }

  // 4. Launcher navigation (DESIGN.md Section 8 & 9.2)
  if (s_viewState == ViewState::Launcher) {
    if (s_appCount == 0)
      return;
    if (b == Button::Next && p == Press::Short) {
      s_launcherCursor = (s_launcherCursor + 1) % s_appCount;
      refreshLauncherCarousel();
      Serial.printf("[AppManager] Launcher focus -> [%u] %s\n",
                    static_cast<unsigned>(s_launcherCursor),
                    s_apps[s_launcherCursor]->name());
    } else if (b == Button::Select && p == Press::Short) {
      Serial.printf("[AppManager] Opening [%u]: %s\n",
                    static_cast<unsigned>(s_launcherCursor),
                    s_apps[s_launcherCursor]->name());
      switchTo(s_launcherCursor);
    } else if (b == Button::Select && p == Press::Long) {
      switchTo(0);
    }
    return;
  }

  // 5. Active App navigation (DESIGN.md Section 8)
  if (s_activeAppIdx >= 0 && static_cast<size_t>(s_activeAppIdx) < s_appCount) {
    App *activeApp = s_apps[s_activeAppIdx];
    if (activeApp->onButton(b, p)) {
      return;
    }

    // Default unconsumed gestures:
    // - Next (Long):   Open Launcher
    // - Select (Long): Back (go to Home if in another app, or Launcher if on
    // Home)
    // - Next (Short):  Cycle next app
    if (b == Button::Next && p == Press::Long) {
      showLauncher();
    } else if (b == Button::Select && p == Press::Long) {
      if (s_activeAppIdx != 0) {
        switchTo(0);
      } else {
        showLauncher();
      }
    } else if (b == Button::Next && p == Press::Short && s_appCount > 1) {
      const size_t nextIdx =
          (static_cast<size_t>(s_activeAppIdx) + 1) % s_appCount;
      switchTo(nextIdx);
    }
  }
}

void handleData(const DataSnapshot &d) {
  if (s_viewState == ViewState::InApp && s_activeAppIdx >= 0 &&
      static_cast<size_t>(s_activeAppIdx) < s_appCount) {
    s_apps[s_activeAppIdx]->onData(d);
  } else if (s_viewState == ViewState::ControlCenter) {
    refreshControlCenterTiles();
  } else if (s_viewState == ViewState::Aod) {
    if (s_aodAlertActive) {
      return; // Do not overwrite alert with incoming data snapshots
    }
    refreshAodContent();
  }
}

void runStressTest(uint16_t switches) {
  if (s_appCount == 0)
    return;

  s_lastInputMs = millis();
  switchTo(0);
  lv_timer_handler();

  const size_t sramBefore = heap_caps_get_free_size(MALLOC_CAP_INTERNAL);
  const size_t psramBefore = heap_caps_get_free_size(MALLOC_CAP_SPIRAM);
  lv_mem_monitor_t lvMemBefore{};
  lv_mem_monitor(&lvMemBefore);

  Serial.printf("\n[StressTest] Starting %u app switches...\n",
                static_cast<unsigned>(switches));
  Kernel::logMemoryStats("stress-before");

  const uint32_t startMs = millis();
  for (uint16_t i = 1; i <= switches; ++i) {
    const size_t target = i % s_appCount;
    switchTo(target);
    s_apps[target]->onTick(millis());
    lv_timer_handler();
  }
  const uint32_t elapsedMs = millis() - startMs;

  const size_t sramAfter = heap_caps_get_free_size(MALLOC_CAP_INTERNAL);
  const size_t psramAfter = heap_caps_get_free_size(MALLOC_CAP_SPIRAM);
  lv_mem_monitor_t lvMemAfter{};
  lv_mem_monitor(&lvMemAfter);

  const int32_t sramDelta =
      static_cast<int32_t>(sramAfter) - static_cast<int32_t>(sramBefore);
  const int32_t psramDelta =
      static_cast<int32_t>(psramAfter) - static_cast<int32_t>(psramBefore);
  const int32_t lvMemDelta = static_cast<int32_t>(lvMemAfter.free_size) -
                             static_cast<int32_t>(lvMemBefore.free_size);

  Kernel::logMemoryStats("stress-after");
  Serial.printf("[StressTest] Completed %u switches in %lums | SRAM delta=%ldB "
                "| PSRAM delta=%ldB | LVGL heap delta=%ldB\n\n",
                static_cast<unsigned>(switches),
                static_cast<unsigned long>(elapsedMs),
                static_cast<long>(sramDelta), static_cast<long>(psramDelta),
                static_cast<long>(lvMemDelta));

  showLauncher();
}

size_t appCount() { return s_appCount; }

int activeAppIndex() {
  return (s_viewState == ViewState::InApp) ? s_activeAppIdx : -1;
}

} // namespace AppManager
