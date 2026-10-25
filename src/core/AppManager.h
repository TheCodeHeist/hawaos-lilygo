#pragma once

#include "core/App.h"
#include <stddef.h>
#include <stdint.h>

namespace AppManager {

void begin();

// Register an app statically at boot (fixed array, no heap allocation).
bool registerApp(App *app);

// Switch to app by index (0 .. appCount()-1).
void switchTo(size_t idx);

// Exit current app (if any) and show the Redmi-style horizontal carousel
// Launcher.
void showLauncher();

// Open or close the 2x2 Control Center overlay (triggered by Chord).
void openControlCenter();
void closeControlCenter();

// Enter or exit Always-On Display (AOD) mode (DESIGN.md Section 10).
void enterAod();
void exitAod();
bool isInAod();

// Show full-screen notification alert (part of AOD at low brightness, or active
// mode).
void showNotificationAlert(const char *app, const char *title, const char *body, uint8_t count);
inline void showNotificationAlert(const char *title, const char *body, uint8_t count) {
  showNotificationAlert("", title, body, count);
}

// Dismiss the notification alert immediately and resume normal rendering.
void dismissNotificationAlert();

// Returns true if notification alert is currently active (suspends other
// rendering).
bool isAlertActive();

// Reset idle activity timer (e.g. on notifications or user interaction).
void resetActivity();

// Instant theme recolor across StatusBar and current screen/app (DESIGN.md
// Section 3.3).
void applyActiveTheme();

// Query whether the currently active view opts into double-press for Button `b`
// (DESIGN.md Section 8).
bool activeAppWantsDouble(Button b);

// Called from the UI task to tick active app / AOD / idle timer.
void tick(uint32_t nowMs);

// Route button events (including Chord and AOD wake) from the UI task.
void handleButton(Button b, Press p);

// Route updated DataSnapshot to the active app / Launcher / AOD.
void handleData(const DataSnapshot &d);

// Execute N rapid app switches and verify zero heap leak.
void runStressTest(uint16_t switches);

size_t appCount();
int activeAppIndex(); // -1 when in Launcher

} // namespace AppManager
