# DESIGN: HawaOS UI/UX Specification

Companion to `HANDOFF.md`. **Where the two conflict, this file wins** (button mapping, Control Center, App interface additions). Items marked **VERIFY** must be checked on the real device or against official docs. Values given as "starting values" are meant to be tuned after seeing them on the actual panel.

Target: LilyGo T-Display S3, 320x170 landscape ST7789 (8-bit parallel), LVGL, two buttons, no touch assumed.

## 1. Principles

1. **Calm and sparse.** One idea per screen, 1 to 3 elements, one big number or title that matters.
2. **Solid, not glass.** No translucency or blur. Cards are solid fills. Alpha blending is avoided everywhere except small, rare cases.
3. **Subtle and snappy motion.** Animations are short, small, and optional. FPS beats flourish (see section 7).
4. **Modern and consistent.** One accent hue (cyan by default, user-selectable, see 3.4), tinted neutrals, one icon family (Lucide), one type family.
5. **Two-button first.** Every screen is fully usable with Next and Select. Rich configuration lives in the Rust desktop companion, not on the device.
6. **Tokens, not hardcoded values.** No raw colors, sizes, or fonts in app code. Everything comes from the theme.

## 2. Screen, grid and shape

- Canvas **320x170**, landscape. Base unit **4 px**. Outer margin **10 px**.
- Status bar: **22 px** tall at the top. Content area: 320x148 below it.
- Corner radii: **8** (chips, small cards), **12** (cards, tiles), **16** (launcher tiles), full pill for progress and toggles.
- Borders: 1 px `border` token on cards in the light theme; in the dark theme borders are optional (surface contrast is enough).
- Keep tap-free: focus is shown with a **2 px `accent` ring** on the focused element (never by scaling it).

## 3. Color

### 3.1 Brand color

Theme color: **`oklch(71.5% 0.143 215.221)`** (soothing cyan, matches Tailwind v4 `cyan-500`, about `#00b8db`). Derive secondary and accent colors from this hue. Do not hardcode hex: write a build-time script (`tools/gen_theme.py` or a small Rust bin) that converts the OKLCH tokens below to sRGB (with gamut clipping) and emits `include/theme_tokens.h` as RGB565 plus RGB888 values. Re-run it as a PlatformIO pre-build step or when tokens change.

Cyan scale (starting values, Tailwind v4 cyan, approximate, VERIFY by eye on the panel):

| Step | OKLCH |
|---|---|
| 300 | `oklch(86.5% 0.127 207)` |
| 400 | `oklch(78.9% 0.154 211.5)` |
| **500 (primary)** | `oklch(71.5% 0.143 215.221)` |
| 600 | `oklch(60.9% 0.126 221.7)` |
| 700 | `oklch(52% 0.105 223)` |
| 800 | `oklch(45% 0.085 224)` |

Roles: **primary** = 500 (fills, active toggles, progress). **accent** = 400 on dark, 700 on light (focus ring, icons, links, small text). **secondary** = low-chroma accent-tinted neutrals (surfaces and dim text below). Use 600/800 for pressed states.

### 3.2 Theme tokens (starting values)

| Token | Dark | Light |
|---|---|---|
| `bg` | `oklch(13.5% 0.03 h)` | `oklch(92% 0.026 h)` |
| `bgGradEnd` (wallpaper bottom) | `oklch(21% 0.072 h)` | `oklch(86.5% 0.044 h)` |
| `surface` | `oklch(23% 0.034 h)` | `oklch(99.4% 0.003 h)` |
| `surface2` (raised/pressed) | `oklch(28.5% 0.04 h)` | `oklch(84.5% 0.034 h)` |
| `border` | `oklch(34% 0.036 h)` | `oklch(70% 0.045 h)` |
| `text` | `oklch(96.5% 0.008 h)` | `oklch(19% 0.03 h)` |
| `textDim` | `oklch(73% 0.03 h)` | `oklch(42% 0.036 h)` |
| `primary` | cyan-500 | cyan-500 |
| `accent` | cyan-400 | cyan-700 |
| `onPrimary` | `oklch(20% 0.03 225)` | `oklch(20% 0.03 225)` |
| `ok` | `oklch(72% 0.17 155)` | `oklch(58% 0.15 155)` |
| `warn` | `oklch(80% 0.15 80)` | `oklch(62% 0.15 70)` |
| `err` | `oklch(66% 0.20 25)` | `oklch(55% 0.20 25)` |

`h` = the hue of the active accent's 500 shade (3.4); cyan gives 215.

Rules:
- **Dark bg is a near-black of the accent, not pure black** (this is an IPS LCD, so true black looks flat grey).
- **cyan-500 is too low-contrast for text or thin icons on light backgrounds.** Use `accent` (cyan-700) there.
- **Text on primary fills is near-black** (`onPrimary`), never white.
- Body text contrast at least 4.5:1, icons and large text at least 3:1.
- RGB565 bands in gradients: every gradient sets `bg_dither_mode` to `Theme::kGradientDither` (error diffusion). `LV_DITHER_GRADIENT` alone does nothing, and the deep tints sit below one RGB565 step, so undithered they band and lose their hue.
- ST7789 panels shift colors; compare on hardware and adjust tokens, not code.

### 3.3 Theme switching

- Themes: **dark**, **light**, optional **auto** (by time of day via NTP, set from the companion).
- Switch by swapping the active palette and re-applying styles (`lv_obj_report_style_change` or equivalent). Default behavior is an **instant recolor** (no crossfade). Persist in `Preferences`; read it at boot before drawing anything.
- **Media screen and AOD are always dark** regardless of theme (see sections 9.6 and 10).

### 3.4 Accent themes

