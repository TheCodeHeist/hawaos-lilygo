# HawaOS (`hawaos-lilygo`)

**HawaOS** is a lightweight desktop dashboard "OS" for the **LilyGo T-Display-S3** (`ESP32-S3R8`), built on **Arduino-ESP32 + FreeRTOS + LVGL 8**, and designed to pair with a **Rust companion daemon** running on a Linux/desktop PC.

---

## Features & Architecture

HawaOS provides an event-driven application framework over FreeRTOS tailored for the constrained internal SRAM and 8 MB OPI PSRAM of the ESP32-S3:

- **Strict Single-Core UI Ownership:** Only the `ui_task` pinned to **Core 1** touches LVGL widgets. Background services run on **Core 0**, update a mutex-guarded [`DataStore`](src/core/DataStore.h), and post events to a lock-free FreeRTOS [`EventBus`](src/core/EventBus.h).
- **PSRAM Framebuffer:** Full `320×170` 16-bit RGB565 LVGL draw buffer (`108,800` bytes) allocated directly in **OPI PSRAM** (`MALLOC_CAP_SPIRAM`), leaving >230 KB of internal SRAM free for WiFi and BLE stacks.
- **Zero-Leak App Lifecycle ([`AppManager`](src/core/AppManager.h)):** Static boot-time app registration with synchronous screen teardown (`onExit()` → `lv_scr_load(newScr)` → `lv_obj_del(oldScr)` → `onEnter(newScr)`), verified across 200+ rapid app switches with `0 B` heap drift.
- **Persistent Top [`StatusBar`](src/ui/StatusBar.h):** Rendered on `lv_layer_top()` so WiFi RSSI, mini clock, battery state, and active view title persist across screen transitions without flicker.
- **Two-Button Navigation ([`Buttons`](src/drivers/Buttons.h)):** 100 Hz hardware button sampler with 25 ms debounce and finite-state classification into `Short`, `Long` (≥ 600 ms), and `Double` (≤ 250 ms) presses.
- **NVS Settings ([`Settings`](src/core/Settings.h)):** Persists WiFi credentials, 16-level hardware backlight brightness, timezone offset, and clock format in ESP32 NVS `Preferences`.

---

## Hardware Target: LilyGo T-Display-S3

| Component | Specification |
|---|---|
| **MCU** | ESP32-S3R8 Dual-Core Xtensa LX7 @ 240 MHz |
| **Memory** | 16 MB QIO Flash, 8 MB OPI PSRAM, 320 KB Internal SRAM |
| **Display** | 1.9" IPS TFT (ST7789V), `170×320` (`320×170` Landscape), 8-bit Parallel (i8080) |
| **Power & Backlight** | `GPIO 15` (`PIN_POWER_ON`), `GPIO 38` (`PIN_LCD_BL`, 16-step pulse dimming) |
| **LCD Parallel Bus** | `CS=6`, `DC=7`, `RST=5`, `WR=8`, `RD=9`, `D0..D7 = {39,40,41,42,45,46,47,48}` |
| **User Buttons** | `GPIO 14` (`Button::Next`), `GPIO 0 / BOOT` (`Button::Select`) — Active `LOW` |
| **Battery ADC** | `GPIO 4` (`PIN_BAT_VOLT`) |

---

## Repository Structure

```text
hawaos-lilygo/
├── platformio.ini            # PlatformIO config (espressif32@6.5.0, Setup206 flags)
├── .clangd                   # Strips Xtensa compiler flags for clangd / Zed LSP
├── .zed/tasks.json           # Zed build, upload, monitor, and compiledb tasks
├── include/
│   ├── config.h              # Verified hardware pins, task stacks, timing constants
│   ├── lv_conf.h             # LVGL 8.x configuration
│   └── secrets.h             # (Gitignored) Local WiFi credentials for initial NVS seed
├── src/
│   ├── main.cpp              # Entry point: Kernel::begin(), deletes Arduino loopTask
│   ├── core/
│   │   ├── Kernel.h/.cpp     # Boot sequence, UI task, periodic memory diagnostics
│   │   ├── App.h             # Base App interface (onEnter/onExit/onTick/onButton/onData)
│   │   ├── AppManager.h/.cpp # App registry, Launcher, screen lifecycle, 200x leak test
│   │   ├── EventBus.h/.cpp   # FreeRTOS event queue
│   │   ├── DataStore.h/.cpp  # Mutex-guarded DataSnapshot
│   │   └── Settings.h/.cpp   # NVS Preferences wrapper
│   ├── drivers/
│   │   ├── Display.h/.cpp    # TFT_eSPI 8-bit parallel init, LVGL PSRAM buffer, backlight
│   │   └── Buttons.h/.cpp    # Debounce + Short/Long/Double press FSM
│   ├── services/
│   │   ├── WifiService.h/.cpp# Core 0 WiFi station manager & auto-reconnect
│   │   └── NtpService.h/.cpp # SNTP time synchronization
│   ├── ui/
│   │   ├── Theme.h/.cpp      # Shared dark dashboard palette & card helpers
│   │   └── StatusBar.h/.cpp  # Persistent top status bar on lv_layer_top()
│   └── apps/
│       └── ClockApp.h/.cpp   # Digital NTP clock, date, and network status app
└── companion/                # (Upcoming) Rust desktop daemon sending UDP stats & notifications
```

