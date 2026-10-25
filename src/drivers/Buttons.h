#pragma once

namespace Buttons {

// Configure GPIO pins and start the button debounce/classification FreeRTOS
// task.
void begin();

// Dynamically swap physical GPIO button pins so physical Top is always Select
// and physical Bottom is always Next regardless of 180° orientation.
void setOrientationInverted(bool inverted);

} // namespace Buttons
