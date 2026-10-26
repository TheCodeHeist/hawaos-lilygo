#pragma once

#include "core/DataStore.h"
#include "ui/icons.h"
#include <lvgl.h>
#include <stdint.h>

enum class Button : uint8_t { Next, Select };
enum class Press : uint8_t { Short, Long, Double, Chord };

inline const char *buttonName(Button b) {
  switch (b) {
  case Button::Next:
    return "Next";
  case Button::Select:
    return "Select";
  default:
    return "?";
  }
}

inline const char *pressName(Press p) {
  switch (p) {
  case Press::Short:
    return "Short";
  case Press::Long:
    return "Long";
  case Press::Double:
    return "Double";
  case Press::Chord:
    return "Chord";
  default:
    return "?";
  }
}

class App {
public:
  virtual ~App() = default;
  virtual const char *name() const = 0;
  virtual const char *icon() const = 0; // Lucide icon macro from src/ui/icons.h
  virtual void onEnter(lv_obj_t *root) = 0; // build LVGL widgets
  virtual void onExit() {}                  // free what you allocated
  virtual void onTick(uint32_t nowMs) {}    // ~10-30 Hz from the UI task
  virtual bool onButton(Button b, Press p) { return false; } // true = consumed
  virtual bool wantsDouble(Button b) const {
    return false;
  } // opt in to Press::Double (DESIGN.md 8)
  virtual bool hidesStatusBar() const {
    return false;
  } // Media screen hides 22px status bar
  virtual bool usesWallpaper() const {
    return false;
  } // Only Home & Launcher use wallpaper
  virtual bool needsCompanion() const {
    return false;
  } // live companion data: covered while it's away (Bluetooth mode)
  virtual void onData(const DataSnapshot &d) {}
};
