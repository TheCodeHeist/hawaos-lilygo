#pragma once

#include <stddef.h>
#include <stdint.h>

// First-run setup (DESIGN.md Section 11.4). While active, the device shows the
// Setup screen, advertises over BLE and waits for the companion's wizard to
// give it WiFi. Runs on Core 0 (polled from the WiFi task); the UI reads it
// through snapshot().
namespace SetupService {

enum class Stage : uint8_t {
  Waiting,    // no companion yet
  Linked,     // the companion's wizard is talking to us
  Joining,    // trying the WiFi it sent
  JoinFailed, // wrong password, no such network or timeout
  Online,     // joined; waiting for the wizard to finish
  Done,       // setup finished, about to show Home
};

enum class Link : uint8_t { None, Usb, Udp, Ble };

struct Snapshot {
  bool active = false;
  Stage stage = Stage::Waiting;
  Link link = Link::None;
  bool paired = false;     // BLE peer proved it knows the code
  uint32_t generation = 0; // bumps on every change
  uint32_t stageSinceMs = 0;
  char host[32] = {0};     // companion's hostname
  char ssid[33] = {0};
  char ip[16] = {0};
  char reason[12] = {0};   // "auth" | "no_ap" | "timeout"
  char pin[7] = {0};       // 6-digit pairing code shown on screen
  char bleName[16] = {0};  // "HawaOS-7F3A"
};

// Enters setup mode if the device has not been set up (call once at boot).
void begin();
// Enters setup mode now (e.g. "Set up again" from on-device Settings).
void start();
bool isActive();
Snapshot snapshot();

// Short id from the MAC, e.g. "7F3A" (also used in the BLE name).
const char *deviceId();

void onCompanionAttached(Link link, const char *host);
void onBleDisconnected();
void onJoinStarted(const char *ssid);
void onJoinResult(bool ok, const char *ip, const char *reason);
void finish(); // wizard done: persist and head Home
void skip();   // user skipped on the device

// Called from the WiFi task: stops BLE once setup is over.
void poll();

// --- Pairing session (BLE): X25519 + 6-digit code -> AES-256-GCM ---
// Takes the companion's base64 public key, writes ours (base64) to outB64.
bool exchangeKey(const char *peerPubB64, char *outB64, size_t outLen);
// Decrypts base64 nonce (12 B) + ciphertext||tag with the session key. Wrong
// code => false. Plaintext is NUL-terminated.
bool decrypt(const char *nonceB64, const char *ctB64, char *out, size_t outLen);
// Checks the companion's proof of the code. Counts failures and rotates the
// code after too many, so it can't be guessed online.
bool verify(const char *nonceB64, const char *ctB64, uint8_t *attemptsLeft);
bool isSessionVerified();

} // namespace SetupService
