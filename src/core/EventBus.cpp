#include "core/EventBus.h"
#include "config.h"

#include <freertos/FreeRTOS.h>
#include <freertos/queue.h>

namespace EventBus {

namespace {
QueueHandle_t s_queue = nullptr;
} // namespace

bool begin() {
  if (s_queue == nullptr) {
    s_queue = xQueueCreate(Config::EVENT_QUEUE_LENGTH, sizeof(Event));
  }
  return s_queue != nullptr;
}

bool post(const Event &ev) {
  if (s_queue == nullptr) {
    return false;
  }
  return xQueueSend(s_queue, &ev, 0) == pdTRUE;
}

bool poll(Event &outEv) {
  if (s_queue == nullptr) {
    return false;
  }
  return xQueueReceive(s_queue, &outEv, 0) == pdTRUE;
}

} // namespace EventBus
