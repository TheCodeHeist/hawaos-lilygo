#pragma once

#include "core/App.h"
#include <lvgl.h>

class NotifyApp final : public App {
public:
  const char *name() const override { return "Notifications"; }
  const char *icon() const override { return ICON_BELL; }

  void onEnter(lv_obj_t *root) override;
  void onExit() override;
  void onTick(uint32_t nowMs) override;
  bool onButton(Button b, Press p) override;
  void onData(const DataSnapshot &d) override;

private:
  void refreshList(const DataSnapshot &d);

  static constexpr int kVisibleRows = 2;
  lv_obj_t *m_emptyIcon = nullptr;
  lv_obj_t *m_emptyLbl = nullptr;

  lv_obj_t *m_cards[kVisibleRows] = {nullptr};
  lv_obj_t *m_icons[kVisibleRows] = {nullptr};
  lv_obj_t *m_titles[kVisibleRows] = {nullptr};
  lv_obj_t *m_bodies[kVisibleRows] = {nullptr};
  lv_obj_t *m_times[kVisibleRows] = {nullptr};

  uint8_t m_cursor = 0;
  uint8_t m_scrollTop = 0;
};
