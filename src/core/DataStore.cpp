#include "core/DataStore.h"

#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <string.h>

namespace DataStore {

namespace {
SemaphoreHandle_t s_mutex = nullptr;
DataSnapshot s_data{};
} // namespace

void begin() {
  if (s_mutex == nullptr) {
    s_mutex = xSemaphoreCreateMutex();
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

void pushNotification(const char *title, const char *body, uint32_t ts) {
  if (title == nullptr || body == nullptr) {
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
    strncpy(item.title, title, sizeof(item.title) - 1);
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
