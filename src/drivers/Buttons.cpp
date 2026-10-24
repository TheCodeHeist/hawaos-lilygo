#include "drivers/Buttons.h"
#include "config.h"
#include "core/App.h"
#include "core/AppManager.h"
#include "core/EventBus.h"
#include "services/WifiService.h"

#include <Arduino.h>
#include <freertos/FreeRTOS.h>
#include <freertos/task.h>

namespace Buttons {

namespace {

enum class SingleState : uint8_t {
  Idle,
  Pressed,
  WaitDouble,
  SecondPressed,
  WaitRelease,
};

struct ButtonTracker {
  uint8_t pin;
  Button logicalBtn;
  bool rawLast;
  bool debounced;
  uint32_t lastRawChangeMs;
  uint32_t stateTsMs;
  SingleState state;
};

ButtonTracker s_next = {
    Config::PIN_BUTTON_NEXT, Button::Next, false, false, 0, 0,
    SingleState::Idle};
ButtonTracker s_select = {
    Config::PIN_BUTTON_SELECT, Button::Select, false, false, 0, 0,
    SingleState::Idle};
bool s_chordLatch = false;

TaskHandle_t s_btnTaskHandle = nullptr;

void emitEvent(Button b, Press p) {
  Serial.printf("[Buttons] %s -> %s\n", buttonName(b), pressName(p));
  Event ev{};
  ev.type = EventType::ButtonPressed;
  ev.btn.button = b;
  ev.btn.press = p;
  EventBus::post(ev);
}

void sampleDebounce(ButtonTracker &b, uint32_t nowMs) {
  const bool raw = (digitalRead(b.pin) == LOW);
  if (raw != b.rawLast) {
    b.rawLast = raw;
    b.lastRawChangeMs = nowMs;
  }
  if ((nowMs - b.lastRawChangeMs) >= Config::BTN_DEBOUNCE_MS) {
    b.debounced = b.rawLast;
  }
}

void updateSingleFsm(ButtonTracker &b, uint32_t nowMs) {
  switch (b.state) {
  case SingleState::Idle:
    if (b.debounced) {
      b.stateTsMs = nowMs;
      b.state = SingleState::Pressed;
    }
    break;

  case SingleState::Pressed:
    if (!b.debounced) {
      // Released before long-press threshold:
      // Only wait for double-press if active app explicitly opts in! (DESIGN.md
      // Section 8)
      if (AppManager::activeAppWantsDouble(b.logicalBtn)) {
        b.stateTsMs = nowMs;
        b.state = SingleState::WaitDouble;
      } else {
        emitEvent(b.logicalBtn, Press::Short);
        b.state = SingleState::Idle;
      }
    } else if ((nowMs - b.stateTsMs) >= Config::BTN_LONG_PRESS_MS) {
      emitEvent(b.logicalBtn, Press::Long);
      b.state = SingleState::WaitRelease;
    }
    break;

  case SingleState::WaitDouble:
    if (b.debounced) {
      b.stateTsMs = nowMs;
      b.state = SingleState::SecondPressed;
    } else if ((nowMs - b.stateTsMs) >= Config::BTN_DOUBLE_GAP_MS) {
      emitEvent(b.logicalBtn, Press::Short);
      b.state = SingleState::Idle;
    }
    break;

  case SingleState::SecondPressed:
    if (!b.debounced) {
      emitEvent(b.logicalBtn, Press::Double);
      b.state = SingleState::Idle;
    } else if ((nowMs - b.stateTsMs) >= Config::BTN_LONG_PRESS_MS) {
      emitEvent(b.logicalBtn, Press::Double);
      b.state = SingleState::WaitRelease;
    }
    break;

  case SingleState::WaitRelease:
    if (!b.debounced) {
      b.state = SingleState::Idle;
    }
    break;
  }
}

void pollSerialCommands() {
  while (Serial.available() > 0) {
    const int ch = Serial.read();
    if (ch == 't' || ch == 'T') {
      Serial.println(
          "[Buttons] Serial 't' -> queuing 200x App switch stress test");
      Event ev{};
      ev.type = EventType::RunStressTest;
      ev.stressIterations = 200;
      EventBus::post(ev);
    } else if (ch == 'r' || ch == 'R') {
      Serial.println(
          "[Buttons] Serial 'r' -> forcing WiFi disconnect/reconnect test");
      WifiService::forceReconnect();
    } else if (ch == 'n') {
      emitEvent(Button::Next, Press::Short);
    } else if (ch == 'l') {
      emitEvent(Button::Next, Press::Long);
    } else if (ch == 's') {
      emitEvent(Button::Select, Press::Short);
    } else if (ch == 'b') {
      emitEvent(Button::Select, Press::Long);
    } else if (ch == 'd') {
      emitEvent(Button::Next, Press::Double);
    } else if (ch == 'c' || ch == 'C') {
      emitEvent(Button::Select, Press::Chord);
    }
  }
}

void buttonsTaskEntry(void * /*param*/) {
  for (;;) {
    const uint32_t nowMs = millis();
    sampleDebounce(s_next, nowMs);
    sampleDebounce(s_select, nowMs);

    // Chord detection: both buttons held simultaneously within
    // Config::BTN_CHORD_WINDOW_MS (80ms)
    if (s_chordLatch) {
      if (!s_next.debounced && !s_select.debounced) {
        s_chordLatch = false;
        s_next.state = SingleState::Idle;
        s_select.state = SingleState::Idle;
      }
    } else if (s_next.debounced && s_select.debounced) {
      const uint32_t diff = (s_next.stateTsMs > s_select.stateTsMs)
                                ? (s_next.stateTsMs - s_select.stateTsMs)
                                : (s_select.stateTsMs - s_next.stateTsMs);
      if (diff <= Config::BTN_CHORD_WINDOW_MS ||
          (s_next.state == SingleState::Pressed &&
           s_select.state == SingleState::Pressed)) {
        s_chordLatch = true;
        s_next.state = SingleState::WaitRelease;
        s_select.state = SingleState::WaitRelease;
        emitEvent(Button::Select, Press::Chord);
      }
    } else {
      updateSingleFsm(s_next, nowMs);
      updateSingleFsm(s_select, nowMs);
    }

    vTaskDelay(pdMS_TO_TICKS(Config::BTN_POLL_PERIOD_MS));
  }
}

} // namespace

void begin() {
  pinMode(s_next.pin, INPUT_PULLUP);
  pinMode(s_select.pin, INPUT_PULLUP);

  xTaskCreatePinnedToCore(
      buttonsTaskEntry, "btn_task", Config::BTN_TASK_STACK_BYTES, nullptr,
      Config::BTN_TASK_PRIORITY, &s_btnTaskHandle, Config::BTN_TASK_CORE);
}

} // namespace Buttons
