#include "core/Settings.h"
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

// Floating Notification Popup Banner on lv_layer_top()
lv_obj_t *s_popupCard = nullptr;
lv_obj_t *s_popupIcon = nullptr;
lv_obj_t *s_popupTitleLbl = nullptr;
lv_obj_t *s_popupBodyLbl = nullptr;
lv_obj_t *s_popupBadgeLbl = nullptr;
bool s_popupVisible = false;
uint32_t s_popupShownAtMs = 0;

constexpr uint32_t kPopupDurationMs = 4500;
constexpr int16_t kPopupVisibleY = 6;
constexpr int16_t kPopupHiddenY = -64;

void popupAnimYCb(void *var, int32_t v) {
  lv_obj_t *obj = static_cast<lv_obj_t *>(var);
  if (obj != nullptr) {
    lv_obj_set_y(obj, static_cast<lv_coord_t>(v));
  }
}

void popupHideReadyCb(lv_anim_t * /*a*/) {
  if (s_popupCard != nullptr && !s_popupVisible) {
    lv_obj_add_flag(s_popupCard, LV_OBJ_FLAG_HIDDEN);
  }
}

void stylePopupCard() {
  if (s_popupCard == nullptr)
    return;
  const ThemeTokens::Palette &p = Theme::active();

  lv_obj_set_style_bg_color(s_popupCard, lv_color_hex(p.surface), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(s_popupCard, LV_OPA_COVER, LV_PART_MAIN);
  lv_obj_set_style_border_color(s_popupCard, lv_color_hex(p.primary),
                                LV_PART_MAIN);
  lv_obj_set_style_border_width(s_popupCard, 2, LV_PART_MAIN);

  if (s_popupIcon != nullptr) {
    lv_obj_set_style_text_color(s_popupIcon, lv_color_hex(p.accent),
                                LV_PART_MAIN);
  }
  if (s_popupTitleLbl != nullptr) {
    lv_obj_set_style_text_color(s_popupTitleLbl, lv_color_hex(p.text),
                                LV_PART_MAIN);
  }
  if (s_popupBodyLbl != nullptr) {
    lv_obj_set_style_text_color(s_popupBodyLbl, lv_color_hex(p.textDim),
                                LV_PART_MAIN);
  }
  if (s_popupBadgeLbl != nullptr) {
    lv_obj_set_style_text_color(s_popupBadgeLbl, lv_color_hex(p.accent),
                                LV_PART_MAIN);
  }
}

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

  // Create floating notification popup card on lv_layer_top() (above s_bar)
  s_popupCard = lv_obj_create(topLayer);
  lv_obj_set_size(s_popupCard, 300, 54);
  lv_obj_set_pos(s_popupCard, 10, kPopupHiddenY);
  lv_obj_set_style_radius(s_popupCard, Config::RADIUS_MD, LV_PART_MAIN);
  lv_obj_set_style_pad_all(s_popupCard, 8, LV_PART_MAIN);
  lv_obj_clear_flag(s_popupCard, LV_OBJ_FLAG_SCROLLABLE);
  lv_obj_add_flag(s_popupCard, LV_OBJ_FLAG_HIDDEN);

  s_popupIcon = lv_label_create(s_popupCard);
  lv_label_set_text(s_popupIcon, ICON_BELL);
  lv_obj_set_style_text_font(s_popupIcon, Theme::iconFont24(), LV_PART_MAIN);
  lv_obj_align(s_popupIcon, LV_ALIGN_LEFT_MID, 2, 0);

  s_popupTitleLbl = lv_label_create(s_popupCard);
  lv_obj_set_style_text_font(s_popupTitleLbl, Theme::fontBodySb(),
                             LV_PART_MAIN);
  lv_label_set_long_mode(s_popupTitleLbl, LV_LABEL_LONG_DOT);
  lv_obj_set_width(s_popupTitleLbl, 216);
  lv_obj_align(s_popupTitleLbl, LV_ALIGN_TOP_LEFT, 34, 0);

  s_popupBadgeLbl = lv_label_create(s_popupCard);
  lv_label_set_text(s_popupBadgeLbl, "NOW");
  lv_obj_set_style_text_font(s_popupBadgeLbl, Theme::fontCaption(),
                             LV_PART_MAIN);
  lv_obj_align(s_popupBadgeLbl, LV_ALIGN_TOP_RIGHT, -2, 1);

  s_popupBodyLbl = lv_label_create(s_popupCard);
  lv_obj_set_style_text_font(s_popupBodyLbl, Theme::fontCaption(),
                             LV_PART_MAIN);
  lv_label_set_long_mode(s_popupBodyLbl, LV_LABEL_LONG_SCROLL_CIRCULAR);
  lv_obj_set_style_anim_speed(s_popupBodyLbl, 28, LV_PART_MAIN);
  lv_obj_set_width(s_popupBodyLbl, 256);
  lv_obj_align(s_popupBodyLbl, LV_ALIGN_BOTTOM_LEFT, 34, 0);

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

  stylePopupCard();
  updateData(DataStore::getSnapshot());
}

