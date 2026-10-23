#pragma once

#include "core/DataStore.h"
#include <lvgl.h>
#include <stdint.h>

enum class Button : uint8_t { Next, Select };
enum class Press : uint8_t { Short, Long, Double };

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
  default:
    return "?";
  }
}

class App {
public:
  virtual ~App() = default;
  virtual const char *name() const = 0;
  virtual void onEnter(lv_obj_t *root) = 0; // build LVGL widgets
  virtual void onExit() {}                  // free what you allocated
  virtual void onTick(uint32_t nowMs) {}    // ~10-30 Hz from the UI task
  virtual bool onButton(Button b, Press p) { return false; } // true = consumed
  virtual void onData(const DataSnapshot &d) {}
};