- The user picks the accent from the **17 vibrant Tailwind v4 scales** (red, orange, amber, yellow, lime, green, emerald, teal, **cyan** (default, brand), sky, blue, indigo, violet, purple, fuchsia, pink, rose). Tailwind's neutrals are not offered.
- The accent swaps `primary`, `accent`, `pressed` and `onPrimary`, using the same steps as cyan (3.1). The neutrals (`bg` through `textDim`) take the accent's hue at fixed lightness and chroma in both modes. Dark: the wallpaper falls from a near-black of the accent into a deep, concentrated tint of it (rose gives wine, amber gives espresso, indigo gives midnight). Light: a pale wash of it (rose gives blush, amber cream, emerald mint), with ink in the same undertone. Status colors stay the same for every accent.
- `onPrimary` is a hue-tinted near-black or white, whichever contrasts more in dark mode. Light mode keeps white unless it drops under 3:1 (yellow, amber, lime fills).
- `tools/gen_theme.py` is the single source. It emits every accent's palettes into `include/theme_tokens.h` (`kPalettes[accent][dark/light]`, in flash) and the same table into `companion/src/ui/accents.rs`. Switching accents only changes an index; nothing is mutated mid-frame.
- One setting drives both UIs. The companion picks it with gradient swatches (500 into 700, top-left to bottom-right) or from the tray. The device can cycle it in Settings. Changes sync both ways through `config` / `config_ack`, persisted by id (`accent` in `Preferences`).
- **Companion only:** fills (`primary`/`pressed`) use 70% of the Tailwind chroma at the same lightness to cut glare on large glass fills; `accent` and the swatches stay full strength. Button ink takes whichever contrasts more. The window backdrop is tinted with the accent hue (dark `oklch(26% 0.05 h)` → `oklch(13% 0.015 h)`, light `oklch(98% 0.012 h)` → `oklch(92% 0.03 h)`, same glass alphas). Selected options in button groups are tinted chips, not solid fills. The device keeps full-strength accents.
- Media and AOD backgrounds still follow album art. A track without artwork (or with grayscale art) uses a deep tint of the accent, `oklch(34% 0.11 h)` (`kMediaTint`), into the accent-tinted `bg`; the companion signals this by sending the legacy defaults `#008DA6` / `#071115`.

## 4. Typography

- Family: **Inter** (SIL OFL). Weights: Regular and SemiBold only.
- Convert with `lv_font_conv`, **4 bpp**, limited glyph ranges, stored in flash.

| Role | Size | Weight | Use |
|---|---|---|---|
| caption | 12 | Regular | timestamps, units, hints |
| body | 14 | Regular | artist, notification body, labels |
| title | 18 | SemiBold | track title, card titles, app name |
| display | 28 | SemiBold | big numbers (CPU %), AOD clock |
| hero | 48 | SemiBold | Home clock |

- Glyph coverage v1: Basic Latin, Latin-1 Supplement, Latin Extended-A, Greek, Cyrillic, plus `° % · – — … ’ “ ”` and digits. Media titles can contain any script, so the companion (`sanitize_for_font`) first composes accents (players often send `n` + a combining tilde rather than `ñ`), keeps every character the fonts cover, reduces other accented letters to their base letter (`ạ` → `a`), and drops the rest, so the device never shows missing-glyph boxes. The device copies text into its fixed buffers without splitting a UTF-8 character. Every other script (CJK, Arabic, Hebrew, Bengali and other Indic scripts, emoji) is drawn by the companion and sent as glyphs (11.9), so the fonts themselves stay this size.
- Long text: ellipsis by default; use slow circular scroll (`LV_LABEL_LONG_SCROLL_CIRCULAR`) only for the **focused media title**.

## 5. Icons: Lucide everywhere

- Source: official **Lucide** icon font (ISC license; ship the license file in the repo). Do not mix in other icon sets.
- Build an **LVGL icon font** from the subset below with `lv_font_conv` at **24, 32 and 48 px**, 4 bpp. Avoid 16 px (2 px strokes blur). Generate `src/ui/icons.h` (names to codepoints) with `tools/gen_icons.*` so adding an icon means adding its name to a list.
- Icons are monochrome glyphs: color them with theme tokens (`text`, `textDim`, `accent`, `onPrimary`).

Subset (Lucide names; trim or extend as needed):
`house, layout-grid, clock, activity, cpu, memory-stick, network, bell, bell-off, music, play, pause, skip-back, skip-forward, volume-2, volume-x, settings, sliders-horizontal, sun, moon, sun-moon, wifi, wifi-off, bluetooth, bluetooth-off, usb, plug, battery-medium, refresh-cw, power, chevron-left, chevron-right, x, check, circle-alert`

## 6. Wallpaper (optional, never required)

Used **only on Home and Launcher**; apps use solid `bg`. Cards on top are solid.
Options, cheapest first (default = 1):
1. **LVGL vertical gradient** from `bg` to `bgGradEnd` (zero memory, dithered). In dark mode both follow the accent hue.
2. **Static image from flash**, 320x170 RGB565 (about 109 KB), one per theme. Loaded straight from flash or copied once into PSRAM.
3. **Companion-pushed picture** (`wallpaper:"image"`), picked in the companion's Device settings, same format and size, saved in flash (11.8).

No alpha-blended overlays on top of wallpapers. The picture is recolored toward `bg` (50%) as it is drawn, in the same pass, so text and cards stay legible in either theme; until it has arrived the gradient shows.

## 7. Motion

Rules: animate **small areas only**, never more than one large animation at a time, always ease-out, always honor a **`reduce_motion`** flag (all durations become 0). Target at least 30 fps during any animation; check with LVGL's perf monitor in dev builds and shorten or remove an animation that drops below.

| Motion | Duration | Notes |
|---|---|---|
| Focus ring moves | 120 ms | position/opacity of the ring only |
| Value change (bar, number) | 150 ms | animate the bar width, not a repaint of the card |
| Toast / small popup | 150 ms | |
| Screen/app switch | 0 ms | hard cut |
| Theme switch | 0 ms | instant recolor |
| Boot backlight ramp | 300 ms | PWM fade, no redraw |
| AOD enter | 450 ms | single color tween (section 10) |
| AOD track-change tint | 450 ms | same tween, old tint to new tint |
| AOD exit | 180 ms | faster than enter |

## 8. Input and navigation

Two buttons: **Next** and **Select** (physical mapping in `config.h`; believed GPIO 0 and GPIO 14, VERIFY; note GPIO 0 is the BOOT strapping pin, fine at runtime).

Gesture engine: debounced; **short press fires on release immediately**. Long press threshold about **500 ms**. A **chord** (both buttons held together within about 80 ms) is its own gesture. **Double press is delivered only if the active app opts in** for that button, so that short presses elsewhere have zero added latency.

