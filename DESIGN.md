# DESIGN: HawaOS UI/UX Specification

Companion to `HANDOFF.md`. **Where the two conflict, this file wins** (button mapping, Control Center, App interface additions). Items marked **VERIFY** must be checked on the real device or against official docs. Values given as "starting values" are meant to be tuned after seeing them on the actual panel.

Target: LilyGo T-Display S3, 320x170 landscape ST7789 (8-bit parallel), LVGL, two buttons, no touch assumed.

## 1. Principles

1. **Calm and sparse.** One idea per screen, 1 to 3 elements, one big number or title that matters.
2. **Solid, not glass.** No translucency or blur. Cards are solid fills. Alpha blending is avoided everywhere except small, rare cases.
3. **Subtle and snappy motion.** Animations are short, small, and optional. FPS beats flourish (see section 7).
4. **Modern and consistent.** One accent hue (cyan), tinted neutrals, one icon family (Lucide), one type family.
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

Roles: **primary** = 500 (fills, active toggles, progress). **accent** = 400 on dark, 700 on light (focus ring, icons, links, small text). **secondary** = low-chroma cyan-tinted neutrals (surfaces and dim text below). Use 600/800 for pressed states.

### 3.2 Theme tokens (starting values)

| Token | Dark | Light |
|---|---|---|
| `bg` | `oklch(17% 0.018 225)` | `oklch(97.5% 0.006 215)` |
| `surface` | `oklch(22% 0.02 225)` | `oklch(99% 0.003 215)` |
| `surface2` (raised/pressed) | `oklch(27% 0.022 225)` | `oklch(95% 0.01 215)` |
| `border` | `oklch(32% 0.02 225)` | `oklch(90% 0.012 215)` |
| `text` | `oklch(96% 0.01 215)` | `oklch(22% 0.03 230)` |
| `textDim` | `oklch(72% 0.02 220)` | `oklch(48% 0.03 225)` |
| `primary` | cyan-500 | cyan-500 |
| `accent` | cyan-400 | cyan-700 |
| `onPrimary` | `oklch(20% 0.03 225)` | `oklch(20% 0.03 225)` |
| `ok` | `oklch(72% 0.17 155)` | `oklch(58% 0.15 155)` |
| `warn` | `oklch(80% 0.15 80)` | `oklch(62% 0.15 70)` |
| `err` | `oklch(66% 0.20 25)` | `oklch(55% 0.20 25)` |

Rules:
- **Dark bg is deep slate, not pure black** (this is an IPS LCD, so true black looks flat grey).
- **cyan-500 is too low-contrast for text or thin icons on light backgrounds.** Use `accent` (cyan-700) there.
- **Text on primary fills is near-black** (`onPrimary`), never white.
- Body text contrast at least 4.5:1, icons and large text at least 3:1.
- RGB565 bands in gradients: enable LVGL gradient dithering if the chosen LVGL version offers it, and check gradients on the device.
- ST7789 panels shift colors; compare on hardware and adjust tokens, not code.

### 3.3 Theme switching

- Themes: **dark**, **light**, optional **auto** (by time of day via NTP, set from the companion).
- Switch by swapping the active palette and re-applying styles (`lv_obj_report_style_change` or equivalent). Default behavior is an **instant recolor** (no crossfade). Persist in `Preferences`; read it at boot before drawing anything.
- **Media screen and AOD are always dark** regardless of theme (see sections 9.6 and 10).

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

- Glyph coverage v1: Basic Latin, Latin-1 Supplement, Latin Extended-A, Greek, Cyrillic, plus `° % · – — … ’ “ ”` and digits. Media titles can contain any script: the companion should **replace unsupported characters** with a placeholder instead of sending them, so the device never shows missing-glyph boxes. Wider coverage (CJK, other scripts) can be added later via LVGL TinyTTF with a Noto subset; complex scripts needing shaping/RTL are out of scope for v1.
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
1. **LVGL vertical gradient** between `bg` and a slightly tinted `bg` (zero memory).
2. **Static image from flash**, 320x170 RGB565 (about 109 KB), one per theme. Loaded straight from flash or copied once into PSRAM.
3. **Companion-pushed wallpaper**, saved to LittleFS and selected in companion settings (same format and size).

