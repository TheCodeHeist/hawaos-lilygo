#pragma once

namespace WifiService {

// Start the Core 0 WiFi connection & monitoring FreeRTOS task.
void begin();

// Update WiFi credentials in Settings and trigger immediate reconnect.
// Saves and joins new credentials; reports wifi_status back to the companion.
void reconfigure(const char *ssid, const char *pass);

// Scans on the WiFi task and sends a wifi_scan message to the companion.
void requestScan();

// Clear saved WiFi credentials from NVS and disconnect from AP.
void forgetWifi();

// Force a disconnect + reconnect cycle (useful for testing router-drop
// recovery).
void forceReconnect();

bool isConnected();

// Re-reads Settings' link mode: in Bluetooth mode Wi-Fi is off but the saved
// network is kept.
void applyLinkMode();

// Turns Wi-Fi on in Bluetooth mode while a firmware update needs it.
void holdForUpdate(bool hold);

} // namespace WifiService
