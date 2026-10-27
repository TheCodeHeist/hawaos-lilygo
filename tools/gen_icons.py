#!/usr/bin/env python3
"""
HawaOS Font & Lucide Icon Generator (tools/gen_icons.py)

Generates:
1. `src/ui/icons.h`: UTF-8 C string macros for the 35 Lucide icons specified in
   DESIGN.md Section 5, plus `LV_FONT_DECLARE(...)` for all custom Inter &
   Lucide LVGL fonts.
2. `src/ui/fonts/font_lucide_{20,24,32,48}.c`: 4 bpp LVGL icon fonts built from
   `assets/fonts/lucide.ttf` via `lv_font_conv`.
3. `src/ui/fonts/font_inter_{12,14}_reg.c` and `font_inter_{14,18,28,48}_sb.c`:
   4 bpp LVGL text fonts built from `assets/fonts/Inter-*.ttf` with the exact
   glyph coverage specified in DESIGN.md Section 4:
   - Basic Latin (`0x20-0x7E`)
   - Latin-1 Supplement (`0xA0-0xFF`)
   - Latin Extended-A (`0x0100-0x017F`)
   - Greek (`0x0370-0x03FF`)
   - Cyrillic (`0x0400-0x04FF`)
   - Punctuation & symbols: `° % · – — … ’ “ ” •` (`0x2013,0x2014,0x2019,0x201C,0x201D,0x2022,0x2026`)
"""

import json
import os
import shutil
import subprocess
from pathlib import Path

ICON_NAMES = [
    "house",
    "layout-grid",
    "clock",
    "activity",
    "cpu",
    "memory-stick",
    "network",
    "bell",
    "bell-off",
    "music",
    "play",
    "pause",
    "skip-back",
    "skip-forward",
    "volume-2",
    "volume-x",
    "settings",
    "sliders-horizontal",
    "sun",
    "moon",
    "sun-moon",
    "wifi",
    "wifi-off",
    "bluetooth",
    "bluetooth-off",
    "usb",
    "plug",
    "battery-medium",
    "refresh-cw",
    "power",
    "chevron-left",
    "chevron-right",
    "x",
    "check",
    "circle-alert",
    "cloud",
    "cloud-sun",
    "cloud-moon",
    "cloud-rain",
    "cloud-snow",
    "cloud-lightning",
    "cloud-fog",
    "wind",
    "droplets",
    "thermometer",
]

ICON_SIZES = [20, 24, 32, 48]

# Inter typography roles from DESIGN.md Section 4
# Full v1 Unicode range for caption/body/title; ASCII + degree/percent/symbols for display/hero numbers & clock
FULL_TEXT_RANGE = (
    "0x20-0x7E,0xA0-0xFF,0x0100-0x017F,0x0370-0x03FF,0x0400-0x04FF,"
    "0x2013,0x2014,0x2018,0x2019,0x201C,0x201D,0x2022,0x2026"
)
DISPLAY_TEXT_RANGE = "0x20-0x7E,0xA0-0xFF,0x2013,0x2014,0x2026"

INTER_FONTS = [
    ("font_inter_12_reg", "Inter-Regular.ttf", 12, FULL_TEXT_RANGE),
    ("font_inter_14_reg", "Inter-Regular.ttf", 14, FULL_TEXT_RANGE),
    ("font_inter_14_sb", "Inter-SemiBold.ttf", 14, FULL_TEXT_RANGE),
    ("font_inter_18_sb", "Inter-SemiBold.ttf", 18, FULL_TEXT_RANGE),
    ("font_inter_28_sb", "Inter-Display-SemiBold.ttf", 28, DISPLAY_TEXT_RANGE),
    ("font_inter_48_sb", "Inter-Display-SemiBold.ttf", 48, DISPLAY_TEXT_RANGE),
]


def find_lv_font_conv() -> str:
    exe = shutil.which("lv_font_conv")
    if exe:
        return exe
    nvm_candidate = Path.home() / ".nvm/versions/node/v24.3.0/bin/lv_font_conv"
    if nvm_candidate.exists():
        return str(nvm_candidate)
    raise RuntimeError("lv_font_conv not found in PATH")


def codepoint_to_c_utf8_literal(cp: int) -> str:
    b = chr(cp).encode("utf-8")
    return '"' + "".join(f"\\x{byte:02x}" for byte in b) + '"'


