#include "core/AppManager.h"
#include "config.h"
#include "core/Kernel.h"
#include "ui/StatusBar.h"
#include "ui/Theme.h"

#include <Arduino.h>
#include <esp_heap_caps.h>
#include <lvgl.h>

namespace AppManager {

namespace {

App *s_apps[Config::MAX_REGISTERED_APPS] = {nullptr};
size_t s_appCount = 0;
int s_activeAppIdx = -1; // -1 = Launcher active
size_t s_launcherCursor = 0;
uint32_t s_switchCounter = 0;

// Launcher UI widget pointers (valid only when s_activeAppIdx == -1)
lv_obj_t *s_launcherCards[Config::MAX_REGISTERED_APPS] = {nullptr};
lv_obj_t *s_launcherFooterLbl = nullptr;

// Helper: replace the current LVGL screen synchronously without leaking objects
lv_obj_t *createAndSwapScreen() {
  lv_obj_t *oldScr = lv_scr_act();
  lv_obj_t *newScr = lv_obj_create(nullptr);
  Theme::styleScreen(newScr);

  lv_scr_load(newScr);
  if (oldScr != nullptr && oldScr != newScr) {
    lv_obj_del(oldScr);
  }
  return newScr;
}

void updateLauncherHighlight() {
  if (s_activeAppIdx != -1) {
    return;
  }
  for (size_t i = 0; i < s_appCount; ++i) {
    lv_obj_t *card = s_launcherCards[i];
    if (card == nullptr) {
      continue;
    }
    const bool selected = (i == s_launcherCursor);
    lv_obj_set_style_bg_color(
        card, lv_color_hex(selected ? Theme::BG_HIGHLIGHT : Theme::BG_SURFACE),
        LV_PART_MAIN);
    lv_obj_set_style_border_color(
        card,
        lv_color_hex(selected ? Theme::ACCENT_CYAN : Theme::BORDER_SUBTLE),
        LV_PART_MAIN);
    lv_obj_set_style_border_width(card, selected ? 2 : 1, LV_PART_MAIN);
  }
}

void buildLauncherScreen(lv_obj_t *root) {
  StatusBar::setTitle("Launcher");

  const int16_t startY = 30;
  const int16_t cardH = (s_appCount <= 3) ? 36 : 28;
  const int16_t stepY = cardH + 6;

  for (size_t i = 0; i < s_appCount; ++i) {
    lv_obj_t *card = lv_obj_create(root);
    lv_obj_set_size(card, 304, cardH);
    lv_obj_align(card, LV_ALIGN_TOP_MID, 0,
                 startY + static_cast<int16_t>(i * stepY));
    lv_obj_set_style_radius(card, 6, LV_PART_MAIN);
    lv_obj_set_style_pad_hor(card, 12, LV_PART_MAIN);
    lv_obj_set_style_pad_ver(card, 2, LV_PART_MAIN);
    lv_obj_clear_flag(card, LV_OBJ_FLAG_SCROLLABLE);

    lv_obj_t *nameLbl = lv_label_create(card);
    lv_label_set_text_fmt(nameLbl, "%u.  %s", static_cast<unsigned>(i + 1),
                          s_apps[i]->name());
    lv_obj_set_style_text_font(nameLbl, &lv_font_montserrat_14, LV_PART_MAIN);
    lv_obj_set_style_text_color(nameLbl, lv_color_hex(Theme::TEXT_PRIMARY),
                                LV_PART_MAIN);
    lv_obj_align(nameLbl, LV_ALIGN_LEFT_MID, 0, 0);

    lv_obj_t *hintLbl = lv_label_create(card);
    lv_label_set_text(hintLbl, "OPEN ->");
    lv_obj_set_style_text_font(hintLbl, &lv_font_montserrat_10, LV_PART_MAIN);
    lv_obj_set_style_text_color(hintLbl, lv_color_hex(Theme::TEXT_MUTED),
                                LV_PART_MAIN);
    lv_obj_align(hintLbl, LV_ALIGN_RIGHT_MID, 0, 0);

    s_launcherCards[i] = card;
  }

  s_launcherFooterLbl = lv_label_create(root);
  lv_label_set_text(
      s_launcherFooterLbl,
      "NEXT(14): Select App  |  SELECT(0): Open  |  Hold NEXT: Home");
  lv_obj_set_style_text_font(s_launcherFooterLbl, &lv_font_montserrat_10,
                             LV_PART_MAIN);
  lv_obj_set_style_text_color(s_launcherFooterLbl,
                              lv_color_hex(Theme::TEXT_DIM), LV_PART_MAIN);
  lv_obj_align(s_launcherFooterLbl, LV_ALIGN_BOTTOM_MID, 0, -3);

  updateLauncherHighlight();
}

} // namespace

void begin() {
  s_appCount = 0;
  s_activeAppIdx = -1;
  s_launcherCursor = 0;
  s_switchCounter = 0;
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

  if (s_activeAppIdx >= 0 && static_cast<size_t>(s_activeAppIdx) < s_appCount) {
    s_apps[s_activeAppIdx]->onExit();
  }

  for (size_t i = 0; i < Config::MAX_REGISTERED_APPS; ++i) {
    s_launcherCards[i] = nullptr;
  }
  s_launcherFooterLbl = nullptr;

  lv_obj_t *newScr = createAndSwapScreen();
  s_activeAppIdx = static_cast<int>(idx);
  s_launcherCursor = idx;
  ++s_switchCounter;

  StatusBar::setTitle(s_apps[idx]->name());
  s_apps[idx]->onEnter(newScr);
}

void showLauncher() {
  if (s_activeAppIdx >= 0 && static_cast<size_t>(s_activeAppIdx) < s_appCount) {
    s_apps[s_activeAppIdx]->onExit();
  }

  s_activeAppIdx = -1;
  lv_obj_t *newScr = createAndSwapScreen();
  buildLauncherScreen(newScr);
  Serial.println("[AppManager] Entered Launcher");
}

void tick(uint32_t nowMs) {
  if (s_activeAppIdx >= 0 && static_cast<size_t>(s_activeAppIdx) < s_appCount) {
    s_apps[s_activeAppIdx]->onTick(nowMs);
  }
}

void handleButton(Button b, Press p) {
  // 1. If in Launcher, handle navigation directly
  if (s_activeAppIdx < 0) {
    if (s_appCount == 0) {
      return;
    }
    if (b == Button::Next && (p == Press::Short || p == Press::Double)) {
      s_launcherCursor = (s_launcherCursor + 1) % s_appCount;
      updateLauncherHighlight();
      Serial.printf("[AppManager] Launcher cursor -> [%u] %s\n",
                    static_cast<unsigned>(s_launcherCursor),
                    s_apps[s_launcherCursor]->name());
    } else if (b == Button::Select && p == Press::Short) {
      Serial.printf("[AppManager] Opening app [%u]: %s\n",
                    static_cast<unsigned>(s_launcherCursor),
                    s_apps[s_launcherCursor]->name());
      switchTo(s_launcherCursor);
    } else if (b == Button::Select && p == Press::Long) {
      runStressTest(200);
    }
    return;
  }

  // 2. Give active app first chance to consume the button event
  App *activeApp = s_apps[s_activeAppIdx];
  if (activeApp->onButton(b, p)) {
    return;
  }

  // 3. Global gestures when not consumed by the app:
  //    - Next (Short): Cycle to next app
  //    - Next (Long):  Return to Launcher
  if (b == Button::Next && p == Press::Short && s_appCount > 0) {
    const size_t nextIdx =
        (static_cast<size_t>(s_activeAppIdx) + 1) % s_appCount;
    Serial.printf("[AppManager] Global Next (Short) -> switching to [%u] %s\n",
                  static_cast<unsigned>(nextIdx), s_apps[nextIdx]->name());
    switchTo(nextIdx);
  } else if (b == Button::Next && p == Press::Long) {
    Serial.println("[AppManager] Global Next (Long) -> returning to Launcher");
    showLauncher();
  }
}

void handleData(const DataSnapshot &d) {
  if (s_activeAppIdx >= 0 && static_cast<size_t>(s_activeAppIdx) < s_appCount) {
    s_apps[s_activeAppIdx]->onData(d);
  }
}

void runStressTest(uint16_t switches) {
  if (s_appCount == 0) {
    return;
  }

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
  if (s_launcherFooterLbl != nullptr) {
    lv_label_set_text_fmt(
        s_launcherFooterLbl, "200x Switch Test OK (dSRAM=%ldB, dPSRAM=%ldB)",
        static_cast<long>(sramDelta), static_cast<long>(psramDelta));
  }
}

size_t appCount() { return s_appCount; }

int activeAppIndex() { return s_activeAppIdx; }

} // namespace AppManager