| Gesture | Launcher / Home | In an app (default) | AOD |
|---|---|---|---|
| Next short | focus next | app-defined (default: next item) | wake only |
| Next long | n/a | **open Launcher** | wake only |
| Select short | open focused app | app-defined primary action | wake only |
| Select long | n/a | **back** | wake only |
| Chord (both) | open **Control Center** | open **Control Center** | wake only |

- Any press in AOD **wakes and is consumed** (never forwarded to the app).
- App interface additions (in `core/App.h`):

```cpp
virtual bool wantsDouble(Button b) const { return false; } // opt in to Press::Double
```

- Media app opts in for **Next double = previous track** (accepting about 250 ms latency on Next-short there).

## 9. Screens

Wireframes are approximate (320x170); dimensions are starting values.

### 9.1 Status bar (22 px, all screens except Boot, AOD, Media)
Left: screen title (caption, `textDim`). Right: connection icon (`usb` / `wifi` / `wifi-off`), `bluetooth` if active, `bell` if unread notifications. 20 px icons, 8 px gaps.

### 9.2 Launcher (Redmi-style menu, horizontal carousel)
```
[ Status bar                                   ]
 +--------+  +--------+  +--------+
 |  icon  |  |  icon  |  |  icon  |   88x88 tiles, radius 16, gap 16
 |  48px  |  | FOCUS  |  |        |   focused: 2 px accent ring
 +--------+  +--------+  +--------+
              App name (title 18)
                 o  o  *  o          page dots (optional if crowded)
```
- 3 tiles visible, focused tile centered; Next cycles forward with wrap. No scaling. Order: Home, Media, Stats, Notifications, Settings.
- Tiles use `surface`; icon color `text`, focused icon `accent`.

### 9.3 Home (dashboard)
Hero time (48) left-aligned, date (body, `textDim`) beneath. Right column: two small chips with CPU % and RAM % (from the companion). If media is playing, a slim now-playing chip (icon `music` + title, ellipsized) along the bottom; Select opens Media.

### 9.4 Stats
Left column: two stacked cards, **CPU** and **RAM**: label (caption), value (display 28), thin 4 px bar (`primary` on `surface2`). Right wide card: **Network**: up/down values (body) above a sparkline (about 60 samples, `lv_chart`, 1.5 px line, `accent`). Bars update with the 150 ms value animation. No arcs or heavy gauges.

### 9.5 Notifications
Vertical list of solid cards, newest first: icon (24), title (body SemiBold), 2-line body (caption), relative time. Next scrolls, Select dismisses the focused item, Select long goes back. Empty state: `bell-off` icon + "All clear".

### 9.6 Media (Now Playing, Spotify-inspired)
Always dark. Background: the **tinted gradient** from the companion (`bg_top` to `bg_bottom`, section 11); falls back to the dark `bg` if none.
```
 +----------+   Title (title 18, 1 line, scroll if long)
 |  album   |   Artist (body, dim)
 |  128x128 |
 |  r=10    |   ====o----------  progress (4 px, primary on ~30% white)
 +----------+   1:23                       3:35   (caption)
                [skip-back] [ (pause) ] [skip-forward]
```
- Art at x=10, y=30, rounded corners via LVGL radius. Right column starts at about x=154.
- Controls: icons 24 px; center button is a **primary-filled circle** with `onPrimary` icon. Show only the state icon (`play` when paused, `pause` when playing).
- Input: Select short = play/pause, Next short = next track, Next double = previous track, Select long = back.
- Progress is **interpolated locally** (section 11); redraw the bar once per second.

### 9.7 Control Center (opened by chord, full-screen 2x2 tiles)
Tiles: **Brightness** (Select cycles 20/40/60/80/100%), **Theme** (`sun` / `moon`, Select toggles), **Connection** (shows USB/WiFi state, Select = reconnect), **AOD** (Select = enter AOD now). Next moves focus, Select long or chord again closes.

### 9.8 On-device Settings (minimal, everything else is in the companion)
Only: WiFi setup/reset, brightness, theme, AOD timeout, **Set Up Again** (back to 9.9), reboot, about/version. Everything else is configured from the Rust companion and pushed to the device (section 11.3).

### 9.9 First-run Setup
Shown instead of Home until setup is done (`setup_done` in `Preferences`; devices provisioned before the wizard count as done). No status bar, no AOD, no Control Center; Select long skips.
```
 HAWAOS SETUP            +-----------+
 Welcome                 |  (bt)     |
 Open the HawaOS         | HawaOS-7F3A
 Companion and choose    |   482     |   <- 6-digit pairing code,
 Add device.             |   913     |      two rows of three
 Hold Select to skip     | or plug in USB-C
                         +-----------+
```
The left column follows the stage: Welcome, Connected (code still shown until a BLE peer pairs), Joining WiFi (spinner in the card), Couldn't connect (wrong password / out of range / no answer, in `err`), You're online (SSID + IP), All set (2.5 s, then Home).

## 10. AOD (always-on display)

Concept: after an idle timeout the UI dims into a low-power ambient mode. When media plays, the background **shifts from black to a deep tint of the album's dominant color** and shows what is playing, with the clock at the top.

**State machine**

| State | Display |
|---|---|
| Active | normal UI, backlight at user level |
| AOD, no media / stopped | black background, clock **centered** (display 28 to 48), dim |
| AOD, media playing | tinted gradient, **clock at top**, album art + title + artist, thin progress bar |
| AOD, media paused | stays tinted for **30 s**, then tweens to black + clock |

