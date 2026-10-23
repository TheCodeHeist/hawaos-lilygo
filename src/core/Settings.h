#pragma once

#include <stddef.h>
#include <stdint.h>

namespace Settings {

void begin();

void getWifiSsid(char *outBuf, size_t maxLen);
void getWifiPass(char *outBuf, size_t maxLen);
void setWifiCredentials(const char *ssid, const char *pass);

uint8_t getBrightness();
void setBrightness(uint8_t level);

int32_t getGmtOffsetSec();
void setGmtOffsetSec(int32_t offsetSec);

bool getUse24Hour();
void setUse24Hour(bool use24h);

} // namespace Settings
