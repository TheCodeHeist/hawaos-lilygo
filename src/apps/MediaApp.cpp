#include "apps/MediaApp.h"
#include "config.h"
#include "core/AppManager.h"
#include "core/Settings.h"
#include "services/CompanionLink.h"
#include "ui/Theme.h"
#include "ui/icons.h"

#include <Arduino.h>
#include <stdio.h>
#include <string.h>

namespace {

constexpr uint32_t kTextWhite = 0xFFFFFF;
constexpr uint32_t kTextSilver = 0xD0DCE0;
constexpr uint32_t kTextDimSilver = 0xA0B4BA;

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
  m_lastBgTop = 0xFFFFFFFF;
  m_lastBgBot = 0xFFFFFFFF;
  m_lastLightBg = false;
  const ThemeTokens::Palette &p = Theme::dark();

  // Root screen uses full-screen vertical gradient tinted with companion bg_top
  // -> bg_bottom
  lv_obj_set_style_bg_opa(root, LV_OPA_COVER, LV_PART_MAIN);
  lv_obj_set_style_bg_grad_dir(root, LV_GRAD_DIR_VER, LV_PART_MAIN);
  lv_obj_set_style_bg_dither_mode(root, Theme::kGradientDither, LV_PART_MAIN);

  // Album Art Card: 120x120 at x=12, y=25 (centered vertically on 170px height)
  // Subtle 1px translucent border with rounded corners (radius 10)
  m_artCard = lv_obj_create(root);
  lv_obj_set_pos(m_artCard, 12, 25);
  lv_obj_set_size(m_artCard, 120, 120);
  lv_obj_set_style_bg_color(m_artCard, lv_color_hex(p.surface), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(m_artCard, LV_OPA_COVER, LV_PART_MAIN);
  lv_obj_set_style_radius(m_artCard, 10, LV_PART_MAIN);
  lv_obj_set_style_border_color(m_artCard, lv_color_hex(0xFFFFFF),
                                LV_PART_MAIN);
  lv_obj_set_style_border_opa(m_artCard, LV_OPA_20, LV_PART_MAIN);
  lv_obj_set_style_border_width(m_artCard, 1, LV_PART_MAIN);
  lv_obj_set_style_pad_all(m_artCard, 0, LV_PART_MAIN);
  lv_obj_set_style_clip_corner(m_artCard, true, LV_PART_MAIN);
  lv_obj_clear_flag(m_artCard, LV_OBJ_FLAG_SCROLLABLE);

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
  lv_img_set_zoom(m_artImg, 240); // 128 * (240/256) = 120px exact fit
  lv_obj_center(m_artImg);
  lv_obj_add_flag(m_artImg, LV_OBJ_FLAG_HIDDEN);

  // Right column starts at x=144 (width 164 px)
  // Small discrete "NOW PLAYING" header
  m_kickerLbl = lv_label_create(root);
  lv_label_set_text(m_kickerLbl, "NOW PLAYING");
  lv_obj_set_style_text_font(m_kickerLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_kickerLbl, lv_color_hex(kTextDimSilver),
                              LV_PART_MAIN);
  lv_obj_set_pos(m_kickerLbl, 144, 14);

  // Track Title: Bold white 18 SemiBold, circular marquee scroll
  m_titleLbl = lv_label_create(root);
  lv_obj_set_style_text_font(m_titleLbl, Theme::fontTitle(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_titleLbl, lv_color_hex(kTextWhite),
                              LV_PART_MAIN);
  lv_label_set_long_mode(m_titleLbl, LV_LABEL_LONG_SCROLL_CIRCULAR);
  lv_obj_set_style_anim_speed(m_titleLbl, 28, LV_PART_MAIN);
  lv_obj_set_width(m_titleLbl, 164);
  lv_obj_set_pos(m_titleLbl, 144, 30);

  // Artist Name: Soft silver-tinted white 14 Regular, circular marquee scroll
  m_artistLbl = lv_label_create(root);
  lv_obj_set_style_text_font(m_artistLbl, Theme::fontBody(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_artistLbl, lv_color_hex(kTextSilver),
                              LV_PART_MAIN);
  lv_label_set_long_mode(m_artistLbl, LV_LABEL_LONG_SCROLL_CIRCULAR);
  lv_obj_set_style_anim_speed(m_artistLbl, 24, LV_PART_MAIN);
  lv_obj_set_width(m_artistLbl, 164);
  lv_obj_set_pos(m_artistLbl, 144, 56);

  // Sleek progress bar: 3 px thin, translucent track with dynamic tint
  // indicator
  m_progBar = lv_bar_create(root);
  lv_obj_set_size(m_progBar, 164, 3);
  lv_obj_set_pos(m_progBar, 144, 88);
  lv_bar_set_range(m_progBar, 0, 1000);
  lv_obj_set_style_radius(m_progBar, LV_RADIUS_CIRCLE, LV_PART_MAIN);
  lv_obj_set_style_radius(m_progBar, LV_RADIUS_CIRCLE, LV_PART_INDICATOR);
  lv_obj_set_style_bg_color(m_progBar, lv_color_hex(0xFFFFFF), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(m_progBar, LV_OPA_30, LV_PART_MAIN);
  lv_obj_set_style_bg_color(m_progBar, lv_color_hex(p.primary),
                            LV_PART_INDICATOR);
  lv_obj_set_style_bg_opa(m_progBar, LV_OPA_COVER, LV_PART_INDICATOR);

  m_posLbl = lv_label_create(root);
  lv_obj_set_style_text_font(m_posLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_posLbl, lv_color_hex(kTextDimSilver),
                              LV_PART_MAIN);
  lv_obj_set_pos(m_posLbl, 144, 95);

  m_durLbl = lv_label_create(root);
  lv_obj_set_style_text_font(m_durLbl, Theme::fontCaption(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_durLbl, lv_color_hex(kTextDimSilver),
                              LV_PART_MAIN);
  lv_obj_align(m_durLbl, LV_ALIGN_TOP_RIGHT, -12, 95);

  // Transport controls: [skip-back] [ (play/pause circle) ] [skip-forward]
  m_prevIcon = lv_label_create(root);
  lv_label_set_text(m_prevIcon, ICON_SKIP_BACK);
  lv_obj_set_style_text_font(m_prevIcon, Theme::iconFont24(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_prevIcon, lv_color_hex(kTextWhite),
                              LV_PART_MAIN);
  lv_obj_set_pos(m_prevIcon, 156, 120);

  // Play button: 36x36 circular button with adaptive icon
  m_playBtnCircle = lv_obj_create(root);
  lv_obj_set_size(m_playBtnCircle, 36, 36);
  lv_obj_set_pos(m_playBtnCircle, 208, 114);
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

  m_nextIcon = lv_label_create(root);
  lv_label_set_text(m_nextIcon, ICON_SKIP_FORWARD);
  lv_obj_set_style_text_font(m_nextIcon, Theme::iconFont24(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_nextIcon, lv_color_hex(kTextWhite),
                              LV_PART_MAIN);
  lv_obj_set_pos(m_nextIcon, 264, 120);

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
  m_kickerLbl = nullptr;
  m_titleLbl = nullptr;
  m_artistLbl = nullptr;
  m_progBar = nullptr;
  m_posLbl = nullptr;
  m_durLbl = nullptr;
  m_prevIcon = nullptr;
  m_playBtnCircle = nullptr;
  m_playIconLbl = nullptr;
  m_nextIcon = nullptr;
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

  const bool hasTrack = (d.mediaTitle[0] != '\0');

  // Background gradient: Uses bg_top and bg_bottom parsed from CompanionLink
  const uint32_t bgTop = Theme::resolveMediaTint(d.mediaBgTop);
  const uint32_t bgBot = Theme::resolveMediaTintBottom(d.mediaBgBottom);

  const ThemeTokens::Palette &p = Theme::dark();
  const bool lightBg =
      Theme::isColorLight(bgTop) || (!d.mediaInkLight && hasTrack);

  // Dynamic high-contrast typography tokens based on background tint:
  const uint32_t titleColor = lightBg ? 0x0A1014 : 0xFFFFFF;
  const uint32_t artistColor = lightBg ? 0x243238 : 0xD0DCE0;
  const uint32_t kickerColor = lightBg ? 0x354850 : 0xA0B4BA;
  const uint32_t subColor = lightBg ? 0x354850 : 0xA0B4BA;
  const uint32_t iconColor = lightBg ? 0x0A1014 : 0xFFFFFF;

  // Dynamic progress bar indicator derived directly from the dominant album art
  // tint:
  const uint32_t progIndicatorColor =
      hasTrack ? Theme::tintAccent(bgTop, p.primary) : p.primary;
  const uint32_t progTrackColor = lightBg ? 0x000000 : 0xFFFFFF;
  const lv_opa_t progTrackOpa = lightBg ? LV_OPA_20 : LV_OPA_30;

  if (bgTop != m_lastBgTop || bgBot != m_lastBgBot ||
      lightBg != m_lastLightBg) {
    m_lastBgTop = bgTop;
    m_lastBgBot = bgBot;
    m_lastLightBg = lightBg;

    lv_obj_set_style_bg_color(m_root, lv_color_hex(bgTop), LV_PART_MAIN);
    lv_obj_set_style_bg_grad_color(m_root, lv_color_hex(bgBot), LV_PART_MAIN);
    lv_obj_set_style_bg_grad_dir(
        m_root, (bgTop != bgBot) ? LV_GRAD_DIR_VER : LV_GRAD_DIR_NONE,
        LV_PART_MAIN);

    // Apply high-contrast text and indicator colors
    lv_obj_set_style_text_color(m_titleLbl, lv_color_hex(titleColor),
                                LV_PART_MAIN);
    lv_obj_set_style_text_color(m_artistLbl, lv_color_hex(artistColor),
                                LV_PART_MAIN);
    lv_obj_set_style_text_color(m_kickerLbl, lv_color_hex(kickerColor),
                                LV_PART_MAIN);
    lv_obj_set_style_text_color(m_posLbl, lv_color_hex(subColor), LV_PART_MAIN);
    lv_obj_set_style_text_color(m_durLbl, lv_color_hex(subColor), LV_PART_MAIN);
    lv_obj_set_style_text_color(m_prevIcon, lv_color_hex(iconColor),
                                LV_PART_MAIN);
    lv_obj_set_style_text_color(m_nextIcon, lv_color_hex(iconColor),
                                LV_PART_MAIN);

    // Progress bar styles: indicator uses album art tint; track adapts contrast
    lv_obj_set_style_bg_color(m_progBar, lv_color_hex(progIndicatorColor),
                              LV_PART_INDICATOR);
    lv_obj_set_style_bg_color(m_progBar, lv_color_hex(progTrackColor),
                              LV_PART_MAIN);
    lv_obj_set_style_bg_opa(m_progBar, progTrackOpa, LV_PART_MAIN);

    // Play button circle styles: matching vibrant tint accent with adaptive
    // icon ink
    lv_obj_set_style_bg_color(m_playBtnCircle, lv_color_hex(progIndicatorColor),
                              LV_PART_MAIN);
    const uint32_t playIconColor =
        Theme::isColorLight(progIndicatorColor) ? 0x051920 : 0xFFFFFF;
    lv_obj_set_style_text_color(m_playIconLbl, lv_color_hex(playIconColor),
                                LV_PART_MAIN);

    // Art card border contrast: subtle 1px border with high contrast
    lv_obj_set_style_border_color(
        m_artCard, lv_color_hex(lightBg ? 0x000000 : 0xFFFFFF), LV_PART_MAIN);

    lv_obj_invalidate(m_root);
    Serial.printf(
        "[MediaApp] Applied tint: top=0x%06lX, bot=0x%06lX, prog=0x%06lX, "
        "lightBg=%d\n",
        static_cast<unsigned long>(bgTop), static_cast<unsigned long>(bgBot),
        static_cast<unsigned long>(progIndicatorColor), lightBg ? 1 : 0);
  }

  if (hasTrack) {
    setLabelTextIfChanged(m_titleLbl, d.mediaTitle);
    setLabelTextIfChanged(m_artistLbl,
                          d.mediaArtist[0] ? d.mediaArtist : "Unknown Artist");
    setLabelTextIfChanged(m_kickerLbl, (d.mediaState == MediaPlayState::Playing)
                                           ? "NOW PLAYING"
                                           : "PAUSED");
  } else {
    setLabelTextIfChanged(m_titleLbl, "Nothing playing");
    setLabelTextIfChanged(m_artistLbl, "Play music on your PC");
    setLabelTextIfChanged(m_kickerLbl, "MEDIA PLAYER");
  }

  // Show pause icon when playing, play icon when paused/stopped
  const char *targetIcon =
      (d.mediaState == MediaPlayState::Playing) ? ICON_PAUSE : ICON_PLAY;
  setLabelTextIfChanged(m_playIconLbl, targetIcon);

  // Show 120x120 RGB565 album art from PSRAM if ready
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