void updateData(const DataSnapshot &d) {
  if (s_connIcon == nullptr)
    return;
  const ThemeTokens::Palette &p = Theme::active();

  // Connection icon: usb if USB companion active; in Bluetooth link mode the
  // Bluetooth state; else wifi / wifi-off.
  const bool bleMode = Settings::getLinkMode() == LinkMode::Ble;
  if (d.usbConnected) {
    lv_label_set_text(s_connIcon, ICON_USB);
    lv_obj_set_style_text_color(s_connIcon, lv_color_hex(p.accent),
                                LV_PART_MAIN);
  } else if (bleMode) {
    lv_label_set_text(s_connIcon,
                      d.bleConnected ? ICON_BLUETOOTH : ICON_BLUETOOTH_OFF);
    lv_obj_set_style_text_color(
        s_connIcon, lv_color_hex(d.bleConnected ? p.accent : p.textDim),
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

  // Setup over BLE (or USB in Bluetooth mode): a second icon for the link.
  if (d.bleConnected && (!bleMode || d.usbConnected)) {
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

void showNotificationPopup(const char *app, const char *title, const char *body,
                           uint8_t count) {
  if (s_popupCard == nullptr)
    return;

  stylePopupCard();

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
  if (s_popupIcon != nullptr) {
    lv_label_set_text(s_popupIcon, iconGlyph);
  }

  // Heading format: ({count}) New Notification "{app_name}"
  char headerBuf[64];
  snprintf(headerBuf, sizeof(headerBuf), "(%u) New Notification \"%s\"",
           (count > 0 ? count : 1), appName);
  lv_label_set_text(s_popupTitleLbl, headerBuf);

  char bodyBuf[160] = {0};
  if (cleanTitle[0] != '\0' && strcmp(cleanTitle, appName) != 0) {
    if (cleanBody[0] != '\0') {
      snprintf(bodyBuf, sizeof(bodyBuf), "%s: %s", cleanTitle, cleanBody);
    } else {
      snprintf(bodyBuf, sizeof(bodyBuf), "%s", cleanTitle);
    }
  } else if (cleanBody[0] != '\0') {
    snprintf(bodyBuf, sizeof(bodyBuf), "%s", cleanBody);
  } else {
    snprintf(bodyBuf, sizeof(bodyBuf), "New notification received");
  }
  lv_label_set_text(s_popupBodyLbl, bodyBuf);

  s_popupVisible = true;
  s_popupShownAtMs = millis();
  lv_anim_del(s_popupCard, popupAnimYCb);
  lv_obj_clear_flag(s_popupCard, LV_OBJ_FLAG_HIDDEN);
  lv_obj_move_foreground(s_popupCard);

  const uint32_t dur = Theme::animDuration(180);
  if (dur == 0) {
    lv_obj_set_y(s_popupCard, kPopupVisibleY);
    lv_obj_invalidate(s_popupCard);
    return;
  }

  lv_anim_t a;
  lv_anim_init(&a);
  lv_anim_set_var(&a, s_popupCard);
  lv_anim_set_values(&a, lv_obj_get_y(s_popupCard), kPopupVisibleY);
  lv_anim_set_time(&a, dur);
  lv_anim_set_path_cb(&a, lv_anim_path_ease_out);
  lv_anim_set_exec_cb(&a, popupAnimYCb);
  lv_anim_set_early_apply(&a, true);
  lv_anim_start(&a);
  lv_obj_invalidate(s_popupCard);
}

void dismissNotificationPopup() {
  if (s_popupCard == nullptr || !s_popupVisible)
    return;

  s_popupVisible = false;
  lv_anim_del(s_popupCard, popupAnimYCb);

  const uint32_t dur = Theme::animDuration(160);
  if (dur == 0) {
    lv_obj_set_y(s_popupCard, kPopupHiddenY);
    lv_obj_add_flag(s_popupCard, LV_OBJ_FLAG_HIDDEN);
    lv_obj_invalidate(s_popupCard);
    return;
  }

  lv_anim_t a;
  lv_anim_init(&a);
  lv_anim_set_var(&a, s_popupCard);
  lv_anim_set_values(&a, lv_obj_get_y(s_popupCard), kPopupHiddenY);
  lv_anim_set_time(&a, dur);
  lv_anim_set_path_cb(&a, lv_anim_path_ease_in);
  lv_anim_set_exec_cb(&a, popupAnimYCb);
  lv_anim_set_ready_cb(&a, popupHideReadyCb);
  lv_anim_set_early_apply(&a, true);
  lv_anim_start(&a);
  lv_obj_invalidate(s_popupCard);
}

bool isPopupVisible() { return s_popupVisible; }

void tick(uint32_t nowMs) {
  if (s_popupVisible) {
    if (nowMs < s_popupShownAtMs) {
      return;
    }
    if (nowMs - s_popupShownAtMs >= kPopupDurationMs) {
      dismissNotificationPopup();
    }
  }
}

} // namespace StatusBar
