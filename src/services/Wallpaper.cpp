#include "services/Wallpaper.h"

#include <Arduino.h>
#include <esp_heap_caps.h>
#include <esp_partition.h>
#include <esp_rom_crc.h>
#include <string.h>

namespace Wallpaper {

namespace {

// Partition layout: header in the first sector, pixels from the second. The
// header is written last, so an interrupted save reads back as no picture.
constexpr uint32_t kMagic = 0x31505748; // "HWP1"
constexpr size_t kPixelsAt = 4096;

struct Header {
  uint32_t magic;
  uint16_t width;
  uint16_t height;
  char hash[16];
  uint32_t crc; // of the pixels
};

const esp_partition_t *s_part = nullptr;
uint8_t *s_live = nullptr;    // drawn by LVGL
uint8_t *s_staging = nullptr; // downloads, then swapped in
lv_img_dsc_t s_dsc{};
bool s_hasImage = false;
char s_storedHash[16] = {0};
volatile bool s_pending = false; // staging holds a committed picture

uint8_t *allocPixels() {
  return static_cast<uint8_t *>(
      heap_caps_malloc(BYTES, MALLOC_CAP_SPIRAM | MALLOC_CAP_8BIT));
}

void pointDsc() {
  s_dsc.header.always_zero = 0;
  s_dsc.header.w = WIDTH;
  s_dsc.header.h = HEIGHT;
  s_dsc.header.cf = LV_IMG_CF_TRUE_COLOR;
  s_dsc.data_size = BYTES;
  s_dsc.data = s_live;
}

} // namespace

void begin() {
  s_live = allocPixels();
  s_staging = allocPixels();
  s_part = esp_partition_find_first(ESP_PARTITION_TYPE_DATA,
                                    ESP_PARTITION_SUBTYPE_DATA_SPIFFS, nullptr);
  if (s_live == nullptr || s_staging == nullptr || s_part == nullptr ||
      s_part->size < kPixelsAt + BYTES) {
    Serial.println("[Wallpaper] No buffer or partition: image wallpapers off");
    return;
  }
  pointDsc();

  Header h{};
  if (esp_partition_read(s_part, 0, &h, sizeof(h)) != ESP_OK ||
      h.magic != kMagic || h.width != WIDTH || h.height != HEIGHT) {
    return; // nothing saved yet
  }
  h.hash[sizeof(h.hash) - 1] = '\0';
  if (esp_partition_read(s_part, kPixelsAt, s_live, BYTES) != ESP_OK ||
      esp_rom_crc32_le(0, s_live, BYTES) != h.crc) {
    Serial.println("[Wallpaper] Saved picture is damaged; ignoring it");
    return;
  }
  strncpy(s_storedHash, h.hash, sizeof(s_storedHash) - 1);
  s_hasImage = true;
  Serial.printf("[Wallpaper] Loaded %s\n", s_storedHash);
}

const lv_img_dsc_t *image() { return s_hasImage ? &s_dsc : nullptr; }

const char *storedHash() { return s_storedHash; }

uint8_t *stagingBuffer() { return s_staging; }

bool canStage() { return s_staging != nullptr && s_part != nullptr && !s_pending; }

bool commit(const char *hash) {
  if (!canStage()) {
    return false;
  }
  Header h{};
  h.magic = kMagic;
  h.width = WIDTH;
  h.height = HEIGHT;
  strncpy(h.hash, hash, sizeof(h.hash) - 1);
  h.crc = esp_rom_crc32_le(0, s_staging, BYTES);
  // Erase pauses flash access (and so drawing) for about a second; only on a
  // new picture.
  const bool saved =
      esp_partition_erase_range(s_part, 0, kPixelsAt + ((BYTES + 4095) & ~4095u)) == ESP_OK &&
      esp_partition_write(s_part, kPixelsAt, s_staging, BYTES) == ESP_OK &&
      esp_partition_write(s_part, 0, &h, sizeof(h)) == ESP_OK;
  if (!saved) {
    Serial.println("[Wallpaper] Couldn't save; showing it until restart");
  }
  strncpy(s_storedHash, h.hash, sizeof(s_storedHash) - 1);
  s_pending = true;
  return true;
}

bool applyPending() {
  if (!s_pending) {
    return false;
  }
  uint8_t *old = s_live;
  s_live = s_staging;
  s_staging = old;
  pointDsc();
  lv_img_cache_invalidate_src(&s_dsc);
  s_hasImage = true;
  s_pending = false;
  return true;
}

} // namespace Wallpaper
