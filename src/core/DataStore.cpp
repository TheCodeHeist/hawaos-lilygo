#include "core/DataStore.h"

#include <Arduino.h>
#include <esp_heap_caps.h>
#include <freertos/FreeRTOS.h>
#include <freertos/semphr.h>
#include <string.h>

namespace DataStore {

namespace {
SemaphoreHandle_t s_mutex = nullptr;
DataSnapshot s_data{};
uint16_t *s_albumArtBuf = nullptr;
} // namespace

void begin() {
  if (s_mutex == nullptr) {
    s_mutex = xSemaphoreCreateMutex();
  }
  if (s_albumArtBuf == nullptr) {
    s_albumArtBuf = static_cast<uint16_t *>(
        heap_caps_malloc(ALBUM_ART_BYTES, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT));
    if (s_albumArtBuf != nullptr) {
      memset(s_albumArtBuf, 0, ALBUM_ART_BYTES);
    }
  }
}

void copyText(char *dst, size_t size, const char *src) {
  if (size == 0) {
    return;
  }
  size_t n = (src != nullptr) ? strnlen(src, size - 1) : 0;
  if (n == size - 1 && src[n] != '\0') {
    // Cut short: back up over continuation bytes to the start of the last
    // character, and drop it unless all of it fit.
    size_t lead = n;
    while (lead > 0 && (static_cast<uint8_t>(src[lead]) & 0xC0) == 0x80) {
      --lead;
    }
    n = lead;
  }
  memcpy(dst, src, n);
  dst[n] = '\0';
}

DataSnapshot getSnapshot() {
  DataSnapshot copy{};
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    copy = s_data;
    xSemaphoreGive(s_mutex);
  }
  return copy;
}

void setSnapshot(const DataSnapshot &snapshot) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data = snapshot;
    xSemaphoreGive(s_mutex);
  }
}

void updateStats(uint32_t ts, uint8_t cpuPct, uint8_t ramPct,
                 uint32_t netUpKbps, uint32_t netDownKbps, bool viaUsb) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.timestamp = ts;
    s_data.cpuPercent = (cpuPct > 100) ? 100 : cpuPct;
    s_data.ramPercent = (ramPct > 100) ? 100 : ramPct;
    s_data.netUpKbps = netUpKbps;
    s_data.netDownKbps = netDownKbps;
    s_data.lastStatsMs = millis();
    s_data.usbConnected = viaUsb;

    for (uint8_t i = 0; i + 1 < DataSnapshot::SPARKLINE_LEN; ++i) {
      s_data.netDownHistory[i] = s_data.netDownHistory[i + 1];
    }
    const uint32_t clampedKbps = (netDownKbps > 65535U) ? 65535U : netDownKbps;
    s_data.netDownHistory[DataSnapshot::SPARKLINE_LEN - 1] =
        static_cast<uint16_t>(clampedKbps);

    xSemaphoreGive(s_mutex);
  }
}

void updateMedia(MediaPlayState state, const char *title, const char *artist,
                 const char *album, uint32_t posMs, uint32_t durMs,
                 const char *artHash, uint32_t bgTop, uint32_t bgBottom,
                 bool inkLight) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.mediaState = state;

    if (title != nullptr) {
      copyText(s_data.mediaTitle, title);
    } else {
      s_data.mediaTitle[0] = '\0';
    }

    if (artist != nullptr) {
      copyText(s_data.mediaArtist, artist);
    } else {
      s_data.mediaArtist[0] = '\0';
    }

    if (album != nullptr) {
      copyText(s_data.mediaAlbum, album);
    } else {
      s_data.mediaAlbum[0] = '\0';
    }

    s_data.mediaPosMs = posMs;
    s_data.mediaDurMs = durMs;
    s_data.mediaRxMillis = millis();
    s_data.mediaBgTop = bgTop;
    s_data.mediaBgBottom = bgBottom;
    s_data.mediaInkLight = inkLight;

    const char *safeHash = (artHash != nullptr) ? artHash : "";
    if (strncmp(s_data.mediaArtHash, safeHash, sizeof(s_data.mediaArtHash)) !=
        0) {
      strncpy(s_data.mediaArtHash, safeHash, sizeof(s_data.mediaArtHash) - 1);
      s_data.mediaArtHash[sizeof(s_data.mediaArtHash) - 1] = '\0';
      s_data.mediaArtReady = false;
    }

    xSemaphoreGive(s_mutex);
  }
}