- Enter AOD after the idle timeout (default **60 s**, configurable from the companion). Any button press exits.
- **Tint, not flood.** The companion sends pre-darkened gradient colors (section 11); never fill the screen with the raw dominant color. Text uses the `ink` hint (light/dark) from the companion for contrast.
- **Layout (media playing):**
```
          12:34                      clock at top, display 28, centered
  +------+   Title (title 18)
  | art  |   Artist (body)
  | 96px |
  +------+
  ----------o----------------------  thin 3 px progress, full width, 20 px margins
```
- **Animation:** one value `t` (0 to 1) animated with `lv_anim`, 450 ms ease-out; each frame sets the background gradient colors via `lv_color_mix(from, to, t)`. Content (art, text) appears on the last frame or with a short opacity step on small objects only. Track changes tween from the previous tint to the new tint with the same function. Exit: 180 ms back to the normal UI.
- **Power:** AOD is always dark regardless of theme. It is an LCD, so black does not save power and burn-in is not a concern. Savings come from **backlight PWM at about 15 to 25%**, UI timer at about **5 Hz**, and 1 Hz progress updates. Do not use light sleep (it would drop WiFi/USB). **Screen off:** `aod_off_min` (companion: Behavior > Screen off after; 0 = never, the default, or 5/15/30/60) turns the backlight fully off after that long in AOD, and stops drawing. A running timer or stopwatch keeps it lit and restarts the count. A notification lights it again (and restarts the count); a button press on the dark screen wakes the device fully, so it can't toggle a timer unseen.

## 11. Companion contract (media, config)

The transport is an open decision (see section 14). Design a `Transport` abstraction so the same messages can travel over **WiFi (UDP for small messages, HTTP or TCP for bulk)** and optionally **USB CDC serial** (length-prefixed frames `[u8 type][u32 len][payload]`). For UDP, one JSON message per datagram, under about 1 KB. The device learns the companion's address from incoming datagrams.

### 11.1 Media state (companion to device)
```json
{"type":"media","state":"playing","title":"...","artist":"...","album":"...",
 "pos_ms":83000,"dur_ms":215000,"ts":1761200000000,
 "art":"a1b2c3d4","bg_top":"#0f3a44","bg_bottom":"#05090b","ink":"light"}
```
- Send on every change and as a **5 s heartbeat**. `state`: `playing | paused | stopped`.
- Device computes position as `pos_ms + (now - receive_time)` while playing, and resyncs on each message.
- `bg_top` / `bg_bottom` / `ink` are computed by the companion (extract the dominant color from a small downscale, clamp OKLCH lightness so it is a deep tint, keep contrast with light or dark text). **The device does no color math.**
- Text in scripts the device fonts lack is sent as glyphs: `gl` and `gl_len` name the pack (11.9). The same goes for `notify` and `weather`.

### 11.2 Album art
- Format: **128x128, RGB565 little-endian, raw, 32 768 bytes**, center-cropped, no alpha. The device only copies bytes into a PSRAM buffer, so **no JPEG decoder is needed**.
- The device fetches art only when the `art` hash differs from the cached one. Default for WiFi: the device **pulls** `GET http://<companion>:7778/art/<hash>` (companion runs a tiny HTTP server). Once paired the response is encrypted: `iv || ciphertext || tag`, 32 796 bytes (11.6). Without HTTP (Bluetooth mode, or USB only) the device sends `{"type":"art_req","hash"}` and the companion **pushes** the same bytes as in-order `{"type":"art_data","hash","off","b64"}` lines of 512 raw bytes (11.7). The device re-asks only after 5 s without a chunk, and gives up on a hash after 3 failed loads (a key mismatch would otherwise repeat forever).
- Show a placeholder (Lucide `music` on `surface2`) until art arrives or when there is none.

### 11.3 Commands and config
Device to companion: `{"type":"cmd","cmd":"play_pause|next|prev|seek","pos_ms":0}`.
Companion to device (persisted in `Preferences`, acknowledged with `config_ack`):
```json
{"type":"config","v":3,"theme":"dark|light|auto","accent":"cyan","brightness":60,
 "aod":true,"aod_timeout_s":60,"aod_off_min":0,"reduce_motion":false,"wallpaper":"gradient|solid|image",
 "wallpaper_hash":"7bb6219d"}
```
`wallpaper_hash` is sent with `image` only (11.8); `config_ack` reports the saved picture's hash (`""` if none).
`v` is a version counter; the device applies and stores the highest version it has seen. `accent` is a Tailwind color id (3.4); unknown ids are ignored. The companion keeps `v` in step with acks and ignores the accent in an ack older than its latest push, so quick changes never flash an old color back.

### 11.4 First-run setup and provisioning
A factory device (or one sent back with Set Up Again) runs `SetupService`: it shows 9.9, listens on USB, and advertises over **BLE** as `HawaOS-XXXX` (last MAC bytes). Manufacturer data `0xFFFF 'H' <0|1>` says whether it is in setup (0) or set up (1), so discovery can tell them apart. In Wi-Fi mode BLE only runs during setup and is shut down afterwards (`NimBLEDevice::deinit`), so it costs no RAM in normal use; in Bluetooth mode it stays up (11.7). While it runs, WiFi modem sleep stays on (required for coexistence).

BLE is just another JSON-lines transport: service `48415741-4f53-4000-8000-000000000001`, RX `...0002` (write), TX `...0003` (notify, MTU-sized chunks). USB, UDP and BLE all feed the same handler.

| Message | Direction | Notes |
|---|---|---|
| `query` -> `config_ack` | both | ack adds `id`, `model`, `fw`, `setup` (true = needs setup) |
| `setup {stage:"begin", host}` | companion -> device | device shows "Connected" |
| `wifi_scan` -> `wifi_scan {nets:[{ssid,rssi,secure}]}` | both | the device's own 2.4 GHz view, strongest first, deduplicated |
| `wifi_join {ssid, pass}` | companion -> device | USB, or encrypted UDP (11.6) |
| `wifi_creds {nonce, ct}` | companion -> device | BLE: AES-256-GCM of `{"ssid","pass"}` |
| `wifi_status {state, ssid, ip?, reason?}` | device -> companion | `connecting`, `connected`, `failed` (`auth` / `no_ap` / `timeout`) |
| `setup {stage:"done"}` | companion -> device | persists `setup_done`, device goes Home |

**BLE pairing.** `setup_key {pub}` both ways (X25519, base64), then key = SHA-256(shared || `"hawaos-setup-v1"` || code), where the code is the 6 digits on the device's screen. `setup_verify {nonce, ct}` proves the companion derived the same key (reply `{ok, attempts_left}`); after 5 wrong codes the device shows a new one. Until verified, BLE accepts only `query`, `wifi_scan`, `setup*`; WiFi fields never travel over BLE unencrypted. A failed join during setup clears the credentials so a wrong password is not retried forever.

