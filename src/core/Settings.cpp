#include "core/Settings.h"
#include "config.h"

#include <Arduino.h>
#include <Preferences.h>
#include <string.h>

#if __has_include("secrets.h")
#include "secrets.h"
#endif

namespace Settings {

namespace {

constexpr const char *kNamespace = "hawaos";
Preferences s_prefs;

char s_wifiSsid[33] = {0};
char s_wifiPass[65] = {0};
uint8_t s_brightness = Config::DEFAULT_BRIGHTNESS;
int32_t s_gmtOffsetSec = Config::DEFAULT_GMT_OFFSET_SEC;
bool s_use24Hour = false;

} // namespace

void begin() {
  s_prefs.begin(kNamespace, false);

  String storedSsid = s_prefs.getString("wifi_ssid", "");
  String storedPass = s_prefs.getString("wifi_pass", "");

#if defined(SECRET_WIFI_SSID) && defined(SECRET_WIFI_PASS)
  if (storedSsid.isEmpty() && strlen(SECRET_WIFI_SSID) > 0) {
    storedSsid = SECRET_WIFI_SSID;
    storedPass = SECRET_WIFI_PASS;
    s_prefs.putString("wifi_ssid", storedSsid);
    s_prefs.putString("wifi_pass", storedPass);
    Serial.printf(
        "[Settings] Seeded NVS WiFi credentials from secrets.h (SSID: %s)\n",
        storedSsid.c_str());
  }
#endif

  strncpy(s_wifiSsid, storedSsid.c_str(), sizeof(s_wifiSsid) - 1);
  s_wifiSsid[sizeof(s_wifiSsid) - 1] = '\0';

  strncpy(s_wifiPass, storedPass.c_str(), sizeof(s_wifiPass) - 1);
  s_wifiPass[sizeof(s_wifiPass) - 1] = '\0';

  s_brightness = s_prefs.getUChar("brightness", Config::DEFAULT_BRIGHTNESS);
  if (s_brightness == 0 || s_brightness > 16) {
    s_brightness = Config::DEFAULT_BRIGHTNESS;
  }
  s_gmtOffsetSec = s_prefs.getInt("gmt_offset", Config::DEFAULT_GMT_OFFSET_SEC);
  s_use24Hour = s_prefs.getBool("use_24h", false);

  Serial.printf(
      "[Settings] Loaded: SSID='%s', brightness=%u, GMT=%+ldh, 24h=%d\n",
      s_wifiSsid, static_cast<unsigned>(s_brightness),
      static_cast<long>(s_gmtOffsetSec / 3600), s_use24Hour ? 1 : 0);
}

void getWifiSsid(char *outBuf, size_t maxLen) {
  if (outBuf == nullptr || maxLen == 0) {
    return;
  }
  strncpy(outBuf, s_wifiSsid, maxLen - 1);
  outBuf[maxLen - 1] = '\0';
}

void getWifiPass(char *outBuf, size_t maxLen) {
  if (outBuf == nullptr || maxLen == 0) {
    return;
  }
  strncpy(outBuf, s_wifiPass, maxLen - 1);
  outBuf[maxLen - 1] = '\0';
}

void setWifiCredentials(const char *ssid, const char *pass) {
  if (ssid == nullptr || pass == nullptr) {
    return;
  }
  strncpy(s_wifiSsid, ssid, sizeof(s_wifiSsid) - 1);
  s_wifiSsid[sizeof(s_wifiSsid) - 1] = '\0';

  strncpy(s_wifiPass, pass, sizeof(s_wifiPass) - 1);
  s_wifiPass[sizeof(s_wifiPass) - 1] = '\0';

  s_prefs.putString("wifi_ssid", s_wifiSsid);
  s_prefs.putString("wifi_pass", s_wifiPass);
  Serial.printf("[Settings] Saved new WiFi credentials to NVS (SSID: '%s')\n",
                s_wifiSsid);
}

uint8_t getBrightness() { return s_brightness; }

void setBrightness(uint8_t level) {
  if (level > 16) {
    level = 16;
  }
  s_brightness = level;
  s_prefs.putUChar("brightness", s_brightness);
}

int32_t getGmtOffsetSec() { return s_gmtOffsetSec; }

void setGmtOffsetSec(int32_t offsetSec) {
  s_gmtOffsetSec = offsetSec;
  s_prefs.putInt("gmt_offset", s_gmtOffsetSec);
}

bool getUse24Hour() { return s_use24Hour; }

void setUse24Hour(bool use24h) {
  s_use24Hour = use24h;
  s_prefs.putBool("use_24h", s_use24Hour);
}

} // namespace Settings
