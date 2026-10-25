#pragma once

#include <lvgl.h>
#include <stdint.h>

namespace Display {

// Initialize power rail, ST7789V 8-bit parallel panel via TFT_eSPI, draw direct
// boot logo, ramp backlight over 300ms (hiding power-on flash), and initialize
// LVGL with PSRAM draw buffer.
bool begin();

// Set backlight brightness using LilyGo's 16-step pulse-dimming controller (0 =
// off, 1..16 = level).
void setBrightness(uint8_t level);

// Smoothly ramp backlight from 0 to targetStep over durationMs (DESIGN.md
// Section 12).
void rampBrightness(uint8_t targetStep, uint32_t durationMs);

// Return current brightness level (0..16).
uint8_t getBrightness();

// Set panel orientation (3 = normal landscape, 1 = inverted 180° landscape).
void setRotation(uint8_t rotation);

} // namespace Display
