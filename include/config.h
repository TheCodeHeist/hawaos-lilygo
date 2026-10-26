#pragma once

#include <stddef.h>
#include <stdint.h>

// =============================================================================
// HawaOS Hardware & UI Configuration — LilyGo T-Display-S3
// Verified against LilyGo repo & DESIGN.md
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

// --- Buttons & Battery ADC (Verified Section 14.2) ---
// Landscape with USB-C on left: Top button (GPIO 0) = Select, Bottom button
// (GPIO 14) = Next
constexpr uint8_t PIN_BUTTON_SELECT =
    0; // Top button (BOOT, active LOW) -> Button::Select
constexpr uint8_t PIN_BUTTON_NEXT =
    14; // Bottom button (active LOW)    -> Button::Next
constexpr uint8_t PIN_BAT_VOLT = 4; // Battery voltage divider ADC pin

// --- Gesture Timing (DESIGN.md Section 8) ---
constexpr uint32_t BTN_POLL_PERIOD_MS = 5; // 200 Hz button sampling task
constexpr uint32_t BTN_DEBOUNCE_MS = 20;   // Debounce threshold
constexpr uint32_t BTN_CHORD_WINDOW_MS =
    80; // Both buttons held within 80ms -> Chord
constexpr uint32_t BTN_LONG_PRESS_MS = 500; // Hold >= 500ms -> Press::Long
constexpr uint32_t BTN_DOUBLE_GAP_MS =
    250; // Second click within 250ms (only if app opts in)

// --- I2C (STEMMA QT / Qwiic) ---
constexpr uint8_t PIN_IIC_SCL = 17;
constexpr uint8_t PIN_IIC_SDA = 18;

// --- Display & Layout Grid (DESIGN.md Section 2) ---
constexpr uint16_t LCD_HOR_RES = 320;
constexpr uint16_t LCD_VER_RES = 170;
constexpr uint8_t LCD_ROTATION = 3; // Landscape (USB-C port on left)
constexpr uint16_t BASE_UNIT_PX = 4;
constexpr uint16_t OUTER_MARGIN_PX = 10;
constexpr uint16_t STATUS_BAR_HEIGHT =
    22; // 22 px top status bar, 320x148 content below
constexpr uint16_t CONTENT_Y_START = STATUS_BAR_HEIGHT;
constexpr uint16_t CONTENT_HEIGHT = LCD_VER_RES - STATUS_BAR_HEIGHT; // 148 px

constexpr uint8_t RADIUS_SM = 8;        // Chips, small cards
constexpr uint8_t RADIUS_MD = 12;       // Cards, tiles
constexpr uint8_t RADIUS_LG = 16;       // Launcher tiles (88x88)
constexpr uint8_t FOCUS_RING_WIDTH = 2; // 2 px accent focus ring

// --- Motion Durations (DESIGN.md Section 7) ---
constexpr uint16_t ANIM_FOCUS_MS = 120;
constexpr uint16_t ANIM_VALUE_MS = 150;
constexpr uint16_t ANIM_TOAST_MS = 150;
constexpr uint16_t ANIM_BOOT_RAMP_MS = 300;
constexpr uint16_t ANIM_AOD_ENTER_MS = 450;
constexpr uint16_t ANIM_AOD_EXIT_MS = 180;

// --- Backlight & AOD Defaults ---
constexpr uint8_t DEFAULT_BRIGHTNESS_PCT =
    80; // 20/40/60/80/100% mapped to 1..16 steps
constexpr uint8_t AOD_BRIGHTNESS_STEP = 3;     // ~18% brightness in AOD
constexpr uint16_t DEFAULT_AOD_TIMEOUT_S = 60; // Enter AOD after 60s idle
constexpr uint32_t SPLASH_MIN_DURATION_MS =
    3000; // Minimum boot logo display duration (3.0s)

// Full-screen draw buffer in PSRAM (320 * 170 pixels = 54,400 px = 108,800
// bytes)
constexpr uint32_t LVGL_BUF_PIXELS = (LCD_HOR_RES * LCD_VER_RES);

// --- Network, Companion & Time Defaults ---
constexpr uint16_t COMPANION_UDP_PORT = 7777;
// USB serial RX buffer. The default 256 B overflows between 25 ms polls when the
// companion streams a weather message with forecast (~400 B) over USB.
constexpr size_t USB_RX_BUFFER_BYTES = 2048;
constexpr uint16_t COMPANION_ART_PORT = 7778;
constexpr int32_t DEFAULT_GMT_OFFSET_SEC = 6 * 3600; // UTC+6 (Asia/Dhaka)
constexpr int32_t DEFAULT_DST_OFFSET_SEC = 0;
constexpr const char *NTP_SERVER_1 = "pool.ntp.org";
constexpr const char *NTP_SERVER_2 = "time.google.com";
constexpr const char *NTP_SERVER_3 = "time.nist.gov";

// --- FreeRTOS Task & Queue Configuration ---
constexpr size_t EVENT_QUEUE_LENGTH = 32;
constexpr size_t MAX_REGISTERED_APPS = 8;

constexpr uint32_t UI_TASK_STACK_BYTES = 10240;
constexpr uint8_t UI_TASK_PRIORITY = 3;
constexpr uint8_t UI_TASK_CORE = 1;
constexpr uint32_t UI_TICK_ACTIVE_MS = 5; // 200 Hz lv_timer_handler when active
constexpr uint32_t UI_TICK_AOD_MS =
    200; // 5 Hz lv_timer_handler in AOD (DESIGN.md 10)
constexpr uint32_t APP_TICK_PERIOD_MS = 50; // 20 Hz App::onTick

constexpr uint32_t BTN_TASK_STACK_BYTES = 3072;
constexpr uint8_t BTN_TASK_PRIORITY = 4;
constexpr uint8_t BTN_TASK_CORE = 1;

constexpr uint32_t WIFI_TASK_STACK_BYTES = 8192;
constexpr uint8_t WIFI_TASK_PRIORITY = 2;
constexpr uint8_t WIFI_TASK_CORE = 0;

// --- Diagnostics ---
constexpr uint32_t HEAP_LOG_INTERVAL_MS = 10000;

} // namespace Config
