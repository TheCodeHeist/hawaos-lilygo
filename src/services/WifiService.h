#pragma once

namespace WifiService {

// Start the Core 0 WiFi connection & monitoring FreeRTOS task.
void begin();

// Update WiFi credentials in Settings and trigger immediate reconnect.
void reconfigure(const char *ssid, const char *pass);

// Force a disconnect + reconnect cycle (useful for testing router-drop
// recovery).
void forceReconnect();

bool isConnected();

} // namespace WifiService