---

## Getting Started

### 1. Prerequisites
- **PlatformIO CLI** (`pipx install platformio`)
- **Zed Editor** (optional, preconfigured via `.zed/tasks.json` and `.clangd`)
- On Fedora/Linux: ensure your user is in the `dialout` group:
  ```bash
  sudo usermod -aG dialout $USER
  ```
  *(If uploads fail intermittently on Fedora, stop `ModemManager` with `sudo systemctl stop ModemManager`.)*

### 2. Configure WiFi (First Boot)
Create `include/secrets.h` (gitignored) to seed your WiFi credentials into NVS on first boot:

```cpp
#pragma once

#define SECRET_WIFI_SSID "YourNetworkSSID"
#define SECRET_WIFI_PASS "YourNetworkPassword"
```

Once flashed once, credentials are saved in ESP32 NVS `Preferences` under the `"hawaos"` namespace.

### 3. Build, Flash & Monitor

```bash
# Build firmware
pio run

# Generate compile_commands.json for clangd / Zed
pio run -t compiledb

# Flash to LilyGo T-Display-S3
pio run -t upload

# Open serial monitor (115200 baud)
pio device monitor
```

> **Download Mode Tip:** If the board does not enter bootloader mode automatically, hold **BOOT (`GPIO 0`)**, tap **RST**, then release **BOOT** and run `pio run -t upload`.

---

## Controls & Serial Shortcuts

### Physical Buttons
- **In Launcher:**
  - `NEXT (GPIO 14)` **Short**: Highlight next app
  - `SELECT (GPIO 0)` **Short**: Launch highlighted app
  - `SELECT (GPIO 0)` **Long**: Run 200× app-switch heap leak stress test
- **Inside Apps (Global Gestures when unconsumed by app):**
  - `NEXT (GPIO 14)` **Short**: Cycle to next app
  - `NEXT (GPIO 14)` **Long**: Return to Launcher
- **Inside `ClockApp`:**
  - `SELECT (GPIO 0)` **Short**: Toggle `12H (AM/PM)` / `24H` mode (persisted in NVS)
  - `SELECT (GPIO 0)` **Double**: Force immediate NTP resync
  - `SELECT (GPIO 0)` **Long**: Force WiFi disconnect/reconnect test

### USB Serial Diagnostic Commands (`115200` baud)
- `t`: Run 200× app-switch heap leak stress test and print SRAM/PSRAM delta
- `r`: Force WiFi disconnect & automatic reconnect cycle
- `n` / `l` / `s` / `d`: Inject simulated button events (`Next Short`, `Next Long`, `Select Short`, `Select Double`)

---

## Roadmap

- [x] **Milestone 1:** Display bring-up (8-bit parallel ST7789V, PSRAM LVGL buffer, splash, memory telemetry)
- [x] **Milestone 2:** Button classifier (`Short`/`Long`/`Double`), `EventBus`, `AppManager`, Launcher, 200× zero-leak test
- [x] **Milestone 3:** NVS `Settings`, Core 0 `WifiService` + `NtpService`, persistent `StatusBar`, `ClockApp`
- [ ] **Milestone 4:** `CompanionLink` (UDP JSON receiver on port `7777`), `StatsApp` (CPU/RAM/Net), Rust companion daemon (`companion/`)
- [ ] **Milestone 5:** `NotifyApp` (desktop notifications) & `SettingsApp` (on-device brightness/preferences UI)
- [ ] **Milestone 6:** OTA firmware updates & `NimBLE-Arduino` BLE service

---

## License

This project is licensed under the [MIT License](LICENSE).
