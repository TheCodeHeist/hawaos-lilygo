#include "drivers/Display.h"
#include "boot_logo.h"
#include "config.h"
#include "core/Settings.h"
#include "theme_tokens.h"

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

// Blends `fg` (RGB888) at 4-bit coverage `a4` over `bg` (RGB888).
uint32_t blend888(uint32_t fg, uint8_t a4, uint32_t bg) {
  if (a4 == 0)
    return bg;
  const uint32_t a = a4 * 17U;
  const uint32_t inv = 255U - a;
  uint32_t out = 0;
  for (int shift = 0; shift <= 16; shift += 8) {
    const uint32_t c = (((fg >> shift) & 0xFF) * a + ((bg >> shift) & 0xFF) * inv + 127U) / 255U;
    out |= c << shift;
  }
  return out;
}

uint8_t a4At(const uint8_t *layer, uint32_t i) {
  const uint8_t b = pgm_read_byte(&layer[i / 2]);
  return (i % 2 == 0) ? (b >> 4) : (b & 0x0F);
}

void drawDirectBootLogo() {
  // The lockup is coverage only (gen_logo.py): the mark and "OS" in the
  // accent, "Hawa" in the text colour, the tagline dimmed, all over the
  // accent-tinted `bg` of the saved theme.
  const bool useDark = Settings::isDarkThemeEffective();
  const ThemeTokens::Palette &p =
      ThemeTokens::kPalettes[Settings::getAccent()][useDark ? 0 : 1];

  s_tft.fillScreen(p.bg565);
  const int16_t x = (Config::LCD_HOR_RES - BootLogo::WIDTH) / 2;
  const int16_t y = (Config::LCD_VER_RES - BootLogo::HEIGHT) / 2;

  uint16_t row[BootLogo::WIDTH];
  s_tft.setSwapBytes(true);
  s_tft.startWrite();
  for (uint16_t ry = 0; ry < BootLogo::HEIGHT; ++ry) {
    const uint32_t base = static_cast<uint32_t>(ry) * BootLogo::WIDTH;
    for (uint16_t rx = 0; rx < BootLogo::WIDTH; ++rx) {
      const uint32_t i = base + rx;
      uint32_t c = p.bg;
      c = blend888(p.textDim, a4At(BootLogo::kDimA4, i), c);
      c = blend888(p.text, a4At(BootLogo::kTextA4, i), c);
      c = blend888(p.accent, a4At(BootLogo::kAccentA4, i), c);
      row[rx] = static_cast<uint16_t>(((c >> 8) & 0xF800) | ((c >> 5) & 0x07E0) |
                                      ((c & 0xFF) >> 3));
    }
    s_tft.pushImage(x, y + ry, BootLogo::WIDTH, 1, row);
  }
  s_tft.endWrite();
  s_tft.setSwapBytes(false);
}

} // namespace

bool begin() {
  // 1. Keep backlight OFF first (DESIGN.md Section 12: hides white power-on
  // flash)
  pinMode(Config::PIN_LCD_BL, OUTPUT);
  digitalWrite(Config::PIN_LCD_BL, LOW);
  s_brightness = 0;

  // 2. Enable board power rail (GPIO 15 MUST be HIGH for battery operation &
  // LCD power)
  pinMode(Config::PIN_POWER_ON, OUTPUT);
  digitalWrite(Config::PIN_POWER_ON, HIGH);

  // 3. Keep LCD RD pin HIGH (write-only 8-bit parallel operation)
  pinMode(Config::PIN_LCD_RD, OUTPUT);
  digitalWrite(Config::PIN_LCD_RD, HIGH);

  // 4. Initialize TFT_eSPI (configured via Setup206 build_flags)
  s_tft.begin();

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

  const uint8_t initialRot = Settings::getOrientationInverted() ? 1 : Config::LCD_ROTATION;
  s_tft.setRotation(initialRot);

  // 5. Draw theme-matched `bg` + centered multi-color boot logo directly before
  // LVGL starts
  drawDirectBootLogo();

  // 6. Ramp backlight from 0 to saved user brightness over 300ms (DESIGN.md
  // Section 12.2)
  rampBrightness(Settings::getBrightnessStep(), Config::ANIM_BOOT_RAMP_MS);

  // 7. Initialize LVGL and allocate full-screen draw buffer in PSRAM behind the
  // visible boot logo
  lv_init();

  const size_t bufBytes = Config::LVGL_BUF_PIXELS * sizeof(lv_color_t);
  s_buf1 = static_cast<lv_color_t *>(
      heap_caps_malloc(bufBytes, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT));

  if (s_buf1 != nullptr) {
    Serial.printf("[Display] Allocated LVGL draw buffer in PSRAM (%u bytes)\n",
                  static_cast<unsigned>(bufBytes));
    lv_disp_draw_buf_init(&s_drawBuf, s_buf1, nullptr, Config::LVGL_BUF_PIXELS);
  } else {
    const size_t fallbackPixels = Config::LCD_HOR_RES * 34;
    s_buf1 = static_cast<lv_color_t *>(
        heap_caps_malloc(fallbackPixels * sizeof(lv_color_t),
                         MALLOC_CAP_INTERNAL | MALLOC_CAP_8BIT));
    if (s_buf1 == nullptr) {
      Serial.println("[Display] FATAL: Could not allocate LVGL draw buffer!");
      return false;
    }
    lv_disp_draw_buf_init(&s_drawBuf, s_buf1, nullptr, fallbackPixels);
  }

  lv_disp_drv_init(&s_dispDrv);
  s_dispDrv.hor_res = Config::LCD_HOR_RES;
  s_dispDrv.ver_res = Config::LCD_VER_RES;
  s_dispDrv.flush_cb = flushCb;
  s_dispDrv.draw_buf = &s_drawBuf;
  lv_disp_drv_register(&s_dispDrv);

  return true;
}

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

void rampBrightness(uint8_t targetStep, uint32_t durationMs) {
  if (targetStep == 0 || Settings::getReduceMotion() || durationMs == 0) {
    setBrightness(targetStep);
    return;
  }
  if (targetStep > 16)
    targetStep = 16;

  const uint32_t stepDelayMs = durationMs / targetStep;
  for (uint8_t step = 1; step <= targetStep; ++step) {
    setBrightness(step);
    if (stepDelayMs > 0) {
      delay(stepDelayMs);
    }
  }
}

uint8_t getBrightness() { return s_brightness; }

void setRotation(uint8_t rotation) {
  s_tft.setRotation(rotation);
  if (lv_scr_act() != nullptr) {
    lv_obj_invalidate(lv_scr_act());
  }
  if (lv_layer_top() != nullptr) {
    lv_obj_invalidate(lv_layer_top());
  }
}

} // namespace Display
