#include "core/Kernel.h"
#include "apps/ClockApp.h"
#include "config.h"
#include "core/App.h"
#include "core/AppManager.h"
#include "core/DataStore.h"
#include "core/EventBus.h"
#include "core/Settings.h"
#include "drivers/Buttons.h"
#include "drivers/Display.h"
#include "services/WifiService.h"
#include "ui/StatusBar.h"
#include "ui/Theme.h"
#include "ui/icons.h"

#include <Arduino.h>
#include <esp_heap_caps.h>
#include <freertos/FreeRTOS.h>
#include <freertos/task.h>
#include <lvgl.h>

namespace Kernel {

namespace {

TaskHandle_t s_uiTaskHandle = nullptr;

// =============================================================================
// On-device Memory & System Diagnostics App (Styled with DESIGN.md tokens)
// =============================================================================
class SysMonitorApp final : public App {
public:
  const char *name() const override { return "Stats"; }
  const char *icon() const override { return ICON_ACTIVITY; }

  void onEnter(lv_obj_t *root) override {
    const ThemeTokens::Palette &p = Theme::active();

    // Left Column: Two stacked cards (SRAM & PSRAM) per DESIGN.md Section 9.4
    // layout
    lv_obj_t *sramCard = Theme::createCard(
        root, 144, 62, 10, Config::CONTENT_Y_START + 8, Config::RADIUS_MD);
    lv_obj_t *sramTitle = lv_label_create(sramCard);
    lv_label_set_text(sramTitle, "SRAM");
    lv_obj_set_style_text_font(sramTitle, Theme::fontCaption(), LV_PART_MAIN);
    lv_obj_set_style_text_color(sramTitle, lv_color_hex(p.textDim),
                                LV_PART_MAIN);
    lv_obj_align(sramTitle, LV_ALIGN_TOP_LEFT, 2, 0);

    m_sramValLbl = lv_label_create(sramCard);
    lv_label_set_text(m_sramValLbl, "--%");
    lv_obj_set_style_text_font(m_sramValLbl, Theme::fontDisplay(),
                               LV_PART_MAIN);
    lv_obj_set_style_text_color(m_sramValLbl, lv_color_hex(p.text),
                                LV_PART_MAIN);
    lv_obj_align(m_sramValLbl, LV_ALIGN_TOP_RIGHT, -2, -2);

    m_sramBar = lv_bar_create(sramCard);
    lv_obj_set_size(m_sramBar, 124, 4);
    lv_obj_align(m_sramBar, LV_ALIGN_BOTTOM_MID, 0, 0);
    lv_bar_set_range(m_sramBar, 0, 100);
    lv_obj_set_style_radius(m_sramBar, LV_RADIUS_CIRCLE, LV_PART_MAIN);
    lv_obj_set_style_radius(m_sramBar, LV_RADIUS_CIRCLE, LV_PART_INDICATOR);
    lv_obj_set_style_bg_color(m_sramBar, lv_color_hex(p.surface2),
                              LV_PART_MAIN);
    lv_obj_set_style_bg_color(m_sramBar, lv_color_hex(p.primary),
                              LV_PART_INDICATOR);

    lv_obj_t *psramCard = Theme::createCard(
        root, 144, 62, 10, Config::CONTENT_Y_START + 78, Config::RADIUS_MD);
    lv_obj_t *psramTitle = lv_label_create(psramCard);
    lv_label_set_text(psramTitle, "PSRAM");
    lv_obj_set_style_text_font(psramTitle, Theme::fontCaption(), LV_PART_MAIN);
    lv_obj_set_style_text_color(psramTitle, lv_color_hex(p.textDim),
                                LV_PART_MAIN);
    lv_obj_align(psramTitle, LV_ALIGN_TOP_LEFT, 2, 0);

    m_psramValLbl = lv_label_create(psramCard);
    lv_label_set_text(m_psramValLbl, "--%");
    lv_obj_set_style_text_font(m_psramValLbl, Theme::fontDisplay(),
                               LV_PART_MAIN);
    lv_obj_set_style_text_color(m_psramValLbl, lv_color_hex(p.text),
                                LV_PART_MAIN);
    lv_obj_align(m_psramValLbl, LV_ALIGN_TOP_RIGHT, -2, -2);

    m_psramBar = lv_bar_create(psramCard);
    lv_obj_set_size(m_psramBar, 124, 4);
    lv_obj_align(m_psramBar, LV_ALIGN_BOTTOM_MID, 0, 0);
    lv_bar_set_range(m_psramBar, 0, 100);
    lv_obj_set_style_radius(m_psramBar, LV_RADIUS_CIRCLE, LV_PART_MAIN);
    lv_obj_set_style_radius(m_psramBar, LV_RADIUS_CIRCLE, LV_PART_INDICATOR);
    lv_obj_set_style_bg_color(m_psramBar, lv_color_hex(p.surface2),
                              LV_PART_MAIN);
    lv_obj_set_style_bg_color(m_psramBar, lv_color_hex(p.primary),
                              LV_PART_INDICATOR);

    // Right Wide Card: Device Uptime & Network Summary
    lv_obj_t *rightCard = Theme::createCard(
        root, 148, 132, 162, Config::CONTENT_Y_START + 8, Config::RADIUS_MD);
    lv_obj_t *rightTitle = lv_label_create(rightCard);
    lv_label_set_text(rightTitle, "DEVICE");
    lv_obj_set_style_text_font(rightTitle, Theme::fontCaption(), LV_PART_MAIN);
    lv_obj_set_style_text_color(rightTitle, lv_color_hex(p.textDim),
                                LV_PART_MAIN);
    lv_obj_align(rightTitle, LV_ALIGN_TOP_LEFT, 2, 0);

    m_uptimeLbl = lv_label_create(rightCard);
    lv_label_set_text(m_uptimeLbl, "00:00:00");
    lv_obj_set_style_text_font(m_uptimeLbl, Theme::fontTitle(), LV_PART_MAIN);
    lv_obj_set_style_text_color(m_uptimeLbl, lv_color_hex(p.accent),
                                LV_PART_MAIN);
    lv_obj_align(m_uptimeLbl, LV_ALIGN_TOP_LEFT, 2, 24);

    m_detailLbl = lv_label_create(rightCard);
    lv_label_set_text(m_detailLbl, "SRAM -- KB\nPSRAM -- KB");
    lv_obj_set_style_text_font(m_detailLbl, Theme::fontCaption(), LV_PART_MAIN);
    lv_obj_set_style_text_color(m_detailLbl, lv_color_hex(p.textDim),
                                LV_PART_MAIN);
    lv_obj_align(m_detailLbl, LV_ALIGN_BOTTOM_LEFT, 2, -2);

    onTick(millis());
  }

