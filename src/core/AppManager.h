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

// Exit current app (if any) and show the HawaOS Launcher screen.
void showLauncher();

// Called from the UI task (~20 Hz) to tick the active app.
void tick(uint32_t nowMs);

// Route button events to the active app or global gestures / launcher.
void handleButton(Button b, Press p);

// Route updated DataSnapshot to the active app.
void handleData(const DataSnapshot &d);

// Execute N rapid app switches and verify zero heap leak.
void runStressTest(uint16_t switches);

size_t appCount();
int activeAppIndex(); // -1 when in Launcher

} // namespace AppManager
