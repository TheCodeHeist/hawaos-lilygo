#pragma once

#include <stdint.h>

// Rollback after a bad firmware update (DESIGN.md Section 11.5). A firmware
// installed over the air boots "on trial": it confirms itself once it has run
// for a while with the UI drawing and the companion heard from again. If it
// crashes, resets or isn't healthy within the deadline, the bootloader goes
// back to the previous firmware. USB-flashed firmware is never on trial.
namespace FirmwareHealth {

void begin();

bool onTrial();     // installed over the air and not confirmed yet
void uiAlive();     // UI task heartbeat
void companionHeard(); // an accepted message from the companion

// Id of a firmware that failed its trial and was rolled back from ("" if
// none since the last update): the first bytes of its ELF SHA-256.
const char *rolledBackFrom();

} // namespace FirmwareHealth
