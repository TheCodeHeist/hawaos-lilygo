#pragma once

#include "core/App.h"

class ClockApp final : public App {
public:
  const char *name() const override { return "Clock"; }
  void onEnter(lv_obj_t *root) override;
  void onExit() override;
  void onTick(uint32_t nowMs) override;
  bool onButton(Button b, Press p) override;
  void onData(const DataSnapshot &d) override;

private:
  void refreshTime();
  void refreshNetwork(const DataSnapshot &d);

  lv_obj_t *m_timeLbl = nullptr;
  lv_obj_t *m_secLbl = nullptr;
  lv_obj_t *m_ampmLbl = nullptr;
  lv_obj_t *m_dateLbl = nullptr;
  lv_obj_t *m_netLbl = nullptr;
  lv_obj_t *m_syncBadge = nullptr;
  uint32_t m_lastSec = 0xFFFFFFFF;
};
