#include "apps/ClockApp.h"
#include "config.h"
#include "core/AppManager.h"
#include "core/DataStore.h"
#include "core/Settings.h"
#include "services/CompanionLink.h"
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
  m_focusedSlot = 0;

  // --- Left Card: Digital Clock & Calendar (144x136) ---
  m_clockCard =
      Theme::createCard(root, 142, 136, Config::OUTER_MARGIN_PX,
                        Config::CONTENT_Y_START + 6, Config::RADIUS_MD);
  lv_obj_set_style_pad_all(m_clockCard, 10, LV_PART_MAIN);

  m_clockTagLbl = lv_label_create(m_clockCard);
  lv_label_set_text(m_clockTagLbl, "TIME & DATE");
  lv_obj_set_style_text_font(m_clockTagLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_clockTagLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_align(m_clockTagLbl, LV_ALIGN_TOP_LEFT, 0, 0);

  m_timeLbl = lv_label_create(m_clockCard);
  lv_label_set_text(m_timeLbl, "--:--");
  lv_obj_set_style_text_font(m_timeLbl, Theme::fontDisplay(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_timeLbl, lv_color_hex(p.text), LV_PART_MAIN);
  lv_obj_align(m_timeLbl, LV_ALIGN_TOP_LEFT, 0, 20);

  m_ampmLbl = lv_label_create(m_clockCard);
  lv_label_set_text(m_ampmLbl, "");
  lv_obj_set_style_text_font(m_ampmLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_ampmLbl, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_align(m_ampmLbl, LV_ALIGN_TOP_LEFT, 0, 52);

  m_dateLbl = lv_label_create(m_clockCard);
  lv_label_set_text(m_dateLbl, "Syncing time...");
  lv_obj_set_style_text_font(m_dateLbl, Theme::fontBody(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_dateLbl, lv_color_hex(p.text), LV_PART_MAIN);
  lv_obj_align(m_dateLbl, LV_ALIGN_TOP_LEFT, 0, 72);

  m_syncLbl = lv_label_create(m_clockCard);
  lv_label_set_text(m_syncLbl, "UTC+6");
  lv_obj_set_style_text_font(m_syncLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_syncLbl, lv_color_hex(p.textDim), LV_PART_MAIN);
  lv_obj_align(m_syncLbl, LV_ALIGN_BOTTOM_LEFT, 0, 0);

  // --- Right Column Card 1: STOPWATCH (154x65) ---
  m_swCard = Theme::createCard(root, 154, 65, 156, Config::CONTENT_Y_START + 6,
                               Config::RADIUS_MD);
  // Three rows (header, digits, hint): the hint is too wide to share the
  // digits row, so the vertical padding is tighter than the horizontal.
  lv_obj_set_style_pad_hor(m_swCard, 8, LV_PART_MAIN);
  lv_obj_set_style_pad_ver(m_swCard, 6, LV_PART_MAIN);

  m_swIcon = lv_label_create(m_swCard);
  lv_label_set_text(m_swIcon, ICON_CLOCK);
  lv_obj_set_style_text_font(m_swIcon, Theme::iconFont20(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_swIcon, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_align(m_swIcon, LV_ALIGN_TOP_LEFT, 0, 0);

  m_swTagLbl = lv_label_create(m_swCard);
  lv_label_set_text(m_swTagLbl, "STOPWATCH");
  lv_obj_set_style_text_font(m_swTagLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_swTagLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_align(m_swTagLbl, LV_ALIGN_TOP_LEFT, 26, 2);

  m_swDigitsLbl = lv_label_create(m_swCard);
  lv_label_set_text(m_swDigitsLbl, "00:00:00");
  lv_obj_set_style_text_font(m_swDigitsLbl, Theme::fontBodySb(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_swDigitsLbl, lv_color_hex(p.text),
                              LV_PART_MAIN);
  lv_obj_align(m_swDigitsLbl, LV_ALIGN_TOP_LEFT, 0, 17);

  m_swMsLbl = lv_label_create(m_swCard);
  lv_label_set_text(m_swMsLbl, ".000");
  lv_obj_set_style_text_font(m_swMsLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_swMsLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_align_to(m_swMsLbl, m_swDigitsLbl, LV_ALIGN_OUT_RIGHT_BOTTOM, 2, -1);

  m_swHintLbl = lv_label_create(m_swCard);
  lv_label_set_text(m_swHintLbl, "[Select] Start");
  lv_obj_set_style_text_font(m_swHintLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_swHintLbl, lv_color_hex(p.accent),
                              LV_PART_MAIN);
  lv_obj_align(m_swHintLbl, LV_ALIGN_BOTTOM_RIGHT, 0, 0);

  // --- Right Column Card 2: TIMER (154x65) ---
  m_tmCard = Theme::createCard(root, 154, 65, 156, Config::CONTENT_Y_START + 77,
                               Config::RADIUS_MD);
  lv_obj_set_style_pad_hor(m_tmCard, 8, LV_PART_MAIN);
  lv_obj_set_style_pad_ver(m_tmCard, 6, LV_PART_MAIN);

  m_tmIcon = lv_label_create(m_tmCard);
  lv_label_set_text(m_tmIcon, ICON_CLOCK);
  lv_obj_set_style_text_font(m_tmIcon, Theme::iconFont20(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_tmIcon, lv_color_hex(p.accent), LV_PART_MAIN);
  lv_obj_align(m_tmIcon, LV_ALIGN_TOP_LEFT, 0, 0);

  m_tmTagLbl = lv_label_create(m_tmCard);
  lv_label_set_text(m_tmTagLbl, "TIMER");
  lv_obj_set_style_text_font(m_tmTagLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_tmTagLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_align(m_tmTagLbl, LV_ALIGN_TOP_LEFT, 26, 2);

  m_tmDigitsLbl = lv_label_create(m_tmCard);
  lv_label_set_text(m_tmDigitsLbl, "00:05:00");
  lv_obj_set_style_text_font(m_tmDigitsLbl, Theme::fontBodySb(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_tmDigitsLbl, lv_color_hex(p.text),
                              LV_PART_MAIN);
  lv_obj_align(m_tmDigitsLbl, LV_ALIGN_TOP_LEFT, 0, 17);

  m_tmMsLbl = lv_label_create(m_tmCard);
  lv_label_set_text(m_tmMsLbl, ".000");
  lv_obj_set_style_text_font(m_tmMsLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_tmMsLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_align_to(m_tmMsLbl, m_tmDigitsLbl, LV_ALIGN_OUT_RIGHT_BOTTOM, 2, -1);

  m_tmHintLbl = lv_label_create(m_tmCard);
  lv_label_set_text(m_tmHintLbl, "[Select] Start");
  lv_obj_set_style_text_font(m_tmHintLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_tmHintLbl, lv_color_hex(p.accent),
                              LV_PART_MAIN);
  lv_obj_align(m_tmHintLbl, LV_ALIGN_BOTTOM_RIGHT, 0, 0);

  m_tmProgBar = lv_bar_create(m_tmCard);
  lv_obj_set_size(m_tmProgBar, 138, 2);
  lv_obj_align(m_tmProgBar, LV_ALIGN_BOTTOM_MID, 0, 4);
  lv_bar_set_range(m_tmProgBar, 0, 100);
  lv_obj_set_style_bg_color(m_tmProgBar, lv_color_hex(p.border), LV_PART_MAIN);
  lv_obj_set_style_bg_color(m_tmProgBar, lv_color_hex(p.accent),
                            LV_PART_INDICATOR);

  updateFocusRing();

  m_lastSec = 0xFFFFFFFF;
  m_lastSubSecMs = 0;
  refreshTime();
  onData(DataStore::getSnapshot());
}

void ClockApp::onExit() {
  m_clockCard = nullptr;
  m_clockTagLbl = nullptr;
  m_timeLbl = nullptr;
  m_ampmLbl = nullptr;
  m_dateLbl = nullptr;
  m_syncLbl = nullptr;

  m_swCard = nullptr;
  m_swIcon = nullptr;
  m_swTagLbl = nullptr;
  m_swDigitsLbl = nullptr;
  m_swMsLbl = nullptr;
  m_swHintLbl = nullptr;

  m_tmCard = nullptr;
  m_tmIcon = nullptr;
  m_tmTagLbl = nullptr;
  m_tmDigitsLbl = nullptr;
  m_tmMsLbl = nullptr;
  m_tmHintLbl = nullptr;
  m_tmProgBar = nullptr;
}

void ClockApp::updateFocusRing() {
  Theme::setFocusRing(m_swCard, m_focusedSlot == 0);
  Theme::setFocusRing(m_tmCard, m_focusedSlot == 1);
}

void ClockApp::onTick(uint32_t nowMs) {
  const uint32_t nowSec = nowMs / 1000;
  if (nowSec != m_lastSec) {
    m_lastSec = nowSec;
    refreshTime();
  }

  // Refresh stopwatch & timer sub-seconds (20 Hz)
  if (nowMs - m_lastSubSecMs >= 50) {
    m_lastSubSecMs = nowMs;
    const DataSnapshot d = DataStore::getSnapshot();
    refreshStopwatch(d);
    refreshTimer(d);
  }
}

bool ClockApp::onButton(Button b, Press p) {
  if (b == Button::Next && p == Press::Short) {
    m_focusedSlot = (m_focusedSlot == 0) ? 1 : 0;
    updateFocusRing();
    return true;
  }

  if (b == Button::Select && p == Press::Short) {
    if (m_focusedSlot == 0) {
      DataStore::toggleStopwatch();
      CompanionLink::sendClockCommand("stopwatch_toggle");
    } else {
      DataStore::toggleTimer();
      CompanionLink::sendClockCommand("timer_toggle");
    }
    const DataSnapshot d = DataStore::getSnapshot();
    refreshStopwatch(d);
    refreshTimer(d);
    return true;
  }

  if (b == Button::Select && p == Press::Long) {
    if (m_focusedSlot == 0) {
      DataStore::resetStopwatch();
      CompanionLink::sendClockCommand("stopwatch_reset");
    } else {
      DataStore::resetTimer();
      CompanionLink::sendClockCommand("timer_reset");
    }
    const DataSnapshot d = DataStore::getSnapshot();
    refreshStopwatch(d);
    refreshTimer(d);
    return true;
  }

  return false;
}

void ClockApp::onData(const DataSnapshot &d) {
  refreshStopwatch(d);
  refreshTimer(d);
  if (m_syncLbl != nullptr) {
    setLabelTextIfChanged(m_syncLbl, d.ntpSynced ? "NTP Synced" : "Local Time");
  }
}

void ClockApp::refreshTime() {
  if (m_timeLbl == nullptr)
    return;

  const time_t now = time(nullptr);
  const bool valid = (now > 1700000000);
  const bool use24 = Settings::getUse24Hour();

  char timeBuf[16] = "--:--";
  char ampmBuf[32] = "";
  char dateBuf[32] = "Syncing NTP...";

  if (valid) {
    struct tm ti{};
    localtime_r(&now, &ti);

    int hr = ti.tm_hour;
    if (!use24) {
      const char *ampm = (ti.tm_hour >= 12) ? "PM" : "AM";
      hr = ti.tm_hour % 12;
      if (hr == 0)
        hr = 12;
      snprintf(ampmBuf, sizeof(ampmBuf), ":%02d %s", ti.tm_sec, ampm);
    } else {
      snprintf(ampmBuf, sizeof(ampmBuf), ":%02d", ti.tm_sec);
    }
    snprintf(timeBuf, sizeof(timeBuf), "%02d:%02d", hr, ti.tm_min);
    strftime(dateBuf, sizeof(dateBuf), "%a, %b %d", &ti);
  } else {
    const uint32_t up = millis() / 1000;
    const int hr = static_cast<int>((up / 3600) % 24);
    const int mn = static_cast<int>((up / 60) % 60);
    const int sc = static_cast<int>(up % 60);
    snprintf(timeBuf, sizeof(timeBuf), "%02d:%02d", hr, mn);
    snprintf(ampmBuf, sizeof(ampmBuf), ":%02d", sc);
  }

  setLabelTextIfChanged(m_timeLbl, timeBuf);
  if (m_ampmLbl != nullptr) {
    setLabelTextIfChanged(m_ampmLbl, ampmBuf);
  }
  if (m_dateLbl != nullptr) {
    setLabelTextIfChanged(m_dateLbl, dateBuf);
  }
}

void ClockApp::refreshStopwatch(const DataSnapshot &d) {
  if (m_swDigitsLbl == nullptr)
    return;

  const uint32_t nowMs = millis();
  const uint32_t elapsed = DataStore::getStopwatchElapsedMs(nowMs);
  const uint32_t totalSec = elapsed / 1000;
  const uint32_t hr = totalSec / 3600;
  const uint32_t mn = (totalSec / 60) % 60;
  const uint32_t sc = totalSec % 60;
  const uint32_t ms = elapsed % 1000;

  char swBuf[16];
  snprintf(swBuf, sizeof(swBuf), "%02lu:%02lu:%02lu",
           static_cast<unsigned long>(hr), static_cast<unsigned long>(mn),
           static_cast<unsigned long>(sc));
  setLabelTextIfChanged(m_swDigitsLbl, swBuf);

  if (m_swMsLbl != nullptr) {
    char msBuf[8];
    snprintf(msBuf, sizeof(msBuf), ".%03lu", static_cast<unsigned long>(ms));
    setLabelTextIfChanged(m_swMsLbl, msBuf);
    lv_obj_align_to(m_swMsLbl, m_swDigitsLbl, LV_ALIGN_OUT_RIGHT_BOTTOM, 2, -1);
  }

  if (m_swHintLbl != nullptr) {
    if (d.stopwatch.state == StopwatchState::Running) {
      setLabelTextIfChanged(m_swHintLbl, "Running");
      lv_obj_set_style_text_color(
          m_swHintLbl, lv_color_hex(Theme::active().accent), LV_PART_MAIN);
    } else if (d.stopwatch.state == StopwatchState::Paused) {
      setLabelTextIfChanged(m_swHintLbl, "Paused");
      lv_obj_set_style_text_color(
          m_swHintLbl, lv_color_hex(Theme::active().textDim), LV_PART_MAIN);
    } else {
      setLabelTextIfChanged(m_swHintLbl, "[Select] Start");
      lv_obj_set_style_text_color(
          m_swHintLbl, lv_color_hex(Theme::active().textDim), LV_PART_MAIN);
    }
  }
}

void ClockApp::refreshTimer(const DataSnapshot &d) {
  if (m_tmDigitsLbl == nullptr)
    return;

  const uint32_t nowMs = millis();
  const uint32_t remMs = DataStore::getTimerRemainingMs(nowMs);
  const uint32_t totalSec = remMs / 1000;
  const uint32_t hr = totalSec / 3600;
  const uint32_t mn = (totalSec / 60) % 60;
  const uint32_t sc = totalSec % 60;
  const uint32_t ms = remMs % 1000;

  char tmBuf[16];
  snprintf(tmBuf, sizeof(tmBuf), "%02lu:%02lu:%02lu",
           static_cast<unsigned long>(hr), static_cast<unsigned long>(mn),
           static_cast<unsigned long>(sc));
  setLabelTextIfChanged(m_tmDigitsLbl, tmBuf);

  if (m_tmMsLbl != nullptr) {
    char msBuf[8];
    snprintf(msBuf, sizeof(msBuf), ".%03lu", static_cast<unsigned long>(ms));
    setLabelTextIfChanged(m_tmMsLbl, msBuf);
    lv_obj_align_to(m_tmMsLbl, m_tmDigitsLbl, LV_ALIGN_OUT_RIGHT_BOTTOM, 2, -1);
  }

  if (m_tmProgBar != nullptr) {
    int pct = 0;
    if (d.timer.durationSec > 0) {
      const uint32_t elapsedSec = (d.timer.durationSec >= totalSec)
                                      ? (d.timer.durationSec - totalSec)
                                      : d.timer.durationSec;
      pct = static_cast<int>((elapsedSec * 100) / d.timer.durationSec);
    }
    lv_bar_set_value(m_tmProgBar, pct, LV_ANIM_OFF);
  }

  if (m_tmHintLbl != nullptr) {
    if (d.timer.state == TimerState::Running) {
      setLabelTextIfChanged(m_tmHintLbl, "Running");
      lv_obj_set_style_text_color(
          m_tmHintLbl, lv_color_hex(Theme::active().accent), LV_PART_MAIN);
    } else if (d.timer.state == TimerState::Paused) {
      setLabelTextIfChanged(m_tmHintLbl, "Paused");
      lv_obj_set_style_text_color(
          m_tmHintLbl, lv_color_hex(Theme::active().textDim), LV_PART_MAIN);
    } else if (d.timer.state == TimerState::Finished) {
      setLabelTextIfChanged(m_tmHintLbl, "Time's Up!");
      lv_obj_set_style_text_color(
          m_tmHintLbl, lv_color_hex(Theme::active().accent), LV_PART_MAIN);
    } else {
      setLabelTextIfChanged(m_tmHintLbl, "[Select] Start");
      lv_obj_set_style_text_color(
          m_tmHintLbl, lv_color_hex(Theme::active().textDim), LV_PART_MAIN);
    }
  }
}
