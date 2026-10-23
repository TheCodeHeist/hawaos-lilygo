#include "services/NtpService.h"
#include "config.h"
#include "core/DataStore.h"
#include "core/EventBus.h"
#include "core/Settings.h"

#include <Arduino.h>
#include <time.h>

namespace NtpService {

namespace {
bool s_configured = false;
bool s_synced = false;
} // namespace

void begin() {
  s_configured = false;
  s_synced = false;
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

} // namespace NtpService
