#include "services/CompanionAuth.h"
#include "core/Settings.h"

#include <Arduino.h>
#include <esp_random.h>
#include <mbedtls/base64.h>
#include <mbedtls/gcm.h>
#include <mbedtls/md.h>
#include <mbedtls/sha256.h>
#include <string.h>

namespace CompanionAuth {

namespace {

constexpr size_t kIvLen = 12;
constexpr size_t kTagLen = 16;
constexpr size_t kMaxPlain = 1100; // one datagram once base64'd

bool s_loaded = false;
bool s_hasKey = false;
uint8_t s_token[16];
uint8_t s_keyIn[32];  // companion -> device
uint8_t s_keyOut[32]; // device -> companion
uint64_t s_lastN = 0;
uint32_t s_sent = 0; // per-boot counter for sealed replies
uint8_t s_buf[kMaxPlain + kTagLen];

bool isHex(const char *s, size_t len) {
  if (s == nullptr || strlen(s) != len)
    return false;
  for (size_t i = 0; i < len; ++i) {
    if (!isxdigit(static_cast<unsigned char>(s[i])))
      return false;
  }
  return true;
}

void deriveKey(const char *label, uint8_t out[32]) {
  mbedtls_sha256_context sha;
  mbedtls_sha256_init(&sha);
  mbedtls_sha256_starts_ret(&sha, 0);
  mbedtls_sha256_update_ret(&sha, s_token, sizeof(s_token));
  mbedtls_sha256_update_ret(&sha, reinterpret_cast<const uint8_t *>(label),
                            strlen(label));
  mbedtls_sha256_finish_ret(&sha, out);
  mbedtls_sha256_free(&sha);
}

void load() {
  if (s_loaded)
    return;
  s_loaded = true;
  char tokenHex[33] = {0};
  Settings::getCompanionToken(tokenHex, sizeof(tokenHex));
  s_hasKey = isHex(tokenHex, 32);
  if (s_hasKey) {
    for (int i = 0; i < 16; ++i) {
      char byte[3] = {tokenHex[i * 2], tokenHex[i * 2 + 1], 0};
      s_token[i] = static_cast<uint8_t>(strtoul(byte, nullptr, 16));
    }
    deriveKey("hawaos-link-c2d", s_keyIn);
    deriveKey("hawaos-link-d2c", s_keyOut);
  }
  memset(tokenHex, 0, sizeof(tokenHex));
  s_lastN = Settings::getAuthCounter();
}

} // namespace

bool hasToken() {
  load();
  return s_hasKey;
}

void setToken(const char *hex) {
  Settings::setCompanionToken(hex); // also clears the stored counter
  s_loaded = false;
  load();
}

bool verify(const char *msg, size_t len, const char *macHex) {
  load();
  if (msg == nullptr || !s_hasKey || !isHex(macHex, 64))
    return false;
  uint8_t mac[32];
  if (mbedtls_md_hmac(mbedtls_md_info_from_type(MBEDTLS_MD_SHA256), s_token,
                      sizeof(s_token), reinterpret_cast<const uint8_t *>(msg),
                      len, mac) != 0) {
    return false;
  }
  static const char *kDigits = "0123456789abcdef";
  // Constant-time compare.
  uint8_t diff = 0;
  for (int i = 0; i < 32; ++i) {
    diff |= static_cast<uint8_t>(tolower(macHex[i * 2]) ^ kDigits[mac[i] >> 4]);
    diff |= static_cast<uint8_t>(tolower(macHex[i * 2 + 1]) ^
                                 kDigits[mac[i] & 0x0F]);
  }
  return diff == 0;
}

Result open(uint64_t n, const char *ivB64, const char *ctB64, char *out,
            size_t outLen) {
  load();
  if (!s_hasKey || ivB64 == nullptr || ctB64 == nullptr || outLen == 0)
    return Result::BadMac;
  uint8_t iv[kIvLen];
  size_t ivLen = 0;
  size_t ctLen = 0;
  if (mbedtls_base64_decode(iv, sizeof(iv), &ivLen,
                            reinterpret_cast<const uint8_t *>(ivB64),
                            strlen(ivB64)) != 0 ||
      ivLen != kIvLen ||
      mbedtls_base64_decode(s_buf, sizeof(s_buf), &ctLen,
                            reinterpret_cast<const uint8_t *>(ctB64),
                            strlen(ctB64)) != 0 ||
      ctLen <= kTagLen || ctLen - kTagLen >= outLen) {
    return Result::BadMac;
  }
  // n travels in the clear but is authenticated as associated data.
  char aad[24];
  const int aadLen =
      snprintf(aad, sizeof(aad), "%llu", static_cast<unsigned long long>(n));
  const size_t msgLen = ctLen - kTagLen;
  mbedtls_gcm_context gcm;
  mbedtls_gcm_init(&gcm);
  const bool ok =
      mbedtls_gcm_setkey(&gcm, MBEDTLS_CIPHER_ID_AES, s_keyIn, 256) == 0 &&
      mbedtls_gcm_auth_decrypt(&gcm, msgLen, iv, sizeof(iv),
                               reinterpret_cast<const uint8_t *>(aad), aadLen,
                               s_buf + msgLen, kTagLen, s_buf,
                               reinterpret_cast<uint8_t *>(out)) == 0;
  mbedtls_gcm_free(&gcm);
  if (!ok) {
    memset(out, 0, outLen);
    return Result::BadMac;
  }
  out[msgLen] = '\0';
  if (n <= s_lastN) {
    memset(out, 0, outLen);
    return Result::Stale;
  }
  s_lastN = n;
  return Result::Ok;
}

void persist() {
  load();
  if (s_lastN > Settings::getAuthCounter()) {
    Settings::setAuthCounter(s_lastN);
  }
}

bool seal(const char *json, char *out, size_t outLen) {
  load();
  const size_t len = (json != nullptr) ? strlen(json) : 0;
  if (!s_hasKey || len == 0 || len > kMaxPlain)
    return false;
  uint8_t iv[kIvLen];
  esp_fill_random(iv, sizeof(iv));
  // e echoes the newest companion n we accepted, d counts our messages;
  // the companion wants (e, d) fresh and increasing.
  const uint32_t d = ++s_sent;
  char aad[36];
  const int aadLen = snprintf(aad, sizeof(aad), "%llu:%lu",
                              static_cast<unsigned long long>(s_lastN),
                              static_cast<unsigned long>(d));
  mbedtls_gcm_context gcm;
  mbedtls_gcm_init(&gcm);
  const bool ok =
      mbedtls_gcm_setkey(&gcm, MBEDTLS_CIPHER_ID_AES, s_keyOut, 256) == 0 &&
      mbedtls_gcm_crypt_and_tag(&gcm, MBEDTLS_GCM_ENCRYPT, len, iv, sizeof(iv),
                                reinterpret_cast<const uint8_t *>(aad), aadLen,
                                reinterpret_cast<const uint8_t *>(json), s_buf,
                                kTagLen, s_buf + len) == 0;
  mbedtls_gcm_free(&gcm);
  if (!ok)
    return false;

  int w = snprintf(out, outLen,
                   "{\"type\":\"sealed\",\"e\":%llu,\"d\":%lu,\"iv\":\"",
                   static_cast<unsigned long long>(s_lastN),
                   static_cast<unsigned long>(d));
  if (w <= 0 || static_cast<size_t>(w) >= outLen)
    return false;
  size_t b64 = 0;
  if (mbedtls_base64_encode(reinterpret_cast<uint8_t *>(out + w), outLen - w,
                            &b64, iv, sizeof(iv)) != 0)
    return false;
  w += b64;
  const char *mid = "\",\"ct\":\"";
  if (static_cast<size_t>(w) + strlen(mid) >= outLen)
    return false;
  memcpy(out + w, mid, strlen(mid));
  w += strlen(mid);
  if (mbedtls_base64_encode(reinterpret_cast<uint8_t *>(out + w), outLen - w,
                            &b64, s_buf, len + kTagLen) != 0)
    return false;
  w += b64;
  if (static_cast<size_t>(w) + 3 > outLen)
    return false;
  out[w++] = '"';
  out[w++] = '}';
  out[w] = '\0';
  return true;
}

} // namespace CompanionAuth