**Time without WiFi.** Until NTP syncs, the device sets its clock (and TZ from the saved offset) from the `ts` in companion stats, so USB-only devices keep correct time.

**Companion wizard** (Add device page; opens automatically when no device is known; tray "Add Device..."): Connect (USB or Bluetooth discovery) -> Pair (BLE only, code entry) -> Wi-Fi (device's network list, password, or "Skip, use USB only") -> Personalize (accent, theme, clock; pushed live, together with every other setting the companion keeps, so a board set up again takes them instead of its defaults, whose acks would otherwise overwrite the companion's) -> Done. While it runs, the daemon stays off the USB port.

**Companion token.** `setup {stage:"done"}` carries a fresh 128-bit token (32 hex): `token` over USB, `token_nonce`/`token_ct` sealed with the pairing key over BLE. The device stores it in `Preferences` (`cmp_token`), the companion in `device_token`. All network traffic is encrypted with keys derived from it (11.6), and firmware update requests are signed with it (11.5). Setup messages (`setup`, `setup_key`, `setup_verify`, `wifi_creds`) are refused over UDP (`auth_error`/`setup_only`), so the token can only be set over USB or a paired BLE link.

### 11.5 Firmware updates (OTA)
The device always pulls the image itself over WiFi; the companion only serves it. Installed from the Firmware card on the companion's Device page ("Install update..." -> pick a PlatformIO `firmware.bin`).

1. The companion checks the file starts with the ESP32 image magic `0xE9` and fits an app slot (6.25 MiB, `default_16MB.csv`), then serves it from the art HTTP server at `http://<lan ip>:7778/fw/<id>.bin` (`id` = first 16 hex of its SHA-256).
2. It sends `{"type":"ota","url","size","sha256"}`. Over **USB** that is enough (physical access). Over **UDP** it adds `mac` = HMAC-SHA256(token, `"<sha256>:<size>"`); a missing or wrong MAC is answered `failed`/`unauthorized`. BLE never accepts `ota`.
3. The device streams the download into the inactive app slot (`Update`), hashing as it goes. Only if all `size` bytes arrive **and** the SHA-256 matches **and** `Update.end()` accepts the image does it mark the new slot bootable and restart. Any failure aborts the write; the running firmware is untouched.

| Message | Direction | Notes |
|---|---|---|
| `ota {url, size, sha256, mac?}` | companion -> device | `mac` required over UDP |
| `ota_status {state, pct, error?}` | device -> companion | `downloading` (every 5%), `verifying`, `done` (restarts ~1 s later), `failed` |

Errors: `unauthorized`, `rejected` (malformed, or an update already running), `no_wifi`, `bad_url`, `http_error`, `size_mismatch`, `no_space`, `flash_write` (also a bad image header on the first block), `incomplete`, `checksum`, `bad_image`. The companion shows each as a plain sentence and gives up with `no_response` after 20 s without a status (the device rebooted mid-download into its old firmware).

Device UI: while an update runs, a full-screen overlay on `lv_layer_top` shows the progress and keeps the screen awake (leaves AOD, no AOD timeout); a failure stays on screen 4 s, and a new request may start straight away. The companion clears its "done" status when the device answers again after restarting; the `fw` in `config_ack` shows the new version.

Known limits: the download is plain HTTP on the LAN, protected by the hash. Over UDP the request itself travels encrypted with the replay counter (11.6), so a captured one can't be resent. Rollback covers firmware that breaks after it boots (below); one that bricks the bootloader or partition table can't happen this way, since updates only write the app slot.

**Rollback.** The stock bootloader has rollback enabled, so a freshly installed slot boots as `PENDING_VERIFY`; `FirmwareHealth` overrides Arduino's `verifyRollbackLater()` so the app decides instead of confirming itself at once. The new firmware confirms itself (`esp_ota_mark_app_valid_cancel_rollback`) once it has been up 20 s with the UI task ticking (heartbeat under 5 s old) and an accepted message from the companion: sealed over UDP or BLE, or anything over USB, since someone is at the cable. An update that cuts the link would otherwise leave the device reachable only by USB. If that hasn't happened within 3 minutes it calls `esp_ota_mark_app_invalid_rollback_and_reboot()`. The check runs on an `esp_timer`, so a hung UI or network task can't stall it. A crash or reset before confirmation (a panic at boot, a watchdog, pulling the power) makes the bootloader go back on the next boot. While on trial, a new `ota` request is refused with `verifying`: it would overwrite the slot a rollback returns to. USB-flashed firmware is never on trial.

The firmware it went back to reports `"rolled_back":"<id>"` in every `config_ack` (`id` = first 4 bytes of the failed image's ELF SHA-256, from `esp_ota_get_last_invalid_partition()`) until the next update reuses that slot. The companion shows "The new firmware didn't start properly, so the device went back to version X" once per id (`rollback_seen` in its config) and marks the update failed. Firmware before 0.8.0 doesn't report this, so the first rollback to 0.7.0 only shows as the version staying the same.

### 11.6 Encrypted network traffic
Once the device has a token, all UDP traffic in both directions is encrypted and authenticated with **AES-256-GCM** (USB and paired BLE stay as they are). One key per direction, so the two sides never share a key and nonce: `k_c2d` = SHA-256(token || `"hawaos-link-c2d"`), `k_d2c` = SHA-256(token || `"hawaos-link-d2c"`). IVs are 12 random bytes per message; `ct` is base64(ciphertext || 16-byte tag).

**Companion to device:** `{"type":"sealed","n":1761495600000,"iv":"<b64>","ct":"<b64>"}`, with the decimal `n` as associated data. `n` is the companion's Unix time in ms, kept strictly increasing in-process; the device accepts it only if it is greater than the last accepted one. The plaintext is handled as if it had arrived over UDP; envelopes don't nest; `ota` still carries its own MAC as well (11.5).

**Device to companion:** `{"type":"sealed","e":<u64>,"d":<u32>,"iv","ct"}`, with `"<e>:<d>"` as associated data. `e` echoes the newest companion `n` the device accepted, `d` counts the device's messages since boot. The companion accepts only if `e` is from its own last 120 s (covers a firmware download, when the device can't read) and (`e`, `d`) is greater than any pair seen, so a captured play/pause or ack can't be replayed.

