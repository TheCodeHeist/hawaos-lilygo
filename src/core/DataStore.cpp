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

} // namespace DataStore
