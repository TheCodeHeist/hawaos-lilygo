#pragma once

#include <stdint.h>

struct DataSnapshot {
  // System stats from Rust companion daemon
  uint32_t timestamp = 0;
  uint8_t cpuPercent = 0;
  uint8_t ramPercent = 0;
  uint32_t netUpKbps = 0;
  uint32_t netDownKbps = 0;
  uint32_t lastStatsMs = 0;

  // Latest notification from companion daemon
  char notifyTitle[48] = {0};
  char notifyBody[128] = {0};
  uint32_t notifyTs = 0;
  uint32_t notifySeq = 0;

  // Connectivity & time state
  bool wifiConnected = false;
  int8_t wifiRssi = 0;
  char wifiSsid[33] = {0};
  char wifiIp[16] = {0};
  bool ntpSynced = false;
  bool bleConnected = false;
};

namespace DataStore {

void begin();
DataSnapshot getSnapshot();
void setSnapshot(const DataSnapshot &snapshot);
void updateWifi(bool connected, int8_t rssi, const char *ssid, const char *ip);
void updateNtp(bool synced);

} // namespace DataStore
