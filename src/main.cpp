#include <Arduino.h>
#include <freertos/FreeRTOS.h>
#include <freertos/task.h>

#include "core/Kernel.h"

void setup() { Kernel::begin(); }

void loop() {
  // All work runs in dedicated FreeRTOS tasks; delete the Arduino loop task to
  // reclaim stack.
  vTaskDelete(nullptr);
}
