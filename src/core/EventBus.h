#pragma once

#include "core/App.h"
#include <stdint.h>

enum class EventType : uint8_t {
  ButtonPressed,
  DataUpdated,
  RunStressTest,
};

struct Event {
  EventType type;
  union {
    struct {
      Button button;
      Press press;
    } btn;
    uint16_t stressIterations;
  };
};

namespace EventBus {

bool begin();
bool post(const Event &ev);
bool poll(Event &outEv);

} // namespace EventBus
