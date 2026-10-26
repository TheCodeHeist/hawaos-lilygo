#include "services/OtaService.h"
#include "core/Settings.h"
#include "services/CompanionLink.h"

#include <Arduino.h>
#include <ArduinoJson.h>
#include <HTTPClient.h>
#include <Update.h>
#include <WiFi.h>
#include <mbedtls/md.h>
#include <mbedtls/sha256.h>
#include <string.h>

namespace OtaService {

namespace {

constexpr uint32_t kReadTimeoutMs = 8000;
constexpr uint32_t kFailedShowMs = 4000;
constexpr size_t kChunk = 4096;

portMUX_TYPE s_mux = portMUX_INITIALIZER_UNLOCKED;
Progress s_progress;
uint32_t s_failedAtMs = 0;

bool s_pending = false;
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
  char tokenHex[33] = {0};
  Settings::getCompanionToken(tokenHex, sizeof(tokenHex));
  if (!isHex(tokenHex, 32) || !isHex(macHex, 64))
    return false;
  uint8_t key[16];
  for (int i = 0; i < 16; ++i) {
    char byte[3] = {tokenHex[i * 2], tokenHex[i * 2 + 1], 0};
    key[i] = static_cast<uint8_t>(strtoul(byte, nullptr, 16));
  }
  char msg[80];
  const int msgLen = snprintf(msg, sizeof(msg), "%s:%lu", sha256Hex,
                              static_cast<unsigned long>(size));
  uint8_t mac[32];
  if (mbedtls_md_hmac(mbedtls_md_info_from_type(MBEDTLS_MD_SHA256), key,
                      sizeof(key), reinterpret_cast<const uint8_t *>(msg),
                      msgLen, mac) != 0) {
    return false;
  }
  char expected[65];
  toHex(mac, sizeof(mac), expected);
  // Constant-time compare.
  uint8_t diff = 0;
  for (int i = 0; i < 64; ++i) {
    diff |= static_cast<uint8_t>(tolower(macHex[i]) ^ expected[i]);
  }
  return diff == 0;
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
    s_pending = false;
    if (WiFi.status() != WL_CONNECTED) {
      fail("no_wifi");
    } else {
      run();
    }
    return;
  }
  if (progress().state == State::Failed &&
      millis() - s_failedAtMs >= kFailedShowMs) {
    setProgress(State::Idle, 0);
  }
}

} // namespace OtaService
