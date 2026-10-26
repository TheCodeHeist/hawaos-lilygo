#pragma once

#include <stddef.h>

// BLE transport for first-run setup: one GATT service with an RX
// characteristic (companion writes) and a TX characteristic (device
// notifies), both carrying newline-delimited JSON like USB and UDP. Only runs
// while SetupService is active; stop() frees the stack's memory.
namespace BleLink {

void start(const char *name);
void stop();
bool isRunning();
bool isConnected();

// Queues one JSON line for the companion (adds the newline, splits by MTU).
void send(const char *jsonLine);

// Pops one complete received line (without newline). Call from the WiFi task.
bool popLine(char *out, size_t outLen);

} // namespace BleLink
