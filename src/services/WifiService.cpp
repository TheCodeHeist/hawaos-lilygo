#include "services/WifiService.h"
#include "config.h"
#include "core/DataStore.h"
#include "core/EventBus.h"
#include "core/Settings.h"
#include "services/CompanionLink.h"
#include "services/NtpService.h"

#include <Arduino.h>
#include <WiFi.h>
#include <freertos/FreeRTOS.h>
#include <freertos/task.h>

namespace WifiService {

namespace {

TaskHandle_t s_wifiTaskHandle = nullptr;
volatile bool s_reconfigRequested = false;
volatile bool s_forceReconnect = false;
volatile bool s_forgetRequested = false;
bool s_wasConnected = false;

void startConnectAttempt(char *ssidBuf, size_t ssidLen, char *passBuf,
                         size_t passLen) {
  Settings::getWifiSsid(ssidBuf, ssidLen);
  Settings::getWifiPass(passBuf, passLen);

  if (ssidBuf[0] == '\0') {
    Serial.println("[WifiService] No WiFi provisioned yet. Waiting for HawaOS "
                   "Companion setup.");
    WiFi.disconnect(true, true);
    DataStore::updateWifi(false, 0, "", "");
    Event ev{};
    ev.type = EventType::DataUpdated;
    EventBus::post(ev);
    return;
  }

  Serial.printf("[WifiService] Connecting to SSID '%s'...\n", ssidBuf);
  WiFi.disconnect(false, false);
  WiFi.begin(ssidBuf, passBuf);
}

void wifiTaskEntry(void * /*param*/) {
  char ssid[33] = {0};
  char pass[65] = {0};

  // Do not let ESP-IDF auto-connect from its hidden net80211 flash sector;
  // only connect using credentials provisioned in HawaOS Settings.
  WiFi.persistent(false);
  WiFi.mode(WIFI_STA);
  WiFi.setAutoReconnect(true);
  WiFi.setSleep(false);

  NtpService::begin();
  CompanionLink::begin();
  startConnectAttempt(ssid, sizeof(ssid), pass, sizeof(pass));

  uint32_t lastAttemptMs = millis();
  uint32_t lastRssiPublishMs = 0;
  int8_t lastPublishedRssi = 0;

  for (;;) {
    const uint32_t nowMs = millis();

    if (s_forgetRequested) {
      s_forgetRequested = false;
      s_reconfigRequested = false;
      s_forceReconnect = false;
      s_wasConnected = false;
      ssid[0] = '\0';
      pass[0] = '\0';
      WiFi.disconnect(true, true);
      DataStore::updateWifi(false, 0, "", "");
      Event ev{};
      ev.type = EventType::DataUpdated;
      EventBus::post(ev);
      CompanionLink::sendConfigAck(Settings::getConfigVersion());
    }

    if (s_reconfigRequested || s_forceReconnect) {
      s_reconfigRequested = false;
      s_forceReconnect = false;
      s_wasConnected = false;
      Settings::getWifiSsid(ssid, sizeof(ssid));
      DataStore::updateWifi(false, 0, ssid, "");
      Event ev{};
      ev.type = EventType::DataUpdated;
      EventBus::post(ev);

      startConnectAttempt(ssid, sizeof(ssid), pass, sizeof(pass));
      lastAttemptMs = nowMs;
    }

    const wl_status_t st = WiFi.status();
    const bool connected = (ssid[0] != '\0') && (st == WL_CONNECTED);

    if (connected) {
      const int8_t rssi = static_cast<int8_t>(WiFi.RSSI());
      String ipStr = WiFi.localIP().toString();

      if (!s_wasConnected) {
        s_wasConnected = true;
        lastPublishedRssi = rssi;
        lastRssiPublishMs = nowMs;

        Serial.printf("[WifiService] Connected! IP=%s | RSSI=%ddBm\n",
                      ipStr.c_str(), static_cast<int>(rssi));

        DataStore::updateWifi(true, rssi, ssid, ipStr.c_str());
        Event ev{};
        ev.type = EventType::DataUpdated;
        EventBus::post(ev);

        NtpService::onWifiConnected();
        CompanionLink::sendConfigAck(Settings::getConfigVersion());
      } else if ((nowMs - lastRssiPublishMs >= 3000) &&
                 (abs(static_cast<int>(rssi) -
                      static_cast<int>(lastPublishedRssi)) >= 2)) {
        lastPublishedRssi = rssi;
        lastRssiPublishMs = nowMs;
        DataStore::updateWifi(true, rssi, ssid, ipStr.c_str());
        Event ev{};
        ev.type = EventType::DataUpdated;
        EventBus::post(ev);
      }

      NtpService::poll();
    } else {
      if (s_wasConnected) {
        s_wasConnected = false;
        Serial.println(
            "[WifiService] Disconnected from AP! Will retry automatically...");
        DataStore::updateWifi(false, 0, ssid, "");
        Event ev{};
        ev.type = EventType::DataUpdated;
        EventBus::post(ev);
        lastAttemptMs = nowMs;
      } else if (ssid[0] != '\0' && (nowMs - lastAttemptMs >= 12000)) {
        Serial.printf(
            "[WifiService] Still disconnected (status=%d), retrying '%s'...\n",
            static_cast<int>(st), ssid);
        WiFi.reconnect();
        lastAttemptMs = nowMs;
      }
    }

    CompanionLink::poll();
    vTaskDelay(pdMS_TO_TICKS(25));
  }
}

} // namespace

void begin() {
  xTaskCreatePinnedToCore(
      wifiTaskEntry, "wifi_task", Config::WIFI_TASK_STACK_BYTES, nullptr,
      Config::WIFI_TASK_PRIORITY, &s_wifiTaskHandle, Config::WIFI_TASK_CORE);
}

void reconfigure(const char *ssid, const char *pass) {
  Settings::setWifiCredentials(ssid, pass);
  s_reconfigRequested = true;
}

void forgetWifi() {
  Settings::clearWifiCredentials();
  s_forgetRequested = true;
}

void forceReconnect() {
  Serial.println("[WifiService] Force reconnect requested");
  s_forceReconnect = true;
}

bool isConnected() { return s_wasConnected; }

} // namespace WifiService
