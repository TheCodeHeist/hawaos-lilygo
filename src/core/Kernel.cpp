#include "core/Kernel.h"
#include "apps/ClockApp.h"
#include "apps/MediaApp.h"
#include "apps/NotifyApp.h"
#include "apps/SettingsApp.h"
#include "apps/StatsApp.h"
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

// DESIGN.md Section 9.2 Launcher order:
// 0: Home, 1: Media, 2: Stats, 3: Notifications, 4: Settings
ClockApp s_homeApp;
MediaApp s_mediaApp;
StatsApp s_statsApp;
NotifyApp s_notifyApp;
SettingsApp s_settingsApp;

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
    case EventType::SettingsChanged:
      if (!AppManager::isInAod()) {
        Display::setBrightness(Settings::getBrightnessStep());
      }
      AppManager::applyActiveTheme();
      break;
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

    // DESIGN.md Section 12.4: Show boot logo for at least
    // SPLASH_MIN_DURATION_MS, then go to Home
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
  Serial.println("  HawaOS v0.4.0 — Milestone 4 Companion & Apps Ready");
  Serial.println("=====================================================");
  logMemoryStats("boot-logo-shown");

  DataStore::begin();
  EventBus::begin();
  Buttons::begin();

  AppManager::begin();
  AppManager::registerApp(&s_homeApp);
  AppManager::registerApp(&s_mediaApp);
  AppManager::registerApp(&s_statsApp);
  AppManager::registerApp(&s_notifyApp);
  AppManager::registerApp(&s_settingsApp);

  // 4. Start Core 0 WiFi + NTP + CompanionLink service (non-blocking)
  WifiService::begin();

  // 5. Start Core 1 UI task
  xTaskCreatePinnedToCore(uiTaskEntry, "ui_task", Config::UI_TASK_STACK_BYTES,
                          nullptr, Config::UI_TASK_PRIORITY, &s_uiTaskHandle,
                          Config::UI_TASK_CORE);
}

} // namespace Kernel
