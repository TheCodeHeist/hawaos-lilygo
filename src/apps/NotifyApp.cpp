#include "apps/NotifyApp.h"
#include "config.h"
#include "ui/StatusBar.h"
#include "ui/Theme.h"
#include "ui/icons.h"

#include <Arduino.h>
#include <stdio.h>
#include <time.h>

namespace {

void formatRelativeTime(uint32_t itemTs, char *out, size_t maxLen) {
  const time_t now = time(nullptr);
  if (itemTs == 0) {
    snprintf(out, maxLen, "now");
    return;
  }
  uint32_t diffSec = 0;
  if (now > 1700000000 && itemTs > 1700000000) {
    diffSec = (static_cast<uint32_t>(now) > itemTs)
                  ? (static_cast<uint32_t>(now) - itemTs)
                  : 0;
  } else {
    const uint32_t upSec = millis() / 1000;
    diffSec = (upSec > itemTs) ? (upSec - itemTs) : 0;
  }

  if (diffSec < 60) {
    snprintf(out, maxLen, "now");
  } else if (diffSec < 3600) {
    snprintf(out, maxLen, "%lum", static_cast<unsigned long>(diffSec / 60));
  } else if (diffSec < 86400) {
    snprintf(out, maxLen, "%luh", static_cast<unsigned long>(diffSec / 3600));
  } else {
    snprintf(out, maxLen, "%lud", static_cast<unsigned long>(diffSec / 86400));
  }
}

} // namespace

