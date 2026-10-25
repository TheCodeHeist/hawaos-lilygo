#pragma once

#include "core/App.h"
#include "ui/icons.h"

class ClockApp final : public App {
public:
  const char *name() const override { return "Clock"; }
  const char *icon() const override { return ICON_CLOCK; }
  bool usesWallpaper() const override { return true; }

  void onEnter(lv_obj_t *root) override;
  void onExit() override;
  void onTick(uint32_t nowMs) override;
  bool onButton(Button b, Press p) override;
  void onData(const DataSnapshot &d) override;

private:
  void refreshTime();
  void refreshStopwatch(const DataSnapshot &d);
  void refreshTimer(const DataSnapshot &d);
  void updateFocusRing();

  // Left side: Digital Clock
  lv_obj_t *m_clockCard = nullptr;
  lv_obj_t *m_clockTagLbl = nullptr;
  lv_obj_t *m_timeLbl = nullptr;
  lv_obj_t *m_ampmLbl = nullptr;
  lv_obj_t *m_dateLbl = nullptr;
  lv_obj_t *m_syncLbl = nullptr;

  // Right side: Stopwatch card
  lv_obj_t *m_swCard = nullptr;
  lv_obj_t *m_swIcon = nullptr;
  lv_obj_t *m_swTagLbl = nullptr;
  lv_obj_t *m_swDigitsLbl = nullptr;
  lv_obj_t *m_swMsLbl = nullptr;
  lv_obj_t *m_swHintLbl = nullptr;

  // Right side: Timer card
  lv_obj_t *m_tmCard = nullptr;
  lv_obj_t *m_tmIcon = nullptr;
  lv_obj_t *m_tmTagLbl = nullptr;
  lv_obj_t *m_tmDigitsLbl = nullptr;
  lv_obj_t *m_tmMsLbl = nullptr;
  lv_obj_t *m_tmHintLbl = nullptr;
  lv_obj_t *m_tmProgBar = nullptr;

  uint8_t m_focusedSlot = 0; // 0 = Stopwatch, 1 = Timer
  uint32_t m_lastSec = 0xFFFFFFFF;
  uint32_t m_lastSubSecMs = 0;
};
