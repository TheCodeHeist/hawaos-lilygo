#pragma once

#include <stdint.h>

enum class MediaPlayState : uint8_t {
  Stopped = 0,
  Playing = 1,
  Paused = 2,
};

struct NotificationItem {
  char app[32] = {0};
  char title[48] = {0};
  char body[96] = {0};
  uint32_t ts = 0;
  bool unread = false;
};

struct WeatherForecastDay {
  char day[8] = {0};
  char icon[24] = {0};
  int16_t tempMin = 0;
  int16_t tempMax = 0;
};

struct WeatherSnapshot {
  bool valid = false;
  uint32_t lastUpdateTs = 0;
  char city[32] = {0};
  int16_t temp = 0;
  int16_t feelsLike = 0;
  int16_t tempMin = 0;
  int16_t tempMax = 0;
  uint8_t humidity = 0;
  uint16_t windSpeed = 0;
  char unit[4] = "C";
  char condition[32] = {0};
  char icon[24] = {0};

  static constexpr uint8_t FORECAST_DAYS = 3;
  WeatherForecastDay forecast[FORECAST_DAYS] = {};
};

struct DataSnapshot {
  static constexpr uint8_t SPARKLINE_LEN = 60;

  // System stats from Rust companion daemon (1 Hz)
  uint32_t timestamp = 0;
  uint8_t cpuPercent = 0;
  uint8_t ramPercent = 0;
  uint32_t netUpKbps = 0;
  uint32_t netDownKbps = 0;
  uint32_t lastStatsMs = 0;

  // 60-sample rolling sparkline history (oldest at 0, newest at 59)
  uint16_t netDownHistory[SPARKLINE_LEN] = {0};

  // Notifications ring/stack (newest at index 0)
  static constexpr uint8_t MAX_NOTIFICATIONS = 6;
  NotificationItem notifications[MAX_NOTIFICATIONS] = {};
  uint8_t notifyCount = 0;
  uint8_t unreadCount = 0;

  // Media state (DESIGN.md Section 11.1)
  MediaPlayState mediaState = MediaPlayState::Stopped;
  char mediaTitle[64] = {0};
  char mediaArtist[48] = {0};
  char mediaAlbum[48] = {0};
  uint32_t mediaPosMs = 0;
  uint32_t mediaDurMs = 0;
  uint32_t mediaRxMillis = 0;
  char mediaArtHash[16] = {0};
  bool mediaArtReady = false;
  uint32_t mediaBgTop = 0x071115;    // Pre-darkened companion tint top
  uint32_t mediaBgBottom = 0x05090B; // Pre-darkened companion tint bottom
  bool mediaInkLight = true;         // true = light ink, false = dark ink

  // Weather state from companion
  WeatherSnapshot weather{};

  // Connectivity & time state
  bool wifiConnected = false;
  int8_t wifiRssi = 0;
  char wifiSsid[33] = {0};
  char wifiIp[16] = {0};
  bool usbConnected = false;
  bool ntpSynced = false;
  bool bleConnected = false;
};

namespace DataStore {

constexpr uint16_t ALBUM_ART_DIM = 128;
constexpr uint32_t ALBUM_ART_BYTES =
    ALBUM_ART_DIM * ALBUM_ART_DIM * 2; // 32768 B

void begin();
DataSnapshot getSnapshot();
void setSnapshot(const DataSnapshot &snapshot);

void updateStats(uint32_t ts, uint8_t cpuPct, uint8_t ramPct,
                 uint32_t netUpKbps, uint32_t netDownKbps, bool viaUsb);
void updateMedia(MediaPlayState state, const char *title, const char *artist,
                 const char *album, uint32_t posMs, uint32_t durMs,
                 const char *artHash, uint32_t bgTop, uint32_t bgBottom,
                 bool inkLight);
void setAlbumArtReady(const char *artHash, bool ready);
uint16_t *albumArtBuffer(); // 128x128 RGB565 buffer in PSRAM

void updateWeather(const WeatherSnapshot &ws);

void updateWifi(bool connected, int8_t rssi, const char *ssid, const char *ip);
void updateNtp(bool synced);
void pushNotification(const char *app, const char *title, const char *body, uint32_t ts);
inline void pushNotification(const char *title, const char *body, uint32_t ts) {
  pushNotification("", title, body, ts);
}
void dismissNotification(uint8_t index);
void clearAllNotifications();
void markAllNotificationsRead();

} // namespace DataStore
