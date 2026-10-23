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

#include <Arduino.h>
#include <esp_heap_caps.h>
#include <freertos/FreeRTOS.h>
#include <freertos/task.h>
#include <lvgl.h>

namespace Kernel {

namespace {

TaskHandle_t s_uiTaskHandle = nullptr;

// =============================================================================
// System Memory & Uptime App (retained alongside ClockApp)
// =============================================================================
class SysMonitorApp final : public App {
public:
  const char *name() const override { return "System Info"; }

  void onEnter(lv_obj_t *root) override {
    Theme::styleScreen(root);

    // SRAM card
    lv_obj_t *sramCard = Theme::createCard(root, 304, 52, 0, 28);
    m_sramLbl = lv_label_create(sramCard);
    lv_label_set_text(m_sramLbl, "SRAM: --");
    lv_obj_set_style_text_font(m_sramLbl, &lv_font_montserrat_12, LV_PART_MAIN);
    lv_obj_set_style_text_color(m_sramLbl, lv_color_hex(Theme::TEXT_PRIMARY),
                                LV_PART_MAIN);
    lv_obj_align(m_sramLbl, LV_ALIGN_TOP_LEFT, 0, 0);

    m_sramBar = lv_bar_create(sramCard);
    lv_obj_set_size(m_sramBar, 286, 8);
    lv_obj_align(m_sramBar, LV_ALIGN_BOTTOM_MID, 0, 0);
    lv_bar_set_range(m_sramBar, 0, 100);
    lv_obj_set_style_bg_color(m_sramBar, lv_color_hex(Theme::BG_DARK),
                              LV_PART_MAIN);
    lv_obj_set_style_bg_color(m_sramBar, lv_color_hex(Theme::ACCENT_GREEN),
                              LV_PART_INDICATOR);

    // PSRAM card
    lv_obj_t *psramCard = Theme::createCard(root, 304, 52, 0, 86);
    m_psramLbl = lv_label_create(psramCard);
    lv_label_set_text(m_psramLbl, "PSRAM: --");
    lv_obj_set_style_text_font(m_psramLbl, &lv_font_montserrat_12,
                               LV_PART_MAIN);
    lv_obj_set_style_text_color(m_psramLbl, lv_color_hex(Theme::TEXT_PRIMARY),
                                LV_PART_MAIN);
    lv_obj_align(m_psramLbl, LV_ALIGN_TOP_LEFT, 0, 0);

    m_psramBar = lv_bar_create(psramCard);
    lv_obj_set_size(m_psramBar, 286, 8);
    lv_obj_align(m_psramBar, LV_ALIGN_BOTTOM_MID, 0, 0);
    lv_bar_set_range(m_psramBar, 0, 100);
    lv_obj_set_style_bg_color(m_psramBar, lv_color_hex(Theme::BG_DARK),
                              LV_PART_MAIN);
    lv_obj_set_style_bg_color(m_psramBar, lv_color_hex(Theme::ACCENT_CYAN),
                              LV_PART_INDICATOR);

    // Footer
    m_footerLbl = lv_label_create(root);
    lv_label_set_text(m_footerLbl, "NEXT: Cycle Apps  |  Hold NEXT: Launcher");
    lv_obj_set_style_text_font(m_footerLbl, &lv_font_montserrat_10,
                               LV_PART_MAIN);
    lv_obj_set_style_text_color(m_footerLbl, lv_color_hex(Theme::TEXT_MUTED),
                                LV_PART_MAIN);
    lv_obj_align(m_footerLbl, LV_ALIGN_BOTTOM_MID, 0, -4);

    onTick(millis());
  }
  void onExit() override {
    m_sramLbl = nullptr;
    m_sramBar = nullptr;
    m_psramLbl = nullptr;
    m_psramBar = nullptr;
    m_footerLbl = nullptr;
  }

