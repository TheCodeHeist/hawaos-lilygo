#include "drivers/Buttons.h"
#include "config.h"
#include "core/App.h"
#include "core/EventBus.h"
#include "services/WifiService.h"

#include <Arduino.h>
#include <freertos/FreeRTOS.h>
#include <freertos/task.h>

namespace Buttons {

namespace {

enum class FsmState : uint8_t {
  Idle,
  FirstPressHeld,
  WaitDoublePress,
  SecondPressHeld,
  WaitRelease,
};

struct ButtonTracker {
  uint8_t pin;
  Button logicalBtn;
  bool rawLast;
  bool debouncedPressed;
  uint32_t lastRawChangeMs;
  uint32_t stateTimestampMs;
  FsmState state;
};

ButtonTracker s_buttons[] = {
    {Config::PIN_BUTTON_2, Button::Next, false, false, 0, 0, FsmState::Idle},
    {Config::PIN_BUTTON_1, Button::Select, false, false, 0, 0, FsmState::Idle},
};

TaskHandle_t s_btnTaskHandle = nullptr;

void emitButtonEvent(const ButtonTracker &b, Press press) {
  Serial.printf("[Buttons] %s (GPIO %u) -> %s\n", buttonName(b.logicalBtn),
                static_cast<unsigned>(b.pin), pressName(press));

  Event ev{};
  ev.type = EventType::ButtonPressed;
  ev.btn.button = b.logicalBtn;
  ev.btn.press = press;
  EventBus::post(ev);
}

void updateButton(ButtonTracker &b, uint32_t nowMs) {
  const bool rawPressed = (digitalRead(b.pin) == LOW);

  if (rawPressed != b.rawLast) {
    b.rawLast = rawPressed;
    b.lastRawChangeMs = nowMs;
  }

  if ((nowMs - b.lastRawChangeMs) >= Config::BTN_DEBOUNCE_MS) {
    b.debouncedPressed = b.rawLast;
  }

  switch (b.state) {
  case FsmState::Idle:
    if (b.debouncedPressed) {
      b.stateTimestampMs = nowMs;
      b.state = FsmState::FirstPressHeld;
    }
    break;

  case FsmState::FirstPressHeld:
    if (!b.debouncedPressed) {
      b.stateTimestampMs = nowMs;
      b.state = FsmState::WaitDoublePress;
    } else if ((nowMs - b.stateTimestampMs) >= Config::BTN_LONG_PRESS_MS) {
      emitButtonEvent(b, Press::Long);
      b.state = FsmState::WaitRelease;
    }
    break;

  case FsmState::WaitDoublePress:
    if (b.debouncedPressed) {
      b.stateTimestampMs = nowMs;
      b.state = FsmState::SecondPressHeld;
    } else if ((nowMs - b.stateTimestampMs) >= Config::BTN_DOUBLE_GAP_MS) {
      emitButtonEvent(b, Press::Short);
      b.state = FsmState::Idle;
    }
    break;

  case FsmState::SecondPressHeld:
    if (!b.debouncedPressed) {
      emitButtonEvent(b, Press::Double);
      b.state = FsmState::Idle;
    } else if ((nowMs - b.stateTimestampMs) >= Config::BTN_LONG_PRESS_MS) {
      emitButtonEvent(b, Press::Double);
      b.state = FsmState::WaitRelease;
    }
    break;

  case FsmState::WaitRelease:
    if (!b.debouncedPressed) {
      b.state = FsmState::Idle;
    }
    break;
  }
}

void pollSerialCommands() {
  while (Serial.available() > 0) {
    const int ch = Serial.read();
    if (ch == 't' || ch == 'T') {
      Serial.println("[Buttons] Serial command 't' -> queuing 200x App switch "
                     "stress test");
      Event ev{};
      ev.type = EventType::RunStressTest;
      ev.stressIterations = 200;
      EventBus::post(ev);
    } else if (ch == 'r' || ch == 'R') {
      Serial.println("[Buttons] Serial command 'r' -> forcing WiFi "
                     "disconnect/reconnect test");
      WifiService::forceReconnect();
    } else if (ch == 'n') {
      emitButtonEvent(s_buttons[0], Press::Short);
    } else if (ch == 'l') {
      emitButtonEvent(s_buttons[0], Press::Long);
    } else if (ch == 's') {
      emitButtonEvent(s_buttons[1], Press::Short);
    } else if (ch == 'd') {
      emitButtonEvent(s_buttons[1], Press::Double);
    }
  }
}

void buttonsTaskEntry(void * /*param*/) {
  for (;;) {
    const uint32_t nowMs = millis();
    for (size_t i = 0; i < sizeof(s_buttons) / sizeof(s_buttons[0]); ++i) {
      updateButton(s_buttons[i], nowMs);
    }
    pollSerialCommands();
    vTaskDelay(pdMS_TO_TICKS(Config::BTN_POLL_PERIOD_MS));
  }
}

} // namespace

void begin() {
  for (size_t i = 0; i < sizeof(s_buttons) / sizeof(s_buttons[0]); ++i) {
    pinMode(s_buttons[i].pin, INPUT_PULLUP);
  }

  xTaskCreatePinnedToCore(
      buttonsTaskEntry, "btn_task", Config::BTN_TASK_STACK_BYTES, nullptr,
      Config::BTN_TASK_PRIORITY, &s_btnTaskHandle, Config::BTN_TASK_CORE);
}

} // namespace Buttons
