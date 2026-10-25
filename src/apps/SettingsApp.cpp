#include "apps/SettingsApp.h"
#include "config.h"
#include "core/AppManager.h"
#include "core/DataStore.h"
#include "core/Settings.h"
#include "drivers/Display.h"
#include "services/WifiService.h"
#include "ui/Theme.h"

#include <Arduino.h>
#include <stdio.h>

void SettingsApp::onEnter(lv_obj_t *root) {
  const ThemeTokens::Palette &p = Theme::active();

  for (int i = 0; i < kVisibleRows; ++i) {
    m_rows[i] = Theme::createCard(root, 300, 38, Config::OUTER_MARGIN_PX,
                                  Config::CONTENT_Y_START + 8 +
                                      static_cast<int16_t>(i * 44),
                                  Config::RADIUS_SM);
    lv_obj_set_style_pad_hor(m_rows[i], 12, LV_PART_MAIN);

    m_rowNames[i] = lv_label_create(m_rows[i]);
    lv_obj_set_style_text_font(m_rowNames[i], Theme::fontBodySb(),
                               LV_PART_MAIN);
    lv_obj_set_style_text_color(m_rowNames[i], lv_color_hex(p.text),
                                LV_PART_MAIN);
    lv_obj_align(m_rowNames[i], LV_ALIGN_LEFT_MID, 0, 0);

    m_rowVals[i] = lv_label_create(m_rows[i]);
    lv_obj_set_style_text_font(m_rowVals[i], Theme::fontBody(), LV_PART_MAIN);
    lv_obj_set_style_text_color(m_rowVals[i], lv_color_hex(p.accent),
                                LV_PART_MAIN);
    lv_obj_align(m_rowVals[i], LV_ALIGN_RIGHT_MID, 0, 0);
  }

  refreshRows();
}

void SettingsApp::onExit() {
  for (int i = 0; i < kVisibleRows; ++i) {
    m_rows[i] = nullptr;
    m_rowNames[i] = nullptr;
    m_rowVals[i] = nullptr;
  }
}

void SettingsApp::onTick(uint32_t /*nowMs*/) {}

bool SettingsApp::onButton(Button b, Press p) {
  if (b == Button::Next && p == Press::Short) {
    m_cursor = (m_cursor + 1) % kTotalItems;
    refreshRows();
    return true;
  }

  if (b == Button::Select && p == Press::Short) {
    switch (m_cursor) {
    case 0: // WiFi Reconnect
      WifiService::forceReconnect();
      refreshRows();
      break;
    case 1: // Brightness (20/40/60/80/100%)
      Settings::cycleBrightnessPct();
      Display::setBrightness(Settings::getBrightnessStep());
      refreshRows();
      break;
    case 2: // Theme (Dark / Light)
      Settings::toggleDarkLightTheme();
      AppManager::applyActiveTheme();
      break;
    case 3: // Orientation (Normal / Inverted)
      Settings::setOrientationInverted(!Settings::getOrientationInverted());
      refreshRows();
      break;
    case 4: // Notification Popup (On / Off)
      Settings::setNotifyPopupEnabled(!Settings::getNotifyPopupEnabled());
      Settings::bumpLocalConfigVersion();
      refreshRows();
      break;
    case 5: { // AOD Timeout (30s / 60s / 120s / 300s / Off)
      if (!Settings::getAodEnabled()) {
        Settings::setAodEnabled(true);
        Settings::setAodTimeoutSec(30);
      } else {
        const uint16_t cur = Settings::getAodTimeoutSec();
        if (cur < 60)
          Settings::setAodTimeoutSec(60);
        else if (cur < 120)
          Settings::setAodTimeoutSec(120);
        else if (cur < 300)
          Settings::setAodTimeoutSec(300);
        else
          Settings::setAodEnabled(false);
      }
      Settings::bumpLocalConfigVersion();
      refreshRows();
      break;
    }
    case 6: // Clock Format (12h / 24h)
      Settings::setUse24Hour(!Settings::getUse24Hour());
      Settings::bumpLocalConfigVersion();
      refreshRows();
      break;
    case 7: // Forget WiFi (clears NVS credentials for Companion setup)
      WifiService::forgetWifi();
      refreshRows();
      break;
    case 8: // Reboot
      ESP.restart();
      break;
    }
    return true;
  }
  return false;
}

void SettingsApp::onData(const DataSnapshot & /*d*/) { refreshRows(); }

void SettingsApp::refreshRows() {
  if (m_rows[0] == nullptr)
    return;

  if (m_cursor < m_scrollTop) {
    m_scrollTop = m_cursor;
  } else if (m_cursor >= m_scrollTop + kVisibleRows) {
    m_scrollTop = m_cursor - kVisibleRows + 1;
  }

  const DataSnapshot d = DataStore::getSnapshot();

  for (int slot = 0; slot < kVisibleRows; ++slot) {
    const int itemIdx = m_scrollTop + slot;
    const char *name = "";
    char valBuf[32] = {0};

    switch (itemIdx) {
    case 0:
      name = "WiFi";
      if (d.wifiConnected) {
        snprintf(valBuf, sizeof(valBuf), "%s", d.wifiIp);
      } else if (d.wifiSsid[0] != '\0') {
        snprintf(valBuf, sizeof(valBuf), "Retry (%s)", d.wifiSsid);
      } else {
        snprintf(valBuf, sizeof(valBuf), "Use Companion");
      }
      break;
    case 1:
      name = "Brightness";
      snprintf(valBuf, sizeof(valBuf), "%u%%",
               static_cast<unsigned>(Settings::getBrightnessPct()));
      break;
    case 2:
      name = "Theme";
      snprintf(valBuf, sizeof(valBuf), "%s",
               Theme::isDark() ? "Dark" : "Light");
      break;
    case 3:
      name = "Orientation";
      snprintf(valBuf, sizeof(valBuf), "%s",
               Settings::getOrientationInverted() ? "Inverted (USB-R)"
                                                  : "Normal (USB-L)");
      break;
    case 4:
      name = "Notify Popup";
      snprintf(valBuf, sizeof(valBuf), "%s",
               Settings::getNotifyPopupEnabled() ? "Enabled" : "Off");
      break;
    case 5:
      name = "AOD Timeout";
      if (!Settings::getAodEnabled()) {
        snprintf(valBuf, sizeof(valBuf), "Off");
      } else {
        snprintf(valBuf, sizeof(valBuf), "%us",
                 static_cast<unsigned>(Settings::getAodTimeoutSec()));
      }
      break;
    case 6:
      name = "Clock Format";
      snprintf(valBuf, sizeof(valBuf), "%s",
               Settings::getUse24Hour() ? "24-hour" : "12-hour");
      break;
    case 7:
      name = "Forget WiFi";
      snprintf(valBuf, sizeof(valBuf), d.wifiSsid[0] ? "Clear NVS" : "Cleared");
      break;
    case 8:
      name = "HawaOS v0.4.0";
      snprintf(valBuf, sizeof(valBuf), "Reboot");
      break;
    }

    lv_label_set_text(m_rowNames[slot], name);
    lv_label_set_text(m_rowVals[slot], valBuf);
    Theme::setFocusRing(m_rows[slot], itemIdx == m_cursor);
  }
}
