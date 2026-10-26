#include "services/OtaService.h"
#include "services/CompanionAuth.h"
#include "core/Settings.h"
#include "services/CompanionLink.h"
#include "services/WifiService.h"

#include <Arduino.h>
#include <ArduinoJson.h>
#include <HTTPClient.h>
#include <Update.h>
#include <WiFi.h>
#include <mbedtls/sha256.h>
#include <string.h>

namespace OtaService {

namespace {

constexpr uint32_t kReadTimeoutMs = 8000;
constexpr uint32_t kFailedShowMs = 4000;
constexpr size_t kChunk = 4096;
// Bluetooth link mode: how long to wait for the saved network to come up.
constexpr uint32_t kWifiWaitMs = 25000;

portMUX_TYPE s_mux = portMUX_INITIALIZER_UNLOCKED;
Progress s_progress;
uint32_t s_failedAtMs = 0;

bool s_pending = false;
uint32_t s_wifiWaitSinceMs = 0; // 0 = not waiting
char s_url[160] = {0};
uint32_t s_size = 0;
char s_sha[65] = {0};
uint8_t s_buf[kChunk];

void setProgress(State state, uint8_t pct, const char *error = nullptr) {
  portENTER_CRITICAL(&s_mux);
  s_progress.state = state;
  s_progress.pct = pct;
  strncpy(s_progress.error, error != nullptr ? error : "",
          sizeof(s_progress.error) - 1);
  portEXIT_CRITICAL(&s_mux);
}

void report(const char *state, uint8_t pct, const char *error = nullptr) {
  JsonDocument doc;
  doc["type"] = "ota_status";
  doc["state"] = state;
  doc["pct"] = pct;
  if (error != nullptr)
    doc["error"] = error;
  char buf[128];
  serializeJson(doc, buf, sizeof(buf));
  CompanionLink::sendJson(buf);
}

void fail(const char *error) {
  Serial.printf("[OTA] Failed: %s\n", error);
  WifiService::holdForUpdate(false);
  if (Update.isRunning()) {
    Update.abort();
  }
  s_failedAtMs = millis();
  setProgress(State::Failed, 0, error);
  report("failed", 0, error);
}

bool isHex(const char *s, size_t len) {
  if (s == nullptr || strlen(s) != len)
    return false;
  for (size_t i = 0; i < len; ++i) {
    if (!isxdigit(static_cast<unsigned char>(s[i])))
      return false;
  }
  return true;
}

void toHex(const uint8_t *in, size_t n, char *out) {
  static const char *kDigits = "0123456789abcdef";
  for (size_t i = 0; i < n; ++i) {
    out[i * 2] = kDigits[in[i] >> 4];
    out[i * 2 + 1] = kDigits[in[i] & 0x0F];
  }
  out[n * 2] = '\0';
}

void run() {
  Serial.printf("[OTA] Downloading %lu bytes from %s\n",
                static_cast<unsigned long>(s_size), s_url);
  setProgress(State::Downloading, 0);
  report("downloading", 0);

  HTTPClient http;
  http.setTimeout(kReadTimeoutMs);
  if (!http.begin(s_url)) {
    fail("bad_url");
    return;
  }
  const int code = http.GET();
  if (code != HTTP_CODE_OK) {
    http.end();
    fail("http_error");
    return;
  }
  const int len = http.getSize();
  if (len > 0 && static_cast<uint32_t>(len) != s_size) {
    http.end();
    fail("size_mismatch");
    return;
  }
  if (!Update.begin(s_size, U_FLASH)) {
    http.end();
    fail("no_space");
    return;
  }

  mbedtls_sha256_context sha;
  mbedtls_sha256_init(&sha);
  mbedtls_sha256_starts_ret(&sha, 0);

  WiFiClient *stream = http.getStreamPtr();
  uint32_t got = 0;
  uint8_t lastPct = 0;
  uint32_t lastDataMs = millis();
  while (got < s_size) {
    const size_t avail = stream->available();
    if (avail == 0) {
      if (!http.connected() || millis() - lastDataMs > kReadTimeoutMs) {
        break;
      }
      vTaskDelay(pdMS_TO_TICKS(2));
      continue;
    }
    size_t want = avail < kChunk ? avail : kChunk;
    if (want > s_size - got)
      want = s_size - got;
    const int n = stream->readBytes(s_buf, want);
    if (n <= 0)
      continue;
    lastDataMs = millis();
    mbedtls_sha256_update_ret(&sha, s_buf, n);
    if (Update.write(s_buf, n) != static_cast<size_t>(n)) {
      mbedtls_sha256_free(&sha);
      http.end();
      fail("flash_write");
      return;
    }
    got += n;
    const uint8_t pct = static_cast<uint8_t>((got * 100ULL) / s_size);
    setProgress(State::Downloading, pct);
    if (pct >= lastPct + 5) {
      lastPct = pct;
      report("downloading", pct);
    }
  }
  http.end();

  uint8_t digest[32];
  mbedtls_sha256_finish_ret(&sha, digest);
  mbedtls_sha256_free(&sha);
  if (got != s_size) {
    fail("incomplete");
    return;
  }

  setProgress(State::Verifying, 100);
  report("verifying", 100);
  char hex[65];
  toHex(digest, sizeof(digest), hex);
  if (strcasecmp(hex, s_sha) != 0) {
    fail("checksum");
    return;
  }
  // Update.end() checks the image header and marks the new slot bootable.
  if (!Update.end(true)) {
    fail("bad_image");
    return;
  }

  setProgress(State::Done, 100);
  report("done", 100);
  Serial.println("[OTA] Update OK, restarting");
  delay(1200); // let the UI show "Restarting" and the status go out
  ESP.restart();
}

} // namespace

bool start(const char *url, uint32_t size, const char *sha256Hex) {
  // A failure still on screen doesn't block a retry.
  const State st = progress().state;
  if (s_pending || (st != State::Idle && st != State::Failed))
    return false;
  if (url == nullptr || strncmp(url, "http://", 7) != 0 ||
      strlen(url) >= sizeof(s_url) || size == 0 || !isHex(sha256Hex, 64)) {
    return false;
  }
  strncpy(s_url, url, sizeof(s_url) - 1);
  strncpy(s_sha, sha256Hex, sizeof(s_sha) - 1);
  s_size = size;
  s_pending = true;
  return true;
}

bool checkMac(const char *sha256Hex, uint32_t size, const char *macHex) {
  if (!isHex(sha256Hex, 64))
    return false;
  char msg[80];
  const int msgLen = snprintf(msg, sizeof(msg), "%s:%lu", sha256Hex,
                              static_cast<unsigned long>(size));
  return CompanionAuth::verify(msg, msgLen, macHex);
}

Progress progress() {
  portENTER_CRITICAL(&s_mux);
  const Progress p = s_progress;
  portEXIT_CRITICAL(&s_mux);
  return p;
}

bool isActive() { return progress().state != State::Idle; }

void poll() {
  if (s_pending) {
    if (WiFi.status() != WL_CONNECTED) {
      // Bluetooth mode keeps Wi-Fi off: bring up the saved network for the
      // download (DESIGN.md Section 11.7).
      char ssid[33] = {0};
      Settings::getWifiSsid(ssid, sizeof(ssid));
      if (Settings::getLinkMode() == LinkMode::Ble && ssid[0] != '\0') {
        if (s_wifiWaitSinceMs == 0) {
          s_wifiWaitSinceMs = millis() | 1;
          Serial.printf("[OTA] Turning on Wi-Fi ('%s') for the update\n", ssid);
          setProgress(State::Downloading, 0);
          report("downloading", 0);
          WifiService::holdForUpdate(true);
          return;
        }
        if (millis() - s_wifiWaitSinceMs < kWifiWaitMs)
          return;
      }
      s_pending = false;
      s_wifiWaitSinceMs = 0;
      fail("no_wifi");
      return;
    }
    s_pending = false;
    s_wifiWaitSinceMs = 0;
    run();
    return;
  }
  if (progress().state == State::Failed &&
      millis() - s_failedAtMs >= kFailedShowMs) {
    setProgress(State::Idle, 0);
  }
}

} // namespace OtaService