  void onTick(uint32_t nowMs) override {
    if (m_sramLbl == nullptr) {
      return;
    }
    const size_t sramFree = heap_caps_get_free_size(MALLOC_CAP_INTERNAL);
    const size_t sramTotal = heap_caps_get_total_size(MALLOC_CAP_INTERNAL);
    const int sramPct =
        (sramTotal > 0)
            ? static_cast<int>(((sramTotal - sramFree) * 100) / sramTotal)
            : 0;
    lv_label_set_text_fmt(m_sramLbl,
                          "Internal SRAM: %u / %u KB free (%d%% used)",
                          static_cast<unsigned>(sramFree / 1024),
                          static_cast<unsigned>(sramTotal / 1024), sramPct);
    lv_bar_set_value(m_sramBar, sramPct, LV_ANIM_OFF);

    const size_t psramFree = heap_caps_get_free_size(MALLOC_CAP_SPIRAM);
    const size_t psramTotal = heap_caps_get_total_size(MALLOC_CAP_SPIRAM);
    const int psramPct =
        (psramTotal > 0)
            ? static_cast<int>(((psramTotal - psramFree) * 100) / psramTotal)
            : 0;
    lv_label_set_text_fmt(m_psramLbl, "OPI PSRAM: %u / %u KB free (%d%% used)",
                          static_cast<unsigned>(psramFree / 1024),
                          static_cast<unsigned>(psramTotal / 1024), psramPct);
    lv_bar_set_value(m_psramBar, psramPct, LV_ANIM_OFF);

    const uint32_t sec = nowMs / 1000;
    lv_label_set_text_fmt(
        m_footerLbl,
        "Uptime: %02lu:%02lu:%02lu  |  NEXT: Cycle  |  Hold NEXT: Home",
        static_cast<unsigned long>(sec / 3600),
        static_cast<unsigned long>((sec % 3600) / 60),
        static_cast<unsigned long>(sec % 60));
  }

private:
  lv_obj_t *m_sramLbl = nullptr;
  lv_obj_t *m_sramBar = nullptr;
  lv_obj_t *m_psramLbl = nullptr;
  lv_obj_t *m_psramBar = nullptr;
  lv_obj_t *m_footerLbl = nullptr;
};

ClockApp s_clockApp;
SysMonitorApp s_sysMonitorApp;

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

    if (!bootSplashDone && (nowMs - bootMs >= Config::SPLASH_DURATION_MS)) {
      bootSplashDone = true;
      Event dummy{};
      while (EventBus::poll(dummy)) {
      }

      // Initialize persistent top StatusBar on lv_layer_top()
      StatusBar::begin();
      AppManager::switchTo(0);
      logMemoryStats("clock-ready");
    }

    if (bootSplashDone) {
      drainEvents();
      StatusBar::tick(nowMs);

      if (nowMs - lastAppTickMs >= Config::APP_TICK_PERIOD_MS) {
        AppManager::tick(nowMs);
        lastAppTickMs = nowMs;
      }
    }

    if (nowMs - lastHeapLogMs >= Config::HEAP_LOG_INTERVAL_MS) {
      logMemoryStats("periodic");
      lastHeapLogMs = nowMs;
    }

    lv_timer_handler();
    vTaskDelay(pdMS_TO_TICKS(Config::UI_TICK_PERIOD_MS));
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
  // 1. Serial + Settings
  Serial.begin(115200);
  const uint32_t t0 = millis();
  while (!Serial && (millis() - t0 < 1500)) {
    delay(10);
  }

  Serial.println("\n================================================");
  Serial.println("  HawaOS v0.3.0 — Milestone 3: WiFi/NTP/Clock");
  Serial.println("================================================");
  logMemoryStats("boot-start");

  Settings::begin();
  DataStore::begin();
  EventBus::begin();

  // 2. Display (splash immediately, apply saved brightness)
  if (!Display::begin()) {
    Serial.println("[Kernel] FATAL: Display::begin() failed!");
    return;
  }
  Display::showSplash();
  Display::setBrightness(Settings::getBrightness());
  logMemoryStats("splash-shown");

  // 3. Buttons driver
  Buttons::begin();

  // 4. AppManager + static app registration
  AppManager::begin();
  AppManager::registerApp(&s_clockApp);
  AppManager::registerApp(&s_sysMonitorApp);

  // 5. Services on Core 0 (WiFi first; NTP triggered on connect)
  WifiService::begin();

  // 6. UI task on Core 1 (Rule 1: Only the UI task touches LVGL from here on)
  xTaskCreatePinnedToCore(uiTaskEntry, "ui_task", Config::UI_TASK_STACK_BYTES,
                          nullptr, Config::UI_TASK_PRIORITY, &s_uiTaskHandle,
                          Config::UI_TASK_CORE);
}

} // namespace Kernel
