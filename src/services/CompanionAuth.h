#pragma once

#include <stddef.h>
#include <stdint.h>

// Network traffic with the companion is encrypted and authenticated
// (DESIGN.md Section 11.6): AES-256-GCM with keys derived from the token
// handed over at setup, one key per direction. Called from the WiFi task only.
namespace CompanionAuth {

// True once setup has given this device a token. Without one, plaintext
// network messages are still accepted (devices set up before encryption).
bool hasToken();

// Stores a new token (setup) and restarts the replay counter.
void setToken(const char *hex);

// Constant-time check of macHex == HMAC-SHA256(token, msg) as hex (ota).
bool verify(const char *msg, size_t len, const char *macHex);

enum class Result : uint8_t { Ok, BadMac, Stale };

// Opens a companion envelope {n, iv, ct}: decrypts into out (NUL-terminated)
// and checks n is newer than any accepted before. On Ok the counter moves to
// n in RAM only.
Result open(uint64_t n, const char *ivB64, const char *ctB64, char *out,
            size_t outLen);

// Saves the counter to flash. Called for messages that change the device,
// so a captured one can't be replayed even after a reboot; display data
// (stats every second) stays in RAM to spare the flash.
void persist();

// Decrypts a companion blob in place (album art): buf holds the ciphertext,
// aad must match what the companion bound in ("art:<hash>"). false if the
// tag fails, in which case buf holds garbage.
bool openInPlace(const char *aad, const uint8_t iv[12], uint8_t *buf,
                 size_t len, const uint8_t tag[16]);

// Seals json for the companion as a complete envelope
// {"type":"sealed","e","d","iv","ct"}. false if out is too small.
bool seal(const char *json, char *out, size_t outLen);

} // namespace CompanionAuth
