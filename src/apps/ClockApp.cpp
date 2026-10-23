#include "apps/ClockApp.h"
#include "config.h"
#include "core/Settings.h"
#include "services/NtpService.h"
#include "services/WifiService.h"
#include "ui/Theme.h"

#include <Arduino.h>
#include <time.h>

void ClockApp::onEnter(lv_obj_t *root) {
  Theme::styleScreen(root);

  // Main clock card below 24px StatusBar
  lv_obj_t *clockCard = Theme::createCard(root, 304, 96, 0, 28);

  // Large HH:MM label
  m_timeLbl = lv_label_create(clockCard);
  lv_label_set_text(m_timeLbl, "--:--");
  lv_obj_set_style_text_font(m_timeLbl, &lv_font_montserrat_48, LV_PART_MAIN);
  lv_obj_set_style_text_color(m_timeLbl, lv_color_hex(Theme::TEXT_PRIMARY),
                              LV_PART_MAIN);
  lv_obj_align(m_timeLbl, LV_ALIGN_TOP_LEFT, 10, 2);

  // Seconds label (:SS)
  m_secLbl = lv_label_create(clockCard);
  lv_label_set_text(m_secLbl, ":--");
  lv_obj_set_style_text_font(m_secLbl, &lv_font_montserrat_24, LV_PART_MAIN);
  lv_obj_set_style_text_color(m_secLbl, lv_color_hex(Theme::ACCENT_CYAN),
                              LV_PART_MAIN);
  lv_obj_align(m_secLbl, LV_ALIGN_TOP_LEFT, 162, 6);

  // AM/PM or 24H badge
  m_ampmLbl = lv_label_create(clockCard);
  lv_label_set_text(m_ampmLbl, "24H");
  lv_obj_set_style_text_font(m_ampmLbl, &lv_font_montserrat_14, LV_PART_MAIN);
  lv_obj_set_style_text_color(m_ampmLbl, lv_color_hex(Theme::TEXT_MUTED),
                              LV_PART_MAIN);
  lv_obj_align(m_ampmLbl, LV_ALIGN_TOP_LEFT, 165, 34);

  // NTP status badge (top-right inside card)
  m_syncBadge = lv_label_create(clockCard);
  lv_label_set_text(m_syncBadge, "SYNCING");
  lv_obj_set_style_text_font(m_syncBadge, &lv_font_montserrat_10, LV_PART_MAIN);
  lv_obj_set_style_text_color(m_syncBadge, lv_color_hex(Theme::ACCENT_AMBER),
                              LV_PART_MAIN);
  lv_obj_align(m_syncBadge, LV_ALIGN_TOP_RIGHT, -4, 4);

  // Full date line at bottom of clockCard
  m_dateLbl = lv_label_create(clockCard);
  lv_label_set_text(m_dateLbl, "Waiting for NTP time sync...");
  lv_obj_set_style_text_font(m_dateLbl, &lv_font_montserrat_14, LV_PART_MAIN);
  lv_obj_set_style_text_color(m_dateLbl, lv_color_hex(Theme::TEXT_MUTED),
                              LV_PART_MAIN);
  lv_obj_align(m_dateLbl, LV_ALIGN_BOTTOM_LEFT, 10, -2);

  // Bottom info strip (WiFi SSID / IP / controls)
  lv_obj_t *infoCard = Theme::createCard(root, 304, 34, 0, 130);
  lv_obj_set_style_pad_ver(infoCard, 4, LV_PART_MAIN);

  m_netLbl = lv_label_create(infoCard);
  lv_label_set_text(m_netLbl, "WiFi: Connecting...");
  lv_obj_set_style_text_font(m_netLbl, &lv_font_montserrat_12, LV_PART_MAIN);
  lv_obj_set_style_text_color(m_netLbl, lv_color_hex(Theme::TEXT_MUTED),
                              LV_PART_MAIN);
  lv_obj_align(m_netLbl, LV_ALIGN_LEFT_MID, 2, 0);

  m_lastSec = 0xFFFFFFFF;
  refreshNetwork(DataStore::getSnapshot());
  refreshTime();
}

void ClockApp::onExit() {
  m_timeLbl = nullptr;
  m_secLbl = nullptr;
  m_ampmLbl = nullptr;
  m_dateLbl = nullptr;
  m_netLbl = nullptr;
  m_syncBadge = nullptr;
}

void ClockApp::onTick(uint32_t /*nowMs*/) { refreshTime(); }