**Replay counter.** It lives in RAM and is saved to `Preferences` (`auth_n`, cleared when a new token is set) only when a sealed `config`, `wifi_join` or `ota` is accepted, so stats every second don't wear the flash. A captured message that changed the device can never be accepted again, even after a reboot; a captured display message newer than the last change could be shown once more after a reboot.

**Peer.** The address replies, media commands and album-art fetches go to is taken only from a trusted packet (sealed, or any packet while the device has no token), so a forged packet can't redirect them. Clock commands from the UI task are queued like media commands, so only the WiFi task touches the socket and the cipher.

**Refusals** go back to the sender only, in plaintext since that sender is untrusted: `{"type":"auth_error","reason","for"}` with `plaintext`, `bad_mac` (failed decryption), `stale` (old `n`), `setup_only` (11.4). With a token, that is the only plaintext the companion accepts over UDP; it shows at most one banner a minute suggesting Add device or USB.

**Album art** (11.2) is sealed with `k_c2d` as `iv || ciphertext || tag`, with `"art:<hash>"` as associated data: the hash comes from the encrypted `media` message, so an image can't be read on the LAN or swapped onto another track. The device decrypts in place; on a failed tag it forgets the loaded hash, so the overwritten buffer is never shown as art, and retries.

**Sizes.** The companion trims notification and media text to the device's field sizes (DataStore.h) so any envelope fits one datagram; the device opens up to 1100 bytes of plaintext.

A device without a token (set up before encryption) still talks plaintext until Add device runs again. Not covered: firmware images travel over plain HTTP (hash-checked, the request encrypted, and they hold no secrets), and traffic sizes and timing are visible on the LAN.

### 11.7 Bluetooth link mode
A device can talk to the companion over **Bluetooth instead of Wi-Fi**. `Settings::LinkMode` (`link_ble` in `Preferences`) is `wifi` or `ble`; USB works in both. It is chosen in the wizard's Wi-Fi step ("Use Bluetooth") or on the companion's Device page (Connection card), and travels as `config {link:"ble"|"wifi"}`; `config_ack` reports it back as `link`, and the companion follows the device's value.

| | Wi-Fi mode | Bluetooth mode |
|---|---|---|
| BLE | setup only | always on, advertising `paired` |
| Wi-Fi radio | joins the saved network | off; the saved network is kept |
| Clock | NTP | `ts` in companion stats |
| Album art, wallpaper | device pulls over HTTP | companion pushes `art_data`, `wp_data` |
| Firmware update | as 11.5 | Wi-Fi turns on for the download, then off again |

**Switching.** The device stores the new mode, acks it over the current link, and applies it 1.5 s later: Bluetooth mode starts BLE and switches Wi-Fi off; Wi-Fi mode stops BLE and rejoins. `ble` is refused without a token, since the everyday BLE traffic is sealed with it, and `wifi` without a saved network, which would leave the device reachable only over USB (the companion disables that button too). Saving a network in Bluetooth mode (`wifi_join`, `config.wifi_ssid`) only stores it and answers `wifi_status {state:"saved"}`; the wizard takes that as done and its finish switches the device to Wi-Fi. During setup (Set Up Again) Wi-Fi may always join, and the wizard always ends by stating `link`, saving whatever the device's ack confirms.

**Finding each other again.** The companion queries every 20 s over its current link. If the device has been silent for 45 s, it also tries the other transport: in Bluetooth mode a sealed UDP query, in Wi-Fi mode a Bluetooth scan every 30 s. The device's ack carries `link`, so a switch whose ack was lost heals within about a minute.

**Security.** Outside setup, BLE follows the UDP rules of 11.6. The GATT link is open, so anyone nearby can take the one connection (denial of service only: nothing is accepted from them), and the daemon ignores a device advertising that it is in setup. The rules: with a token, only `sealed` envelopes are accepted (same keys and replay counter), except `art_data`, `wp_data` and `gl_data` chunks, which are pieces of an encrypted blob checked when complete. Plaintext is answered `auth_error`/`plaintext`, setup messages `setup_only`, and the device's own messages over BLE are sealed. The GATT link itself is unauthenticated; everything rides on the app-layer encryption.

**Companion.** `ble_daemon.rs` keeps a connection to `HawaOS-<device_id>` while `use_ble` is set and the wizard isn't running, reconnecting every few seconds when it drops. It writes without response (link-layer acknowledged) and the device asks for a 7.5–15 ms connection interval, so a 32 KB cover arrives in about a second. A firmware update or link switch requested while the link is reconnecting waits up to 15 s for it.

**Firmware update over Bluetooth mode.** The `ota` request arrives sealed over BLE; `OtaService` calls `WifiService::holdForUpdate(true)`, waits up to 25 s for the saved network, then downloads as in 11.5. Without a saved network it fails with `no_wifi`.

**Not connected.** Apps that show live companion data (`App::needsCompanion()`: Stats, Media, Weather) are covered by a "Not connected" card with `bluetooth-off` while `CompanionLink::isLinked()` is false: in Bluetooth mode, no sealed BLE message and no USB stats in the last 8 s (a link can stay "connected" on the PC side after the companion quits). The status bar shows `bluetooth` (accent) or `bluetooth-off` (dim) in place of the Wi-Fi icon, and Home's chip says "Linked over Bluetooth" or "Waiting for the companion (Bluetooth)".

### 11.8 Image wallpaper
The companion turns a picked picture into the device's wallpaper (`wallpaper.rs`): the largest centered crop with the screen's 320:170 ratio, Lanczos-resized, then Floyd-Steinberg dithered to RGB565 so photos don't band. It keeps the result as `wallpaper-<hash>.png` next to `config.json`, already in the device's colors, and serves the pixels from it; `<hash>` is the first 4 bytes of SHA-1 over the RGB565 data, as for art.

