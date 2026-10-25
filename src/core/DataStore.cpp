#include "core/DataStore.h"

#include <Arduino.h>
#include <esp_heap_caps.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <string.h>

namespace DataStore {

namespace {
SemaphoreHandle_t s_mutex = nullptr;
DataSnapshot s_data{};
uint16_t *s_albumArtBuf = nullptr;
} // namespace

void begin() {
  if (s_mutex == nullptr) {
    s_mutex = xSemaphoreCreateMutex();
  }
  if (s_albumArtBuf == nullptr) {
    s_albumArtBuf = static_cast<uint16_t *>(
        heap_caps_malloc(ALBUM_ART_BYTES, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT));
    if (s_albumArtBuf != nullptr) {
      memset(s_albumArtBuf, 0, ALBUM_ART_BYTES);
    }
  }
}

DataSnapshot getSnapshot() {
  DataSnapshot copy{};
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    copy = s_data;
    xSemaphoreGive(s_mutex);
  }
  return copy;
}

void setSnapshot(const DataSnapshot &snapshot) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data = snapshot;
    xSemaphoreGive(s_mutex);
  }
}

void updateStats(uint32_t ts, uint8_t cpuPct, uint8_t ramPct,
                 uint32_t netUpKbps, uint32_t netDownKbps, bool viaUsb) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.timestamp = ts;
    s_data.cpuPercent = (cpuPct > 100) ? 100 : cpuPct;
    s_data.ramPercent = (ramPct > 100) ? 100 : ramPct;
    s_data.netUpKbps = netUpKbps;
    s_data.netDownKbps = netDownKbps;
    s_data.lastStatsMs = millis();
    s_data.usbConnected = viaUsb;

    for (uint8_t i = 0; i + 1 < DataSnapshot::SPARKLINE_LEN; ++i) {
      s_data.netDownHistory[i] = s_data.netDownHistory[i + 1];
    }
    const uint32_t clampedKbps = (netDownKbps > 65535U) ? 65535U : netDownKbps;
    s_data.netDownHistory[DataSnapshot::SPARKLINE_LEN - 1] =
        static_cast<uint16_t>(clampedKbps);

    xSemaphoreGive(s_mutex);
  }
}

void updateMedia(MediaPlayState state, const char *title, const char *artist,
                 const char *album, uint32_t posMs, uint32_t durMs,
                 const char *artHash, uint32_t bgTop, uint32_t bgBottom,
                 bool inkLight) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.mediaState = state;

    if (title != nullptr) {
      strncpy(s_data.mediaTitle, title, sizeof(s_data.mediaTitle) - 1);
      s_data.mediaTitle[sizeof(s_data.mediaTitle) - 1] = '\0';
    } else {
      s_data.mediaTitle[0] = '\0';
    }

    if (artist != nullptr) {
      strncpy(s_data.mediaArtist, artist, sizeof(s_data.mediaArtist) - 1);
      s_data.mediaArtist[sizeof(s_data.mediaArtist) - 1] = '\0';
    } else {
      s_data.mediaArtist[0] = '\0';
    }

    if (album != nullptr) {
      strncpy(s_data.mediaAlbum, album, sizeof(s_data.mediaAlbum) - 1);
      s_data.mediaAlbum[sizeof(s_data.mediaAlbum) - 1] = '\0';
    } else {
      s_data.mediaAlbum[0] = '\0';
    }

    s_data.mediaPosMs = posMs;
    s_data.mediaDurMs = durMs;
    s_data.mediaRxMillis = millis();
    s_data.mediaBgTop = bgTop;
    s_data.mediaBgBottom = bgBottom;
    s_data.mediaInkLight = inkLight;

    const char *safeHash = (artHash != nullptr) ? artHash : "";
    if (strncmp(s_data.mediaArtHash, safeHash, sizeof(s_data.mediaArtHash)) !=
        0) {
      strncpy(s_data.mediaArtHash, safeHash, sizeof(s_data.mediaArtHash) - 1);
      s_data.mediaArtHash[sizeof(s_data.mediaArtHash) - 1] = '\0';
      s_data.mediaArtReady = false;
    }

    xSemaphoreGive(s_mutex);
  }
}

