#include "ui/StatusBar.h"
#include "config.h"
#include "core/Settings.h"
#include "ui/Theme.h"

#include <Arduino.h>
#include <lvgl.h>
#include <time.h>

namespace StatusBar {

namespace {

lv_obj_t *s_bar = nullptr;
lv_obj_t *s_titleLbl = nullptr;
lv_obj_t *s_clockLbl = nullptr;
lv_obj_t *s_wifiLbl = nullptr;
lv_obj_t *s_batLbl = nullptr;
uint32_t s_lastTickMs = 0;

} // namespace

void begin() {
  if (s_bar != nullptr) {
    return;
  }

  lv_obj_t *topLayer = lv_layer_top();

  s_bar = lv_obj_create(topLayer);
  lv_obj_set_size(s_bar, Config::LCD_HOR_RES, Config::STATUS_BAR_HEIGHT);
  lv_obj_align(s_bar, LV_ALIGN_TOP_MID, 0, 0);
  lv_obj_set_style_bg_color(s_bar, lv_color_hex(0x111827), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(s_bar, LV_OPA_COVER, LV_PART_MAIN);
  lv_obj_set_style_border_color(s_bar, lv_color_hex(Theme::BORDER_SUBTLE),
                                LV_PART_MAIN);
  lv_obj_set_style_border_width(s_bar, 1, LV_PART_MAIN);
  lv_obj_set_style_border_side(s_bar, LV_BORDER_SIDE_BOTTOM, LV_PART_MAIN);
  lv_obj_set_style_radius(s_bar, 0, LV_PART_MAIN);
  lv_obj_set_style_pad_hor(s_bar, 8, LV_PART_MAIN);
  lv_obj_set_style_pad_ver(s_bar, 2, LV_PART_MAIN);
  lv_obj_clear_flag(s_bar, LV_OBJ_FLAG_SCROLLABLE);

  // Left: App / View title
  s_titleLbl = lv_label_create(s_bar);
  lv_label_set_text(s_titleLbl, "HawaOS");
  lv_obj_set_style_text_font(s_titleLbl, &lv_font_montserrat_12, LV_PART_MAIN);
  lv_obj_set_style_text_color(s_titleLbl, lv_color_hex(Theme::ACCENT_CYAN),
                              LV_PART_MAIN);
  lv_obj_align(s_titleLbl, LV_ALIGN_LEFT_MID, 0, 0);

  // Center: Mini clock
  s_clockLbl = lv_label_create(s_bar);
  lv_label_set_text(s_clockLbl, "--:--");
  lv_obj_set_style_text_font(s_clockLbl, &lv_font_montserrat_12, LV_PART_MAIN);
  lv_obj_set_style_text_color(s_clockLbl, lv_color_hex(Theme::TEXT_PRIMARY),
                              LV_PART_MAIN);
  lv_obj_align(s_clockLbl, LV_ALIGN_CENTER, 10, 0);

  // Right: Battery icon/voltage
  s_batLbl = lv_label_create(s_bar);
  lv_label_set_text(s_batLbl, LV_SYMBOL_BATTERY_FULL);
  lv_obj_set_style_text_font(s_batLbl, &lv_font_montserrat_12, LV_PART_MAIN);
  lv_obj_set_style_text_color(s_batLbl, lv_color_hex(Theme::ACCENT_GREEN),
                              LV_PART_MAIN);
  lv_obj_align(s_batLbl, LV_ALIGN_RIGHT_MID, 0, 0);

  // Right (next to battery): WiFi icon + RSSI
  s_wifiLbl = lv_label_create(s_bar);
  lv_label_set_text(s_wifiLbl, LV_SYMBOL_WIFI " --");
  lv_obj_set_style_text_font(s_wifiLbl, &lv_font_montserrat_12, LV_PART_MAIN);
  lv_obj_set_style_text_color(s_wifiLbl, lv_color_hex(Theme::TEXT_DIM),
                              LV_PART_MAIN);
  lv_obj_align(s_wifiLbl, LV_ALIGN_RIGHT_MID, -26, 0);

  updateData(DataStore::getSnapshot());
}

void setTitle(const char *title) {
  if (s_titleLbl != nullptr && title != nullptr) {
    lv_label_set_text_fmt(s_titleLbl, "HawaOS // %s", title);
  }
}

void updateData(const DataSnapshot &d) {
  if (s_wifiLbl == nullptr) {
    return;
  }

  if (d.wifiConnected) {
    lv_label_set_text_fmt(s_wifiLbl, LV_SYMBOL_WIFI " %ddB",
                          static_cast<int>(d.wifiRssi));
    const uint32_t color = (d.wifiRssi >= -67)   ? Theme::ACCENT_GREEN
                           : (d.wifiRssi >= -78) ? Theme::ACCENT_CYAN
                                                 : Theme::ACCENT_AMBER;
    lv_obj_set_style_text_color(s_wifiLbl, lv_color_hex(color), LV_PART_MAIN);
  } else {
    lv_label_set_text(s_wifiLbl, LV_SYMBOL_WIFI " Off");
    lv_obj_set_style_text_color(s_wifiLbl, lv_color_hex(Theme::TEXT_DIM),
                                LV_PART_MAIN);
  }
}

void tick(uint32_t nowMs) {
  if (s_bar == nullptr || (nowMs - s_lastTickMs < 500)) {
    return;
  }
  s_lastTickMs = nowMs;

  const time_t now = time(nullptr);
  if (now > 1700000000) {
    struct tm ti{};
    localtime_r(&now, &ti);
    if (Settings::getUse24Hour()) {
      lv_label_set_text_fmt(s_clockLbl, "%02d:%02d", ti.tm_hour, ti.tm_min);
    } else {
      int hr12 = ti.tm_hour % 12;
      if (hr12 == 0)
        hr12 = 12;
      lv_label_set_text_fmt(s_clockLbl, "%02d:%02d %s", hr12, ti.tm_min,
                            (ti.tm_hour >= 12) ? "PM" : "AM");
    }
  } else {
    lv_label_set_text(s_clockLbl, "NTP...");
  }
}

} // namespace StatusBar
