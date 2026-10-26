#!/usr/bin/env python3
"""
HawaOS Theme Token Generator (tools/gen_theme.py)

Converts the OKLCH design tokens from DESIGN.md (Section 3.1 & 3.2) into sRGB
with gamut clipping and emits `include/theme_tokens.h` containing both RGB888
(0xRRGGBB) and RGB565 (0xFFFF) constants for Dark and Light themes.

Can be invoked standalone (`python3 tools/gen_theme.py`) or automatically as a
PlatformIO pre-build script (`extra_scripts = pre:tools/gen_theme.py`).
"""

import math
import os
from pathlib import Path


def oklch_to_oklab(L: float, C: float, h_deg: float):
    """Convert OKLCH (L in 0..1, C >= 0, h in degrees) to Oklab (L, a, b)."""
    h_rad = math.radians(h_deg)
    a = C * math.cos(h_rad)
    b = C * math.sin(h_rad)
    return L, a, b


def oklab_to_linear_srgb(L: float, a: float, b: float):
    """Convert Oklab (L, a, b) to linear sRGB (r, g, b)."""
    l_ = L + 0.3963377774 * a + 0.2158037573 * b
    m_ = L - 0.1055613458 * a - 0.0638541728 * b
    s_ = L - 0.0894841775 * a - 1.2914855480 * b

    l = l_ * l_ * l_
    m = m_ * m_ * m_
    s = s_ * s_ * s_

    r = +4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s
    g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s
    bl = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s
    return r, g, bl


def linear_to_srgb_channel(x: float) -> float:
    """Transfer function from linear light to gamma-encoded sRGB [0, 1]."""
    if x <= 0.0031308:
        return 12.92 * x
    return 1.055 * (x ** (1.0 / 2.4)) - 0.055


def in_srgb_gamut(r: float, g: float, b: float, eps: float = 1e-6) -> bool:
    return (
        (-eps <= r <= 1.0 + eps)
        and (-eps <= g <= 1.0 + eps)
        and (-eps <= b <= 1.0 + eps)
    )


def oklch_to_srgb8(L_pct: float, C: float, h_deg: float):
    """
    Convert OKLCH (L in percent 0..100, C, h in degrees) to 8-bit sRGB (R, G, B)
    using chroma reduction gamut clipping when outside sRGB.
    """
    L = max(0.0, min(1.0, L_pct / 100.0))
    if L <= 0.0:
        return 0, 0, 0
    if L >= 1.0:
        return 255, 255, 255

    # Binary search chroma if out of sRGB gamut
    low_c = 0.0
    high_c = C
    _, a, b = oklch_to_oklab(L, C, h_deg)
    rl, gl, bl = oklab_to_linear_srgb(L, a, b)

    if not in_srgb_gamut(rl, gl, bl):
        for _ in range(20):
            mid_c = 0.5 * (low_c + high_c)
            _, a_m, b_m = oklch_to_oklab(L, mid_c, h_deg)
            rm, gm, bm = oklab_to_linear_srgb(L, a_m, b_m)
            if in_srgb_gamut(rm, gm, bm):
                low_c = mid_c
                rl, gl, bl = rm, gm, bm
            else:
                high_c = mid_c

    rl = max(0.0, min(1.0, rl))
    gl = max(0.0, min(1.0, gl))
    bl = max(0.0, min(1.0, bl))

    R = int(round(linear_to_srgb_channel(rl) * 255.0))
    G = int(round(linear_to_srgb_channel(gl) * 255.0))
    B = int(round(linear_to_srgb_channel(bl) * 255.0))
    return max(0, min(255, R)), max(0, min(255, G)), max(0, min(255, B))


def rgb888_hex(r: int, g: int, b: int) -> int:
    return (r << 16) | (g << 8) | b


