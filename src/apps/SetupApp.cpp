#include "apps/SetupApp.h"
#include "config.h"
#include "ui/Theme.h"
#include "ui/icons.h"

#include <stdio.h>
#include <string.h>

using SetupService::Link;
using SetupService::Stage;

namespace {

constexpr int16_t kLeftX = 16;
constexpr int16_t kLeftW = 168;
constexpr int16_t kCardX = 196;
constexpr int16_t kCardY = 14;
constexpr int16_t kCardW = 112;
constexpr int16_t kCardH = 142;

lv_obj_t *makeLabel(lv_obj_t *parent, const lv_font_t *font, uint32_t color) {
  lv_obj_t *lbl = lv_label_create(parent);
  lv_obj_set_style_text_font(lbl, font, LV_PART_MAIN);
  lv_obj_set_style_text_color(lbl, lv_color_hex(color), LV_PART_MAIN);
  return lbl;
}

void setTextIfChanged(lv_obj_t *lbl, const char *text) {
  const char *cur = lv_label_get_text(lbl);
  if (cur == nullptr || strcmp(cur, text) != 0) {
    lv_label_set_text(lbl, text);
  }
}

// Big icon over a one-line caption, centered in the card.
void iconBadge(lv_obj_t *card, const char *icon, uint32_t iconColor,
               const char *caption) {
  const ThemeTokens::Palette &p = Theme::active();
  lv_obj_t *ic = makeLabel(card, Theme::iconFont48(), iconColor);
  lv_label_set_text(ic, icon);
  lv_obj_align(ic, LV_ALIGN_CENTER, 0, -14);

  lv_obj_t *cap = makeLabel(card, Theme::fontCaption(), p.textDim);
  lv_label_set_long_mode(cap, LV_LABEL_LONG_DOT);
  lv_obj_set_width(cap, kCardW - 16);
  lv_obj_set_style_text_align(cap, LV_TEXT_ALIGN_CENTER, LV_PART_MAIN);
  lv_label_set_text(cap, caption);
  lv_obj_align(cap, LV_ALIGN_CENTER, 0, 30);
}

} // namespace

void SetupApp::onEnter(lv_obj_t *root) {
  const ThemeTokens::Palette &p = Theme::active();
  m_generation = 0xFFFFFFFF;
  m_badgeStage = Stage::Done;
  m_badgePin[0] = '\0';
  m_skipped = false;

  m_kickerLbl = makeLabel(root, Theme::fontCaption(), p.accent);
  lv_label_set_text(m_kickerLbl, "HAWAOS SETUP");
  lv_obj_set_pos(m_kickerLbl, kLeftX, 18);

  m_titleLbl = makeLabel(root, Theme::fontTitle(), p.text);
  lv_label_set_long_mode(m_titleLbl, LV_LABEL_LONG_DOT);
  lv_obj_set_width(m_titleLbl, kLeftW);
  lv_obj_set_pos(m_titleLbl, kLeftX, 36);

  m_bodyLbl = makeLabel(root, Theme::fontBody(), p.textDim);
  lv_label_set_long_mode(m_bodyLbl, LV_LABEL_LONG_WRAP);
  lv_obj_set_width(m_bodyLbl, kLeftW);
  lv_obj_set_style_text_line_space(m_bodyLbl, 2, LV_PART_MAIN);
  lv_obj_set_pos(m_bodyLbl, kLeftX, 64);

  m_hintLbl = makeLabel(root, Theme::fontCaption(), p.textDim);
  lv_label_set_text(m_hintLbl, "Hold Select to skip");
  lv_obj_set_pos(m_hintLbl, kLeftX, 140);

  m_card = Theme::createCard(root, kCardW, kCardH, kCardX, kCardY,
                             Config::RADIUS_LG);
  lv_obj_set_style_pad_all(m_card, 0, LV_PART_MAIN);

  refresh(SetupService::snapshot());
}

void SetupApp::onExit() {
  m_kickerLbl = nullptr;
  m_titleLbl = nullptr;
  m_bodyLbl = nullptr;
  m_hintLbl = nullptr;
  m_card = nullptr;
}

void SetupApp::onTick(uint32_t /*nowMs*/) {
  const SetupService::Snapshot s = SetupService::snapshot();
  if (s.generation != m_generation) {
    refresh(s);
  }
}

bool SetupApp::onButton(Button b, Press p) {
  if (b == Button::Select && p == Press::Long) {
    m_skipped = true;
    SetupService::skip();
  }
  return true; // nothing else works until setup is done or skipped
}

