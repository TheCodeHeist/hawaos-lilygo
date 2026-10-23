#pragma once

#include "core/App.h"
#include <lvgl.h>

class MediaApp final : public App {
public:
  const char *name() const override { return "Media"; }
  const char *icon() const override { return ICON_MUSIC; }
  bool hidesStatusBar() const override { return true; }

  bool wantsDouble(Button b) const override { return b == Button::Next; }

  void onEnter(lv_obj_t *root) override;
  void onExit() override;
  void onTick(uint32_t nowMs) override;
  bool onButton(Button b, Press p) override;
  void onData(const DataSnapshot &d) override;

private:
  void refreshMedia(const DataSnapshot &d);
  void refreshProgress(const DataSnapshot &d);

  lv_obj_t *m_root = nullptr;
  lv_obj_t *m_artCard = nullptr;
  lv_obj_t *m_artPlaceholderIcon = nullptr;
  lv_obj_t *m_artImg = nullptr;
  lv_img_dsc_t m_artDsc{};

  lv_obj_t *m_titleLbl = nullptr;
  lv_obj_t *m_artistLbl = nullptr;
  lv_obj_t *m_progBar = nullptr;
  lv_obj_t *m_posLbl = nullptr;
  lv_obj_t *m_durLbl = nullptr;
  lv_obj_t *m_playBtnCircle = nullptr;
  lv_obj_t *m_playIconLbl = nullptr;

  uint32_t m_lastProgressSec = 0xFFFFFFFF;
};