def rgb565_hex(r: int, g: int, b: int) -> int:
    return ((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3)


# Tailwind v4 vibrant scales (steps 300..800), in Tailwind's own order. The
# neutrals (slate/gray/zinc/neutral/stone) are left out on purpose: an accent
# has to read as a color against the tinted neutrals below. `cyan` is the
# HawaOS brand color and the default.
ACCENT_STEPS = (300, 400, 500, 600, 700, 800)
ACCENTS = {
    "red": ((80.8, 0.114, 19.571), (70.4, 0.191, 22.216), (63.7, 0.237, 25.331), (57.7, 0.245, 27.325), (50.5, 0.213, 27.518), (44.4, 0.177, 26.899)),
    "orange": ((83.7, 0.128, 66.29), (75.0, 0.183, 55.934), (70.5, 0.213, 47.604), (64.6, 0.222, 41.116), (55.3, 0.195, 38.402), (47.0, 0.157, 37.304)),
    "amber": ((87.9, 0.169, 91.605), (82.8, 0.189, 84.429), (76.9, 0.188, 70.08), (66.6, 0.179, 58.318), (55.5, 0.163, 48.998), (47.3, 0.137, 46.201)),
    "yellow": ((90.5, 0.182, 98.111), (85.2, 0.199, 91.936), (79.5, 0.184, 86.047), (68.1, 0.162, 75.834), (55.4, 0.135, 66.442), (47.6, 0.114, 61.907)),
    "lime": ((89.7, 0.196, 126.665), (84.1, 0.238, 128.85), (76.8, 0.233, 130.85), (64.8, 0.2, 131.684), (53.2, 0.157, 131.589), (45.3, 0.124, 130.933)),
    "green": ((87.1, 0.15, 154.449), (79.2, 0.209, 151.711), (72.3, 0.219, 149.579), (62.7, 0.194, 149.214), (52.7, 0.154, 150.069), (44.8, 0.119, 151.328)),
    "emerald": ((84.5, 0.143, 164.978), (76.5, 0.177, 163.223), (69.6, 0.17, 162.48), (59.6, 0.145, 163.225), (50.8, 0.118, 165.612), (43.2, 0.095, 166.913)),
    "teal": ((85.5, 0.138, 181.071), (77.7, 0.152, 181.912), (70.4, 0.14, 182.503), (60.0, 0.118, 184.704), (51.1, 0.096, 186.391), (43.7, 0.078, 188.216)),
    "cyan": ((86.5, 0.127, 207.078), (78.9, 0.154, 211.53), (71.5, 0.143, 215.221), (60.9, 0.126, 221.723), (52.0, 0.105, 223.128), (45.0, 0.085, 224.283)),
    "sky": ((82.8, 0.111, 230.318), (74.6, 0.16, 232.661), (68.5, 0.169, 237.323), (58.8, 0.158, 241.966), (50.0, 0.134, 242.749), (44.3, 0.11, 240.79)),
    "blue": ((80.9, 0.105, 251.813), (70.7, 0.165, 254.624), (62.3, 0.214, 259.815), (54.6, 0.245, 262.881), (48.8, 0.243, 264.376), (42.4, 0.199, 265.638)),
    "indigo": ((78.5, 0.115, 274.713), (67.3, 0.182, 276.935), (58.5, 0.233, 277.117), (51.1, 0.262, 276.966), (45.7, 0.24, 277.023), (39.8, 0.195, 277.366)),
    "violet": ((81.1, 0.111, 293.571), (70.2, 0.183, 293.541), (60.6, 0.25, 292.717), (54.1, 0.281, 293.009), (49.1, 0.27, 292.581), (43.2, 0.232, 292.759)),
    "purple": ((82.7, 0.119, 306.383), (71.4, 0.203, 305.504), (62.7, 0.265, 303.9), (55.8, 0.288, 302.321), (49.6, 0.265, 301.924), (43.8, 0.218, 303.724)),
    "fuchsia": ((83.3, 0.145, 321.434), (74.0, 0.238, 322.16), (66.7, 0.295, 322.15), (59.1, 0.293, 322.896), (51.8, 0.253, 323.949), (45.2, 0.211, 324.591)),
    "pink": ((82.3, 0.12, 346.018), (71.8, 0.202, 349.761), (65.6, 0.241, 354.308), (59.2, 0.249, 0.584), (52.5, 0.223, 3.958), (45.9, 0.187, 3.815)),
    "rose": ((81.0, 0.117, 11.638), (71.2, 0.194, 13.428), (64.5, 0.246, 16.439), (58.6, 0.253, 17.585), (51.4, 0.222, 16.935), (45.5, 0.188, 13.697)),
}
DEFAULT_ACCENT = "cyan"


def accent_step(name: str, step: int):
    return ACCENTS[name][ACCENT_STEPS.index(step)]


# Cyan brand scale from DESIGN.md Section 3.1 (kept as named constants)
CYAN_SCALE = {f"CYAN_{s}": accent_step("cyan", s) for s in ACCENT_STEPS}

# Accent roles per theme (DESIGN.md 3.1): dark = 500/400/600, light = 600/700/800.
ACCENT_ROLE_STEPS = {
    "dark": {"primary": 500, "accent": 400, "pressed": 600},
    "light": {"primary": 600, "accent": 700, "pressed": 800},
}


def rel_luminance(rgb) -> float:
    def lin(c):
        c = c / 255.0
        return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4

    r, g, b = (lin(c) for c in rgb)
    return 0.2126 * r + 0.7152 * g + 0.0722 * b


def contrast(a, b) -> float:
    la, lb = sorted((rel_luminance(a), rel_luminance(b)), reverse=True)
    return (la + 0.05) / (lb + 0.05)


def on_primary_token(name: str, mode: str, fill_token=None, max_contrast=False):
    """Ink for text/icons on `primary` fills: a hue-tinted near-black or white.

    Dark mode takes whichever contrasts more. Light mode keeps white unless it
    drops below 3:1 (icons and large text), as on yellow/amber/lime fills.
    `fill_token` overrides the fill (the companion's softened primary);
    `max_contrast` always takes the stronger ink (small button labels).
    """
    hue = accent_step(name, 500)[2]
    ink = (20.0, 0.030, round((hue + 10.0) % 360.0, 3))
    white = (99.0, 0.003, round(hue, 3))
    if fill_token is None:
        fill_token = accent_step(name, ACCENT_ROLE_STEPS[mode]["primary"])
    fill = oklch_to_srgb8(*fill_token)
    c_ink = contrast(fill, oklch_to_srgb8(*ink))
    c_white = contrast(fill, oklch_to_srgb8(*white))
    if mode == "dark" or max_contrast:
        return ink if c_ink >= c_white else white
    return white if c_white >= 3.0 else ink


def accent_roles(name: str, mode: str):
    roles = {k: accent_step(name, s) for k, s in ACCENT_ROLE_STEPS[mode].items()}
    roles["onPrimary"] = on_primary_token(name, mode)
    return roles


# Semantic tokens from DESIGN.md Section 3.2 & Section 6 (vertical gradient bottom tint).
# Accent roles (primary/accent/pressed/onPrimary) come from the chosen accent.
# Dark neutrals are (L%, C) pairs that take the accent's hue (`AccentHue`): the
# wallpaper falls from a near-black of the accent into a deep, concentrated
# tint of it (rose -> wine, amber -> espresso), and cards, tracks and dim text
# carry the same undertone so nothing reads as off-hue slate.
class AccentHue(tuple):
    """(L%, C) resolved at the accent's 500 hue."""


DARK_TOKENS = {
    "bg": AccentHue((13.5, 0.030)),
    "bgGradEnd": AccentHue((21.0, 0.072)),  # Concentrated accent-tinted bottom
    "surface": AccentHue((23.0, 0.034)),
    "surface2": AccentHue((28.5, 0.040)),
    "border": AccentHue((34.0, 0.036)),
    "text": AccentHue((96.5, 0.008)),
    "textDim": AccentHue((73.0, 0.030)),
    "primary": None,
    "accent": None,
    "pressed": None,
    "onPrimary": None,
    "ok": (72.0, 0.170, 155.0),
    "warn": (80.0, 0.150, 80.0),
    "err": (66.0, 0.200, 25.0),
}

# Tuned for 16-bit RGB565 ST7789V LCD gamma so pure white cards (`surface`),
# cool ice-cyan backgrounds (`bg` / `bgGradEnd`), progress tracks (`surface2`),
# and borders (`border`) have crisp separation without washing out.
LIGHT_TOKENS = {
    "bg": (90.5, 0.032, 212.0),
    "bgGradEnd": (
        84.5,
        0.048,
        212.0,
    ),  # Crisp sky-cyan vertical wallpaper gradient bottom
    "surface": (99.5, 0.002, 215.0),  # Pure white cards
    "surface2": (83.0, 0.038, 214.0),  # Distinct cool cyan-slate track/chip fill
    "border": (68.0, 0.055, 216.0),  # Crisp card border on 16-bit RGB565
    "text": (18.0, 0.035, 228.0),  # Deep ink
    "textDim": (40.0, 0.042, 222.0),  # High-contrast secondary ink
    "primary": None,
    "accent": None,
    "pressed": None,
    "onPrimary": None,  # White on primary buttons where it reads (>= 3:1)
    "ok": (55.0, 0.155, 155.0),
    "warn": (58.0, 0.155, 70.0),
    "err": (52.0, 0.200, 25.0),
}


def resolved_tokens(name: str, mode: str):
    base = DARK_TOKENS if mode == "dark" else LIGHT_TOKENS
    roles = accent_roles(name, mode)
    hue = accent_step(name, 500)[2]

    def resolve(k, v):
        if v is None:
            return roles[k]
        if isinstance(v, AccentHue):
            return (v[0], v[1], hue)
        return v

    return {k: resolve(k, v) for k, v in base.items()}


def palette_lines(tokens, indent: str):
    lines = []
    for key, (l_pct, c, h) in tokens.items():
        r, g, b = oklch_to_srgb8(l_pct, c, h)
        lines.append(
            f"{indent}/* .{key} = */ 0x{rgb888_hex(r, g, b):06X}, // oklch({l_pct}% {c} {h})"
        )
    for key, (l_pct, c, h) in tokens.items():
        r, g, b = oklch_to_srgb8(l_pct, c, h)
        lines.append(f"{indent}/* .{key}565 = */ 0x{rgb565_hex(r, g, b):04X},")
    return lines


def write_if_changed(out_path: Path, content: str):
    if out_path.exists() and out_path.read_text() == content:
        return
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(content)
    print(f"[gen_theme] Generated {out_path}")


def generate_header(repo_root: Path):
    out_path = repo_root / "include" / "theme_tokens.h"
    names = list(ACCENTS.keys())
    lines = [
        "// =============================================================================",
        "// AUTO-GENERATED BY tools/gen_theme.py — DO NOT EDIT MANUALLY",
        "// Source: DESIGN.md Section 3 (OKLCH -> sRGB with gamut clipping)",
        "// =============================================================================",
        "#pragma once",
        "",
        "#include <stdint.h>",
        "",
        "namespace ThemeTokens {",
        "",
        "// --- Brand Cyan Scale (RGB888 & RGB565) ---",
    ]

    for name, (l_pct, c, h) in CYAN_SCALE.items():
        r, g, b = oklch_to_srgb8(l_pct, c, h)
        c888 = rgb888_hex(r, g, b)
        c565 = rgb565_hex(r, g, b)
        lines.append(
            f"constexpr uint32_t {name}_RGB888 = 0x{c888:06X}; // oklch({l_pct}% {c} {h})"
        )
        lines.append(f"constexpr uint16_t {name}_RGB565 = 0x{c565:04X};")

    lines.extend(["", "struct Palette {"])
    for key in DARK_TOKENS.keys():
        lines.append(f"    uint32_t {key};")
    for key in DARK_TOKENS.keys():
        lines.append(f"    uint16_t {key}565;")
    lines.extend(
        [
            "};",
            "",
            "// --- Accent themes (Tailwind v4 vibrant scales) ---",
            "// Ids are the wire/NVS names; only the accent roles differ per entry.",
            f"constexpr uint8_t kAccentCount = {len(names)};",
            f"constexpr uint8_t kDefaultAccent = {names.index(DEFAULT_ACCENT)}; // {DEFAULT_ACCENT}",
            "constexpr const char *kAccentIds[kAccentCount] = {",
            "    " + ", ".join(f'"{n}"' for n in names) + ",",
            "};",
            "constexpr const char *kAccentLabels[kAccentCount] = {",
            "    " + ", ".join(f'"{n.capitalize()}"' for n in names) + ",",
            "};",
            "",
            "// kPalettes[accent][0] = dark, kPalettes[accent][1] = light",
            "constexpr Palette kPalettes[kAccentCount][2] = {",
        ]
    )
    for name in names:
        lines.append(f"    {{ // {name}")
        for mode in ("dark", "light"):
            lines.append(f"        {{ // {mode}")
            lines.extend(palette_lines(resolved_tokens(name, mode), " " * 12))
            lines.append("        },")
        lines.append("    },")
    lines.extend(
        [
            "};",
            "",
            "// Default (brand cyan) palettes",
            "static constexpr const Palette &kDarkPalette = kPalettes[kDefaultAccent][0];",
            "static constexpr const Palette &kLightPalette = kPalettes[kDefaultAccent][1];",
            "",
            "} // namespace ThemeTokens",
            "",
        ]
    )
    write_if_changed(out_path, "\n".join(lines))


# Companion-only tuning. Large glass fills at full Tailwind chroma glare on a
# desktop window, so fills (primary/pressed) keep their lightness at reduced
# chroma; `accent` (small text, icons, rings) and the swatches stay full strength.
COMPANION_FILL_CHROMA = 0.7

# Window backdrop gradient (top -> bottom), tinted with the accent hue. The
# alphas live in companion/src/ui/theme.rs so the compositor blur shows through.
COMPANION_BACKDROP = {
    "dark": ((26.0, 0.050), (13.0, 0.015)),
    "light": ((98.0, 0.012), (92.0, 0.030)),
}


def soften(token):
    l_pct, c, h = token
    return (l_pct, round(c * COMPANION_FILL_CHROMA, 4), h)


def companion_roles(name: str, mode: str):
    roles = accent_roles(name, mode)
    roles["primary"] = soften(roles["primary"])
    roles["pressed"] = soften(roles["pressed"])
    roles["onPrimary"] = on_primary_token(name, mode, roles["primary"], max_contrast=True)
    hue = accent_step(name, 500)[2]
    (top_l, top_c), (bot_l, bot_c) = COMPANION_BACKDROP[mode]
    roles["bgTop"] = (top_l, top_c, hue)
    roles["bgBottom"] = (bot_l, bot_c, hue)
    return roles


def generate_companion_accents(repo_root: Path):
    """Emit the same accent table for the Rust companion (sRGB hex)."""
    out_path = repo_root / "companion" / "src" / "ui" / "accents.rs"
    if not out_path.parent.exists():
        return

    def hex_of(token):
        return f"0x{rgb888_hex(*oklch_to_srgb8(*token)):06x}"

    lines = [
        "// AUTO-GENERATED BY tools/gen_theme.py — DO NOT EDIT MANUALLY.",
        "// Same Tailwind accent scales and role mapping as include/theme_tokens.h.",
        "",
        "/// Accent colors for one theme mode (sRGB `0xRRGGBB`). Fills are softened",
        "/// (lower chroma) versus the device; `accent` is full strength.",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        "pub struct AccentRoles {",
        "    pub primary: u32,",
        "    pub accent: u32,",
        "    pub pressed: u32,",
        "    pub on_primary: u32,",
        "    /// Window backdrop gradient, opaque; theme.rs adds the glass alpha.",
        "    pub bg_top: u32,",
        "    pub bg_bottom: u32,",
        "}",
        "",
        "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        "pub struct Accent {",
        "    /// Wire / config id, e.g. `\"cyan\"`.",
        "    pub id: &'static str,",
        "    pub label: &'static str,",
        "    pub dark: AccentRoles,",
        "    pub light: AccentRoles,",
        "    /// Swatch gradient: the 500 shade into the deeper 700 shade.",
        "    pub swatch: (u32, u32),",
        "}",
        "",
        f'pub const DEFAULT_ACCENT: &str = "{DEFAULT_ACCENT}";',
        "",
        f"pub const ACCENTS: [Accent; {len(ACCENTS)}] = [",
    ]
    for name in ACCENTS:
        lines.append("    Accent {")
        lines.append(f'        id: "{name}",')
        lines.append(f'        label: "{name.capitalize()}",')
        for mode in ("dark", "light"):
            r = companion_roles(name, mode)
            lines.append(
                f"        {mode}: AccentRoles {{ primary: {hex_of(r['primary'])}, accent: {hex_of(r['accent'])}, "
                f"pressed: {hex_of(r['pressed'])}, on_primary: {hex_of(r['onPrimary'])}, "
                f"bg_top: {hex_of(r['bgTop'])}, bg_bottom: {hex_of(r['bgBottom'])} }},"
            )
        lines.append(
            f"        swatch: ({hex_of(accent_step(name, 500))}, {hex_of(accent_step(name, 700))}),"
        )
        lines.append("    },")
    lines.extend(["];", ""])
    write_if_changed(out_path, "\n".join(lines))


def generate_all(repo_root: Path):
    generate_header(repo_root)
    generate_companion_accents(repo_root)


if __name__ == "__main__":
    root = Path(__file__).resolve().parent.parent
    generate_all(root)
else:
    # Invoked by PlatformIO SCons `extra_scripts = pre:tools/gen_theme.py`
    try:
        Import("env")  # type: ignore
        project_dir = Path(env.subst("$PROJECT_DIR"))  # type: ignore
        generate_all(project_dir)
    except Exception:
        pass
