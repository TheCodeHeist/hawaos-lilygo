#pragma once

#include "core/App.h"
#include <lvgl.h>

class SettingsApp final : public App {
public:
  const char *name() const override { return "Settings"; }
  const char *icon() const override { return ICON_SETTINGS; }

  void onEnter(lv_obj_t *root) override;
  void onExit() override;
  void onTick(uint32_t nowMs) override;
  bool onButton(Button b, Press p) override;
  void onData(const DataSnapshot &d) override;

private:
  void refreshRows();

  static constexpr int kVisibleRows = 3;
  static constexpr int kTotalItems = 11;

  lv_obj_t *m_rows[kVisibleRows] = {nullptr};
  lv_obj_t *m_rowNames[kVisibleRows] = {nullptr};
  lv_obj_t *m_rowVals[kVisibleRows] = {nullptr};

  int m_cursor = 0;
  int m_scrollTop = 0;
};