void NotifyApp::onEnter(lv_obj_t *root) {
  const ThemeTokens::Palette &p = Theme::active();

  DataStore::markAllNotificationsRead();
  StatusBar::updateData(DataStore::getSnapshot());

  // Empty state: bell-off icon + "All clear" (DESIGN.md Section 9.5)
  m_emptyIcon = lv_label_create(root);
  lv_label_set_text(m_emptyIcon, ICON_BELL_OFF);
  lv_obj_set_style_text_font(m_emptyIcon, Theme::iconFont32(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_emptyIcon, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_align(m_emptyIcon, LV_ALIGN_CENTER, 0, -6);

  m_emptyLbl = lv_label_create(root);
  lv_label_set_text(m_emptyLbl, "All clear");
  lv_obj_set_style_text_font(m_emptyLbl, Theme::fontBodySb(), LV_PART_MAIN);
  lv_obj_set_style_text_color(m_emptyLbl, lv_color_hex(p.textDim),
                              LV_PART_MAIN);
  lv_obj_align(m_emptyLbl, LV_ALIGN_CENTER, 0, 26);

  // Two visible notification cards (height 62 px, 8 px gap) so 2-line body fits
  // cleanly
  for (int i = 0; i < kVisibleRows; ++i) {
    const int16_t y =
        Config::CONTENT_Y_START + 8 + static_cast<int16_t>(i * 70);
    lv_obj_t *card = Theme::createCard(root, 300, 62, Config::OUTER_MARGIN_PX,
                                       y, Config::RADIUS_MD);
    lv_obj_set_style_pad_all(card, 8, LV_PART_MAIN);

    lv_obj_t *icon = lv_label_create(card);
    lv_label_set_text(icon, ICON_BELL);
    lv_obj_set_style_text_font(icon, Theme::iconFont24(), LV_PART_MAIN);
    lv_obj_set_style_text_color(icon, lv_color_hex(p.accent), LV_PART_MAIN);
    lv_obj_align(icon, LV_ALIGN_LEFT_MID, 2, 0);

    lv_obj_t *title = lv_label_create(card);
    lv_obj_set_style_text_font(title, Theme::fontBodySb(), LV_PART_MAIN);
    lv_obj_set_style_text_color(title, lv_color_hex(p.text), LV_PART_MAIN);
    lv_label_set_long_mode(title, LV_LABEL_LONG_DOT);
    lv_obj_set_width(title, 198);
    lv_obj_align(title, LV_ALIGN_TOP_LEFT, 34, 0);

    lv_obj_t *timeLbl = lv_label_create(card);
    lv_obj_set_style_text_font(timeLbl, Theme::fontCaption(), LV_PART_MAIN);
    lv_obj_set_style_text_color(timeLbl, lv_color_hex(p.textDim), LV_PART_MAIN);
    lv_obj_align(timeLbl, LV_ALIGN_TOP_RIGHT, -2, 2);

    lv_obj_t *body = lv_label_create(card);
    lv_obj_set_style_text_font(body, Theme::fontCaption(), LV_PART_MAIN);
    lv_obj_set_style_text_color(body, lv_color_hex(p.textDim), LV_PART_MAIN);
    lv_label_set_long_mode(body, LV_LABEL_LONG_DOT);
    lv_label_set_recolor(body, true); // emphasis in the accent (11.10)
    // Two caption lines (17 px each), drawn 3 px closer so they fit the card.
    lv_obj_set_style_text_line_space(body, -3, LV_PART_MAIN);
    lv_obj_set_size(body, 244, 32);
    lv_obj_align(body, LV_ALIGN_TOP_LEFT, 34, 20);

    m_cards[i] = card;
    m_icons[i] = icon;
    m_titles[i] = title;
    m_times[i] = timeLbl;
    m_bodies[i] = body;
  }

  m_cursor = 0;
  m_scrollTop = 0;
  refreshList(DataStore::getSnapshot());
}

void NotifyApp::onExit() {
  m_emptyIcon = nullptr;
  m_emptyLbl = nullptr;
  for (int i = 0; i < kVisibleRows; ++i) {
    m_cards[i] = nullptr;
    m_icons[i] = nullptr;
    m_titles[i] = nullptr;
    m_bodies[i] = nullptr;
    m_times[i] = nullptr;
  }
}

void NotifyApp::onTick(uint32_t /*nowMs*/) {}

bool NotifyApp::onButton(Button b, Press p) {
  const DataSnapshot d = DataStore::getSnapshot();
  if (d.notifyCount == 0) {
    return false;
  }

  // Next short scrolls through notifications
  if (b == Button::Next && p == Press::Short) {
    m_cursor = (m_cursor + 1) % d.notifyCount;
    refreshList(d);
    return true;
  }

  // Select short dismisses focused item
  if (b == Button::Select && p == Press::Short) {
    DataStore::dismissNotification(m_cursor);
    const DataSnapshot updated = DataStore::getSnapshot();
    if (m_cursor >= updated.notifyCount && updated.notifyCount > 0) {
      m_cursor = updated.notifyCount - 1;
    }
    StatusBar::updateData(updated);
    refreshList(updated);
    return true;
  }

  return false;
}

void NotifyApp::onData(const DataSnapshot &d) {
  DataStore::markAllNotificationsRead();
  refreshList(d);
}

void NotifyApp::refreshList(const DataSnapshot &d) {
  if (m_emptyIcon == nullptr)
    return;

  if (d.notifyCount == 0) {
    lv_obj_clear_flag(m_emptyIcon, LV_OBJ_FLAG_HIDDEN);
    lv_obj_clear_flag(m_emptyLbl, LV_OBJ_FLAG_HIDDEN);
    for (int i = 0; i < kVisibleRows; ++i) {
      lv_obj_add_flag(m_cards[i], LV_OBJ_FLAG_HIDDEN);
    }
    return;
  }

  lv_obj_add_flag(m_emptyIcon, LV_OBJ_FLAG_HIDDEN);
  lv_obj_add_flag(m_emptyLbl, LV_OBJ_FLAG_HIDDEN);

  if (m_cursor >= d.notifyCount) {
    m_cursor = d.notifyCount - 1;
  }
  if (m_cursor < m_scrollTop) {
    m_scrollTop = m_cursor;
  } else if (m_cursor >= m_scrollTop + kVisibleRows) {
    m_scrollTop = m_cursor - kVisibleRows + 1;
  }

  for (int slot = 0; slot < kVisibleRows; ++slot) {
    const uint8_t itemIdx = m_scrollTop + static_cast<uint8_t>(slot);
    if (itemIdx < d.notifyCount) {
      const NotificationItem &item = d.notifications[itemIdx];
      lv_obj_clear_flag(m_cards[slot], LV_OBJ_FLAG_HIDDEN);
      char title[96];
      char body[192];
      const uint32_t accent = Theme::active().accent;
      Theme::sanitizeText(item.title, title, sizeof(title));
      Theme::formatText(item.body, body, sizeof(body), true, &accent);
      lv_label_set_text(m_titles[slot], title);
      lv_label_set_text(m_bodies[slot], body);

      char relBuf[16];
      formatRelativeTime(item.ts, relBuf, sizeof(relBuf));
      lv_label_set_text(m_times[slot], relBuf);

      Theme::setFocusRing(m_cards[slot], itemIdx == m_cursor);
    } else {
      lv_obj_add_flag(m_cards[slot], LV_OBJ_FLAG_HIDDEN);
    }
  }
}
