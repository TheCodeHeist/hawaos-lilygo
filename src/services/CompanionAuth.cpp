#include "services/CompanionAuth.h"
#include "core/Settings.h"

#include <Arduino.h>
#include <mbedtls/md.h>
#include <string.h>

namespace CompanionAuth {

namespace {

bool s_loaded = false;
bool s_hasKey = false;
uint8_t s_key[16];
uint64_t s_lastN = 0;

bool isHex(const char *s, size_t len) {
  if (s == nullptr || strlen(s) != len)
    return false;
  for (size_t i = 0; i < len; ++i) {
    if (!isxdigit(static_cast<unsigned char>(s[i])))
      return false;
  }
  return true;
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
      s_key[i] = static_cast<uint8_t>(strtoul(byte, nullptr, 16));
    }
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
  if (mbedtls_md_hmac(mbedtls_md_info_from_type(MBEDTLS_MD_SHA256), s_key,
                      sizeof(s_key), reinterpret_cast<const uint8_t *>(msg),
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

Result accept(uint64_t n, const char *body, const char *macHex) {
  if (body == nullptr)
    return Result::BadMac;
  // "<n>:" prefix, then the body exactly as sent; HMAC it in one buffer.
  char prefix[24];
  const int prefixLen =
      snprintf(prefix, sizeof(prefix), "%llu:", static_cast<unsigned long long>(n));
  const size_t bodyLen = strlen(body);
  char *msg = static_cast<char *>(malloc(prefixLen + bodyLen));
  if (msg == nullptr)
    return Result::BadMac;
  memcpy(msg, prefix, prefixLen);
  memcpy(msg + prefixLen, body, bodyLen);
  const bool ok = verify(msg, prefixLen + bodyLen, macHex);
  free(msg);
  if (!ok)
    return Result::BadMac;
  if (n <= s_lastN)
    return Result::Stale;
  s_lastN = n;
  return Result::Ok;
}

void persist() {
  load();
  if (s_lastN > Settings::getAuthCounter()) {
    Settings::setAuthCounter(s_lastN);
  }
}

} // namespace CompanionAuth
