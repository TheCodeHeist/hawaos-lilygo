#pragma once

#include "core/DataStore.h"
#include <stdint.h>

namespace StatusBar {

// Create persistent status bar on lv_layer_top() (must be called from UI task).
void begin();

// Update the left-hand context/app title.
void setTitle(const char *title);

// Refresh WiFi / BLE / NTP indicators from DataSnapshot (UI task only).
void updateData(const DataSnapshot &d);

// Periodic tick (~1 Hz) to refresh mini clock & battery state (UI task only).
void tick(uint32_t nowMs);

} // namespace StatusBar
