#include "services/CompanionLink.h"
#include "config.h"
#include "core/DataStore.h"
#include "core/EventBus.h"
#include "core/Settings.h"
#include "services/WifiService.h"
#include "ui/Theme.h"

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
  if (hexStr == nullptr) {
    return fallback;
  }
  while (*hexStr == ' ' || *hexStr == '\t' || *hexStr == '"' ||
         *hexStr == '\'') {
    ++hexStr;
  }
  if (*hexStr == '\0') {
    return fallback;
  }
  if (*hexStr == '#') {
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

void postNotificationArrived() {
  Event ev{};
  ev.type = EventType::NotificationArrived;
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
  Serial.println(jsonStr);
}

void handleJsonMessage(const char *payload, size_t len, bool viaUsb) {
  JsonDocument doc;
  const DeserializationError err = deserializeJson(doc, payload, len);
  if (err) {
    return;
  }

  s_lastRxViaUsb = viaUsb;

  const char *hostIpStr = doc["host_ip"] | "";
  if (hostIpStr[0] != '\0') {
    IPAddress hostIp;
    if (hostIp.fromString(hostIpStr)) {
      s_companionIp = hostIp;
      s_hasCompanionPeer = true;
    }
  }

  const char *type = doc["type"] | "";

  if (strcmp(type, "query") == 0) {
    sendConfigAck(Settings::getConfigVersion());
    return;
  }

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

    bool pushedNotify = false;
    if (doc["notify"].is<JsonArrayConst>()) {
      JsonArrayConst arr = doc["notify"].as<JsonArrayConst>();
      for (JsonObjectConst item : arr) {
        const char *app = item["app"] | "";
        const char *title = item["title"] | "";
        const char *body = item["body"] | "";
        const uint32_t nts = item["ts"] | ts;
        if (title[0] != '\0' || app[0] != '\0') {
          const char *a = (app[0] != '\0') ? app : ((title[0] != '\0') ? title : "Notification");
          const char *t = (title[0] != '\0') ? title : a;
          DataStore::pushNotification(a, t, body, nts);
          pushedNotify = true;
        }
      }
    }

    if (pushedNotify) {
      postNotificationArrived();
    } else {
      postDataUpdated();
    }
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

    const char *bgTopStr = doc["bg_top"].is<const char *>()
                               ? doc["bg_top"].as<const char *>()
                               : nullptr;
    const char *bgBotStr = doc["bg_bottom"].is<const char *>()
                               ? doc["bg_bottom"].as<const char *>()
                               : nullptr;
    const uint32_t bgTop = parseHexColor(bgTopStr, Theme::kLegacyMediaTint);
    const uint32_t bgBot = parseHexColor(bgBotStr, Theme::kLegacyMediaBottom);

    Serial.printf(
        "[CompanionLink] Media RX: '%s' | bgTop=0x%06lX (raw='%s') | "
        "bgBot=0x%06lX (raw='%s')\n",
        title, static_cast<unsigned long>(bgTop), bgTopStr ? bgTopStr : "null",
        static_cast<unsigned long>(bgBot), bgBotStr ? bgBotStr : "null");

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

  if (strcmp(type, "weather") == 0) {
    WeatherSnapshot ws{};
    const char *cityStr = doc["city"] | "";
    strncpy(ws.city, cityStr, sizeof(ws.city) - 1);
    ws.temp = doc["temp"] | 0;
    ws.feelsLike = doc["feels_like"] | ws.temp;
    ws.tempMin = doc["temp_min"] | ws.temp;
    ws.tempMax = doc["temp_max"] | ws.temp;
    ws.humidity = doc["humidity"] | 0;
    ws.windSpeed = doc["wind_speed"] | 0;
    const char *unitStr = doc["unit"] | "c";
    strncpy(ws.unit, (unitStr[0] == 'f' || unitStr[0] == 'F') ? "F" : "C",
            sizeof(ws.unit) - 1);
    const char *condStr = doc["condition"] | "Clear";
    strncpy(ws.condition, condStr, sizeof(ws.condition) - 1);
    const char *iconStr = doc["icon"] | "cloud-sun";
    strncpy(ws.icon, iconStr, sizeof(ws.icon) - 1);

    if (doc["forecast"].is<JsonArrayConst>()) {
      JsonArrayConst arr = doc["forecast"].as<JsonArrayConst>();
      uint8_t idx = 0;
      for (JsonObjectConst item : arr) {
        if (idx >= WeatherSnapshot::FORECAST_DAYS)
          break;
        const char *day = item["day"] | "";
        const char *fcIcon = item["icon"] | "cloud-sun";
        strncpy(ws.forecast[idx].day, day, sizeof(ws.forecast[idx].day) - 1);
        strncpy(ws.forecast[idx].icon, fcIcon, sizeof(ws.forecast[idx].icon) - 1);
        ws.forecast[idx].tempMin = item["min"] | 0;
        ws.forecast[idx].tempMax = item["max"] | 0;
        ++idx;
      }
    }

    Serial.printf("[CompanionLink] Weather RX: %s %d°%s (%s)\n", ws.city,
                  ws.temp, ws.unit, ws.condition);
    DataStore::updateWeather(ws);
    postDataUpdated();
    return;
  }

  if (strcmp(type, "clock") == 0) {
    if (doc["action"].is<const char *>()) {
      const char *act = doc["action"];
      if (strcmp(act, "stopwatch_start") == 0) {
        DataStore::startStopwatch();
      } else if (strcmp(act, "stopwatch_pause") == 0) {
        DataStore::pauseStopwatch();
      } else if (strcmp(act, "stopwatch_reset") == 0) {
        DataStore::resetStopwatch();
      } else if (strcmp(act, "stopwatch_toggle") == 0) {
        DataStore::toggleStopwatch();
      } else if (strcmp(act, "timer_start") == 0) {
        if (doc["duration_ms"].is<uint32_t>() && doc["duration_ms"].as<uint32_t>() > 0) {
          DataStore::startTimerMs(doc["duration_ms"].as<uint32_t>());
        } else {
          const uint32_t dur = doc["duration"] | 300U;
          DataStore::startTimer(dur);
        }
      } else if (strcmp(act, "timer_pause") == 0) {
        DataStore::pauseTimer();
      } else if (strcmp(act, "timer_resume") == 0) {
        DataStore::resumeTimer();
      } else if (strcmp(act, "timer_reset") == 0) {
        DataStore::resetTimer();
      } else if (strcmp(act, "timer_toggle") == 0) {
        DataStore::toggleTimer();
      }
    } else {
      StopwatchState swState = StopwatchState::Stopped;
      const char *swStr = doc["stopwatch_state"] | "stopped";
      if (strcmp(swStr, "running") == 0) swState = StopwatchState::Running;
      else if (strcmp(swStr, "paused") == 0) swState = StopwatchState::Paused;
      const uint32_t swMs = doc["stopwatch_ms"] | 0U;

      TimerState tmState = TimerState::Stopped;
      const char *tmStr = doc["timer_state"] | "stopped";
      if (strcmp(tmStr, "running") == 0) tmState = TimerState::Running;
      else if (strcmp(tmStr, "paused") == 0) tmState = TimerState::Paused;
      else if (strcmp(tmStr, "finished") == 0) tmState = TimerState::Finished;
      const uint32_t tmDur = doc["timer_duration_sec"] | 300U;
      const uint32_t tmRem = doc["timer_remaining_sec"] | tmDur;

      DataStore::syncClock(swState, swMs, tmState, tmDur, tmRem);
    }
    postDataUpdated();
    return;
  }

  if (strcmp(type, "notify") == 0) {
    const char *app = doc["app"] | "";
    const char *title = doc["title"] | "";
    const char *body = doc["body"] | "";
    const uint32_t ts = doc["ts"] | static_cast<uint32_t>(millis() / 1000);
    const char *a = (app[0] != '\0') ? app : ((title[0] != '\0') ? title : "Notification");
    const char *t = (title[0] != '\0') ? title : a;
    Serial.printf("[CompanionLink] Notify RX via %s: app='%s', title='%s', body='%s', ts=%lu\n",
                  viaUsb ? "USB" : "UDP", a, t, body, static_cast<unsigned long>(ts));
    DataStore::pushNotification(a, t, body, ts);
    postNotificationArrived();
    return;
  }

  if (strcmp(type, "config") == 0) {
    const uint32_t v = doc["v"] | 0U;

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

    if (doc["accent"].is<const char *>()) {
      if (!Settings::setAccentById(doc["accent"].as<const char *>())) {
        Serial.printf("[CompanionLink] Unknown accent '%s', keeping '%s'\n",
                      doc["accent"].as<const char *>(), Settings::getAccentId());
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
    if (doc["notify_popup"].is<bool>()) {
      Settings::setNotifyPopupEnabled(doc["notify_popup"].as<bool>());
    }
    if (doc["orientation"].is<const char *>()) {
      const char *orient = doc["orientation"];
      Settings::setOrientationInverted(strcmp(orient, "inverted") == 0);
    } else if (doc["orientation_inverted"].is<bool>()) {
      Settings::setOrientationInverted(doc["orientation_inverted"].as<bool>());
    }
    if (doc["forget_wifi"].is<bool>() && doc["forget_wifi"].as<bool>()) {
      WifiService::forgetWifi();
    } else if (doc["wifi_ssid"].is<const char *>() &&
               doc["wifi_pass"].is<const char *>()) {
      const char *newSsid = doc["wifi_ssid"];
      const char *newPass = doc["wifi_pass"];
      if (newSsid[0] != '\0') {
        WifiService::reconfigure(newSsid, newPass);
      }
    }

    const uint32_t curVer = Settings::getConfigVersion();
    const uint32_t nextVer = (v > curVer) ? v : (curVer + 1);
    Settings::setConfigVersion(nextVer);
    sendConfigAck(nextVer);
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

  const uint32_t nowMs = millis();
  if (!s_hasCompanionPeer || WiFi.status() != WL_CONNECTED) {
    static uint32_t s_lastArtDiagMs = 0;
    if (nowMs - s_lastArtDiagMs >= 3000) {
      s_lastArtDiagMs = nowMs;
      Serial.printf(
          "[CompanionLink] Pending art '%s' waiting (wifi=%d, peer=%d, "
          "companionIp=%s)\n",
          s_pendingArtHash, WiFi.status() == WL_CONNECTED ? 1 : 0,
          s_hasCompanionPeer ? 1 : 0, s_companionIp.toString().c_str());
    }
    return;
  }

  if (nowMs - s_lastArtAttemptMs < 1200)
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
    Serial.printf("[CompanionLink] Art HTTP begin failed: %s\n", url);
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
    } else {
      Serial.printf("[CompanionLink] Art read truncated (%u / %u bytes)\n",
                    static_cast<unsigned>(totalRead),
                    static_cast<unsigned>(DataStore::ALBUM_ART_BYTES));
    }
  } else {
    Serial.printf("[CompanionLink] Art GET failed: %s code=%d size=%d\n", url,
                  code, http.getSize());
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
      strncpy(cmdLocal, s_pendingCmd, sizeof(s_pendingCmd) - 1);
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
    const char *themeStr = "dark";
    const ThemeMode tm = Settings::getThemeMode();
    if (tm == ThemeMode::Light) {
      themeStr = "light";
    } else if (tm == ThemeMode::Auto) {
      themeStr = "auto";
    }
    const char *wpStr = (Settings::getWallpaperMode() == WallpaperMode::Solid)
                            ? "solid"
                            : "gradient";
    const String ipStr = WifiService::isConnected() ? WiFi.localIP().toString()
                                                    : String("0.0.0.0");
    char ssidBuf[33] = {0};
    Settings::getWifiSsid(ssidBuf, sizeof(ssidBuf));

    const char *orientStr =
        Settings::getOrientationInverted() ? "inverted" : "normal";

    char buf[448];
    snprintf(buf, sizeof(buf),
             "{\"type\":\"config_ack\",\"v\":%lu,\"ip\":\"%s\",\"wifi\":%s,"
             "\"ssid\":\"%s\",\"theme\":\"%s\",\"accent\":\"%s\","
             "\"brightness\":%u,\"aod\":%s,"
             "\"aod_timeout_s\":%u,\"reduce_motion\":%s,\"wallpaper\":\"%s\","
             "\"use_24h\":%s,\"notify_popup\":%s,\"orientation\":\"%s\"}",
             static_cast<unsigned long>(ackVer), ipStr.c_str(),
             WifiService::isConnected() ? "true" : "false", ssidBuf, themeStr,
             Settings::getAccentId(),
             static_cast<unsigned>(Settings::getBrightnessPct()),
             Settings::getAodEnabled() ? "true" : "false",
             static_cast<unsigned>(Settings::getAodTimeoutSec()),
             Settings::getReduceMotion() ? "true" : "false", wpStr,
             Settings::getUse24Hour() ? "true" : "false",
             Settings::getNotifyPopupEnabled() ? "true" : "false",
             orientStr);
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
        } else if (s_serialLen == 1) {
          const char ch = s_serialLine[0];
          if (ch == 't' || ch == 'T') {
            Event ev{};
            ev.type = EventType::RunStressTest;
            ev.stressIterations = 200;
            EventBus::post(ev);
          } else if (ch == 'r' || ch == 'R') {
            WifiService::forceReconnect();
          }
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

void sendClockCommand(const char *cmd) {
  if (cmd == nullptr)
    return;
  char buf[96];
  snprintf(buf, sizeof(buf), "{\"type\":\"clock_cmd\",\"cmd\":\"%s\"}", cmd);
  sendJsonPacket(buf);
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

bool hasPeer() { return s_hasCompanionPeer || s_lastRxViaUsb; }

} // namespace CompanionLink
