#include "services/CompanionLink.h"
#include "config.h"
#include "core/DataStore.h"
#include "core/EventBus.h"
#include "core/Settings.h"
#include "services/BleLink.h"
#include "services/CompanionAuth.h"
#include "services/FirmwareHealth.h"
#include <mbedtls/base64.h>
#include "services/NtpService.h"
#include "services/OtaService.h"
#include "services/SetupService.h"
#include "services/GlyphCache.h"
#include "services/Wallpaper.h"
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
// Sender of the UDP packet being handled; becomes the peer only once
// trusted (sealed, or no token yet).
IPAddress s_rxIp;
uint16_t s_rxPort = 0;

char s_rxBuf[1460] = {0};
char s_serialLine[1024] = {0};
char s_bleLine[1460] = {0};
size_t s_serialLen = 0;

// A blob the companion serves by hash: album art, the wallpaper or a glyph
// pack. Pulled over HTTP on Wi-Fi, else pushed in <kind>_data chunks
// (Bluetooth, USB).
struct ImageSlot {
  const char *kind;  // "art" | "wp" | "gl": URL path, message prefix and AAD
  const char *label; // for the log
  size_t bytes;      // the buffer's size
  uint8_t *(*buffer)();                       // where the bytes land
  void (*done)(const char *hash, size_t len); // called once authentic
  uint32_t pullTimeoutMs;
  size_t len; // this transfer's size: `bytes`, or less for a glyph pack
  char pending[16];
  char loaded[16]; // what the buffer holds
  uint32_t lastAttemptMs;
  uint8_t failures; // gives up after a few (finishImage)
  // Pushed chunks: iv and tag land here, the ciphertext straight in the buffer.
  size_t pushGot;
  uint8_t iv[12];
  uint8_t tag[16];
};
// Last sealed message over the everyday Bluetooth link, and last stats over
// USB (isLinked()).
volatile uint32_t s_lastBleRxMs = 0;
volatile uint32_t s_lastUsbStatsMs = 0;
// A link-mode change applies shortly after its config_ack goes out.
uint32_t s_linkApplyAtMs = 0;

SemaphoreHandle_t s_txMutex = nullptr;
char s_pendingCmd[24] = {0};
// Clock commands come from the UI task; queued so only the WiFi task sends.
char s_pendingClockCmd[24] = {0};
bool s_hasPendingClockCmd = false;
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
  // Encrypted once paired (DESIGN.md Section 11.6): UDP always, BLE outside
  // setup (the setup protocol has its own pairing). USB stays plain.
  const bool paired = CompanionAuth::hasToken();
  const bool udp = s_hasCompanionPeer && WiFi.status() == WL_CONNECTED;
  const bool ble = BleLink::isConnected();
  const bool sealBle = ble && paired && !SetupService::isActive();
  static char sealed[1460];
  const bool haveSealed = paired && (udp || sealBle) &&
                          CompanionAuth::seal(jsonStr, sealed, sizeof(sealed));
  if (udp && (!paired || haveSealed)) {
    const char *pkt = paired ? sealed : jsonStr;
    s_udp.beginPacket(s_companionIp, s_companionPort);
    s_udp.write(reinterpret_cast<const uint8_t *>(pkt), strlen(pkt));
    s_udp.endPacket();
  }
  Serial.println(jsonStr);
  if (ble && (!sealBle || haveSealed)) {
    BleLink::send(sealBle ? sealed : jsonStr);
  }
}

using SetupService::Link;

// Over BLE (setup only) a peer must prove it can read the pairing code before
// anything but discovery and pairing messages is accepted.
bool allowedOverBle(const char *type) {
  static const char *const kOpen[] = {"query", "wifi_scan", "setup",
                                      "setup_key", "setup_verify"};
  for (const char *t : kOpen) {
    if (strcmp(type, t) == 0)
      return true;
  }
  return SetupService::isSessionVerified() &&
         (strcmp(type, "wifi_creds") == 0 || strcmp(type, "config") == 0);
}

void reply(const JsonDocument &doc) {
  char buf[256];
  serializeJson(doc, buf, sizeof(buf));
  sendJsonPacket(buf);
}

