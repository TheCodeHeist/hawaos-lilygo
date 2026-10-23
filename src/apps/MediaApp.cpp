#include "apps/MediaApp.h"
#include "config.h"
#include "core/AppManager.h"
#include "services/CompanionLink.h"
#include "ui/Theme.h"
#include "ui/icons.h"

#include <Arduino.h>
#include <stdio.h>
#include <string.h>

namespace {

void formatDurationMs(uint32_t ms, char *out, size_t maxLen) {
  const uint32_t totalSec = ms / 1000;
  const uint32_t mins = totalSec / 60;
  const uint32_t secs = totalSec % 60;
  snprintf(out, maxLen, "%lu:%02lu", static_cast<unsigned long>(mins),
           static_cast<unsigned long>(secs));
}

void setLabelTextIfChanged(lv_obj_t *lbl, const char *newText) {
  if (lbl == nullptr || newText == nullptr)
    return;
  const char *cur = lv_label_get_text(lbl);
  if (cur == nullptr || strcmp(cur, newText) != 0) {
    lv_label_set_text(lbl, newText);
  }
}

} // namespace

void MediaApp::onEnter(lv_obj_t *root) {
  m_root = root;
  const ThemeTokens::Palette &p = Theme::dark();

  // Album Art container (128x128 at x=10, y=21, radius 10)
  m_artCard = Theme::createCard(root, 128, 128, 10, 21, 10, true);
  lv_obj_set_style_pad_all(m_artCard, 0, LV_PART_MAIN);
  lv_obj_set_style_bg_color(m_artCard, lv_color_hex(p.surface2), LV_PART_MAIN);
  lv_obj_set_style_clip_corner(m_artCard, true, LV_PART_MAIN);

  m_artPlaceholderIcon = lv_label_create(m_artCard);
  lv_label_set_text(m_artPlaceholderIcon, ICON_MUSIC);
  lv_obj_set_style_text_font(m_artPlaceholderIcon, Theme::iconFont48(),
                             LV_PART_MAIN);
  lv_obj_set_style_text_color(m_artPlaceholderIcon, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_center(m_artPlaceholderIcon);

  m_artDsc.header.always_zero = 0;
  m_artDsc.header.w = DataStore::ALBUM_ART_DIM;
  m_artDsc.header.h = DataStore::ALBUM_ART_DIM;
  m_artDsc.header.cf = LV_IMG_CF_TRUE_COLOR;
  m_artDsc.data_size = DataStore::ALBUM_ART_BYTES;
  m_artDsc.data =
      reinterpret_cast<const uint8_t *>(DataStore::albumArtBuffer());

  m_artImg = lv_img_create(m_artCard);
  lv_img_set_src(m_artImg, &m_artDsc);
  lv_obj_center(m_artImg);
  lv_obj_add_flag(m_artImg, LV_OBJ_FLAG_HIDDEN);

  // Right column starts at x=150 (width 158)
  // Both Title and Artist use single-line circular scroll so long names never
  // wrap and overwrite each other
  m_titleLbl = lv_label_create(root);
  lv_obj_set_style_text_font(m_titleLbl, Theme::fontTitle(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_titleLbl, lv_color_hex(p.text), LV_PART_MAIN);
  lv_label_set_long_mode(m_titleLbl, LV_LABEL_LONG_SCROLL_CIRCULAR);
  lv_obj_set_style_anim_speed(m_titleLbl, 28, LV_PART_MAIN);
  lv_obj_set_width(m_titleLbl, 158);
  lv_obj_set_pos(m_titleLbl, 150, 20);

  m_artistLbl = lv_label_create(root);
  lv_obj_set_style_text_font(m_artistLbl, Theme::fontBody(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_artistLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_label_set_long_mode(m_artistLbl, LV_LABEL_LONG_SCROLL_CIRCULAR);
  lv_obj_set_style_anim_speed(m_artistLbl, 24, LV_PART_MAIN);
  lv_obj_set_width(m_artistLbl, 158);
  lv_obj_set_pos(m_artistLbl, 150, 48);

  // Progress bar (4 px, primary on surface2)
  m_progBar = lv_bar_create(root);
  lv_obj_set_size(m_progBar, 158, 4);
  lv_obj_set_pos(m_progBar, 150, 82);
  lv_bar_set_range(m_progBar, 0, 1000);
  lv_obj_set_style_radius(m_progBar, LV_RADIUS_CIRCLE, LV_PART_MAIN);
  lv_obj_set_style_radius(m_progBar, LV_RADIUS_CIRCLE, LV_PART_INDICATOR);
  lv_obj_set_style_bg_color(m_progBar, lv_color_hex(0xFFFFFF), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(m_progBar, LV_OPA_30, LV_PART_MAIN);
  lv_obj_set_style_bg_color(m_progBar, lv_color_hex(p.primary),
                            LV_PART_INDICATOR);

  m_posLbl = lv_label_create(root);
  lv_obj_set_style_text_font(m_posLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_posLbl, lv_color_hex(p.textDim), LV_PART_MAIN);
  lv_obj_set_pos(m_posLbl, 150, 90);

  m_durLbl = lv_label_create(root);
  lv_obj_set_style_text_font(m_durLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_durLbl, lv_color_hex(p.textDim), LV_PART_MAIN);
  lv_obj_align(m_durLbl, LV_ALIGN_TOP_RIGHT, -12, 90);

  // Transport controls: [skip-back] [ (play/pause circle) ] [skip-forward]
  lv_obj_t *prevIcon = lv_label_create(root);
  lv_label_set_text(prevIcon, ICON_SKIP_BACK);
  lv_obj_set_style_text_font(prevIcon, Theme::iconFont24(), LV_PART_MAIN);
  lv_obj_set_style_text_color(prevIcon, lv_color_hex(p.text), LV_PART_MAIN);
  lv_obj_set_pos(prevIcon, 166, 118);

  m_playBtnCircle = lv_obj_create(root);
  lv_obj_set_size(m_playBtnCircle, 38, 38);
  lv_obj_set_pos(m_playBtnCircle, 210, 111);
  lv_obj_set_style_radius(m_playBtnCircle, LV_RADIUS_CIRCLE, LV_PART_MAIN);
  lv_obj_set_style_bg_color(m_playBtnCircle, lv_color_hex(p.primary),
                            LV_PART_MAIN);
  lv_obj_set_style_bg_opa(m_playBtnCircle, LV_OPA_COVER, LV_PART_MAIN);
  lv_obj_set_style_border_width(m_playBtnCircle, 0, LV_PART_MAIN);
  lv_obj_clear_flag(m_playBtnCircle, LV_OBJ_FLAG_SCROLLABLE);

  m_playIconLbl = lv_label_create(m_playBtnCircle);
  lv_label_set_text(m_playIconLbl, ICON_PLAY);
  lv_obj_set_style_text_font(m_playIconLbl, Theme::iconFont24(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_playIconLbl, lv_color_hex(p.onPrimary),
                              LV_PART_MAIN);
  lv_obj_center(m_playIconLbl);

  lv_obj_t *nextIcon = lv_label_create(root);
  lv_label_set_text(nextIcon, ICON_SKIP_FORWARD);
  lv_obj_set_style_text_font(nextIcon, Theme::iconFont24(), LV_PART_MAIN);
  lv_obj_set_style_text_color(nextIcon, lv_color_hex(p.text), LV_PART_MAIN);
  lv_obj_set_pos(nextIcon, 268, 118);

  m_lastProgressSec = 0xFFFFFFFF;
  const DataSnapshot snap = DataStore::getSnapshot();
  refreshMedia(snap);
  refreshProgress(snap);
}

void MediaApp::onExit() {
  m_root = nullptr;
  m_artCard = nullptr;
  m_artPlaceholderIcon = nullptr;
  m_artImg = nullptr;
  m_titleLbl = nullptr;
  m_artistLbl = nullptr;
  m_progBar = nullptr;
  m_posLbl = nullptr;
  m_durLbl = nullptr;
  m_playBtnCircle = nullptr;
  m_playIconLbl = nullptr;
}

void MediaApp::onTick(uint32_t nowMs) {
  const uint32_t sec = nowMs / 1000;
  if (sec != m_lastProgressSec) {
    m_lastProgressSec = sec;
    refreshProgress(DataStore::getSnapshot());
  }
}

bool MediaApp::onButton(Button b, Press p) {
  // Media player controls:
  // - Select short = play/pause
  // - Select long  = next track (prevents accidental skip on Next single press)
  // - Next double  = previous track
  // - Next short / Next long = unconsumed (cycles app / opens Launcher)
  if (b == Button::Select && p == Press::Short) {
    CompanionLink::sendMediaCommand("play_pause");
    return true;
  }
  if (b == Button::Select && p == Press::Long) {
    CompanionLink::sendMediaCommand("next");
    return true;
  }
  if (b == Button::Next && p == Press::Double) {
    CompanionLink::sendMediaCommand("prev");
    return true;
  }
  return false;
}

void MediaApp::onData(const DataSnapshot &d) {
  refreshMedia(d);
  refreshProgress(d);
}

void MediaApp::refreshMedia(const DataSnapshot &d) {
  if (m_root == nullptr)
    return;
  const ThemeTokens::Palette &p = Theme::dark();

  // Apply companion tinted gradient background (or fallback to dark bg)
  const bool hasTrack =
      (d.mediaState != MediaPlayState::Stopped && d.mediaTitle[0] != '\0');
  const uint32_t bgTop = hasTrack ? d.mediaBgTop : p.bg;
  const uint32_t bgBot = hasTrack ? d.mediaBgBottom : p.bg;

  lv_obj_set_style_bg_color(m_root, lv_color_hex(bgTop), LV_PART_MAIN);
  lv_obj_set_style_bg_grad_color(m_root, lv_color_hex(bgBot), LV_PART_MAIN);
  lv_obj_set_style_bg_grad_dir(m_root, LV_GRAD_DIR_VER, LV_PART_MAIN);

  if (hasTrack) {
    setLabelTextIfChanged(m_titleLbl, d.mediaTitle);
    setLabelTextIfChanged(m_artistLbl,
                          d.mediaArtist[0] ? d.mediaArtist : "Unknown Artist");
  } else {
    setLabelTextIfChanged(m_titleLbl, "Nothing playing");
    setLabelTextIfChanged(m_artistLbl, "Play music on your PC");
  }

  // Show pause icon when playing, play icon when paused/stopped
  const char *targetIcon =
      (d.mediaState == MediaPlayState::Playing) ? ICON_PAUSE : ICON_PLAY;
  setLabelTextIfChanged(m_playIconLbl, targetIcon);

  // Show 128x128 RGB565 album art from PSRAM if ready
  if (hasTrack && d.mediaArtReady && DataStore::albumArtBuffer() != nullptr) {
    lv_obj_clear_flag(m_artImg, LV_OBJ_FLAG_HIDDEN);
    lv_obj_add_flag(m_artPlaceholderIcon, LV_OBJ_FLAG_HIDDEN);
    lv_obj_invalidate(m_artImg);
  } else {
    lv_obj_add_flag(m_artImg, LV_OBJ_FLAG_HIDDEN);
    lv_obj_clear_flag(m_artPlaceholderIcon, LV_OBJ_FLAG_HIDDEN);
  }
}

void MediaApp::refreshProgress(const DataSnapshot &d) {
  if (m_progBar == nullptr)
    return;

  if (d.mediaState == MediaPlayState::Stopped || d.mediaDurMs == 0) {
    lv_bar_set_value(m_progBar, 0, LV_ANIM_OFF);
    setLabelTextIfChanged(m_posLbl, "0:00");
    setLabelTextIfChanged(m_durLbl, "0:00");
    return;
  }

  uint32_t curPosMs = d.mediaPosMs;
  if (d.mediaState == MediaPlayState::Playing && d.mediaRxMillis > 0) {
    const uint32_t elapsed = millis() - d.mediaRxMillis;
    curPosMs = min(d.mediaDurMs, d.mediaPosMs + elapsed);
  }

  const int32_t permille =
      static_cast<int32_t>((static_cast<uint64_t>(curPosMs) * 1000ULL) /
                           static_cast<uint64_t>(d.mediaDurMs));
  lv_bar_set_value(m_progBar, permille, LV_ANIM_OFF);

  char posStr[16];
  char durStr[16];
  formatDurationMs(curPosMs, posStr, sizeof(posStr));
  formatDurationMs(d.mediaDurMs, durStr, sizeof(durStr));
  setLabelTextIfChanged(m_posLbl, posStr);
  setLabelTextIfChanged(m_durLbl, durStr);
}
