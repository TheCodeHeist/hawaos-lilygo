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


# Cyan brand scale from DESIGN.md Section 3.1
CYAN_SCALE = {
    "CYAN_300": (86.5, 0.127, 207.0),
    "CYAN_400": (78.9, 0.154, 211.5),
    "CYAN_500": (71.5, 0.143, 215.221),
    "CYAN_600": (60.9, 0.126, 221.7),
    "CYAN_700": (52.0, 0.105, 223.0),
    "CYAN_800": (45.0, 0.085, 224.0),
}

# Semantic tokens from DESIGN.md Section 3.2 & Section 6 (vertical gradient bottom tint)
DARK_TOKENS = {
    "bg": (17.0, 0.018, 225.0),
    "bgGradEnd": (
        20.5,
        0.028,
        220.0,
    ),  # Subtle tinted vertical wallpaper gradient bottom
    "surface": (22.0, 0.020, 225.0),
    "surface2": (27.0, 0.022, 225.0),
    "border": (32.0, 0.020, 225.0),
    "text": (96.0, 0.010, 215.0),
    "textDim": (72.0, 0.020, 220.0),
    "primary": CYAN_SCALE["CYAN_500"],
    "accent": CYAN_SCALE["CYAN_400"],
    "pressed": CYAN_SCALE["CYAN_600"],
    "onPrimary": (20.0, 0.030, 225.0),
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
    "primary": CYAN_SCALE["CYAN_600"],
    "accent": CYAN_SCALE["CYAN_700"],
    "pressed": CYAN_SCALE["CYAN_800"],
    "onPrimary": (99.0, 0.003, 215.0),  # Crisp white icon/text on primary buttons
    "ok": (55.0, 0.155, 155.0),
    "warn": (58.0, 0.155, 70.0),
    "err": (52.0, 0.200, 25.0),
}


def generate_header(repo_root: Path):
    out_path = repo_root / "include" / "theme_tokens.h"
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

    lines.extend(
        [
            "",
            "struct Palette {",
        ]
    )
    for key in DARK_TOKENS.keys():
        lines.append(f"    uint32_t {key};")
    for key in DARK_TOKENS.keys():
        lines.append(f"    uint16_t {key}565;")
    lines.extend(
        [
            "};",
            "",
            "constexpr Palette kDarkPalette = {",
        ]
    )

    for key, (l_pct, c, h) in DARK_TOKENS.items():
        r, g, b = oklch_to_srgb8(l_pct, c, h)
        lines.append(
            f"    /* .{key} = */ 0x{rgb888_hex(r, g, b):06X}, // oklch({l_pct}% {c} {h})"
        )
    for key, (l_pct, c, h) in DARK_TOKENS.items():
        r, g, b = oklch_to_srgb8(l_pct, c, h)
        lines.append(f"    /* .{key}565 = */ 0x{rgb565_hex(r, g, b):04X},")
    lines.extend(
        [
            "};",
            "",
            "constexpr Palette kLightPalette = {",
        ]
    )

    for key, (l_pct, c, h) in LIGHT_TOKENS.items():
        r, g, b = oklch_to_srgb8(l_pct, c, h)
        lines.append(
            f"    /* .{key} = */ 0x{rgb888_hex(r, g, b):06X}, // oklch({l_pct}% {c} {h})"
        )
    for key, (l_pct, c, h) in LIGHT_TOKENS.items():
        r, g, b = oklch_to_srgb8(l_pct, c, h)
        lines.append(f"    /* .{key}565 = */ 0x{rgb565_hex(r, g, b):04X},")
    lines.extend(
        [
            "};",
            "",
            "} // namespace ThemeTokens",
            "",
        ]
    )

    content = "\n".join(lines)
    if out_path.exists() and out_path.read_text() == content:
        return
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(content)
    print(f"[gen_theme] Generated {out_path}")


if __name__ == "__main__":
    root = Path(__file__).resolve().parent.parent
    generate_header(root)
else:
    # Invoked by PlatformIO SCons `extra_scripts = pre:tools/gen_theme.py`
    try:
        Import("env")  # type: ignore
        project_dir = Path(env.subst("$PROJECT_DIR"))  # type: ignore
        generate_header(project_dir)
    except Exception:
        pass
