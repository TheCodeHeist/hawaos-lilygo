#pragma once

#include <stddef.h>
#include <stdint.h>

// Network messages that change the device (DESIGN.md Section 11.6) are signed
// with HMAC-SHA256 keyed by the token the companion handed over at setup.
namespace CompanionAuth {

// True once setup has given this device a token. Without one, unsigned
// network settings are still accepted (devices set up before signing).
bool hasToken();

// Constant-time check of macHex == HMAC-SHA256(token, msg) as hex.
bool verify(const char *msg, size_t len, const char *macHex);

enum class Result : uint8_t { Ok, BadMac, Stale };

// Checks a signed envelope's MAC over "<n>:<body>" and that n is newer than
// any accepted before; on Ok, stores n so the packet can't be replayed.
Result accept(uint64_t n, const char *body, const char *macHex);

} // namespace CompanionAuth
