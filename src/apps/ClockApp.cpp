#include "apps/ClockApp.h"
#include "config.h"
#include "core/AppManager.h"
#include "core/Settings.h"
#include "ui/Theme.h"
#include "ui/icons.h"

#include <Arduino.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

namespace {

void setLabelTextIfChanged(lv_obj_t *lbl, const char *newText) {
  if (lbl == nullptr || newText == nullptr)
    return;
  const char *cur = lv_label_get_text(lbl);
  if (cur == nullptr || strcmp(cur, newText) != 0) {
    lv_label_set_text(lbl, newText);
  }
}

} // namespace

void ClockApp::onEnter(lv_obj_t *root) {
  const ThemeTokens::Palette &p = Theme::active();

  // --- Left Column: Hero Time (48 SemiBold) + AM/PM + Date (Body 14, textDim)
  // ---
  m_timeLbl = lv_label_create(root);
  lv_label_set_text(m_timeLbl, "--:--");
  lv_obj_set_style_text_font(m_timeLbl, Theme::fontHero(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_timeLbl, lv_color_hex(p.text), LV_PART_MAIN);
  lv_obj_set_pos(m_timeLbl, Config::OUTER_MARGIN_PX,
                 Config::CONTENT_Y_START + 12);

  m_ampmLbl = lv_label_create(root);
  lv_label_set_text(m_ampmLbl, "");
  lv_obj_set_style_text_font(m_ampmLbl, Theme::fontBodySb(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_ampmLbl, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_align_to(m_ampmLbl, m_timeLbl, LV_ALIGN_OUT_RIGHT_BOTTOM, 6, -8);

  m_dateLbl = lv_label_create(root);
  lv_label_set_text(m_dateLbl, "Syncing time...");
  lv_obj_set_style_text_font(m_dateLbl, Theme::fontBody(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_dateLbl, lv_color_hex(p.textDim), LV_PART_MAIN);
  lv_obj_set_pos(m_dateLbl, Config::OUTER_MARGIN_PX + 2,
                 Config::CONTENT_Y_START + 68);

  // --- Right Column: Two small solid chips with CPU % and RAM % (radius 8) ---
  m_cpuChip = Theme::createCard(
      root, 108, 36, 202, Config::CONTENT_Y_START + 10, Config::RADIUS_SM);
  lv_obj_set_style_pad_hor(m_cpuChip, 10, LV_PART_MAIN);
  lv_obj_set_style_pad_ver(m_cpuChip, 6, LV_PART_MAIN);

  lv_obj_t *cpuIcon = lv_label_create(m_cpuChip);
  lv_label_set_text(cpuIcon, ICON_CPU);
  lv_obj_set_style_text_font(cpuIcon, Theme::iconFont20(), LV_PART_MAIN);
  lv_obj_set_style_text_color(cpuIcon, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_align(cpuIcon, LV_ALIGN_LEFT_MID, 0, 0);

  m_cpuValLbl = lv_label_create(m_cpuChip);
  lv_label_set_text(m_cpuValLbl, "CPU --%");
  lv_obj_set_style_text_font(m_cpuValLbl, Theme::fontBodySb(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_cpuValLbl, lv_color_hex(p.text), LV_PART_MAIN);
  lv_obj_align(m_cpuValLbl, LV_ALIGN_RIGHT_MID, 0, 0);

  m_ramChip = Theme::createCard(
      root, 108, 36, 202, Config::CONTENT_Y_START + 54, Config::RADIUS_SM);
  lv_obj_set_style_pad_hor(m_ramChip, 10, LV_PART_MAIN);
  lv_obj_set_style_pad_ver(m_ramChip, 6, LV_PART_MAIN);

  lv_obj_t *ramIcon = lv_label_create(m_ramChip);
  lv_label_set_text(ramIcon, ICON_MEMORY_STICK);
  lv_obj_set_style_text_font(ramIcon, Theme::iconFont20(), LV_PART_MAIN);
  lv_obj_set_style_text_color(ramIcon, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_align(ramIcon, LV_ALIGN_LEFT_MID, 0, 0);

  m_ramValLbl = lv_label_create(m_ramChip);
  lv_label_set_text(m_ramValLbl, "RAM --%");
  lv_obj_set_style_text_font(m_ramValLbl, Theme::fontBodySb(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_ramValLbl, lv_color_hex(p.text), LV_PART_MAIN);
  lv_obj_align(m_ramValLbl, LV_ALIGN_RIGHT_MID, 0, 0);

  // --- Bottom Now-Playing / Status Chip (radius 8, width 300, height 34) ---
  m_mediaChip =
      Theme::createCard(root, 300, 34, Config::OUTER_MARGIN_PX,
                        Config::CONTENT_Y_START + 104, Config::RADIUS_SM);
  lv_obj_set_style_pad_hor(m_mediaChip, 10, LV_PART_MAIN);
  lv_obj_set_style_pad_ver(m_mediaChip, 4, LV_PART_MAIN);

  lv_obj_t *musicIcon = lv_label_create(m_mediaChip);
  lv_label_set_text(musicIcon, ICON_MUSIC);
  lv_obj_set_style_text_font(musicIcon, Theme::iconFont20(), LV_PART_MAIN);
  lv_obj_set_style_text_color(musicIcon, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_align(musicIcon, LV_ALIGN_LEFT_MID, 0, 0);

  m_mediaTextLbl = lv_label_create(m_mediaChip);
  lv_label_set_long_mode(m_mediaTextLbl, LV_LABEL_LONG_SCROLL_CIRCULAR);
  lv_obj_set_style_anim_speed(m_mediaTextLbl, 26, LV_PART_MAIN);
  lv_obj_set_width(m_mediaTextLbl, 252);
  lv_obj_set_style_text_font(m_mediaTextLbl, Theme::fontBody(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_mediaTextLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_align(m_mediaTextLbl, LV_ALIGN_LEFT_MID, 28, 0);

  m_lastSec = 0xFFFFFFFF;
  refreshDashboard(DataStore::getSnapshot());
  refreshTime();
}

void ClockApp::onExit() {
  m_timeLbl = nullptr;
  m_ampmLbl = nullptr;
  m_dateLbl = nullptr;
  m_cpuChip = nullptr;
  m_cpuValLbl = nullptr;
  m_ramChip = nullptr;
  m_ramValLbl = nullptr;
  m_mediaChip = nullptr;
  m_mediaTextLbl = nullptr;
}

void ClockApp::onTick(uint32_t /*nowMs*/) { refreshTime(); }

bool ClockApp::onButton(Button b, Press p) {
  // DESIGN.md Section 9.3: Select short opens Media if playing, or toggles
  // 12/24h clock
  if (b == Button::Select && p == Press::Short) {
    const DataSnapshot snap = DataStore::getSnapshot();
    if (snap.mediaState == MediaPlayState::Playing &&
        AppManager::appCount() > 1) {
      AppManager::switchTo(1);
      return true;
    }
    Settings::setUse24Hour(!Settings::getUse24Hour());
    Settings::bumpLocalConfigVersion();
    m_lastSec = 0xFFFFFFFF;
    refreshTime();
    return true;
  }
  return false;
}

void ClockApp::onData(const DataSnapshot &d) {
  refreshDashboard(d);
  m_lastSec = 0xFFFFFFFF;
  refreshTime();
}

void ClockApp::refreshTime() {
  if (m_timeLbl == nullptr)
    return;

  const time_t now = time(nullptr);
  if (now <= 1700000000) {
    const uint32_t upSec = millis() / 1000;
    if (upSec == m_lastSec)
      return;
    m_lastSec = upSec;
    lv_label_set_text_fmt(m_timeLbl, "%02lu:%02lu",
                          static_cast<unsigned long>((upSec / 60) % 60),
                          static_cast<unsigned long>(upSec % 60));
    if (m_ampmLbl != nullptr) {
      lv_label_set_text(m_ampmLbl, "");
    }
    return;
  }

  if (static_cast<uint32_t>(now) == m_lastSec)
    return;
  m_lastSec = static_cast<uint32_t>(now);

  struct tm ti{};
  localtime_r(&now, &ti);

  const bool use24 = Settings::getUse24Hour();
  int displayHour = ti.tm_hour;
  if (!use24) {
    displayHour = ti.tm_hour % 12;
    if (displayHour == 0)
      displayHour = 12;
  }

  lv_label_set_text_fmt(m_timeLbl, "%02d:%02d", displayHour, ti.tm_min);

  if (m_ampmLbl != nullptr) {
    if (!use24) {
      lv_label_set_text(m_ampmLbl, (ti.tm_hour >= 12) ? "PM" : "AM");
      lv_obj_align_to(m_ampmLbl, m_timeLbl, LV_ALIGN_OUT_RIGHT_BOTTOM, 6, -8);
    } else {
      lv_label_set_text(m_ampmLbl, "");
    }
  }

  char dateBuf[40];
  strftime(dateBuf, sizeof(dateBuf), "%a \u00b7 %b %d", &ti);
  lv_label_set_text(m_dateLbl, dateBuf);
}

void ClockApp::refreshDashboard(const DataSnapshot &d) {
  if (m_cpuValLbl == nullptr)
    return;
  const ThemeTokens::Palette &p = Theme::active();

  if (d.lastStatsMs > 0) {
    lv_label_set_text_fmt(m_cpuValLbl, "CPU %u%%",
                          static_cast<unsigned>(d.cpuPercent));
    lv_label_set_text_fmt(m_ramValLbl, "RAM %u%%",
                          static_cast<unsigned>(d.ramPercent));
  } else {
    lv_label_set_text(m_cpuValLbl, "CPU --%");
    lv_label_set_text(m_ramValLbl, "RAM --%");
  }

  char chipText[140] = {0};
  if (d.mediaState == MediaPlayState::Playing && d.mediaTitle[0] != '\0') {
    lv_obj_clear_flag(m_mediaChip, LV_OBJ_FLAG_HIDDEN);
    Theme::setFocusRing(m_mediaChip, true);
    lv_obj_set_style_text_color(m_mediaTextLbl, lv_color_hex(p.text),
                                LV_PART_MAIN);
    if (d.mediaArtist[0] != '\0') {
      snprintf(chipText, sizeof(chipText), "%s \u00b7 %s", d.mediaTitle,
               d.mediaArtist);
    } else {
      snprintf(chipText, sizeof(chipText), "%s", d.mediaTitle);
    }
  } else {
    Theme::setFocusRing(m_mediaChip, false);
    lv_obj_set_style_text_color(m_mediaTextLbl, lv_color_hex(p.textDim),
                                LV_PART_MAIN);
    if (d.wifiConnected) {
      snprintf(chipText, sizeof(chipText), "%s \u00b7 %s", d.wifiSsid,
               d.wifiIp);
    } else if (d.wifiSsid[0] != '\0') {
      snprintf(chipText, sizeof(chipText), "Connecting to %s...", d.wifiSsid);
    } else {
      snprintf(chipText, sizeof(chipText),
               "Setup WiFi in HawaOS Companion (USB)");
    }
  }
  setLabelTextIfChanged(m_mediaTextLbl, chipText);
}
