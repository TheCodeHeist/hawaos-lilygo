#pragma once

#include "core/DataStore.h"
#include <stdint.h>

namespace StatusBar {

// Create persistent 22px status bar + notification toast banner on
// lv_layer_top() (must be called from UI task).
void begin();

// Show or hide the status bar (hidden on Boot, AOD, and Media per DESIGN.md
// Section 9.1).
void setVisible(bool visible);

// Update the left-hand screen title (caption, `textDim`).
void setTitle(const char *title);

// Re-apply active theme colors immediately on theme switch (UI task only).
void applyTheme();

// Refresh WiFi / USB / BLE / Bell unread icons from DataSnapshot (UI task
// only).
void updateData(const DataSnapshot &d);

// Show a floating notification popup banner on lv_layer_top() when a new
// notification arrives (if enabled in Settings).
void showNotificationPopup(const char *title, const char *body);

// Dismiss the notification popup banner immediately if visible.
void dismissNotificationPopup();

// Returns true if the notification popup banner is currently shown.
bool isPopupVisible();

// Periodic tick (UI task only).
void tick(uint32_t nowMs);

} // namespace StatusBar