// Over the network, setup only runs on USB or paired BLE; a token arriving
// over UDP could otherwise be replaced by anyone on the LAN.
bool setupOnly(const char *type) {
  static const char *const kSetup[] = {"setup", "setup_key", "setup_verify",
                                       "wifi_creds"};
  for (const char *t : kSetup) {
    if (strcmp(type, t) == 0)
      return true;
  }
  return false;
}

// Sealed messages that change the device save the replay counter to flash.
bool changesDevice(const char *type) {
  return strcmp(type, "config") == 0 || strcmp(type, "wifi_join") == 0 ||
         strcmp(type, "ota") == 0;
}

// Answered to the sender, not the trusted peer, so junk from elsewhere on
// the LAN can't flood the companion with errors.
void replyAuthError(Link link, const char *reason, const char *forType) {
  Serial.printf("[CompanionLink] %s: refused '%s' (%s)\n",
                link == Link::Ble ? "BLE" : "UDP", forType, reason);
  JsonDocument out;
  out["type"] = "auth_error";
  out["reason"] = reason;
  out["for"] = forType;
  char buf[128];
  const size_t n = serializeJson(out, buf, sizeof(buf));
  if (link == Link::Ble) {
    BleLink::send(buf);
    return;
  }
  s_udp.beginPacket(s_rxIp, s_rxPort);
  s_udp.write(reinterpret_cast<const uint8_t *>(buf), n);
  s_udp.endPacket();
}

uint8_t *artBuffer() {
  return reinterpret_cast<uint8_t *>(DataStore::albumArtBuffer());
}

void artDone(const char *hash, size_t) {
  DataStore::setAlbumArtReady(hash, true);
  postDataUpdated();
}

void wallpaperDone(const char *hash, size_t) {
  Wallpaper::commit(hash);
  postSettingsChanged(); // the UI swaps it in and redraws
}

ImageSlot s_art{"art", "128x128 RGB565 art", DataStore::ALBUM_ART_BYTES,
                artBuffer, artDone, 1500};
ImageSlot s_wp{"wp", "320x170 wallpaper", Wallpaper::BYTES,
               Wallpaper::stagingBuffer, wallpaperDone, 4000};

void glyphsDone(const char *hash, size_t len) {
  GlyphCache::commit(hash, len);
  postDataUpdated(); // the UI adds them and redraws
}

ImageSlot s_gl{"gl", "glyph pack", GlyphCache::kPackBytes,
               GlyphCache::stagingBuffer, glyphsDone, 1500};

// Glyph packs wait here while another downloads: a notification's and the
// now-playing title's can arrive together.
struct GlyphWant {
  char hash[16];
  size_t len;
};
GlyphWant s_glQueue[4] = {};
size_t s_glQueued = 0;

// Paired: images arrive as iv || ciphertext || tag (DESIGN.md Section 11.6).
size_t wireBytes(const ImageSlot &s) {
  return s.len + (CompanionAuth::hasToken() ? sizeof(s.iv) + sizeof(s.tag) : 0);
}

void wantImage(ImageSlot &s, const char *hash, size_t len = 0) {
  if (hash[0] == '\0') {
    s.pending[0] = '\0';
  } else if (strncmp(hash, s.pending, sizeof(s.pending)) != 0) {
    s.failures = 0;
    s.len = (len != 0) ? len : s.bytes;
    strncpy(s.pending, hash, sizeof(s.pending) - 1);
    s.pending[sizeof(s.pending) - 1] = '\0';
  }
}

// A message's text needs glyph pack `gl` (gl_len bytes) unless it's cached.
void wantGlyphs(JsonDocument &doc) {
  const char *hash = doc["gl"] | "";
  const size_t len = doc["gl_len"] | 0U;
  if (hash[0] == '\0' || len == 0 || len > GlyphCache::kPackBytes ||
      strlen(hash) >= sizeof(s_gl.pending) || GlyphCache::hasPack(hash) ||
      strcmp(hash, s_gl.pending) == 0) {
    return;
  }
  for (size_t i = 0; i < s_glQueued; ++i) {
    if (strcmp(s_glQueue[i].hash, hash) == 0) {
      return;
    }
  }
  if (s_gl.pending[0] == '\0') {
    wantImage(s_gl, hash, len);
    return;
  }
  if (s_glQueued == sizeof(s_glQueue) / sizeof(s_glQueue[0])) {
    memmove(&s_glQueue[0], &s_glQueue[1], sizeof(s_glQueue) - sizeof(s_glQueue[0]));
    --s_glQueued;
  }
  strcpy(s_glQueue[s_glQueued].hash, hash);
  s_glQueue[s_glQueued++].len = len;
}