  void onExit() override {
    m_sramValLbl = nullptr;
    m_sramBar = nullptr;
    m_psramValLbl = nullptr;
    m_psramBar = nullptr;
    m_uptimeLbl = nullptr;
    m_detailLbl = nullptr;
  }

  void onTick(uint32_t nowMs) override {
    if (m_sramValLbl == nullptr)
      return;

    const size_t sramFree = heap_caps_get_free_size(MALLOC_CAP_INTERNAL);
    const size_t sramTotal = heap_caps_get_total_size(MALLOC_CAP_INTERNAL);
    const int sramPct =
        (sramTotal > 0)
            ? static_cast<int>(((sramTotal - sramFree) * 100) / sramTotal)
            : 0;
    lv_label_set_text_fmt(m_sramValLbl, "%d%%", sramPct);
    lv_bar_set_value(m_sramBar, sramPct, LV_ANIM_OFF);

    const size_t psramFree = heap_caps_get_free_size(MALLOC_CAP_SPIRAM);
    const size_t psramTotal = heap_caps_get_total_size(MALLOC_CAP_SPIRAM);
    const int psramPct =
        (psramTotal > 0)
            ? static_cast<int>(((psramTotal - psramFree) * 100) / psramTotal)
            : 0;
    lv_label_set_text_fmt(m_psramValLbl, "%d%%", psramPct);
    lv_bar_set_value(m_psramBar, psramPct, LV_ANIM_OFF);

    const uint32_t sec = nowMs / 1000;
    lv_label_set_text_fmt(m_uptimeLbl, "%02lu:%02lu:%02lu",
                          static_cast<unsigned long>(sec / 3600),
                          static_cast<unsigned long>((sec % 3600) / 60),
                          static_cast<unsigned long>(sec % 60));

    lv_label_set_text_fmt(
        m_detailLbl, "SRAM: %u KB free\nPSRAM: %u KB free\nChord: Controls",
        static_cast<unsigned>(sramFree / 1024),
        static_cast<unsigned>(psramFree / 1024));
  }

private:
  lv_obj_t *m_sramValLbl = nullptr;
  lv_obj_t *m_sramBar = nullptr;
  lv_obj_t *m_psramValLbl = nullptr;
  lv_obj_t *m_psramBar = nullptr;
  lv_obj_t *m_uptimeLbl = nullptr;
  lv_obj_t *m_detailLbl = nullptr;
};

// =============================================================================
// On-device Minimal Settings App (DESIGN.md Section 9.8)
// =============================================================================
class SettingsPlaceholderApp final : public App {
public:
  const char *name() const override { return "Settings"; }
  const char *icon() const override { return ICON_SETTINGS; }

