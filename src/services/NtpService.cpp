#include "services/NtpService.h"
#include "config.h"
#include "core/DataStore.h"
#include "core/EventBus.h"
#include "core/Settings.h"

#include <Arduino.h>
#include <stdlib.h>
#include <sys/time.h>
#include <time.h>

namespace NtpService {

namespace {
bool s_configured = false;
bool s_synced = false;

// The RTC keeps the time across resets, but not TZ: set it from the saved
// offset so the clock is local before NTP (or the companion) runs.
void applyTimezone() {
  const long off = Settings::getGmtOffsetSec();
  const long absOff = labs(off);
  char tz[16];
  // POSIX TZ offsets are inverted: UTC+6 is "UTC-6".
  snprintf(tz, sizeof(tz), "UTC%c%ld:%02ld", off >= 0 ? '-' : '+', absOff / 3600,
           (absOff % 3600) / 60);
  setenv("TZ", tz, 1);
  tzset();
}
} // namespace

void begin() {
  s_configured = false;
  s_synced = false;
  applyTimezone();
}

void onWifiConnected() { requestSync(); }

void requestSync() {
  const int32_t gmtOffset = Settings::getGmtOffsetSec();
  configTime(gmtOffset, Config::DEFAULT_DST_OFFSET_SEC, Config::NTP_SERVER_1,
             Config::NTP_SERVER_2, Config::NTP_SERVER_3);
  s_configured = true;
  Serial.printf(
      "[NtpService] Configured SNTP (GMT offset=%+lds, servers=%s, %s)\n",
      static_cast<long>(gmtOffset), Config::NTP_SERVER_1, Config::NTP_SERVER_2);
}

void poll() {
  if (!s_configured) {
    return;
  }

  const time_t now = time(nullptr);
  // Any epoch after Nov 2023 (1700000000) indicates valid NTP sync
  const bool valid = (now > 1700000000);
  if (valid != s_synced) {
    s_synced = valid;
    DataStore::updateNtp(s_synced);
    Event ev{};
    ev.type = EventType::DataUpdated;
    EventBus::post(ev);

    if (s_synced) {
      struct tm timeinfo{};
      localtime_r(&now, &timeinfo);
      char buf[40];
      strftime(buf, sizeof(buf), "%Y-%m-%d %H:%M:%S", &timeinfo);
      Serial.printf("[NtpService] Time synchronized: %s\n", buf);
    }
  }
}

bool isSynced() { return s_synced; }

void setFromCompanion(uint32_t epochSec) {
  if (s_synced || epochSec < 1700000000UL)
    return; // NTP (or an earlier stamp) already set the clock

  applyTimezone();
  const timeval tv{static_cast<time_t>(epochSec), 0};
  settimeofday(&tv, nullptr);
  s_synced = true;
  DataStore::updateNtp(true);
  Event ev{};
  ev.type = EventType::DataUpdated;
  EventBus::post(ev);
  Serial.println("[NtpService] Clock set from companion");
}

} // namespace NtpService