// Decrypts the buffer if paired and publishes it, or forgets it.
void finishImage(ImageSlot &s, bool complete, const uint8_t *iv,
                 const uint8_t *tag) {
  uint8_t *buf = s.buffer();
  bool authentic = complete && buf != nullptr;
  if (authentic && CompanionAuth::hasToken()) {
    char aad[24];
    snprintf(aad, sizeof(aad), "%s:%s", s.kind, s.pending);
    authentic = CompanionAuth::openInPlace(aad, iv, buf, s.len, tag);
  }
  if (authentic) {
    s.failures = 0;
    strncpy(s.loaded, s.pending, sizeof(s.loaded) - 1);
    s.loaded[sizeof(s.loaded) - 1] = '\0';
    s.pending[0] = '\0';
    Serial.printf("[CompanionLink] Pulled %s (%s)\n", s.label, s.loaded);
    s.done(s.loaded, s.len);
  } else {
    // The buffer no longer holds what it was loaded with.
    s.loaded[0] = '\0';
    Serial.printf("[CompanionLink] %s %s (%s)\n", s.kind,
                  complete ? "failed authentication" : "read truncated",
                  s.pending);
    // A key mismatch fails the same way every time: stop asking.
    if (++s.failures >= 3) {
      s.failures = 0;
      s.pending[0] = '\0';
      Serial.printf("[CompanionLink] Giving up on this %s\n", s.kind);
    }
  }
}

// <kind>_data {hash, off, b64}: in-order pieces of the wire blob, pushed where
// the device can't pull over HTTP (<kind>_req).
void handleImageData(ImageSlot &s, JsonDocument &doc) {
  uint8_t *buf = s.buffer();
  if (buf == nullptr || s.pending[0] == '\0' ||
      strcmp(doc["hash"] | "", s.pending) != 0) {
    return;
  }
  const size_t off = doc["off"] | 0U;
  if (off == 0) {
    s.pushGot = 0;
  } else if (off != s.pushGot) {
    s.pushGot = 0; // a piece went missing; the next request starts over
    return;
  }
  const char *b64 = doc["b64"] | "";
  uint8_t chunk[768];
  size_t n = 0;
  const size_t wire = wireBytes(s);
  if (mbedtls_base64_decode(chunk, sizeof(chunk), &n,
                            reinterpret_cast<const uint8_t *>(b64),
                            strlen(b64)) != 0 ||
      off + n > wire) {
    s.pushGot = 0;
    return;
  }
  const size_t ivLen = CompanionAuth::hasToken() ? sizeof(s.iv) : 0;
  for (size_t i = 0; i < n; ++i) {
    const size_t at = off + i;
    if (at < ivLen) {
      s.iv[at] = chunk[i];
    } else if (at < ivLen + s.len) {
      buf[at - ivLen] = chunk[i];
    } else {
      s.tag[at - ivLen - s.len] = chunk[i];
    }
  }
  s.pushGot = off + n;
  s.lastAttemptMs = millis(); // progress: don't ask again mid-transfer
  if (s.pushGot == wire) {
    s.pushGot = 0;
    finishImage(s, true, s.iv, s.tag);
  }
}