bool ClockApp::onButton(Button b, Press p) {
  if (b == Button::Select && p == Press::Short) {
    const bool nextMode = !Settings::getUse24Hour();
    Settings::setUse24Hour(nextMode);
    m_lastSec = 0xFFFFFFFF;
    refreshTime();
    return true;
  }
  if (b == Button::Select && p == Press::Double) {
    NtpService::requestSync();
    if (m_syncBadge != nullptr) {
      lv_label_set_text(m_syncBadge, "RESYNC...");
      lv_obj_set_style_text_color(m_syncBadge, lv_color_hex(Theme::ACCENT_CYAN),
                                  LV_PART_MAIN);
    }
    return true;
  }
  if (b == Button::Select && p == Press::Long) {
    WifiService::forceReconnect();
    return true;
  }
  return false; // Let AppManager handle Next (Short -> cycle, Long -> Launcher)
}

void ClockApp::onData(const DataSnapshot &d) {
  refreshNetwork(d);
  m_lastSec = 0xFFFFFFFF;
  refreshTime();
}

void ClockApp::refreshTime() {
  if (m_timeLbl == nullptr) {
    return;
  }

  const time_t now = time(nullptr);
  if (now <= 1700000000) {
    const uint32_t upSec = millis() / 1000;
    if (upSec == m_lastSec) {
      return;
    }
    m_lastSec = upSec;
    lv_label_set_text(m_timeLbl, "--:--");
    lv_label_set_text_fmt(m_secLbl, ":%02lu",
                          static_cast<unsigned long>(upSec % 60));
    lv_label_set_text(m_ampmLbl, Settings::getUse24Hour() ? "24H" : "12H");
    return;
  }

  if (static_cast<uint32_t>(now) == m_lastSec) {
    return;
  }
  m_lastSec = static_cast<uint32_t>(now);

  struct tm ti{};
  localtime_r(&now, &ti);

  const bool use24 = Settings::getUse24Hour();
  int displayHour = ti.tm_hour;
  if (!use24) {
    displayHour = ti.tm_hour % 12;
    if (displayHour == 0) {
      displayHour = 12;
    }
    lv_label_set_text(m_ampmLbl, (ti.tm_hour >= 12) ? "PM" : "AM");
  } else {
    lv_label_set_text(m_ampmLbl, "24H");
  }

  lv_label_set_text_fmt(m_timeLbl, "%02d:%02d", displayHour, ti.tm_min);
  lv_label_set_text_fmt(m_secLbl, ":%02d", ti.tm_sec);

  char dateBuf[48];
  strftime(dateBuf, sizeof(dateBuf), "%a, %b %d, %Y", &ti);
  const int32_t gmtHours = Settings::getGmtOffsetSec() / 3600;
  lv_label_set_text_fmt(m_dateLbl, "%s  (UTC%+ld)", dateBuf,
                        static_cast<long>(gmtHours));

  if (m_syncBadge != nullptr) {
    lv_label_set_text(m_syncBadge, "NTP OK");
    lv_obj_set_style_text_color(m_syncBadge, lv_color_hex(Theme::ACCENT_GREEN),
                                LV_PART_MAIN);
  }
}

void ClockApp::refreshNetwork(const DataSnapshot &d) {
  if (m_netLbl == nullptr) {
    return;
  }

  if (d.wifiConnected) {
    lv_label_set_text_fmt(m_netLbl, "%s  |  %s  |  SEL: 12/24h", d.wifiSsid,
                          d.wifiIp);
    lv_obj_set_style_text_color(m_netLbl, lv_color_hex(Theme::ACCENT_GREEN),
                                LV_PART_MAIN);
  } else {
    lv_label_set_text_fmt(m_netLbl,
                          "WiFi connecting to '%s'... (Hold SEL: Retry)",
                          d.wifiSsid[0] ? d.wifiSsid : "none");
    lv_obj_set_style_text_color(m_netLbl, lv_color_hex(Theme::ACCENT_AMBER),
                                LV_PART_MAIN);
  }

  if (m_syncBadge != nullptr) {
    if (d.ntpSynced) {
      lv_label_set_text(m_syncBadge, "NTP OK");
      lv_obj_set_style_text_color(
          m_syncBadge, lv_color_hex(Theme::ACCENT_GREEN), LV_PART_MAIN);
    } else {
      lv_label_set_text(m_syncBadge, "SYNCING");
      lv_obj_set_style_text_color(
          m_syncBadge, lv_color_hex(Theme::ACCENT_AMBER), LV_PART_MAIN);
    }
  }
}