void setAlbumArtReady(const char *artHash, bool ready) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    if (artHash != nullptr && strncmp(s_data.mediaArtHash, artHash,
                                      sizeof(s_data.mediaArtHash)) == 0) {
      s_data.mediaArtReady = ready;
    }
    xSemaphoreGive(s_mutex);
  }
}

uint16_t *albumArtBuffer() { return s_albumArtBuf; }

void updateWeather(const WeatherSnapshot &ws) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.weather = ws;
    s_data.weather.valid = true;
    s_data.weather.lastUpdateTs = millis();
    xSemaphoreGive(s_mutex);
  }
}

void setBleConnected(bool connected) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.bleConnected = connected;
    xSemaphoreGive(s_mutex);
  }
}

void updateWifi(bool connected, int8_t rssi, const char *ssid, const char *ip) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.wifiConnected = connected;
    s_data.wifiRssi = rssi;
    if (ssid != nullptr) {
      copyText(s_data.wifiSsid, ssid);
    } else {
      s_data.wifiSsid[0] = '\0';
    }
    if (ip != nullptr) {
      strncpy(s_data.wifiIp, ip, sizeof(s_data.wifiIp) - 1);
      s_data.wifiIp[sizeof(s_data.wifiIp) - 1] = '\0';
    } else {
      s_data.wifiIp[0] = '\0';
    }
    xSemaphoreGive(s_mutex);
  }
}

void updateNtp(bool synced) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.ntpSynced = synced;
    xSemaphoreGive(s_mutex);
  }
}

void pushNotification(const char *app, const char *title, const char *body, uint32_t ts) {
  if ((app == nullptr || app[0] == '\0') && (title == nullptr || title[0] == '\0') &&
      (body == nullptr || body[0] == '\0')) {
    return;
  }
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    const uint8_t cap = DataSnapshot::MAX_NOTIFICATIONS;
    const uint8_t shiftCount =
        (s_data.notifyCount < cap) ? s_data.notifyCount : (cap - 1);
    for (int i = static_cast<int>(shiftCount); i > 0; --i) {
      s_data.notifications[i] = s_data.notifications[i - 1];
    }
    NotificationItem &item = s_data.notifications[0];
    const char *safeApp = (app != nullptr && app[0] != '\0')
                              ? app
                              : ((title != nullptr && title[0] != '\0') ? title : "Notification");
    copyText(item.app, safeApp);
    copyText(item.title, title ? title : "");
    copyText(item.body, body);
    item.ts = ts;
    item.unread = true;

    if (s_data.notifyCount < cap) {
      ++s_data.notifyCount;
    }
    uint8_t unread = 0;
    for (uint8_t i = 0; i < s_data.notifyCount; ++i) {
      if (s_data.notifications[i].unread)
        ++unread;
    }
    s_data.unreadCount = unread;
    xSemaphoreGive(s_mutex);
  }
}

void dismissNotification(uint8_t index) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    if (index < s_data.notifyCount) {
      for (uint8_t i = index; i + 1 < s_data.notifyCount; ++i) {
        s_data.notifications[i] = s_data.notifications[i + 1];
      }
      --s_data.notifyCount;
      uint8_t unread = 0;
      for (uint8_t i = 0; i < s_data.notifyCount; ++i) {
        if (s_data.notifications[i].unread)
          ++unread;
      }
      s_data.unreadCount = unread;
    }
    xSemaphoreGive(s_mutex);
  }
}

void clearAllNotifications() {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.notifyCount = 0;
    s_data.unreadCount = 0;
    xSemaphoreGive(s_mutex);
  }
}

void markAllNotificationsRead() {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    for (uint8_t i = 0; i < s_data.notifyCount; ++i) {
      s_data.notifications[i].unread = false;
    }
    s_data.unreadCount = 0;
    xSemaphoreGive(s_mutex);
  }
}

void startStopwatch() {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    if (s_data.stopwatch.state != StopwatchState::Running) {
      s_data.stopwatch.state = StopwatchState::Running;
      s_data.stopwatch.lastStartMs = millis();
    }
    xSemaphoreGive(s_mutex);
  }
}

void pauseStopwatch() {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    if (s_data.stopwatch.state == StopwatchState::Running) {
      const uint32_t now = millis();
      s_data.stopwatch.elapsedMs += (now - s_data.stopwatch.lastStartMs);
      s_data.stopwatch.state = StopwatchState::Paused;
    }
    xSemaphoreGive(s_mutex);
  }
}

