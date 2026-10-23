#include "ui/StatusBar.h"
#include "config.h"
#include "ui/Theme.h"
#include "ui/icons.h"

#include <Arduino.h>
#include <lvgl.h>

namespace StatusBar {

namespace {

lv_obj_t *s_bar = nullptr;
lv_obj_t *s_titleLbl = nullptr;
lv_obj_t *s_connIcon = nullptr;
lv_obj_t *s_bleIcon = nullptr;
lv_obj_t *s_bellIcon = nullptr;

} // namespace

void begin() {
  if (s_bar != nullptr) {
    return;
  }

  lv_obj_t *topLayer = lv_layer_top();

  s_bar = lv_obj_create(topLayer);
  lv_obj_set_size(s_bar, Config::LCD_HOR_RES, Config::STATUS_BAR_HEIGHT);
  lv_obj_align(s_bar, LV_ALIGN_TOP_MID, 0, 0);
  lv_obj_set_style_border_width(s_bar, 0, LV_PART_MAIN);
  lv_obj_set_style_radius(s_bar, 0, LV_PART_MAIN);
  lv_obj_set_style_pad_hor(s_bar, Config::OUTER_MARGIN_PX, LV_PART_MAIN);
  lv_obj_set_style_pad_ver(s_bar, 0, LV_PART_MAIN);
  lv_obj_clear_flag(s_bar, LV_OBJ_FLAG_SCROLLABLE);

  // Left: screen title (caption 12, textDim)
  s_titleLbl = lv_label_create(s_bar);
  lv_label_set_text(s_titleLbl, "Home");
  lv_obj_set_style_text_font(s_titleLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_align(s_titleLbl, LV_ALIGN_LEFT_MID, 0, 0);

  // Right icons: 20 px Lucide icons, 8 px gaps
  s_connIcon = lv_label_create(s_bar);
  lv_label_set_text(s_connIcon, ICON_WIFI_OFF);
  lv_obj_set_style_text_font(s_connIcon, Theme::iconFont20(), LV_PART_MAIN);
  lv_obj_align(s_connIcon, LV_ALIGN_RIGHT_MID, 0, 0);

  s_bleIcon = lv_label_create(s_bar);
  lv_label_set_text(s_bleIcon, ICON_BLUETOOTH);
  lv_obj_set_style_text_font(s_bleIcon, Theme::iconFont20(), LV_PART_MAIN);
  lv_obj_align(s_bleIcon, LV_ALIGN_RIGHT_MID, -28, 0);
  lv_obj_add_flag(s_bleIcon, LV_OBJ_FLAG_HIDDEN);

  s_bellIcon = lv_label_create(s_bar);
  lv_label_set_text(s_bellIcon, ICON_BELL);
  lv_obj_set_style_text_font(s_bellIcon, Theme::iconFont20(), LV_PART_MAIN);
  lv_obj_align(s_bellIcon, LV_ALIGN_RIGHT_MID, -28, 0);
  lv_obj_add_flag(s_bellIcon, LV_OBJ_FLAG_HIDDEN);

  applyTheme();
  updateData(DataStore::getSnapshot());
}

void setVisible(bool visible) {
  if (s_bar == nullptr)
    return;
  if (visible) {
    lv_obj_clear_flag(s_bar, LV_OBJ_FLAG_HIDDEN);
  } else {
    lv_obj_add_flag(s_bar, LV_OBJ_FLAG_HIDDEN);
  }
}

void setTitle(const char *title) {
  if (s_titleLbl != nullptr && title != nullptr) {
    lv_label_set_text(s_titleLbl, title);
  }
}

void applyTheme() {
  if (s_bar == nullptr)
    return;
  const ThemeTokens::Palette &p = Theme::active();

  lv_obj_set_style_bg_color(s_bar, lv_color_hex(p.bg), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(s_bar, LV_OPA_COVER, LV_PART_MAIN);
  lv_obj_set_style_text_color(s_titleLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);

  updateData(DataStore::getSnapshot());
}

void updateData(const DataSnapshot &d) {
  if (s_connIcon == nullptr)
    return;
  const ThemeTokens::Palette &p = Theme::active();

  // Connection icon: usb if USB companion active, wifi if WiFi connected, else
  // wifi-off
  if (d.usbConnected) {
    lv_label_set_text(s_connIcon, ICON_USB);
    lv_obj_set_style_text_color(s_connIcon, lv_color_hex(p.accent),
                                LV_PART_MAIN);
  } else if (d.wifiConnected) {
    lv_label_set_text(s_connIcon, ICON_WIFI);
    lv_obj_set_style_text_color(s_connIcon, lv_color_hex(p.text), LV_PART_MAIN);
  } else {
    lv_label_set_text(s_connIcon, ICON_WIFI_OFF);
    lv_obj_set_style_text_color(s_connIcon, lv_color_hex(p.textDim),
                                LV_PART_MAIN);
  }

  int16_t rightOffset = -28; // 20px icon + 8px gap

  if (d.bleConnected) {
    lv_obj_clear_flag(s_bleIcon, LV_OBJ_FLAG_HIDDEN);
    lv_obj_set_style_text_color(s_bleIcon, lv_color_hex(p.accent),
                                LV_PART_MAIN);
    lv_obj_align(s_bleIcon, LV_ALIGN_RIGHT_MID, rightOffset, 0);
    rightOffset -= 28;
  } else {
    lv_obj_add_flag(s_bleIcon, LV_OBJ_FLAG_HIDDEN);
  }

  if (d.unreadCount > 0) {
    lv_obj_clear_flag(s_bellIcon, LV_OBJ_FLAG_HIDDEN);
    lv_obj_set_style_text_color(s_bellIcon, lv_color_hex(p.accent),
                                LV_PART_MAIN);
    lv_obj_align(s_bellIcon, LV_ALIGN_RIGHT_MID, rightOffset, 0);
  } else {
    lv_obj_add_flag(s_bellIcon, LV_OBJ_FLAG_HIDDEN);
  }
}

void tick(uint32_t /*nowMs*/) {
  // StatusBar is event-driven via updateData() and setTitle() per DESIGN.md 9.1
}

} // namespace StatusBar
