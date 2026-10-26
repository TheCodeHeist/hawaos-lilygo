#include "services/SetupService.h"
#include "config.h"
#include "core/Settings.h"
#include "services/BleLink.h"
#include "services/WifiService.h"

#include <Arduino.h>
#include <WiFi.h>
#include <esp_mac.h>
#include <esp_random.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <mbedtls/base64.h>
#include <mbedtls/ecdh.h>
#include <mbedtls/gcm.h>
#include <mbedtls/sha256.h>
#include <string.h>

namespace SetupService {

namespace {

// Domain separation for the session key and the code check.
constexpr const char *kKdfLabel = "hawaos-setup-v1";
constexpr const char *kVerifyText = "hawaos-setup-v1";
constexpr uint8_t kMaxVerifyAttempts = 5;
constexpr uint32_t kBleStopDelayMs = 1500; // let the last notify go out

SemaphoreHandle_t s_lock = nullptr;
Snapshot s_state;
char s_deviceId[5] = {0};

uint8_t s_privKey[32] = {0};
uint8_t s_sessionKey[32] = {0};
bool s_haveSession = false;
bool s_verified = false;
uint8_t s_failedVerifies = 0;

int fillRandom(void * /*ctx*/, unsigned char *buf, size_t len) {
  esp_fill_random(buf, len);
  return 0;
}

class Guard {
public:
  Guard() { xSemaphoreTake(s_lock, portMAX_DELAY); }
  ~Guard() { xSemaphoreGive(s_lock); }
};

// Callers hold the lock.
void setStageLocked(Stage stage) {
  s_state.stage = stage;
  s_state.stageSinceMs = millis();
  ++s_state.generation;
}

void newPinLocked() {
  snprintf(s_state.pin, sizeof(s_state.pin), "%06lu",
           static_cast<unsigned long>(esp_random() % 1000000UL));
  ++s_state.generation;
}

void resetSession() {
  if (s_lock != nullptr) {
    Guard g;
    if (s_state.paired) {
      s_state.paired = false;
      ++s_state.generation;
    }
  }
  memset(s_privKey, 0, sizeof(s_privKey));
  memset(s_sessionKey, 0, sizeof(s_sessionKey));
  s_haveSession = false;
  s_verified = false;
}

void copyStr(char *dst, size_t dstLen, const char *src) {
  if (src == nullptr) {
    dst[0] = '\0';
    return;
  }
  strncpy(dst, src, dstLen - 1);
  dst[dstLen - 1] = '\0';
}

bool b64Decode(const char *in, uint8_t *out, size_t outLen, size_t *written) {
  if (in == nullptr)
    return false;
  return mbedtls_base64_decode(out, outLen, written,
                               reinterpret_cast<const unsigned char *>(in),
                               strlen(in)) == 0;
}

} // namespace

void begin() {
  if (s_lock == nullptr) {
    s_lock = xSemaphoreCreateMutex();
  }
  uint8_t mac[6] = {0};
  esp_read_mac(mac, ESP_MAC_BT);
  snprintf(s_deviceId, sizeof(s_deviceId), "%02X%02X", mac[4], mac[5]);
  {
    Guard g;
    snprintf(s_state.bleName, sizeof(s_state.bleName), "HawaOS-%s", s_deviceId);
  }
  if (!Settings::isSetupDone()) {
    start();
  } else if (Settings::getLinkMode() == LinkMode::Ble) {
    // Everyday link (DESIGN.md Section 11.7); before the WiFi task starts,
    // so it comes up in modem sleep.
    BleLink::start(s_state.bleName, true);
  }
}

void start() {
  {
    Guard g;
    if (s_state.active)
      return;
    s_state.active = true;
    s_state.link = Link::None;
    s_state.host[0] = '\0';
    s_state.ssid[0] = '\0';
    s_state.ip[0] = '\0';
    s_state.reason[0] = '\0';
    newPinLocked();
    setStageLocked(Stage::Waiting);
  }
  resetSession();
  s_failedVerifies = 0;
  Serial.printf("[Setup] Setup mode: BLE '%s'\n", s_state.bleName);
  // WiFi + BLE coexistence requires modem sleep (else the WiFi driver aborts).
  WiFi.setSleep(true);
  if (BleLink::isRunning()) {
    BleLink::setPaired(false); // Set Up Again in Bluetooth mode
  } else {
    BleLink::start(s_state.bleName, false);
  }
}

bool isActive() {
  Guard g;
  return s_state.active;
}

Snapshot snapshot() {
  Guard g;
  return s_state;
}

const char *deviceId() { return s_deviceId; }

void onCompanionAttached(Link link, const char *host) {
  Guard g;
  if (!s_state.active)
    return;
  s_state.link = link;
  copyStr(s_state.host, sizeof(s_state.host), host);
  if (s_state.stage == Stage::Waiting) {
    setStageLocked(Stage::Linked);
  } else {
    ++s_state.generation;
  }
}

void onBleDisconnected() {
  resetSession();
  Guard g;
  if (s_state.active && s_state.link == Link::Ble &&
      s_state.stage != Stage::Done) {
    s_state.link = Link::None;
    setStageLocked(Stage::Waiting);
  }
}

void onJoinStarted(const char *ssid) {
  Guard g;
  if (!s_state.active)
    return;
  copyStr(s_state.ssid, sizeof(s_state.ssid), ssid);
  s_state.reason[0] = '\0';
  setStageLocked(Stage::Joining);
}

void onJoinResult(bool ok, const char *ip, const char *reason) {
  Guard g;
  if (!s_state.active)
    return;
  if (ok) {
    copyStr(s_state.ip, sizeof(s_state.ip), ip);
    setStageLocked(Stage::Online);
  } else {
    copyStr(s_state.reason, sizeof(s_state.reason), reason);
    setStageLocked(Stage::JoinFailed);
  }
}

void finish() {
  Settings::setSetupDone(true);
  Guard g;
  if (s_state.active) {
    setStageLocked(Stage::Done);
  }
}

void skip() {
  Settings::setSetupDone(true);
  {
    Guard g;
    if (!s_state.active)
      return;
    setStageLocked(Stage::Done);
  }
  Serial.println("[Setup] Skipped on the device");
}

void poll() {
  bool stopNow = false;
  {
    Guard g;
    if (s_state.active && s_state.stage == Stage::Done &&
        millis() - s_state.stageSinceMs >= kBleStopDelayMs) {
      s_state.active = false;
      ++s_state.generation;
      stopNow = true;
    }
  }
  if (stopNow) {
    resetSession();
    if (Settings::getLinkMode() == LinkMode::Ble) {
      BleLink::setPaired(true);
      WifiService::applyLinkMode(); // a network joined during setup goes off
      Serial.println("[Setup] Setup complete, staying on Bluetooth");
    } else {
      BleLink::stop();
      WiFi.setSleep(false);
      Serial.println("[Setup] Setup complete, BLE off");
    }
  }
}

void applyLinkMode() {
  if (isActive())
    return;
  if (Settings::getLinkMode() == LinkMode::Ble) {
    WiFi.setSleep(true);
    if (!BleLink::isRunning()) {
      BleLink::start(s_state.bleName, true);
    }
  } else if (BleLink::isRunning()) {
    BleLink::stop();
    WiFi.setSleep(false);
    Serial.println("[Setup] Link mode Wi-Fi, BLE off");
  }
}

bool exchangeKey(const char *peerPubB64, char *outB64, size_t outLen) {
  uint8_t peerPub[32];
  size_t peerLen = 0;
  if (!b64Decode(peerPubB64, peerPub, sizeof(peerPub), &peerLen) ||
      peerLen != 32) {
    return false;
  }

  mbedtls_ecp_group grp;
  mbedtls_mpi d, z;
  mbedtls_ecp_point q, peer;
  mbedtls_ecp_group_init(&grp);
  mbedtls_mpi_init(&d);
  mbedtls_mpi_init(&z);
  mbedtls_ecp_point_init(&q);
  mbedtls_ecp_point_init(&peer);

  uint8_t ourPub[32] = {0};
  uint8_t shared[32] = {0};
  size_t pubLen = 0;
  bool ok =
      mbedtls_ecp_group_load(&grp, MBEDTLS_ECP_DP_CURVE25519) == 0 &&
      mbedtls_ecp_gen_keypair(&grp, &d, &q, fillRandom, nullptr) == 0 &&
      // Montgomery curves use the RFC 7748 encoding: 32-byte little-endian X.
      mbedtls_ecp_point_write_binary(&grp, &q, MBEDTLS_ECP_PF_UNCOMPRESSED,
                                     &pubLen, ourPub, sizeof(ourPub)) == 0 &&
      pubLen == 32 &&
      mbedtls_ecp_point_read_binary(&grp, &peer, peerPub, 32) == 0 &&
      mbedtls_ecdh_compute_shared(&grp, &z, &peer, &d, fillRandom, nullptr) ==
          0 &&
      mbedtls_mpi_write_binary_le(&z, shared, sizeof(shared)) == 0;

  mbedtls_ecp_group_free(&grp);
  mbedtls_mpi_free(&d);
  mbedtls_mpi_free(&z);
  mbedtls_ecp_point_free(&q);
  mbedtls_ecp_point_free(&peer);
  if (!ok) {
    return false;
  }

  // key = SHA-256(shared || label || code): only someone who can read the
  // code off this screen derives the same key.
  char pin[7];
  {
    Guard g;
    memcpy(pin, s_state.pin, sizeof(pin));
  }
  mbedtls_sha256_context sha;
  mbedtls_sha256_init(&sha);
  mbedtls_sha256_starts_ret(&sha, 0);
  mbedtls_sha256_update_ret(&sha, shared, sizeof(shared));
  mbedtls_sha256_update_ret(&sha, reinterpret_cast<const uint8_t *>(kKdfLabel),
                            strlen(kKdfLabel));
  mbedtls_sha256_update_ret(&sha, reinterpret_cast<const uint8_t *>(pin), 6);
  mbedtls_sha256_finish_ret(&sha, s_sessionKey);
  mbedtls_sha256_free(&sha);
  memset(shared, 0, sizeof(shared));
  s_haveSession = true;
  s_verified = false;

  size_t outWritten = 0;
  return mbedtls_base64_encode(reinterpret_cast<unsigned char *>(outB64),
                               outLen, &outWritten, ourPub,
                               sizeof(ourPub)) == 0;
}

bool decrypt(const char *nonceB64, const char *ctB64, char *out,
             size_t outLen) {
  if (!s_haveSession || outLen == 0)
    return false;
  uint8_t nonce[12];
  uint8_t ct[256];
  size_t nonceLen = 0;
  size_t ctLen = 0;
  if (!b64Decode(nonceB64, nonce, sizeof(nonce), &nonceLen) ||
      nonceLen != sizeof(nonce) || !b64Decode(ctB64, ct, sizeof(ct), &ctLen) ||
      ctLen < 16 || ctLen - 16 >= outLen) {
    return false;
  }
  const size_t msgLen = ctLen - 16; // ciphertext || 16-byte tag

  mbedtls_gcm_context gcm;
  mbedtls_gcm_init(&gcm);
  const bool ok =
      mbedtls_gcm_setkey(&gcm, MBEDTLS_CIPHER_ID_AES, s_sessionKey, 256) == 0 &&
      mbedtls_gcm_auth_decrypt(&gcm, msgLen, nonce, sizeof(nonce), nullptr, 0,
                               ct + msgLen, 16, ct,
                               reinterpret_cast<unsigned char *>(out)) == 0;
  mbedtls_gcm_free(&gcm);
  out[ok ? msgLen : 0] = '\0';
  return ok;
}

bool verify(const char *nonceB64, const char *ctB64, uint8_t *attemptsLeft) {
  char plain[32];
  const bool ok = decrypt(nonceB64, ctB64, plain, sizeof(plain)) &&
                  strcmp(plain, kVerifyText) == 0;
  if (ok) {
    s_verified = true;
    s_failedVerifies = 0;
    Guard g;
    s_state.paired = true;
    ++s_state.generation;
  } else if (++s_failedVerifies >= kMaxVerifyAttempts) {
    // Too many wrong codes: show a fresh one and drop the session.
    s_failedVerifies = 0;
    resetSession();
    Guard g;
    newPinLocked();
  }
  if (attemptsLeft != nullptr) {
    *attemptsLeft = kMaxVerifyAttempts - s_failedVerifies;
  }
  return ok;
}

bool isSessionVerified() { return s_verified; }

} // namespace SetupService