No alpha-blended overlays on top of wallpapers.

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

### 9.6 Media (Spotify-style Now Playing)
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
Only: WiFi setup/reset, brightness, theme, AOD timeout, reboot, about/version. Everything else is configured from the Rust companion and pushed to the device (section 11.3).

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
- **Power:** AOD is always dark regardless of theme. It is an LCD, so black does not save power and burn-in is not a concern. Savings come from **backlight PWM at about 15 to 25%**, UI timer at about **5 Hz**, and 1 Hz progress updates. Do not use light sleep (it would drop WiFi/USB). Optional later: a companion setting to switch the backlight fully off after N minutes.

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
- The companion replaces characters the device fonts cannot render (section 4).

### 11.2 Album art
- Format: **128x128, RGB565 little-endian, raw, 32 768 bytes**, center-cropped, no alpha. The device only copies bytes into a PSRAM buffer, so **no JPEG decoder is needed**.
- The device fetches art only when the `art` hash differs from the cached one. Default for WiFi: the device **pulls** `GET http://<companion>:7778/art/<hash>` (companion runs a tiny HTTP server). Over serial: the companion pushes an art frame.
- Show a placeholder (Lucide `music` on `surface2`) until art arrives or when there is none.

### 11.3 Commands and config
Device to companion: `{"type":"cmd","cmd":"play_pause|next|prev|seek","pos_ms":0}`.
Companion to device (persisted in `Preferences`, acknowledged with `config_ack`):
```json
{"type":"config","v":3,"theme":"dark|light|auto","brightness":60,
 "aod":true,"aod_timeout_s":60,"reduce_motion":false,"wallpaper":"gradient"}
```
`v` is a version counter; the device applies and stores the highest version it has seen.

## 12. Boot logo

Sequence (target: logo visible as soon as the Arduino core hands over control):
1. First thing in `setup()`: keep the **backlight off**, init the panel, fill with the saved theme's `bg` (read `theme` from `Preferences` first; default dark), draw the logo centered.
2. **Ramp backlight** from 0 to the user level over about 300 ms (PWM fade, no redraws; hides the white power-on flash).
3. Initialize the rest of the system behind it. **Do not block on WiFi or the companion.**
4. Show the logo for at least about **600 ms**, then go to Home. The status bar shows "connecting" until a transport is up.

Asset: the user supplies the HawaOS logo as **SVG or transparent PNG** (ideally 1x and 2x) at `assets/logo.svg`. Preferred: a **single-color mark rendered as an A8 (alpha-only) bitmap**, tinted with `text`/`primary` at draw time so it works in both themes; at about 200x80 that is about 16 KB. A multi-color logo needs per-theme RGB565A8 variants (about 3 bytes per pixel). Provide `tools/gen_logo.*` that converts the asset to a C array at build time. Draw it before LVGL starts if that is faster (direct `pushImage` or equivalent).

## 13. Performance and memory

- RGB565 framebuffer is about 109 KB; the real constraint is internal RAM (WiFi + BLE + LVGL).
- LVGL partial-render buffers: benchmark **internal DMA-capable RAM vs PSRAM** (e.g. two buffers of about 320x20 lines); use internal RAM if it fits and is faster, else PSRAM. Large assets (art buffer, wallpaper, fonts' decoded data) go in PSRAM.
- Dev builds enable LVGL perf/memory monitors and log FPS, free internal heap and free PSRAM.
- Fonts and icons in flash; no runtime font generation.

## 14. Open decisions (ask the user before building each)

1. **Transport:** WiFi only, USB serial as primary (no WiFi credentials, fast, reliable, but debug logs cannot share that port), or both behind the `Transport` abstraction.
2. **Button mapping:** which physical button is Next vs Select.
3. **LVGL version:** 8 vs 9 (APIs differ). Pick one, pin it, keep it consistent.
4. **Unicode coverage** beyond v1 (CJK or other scripts via TinyTTF).
5. **Logo asset** format and colors (single-color A8 vs multi-color).
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