void setAlbumArtReady(const char *artHash, bool ready) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    if (artHash != nullptr && strncmp(s_data.mediaArtHash, artHash,
                                      sizeof(s_data.mediaArtHash)) == 0) {
      s_data.mediaArtReady = ready;
    }
    xSemaphoreGive(s_mutex);
  }
}

uint16_t *albumArtBuffer() { return s_albumArtBuf; }

void updateWeather(const WeatherSnapshot &ws) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.weather = ws;
    s_data.weather.valid = true;
    s_data.weather.lastUpdateTs = millis();
    xSemaphoreGive(s_mutex);
  }
}

void updateWifi(bool connected, int8_t rssi, const char *ssid, const char *ip) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.wifiConnected = connected;
    s_data.wifiRssi = rssi;
    if (ssid != nullptr) {
      strncpy(s_data.wifiSsid, ssid, sizeof(s_data.wifiSsid) - 1);
      s_data.wifiSsid[sizeof(s_data.wifiSsid) - 1] = '\0';
    } else {
      s_data.wifiSsid[0] = '\0';
    }
    if (ip != nullptr) {
      strncpy(s_data.wifiIp, ip, sizeof(s_data.wifiIp) - 1);
      s_data.wifiIp[sizeof(s_data.wifiIp) - 1] = '\0';
    } else {
      s_data.wifiIp[0] = '\0';
    }
    xSemaphoreGive(s_mutex);
  }
}

void updateNtp(bool synced) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.ntpSynced = synced;
    xSemaphoreGive(s_mutex);
  }
}

void pushNotification(const char *app, const char *title, const char *body, uint32_t ts) {
  if ((app == nullptr || app[0] == '\0') && (title == nullptr || title[0] == '\0') &&
      (body == nullptr || body[0] == '\0')) {
    return;
  }
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    const uint8_t cap = DataSnapshot::MAX_NOTIFICATIONS;
    const uint8_t shiftCount =
        (s_data.notifyCount < cap) ? s_data.notifyCount : (cap - 1);
    for (int i = static_cast<int>(shiftCount); i > 0; --i) {
      s_data.notifications[i] = s_data.notifications[i - 1];
    }
    NotificationItem &item = s_data.notifications[0];
    const char *safeApp = (app != nullptr && app[0] != '\0')
                              ? app
                              : ((title != nullptr && title[0] != '\0') ? title : "Notification");
    strncpy(item.app, safeApp, sizeof(item.app) - 1);
    item.app[sizeof(item.app) - 1] = '\0';
    strncpy(item.title, title ? title : "", sizeof(item.title) - 1);
    item.title[sizeof(item.title) - 1] = '\0';
    strncpy(item.body, body, sizeof(item.body) - 1);
    item.body[sizeof(item.body) - 1] = '\0';
    item.ts = ts;
    item.unread = true;

    if (s_data.notifyCount < cap) {
      ++s_data.notifyCount;
    }
    uint8_t unread = 0;
    for (uint8_t i = 0; i < s_data.notifyCount; ++i) {
      if (s_data.notifications[i].unread)
        ++unread;
    }
    s_data.unreadCount = unread;
    xSemaphoreGive(s_mutex);
  }
}

void dismissNotification(uint8_t index) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    if (index < s_data.notifyCount) {
      for (uint8_t i = index; i + 1 < s_data.notifyCount; ++i) {
        s_data.notifications[i] = s_data.notifications[i + 1];
      }
      --s_data.notifyCount;
      uint8_t unread = 0;
      for (uint8_t i = 0; i < s_data.notifyCount; ++i) {
        if (s_data.notifications[i].unread)
          ++unread;
      }
      s_data.unreadCount = unread;
    }
    xSemaphoreGive(s_mutex);
  }
}

void clearAllNotifications() {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.notifyCount = 0;
    s_data.unreadCount = 0;
    xSemaphoreGive(s_mutex);
  }
}

void markAllNotificationsRead() {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    for (uint8_t i = 0; i < s_data.notifyCount; ++i) {
      s_data.notifications[i].unread = false;
    }
    s_data.unreadCount = 0;
    xSemaphoreGive(s_mutex);
  }
}

} // namespace DataStore
