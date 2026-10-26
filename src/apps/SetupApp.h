#pragma once

#include "core/App.h"
#include "services/SetupService.h"
#include <lvgl.h>

// First-run Setup screen (DESIGN.md Section 9.9). Not in the launcher:
// AppManager shows it until SetupService reports the wizard is done.
class SetupApp final : public App {
public:
  const char *name() const override { return "Setup"; }
  const char *icon() const override { return ICON_SETTINGS; }
  bool hidesStatusBar() const override { return true; }
  bool usesWallpaper() const override { return true; }

  void onEnter(lv_obj_t *root) override;
  void onExit() override;
  void onTick(uint32_t nowMs) override;
  bool onButton(Button b, Press p) override;

private:
  void refresh(const SetupService::Snapshot &s);
  void buildBadge(const SetupService::Snapshot &s);

  lv_obj_t *m_kickerLbl = nullptr;
  lv_obj_t *m_titleLbl = nullptr;
  lv_obj_t *m_bodyLbl = nullptr;
  lv_obj_t *m_hintLbl = nullptr;
  lv_obj_t *m_card = nullptr;

  uint32_t m_generation = 0xFFFFFFFF;
  // Badge content is rebuilt only when what it shows changes.
  SetupService::Stage m_badgeStage = SetupService::Stage::Done;
  SetupService::Link m_badgeLink = SetupService::Link::None;
  bool m_badgePaired = false;
  char m_badgePin[7] = {0};
  bool m_skipped = false;
};
