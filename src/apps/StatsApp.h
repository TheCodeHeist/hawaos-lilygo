#pragma once

#include "core/App.h"
#include <lvgl.h>

class StatsApp final : public App {
public:
  const char *name() const override { return "Stats"; }
  const char *icon() const override { return ICON_ACTIVITY; }
  bool needsCompanion() const override { return true; }

  void onEnter(lv_obj_t *root) override;
  void onExit() override;
  void onTick(uint32_t nowMs) override;
  bool onButton(Button b, Press p) override;
  void onData(const DataSnapshot &d) override;

private:
  void refreshStats(const DataSnapshot &d);

  lv_obj_t *m_cpuTitleLbl = nullptr;
  lv_obj_t *m_cpuValLbl = nullptr;
  lv_obj_t *m_cpuBar = nullptr;

  lv_obj_t *m_ramTitleLbl = nullptr;
  lv_obj_t *m_ramValLbl = nullptr;
  lv_obj_t *m_ramBar = nullptr;

  lv_obj_t *m_netTitleLbl = nullptr;
  lv_obj_t *m_netUpDownLbl = nullptr;
  lv_obj_t *m_chart = nullptr;
  lv_chart_series_t *m_netSeries = nullptr;
  bool m_showOnDevice = false;
};