void resetStopwatch() {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.stopwatch.state = StopwatchState::Stopped;
    s_data.stopwatch.elapsedMs = 0;
    s_data.stopwatch.lastStartMs = 0;
    xSemaphoreGive(s_mutex);
  }
}

void toggleStopwatch() {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    const uint32_t now = millis();
    if (s_data.stopwatch.state == StopwatchState::Running) {
      s_data.stopwatch.elapsedMs += (now - s_data.stopwatch.lastStartMs);
      s_data.stopwatch.state = StopwatchState::Paused;
    } else {
      s_data.stopwatch.state = StopwatchState::Running;
      s_data.stopwatch.lastStartMs = now;
    }
    xSemaphoreGive(s_mutex);
  }
}

void startTimer(uint32_t durationSec) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    if (durationSec > 0) {
      s_data.timer.durationSec = durationSec;
    }
    s_data.timer.remainingSec = s_data.timer.durationSec;
    s_data.timer.targetEpochMs = millis() + (s_data.timer.durationSec * 1000UL);
    s_data.timer.state = TimerState::Running;
    xSemaphoreGive(s_mutex);
  }
}

void startTimerMs(uint32_t durationMs) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    if (durationMs > 0) {
      s_data.timer.durationSec = (durationMs + 999) / 1000;
    }
    s_data.timer.remainingSec = s_data.timer.durationSec;
    s_data.timer.targetEpochMs = millis() + durationMs;
    s_data.timer.state = TimerState::Running;
    xSemaphoreGive(s_mutex);
  }
}

void pauseTimer() {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    if (s_data.timer.state == TimerState::Running) {
      const uint32_t now = millis();
      if (now < s_data.timer.targetEpochMs) {
        s_data.timer.remainingSec = (s_data.timer.targetEpochMs - now + 999) / 1000;
      } else {
        s_data.timer.remainingSec = 0;
      }
      s_data.timer.state = TimerState::Paused;
    }
    xSemaphoreGive(s_mutex);
  }
}

void resumeTimer() {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    if (s_data.timer.state == TimerState::Paused || s_data.timer.state == TimerState::Stopped) {
      if (s_data.timer.remainingSec == 0) {
        s_data.timer.remainingSec = (s_data.timer.durationSec > 0) ? s_data.timer.durationSec : 300;
      }
      s_data.timer.targetEpochMs = millis() + (s_data.timer.remainingSec * 1000UL);
      s_data.timer.state = TimerState::Running;
    }
    xSemaphoreGive(s_mutex);
  }
}

void resetTimer() {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.timer.state = TimerState::Stopped;
    s_data.timer.remainingSec = (s_data.timer.durationSec > 0) ? s_data.timer.durationSec : 300;
    xSemaphoreGive(s_mutex);
  }
}

void toggleTimer() {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    const uint32_t now = millis();
    if (s_data.timer.state == TimerState::Running) {
      if (now < s_data.timer.targetEpochMs) {
        s_data.timer.remainingSec = (s_data.timer.targetEpochMs - now + 999) / 1000;
      } else {
        s_data.timer.remainingSec = 0;
      }
      s_data.timer.state = TimerState::Paused;
    } else if (s_data.timer.state == TimerState::Paused) {
      if (s_data.timer.remainingSec == 0) {
        s_data.timer.remainingSec = (s_data.timer.durationSec > 0) ? s_data.timer.durationSec : 300;
      }
      s_data.timer.targetEpochMs = now + (s_data.timer.remainingSec * 1000UL);
      s_data.timer.state = TimerState::Running;
    } else { // Stopped or Finished
      const uint32_t dur = (s_data.timer.durationSec > 0) ? s_data.timer.durationSec : 300;
      s_data.timer.durationSec = dur;
      s_data.timer.remainingSec = dur;
      s_data.timer.targetEpochMs = now + (dur * 1000UL);
      s_data.timer.state = TimerState::Running;
    }
    xSemaphoreGive(s_mutex);
  }
}

