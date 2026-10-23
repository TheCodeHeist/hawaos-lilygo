#pragma once

#include <stddef.h>
#include <stdint.h>

// =============================================================================
// HawaOS Hardware Configuration — LilyGo T-Display-S3 (Verified from LilyGo
// repo) Source:
// https://github.com/Xinyuan-LilyGO/T-Display-S3/blob/main/examples/factory/pin_config.h
// =============================================================================

namespace Config {

// --- Power & Backlight ---
constexpr uint8_t PIN_POWER_ON =
    15; // Must be HIGH to power LCD (especially on battery)
constexpr uint8_t PIN_LCD_BL = 38; // 16-step pulse-dimming backlight controller

// --- 8-bit Parallel ST7789V LCD Pins ---
constexpr uint8_t PIN_LCD_RES = 5;
constexpr uint8_t PIN_LCD_CS = 6;
constexpr uint8_t PIN_LCD_DC = 7;
constexpr uint8_t PIN_LCD_WR = 8;
constexpr uint8_t PIN_LCD_RD = 9;

constexpr uint8_t PIN_LCD_D0 = 39;
constexpr uint8_t PIN_LCD_D1 = 40;
constexpr uint8_t PIN_LCD_D2 = 41;
constexpr uint8_t PIN_LCD_D3 = 42;
constexpr uint8_t PIN_LCD_D4 = 45;
constexpr uint8_t PIN_LCD_D5 = 46;
constexpr uint8_t PIN_LCD_D6 = 47;
constexpr uint8_t PIN_LCD_D7 = 48;

// --- Buttons & Battery ADC ---
constexpr uint8_t PIN_BUTTON_1 =
    0; // BOOT button (active LOW) -> Button::Select
constexpr uint8_t PIN_BUTTON_2 = 14; // User button (active LOW) -> Button::Next
constexpr uint8_t PIN_BAT_VOLT = 4;  // Battery voltage divider ADC pin

// --- Button Timing ---
constexpr uint32_t BTN_POLL_PERIOD_MS = 10; // 100 Hz button sampling task
constexpr uint32_t BTN_DEBOUNCE_MS = 25;    // Debounce threshold
constexpr uint32_t BTN_LONG_PRESS_MS = 600; // Hold >= 600ms -> Press::Long
constexpr uint32_t BTN_DOUBLE_GAP_MS =
    250; // Second click within 250ms -> Press::Double

// --- I2C (STEMMA QT / Qwiic) ---
constexpr uint8_t PIN_IIC_SCL = 17;
constexpr uint8_t PIN_IIC_SDA = 18;

// --- Display Geometry (Landscape) ---
constexpr uint16_t LCD_HOR_RES = 320;
constexpr uint16_t LCD_VER_RES = 170;
constexpr uint16_t STATUS_BAR_HEIGHT = 24;
constexpr uint8_t LCD_ROTATION = 3;        // Landscape (USB port on left)
constexpr uint8_t DEFAULT_BRIGHTNESS = 14; // 0..16

// Full-screen draw buffer in PSRAM (320 * 170 pixels = 54,400 px = 108,800
// bytes)
constexpr uint32_t LVGL_BUF_PIXELS = (LCD_HOR_RES * LCD_VER_RES);

// --- Network & Time Defaults ---
constexpr int32_t DEFAULT_GMT_OFFSET_SEC = 6 * 3600; // UTC+6 (Asia/Dhaka)
constexpr int32_t DEFAULT_DST_OFFSET_SEC = 0;
constexpr const char *NTP_SERVER_1 = "pool.ntp.org";
constexpr const char *NTP_SERVER_2 = "time.google.com";
constexpr const char *NTP_SERVER_3 = "time.nist.gov";
constexpr uint32_t WIFI_RETRY_INTERVAL_MS = 5000;

// --- FreeRTOS Task & Queue Configuration ---
constexpr size_t EVENT_QUEUE_LENGTH = 32;
constexpr size_t MAX_REGISTERED_APPS = 8;

constexpr uint32_t UI_TASK_STACK_BYTES = 8192;
constexpr uint8_t UI_TASK_PRIORITY = 3;
constexpr uint8_t UI_TASK_CORE = 1;
constexpr uint32_t UI_TICK_PERIOD_MS = 5;   // 200 Hz lv_timer_handler pump
constexpr uint32_t APP_TICK_PERIOD_MS = 50; // 20 Hz App::onTick

constexpr uint32_t BTN_TASK_STACK_BYTES = 3072;
constexpr uint8_t BTN_TASK_PRIORITY = 4;
constexpr uint8_t BTN_TASK_CORE = 1;

constexpr uint32_t WIFI_TASK_STACK_BYTES = 4096;
constexpr uint8_t WIFI_TASK_PRIORITY = 2;
constexpr uint8_t WIFI_TASK_CORE = 0;

// --- Diagnostics ---
constexpr uint32_t HEAP_LOG_INTERVAL_MS = 10000; // Log heap & PSRAM every 10s
constexpr uint32_t SPLASH_DURATION_MS = 1200;    // Boot splash duration

} // namespace Config
