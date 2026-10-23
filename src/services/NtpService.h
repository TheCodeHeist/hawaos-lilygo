#pragma once

namespace NtpService {

void begin();
void onWifiConnected();
void requestSync();
void poll();
bool isSynced();

} // namespace NtpService
