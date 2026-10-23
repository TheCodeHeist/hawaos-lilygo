#pragma once

#include <lvgl.h>
#include <stdint.h>

namespace Display {

// Initialize power rail, ST7789V 8-bit parallel panel via TFT_eSPI, and LVGL
// with PSRAM draw buffer.
bool begin();

// Render an immediate boot splash screen and bring up the backlight.
// Must be called from setup() before the UI task starts (or from the UI task).
void showSplash();

// Set backlight brightness using LilyGo's 16-step pulse-dimming controller (0 =
// off, 1..16 = level).
void setBrightness(uint8_t level);

// Return current brightness level (0..16).
uint8_t getBrightness();

} // namespace Display