def run_lv_font_conv(
    lv_font_conv: str, font_path: Path, size: int, ranges: str, out_file: Path
):
    cmd = [
        lv_font_conv,
        "--font",
        str(font_path),
        "--size",
        str(size),
        "--bpp",
        "4",
        "--format",
        "lvgl",
        "--no-compress",
        "--lv-include",
        "lvgl.h",
        "-r",
        ranges,
        "-o",
        str(out_file),
    ]
    subprocess.run(cmd, check=True)
    print(
        f"[gen_icons] Built {out_file.relative_to(out_file.parents[2])} ({out_file.stat().st_size // 1024} KB)"
    )


def link_glyph_fallback(out_file: Path, size: int):
    """Points a text font at the glyph cache font of its size, which draws
    what the companion sends for other scripts (DESIGN.md Section 11.9)."""
    fallback = f"font_glyphs_{size}"
    text = out_file.read_text()
    text = text.replace(
        "/*Initialize a public general font descriptor*/",
        f"/*Glyphs the companion sends (src/services/GlyphCache.cpp)*/\n"
        f"extern lv_font_t {fallback};\n\n"
        "/*Initialize a public general font descriptor*/",
        1,
    )
    text = text.replace("    .fallback = NULL,", f"    .fallback = &{fallback},", 1)
    out_file.write_text(text)


def main():
    repo_root = Path(__file__).resolve().parent.parent
    fonts_dir = repo_root / "assets" / "fonts"
    out_fonts_dir = repo_root / "src" / "ui" / "fonts"
    out_fonts_dir.mkdir(parents=True, exist_ok=True)

    info_path = fonts_dir / "lucide_info.json"
    info = json.loads(info_path.read_text())

    codepoints = []
    header_macros = []

    for name in ICON_NAMES:
        if name not in info:
            raise KeyError(f"Icon '{name}' not found in {info_path}")
        hex_code = info[name]["encodedCode"].lstrip("\\")
        cp = int(hex_code, 16)
        codepoints.append(cp)
        macro_name = "ICON_" + name.upper().replace("-", "_")
        utf8_lit = codepoint_to_c_utf8_literal(cp)
        header_macros.append((macro_name, name, cp, utf8_lit))

    # Write src/ui/icons.h
    icons_h_path = repo_root / "src" / "ui" / "icons.h"
    lines = [
        "// =============================================================================",
        "// AUTO-GENERATED BY tools/gen_icons.py — DO NOT EDIT MANUALLY",
        "// Source: DESIGN.md Sections 4 & 5 (Inter Typography + Lucide Icons)",
        "// =============================================================================",
        "#pragma once",
        "",
        "#include <lvgl.h>",
        "",
        "// --- Custom Inter Fonts (4 bpp, stored in Flash) ---",
        "LV_FONT_DECLARE(font_inter_12_reg);",
        "LV_FONT_DECLARE(font_inter_14_reg);",
        "LV_FONT_DECLARE(font_inter_14_sb);",
        "LV_FONT_DECLARE(font_inter_18_sb);",
        "LV_FONT_DECLARE(font_inter_28_sb);",
        "LV_FONT_DECLARE(font_inter_48_sb);",
        "",
        "// --- Custom Lucide Icon Fonts (4 bpp, stored in Flash) ---",
        "LV_FONT_DECLARE(font_lucide_20);",
        "LV_FONT_DECLARE(font_lucide_24);",
        "LV_FONT_DECLARE(font_lucide_32);",
        "LV_FONT_DECLARE(font_lucide_48);",
        "",
        "// --- Lucide UTF-8 Glyph Literals ---",
    ]
    for macro_name, orig_name, cp, utf8_lit in header_macros:
        lines.append(f"#define {macro_name:<26} {utf8_lit} // {orig_name} (U+{cp:04X})")
    lines.append("")

    icons_h_path.write_text("\n".join(lines))
    print(f"[gen_icons] Generated {icons_h_path.relative_to(repo_root)}")

    lv_font_conv = find_lv_font_conv()

    # Build Lucide icon fonts
    lucide_ttf = fonts_dir / "lucide.ttf"
    icon_range_str = ",".join(f"0x{cp:04X}" for cp in sorted(codepoints))
    for sz in ICON_SIZES:
        out_c = out_fonts_dir / f"font_lucide_{sz}.c"
        run_lv_font_conv(lv_font_conv, lucide_ttf, sz, icon_range_str, out_c)

    # Build Inter text fonts
    for c_name, ttf_file, sz, rng in INTER_FONTS:
        ttf_path = fonts_dir / ttf_file
        out_c = out_fonts_dir / f"{c_name}.c"
        run_lv_font_conv(lv_font_conv, ttf_path, sz, rng, out_c)
        if rng == FULL_TEXT_RANGE:
            link_glyph_fallback(out_c, sz)


if __name__ == "__main__":
    main()
