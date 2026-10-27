#pragma once

#include <lvgl.h>
#include <stddef.h>
#include <stdint.h>

// Image wallpaper (DESIGN.md Section 11.8): a 320x170 RGB565 picture from
// the companion, kept in the flash partition the default table reserves for
// SPIFFS (unused otherwise), so it survives restarts and OTA updates.
//
// Downloads land in a staging buffer; commit() saves it, and the UI task
// swaps it in with applyPending(), so the screen never shows a partial one.
namespace Wallpaper {

constexpr uint16_t WIDTH = 320;
constexpr uint16_t HEIGHT = 170;
constexpr uint32_t BYTES = WIDTH * HEIGHT * 2; // 108800 B

void begin(); // allocates buffers and loads the saved picture

// The picture to draw, or nullptr if there is none yet. UI task only.
const lv_img_dsc_t *image();
// Hash of the saved picture ("" if none).
const char *storedHash();

// For the network task: where a download goes, and whether it may start (a
// committed picture the UI hasn't taken yet still occupies the staging buffer).
uint8_t *stagingBuffer();
bool canStage();
// Saves the staged picture under `hash` and queues it for the UI.
bool commit(const char *hash);
// UI task: swaps in a committed picture. True if the image changed.
bool applyPending();

} // namespace Wallpaper
