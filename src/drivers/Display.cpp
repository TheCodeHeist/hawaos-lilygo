#include "drivers/Display.h"
#include "config.h"

#include <Arduino.h>
#include <TFT_eSPI.h>
#include <esp_heap_caps.h>

namespace Display {

namespace {

TFT_eSPI s_tft = TFT_eSPI();
lv_disp_draw_buf_t s_drawBuf;
lv_disp_drv_t s_dispDrv;
lv_color_t *s_buf1 = nullptr;
uint8_t s_brightness = 0;

struct LcdCmd {
  uint8_t cmd;
  uint8_t data[14];
  uint8_t len; // Bit 7 (0x80) indicates 120ms post-command delay
};

// Verified from LilyGo T-Display-S3 examples/tft/tft.ino &
// examples/factory/factory.ino (LCD_MODULE_CMD_1)
constexpr LcdCmd kSt7789vInitCmds[] = {
    {0x11, {0}, 0 | 0x80},
    {0x3A, {0x05}, 1},
    {0xB2, {0x0B, 0x0B, 0x00, 0x33, 0x33}, 5},
    {0xB7, {0x75}, 1},
    {0xBB, {0x28}, 1},
    {0xC0, {0x2C}, 1},
    {0xC2, {0x01}, 1},
    {0xC3, {0x1F}, 1},
    {0xC6, {0x13}, 1},
    {0xD0, {0xA7}, 1},
    {0xD0, {0xA4, 0xA1}, 2},
    {0xD6, {0xA1}, 1},
    {0xE0,
     {0xF0, 0x05, 0x0A, 0x06, 0x06, 0x03, 0x2B, 0x32, 0x43, 0x36, 0x11, 0x10,
      0x2B, 0x32},
     14},
    {0xE1,
     {0xF0, 0x08, 0x0C, 0x0B, 0x09, 0x24, 0x2B, 0x22, 0x43, 0x38, 0x15, 0x16,
      0x2F, 0x37},
     14},
};

void flushCb(lv_disp_drv_t *disp, const lv_area_t *area, lv_color_t *color_p) {
  const uint32_t w = (area->x2 - area->x1 + 1);
  const uint32_t h = (area->y2 - area->y1 + 1);

  s_tft.startWrite();
  s_tft.setAddrWindow(area->x1, area->y1, w, h);
  s_tft.pushColors(reinterpret_cast<uint16_t *>(&color_p->full), w * h, true);
  s_tft.endWrite();

  lv_disp_flush_ready(disp);
}

} // namespace

bool begin() {
  // 1. Enable board power rail (GPIO 15 MUST be HIGH for battery operation &
  // LCD power)
  pinMode(Config::PIN_POWER_ON, OUTPUT);
  digitalWrite(Config::PIN_POWER_ON, HIGH);

  // 2. Keep LCD RD pin HIGH (write-only 8-bit parallel operation)
  pinMode(Config::PIN_LCD_RD, OUTPUT);
  digitalWrite(Config::PIN_LCD_RD, HIGH);

  // Keep backlight off during panel init to prevent startup glitch
  pinMode(Config::PIN_LCD_BL, OUTPUT);
  digitalWrite(Config::PIN_LCD_BL, LOW);

  // 3. Initialize TFT_eSPI (configured via Setup206 build_flags)
  s_tft.begin();

  // Apply LilyGo's updated ST7789V gamma/voltage table (LCD_MODULE_CMD_1)
  for (size_t i = 0; i < sizeof(kSt7789vInitCmds) / sizeof(kSt7789vInitCmds[0]);
       ++i) {
    s_tft.writecommand(kSt7789vInitCmds[i].cmd);
    const uint8_t dataLen = kSt7789vInitCmds[i].len & 0x7F;
    for (uint8_t j = 0; j < dataLen; ++j) {
      s_tft.writedata(kSt7789vInitCmds[i].data[j]);
    }
    if (kSt7789vInitCmds[i].len & 0x80) {
      delay(120);
    }
  }

  s_tft.setRotation(Config::LCD_ROTATION);
  s_tft.fillScreen(TFT_BLACK);

  // 4. Initialize LVGL and allocate draw buffer in PSRAM
  lv_init();

  const size_t bufBytes = Config::LVGL_BUF_PIXELS * sizeof(lv_color_t);
  s_buf1 = static_cast<lv_color_t *>(
      heap_caps_malloc(bufBytes, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT));

  if (s_buf1 != nullptr) {
    Serial.printf("[Display] Allocated LVGL draw buffer in PSRAM (%u bytes)\n",
                  static_cast<unsigned>(bufBytes));
  } else {
    // Fallback to internal RAM (1/5 screen height) only if PSRAM allocation
    // fails
    const size_t fallbackPixels = Config::LCD_HOR_RES * 34;
    s_buf1 = static_cast<lv_color_t *>(
        heap_caps_malloc(fallbackPixels * sizeof(lv_color_t),
                         MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT));
    Serial.printf("[Display] WARNING: PSRAM alloc failed; using internal "
                  "buffer (%u px)\n",
                  static_cast<unsigned>(fallbackPixels));
    if (s_buf1 == nullptr) {
      Serial.println("[Display] FATAL: Could not allocate LVGL draw buffer!");
      return false;
    }
    lv_disp_draw_buf_init(&s_drawBuf, s_buf1, nullptr, fallbackPixels);
    goto register_drv;
  }

  lv_disp_draw_buf_init(&s_drawBuf, s_buf1, nullptr, Config::LVGL_BUF_PIXELS);

register_drv:
  lv_disp_drv_init(&s_dispDrv);
  s_dispDrv.hor_res = Config::LCD_HOR_RES;
  s_dispDrv.ver_res = Config::LCD_VER_RES;
  s_dispDrv.flush_cb = flushCb;
  s_dispDrv.draw_buf = &s_drawBuf;
  lv_disp_drv_register(&s_dispDrv);

  return true;
}

void showSplash() {
  lv_obj_t *scr = lv_scr_act();
  lv_obj_clean(scr);
  lv_obj_set_style_bg_color(scr, lv_color_hex(0x0B0F19), LV_PART_MAIN);
  lv_obj_set_style_bg_opa(scr, LV_OPA_COVER, LV_PART_MAIN);

  // Title
  lv_obj_t *title = lv_label_create(scr);
  lv_label_set_text(title, "HawaOS");
  lv_obj_set_style_text_font(title, &lv_font_montserrat_32, LV_PART_MAIN);
  lv_obj_set_style_text_color(title, lv_color_hex(0x38BDF8), LV_PART_MAIN);
  lv_obj_align(title, LV_ALIGN_CENTER, 0, -18);

  // Subtitle
  lv_obj_t *sub = lv_label_create(scr);
  lv_label_set_text(sub, "LilyGo T-Display S3  |  v0.1.0");
  lv_obj_set_style_text_font(sub, &lv_font_montserrat_14, LV_PART_MAIN);
  lv_obj_set_style_text_color(sub, lv_color_hex(0x94A3B8), LV_PART_MAIN);
  lv_obj_align(sub, LV_ALIGN_CENTER, 0, 18);

  // Flush the splash frame before turning on the backlight
  lv_timer_handler();
  setBrightness(Config::DEFAULT_BRIGHTNESS);
}

// Verified from LilyGo T-Display-S3 examples/factory/factory.ino
// The backlight IC on GPIO 38 has 16 pulse-stepped brightness levels (0 = off,
// 16 = max)
void setBrightness(uint8_t value) {
  constexpr uint8_t kSteps = 16;
  if (value > kSteps) {
    value = kSteps;
  }

  if (value == 0) {
    digitalWrite(Config::PIN_LCD_BL, LOW);
    delay(3);
    s_brightness = 0;
    return;
  }

  if (s_brightness == 0) {
    digitalWrite(Config::PIN_LCD_BL, HIGH);
    s_brightness = kSteps;
    delayMicroseconds(30);
  }

  const int from = kSteps - s_brightness;
  const int to = kSteps - value;
  const int num = (kSteps + to - from) % kSteps;
  for (int i = 0; i < num; ++i) {
    digitalWrite(Config::PIN_LCD_BL, LOW);
    digitalWrite(Config::PIN_LCD_BL, HIGH);
  }
  s_brightness = value;
}

uint8_t getBrightness() { return s_brightness; }

} // namespace Display
