#pragma once

#include <stddef.h>
#include <stdint.h>

// Firmware updates over WiFi (DESIGN.md Section 11.5). The companion serves
// the image over HTTP; the device streams it into the inactive app slot,
// hashing as it goes, and only switches slots if the SHA-256 matches.
namespace OtaService {

enum class State : uint8_t { Idle, Downloading, Verifying, Done, Failed };

struct Progress {
  State state = State::Idle;
  uint8_t pct = 0;
  char error[48] = {0};
};

// Queues an update (runs on the WiFi task via poll()). false if one is
// already running or the request is malformed.
bool start(const char *url, uint32_t size, const char *sha256Hex);

// HMAC-SHA256(token, "<sha256>:<size>") as hex: proves a network request
// comes from the companion that set this device up.
bool checkMac(const char *sha256Hex, uint32_t size, const char *macHex);

Progress progress();
bool isActive(); // downloading, verifying, or just finished/failed

// Called from the WiFi task loop; blocks for the length of the download.
void poll();

} // namespace OtaService
