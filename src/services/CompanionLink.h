#pragma once

#include <stdint.h>

namespace CompanionLink {

void begin();

// Poll UDP (port 7777), USB Serial, and pending HTTP album-art fetches on Core
// 0
void poll();

// Send outbound command to the Rust companion daemon (DESIGN.md Section 11.3)
// e.g. "play_pause", "next", "prev"
void sendMediaCommand(const char *cmd, uint32_t posMs = 0);

// Send clock command (e.g. "timer_toggle", "stopwatch_toggle")
void sendClockCommand(const char *cmd);

// Send config acknowledgement to companion
void sendConfigAck(uint32_t version);

} // namespace CompanionLink