void SetupApp::refresh(const SetupService::Snapshot &s) {
  if (m_titleLbl == nullptr)
    return;
  m_generation = s.generation;
  const ThemeTokens::Palette &p = Theme::active();

  char title[48];
  char body[128];
  uint32_t bodyColor = p.textDim;

  switch (s.stage) {
  case Stage::Waiting:
    snprintf(title, sizeof(title), "Welcome");
    snprintf(body, sizeof(body),
             "Open the HawaOS Companion and choose Add device.");
    break;
  case Stage::Linked:
    snprintf(title, sizeof(title), "Connected");
    if (s.link == Link::Ble && !s.paired) {
      snprintf(body, sizeof(body), "Enter the code on the right in the "
                                   "companion.");
    } else {
      snprintf(body, sizeof(body),
               "Pick a WiFi network in the companion on %s.",
               s.host[0] != '\0' ? s.host : "your computer");
    }
    break;
  case Stage::Joining:
    snprintf(title, sizeof(title), "Joining WiFi");
    snprintf(body, sizeof(body), "Connecting to %s...", s.ssid);
    break;
  case Stage::JoinFailed:
    snprintf(title, sizeof(title), "Couldn't connect");
    if (strcmp(s.reason, "auth") == 0) {
      snprintf(body, sizeof(body), "Wrong password for %s. Try again in the "
                                   "companion.",
               s.ssid);
    } else if (strcmp(s.reason, "no_ap") == 0) {
      snprintf(body, sizeof(body), "%s is out of range. Move closer or pick "
                                   "another network.",
               s.ssid);
    } else {
      snprintf(body, sizeof(body), "%s didn't answer. Try again in the "
                                   "companion.",
               s.ssid);
    }
    bodyColor = p.err;
    break;
  case Stage::Online:
    snprintf(title, sizeof(title), "You're online");
    snprintf(body, sizeof(body), "%s\n%s\nFinish up in the companion.", s.ssid,
             s.ip);
    break;
  case Stage::Done:
  default:
    snprintf(title, sizeof(title), "All set!");
    snprintf(body, sizeof(body), m_skipped
                                     ? "You can set up later from Settings."
                                     : "Enjoy HawaOS.");
    break;
  }

  setTextIfChanged(m_titleLbl, title);
  setTextIfChanged(m_bodyLbl, body);
  lv_obj_set_style_text_color(m_bodyLbl, lv_color_hex(bodyColor), LV_PART_MAIN);
  if (s.stage == Stage::Done) {
    lv_obj_add_flag(m_hintLbl, LV_OBJ_FLAG_HIDDEN);
  } else {
    lv_obj_clear_flag(m_hintLbl, LV_OBJ_FLAG_HIDDEN);
  }

  buildBadge(s);
}

void SetupApp::buildBadge(const SetupService::Snapshot &s) {
  if (s.stage == m_badgeStage && s.link == m_badgeLink &&
      s.paired == m_badgePaired && strcmp(s.pin, m_badgePin) == 0) {
    return;
  }
  m_badgeStage = s.stage;
  m_badgeLink = s.link;
  m_badgePaired = s.paired;
  memcpy(m_badgePin, s.pin, sizeof(m_badgePin));

  lv_obj_clean(m_card);
  const ThemeTokens::Palette &p = Theme::active();

  const bool showCode =
      (s.stage == Stage::Waiting) ||
      (s.stage == Stage::Linked && s.link == Link::Ble && !s.paired);

  if (showCode) {
    // Bluetooth name on top, the 6-digit code as two rows of three below,
    // USB as the alternative at the bottom.
    lv_obj_t *bt = makeLabel(m_card, Theme::iconFont20(), p.accent);
    lv_label_set_text(bt, ICON_BLUETOOTH);
    lv_obj_align(bt, LV_ALIGN_TOP_MID, 0, 10);

    lv_obj_t *name = makeLabel(m_card, Theme::fontCaption(), p.textDim);
    lv_label_set_text(name, s.bleName);
    lv_obj_align(name, LV_ALIGN_TOP_MID, 0, 31);

    char row[4] = {0};
    for (int i = 0; i < 2; ++i) {
      memcpy(row, s.pin + i * 3, 3);
      lv_obj_t *digits = makeLabel(m_card, Theme::fontDisplay(), p.text);
      lv_obj_set_style_text_letter_space(digits, 3, LV_PART_MAIN);
      lv_label_set_text(digits, row);
      lv_obj_align(digits, LV_ALIGN_TOP_MID, 0, 50 + i * 30);
    }

    lv_obj_t *usb = makeLabel(m_card, Theme::fontCaption(), p.textDim);
    lv_label_set_text(usb, "or plug in USB-C");
    lv_obj_align(usb, LV_ALIGN_BOTTOM_MID, 0, -8);
    return;
  }

  switch (s.stage) {
  case Stage::Linked:
    if (s.link == Link::Ble) {
      iconBadge(m_card, ICON_BLUETOOTH, p.accent, "Paired");
    } else {
      iconBadge(m_card, s.link == Link::Usb ? ICON_USB : ICON_WIFI, p.accent,
                s.link == Link::Usb ? "USB-C" : "WiFi");
    }
    break;
  case Stage::Joining: {
    lv_obj_t *spin = lv_spinner_create(m_card, 1000, 60);
    lv_obj_set_size(spin, 52, 52);
    lv_obj_align(spin, LV_ALIGN_CENTER, 0, -14);
    lv_obj_set_style_arc_width(spin, 5, LV_PART_MAIN);
    lv_obj_set_style_arc_color(spin, lv_color_hex(p.surface2), LV_PART_MAIN);
    lv_obj_set_style_arc_width(spin, 5, LV_PART_INDICATOR);
    lv_obj_set_style_arc_color(spin, lv_color_hex(p.accent), LV_PART_INDICATOR);
    lv_obj_t *cap = makeLabel(m_card, Theme::fontCaption(), p.textDim);
    lv_label_set_long_mode(cap, LV_LABEL_LONG_DOT);
    lv_obj_set_width(cap, kCardW - 16);
    lv_obj_set_style_text_align(cap, LV_TEXT_ALIGN_CENTER, LV_PART_MAIN);
    lv_label_set_text(cap, s.ssid);
    lv_obj_align(cap, LV_ALIGN_CENTER, 0, 30);
    break;
  }
  case Stage::JoinFailed:
    iconBadge(m_card, ICON_CIRCLE_ALERT, p.err, s.ssid);
    break;
  case Stage::Online:
  case Stage::Done:
  default:
    iconBadge(m_card, ICON_CHECK, p.ok, s.stage == Stage::Done ? "Ready" : s.ip);
    break;
  }
}