// First-run setup messages (DESIGN.md Section 11.4). Returns true if handled.
bool handleSetupMessage(const char *type, JsonDocument &doc, Link link,
                        bool trusted) {
  // Plain BLE is the setup session: no WiFi or firmware over it.
  const bool plainBle = (link == Link::Ble && !trusted);
  if (strcmp(type, "wifi_scan") == 0) {
    WifiService::requestScan();
    return true;
  }
  if (strcmp(type, "setup") == 0) {
    const char *stage = doc["stage"] | "";
    if (strcmp(stage, "begin") == 0) {
      SetupService::onCompanionAttached(link, doc["host"] | "");
    } else if (strcmp(stage, "done") == 0) {
      // The companion's secret for authorizing network firmware updates:
      // plain over USB, sealed with the pairing key over BLE.
      char token[48] = {0};
      if (link == Link::Ble) {
        SetupService::decrypt(doc["token_nonce"] | "", doc["token_ct"] | "",
                              token, sizeof(token));
      } else {
        strncpy(token, doc["token"] | "", sizeof(token) - 1);
      }
      if (strlen(token) == 32) {
        CompanionAuth::setToken(token);
      }
      memset(token, 0, sizeof(token));
      SetupService::finish();
      sendConfigAck(Settings::getConfigVersion());
    }
    return true;
  }
  if (strcmp(type, "setup_key") == 0) {
    char ourPub[48] = {0};
    JsonDocument out;
    out["type"] = "setup_key";
    if (SetupService::exchangeKey(doc["pub"] | "", ourPub, sizeof(ourPub))) {
      out["pub"] = ourPub;
    } else {
      out["error"] = "bad_key";
    }
    reply(out);
    return true;
  }
  if (strcmp(type, "setup_verify") == 0) {
    uint8_t attemptsLeft = 0;
    const bool ok = SetupService::verify(doc["nonce"] | "", doc["ct"] | "",
                                         &attemptsLeft);
    JsonDocument out;
    out["type"] = "setup_verify";
    out["ok"] = ok;
    out["attempts_left"] = attemptsLeft;
    reply(out);
    return true;
  }
  if (strcmp(type, "wifi_creds") == 0) {
    // Encrypted {"ssid","pass"} (BLE); USB/UDP use plain wifi_join instead.
    char plain[160];
    JsonDocument creds;
    if (SetupService::decrypt(doc["nonce"] | "", doc["ct"] | "", plain,
                              sizeof(plain)) &&
        !deserializeJson(creds, plain)) {
      const char *ssid = creds["ssid"] | "";
      if (ssid[0] != '\0') {
        WifiService::reconfigure(ssid, creds["pass"] | "");
      }
    } else {
      JsonDocument out;
      out["type"] = "wifi_status";
      out["state"] = "failed";
      out["reason"] = "decrypt";
      reply(out);
    }
    memset(plain, 0, sizeof(plain));
    return true;
  }
  if (strcmp(type, "ota") == 0 && !plainBle) {
    const char *sha = doc["sha256"] | "";
    const uint32_t size = doc["size"] | 0U;
    const char *err = nullptr;
    // USB means someone is physically here; over the network the request
    // must be signed with the token from setup.
    if (link == Link::Udp && !OtaService::checkMac(sha, size, doc["mac"] | "")) {
      err = "unauthorized";
    } else if (FirmwareHealth::onTrial()) {
      // Writing now would overwrite the firmware a rollback goes back to.
      err = "verifying";
    } else if (!OtaService::start(doc["url"] | "", size, sha)) {
      err = "rejected";
    }
    if (err != nullptr) {
      JsonDocument out;
      out["type"] = "ota_status";
      out["state"] = "failed";
      out["error"] = err;
      reply(out);
    }
    return true;
  }
  if (strcmp(type, "wifi_join") == 0 && !plainBle) {
    const char *ssid = doc["ssid"] | "";
    if (ssid[0] != '\0') {
      WifiService::reconfigure(ssid, doc["pass"] | "");
    }
    return true;
  }
  return false;
}

