#pragma once

#include <stddef.h>
#include <stdint.h>

// Everything the companion sends over UDP is signed (DESIGN.md Section 11.6)
// with HMAC-SHA256 keyed by the token handed over at setup. Called from the
// WiFi task only.
namespace CompanionAuth {

// True once setup has given this device a token. Without one, unsigned
// network messages are still accepted (devices set up before signing).
bool hasToken();

// Stores a new token (setup) and restarts the replay counter.
void setToken(const char *hex);

// Constant-time check of macHex == HMAC-SHA256(token, msg) as hex.
bool verify(const char *msg, size_t len, const char *macHex);

enum class Result : uint8_t { Ok, BadMac, Stale };

// Checks a signed envelope's MAC over "<n>:<body>" and that n is newer than
// any accepted before. On Ok the counter moves to n in RAM only.
Result accept(uint64_t n, const char *body, const char *macHex);

// Saves the counter to flash. Called for messages that change the device,
// so a captured one can't be replayed even after a reboot; display data
// (stats every second) stays in RAM to spare the flash.
void persist();

} // namespace CompanionAuth
