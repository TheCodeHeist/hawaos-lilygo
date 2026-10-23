#pragma once

#include "core/App.h"
#include "ui/icons.h"

class ClockApp final : public App {
public:
  const char *name() const override { return "Home"; }
  const char *icon() const override { return ICON_HOUSE; }
  bool usesWallpaper() const override {
    return true;
  } // DESIGN.md Section 6: Home uses wallpaper

  void onEnter(lv_obj_t *root) override;
  void onExit() override;
  void onTick(uint32_t nowMs) override;
  bool onButton(Button b, Press p) override;
  void onData(const DataSnapshot &d) override;

private:
  void refreshTime();
  void refreshDashboard(const DataSnapshot &d);

  lv_obj_t *m_timeLbl = nullptr;
  lv_obj_t *m_ampmLbl = nullptr;
  lv_obj_t *m_dateLbl = nullptr;
  lv_obj_t *m_cpuChip = nullptr;
  lv_obj_t *m_cpuValLbl = nullptr;
  lv_obj_t *m_ramChip = nullptr;
  lv_obj_t *m_ramValLbl = nullptr;
  lv_obj_t *m_mediaChip = nullptr;
  lv_obj_t *m_mediaTextLbl = nullptr;
  uint32_t m_lastSec = 0xFFFFFFFF;
};