void handleJsonMessage(const char *payload, size_t len, Link link,
                       bool trusted = false) {
  const bool viaUsb = (link == Link::Usb);
  JsonDocument doc;
  const DeserializationError err = deserializeJson(doc, payload, len);
  if (err) {
    return;
  }

  const char *type = doc["type"] | "";
  // Everyday Bluetooth (DESIGN.md Section 11.7) follows the UDP rules.
  const bool bleEveryday = link == Link::Ble && !SetupService::isActive() &&
                           CompanionAuth::hasToken();
  if (!trusted && (link == Link::Udp || bleEveryday) &&
      strcmp(type, "sealed") == 0) {
    // With a token, network traffic is encrypted (DESIGN.md Section 11.6).
    static char plain[1200];
    switch (CompanionAuth::open(doc["n"] | 0ULL, doc["iv"] | "",
                                doc["ct"] | "", plain, sizeof(plain))) {
    case CompanionAuth::Result::Ok:
      handleJsonMessage(plain, strlen(plain), link, true);
      break;
    case CompanionAuth::Result::Stale:
      replyAuthError(link, "stale", type);
      break;
    case CompanionAuth::Result::BadMac:
      replyAuthError(link, "bad_mac", type);
      break;
    }
    return;
  }
  if (link == Link::Ble) {
    if (bleEveryday) {
      if (setupOnly(type)) {
        replyAuthError(link, "setup_only", type);
        return;
      }
      // Image chunks are pieces of an encrypted blob, checked when complete.
      if (!trusted && strcmp(type, "art_data") != 0 &&
          strcmp(type, "wp_data") != 0 && strcmp(type, "gl_data") != 0) {
        replyAuthError(link, "plaintext", type);
        return;
      }
    } else if (!allowedOverBle(type)) {
      Serial.printf("[CompanionLink] BLE: ignored '%s' (not paired)\n", type);
      return;
    }
  }
  if (trusted && changesDevice(type)) {
    CompanionAuth::persist();
  }
  if (trusted && link == Link::Ble) {
    s_lastBleRxMs = millis() | 1;
  }
  if (viaUsb && strcmp(type, "stats") == 0) {
    s_lastUsbStatsMs = millis() | 1;
  }
  if (link == Link::Udp) {
    if (setupOnly(type)) {
      replyAuthError(link, "setup_only", type);
      return;
    }
    if (!trusted && CompanionAuth::hasToken()) {
      replyAuthError(link, "plaintext", type);
      return;
    }
    // Trusted from here: this is where replies and media commands go.
    s_companionIp = s_rxIp;
    s_companionPort = s_rxPort;
    s_hasCompanionPeer = true;
  }

  s_lastRxViaUsb = viaUsb;
  if (trusted || viaUsb) {
    FirmwareHealth::companionHeard(); // sealed, or someone at the cable
  }

  const char *hostIpStr = doc["host_ip"] | "";
  if (hostIpStr[0] != '\0') {
    IPAddress hostIp;
    if (hostIp.fromString(hostIpStr)) {
      s_companionIp = hostIp;
      s_hasCompanionPeer = true;
    }
  }

  if (handleSetupMessage(type, doc, link, trusted)) {
    return;
  }

  if (strcmp(type, "query") == 0) {
    sendConfigAck(Settings::getConfigVersion());
    return;
  }

  if (strcmp(type, "art_data") == 0) {
    handleImageData(s_art, doc);
    return;
  }
  if (strcmp(type, "wp_data") == 0) {
    handleImageData(s_wp, doc);
    return;
  }
  if (strcmp(type, "gl_data") == 0) {
    handleImageData(s_gl, doc);
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
    NtpService::setFromCompanion(ts);

    // `lock` only while the computer is locked; `mic` (in_use, muted)
    // when it has one.
    const char *lock = doc["lock"] | "";
    JsonObjectConst mic = doc["mic"].as<JsonObjectConst>();
    DataStore::updatePc(strcmp(lock, "aod") == 0   ? PcLock::Aod
                        : strcmp(lock, "off") == 0 ? PcLock::ScreenOff
                                                   : PcLock::Unlocked,
                        !mic.isNull(), mic["in_use"] | false,
                        mic["muted"] | false);

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
    wantGlyphs(doc);

    if (artHash[0] != '\0' &&
        strncmp(artHash, s_art.loaded, sizeof(s_art.loaded)) == 0) {
      DataStore::setAlbumArtReady(artHash, true);
      s_art.pending[0] = '\0';
    } else {
      wantImage(s_art, artHash);
    }

    postDataUpdated();
    return;
  }

  // Synced lyrics: the line the song is at (DESIGN.md 9.6).
  if (strcmp(type, "lyric") == 0) {
    DataStore::updateLyric(doc["on"] | false, doc["line"] | "");
    wantGlyphs(doc);
    postDataUpdated();
    return;
  }

  if (strcmp(type, "weather") == 0) {
    WeatherSnapshot ws{};
    const char *cityStr = doc["city"] | "";
    DataStore::copyText(ws.city, cityStr);
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
    DataStore::copyText(ws.condition, condStr);
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
    wantGlyphs(doc);
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
    wantGlyphs(doc);
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
    if (doc["aod_off_min"].is<uint16_t>()) {
      Settings::setAodOffMin(doc["aod_off_min"].as<uint16_t>());
    }
    if (doc["reduce_motion"].is<bool>()) {
      Settings::setReduceMotion(doc["reduce_motion"].as<bool>());
    }
    if (doc["wallpaper"].is<const char *>()) {
      const char *wp = doc["wallpaper"];
      if (strcmp(wp, "image") == 0) {
        Settings::setWallpaperMode(WallpaperMode::Image);
        // Fetched unless it's the saved one; gradient until it arrives.
        const char *hash = doc["wallpaper_hash"] | "";
        wantImage(s_wp, strcmp(hash, Wallpaper::storedHash()) == 0 ? "" : hash);
      } else {
        Settings::setWallpaperMode(strcmp(wp, "solid") == 0
                                       ? WallpaperMode::Solid
                                       : WallpaperMode::Gradient);
      }
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
    if (link == Link::Ble && !trusted) {
      // WiFi over the BLE setup session only travels as wifi_creds.
    } else if (doc["forget_wifi"].is<bool>() && doc["forget_wifi"].as<bool>()) {
      WifiService::forgetWifi();
    } else if (doc["wifi_ssid"].is<const char *>() &&
               doc["wifi_pass"].is<const char *>()) {
      const char *newSsid = doc["wifi_ssid"];
      const char *newPass = doc["wifi_pass"];
      if (newSsid[0] != '\0') {
        WifiService::reconfigure(newSsid, newPass);
      }
    }

    // Link mode (DESIGN.md Section 11.7). Bluetooth needs the token: its
    // everyday traffic is sealed with it.
    if (doc["link"].is<const char *>()) {
      const LinkMode want = strcmp(doc["link"], "ble") == 0 ? LinkMode::Ble
                                                            : LinkMode::Wifi;
      // Wi-Fi mode without a saved network would strand the device (USB only).
      char ssid[33] = {0};
      Settings::getWifiSsid(ssid, sizeof(ssid));
      const bool ok = want == LinkMode::Wifi ? ssid[0] != '\0'
                                             : CompanionAuth::hasToken();
      if (want != Settings::getLinkMode() && ok) {
        Settings::setLinkMode(want);
        s_linkApplyAtMs = millis() | 1;
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

void fetchPending(ImageSlot &s) {
  if (s.pending[0] == '\0')
    return;
  if ((&s == &s_wp && !Wallpaper::canStage()) ||
      (&s == &s_gl && !GlyphCache::canStage()))
    return; // the UI hasn't taken the last one yet

  const uint32_t nowMs = millis();
  // Without HTTP (Bluetooth or USB), ask the companion to push it.
  const bool canPull = s_hasCompanionPeer && WiFi.status() == WL_CONNECTED;
  const bool canPush =
      (BleLink::isConnected() && !SetupService::isActive()) || s_lastRxViaUsb;
  if (!canPull && canPush) {
    if (nowMs - s.lastAttemptMs < 5000)
      return;
    s.lastAttemptMs = nowMs;
    s.pushGot = 0;
    char req[64];
    snprintf(req, sizeof(req), "{\"type\":\"%s_req\",\"hash\":\"%s\"}",
             s.kind, s.pending);
    sendJsonPacket(req);
    return;
  }
  if (!canPull) {
    static uint32_t s_lastDiagMs = 0;
    if (nowMs - s_lastDiagMs >= 3000) {
      s_lastDiagMs = nowMs;
      Serial.printf(
          "[CompanionLink] Pending %s '%s' waiting (wifi=%d, peer=%d, "
          "companionIp=%s)\n",
          s.kind, s.pending, WiFi.status() == WL_CONNECTED ? 1 : 0,
          s_hasCompanionPeer ? 1 : 0, s_companionIp.toString().c_str());
    }
    return;
  }

  if (nowMs - s.lastAttemptMs < 1200)
    return;
  s.lastAttemptMs = nowMs;

  uint8_t *dst = s.buffer();
  if (dst == nullptr)
    return;

  char url[96];
  snprintf(url, sizeof(url), "http://%s:%u/%s/%s",
           s_companionIp.toString().c_str(),
           static_cast<unsigned>(Config::COMPANION_ART_PORT), s.kind,
           s.pending);

  HTTPClient http;
  http.setTimeout(1500);
  if (!http.begin(url)) {
    Serial.printf("[CompanionLink] HTTP begin failed: %s\n", url);
    return;
  }

  // Paired: iv || ciphertext || tag, bound to the hash (DESIGN.md 11.6).
  const bool sealed = CompanionAuth::hasToken();
  constexpr size_t kIv = 12;
  constexpr size_t kTag = 16;

  const int code = http.GET();
  if (code == HTTP_CODE_OK &&
      http.getSize() == static_cast<int>(wireBytes(s))) {
    WiFiClient *stream = http.getStreamPtr();
    const uint32_t startMs = millis();
    auto readExact = [&](uint8_t *out, size_t want) -> size_t {
      size_t got = 0;
      while (http.connected() && got < want &&
             (millis() - startMs) < s.pullTimeoutMs) {
        const size_t avail = stream->available();
        if (avail > 0) {
          const int r = stream->readBytes(out + got, min(avail, want - got));
          if (r > 0)
            got += static_cast<size_t>(r);
        } else {
          delay(1);
        }
      }
      return got;
    };

    uint8_t iv[kIv];
    uint8_t tag[kTag];
    const bool complete = (!sealed || readExact(iv, kIv) == kIv) &&
                          readExact(dst, s.len) == s.len &&
                          (!sealed || readExact(tag, kTag) == kTag);
    finishImage(s, complete, iv, tag);
  } else {
    Serial.printf("[CompanionLink] GET failed: %s code=%d size=%d\n", url,
                  code, http.getSize());
  }
  http.end();
}

// Glyphs first (small, and text is missing without them), then album art
// (changes with every track), then the wallpaper (rarely).
void fetchPendingImages() {
  while (s_gl.pending[0] == '\0' && s_glQueued > 0) {
    const GlyphWant next = s_glQueue[0];
    memmove(&s_glQueue[0], &s_glQueue[1], sizeof(s_glQueue) - sizeof(s_glQueue[0]));
    --s_glQueued;
    if (!GlyphCache::hasPack(next.hash)) {
      wantImage(s_gl, next.hash, next.len);
    }
  }
  fetchPending(s_gl);
  fetchPending(s_art);
  fetchPending(s_wp);
}

void flushPendingOutbound() {
  char cmdLocal[24] = {0};
  uint32_t posLocal = 0;
  bool sendCmd = false;
  uint32_t ackVer = 0;
  bool sendAck = false;
  char clockLocal[24] = {0};
  bool sendClock = false;

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
    if (s_hasPendingClockCmd) {
      strncpy(clockLocal, s_pendingClockCmd, sizeof(clockLocal) - 1);
      s_hasPendingClockCmd = false;
      sendClock = true;
    }
    xSemaphoreGive(s_txMutex);
  }

  if (sendClock) {
    char buf[96];
    snprintf(buf, sizeof(buf), "{\"type\":\"clock_cmd\",\"cmd\":\"%s\"}",
             clockLocal);
    sendJsonPacket(buf);
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
    const WallpaperMode wpMode = Settings::getWallpaperMode();
    const char *wpStr = wpMode == WallpaperMode::Solid   ? "solid"
                        : wpMode == WallpaperMode::Image ? "image"
                                                         : "gradient";
    const String ipStr = WifiService::isConnected() ? WiFi.localIP().toString()
                                                    : String("0.0.0.0");
    char ssidBuf[33] = {0};
    Settings::getWifiSsid(ssidBuf, sizeof(ssidBuf));

    const char *orientStr =
        Settings::getOrientationInverted() ? "inverted" : "normal";

    // A failed update went back to this firmware (DESIGN.md 11.5).
    char rolledBack[40] = {0};
    if (FirmwareHealth::rolledBackFrom()[0] != '\0') {
      snprintf(rolledBack, sizeof(rolledBack), ",\"rolled_back\":\"%s\"",
               FirmwareHealth::rolledBackFrom());
    }
    char buf[768];
    snprintf(buf, sizeof(buf),
             "{\"type\":\"config_ack\",\"id\":\"%s\",\"model\":\"%s\","
             "\"fw\":\"%s\",\"gl\":1,\"fmt\":1,\"setup\":%s,"
             "\"v\":%lu,\"ip\":\"%s\",\"wifi\":%s,"
             "\"ssid\":\"%s\",\"theme\":\"%s\",\"accent\":\"%s\","
             "\"brightness\":%u,\"aod\":%s,"
             "\"aod_timeout_s\":%u,\"aod_off_min\":%u,\"reduce_motion\":%s,\"wallpaper\":\"%s\","
             "\"wallpaper_hash\":\"%s\","
             "\"use_24h\":%s,\"notify_popup\":%s,\"orientation\":\"%s\","
             "\"link\":\"%s\"%s}",
             SetupService::deviceId(), Config::DEVICE_MODEL, Config::FW_VERSION,
             Settings::isSetupDone() ? "false" : "true",
             static_cast<unsigned long>(ackVer), ipStr.c_str(),
             WifiService::isConnected() ? "true" : "false", ssidBuf, themeStr,
             Settings::getAccentId(),
             static_cast<unsigned>(Settings::getBrightnessPct()),
             Settings::getAodEnabled() ? "true" : "false",
             static_cast<unsigned>(Settings::getAodTimeoutSec()),
             static_cast<unsigned>(Settings::getAodOffMin()),
             Settings::getReduceMotion() ? "true" : "false", wpStr,
             Wallpaper::storedHash(),
             Settings::getUse24Hour() ? "true" : "false",
             Settings::getNotifyPopupEnabled() ? "true" : "false",
             orientStr,
             Settings::getLinkMode() == LinkMode::Ble ? "ble" : "wifi",
             rolledBack);
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
          handleJsonMessage(s_serialLine, s_serialLen, Link::Usb);
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
  if (s_linkApplyAtMs != 0 && millis() - s_linkApplyAtMs >= 1500) {
    s_linkApplyAtMs = 0;
    SetupService::applyLinkMode();
    WifiService::applyLinkMode();
  }
  pollUsbSerial();
  while (BleLink::popLine(s_bleLine, sizeof(s_bleLine))) {
    if (s_bleLine[0] == '{') {
      handleJsonMessage(s_bleLine, strlen(s_bleLine), Link::Ble);
    }
  }

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
        s_rxIp = s_udp.remoteIP();
        s_rxPort = s_udp.remotePort();

        const int readLen =
            s_udp.read(s_rxBuf, static_cast<int>(sizeof(s_rxBuf) - 1));
        if (readLen > 0) {
          s_rxBuf[readLen] = '\0';
          handleJsonMessage(s_rxBuf, static_cast<size_t>(readLen), Link::Udp);
        }
        pktSize = s_udp.parsePacket();
      }
    }

    flushPendingOutbound();
    fetchPendingImages();
  } else {
    if (s_udpStarted) {
      s_udp.stop();
      s_udpStarted = false;
    }
    flushPendingOutbound();
    fetchPendingImages(); // pushed over Bluetooth or USB (<kind>_req)
  }
}

void sendJson(const char *json) { sendJsonPacket(json); }

void sendClockCommand(const char *cmd) {
  if (cmd == nullptr || s_txMutex == nullptr)
    return;
  if (xSemaphoreTake(s_txMutex, pdMS_TO_TICKS(10)) == pdTRUE) {
    strncpy(s_pendingClockCmd, cmd, sizeof(s_pendingClockCmd) - 1);
    s_pendingClockCmd[sizeof(s_pendingClockCmd) - 1] = '\0';
    s_hasPendingClockCmd = true;
    xSemaphoreGive(s_txMutex);
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

bool isLinked() {
  if (Settings::getLinkMode() != LinkMode::Ble)
    return true;
  // Stats arrive every second; a few missed means the companion is gone.
  // Stats over the USB cable count too.
  const uint32_t now = millis();
  const bool usb = s_lastUsbStatsMs != 0 && now - s_lastUsbStatsMs < 8000;
  const bool ble = BleLink::isConnected() && s_lastBleRxMs != 0 &&
                   now - s_lastBleRxMs < 8000;
  return usb || ble;
}

bool hasPeer() { return s_hasCompanionPeer || s_lastRxViaUsb; }

} // namespace CompanionLink