Transfer works like album art (11.2), with `wp` for `art`: the device pulls `GET /wp/<hash>` over Wi-Fi, or asks with `{"type":"wp_req","hash"}` and receives `wp_data` chunks over Bluetooth or USB. Paired, the blob is `iv || ciphertext || tag` with AAD `wp:<hash>`, 108 828 bytes; about 3.5 s over Bluetooth, 2 s over USB. Both use one transfer engine on each side (`ImageSlot` in `CompanionLink.cpp`, `image_chunks` in `worker.rs`), each image with its own buffer, so a cover and a wallpaper can arrive at the same time.

The device keeps it in the `spiffs` data partition of the default table, which nothing else uses and which OTA updates don't touch: a header in the first 4 KB sector (magic `HWP1`, size, hash, CRC-32 of the pixels), the pixels from 4 KB. The header is written last, so an interrupted save reads back as no picture. Downloads land in a PSRAM staging buffer; `Wallpaper::commit()` saves it (the erase pauses flash access, and so drawing, for about a second) and the UI task swaps it in on `SettingsChanged`, so the screen never shows half a picture. At boot the saved picture is loaded into PSRAM if its CRC checks out. A device on older firmware treats `image` as `gradient`, and its ack moves the companion back to that.

### 11.9 Text in other scripts
The device fonts cover Latin, Greek and Cyrillic (section 4). Fonts for everything else would cost megabytes of flash and still couldn't draw Bengali or Arabic, whose letters change shape with their neighbours. So the companion draws that text itself and sends the result as glyphs (`glyphs.rs`):

1. It shapes the text with the system's fonts (`cosmic-text`: HarfBuzz-style shaping, bidi, font fallback), the way a desktop would draw it. A color emoji font that doesn't rasterise is retried in Noto Emoji.
2. A cluster the device can draw itself (one Latin, Greek or Cyrillic character) stays as text. Any other cluster (a CJK character, a Bengali syllable, an Arabic letter in its joined form, an emoji) becomes one private-use character, `U+E000`–`U+F8FE`, and the clusters go out in **display order**, so the device draws left to right without knowing about bidi or shaping. Fields that wrap across lines (notification bodies) read right to left within each line only.
3. Each cluster is rasterised at the device's three text sizes (12, 14, 18 px) as a 4-bit alpha bitmap, the same layout as LVGL font bitmaps. The message names its **glyph pack** with `"gl":"<hash>","gl_len":<bytes>`: `HGL1`, a 32-bit session id, a glyph count, then per glyph its character, size, box, offsets, advance and bitmap. Packs are at most 32 KB.

The device fetches packs like album art (11.2), with `gl` for `art` (`GET /gl/<hash>`, or `gl_req` and `gl_data` chunks; sealed with AAD `gl:<hash>`), queueing up to four while one downloads. `GlyphCache` adds them to a 384 KB PSRAM cache on the UI task. The Inter text fonts fall back to `font_glyphs_{12,14,18}` (set by `tools/gen_icons.py`), whose callbacks look glyphs up there, so every existing label (scrolling, colours, themes) draws them unchanged; afterwards labels holding such text are set again so they re-measure. A new session id (the companion restarted and is numbering clusters afresh) clears the cache, as does running out of room, after which messages bring back what they need. Until a pack arrives the text is simply blank (`LV_USE_FONT_PLACEHOLDER 0`).

The companion only sends glyphs to firmware that says it can draw them (`"gl":1` in `config_ack`); older firmware gets the cleaned-up text of section 4. A message's text is converted once and cached, so the 5 s media heartbeat costs nothing and names the same pack, which the device already has.