  void onEnter(lv_obj_t *root) override {
    const ThemeTokens::Palette &p = Theme::active();

    for (int i = 0; i < 3; ++i) {
      m_rows[i] = Theme::createCard(root, 300, 38, 10,
                                    Config::CONTENT_Y_START + 8 +
                                        static_cast<int16_t>(i * 44),
                                    Config::RADIUS_SM);
      lv_obj_set_style_pad_hor(m_rows[i], 12, LV_PART_MAIN);

      m_rowNames[i] = lv_label_create(m_rows[i]);
      lv_obj_set_style_text_font(m_rowNames[i], Theme::fontBodySb(),
                                 LV_PART_MAIN);
      lv_obj_set_style_text_color(m_rowNames[i], lv_color_hex(p.text),
                                  LV_PART_MAIN);
      lv_obj_align(m_rowNames[i], LV_ALIGN_LEFT_MID, 0, 0);

      m_rowVals[i] = lv_label_create(m_rows[i]);
      lv_obj_set_style_text_font(m_rowVals[i], Theme::fontBody(), LV_PART_MAIN);
      lv_obj_set_style_text_color(m_rowVals[i], lv_color_hex(p.accent),
                                  LV_PART_MAIN);
      lv_obj_align(m_rowVals[i], LV_ALIGN_RIGHT_MID, 0, 0);
    }

    refreshRows();
  }

  void onExit() override {
    for (int i = 0; i < 3; ++i) {
      m_rows[i] = nullptr;
      m_rowNames[i] = nullptr;
      m_rowVals[i] = nullptr;
    }
  }

  bool onButton(Button b, Press p) override {
    if (b == Button::Next && p == Press::Short) {
      m_cursor = (m_cursor + 1) % 3;
      refreshRows();
      return true;
    }
    if (b == Button::Select && p == Press::Short) {
      if (m_cursor == 0) {
        Settings::cycleBrightnessPct();
        Display::setBrightness(Settings::getBrightnessStep());
        refreshRows();
      } else if (m_cursor == 1) {
        Settings::toggleDarkLightTheme();
        AppManager::applyActiveTheme();
      } else if (m_cursor == 2) {
        Settings::setReduceMotion(!Settings::getReduceMotion());
        refreshRows();
      }
      return true;
    }
    return false;
  }

private:
  void refreshRows() {
    if (m_rows[0] == nullptr)
      return;
    lv_label_set_text(m_rowNames[0], "Brightness");
    lv_label_set_text_fmt(m_rowVals[0], "%u%%",
                          static_cast<unsigned>(Settings::getBrightnessPct()));

    lv_label_set_text(m_rowNames[1], "Theme");
    lv_label_set_text(m_rowVals[1], Theme::isDark() ? "Dark" : "Light");

    lv_label_set_text(m_rowNames[2], "Reduce Motion");
    lv_label_set_text(m_rowVals[2], Settings::getReduceMotion() ? "On" : "Off");

    for (int i = 0; i < 3; ++i) {
      Theme::setFocusRing(m_rows[i], i == m_cursor);
    }
  }

