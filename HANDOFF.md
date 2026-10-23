# HANDOFF: HawaOS (`hawaos-lilygo`), a desktop dashboard "OS" for the LilyGo T-Display S3

You are picking up a project from a design discussion. Read this whole file before writing code. Decisions marked **LOCKED** were made deliberately: do not revisit them without asking the user. Items marked **VERIFY** came from memory and must be checked against official sources before you rely on them.

## 1. Goal

Build **HawaOS**, firmware for a **LilyGo T-Display S3** that behaves like a small "OS" and acts as a **desktop dashboard**: a screen manager plus apps (clock, system stats, notifications, settings), fed by a **Rust companion daemon** running on the user's PC.

"OS" here means an app framework on top of FreeRTOS: screen manager, app interface, task scheduler, settings storage, OTA. The ESP32-S3 has no MMU, so there is no process isolation or dynamic app loading. Do not try to build those.

## 2. Locked decisions

- **LOCKED:** Firmware uses the **Arduino framework** (Arduino-ESP32 on top of ESP-IDF/FreeRTOS), built with **PlatformIO CLI**. The user knows Arduino, not Rust-on-ESP.
- **LOCKED:** Desktop companion is written in **Rust**, in the same repo under `companion/`.
- **LOCKED:** Editor is **Zed** (no VS Code, no PlatformIO IDE extension). Dev machine is **Fedora Linux**.
- **LOCKED:** Do **not** use Adafruit GFX / Adafruit_ST7789. The T-Display S3 panel (ST7789) is on an **8-bit parallel bus**, and Adafruit's driver is SPI-only.
- Display stack: **LVGL** on top of **TFT_eSPI** or **LovyanGFX** (default to whichever LilyGo's official repo supports with the least friction; TFT_eSPI is the likely start). Both are covered in LilyGo's docs.
- BLE: **NimBLE-Arduino** (not the default Bluedroid stack), for RAM reasons when WiFi and BLE coexist.
- Escape hatch: if Arduino's prebuilt core becomes limiting (custom `sdkconfig`, power tuning), Arduino-ESP32 can be used as an ESP-IDF component, so code can be kept. Only do this if a real wall is hit, and ask first.

## 3. Hardware facts (VERIFY against LilyGo's official repo/docs)

Source of truth: LilyGo's `T-Display-S3` GitHub repository (look for `pin_config.h`, the example `platformio.ini`, and their TFT_eSPI setup/`User_Setup` files). Copy their values; do not trust the values below.

- ESP32-S3, dual core 240 MHz, 16 MB flash, usually 8 MB PSRAM (check the user's unit).
- Display: 170x320 ST7789, 8-bit parallel. Use landscape (320x170) for the dashboard.
- Two user buttons, believed to be GPIO 0 (BOOT) and GPIO 14.
- There is a power-enable pin and a backlight pin that must be driven for the screen to come on (believed GPIO 15 and GPIO 38). Confirm in `pin_config.h`.
- The touch variant exists; the user has not said which they own. Assume **non-touch, two buttons**, and ask if it matters.
- Internal RAM is the real constraint (WiFi + BLE + LVGL). Put LVGL draw buffers and large allocations in **PSRAM**.

## 4. Repo layout

```
hawaos-lilygo/
├── platformio.ini
├── .clangd
├── .zed/tasks.json
├── HANDOFF.md
├── include/
│   └── config.h              # pins, feature flags, task stack sizes
├── src/
│   ├── main.cpp              # setup(): Kernel::begin(); loop(): vTaskDelete(NULL) or empty
│   ├── core/
│   │   ├── Kernel.h/.cpp     # boot order, creates FreeRTOS tasks
│   │   ├── App.h             # app interface
│   │   ├── AppManager.h/.cpp # registry, switching, lifecycle
│   │   ├── EventBus.h/.cpp   # FreeRTOS queue of Event structs
│   │   ├── DataStore.h/.cpp  # DataSnapshot, latest companion data, mutex-guarded
│   │   └── Settings.h/.cpp   # Preferences wrapper
│   ├── drivers/
│   │   ├── Display.h/.cpp    # panel + LVGL init, flush cb, backlight
│   │   └── Buttons.h/.cpp    # debounce -> short/long/double events
│   ├── services/             # each runs in its own task
│   │   ├── WifiService.*
│   │   ├── NtpService.*
│   │   ├── CompanionLink.*   # receives JSON from the Rust daemon
│   │   ├── BleService.*      # NimBLE
│   │   └── OtaService.*
│   ├── ui/
│   │   ├── Theme.h/.cpp
│   │   ├── StatusBar.*       # wifi/ble/battery icons, always visible
│   │   └── widgets/          # gauge, sparkline, list
│   └── apps/
│       ├── ClockApp.*
│       ├── StatsApp.*
│       ├── NotifyApp.*
│       └── SettingsApp.*
└── companion/                # separate Cargo project
    ├── Cargo.toml
    └── src/
```

`DataSnapshot` is referenced by `App::onData` but must be defined in `core/DataStore.h`; include it from `App.h`.

## 5. Tooling setup (Zed + PlatformIO CLI)

Install: `pipx install platformio`.

`platformio.ini` starting point (merge with LilyGo's official one, especially PSRAM and TFT pin flags):

```ini
[env:tdisplay-s3]
platform = espressif32
board = lilygo-t-display-s3
framework = arduino
monitor_speed = 115200
build_flags =
    -DARDUINO_USB_CDC_ON_BOOT=1
    -Isrc
    -Iinclude
lib_deps =
    lvgl/lvgl
    h2zero/NimBLE-Arduino
    bodmer/TFT_eSPI
    bblanchon/ArduinoJson
```

- `-Isrc -Iinclude` lets code use `#include "core/App.h"`.
- Pin versions in `lib_deps` once things work. LVGL major versions (8 vs 9) have different APIs; pick one and stay consistent.
- Zed uses clangd. Generate the compilation database with `pio run -t compiledb`, and re-run it after any change to `lib_deps` or `build_flags`.
- `.clangd` must strip Xtensa flags clangd does not understand:

```yaml
CompileFlags:
  Remove: [-mlongcalls, -mtext-section-literals, -fstrict-volatile-bitfields, -fno-tree-switch-conversion, -mfix-esp32-psram-cache-issue]
```

`.zed/tasks.json`:

```json
[
  { "label": "PIO Build",     "command": "pio run" },
  { "label": "PIO Upload",    "command": "pio run -t upload" },
  { "label": "PIO Monitor",   "command": "pio device monitor", "use_new_terminal": true },
  { "label": "PIO Compiledb", "command": "pio run -t compiledb" }
]
```

Fedora flashing gotchas (tell the user rather than silently working around them):
- User must be in the `dialout` group (`sudo usermod -aG dialout $USER`, then re-login).
- ModemManager can grab the ESP USB port and break uploads; stop or mask it if uploads fail randomly.
- If the port does not appear: hold BOOT, tap RST, release BOOT to enter download mode.

## 6. Architecture rules (do not break these)

1. **Only the UI task touches LVGL.** Services never create or update widgets. They update `DataStore` and post events to the `EventBus`. This is the main stability rule.
2. **Tasks:**

   | Task | Core | Job |
   |---|---|---|
   | UI | 1 | `lv_timer_handler()`, `AppManager`, drains EventBus |
   | WiFi/Companion | 0 | connect, receive and parse JSON, update `DataStore`, post `DataUpdated` |
   | BLE (NimBLE) | 0 | advertising/connection events |
   | Buttons | 1 (or ISR + small task) | debounce, classify, post `ButtonEvent` |

3. **App interface:**

```cpp
enum class Button { Next, Select };
enum class Press  { Short, Long, Double };

class App {
public:
  virtual ~App() = default;
  virtual const char* name() const = 0;
  virtual void onEnter(lv_obj_t* root) = 0;      // build LVGL widgets
  virtual void onExit() {}                       // free what you allocated
  virtual void onTick(uint32_t nowMs) {}         // ~10-30 Hz from the UI task
  virtual bool onButton(Button b, Press p) { return false; } // true = consumed
  virtual void onData(const DataSnapshot& d) {}
};
```

4. **AppManager:** static registration at boot (fixed array, no heap churn). `switchTo(idx)` does `onExit()` on the old app, deletes its screen, creates a fresh `lv_obj_create(NULL)` screen, then `onEnter()` on the new one. Global gestures (e.g. long-press Next = launcher) apply only if the app's `onButton` returned false.
5. **Input:** two buttons only, so design around short / long / double press.
6. **Memory:** LVGL draw buffers in PSRAM (`heap_caps_malloc(..., MALLOC_CAP_SPIRAM)`); log free internal heap and PSRAM at boot and periodically over serial.
7. **Boot order in `Kernel::begin()`:** Serial + Settings -> Display (splash immediately) -> Buttons -> AppManager + register apps -> services (WiFi first; NTP/Companion/OTA after connect) -> UI task.
8. **Never block the UI task** with network calls or long delays.

## 7. Companion protocol (default, adjustable)

- Transport: **UDP**, daemon -> device, default port `7777` (device just listens; the daemon does not care if it is offline).
- One small JSON object per message, with a `type` field so new kinds can be added:

```json
{"type":"stats","t":1761200000,"cpu":37,"ram":61,"net":{"up":120,"down":850}}
{"type":"notify","title":"Build finished","body":"cargo build ok","ts":1761200005}
```

- Keep messages well under one UDP packet (~1 KB). Parse with ArduinoJson using a fixed-size `StaticJsonDocument`/`JsonDocument` and reject malformed input without crashing.
- Device IP: start with a config value in the daemon (and `Settings` on the device for WiFi creds); add mDNS discovery later if wanted.
- Rust daemon starting point: `tokio`, `serde`/`serde_json`, `sysinfo` for CPU/RAM/network, a 1 s interval loop. Keep the message structs in one module so the schema is easy to keep in sync with the firmware.

## 8. Milestones (stop and report after each; each must be visibly working)

1. **Hello display.** Project builds and flashes; screen powers on, shows a splash and one hardcoded LVGL screen. Serial logs heap and PSRAM. *Accept:* stable for 10+ minutes, no resets.
2. **Buttons + AppManager.** Buttons classified into short/long/double and logged. Two placeholder apps; Next cycles, long-press returns to launcher. *Accept:* switching apps 200 times leaks no heap.
3. **WiFi + NTP + ClockApp.** WiFi credentials stored via `Preferences` (simple provisioning flow or a gitignored `secrets.h` first), NTP time shown in ClockApp, status bar with WiFi icon. *Accept:* time correct, reconnects after router reboot.
4. **CompanionLink + StatsApp.** Rust daemon sends stats over UDP; device parses into `DataStore`; StatsApp renders gauges/sparkline. *Accept:* values update every second, UI stays smooth, no crash if daemon stops or sends garbage.
5. **Notifications + Settings.** NotifyApp shows incoming `notify` messages; SettingsApp adjusts brightness and similar, persisted via `Preferences`.
6. **OTA + BLE (optional/last).** ArduinoOTA or HTTP OTA; NimBLE service for status or provisioning. Ask the user which BLE use case they want before building it.

## 9. Working agreements

- Work one milestone at a time; do not scaffold everything up front with empty stubs.
- Before using any pin, flag, or library setting listed under "VERIFY", confirm it from LilyGo's repo and say what you confirmed.
- If a library API differs from what is written here, trust the library's docs and note the difference.
- Prefer small, compilable commits. Run `pio run` after each change and fix warnings that matter.
- If something needs the physical device (flashing, observing the screen), tell the user exactly what to do and what output to paste back; do not assume it worked.
- Ask the user before: switching display library, switching LVGL major version, moving to an ESP-IDF project, adding dependencies beyond those listed, or changing the protocol.
- Do not add features beyond the milestone list without asking.

## 10. Definition of done (v1)

Firmware builds clean with `pio run`; boots to a launcher in a few seconds; Clock, Stats, Notify, and Settings apps work; the Rust daemon feeds live PC stats; losing WiFi or the daemon never freezes the UI; settings persist across reboots; heap and PSRAM usage stay stable over a multi-hour soak.
