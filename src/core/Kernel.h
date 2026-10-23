#pragma once

namespace Kernel {

// Boot HawaOS following the strict initialization sequence and start FreeRTOS
// tasks.
void begin();

// Log free/minimum/total internal SRAM and OPI PSRAM over Serial.
void logMemoryStats(const char *tag);

} // namespace Kernel
