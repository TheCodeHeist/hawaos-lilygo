#include "services/WifiService.h"
#include "config.h"
#include "core/DataStore.h"
#include "core/EventBus.h"
#include "core/Settings.h"
#include "services/CompanionLink.h"
#include "services/BleLink.h"
#include "services/NtpService.h"
#include "services/SetupService.h"

#include <ArduinoJson.h>

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
volatile bool s_scanRequested = false;

// A join the companion asked for (reconfigure): reported back as
// wifi_status connected/failed so the setup wizard can react.
volatile bool s_joinRequested = false;
bool s_joinActive = false;
uint32_t s_joinStartMs = 0;
volatile uint8_t s_lastDiscReason = 0;
volatile uint8_t s_authFailures = 0;
volatile uint8_t s_noApFailures = 0;

bool isAuthFailure(uint8_t reason) {
  return reason == WIFI_REASON_AUTH_FAIL || reason == WIFI_REASON_AUTH_EXPIRE ||
         reason == WIFI_REASON_4WAY_HANDSHAKE_TIMEOUT ||
         reason == WIFI_REASON_HANDSHAKE_TIMEOUT;
}

void onStaDisconnected(arduino_event_id_t /*event*/, arduino_event_info_t info) {
  const uint8_t reason = info.wifi_sta_disconnected.reason;
  s_lastDiscReason = reason;
  if (isAuthFailure(reason)) {
    ++s_authFailures;
  } else if (reason == WIFI_REASON_NO_AP_FOUND) {
    ++s_noApFailures;
  }
}

void sendWifiStatus(const char *state, const char *ssid, const char *ip,
                    const char *reason) {
  JsonDocument doc;
  doc["type"] = "wifi_status";
  doc["state"] = state;
  doc["ssid"] = ssid;
  if (ip != nullptr)
    doc["ip"] = ip;
  if (reason != nullptr)
    doc["reason"] = reason;
  char buf[160];
  serializeJson(doc, buf, sizeof(buf));
  CompanionLink::sendJson(buf);
}

void finishJoin(bool ok, const char *ssid, const char *ip, const char *reason) {
  s_joinActive = false;
  Serial.printf("[WifiService] Join '%s' %s%s\n", ssid,
                ok ? "succeeded" : "failed: ", ok ? "" : reason);
  sendWifiStatus(ok ? "connected" : "failed", ssid, ip, reason);
  SetupService::onJoinResult(ok, ip, reason);
}

// Blocking scan (~2 s); answers with the strongest 2.4 GHz networks the
// device itself can see, one entry per SSID.
void runScan() {
  WiFi.mode(WIFI_STA); // the radio is off while nothing is provisioned
  WiFi.setSleep(BleLink::isRunning());
  const int16_t n = WiFi.scanNetworks(false, false);
  JsonDocument doc;
  doc["type"] = "wifi_scan";
  JsonArray nets = doc["nets"].to<JsonArray>();
  constexpr uint8_t kMaxNets = 12;
  for (int16_t i = 0; i < n && nets.size() < kMaxNets; ++i) {
    const String ssid = WiFi.SSID(i);
    if (ssid.isEmpty())
      continue;
    bool dup = false;
    for (JsonObject existing : nets) {
      if (ssid == existing["ssid"].as<const char *>()) {
        dup = true; // scan results are sorted by RSSI: keep the first
        break;
      }
    }
    if (dup)
      continue;
    JsonObject net = nets.add<JsonObject>();
    net["ssid"] = ssid;
    net["rssi"] = WiFi.RSSI(i);
    net["secure"] = WiFi.encryptionType(i) != WIFI_AUTH_OPEN;
  }
  WiFi.scanDelete();
  char buf[1024];
  serializeJson(doc, buf, sizeof(buf));
  CompanionLink::sendJson(buf);
  Serial.printf("[WifiService] Scan: %d networks\n", static_cast<int>(n));
}

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
  s_authFailures = 0;
  s_noApFailures = 0;
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
  // Modem sleep must stay on while BLE runs (setup), or the WiFi driver
  // aborts; off otherwise for snappier UDP.
  WiFi.setSleep(BleLink::isRunning());
  WiFi.onEvent(onStaDisconnected, ARDUINO_EVENT_WIFI_STA_DISCONNECTED);

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
      if (s_joinRequested) {
        s_joinRequested = false;
        s_joinActive = true;
        s_joinStartMs = nowMs;
        sendWifiStatus("connecting", ssid, nullptr, nullptr);
        SetupService::onJoinStarted(ssid);
      }
    }

    if (s_scanRequested) {
      s_scanRequested = false;
      runScan();
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
        if (s_joinActive) {
          finishJoin(true, ssid, ipStr.c_str(), nullptr);
        }
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

    if (s_joinActive && !connected) {
      const char *reason = nullptr;
      // One auth failure can be a fluke on a busy AP; two is a wrong password.
      if (s_authFailures >= 2) {
        reason = "auth";
      } else if (s_noApFailures >= 3) {
        reason = "no_ap";
      } else if (nowMs - s_joinStartMs >= Config::WIFI_JOIN_TIMEOUT_MS) {
        reason = "timeout";
      }
      if (reason != nullptr) {
        finishJoin(false, ssid, nullptr, reason);
        if (SetupService::isActive()) {
          // During setup, don't keep retrying credentials that failed.
          Settings::clearWifiCredentials();
          ssid[0] = '\0';
          pass[0] = '\0';
          WiFi.disconnect(false, true);
          DataStore::updateWifi(false, 0, "", "");
        }
      }
    }

    CompanionLink::poll();
    SetupService::poll();
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
  s_joinRequested = true;
  s_reconfigRequested = true;
}

void requestScan() { s_scanRequested = true; }

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