void syncClock(StopwatchState swState, uint32_t swElapsedMs,
               TimerState tmState, uint32_t tmDurationSec, uint32_t tmRemainingSec) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    const uint32_t now = millis();
    s_data.stopwatch.state = swState;
    s_data.stopwatch.elapsedMs = swElapsedMs;
    s_data.stopwatch.lastStartMs = now;

    s_data.timer.state = tmState;
    s_data.timer.durationSec = (tmDurationSec > 0) ? tmDurationSec : 300;
    s_data.timer.remainingSec = tmRemainingSec;
    if (tmState == TimerState::Running) {
      s_data.timer.targetEpochMs = now + (tmRemainingSec * 1000UL);
    }
    xSemaphoreGive(s_mutex);
  }
}

void tickClock(uint32_t nowMs) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(10)) == pdTRUE) {
    if (s_data.timer.state == TimerState::Running) {
      if (nowMs >= s_data.timer.targetEpochMs) {
        s_data.timer.remainingSec = 0;
        s_data.timer.state = TimerState::Finished;
      } else {
        s_data.timer.remainingSec = (s_data.timer.targetEpochMs - nowMs + 999) / 1000;
      }
    }
    xSemaphoreGive(s_mutex);
  }
}

uint32_t getStopwatchElapsedMs(uint32_t nowMs) {
  uint32_t el = 0;
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(10)) == pdTRUE) {
    el = s_data.stopwatch.elapsedMs;
    if (s_data.stopwatch.state == StopwatchState::Running) {
      el += (nowMs - s_data.stopwatch.lastStartMs);
    }
    xSemaphoreGive(s_mutex);
  }
  return el;
}

uint32_t getTimerRemainingSec(uint32_t nowMs) {
  uint32_t rem = 0;
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(10)) == pdTRUE) {
    if (s_data.timer.state == TimerState::Running) {
      if (nowMs >= s_data.timer.targetEpochMs) {
        rem = 0;
      } else {
        rem = (s_data.timer.targetEpochMs - nowMs + 999) / 1000;
      }
    } else {
      rem = s_data.timer.remainingSec;
    }
    xSemaphoreGive(s_mutex);
  }
  return rem;
}

uint32_t getTimerRemainingMs(uint32_t nowMs) {
  uint32_t rem = 0;
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(10)) == pdTRUE) {
    if (s_data.timer.state == TimerState::Running) {
      if (nowMs >= s_data.timer.targetEpochMs) {
        rem = 0;
      } else {
        rem = s_data.timer.targetEpochMs - nowMs;
      }
    } else {
      rem = s_data.timer.remainingSec * 1000UL;
    }
    xSemaphoreGive(s_mutex);
  }
  return rem;
}

void updateLyric(bool on, const char *line) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.lyricOn = on;
    copyText(s_data.lyric, on && line != nullptr ? line : "");
    s_data.lyricRxMs = millis();
    xSemaphoreGive(s_mutex);
  }
}

bool lyricShown(const DataSnapshot &d, uint32_t nowMs) {
  return d.lyricOn && d.mediaTitle[0] != '\0' && d.lyricRxMs != 0 &&
         static_cast<int32_t>(nowMs - d.lyricRxMs) < 12000;
}

void updatePc(PcLock lock, bool micKnown, bool micInUse, bool micMuted) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    s_data.pc.lock = lock;
    s_data.pc.micKnown = micKnown;
    s_data.pc.micInUse = micKnown && micInUse;
    s_data.pc.micMuted = micKnown && micMuted;
    xSemaphoreGive(s_mutex);
  }
}

PcState pcState(uint32_t nowMs) {
  PcState pc{};
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(10)) == pdTRUE) {
    pc = s_data.pc;
    // Signed: stats may land after the caller read the clock.
    if (s_data.lastStatsMs == 0 ||
        static_cast<int32_t>(nowMs - s_data.lastStatsMs) > 5000) {
      pc.micKnown = pc.micInUse = pc.micMuted = false;
    }
    xSemaphoreGive(s_mutex);
  }
  return pc;
}

void setMicMuted(bool muted) {
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(20)) == pdTRUE) {
    if (s_data.pc.micKnown) {
      s_data.pc.micMuted = muted;
    }
    xSemaphoreGive(s_mutex);
  }
}

bool isClockToolRunning() {
  bool running = false;
  if (s_mutex != nullptr &&
      xSemaphoreTake(s_mutex, pdMS_TO_TICKS(10)) == pdTRUE) {
    running = (s_data.stopwatch.state == StopwatchState::Running ||
               s_data.timer.state == TimerState::Running);
    xSemaphoreGive(s_mutex);
  }
  return running;
}

} // namespace DataStore
