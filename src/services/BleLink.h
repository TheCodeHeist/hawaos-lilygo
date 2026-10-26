#pragma once

#include <stddef.h>

// BLE transport: one GATT service with an RX characteristic (companion
// writes) and a TX characteristic (device notifies), both carrying
// newline-delimited JSON like USB and UDP. Runs during setup and, in
// Bluetooth link mode, all the time; stop() frees the stack's memory.
namespace BleLink {

// paired goes into the advertisement (manufacturer data 0xFFFF 'H' <0|1>) so
// the companion can tell a device in setup from one already set up.
void start(const char *name, bool paired);
void setPaired(bool paired);
void stop();
bool isRunning();
bool isConnected();

// Queues one JSON line for the companion (adds the newline, splits by MTU).
void send(const char *jsonLine);

// Pops one complete received line (without newline). Call from the WiFi task.
bool popLine(char *out, size_t outLen);

} // namespace BleLink
