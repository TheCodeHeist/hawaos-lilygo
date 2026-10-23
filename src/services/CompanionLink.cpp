#include "services/CompanionLink.h"
#include "config.h"
#include "core/DataStore.h"
#include "core/EventBus.h"
#include "core/Settings.h"
#include "services/WifiService.h"

#include <Arduino.h>
#include <ArduinoJson.h>
#include <HTTPClient.h>
#include <WiFi.h>
#include <WiFiUdp.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <stdlib.h>
#include <string.h>

namespace CompanionLink {

namespace {

WiFiUDP s_udp;
bool s_udpStarted = false;
IPAddress s_companionIp;
uint16_t s_companionPort = Config::COMPANION_UDP_PORT;
bool s_hasCompanionPeer = false;
bool s_lastRxViaUsb = false;

char s_rxBuf[1460] = {0};
char s_serialLine[1024] = {0};
size_t s_serialLen = 0;

char s_pendingArtHash[16] = {0};
char s_loadedArtHash[16] = {0};
uint32_t s_lastArtAttemptMs = 0;

SemaphoreHandle_t s_txMutex = nullptr;
char s_pendingCmd[24] = {0};
uint32_t s_pendingCmdPosMs = 0;
bool s_hasPendingCmd = false;
uint32_t s_pendingAckVer = 0;
bool s_hasPendingAck = false;

uint32_t parseHexColor(const char *hexStr, uint32_t fallback) {
  if (hexStr == nullptr || hexStr[0] == '\0') {
    return fallback;
  }
  if (hexStr[0] == '#') {
    ++hexStr;
  }
  char *endPtr = nullptr;
  const unsigned long val = strtoul(hexStr, &endPtr, 16);
  if (endPtr == hexStr) {
    return fallback;
  }
  return static_cast<uint32_t>(val & 0xFFFFFFUL);
}

void postDataUpdated() {
  Event ev{};
  ev.type = EventType::DataUpdated;
  EventBus::post(ev);
}

void postSettingsChanged() {
  Event ev{};
  ev.type = EventType::SettingsChanged;
  EventBus::post(ev);
}

void sendJsonPacket(const char *jsonStr) {
  if (jsonStr == nullptr)
    return;
  if (s_hasCompanionPeer && WiFi.status() == WL_CONNECTED) {
    s_udp.beginPacket(s_companionIp, s_companionPort);
    s_udp.write(reinterpret_cast<const uint8_t *>(jsonStr), strlen(jsonStr));
    s_udp.endPacket();
  }
  if (s_lastRxViaUsb) {
    Serial.println(jsonStr);
  }
}

void handleJsonMessage(const char *payload, size_t len, bool viaUsb) {
  JsonDocument doc;
  const DeserializationError err = deserializeJson(doc, payload, len);
  if (err) {
    return;
  }

  s_lastRxViaUsb = viaUsb;
  const char *type = doc["type"] | "";

  if (strcmp(type, "stats") == 0) {
    const uint32_t ts = doc["ts"] | 0U;
    const uint8_t cpu = doc["cpu"] | 0U;
    const uint8_t ram = doc["ram"] | 0U;
    uint32_t netUp = doc["net_up_kbps"] | 0U;
    uint32_t netDown = doc["net_down_kbps"] | 0U;
    if (netUp == 0 && doc["net"]["up"].is<uint32_t>()) {
      netUp = doc["net"]["up"].as<uint32_t>();
    }
    if (netDown == 0 && doc["net"]["down"].is<uint32_t>()) {
      netDown = doc["net"]["down"].as<uint32_t>();
    }

    DataStore::updateStats(ts, cpu, ram, netUp, netDown, viaUsb);

    if (doc["notify"].is<JsonArrayConst>()) {
      JsonArrayConst arr = doc["notify"].as<JsonArrayConst>();
      for (JsonObjectConst item : arr) {
        const char *title = item["title"] | "";
        const char *body = item["body"] | "";
        const uint32_t nts = item["ts"] | ts;
        if (title[0] != '\0') {
          DataStore::pushNotification(title, body, nts);
        }
      }
    }

    postDataUpdated();
    return;
  }

  if (strcmp(type, "media") == 0) {
    const char *stateStr = doc["state"] | "stopped";
    MediaPlayState state = MediaPlayState::Stopped;
    if (strcmp(stateStr, "playing") == 0) {
      state = MediaPlayState::Playing;
    } else if (strcmp(stateStr, "paused") == 0) {
      state = MediaPlayState::Paused;
    }

    const char *title = doc["title"] | "";
    const char *artist = doc["artist"] | "";
    const char *album = doc["album"] | "";
    const uint32_t posMs = doc["pos_ms"] | 0U;
    const uint32_t durMs = doc["dur_ms"] | 0U;
    const char *artHash = doc["art"] | "";
    const uint32_t bgTop = parseHexColor(doc["bg_top"] | "#071115", 0x071115);
    const uint32_t bgBot =
        parseHexColor(doc["bg_bottom"] | "#05090b", 0x05090B);
    const char *inkStr = doc["ink"] | "light";
    const bool inkLight = (strcmp(inkStr, "dark") != 0);

    DataStore::updateMedia(state, title, artist, album, posMs, durMs, artHash,
                           bgTop, bgBot, inkLight);

    if (artHash[0] != '\0') {
      if (strncmp(artHash, s_loadedArtHash, sizeof(s_loadedArtHash)) == 0) {
        DataStore::setAlbumArtReady(artHash, true);
      } else {
        strncpy(s_pendingArtHash, artHash, sizeof(s_pendingArtHash) - 1);
        s_pendingArtHash[sizeof(s_pendingArtHash) - 1] = '\0';
      }
    } else {
      s_pendingArtHash[0] = '\0';
    }

    postDataUpdated();
    return;
  }

  if (strcmp(type, "notify") == 0) {
    const char *title = doc["title"] | "";
    const char *body = doc["body"] | "";
    const uint32_t ts = doc["ts"] | static_cast<uint32_t>(millis() / 1000);
    if (title[0] != '\0') {
      DataStore::pushNotification(title, body, ts);
      postDataUpdated();
    }
    return;
  }

  if (strcmp(type, "config") == 0) {
    const uint32_t v = doc["v"] | 0U;
    if (v > 0 && v < Settings::getConfigVersion()) {
      sendConfigAck(Settings::getConfigVersion());
      return;
    }

    if (doc["theme"].is<const char *>()) {
      const char *th = doc["theme"];
      if (strcmp(th, "light") == 0) {
        Settings::setThemeMode(ThemeMode::Light);
      } else if (strcmp(th, "auto") == 0) {
        Settings::setThemeMode(ThemeMode::Auto);
      } else {
        Settings::setThemeMode(ThemeMode::Dark);
      }
    }

    if (doc["brightness"].is<uint8_t>()) {
      Settings::setBrightnessPct(doc["brightness"].as<uint8_t>());
    }
    if (doc["aod"].is<bool>()) {
      Settings::setAodEnabled(doc["aod"].as<bool>());
    }
    if (doc["aod_timeout_s"].is<uint16_t>()) {
      Settings::setAodTimeoutSec(doc["aod_timeout_s"].as<uint16_t>());
    }
    if (doc["reduce_motion"].is<bool>()) {
      Settings::setReduceMotion(doc["reduce_motion"].as<bool>());
    }
    if (doc["wallpaper"].is<const char *>()) {
      const char *wp = doc["wallpaper"];
      Settings::setWallpaperMode((strcmp(wp, "solid") == 0)
                                     ? WallpaperMode::Solid
                                     : WallpaperMode::Gradient);
    }
    if (doc["use_24h"].is<bool>()) {
      Settings::setUse24Hour(doc["use_24h"].as<bool>());
    }
    if (doc["wifi_ssid"].is<const char *>() &&
        doc["wifi_pass"].is<const char *>()) {
      const char *newSsid = doc["wifi_ssid"];
      const char *newPass = doc["wifi_pass"];
      if (newSsid[0] != '\0') {
        WifiService::reconfigure(newSsid, newPass);
      }
    }

    if (v > 0) {
      Settings::setConfigVersion(v);
      sendConfigAck(v);
    }
    postSettingsChanged();
    return;
  }
}

void fetchPendingAlbumArt() {
  if (s_pendingArtHash[0] == '\0')
    return;
  if (strncmp(s_pendingArtHash, s_loadedArtHash, sizeof(s_loadedArtHash)) ==
      0) {
    DataStore::setAlbumArtReady(s_pendingArtHash, true);
    s_pendingArtHash[0] = '\0';
    return;
  }
  if (!s_hasCompanionPeer || WiFi.status() != WL_CONNECTED)
    return;

  const uint32_t nowMs = millis();
  if (nowMs - s_lastArtAttemptMs < 2000)
    return;
  s_lastArtAttemptMs = nowMs;

  uint16_t *artBuf = DataStore::albumArtBuffer();
  if (artBuf == nullptr)
    return;

  char url[96];
  snprintf(url, sizeof(url), "http://%s:%u/art/%s",
           s_companionIp.toString().c_str(),
           static_cast<unsigned>(Config::COMPANION_ART_PORT), s_pendingArtHash);

  HTTPClient http;
  http.setTimeout(1500);
  if (!http.begin(url)) {
    return;
  }

  const int code = http.GET();
  if (code == HTTP_CODE_OK &&
      http.getSize() == static_cast<int>(DataStore::ALBUM_ART_BYTES)) {
    WiFiClient *stream = http.getStreamPtr();
    uint8_t *dst = reinterpret_cast<uint8_t *>(artBuf);
    size_t totalRead = 0;
    const uint32_t startMs = millis();

    while (http.connected() && totalRead < DataStore::ALBUM_ART_BYTES &&
           (millis() - startMs) < 1500) {
      const size_t avail = stream->available();
      if (avail > 0) {
        const size_t toRead =
            min(avail, DataStore::ALBUM_ART_BYTES - totalRead);
        const int r = stream->readBytes(dst + totalRead, toRead);
        if (r > 0) {
          totalRead += static_cast<size_t>(r);
        }
      } else {
        delay(1);
      }
    }

    if (totalRead == DataStore::ALBUM_ART_BYTES) {
      strncpy(s_loadedArtHash, s_pendingArtHash, sizeof(s_loadedArtHash) - 1);
      s_loadedArtHash[sizeof(s_loadedArtHash) - 1] = '\0';
      DataStore::setAlbumArtReady(s_loadedArtHash, true);
      s_pendingArtHash[0] = '\0';
      Serial.printf("[CompanionLink] Pulled 128x128 RGB565 art (%s)\n",
                    s_loadedArtHash);
      postDataUpdated();
    }
  }
  http.end();
}

void flushPendingOutbound() {
  char cmdLocal[24] = {0};
  uint32_t posLocal = 0;
  bool sendCmd = false;
  uint32_t ackVer = 0;
  bool sendAck = false;

  if (s_txMutex != nullptr &&
      xSemaphoreTake(s_txMutex, pdMS_TO_TICKS(10)) == pdTRUE) {
    if (s_hasPendingCmd) {
      strncpy(cmdLocal, s_pendingCmd, sizeof(cmdLocal) - 1);
      posLocal = s_pendingCmdPosMs;
      s_hasPendingCmd = false;
      sendCmd = true;
    }
    if (s_hasPendingAck) {
      ackVer = s_pendingAckVer;
      s_hasPendingAck = false;
      sendAck = true;
    }
    xSemaphoreGive(s_txMutex);
  }

  if (sendCmd) {
    char buf[96];
    snprintf(buf, sizeof(buf),
             "{\"type\":\"cmd\",\"cmd\":\"%s\",\"pos_ms\":%lu}", cmdLocal,
             static_cast<unsigned long>(posLocal));
    sendJsonPacket(buf);
  }
  if (sendAck) {
    char buf[64];
    snprintf(buf, sizeof(buf), "{\"type\":\"config_ack\",\"v\":%lu}",
             static_cast<unsigned long>(ackVer));
    sendJsonPacket(buf);
  }
}

void pollUsbSerial() {
  while (Serial.available() > 0) {
    const char c = static_cast<char>(Serial.read());
    if (c == '\n' || c == '\r') {
      if (s_serialLen > 0) {
        s_serialLine[s_serialLen] = '\0';
        if (s_serialLine[0] == '{') {
          handleJsonMessage(s_serialLine, s_serialLen, true);
        }
        s_serialLen = 0;
      }
    } else if (s_serialLen + 1 < sizeof(s_serialLine)) {
      s_serialLine[s_serialLen++] = c;
    } else {
      s_serialLen = 0; // Overflow protection
    }
  }
}

} // namespace

void begin() {
  if (s_txMutex == nullptr) {
    s_txMutex = xSemaphoreCreateMutex();
  }
}

void poll() {
  pollUsbSerial();

  if (WiFi.status() == WL_CONNECTED) {
    if (!s_udpStarted) {
      if (s_udp.begin(Config::COMPANION_UDP_PORT)) {
        s_udpStarted = true;
        Serial.printf("[CompanionLink] Listening on UDP port %u\n",
                      static_cast<unsigned>(Config::COMPANION_UDP_PORT));
      }
    }

    if (s_udpStarted) {
      int pktSize = s_udp.parsePacket();
      while (pktSize > 0) {
        s_companionIp = s_udp.remoteIP();
        s_companionPort = s_udp.remotePort();
        s_hasCompanionPeer = true;

        const int readLen =
            s_udp.read(s_rxBuf, static_cast<int>(sizeof(s_rxBuf) - 1));
        if (readLen > 0) {
          s_rxBuf[readLen] = '\0';
          handleJsonMessage(s_rxBuf, static_cast<size_t>(readLen), false);
        }
        pktSize = s_udp.parsePacket();
      }
    }

    flushPendingOutbound();
    fetchPendingAlbumArt();
  } else {
    if (s_udpStarted) {
      s_udp.stop();
      s_udpStarted = false;
    }
    flushPendingOutbound();
  }
}

void sendMediaCommand(const char *cmd, uint32_t posMs) {
  if (cmd == nullptr || s_txMutex == nullptr)
    return;
  if (xSemaphoreTake(s_txMutex, pdMS_TO_TICKS(10)) == pdTRUE) {
    strncpy(s_pendingCmd, cmd, sizeof(s_pendingCmd) - 1);
    s_pendingCmd[sizeof(s_pendingCmd) - 1] = '\0';
    s_pendingCmdPosMs = posMs;
    s_hasPendingCmd = true;
    xSemaphoreGive(s_txMutex);
  }
}

void sendConfigAck(uint32_t version) {
  if (s_txMutex == nullptr)
    return;
  if (xSemaphoreTake(s_txMutex, pdMS_TO_TICKS(10)) == pdTRUE) {
    s_pendingAckVer = version;
    s_hasPendingAck = true;
    xSemaphoreGive(s_txMutex);
  }
}

} // namespace CompanionLink
