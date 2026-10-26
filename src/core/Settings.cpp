#include "core/Settings.h"
#include "config.h"
#include "services/CompanionLink.h"
#include "drivers/Buttons.h"
#include "drivers/Display.h"
#include "theme_tokens.h"

#include <Arduino.h>
#include <Preferences.h>
#include <string.h>
#include <time.h>

namespace Settings {

namespace {

constexpr const char *kNamespace = "hawaos";
Preferences s_prefs;

char s_wifiSsid[33] = {0};
char s_wifiPass[65] = {0};
uint8_t s_brightnessPct = Config::DEFAULT_BRIGHTNESS_PCT;
ThemeMode s_themeMode = ThemeMode::Dark;
// Read by the UI task while CompanionLink may write it; a byte store is atomic.
volatile uint8_t s_accent = ThemeTokens::kDefaultAccent;
bool s_aodEnabled = true;
uint16_t s_aodTimeoutSec = Config::DEFAULT_AOD_TIMEOUT_S;
bool s_reduceMotion = false;
WallpaperMode s_wallpaperMode = WallpaperMode::Gradient;
bool s_notifyPopupEnabled = true;
uint32_t s_configVersion = 0;
int32_t s_gmtOffsetSec = Config::DEFAULT_GMT_OFFSET_SEC;
bool s_use24Hour = false;
bool s_orientationInverted = false;
bool s_setupDone = false;

int8_t accentIndexOf(const char *id) {
  if (id == nullptr)
    return -1;
  for (uint8_t i = 0; i < ThemeTokens::kAccentCount; ++i) {
    if (strcmp(id, ThemeTokens::kAccentIds[i]) == 0)
      return static_cast<int8_t>(i);
  }
  return -1;
}

uint8_t snapBrightnessPct(uint8_t pct) {
  if (pct <= 25)
    return 20;
  if (pct <= 45)
    return 40;
  if (pct <= 65)
    return 60;
  if (pct <= 85)
    return 80;
  return 100;
}

} // namespace

void begin() {
  s_prefs.begin(kNamespace, false);

  // Clean up legacy testing keys ("wifi_ssid" / "wifi_pass") that were seeded
  // from secrets.h, and use dedicated Companion provisioning keys ("prov_ssid"
  // / "prov_pass").
  if (s_prefs.isKey("wifi_ssid") || s_prefs.isKey("wifi_pass")) {
    s_prefs.remove("wifi_ssid");
    s_prefs.remove("wifi_pass");
    Serial.println("[Settings] Cleared legacy testing WiFi keys; using "
                   "Companion provisioning.");
  }

  String storedSsid = s_prefs.getString("prov_ssid", "");
  String storedPass = s_prefs.getString("prov_pass", "");

  strncpy(s_wifiSsid, storedSsid.c_str(), sizeof(s_wifiSsid) - 1);
  s_wifiSsid[sizeof(s_wifiSsid) - 1] = '\0';

  strncpy(s_wifiPass, storedPass.c_str(), sizeof(s_wifiPass) - 1);
  s_wifiPass[sizeof(s_wifiPass) - 1] = '\0';

  s_brightnessPct = snapBrightnessPct(
      s_prefs.getUChar("bright_pct", Config::DEFAULT_BRIGHTNESS_PCT));
  const uint8_t rawTheme =
      s_prefs.getUChar("theme", static_cast<uint8_t>(ThemeMode::Dark));
  s_themeMode =
      (rawTheme <= 2) ? static_cast<ThemeMode>(rawTheme) : ThemeMode::Dark;
  // isKey() first: getString() on a missing key logs an NVS error.
  const String storedAccent =
      s_prefs.isKey("accent")
          ? s_prefs.getString("accent", "")
          : String(ThemeTokens::kAccentIds[ThemeTokens::kDefaultAccent]);
  const int8_t accentIdx = accentIndexOf(storedAccent.c_str());
  s_accent = (accentIdx >= 0) ? static_cast<uint8_t>(accentIdx)
                              : ThemeTokens::kDefaultAccent;
  s_aodEnabled = s_prefs.getBool("aod_en", true);
  s_aodTimeoutSec = s_prefs.getUShort("aod_sec", Config::DEFAULT_AOD_TIMEOUT_S);
  if (s_aodTimeoutSec < 10) {
    s_aodTimeoutSec = Config::DEFAULT_AOD_TIMEOUT_S;
  }
  s_reduceMotion = s_prefs.getBool("red_motion", false);
  s_wallpaperMode =
      static_cast<WallpaperMode>(s_prefs.getUChar("wallpaper", 0));
  s_notifyPopupEnabled = s_prefs.getBool("notif_pop", true);
  s_configVersion = s_prefs.getUInt("cfg_ver", 1);
  s_gmtOffsetSec = s_prefs.getInt("gmt_offset", Config::DEFAULT_GMT_OFFSET_SEC);
  s_use24Hour = s_prefs.getBool("use_24h", false);
  s_orientationInverted = s_prefs.getBool("orient_inv", false);
  // Devices provisioned before the setup wizard existed count as set up.
  s_setupDone = s_prefs.isKey("setup_done") ? s_prefs.getBool("setup_done", false)
                                            : (s_wifiSsid[0] != '\0');

  Serial.printf("[Settings] Loaded: SSID='%s', bright=%u%%, theme=%u, "
                "accent=%s, aod=%d(%us), redMotion=%d, notifyPopup=%d, v=%lu\n",
                s_wifiSsid, static_cast<unsigned>(s_brightnessPct),
                static_cast<unsigned>(s_themeMode), getAccentId(),
                s_aodEnabled ? 1 : 0,
                static_cast<unsigned>(s_aodTimeoutSec), s_reduceMotion ? 1 : 0,
                s_notifyPopupEnabled ? 1 : 0,
                static_cast<unsigned long>(s_configVersion));
}

void getWifiSsid(char *outBuf, size_t maxLen) {
  if (outBuf == nullptr || maxLen == 0)
    return;
  strncpy(outBuf, s_wifiSsid, maxLen - 1);
  outBuf[maxLen - 1] = '\0';
}

void getWifiPass(char *outBuf, size_t maxLen) {
  if (outBuf == nullptr || maxLen == 0)
    return;
  strncpy(outBuf, s_wifiPass, maxLen - 1);
  outBuf[maxLen - 1] = '\0';
}

void setWifiCredentials(const char *ssid, const char *pass) {
  if (ssid == nullptr || pass == nullptr)
    return;
  strncpy(s_wifiSsid, ssid, sizeof(s_wifiSsid) - 1);
  s_wifiSsid[sizeof(s_wifiSsid) - 1] = '\0';

  strncpy(s_wifiPass, pass, sizeof(s_wifiPass) - 1);
  s_wifiPass[sizeof(s_wifiPass) - 1] = '\0';

  s_prefs.putString("prov_ssid", s_wifiSsid);
  s_prefs.putString("prov_pass", s_wifiPass);
  Serial.printf(
      "[Settings] Saved provisioned WiFi credentials to NVS (SSID: '%s')\n",
      s_wifiSsid);
}

void clearWifiCredentials() {
  s_wifiSsid[0] = '\0';
  s_wifiPass[0] = '\0';
  s_prefs.remove("prov_ssid");
  s_prefs.remove("prov_pass");
  Serial.println("[Settings] Cleared provisioned WiFi credentials from NVS");
}

uint8_t getBrightnessPct() { return s_brightnessPct; }

uint8_t getBrightnessStep() {
  // Map 20/40/60/80/100% -> 3/6/10/13/16 hardware steps (1..16)
  switch (s_brightnessPct) {
  case 20:
    return 3;
  case 40:
    return 6;
  case 60:
    return 10;
  case 80:
    return 13;
  case 100:
    return 16;
  default:
    return 13;
  }
}

void setBrightnessPct(uint8_t pct) {
  s_brightnessPct = snapBrightnessPct(pct);
  s_prefs.putUChar("bright_pct", s_brightnessPct);
}

void cycleBrightnessPct() {
  uint8_t next = 20;
  if (s_brightnessPct < 40)
    next = 40;
  else if (s_brightnessPct < 60)
    next = 60;
  else if (s_brightnessPct < 80)
    next = 80;
  else if (s_brightnessPct < 100)
    next = 100;
  else
    next = 20;
  setBrightnessPct(next);
  bumpLocalConfigVersion();
}

ThemeMode getThemeMode() { return s_themeMode; }

bool isDarkThemeEffective() {
  if (s_themeMode == ThemeMode::Dark)
    return true;
  if (s_themeMode == ThemeMode::Light)
    return false;
  // Auto mode via NTP time: Dark from 18:00 to 06:59, Light from 07:00 to 17:59
  const time_t now = time(nullptr);
  if (now > 1700000000) {
    struct tm ti{};
    localtime_r(&now, &ti);
    return (ti.tm_hour < 7 || ti.tm_hour >= 18);
  }
  return true;
}

void setThemeMode(ThemeMode mode) {
  s_themeMode = mode;
  s_prefs.putUChar("theme", static_cast<uint8_t>(mode));
}

void toggleDarkLightTheme() {
  const bool currentlyDark = isDarkThemeEffective();
  setThemeMode(currentlyDark ? ThemeMode::Light : ThemeMode::Dark);
  bumpLocalConfigVersion();
}

uint8_t getAccent() { return s_accent; }

void setAccent(uint8_t idx) {
  if (idx >= ThemeTokens::kAccentCount)
    idx = ThemeTokens::kDefaultAccent;
  if (idx == s_accent)
    return;
  s_accent = idx;
  s_prefs.putString("accent", ThemeTokens::kAccentIds[idx]);
}

bool setAccentById(const char *id) {
  const int8_t idx = accentIndexOf(id);
  if (idx < 0)
    return false;
  setAccent(static_cast<uint8_t>(idx));
  return true;
}

const char *getAccentId() { return ThemeTokens::kAccentIds[s_accent]; }

void cycleAccent() {
  setAccent(static_cast<uint8_t>((s_accent + 1) % ThemeTokens::kAccentCount));
  bumpLocalConfigVersion();
}

bool getAodEnabled() { return s_aodEnabled; }

void setAodEnabled(bool enabled) {
  s_aodEnabled = enabled;
  s_prefs.putBool("aod_en", s_aodEnabled);
}

uint16_t getAodTimeoutSec() { return s_aodTimeoutSec; }

void setAodTimeoutSec(uint16_t sec) {
  if (sec < 10)
    sec = 10;
  s_aodTimeoutSec = sec;
  s_prefs.putUShort("aod_sec", s_aodTimeoutSec);
}

bool getReduceMotion() { return s_reduceMotion; }

void setReduceMotion(bool reduce) {
  s_reduceMotion = reduce;
  s_prefs.putBool("red_motion", s_reduceMotion);
}

WallpaperMode getWallpaperMode() { return s_wallpaperMode; }

void setWallpaperMode(WallpaperMode mode) {
  s_wallpaperMode = mode;
  s_prefs.putUChar("wallpaper", static_cast<uint8_t>(mode));
}

bool getNotifyPopupEnabled() { return s_notifyPopupEnabled; }

void setNotifyPopupEnabled(bool enabled) {
  s_notifyPopupEnabled = enabled;
  s_prefs.putBool("notif_pop", s_notifyPopupEnabled);
}

uint32_t getConfigVersion() { return s_configVersion; }

void setConfigVersion(uint32_t version) {
  s_configVersion = version;
  s_prefs.putUInt("cfg_ver", s_configVersion);
}

void bumpLocalConfigVersion() {
  ++s_configVersion;
  s_prefs.putUInt("cfg_ver", s_configVersion);
  CompanionLink::sendConfigAck(s_configVersion);
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

bool getOrientationInverted() { return s_orientationInverted; }

void setOrientationInverted(bool inverted) {
  s_orientationInverted = inverted;
  s_prefs.putBool("orient_inv", s_orientationInverted);
  Display::setRotation(s_orientationInverted ? 1 : 3);
  Buttons::setOrientationInverted(s_orientationInverted);
  bumpLocalConfigVersion();
}

bool isSetupDone() { return s_setupDone; }

void setSetupDone(bool done) {
  s_setupDone = done;
  s_prefs.putBool("setup_done", s_setupDone);
}

void getCompanionToken(char *outBuf, size_t maxLen) {
  if (outBuf == nullptr || maxLen == 0)
    return;
  outBuf[0] = '\0';
  if (s_prefs.isKey("cmp_token")) {
    s_prefs.getString("cmp_token", outBuf, maxLen);
  }
}

void setCompanionToken(const char *hex) {
  s_prefs.putString("cmp_token", hex != nullptr ? hex : "");
  s_prefs.remove("auth_n");
}

uint64_t getAuthCounter() { return s_prefs.getULong64("auth_n", 0); }

void setAuthCounter(uint64_t n) { s_prefs.putULong64("auth_n", n); }

} // namespace Settings