### 11.10 Notification formatting
Notification bodies may carry markup (the freedesktop spec's `body-markup`; KDE advertises it, and browsers and chat apps use it). The companion parses it (`notifications.rs`) instead of throwing it away:

- `<b>`, `<strong>`, `<i>`, `<em>`, `<u>` become one emphasised run between `U+0002` and `U+0003`; nested ones merge, an unclosed one is closed.
- `<br>`, and the end of `<p>`, `<div>`, `<li>`, `<tr>`, and newlines in the text become line breaks (`\n`); blank lines are dropped.
- Links show their text, images their `alt`, other tags nothing. Entities (`&amp;`, `&#233;`, `&#x1F600;` …) are decoded here. A `<` that doesn't start a tag is text.
- The app name and summary are flattened to one plain line.

The device (`Theme::formatText`) draws emphasis in the theme's **accent colour** rather than a heavier weight: an LVGL label can't mix fonts, and a colour works everywhere, including the popup's scrolling line, with no extra font. Labels showing bodies have recolor on, so a run becomes `#RRGGBB text#` and a literal `#` is written `##` (closing and reopening a run around it). The Notifications list and the always-on alert keep line breaks (the list shows two lines); the popup's single line shows them as ` · `. Text is cut on character boundaries and a cut run is always closed.

The companion only sends these marks to firmware that reports `"fmt":1` in `config_ack`; older firmware gets one flat line. They pass through glyph conversion (11.9) untouched.

## 12. Boot logo

Sequence (target: logo visible as soon as the Arduino core hands over control):
1. First thing in `setup()`: keep the **backlight off**, init the panel, fill with the saved theme's `bg` (read `theme` and `accent` from `Preferences` first; default dark cyan), draw the logo centered, blended over that `bg` in the theme's colours (below).
2. **Ramp backlight** from 0 to the user level over about 300 ms (PWM fade, no redraws; hides the white power-on flash).
3. Initialize the rest of the system behind it. **Do not block on WiFi or the companion.**
4. Show the logo for at least about **600 ms**, then go to Home. The status bar shows "connecting" until a transport is up.

Asset: `assets/logo.svg`, a **one-colour mark** (a rounded square with the wind swirl cut out; brand colour `#0092B8`), and `assets/logo.png` rendered from it at 1000 px. Because it is one colour, only its shape is used, and it takes the **accent** wherever it appears:

- **Boot lockup** (220x88: the mark, "Hawa" + "OS", and the tagline). `tools/gen_logo.py` (a PlatformIO pre-build script) rasterises the SVG with ImageMagick (or downscales the PNG without it) and emits three **4-bit alpha layers** in `include/boot_logo.h`: the mark and "OS" (drawn in `accent`), "Hawa" (`text`) and the tagline (`textDim`). `Display.cpp` blends them over `bg` of the saved theme and accent with `pushImage`, before LVGL starts. One set of layers serves all 17 accents in both modes: about 29 KB, against 116 KB for the earlier per-mode RGB565 + A8 images.
- **Companion window** (sidebar and wizard): `ui::logo(pal.accent)` keeps the PNG's alpha at 96 px and fills it with the accent, one cached image per colour, so it changes with the accent.
- **App icon** (Flatpak, app menu) and **tray icon**: the brand colours, from the SVG (`hicolor/scalable`) and a 256 px PNG; these don't follow a device's theme.

## 13. Performance and memory

- RGB565 framebuffer is about 109 KB; the real constraint is internal RAM (WiFi + BLE + LVGL).
- LVGL partial-render buffers: benchmark **internal DMA-capable RAM vs PSRAM** (e.g. two buffers of about 320x20 lines); use internal RAM if it fits and is faster, else PSRAM. Large assets (art buffer, wallpaper, fonts' decoded data) go in PSRAM.
- Dev builds enable LVGL perf/memory monitors and log FPS, free internal heap and free PSRAM.
- Fonts and icons in flash; no runtime font generation.

## 14. Open decisions (ask the user before building each)

1. **Transport:** WiFi only, USB serial as primary (no WiFi credentials, fast, reliable, but debug logs cannot share that port), or both behind the `Transport` abstraction.
2. **Button mapping:** which physical button is Next vs Select.
3. **LVGL version:** 8 vs 9 (APIs differ). Pick one, pin it, keep it consistent.
4. ~~**Unicode coverage** beyond v1~~: done, as companion-drawn glyphs (11.9).
5. ~~**Logo asset** format and colors~~: one-colour SVG, tinted with the accent (section 12).
6. **Wallpaper** default (gradient is the suggested default).
7. **Touch variant:** assumed non-touch.

## 15. Visual QA checklist (per milestone)

- Dark and light both look intentional; no hardcoded colors in app code.
- Text legible at arm's length; contrast rules in section 3.2 hold.
- No animation drops below about 30 fps; `reduce_motion` really disables them.
- Launcher, Home, Stats, Notifications, Media, Control Center and AOD all fully usable with two buttons.
- Media: art, title, artist, progress and controls update correctly on track change, pause and seek; unsupported characters never show boxes.
- AOD: enter/exit transitions work in all four states; clock at top when media is playing; backlight dims.
- Boot: no white flash, logo appears quickly, fade-in is smooth, theme matches the saved one.
- Heap and PSRAM stay stable over a multi-hour soak with media changing and AOD cycling.

## 16. Companion platforms (Linux, Windows)

Everything that differs between desktops sits behind `companion/src/platform`: one module per OS with the same functions, so the protocol, art processing, glyphs, setup and the window are shared. The tray's menu is shared too (`tray_actions.rs`), each platform only drawing it.

| | Linux | Windows |
|---|---|---|
| Now playing, position, media keys | MPRIS through `playerctl` | System Media Transport Controls (WinRT), the sessions Windows' media flyout shows |
| Album art | the player's `file://` or `http(s)://` URL | the session's thumbnail stream, read once per track |
| Notifications | `dbus-monitor` on the session bus (11.10) | not yet: `UserNotificationListener` only works for packaged, signed apps |
| Tray | StatusNotifierItem (`ksni`) | notification-area icon (`tray-icon`) on its own thread with a message loop |
| Start with system | `~/.config/autostart/*.desktop` | `HKCU\…\CurrentVersion\Run` value `HawaOS Companion` |
| Setup's Wi-Fi list | NetworkManager (`nmcli`) | WLAN API; 2.4 GHz only on both |
| Board's serial port | `/dev/ttyACM*`, `/dev/ttyUSB*` | COM ports with Espressif's USB vendor id `303a` |
| Settings | `~/.config/hawaos-companion` | `%APPDATA%\hawaos-companion` |
| Glyph fonts (11.9) | the system's (Noto), Noto Emoji for emoji | the system's (Nirmala UI, Yu Gothic, Microsoft YaHei, Malgun Gothic), Segoe UI Symbol for emoji |
| Package | Flatpak (`install.sh`) | Inno Setup installer (`install.ps1`), per user, no administrator rights |

**Update notice** (`update_check.rs`). Thirty seconds after start and then every 12 hours, unless **Check for updates** is off, the companion asks GitHub's API for the latest release (drafts and pre-releases aren't "latest") and compares its version numerically with its own. A newer one shows as a card at the top of the window (how to update this kind of install, **What's new** opening the release page, and a close button that hides it until the next version) and as the first tray menu entry. The tray asks the window to open the page (`open_url_requested`), since only gpui can open a browser on every platform.

**One companion per user** (`single_instance.rs`). The first one listens on a local socket named for the user (an abstract Unix socket on Linux, which the Flatpak shares with the host; a named pipe on Windows), and both disappear with the process, so a crash leaves nothing stale. A later launch connects to it and exits: a normal launch asks it to show its window, an autostart (`--minimized`) one just leaves. Two launches at the same moment can't both claim the name; the loser hands over too. Without this, a second copy fought the first over the device's ports and links.

Bluetooth (`btleplug`: BlueZ / WinRT), serial (`serialport`), system stats (`sysinfo`), the window (gpui) and the network code are cross-platform already. On Windows the release `.exe` runs without a console window and carries the app icon (`build.rs`); the first network use brings up Windows Firewall's prompt, which needs allowing on private networks for the Wi-Fi link.

**Builds.** `.github/workflows/windows.yml` tests and builds the Windows companion on every change and keeps the installer as an artifact; when a release is published (`tools/release.sh`), it attaches `hawaos-companion-<version>-windows-x64-setup.exe` and its checksum. The installer is unsigned for now, so SmartScreen warns when it's run from a browser download; `install.ps1` downloads it itself (no mark of the web, so no warning), checks the checksum, and runs it silently. The workflow tests that script: a first install, an update, and uninstall. Updates leave the autostart choice alone (the installer offers it on a first install only). Signing (which would also allow notification mirroring) is open for 1.0.0.

