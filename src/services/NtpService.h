#pragma once

#include <stdint.h>

namespace NtpService {

void begin();
void onWifiConnected();
void requestSync();
void poll();
bool isSynced();

// USB-only devices have no NTP: take the time from companion stats instead.
void setFromCompanion(uint32_t epochSec);

} // namespace NtpService