  lv_obj_t *m_rows[3] = {nullptr};
  lv_obj_t *m_rowNames[3] = {nullptr};
  lv_obj_t *m_rowVals[3] = {nullptr};
  int m_cursor = 0;
};

ClockApp s_homeApp;
SysMonitorApp s_statsApp;
SettingsPlaceholderApp s_settingsApp;

void drainEvents() {
  Event ev{};
  while (EventBus::poll(ev)) {
    switch (ev.type) {
    case EventType::ButtonPressed:
      AppManager::handleButton(ev.btn.button, ev.btn.press);
      break;
    case EventType::DataUpdated: {
      const DataSnapshot snap = DataStore::getSnapshot();
      StatusBar::updateData(snap);
      AppManager::handleData(snap);
      break;
    }
    case EventType::RunStressTest:
      AppManager::runStressTest(ev.stressIterations);
      break;
    }
  }
}

void uiTaskEntry(void * /*param*/) {
  const uint32_t bootMs = millis();
  bool bootSplashDone = false;
  uint32_t lastAppTickMs = bootMs;
  uint32_t lastHeapLogMs = bootMs;

  for (;;) {
    const uint32_t nowMs = millis();

    // DESIGN.md Section 12.4: Show boot logo for at least 600ms, then go to
    // Home
    if (!bootSplashDone && (nowMs - bootMs >= Config::SPLASH_MIN_DURATION_MS)) {
      bootSplashDone = true;
      Event dummy{};
      while (EventBus::poll(dummy)) {
      }

      StatusBar::begin();
      AppManager::switchTo(0); // Open Home
      logMemoryStats("home-ready");
    }

    if (bootSplashDone) {
      drainEvents();
      StatusBar::tick(nowMs);

      if (nowMs - lastAppTickMs >= Config::APP_TICK_PERIOD_MS) {
        AppManager::tick(nowMs);
        lastAppTickMs = nowMs;
      }

      lv_timer_handler();
    }

    if (nowMs - lastHeapLogMs >= Config::HEAP_LOG_INTERVAL_MS) {
      logMemoryStats("periodic");
      lastHeapLogMs = nowMs;
    }

    // DESIGN.md Section 10: 5 Hz UI timer in AOD mode, 200 Hz when active
    const uint32_t tickDelayMs = AppManager::isInAod()
                                     ? Config::UI_TICK_AOD_MS
                                     : Config::UI_TICK_ACTIVE_MS;
    vTaskDelay(pdMS_TO_TICKS(tickDelayMs));
  }
}

} // namespace

void logMemoryStats(const char *tag) {
  const size_t sramFree = heap_caps_get_free_size(MALLOC_CAP_INTERNAL);
  const size_t sramMinFree =
      heap_caps_get_minimum_free_size(MALLOC_CAP_INTERNAL);
  const size_t sramTotal = heap_caps_get_total_size(MALLOC_CAP_INTERNAL);

  const size_t psramFree = heap_caps_get_free_size(MALLOC_CAP_SPIRAM);
  const size_t psramMinFree =
      heap_caps_get_minimum_free_size(MALLOC_CAP_SPIRAM);
  const size_t psramTotal = heap_caps_get_total_size(MALLOC_CAP_SPIRAM);

  Serial.printf(
      "[Mem][%s] up=%lus | SRAM free=%uB (min=%uB, total=%uB) | PSRAM free=%uB "
      "(min=%uB, total=%uB)\n",
      tag, static_cast<unsigned long>(millis() / 1000),
      static_cast<unsigned>(sramFree), static_cast<unsigned>(sramMinFree),
      static_cast<unsigned>(sramTotal), static_cast<unsigned>(psramFree),
      static_cast<unsigned>(psramMinFree), static_cast<unsigned>(psramTotal));
}

void begin() {
  // 1. Read saved theme & brightness from NVS Preferences BEFORE drawing
  // anything (DESIGN.md 3.3 & 12.1)
  Settings::begin();

  // 2. Immediately initialize panel with backlight off, draw theme-matched boot
  // logo, and ramp backlight over 300ms
  if (!Display::begin()) {
    Serial.begin(115200);
    Serial.println("[Kernel] FATAL: Display::begin() failed!");
    return;
  }

  // 3. Initialize Serial, DataStore, EventBus, Buttons, AppManager, and
  // background services behind the visible boot logo
  Serial.begin(115200);
  Serial.println("\n=====================================================");
  Serial.println("  HawaOS v0.3.1 — DESIGN.md UI/UX System Activated");
  Serial.println("=====================================================");
  logMemoryStats("boot-logo-shown");

  DataStore::begin();
  EventBus::begin();
  Buttons::begin();

  AppManager::begin();
  AppManager::registerApp(&s_homeApp);
  AppManager::registerApp(&s_statsApp);
  AppManager::registerApp(&s_settingsApp);

  // 4. Start Core 0 WiFi + NTP service (non-blocking)
  WifiService::begin();

  // 5. Start Core 1 UI task
  xTaskCreatePinnedToCore(uiTaskEntry, "ui_task", Config::UI_TASK_STACK_BYTES,
                          nullptr, Config::UI_TASK_PRIORITY, &s_uiTaskHandle,
                          Config::UI_TASK_CORE);
}

} // namespace Kernel
