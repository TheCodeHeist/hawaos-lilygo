#pragma once

#include <stddef.h>
#include <stdint.h>

enum class ThemeMode : uint8_t {
  Dark = 0,
  Light = 1,
  Auto = 2,
};

enum class WallpaperMode : uint8_t {
  Gradient = 0,
  Solid = 1,
};

namespace Settings {

void begin();

void getWifiSsid(char *outBuf, size_t maxLen);
void getWifiPass(char *outBuf, size_t maxLen);
void setWifiCredentials(const char *ssid, const char *pass);
void clearWifiCredentials();

// Brightness stored as percentage (20, 40, 60, 80, 100) and mapped to 1..16
// hardware steps
uint8_t getBrightnessPct();
uint8_t getBrightnessStep();
void setBrightnessPct(uint8_t pct);
void cycleBrightnessPct(); // Cycles 20 -> 40 -> 60 -> 80 -> 100 -> 20

ThemeMode getThemeMode();
bool isDarkThemeEffective();
void setThemeMode(ThemeMode mode);
void toggleDarkLightTheme();

bool getAodEnabled();
void setAodEnabled(bool enabled);

uint16_t getAodTimeoutSec();
void setAodTimeoutSec(uint16_t sec);

bool getReduceMotion();
void setReduceMotion(bool reduce);

WallpaperMode getWallpaperMode();
void setWallpaperMode(WallpaperMode mode);

bool getNotifyPopupEnabled();
void setNotifyPopupEnabled(bool enabled);

uint32_t getConfigVersion();
void setConfigVersion(uint32_t version);
void bumpLocalConfigVersion();

int32_t getGmtOffsetSec();
void setGmtOffsetSec(int32_t offsetSec);

bool getUse24Hour();
void setUse24Hour(bool use24h);

bool getOrientationInverted();
void setOrientationInverted(bool inverted);

} // namespace Settings
