#!/usr/bin/env python3
"""
HawaOS Boot Logo Generator (tools/gen_logo.py)

Supports DESIGN.md Section 12 & Section 14.5 (Multi-color logo with per-theme
RGB565 color + A8 alpha variants. Display.cpp blends them over the active
accent's `bg` at boot and pushes the result directly with `TFT_eSPI`, before
LVGL starts).

Renders:
- Top lockup: `assets/logo.png` icon mark + "HawaOS" wordmark (or full wide logo)
- Subline: "Breezy Elegance, Materialised"
"""

import math
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

LOGO_W = 220
LOGO_H = 88
SUBLINE_TEXT = "Breezy Elegance, Materialised"

def load_raw_logo_asset(repo_root: Path, variant: str) -> Image.Image | None:
    assets_dir = repo_root / "assets"
    candidates = [
        assets_dir / f"logo_{variant}.png",
        assets_dir / f"logo_{variant}.svg",
        assets_dir / "logo.png",
        assets_dir / "logo.svg",
    ]
    for path in candidates:
        if path.exists():
            if path.suffix.lower() == ".svg":
                tmp_png = assets_dir / f".tmp_logo_{variant}.png"
                subprocess.run(
                    [
                        "magick",
                        "-background",
                        "none",
                        str(path),
                        "-resize",
                        "256x256",
                        str(tmp_png),
                    ],
                    check=True,
                )
                img = Image.open(tmp_png).convert("RGBA")
                tmp_png.unlink(missing_ok=True)
                return img
            return Image.open(path).convert("RGBA")
    return None


def make_procedural_icon(size: int = 52) -> Image.Image:
    icon = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(icon)
    half = (size - 1) / 2.0
    box_half = half - 12.0
    for y in range(size):
        t = y / float(max(1, size - 1))
        r = int(0x00 * (1 - t) + 0x00 * t)
        g = int(0xD1 * (1 - t) + 0x74 * t)
        b = int(0xEC * (1 - t) + 0x91 * t)
        for x in range(size):
            dx = max(abs(x - half) - box_half, 0.0)
            dy = max(abs(y - half) - box_half, 0.0)
            dist = math.hypot(dx, dy)
            if dist <= 12.0:
                alpha = int(max(0.0, min(1.0, 12.5 - dist)) * 255)
                icon.putpixel((x, y), (r, g, b, alpha))
    draw.rounded_rectangle((13, 12, 19, 39), radius=2, fill=(5, 25, 32, 255))
    draw.rounded_rectangle((33, 12, 39, 39), radius=2, fill=(5, 25, 32, 255))
    draw.rounded_rectangle((16, 23, 36, 28), radius=2, fill=(83, 234, 253, 255))
    return icon


def load_or_render_rgba(repo_root: Path, variant: str) -> Image.Image:
    canvas = Image.new("RGBA", (LOGO_W, LOGO_H), (0, 0, 0, 0))
    draw = ImageDraw.Draw(canvas)

    # Theme text colors matching include/theme_tokens.h
    text_main = (233, 243, 246, 255) if variant == "dark" else (0, 20, 30, 255)
    text_cyan = (0, 182, 212, 255) if variant == "dark" else (0, 116, 145, 255)
    text_dim = (130, 159, 169, 255) if variant == "dark" else (45, 77, 88, 255)

    raw_img = load_raw_logo_asset(repo_root, variant)
    icon_size = 52

    title_font_path = repo_root / "assets" / "fonts" / "Inter-Display-SemiBold.ttf"
    sub_font_path = repo_root / "assets" / "fonts" / "Inter-Regular.ttf"

    title_font = (
        ImageFont.truetype(str(title_font_path), 28)
        if title_font_path.exists()
        else ImageFont.load_default()
    )
    sub_font = (
        ImageFont.truetype(str(sub_font_path), 12)
        if sub_font_path.exists()
        else ImageFont.load_default()
    )

    if raw_img is not None and raw_img.width > raw_img.height * 1.35:
        # Wide horizontal wordmark asset: scale to top region
        img = raw_img.copy()
        img.thumbnail((LOGO_W - 12, 56), Image.Resampling.LANCZOS)
        ox = (LOGO_W - img.width) // 2
        oy = (58 - img.height) // 2 + 2
        canvas.paste(img, (ox, oy), img)
    else:
        # Square icon mark (assets/logo.png) + "HawaOS" wordmark lockup
        if raw_img is not None:
            icon = raw_img.resize((icon_size, icon_size), Image.Resampling.LANCZOS)
        else:
            icon = make_procedural_icon(icon_size)

        w_hawa = draw.textlength("Hawa", font=title_font)
        w_os = draw.textlength("OS", font=title_font)
        gap = 12
        total_lockup_w = int(round(icon_size + gap + w_hawa + w_os))
        start_x = (LOGO_W - total_lockup_w) // 2
        icon_y = 4

        canvas.paste(icon, (start_x, icon_y), icon)

        text_x = start_x + icon_size + gap
        text_y = icon_y + 10
        draw.text((text_x, text_y), "Hawa", font=title_font, fill=text_main)
        draw.text(
            (int(round(text_x + w_hawa + 1)), text_y),
            "OS",
            font=title_font,
            fill=text_cyan,
        )

    # Subline: "Breezy Elegance, Materialised" centered below lockup
    w_sub = draw.textlength(SUBLINE_TEXT, font=sub_font)
    sub_x = int(round((LOGO_W - w_sub) / 2.0))
    sub_y = 66
    draw.text((sub_x, sub_y), SUBLINE_TEXT, font=sub_font, fill=text_dim)

    return canvas


def split_rgb565_a8(img: Image.Image):
    """Straight (unpremultiplied) RGB565 colors plus an A8 alpha plane."""
    colors, alphas = [], []
    for y in range(img.height):
        for x in range(img.width):
            r, g, b, a = img.getpixel((x, y))
            colors.append(((r & 0xF8) << 8) | ((g & 0xFC) << 3) | (b >> 3))
            alphas.append(a)
    return colors, alphas


def array_lines(ctype: str, name: str, values, fmt: str, per_line: int):
    lines = [f"const {ctype} {name}[] PROGMEM = {{"]
    for i in range(0, len(values), per_line):
        lines.append("    " + ", ".join(fmt.format(v) for v in values[i : i + per_line]) + ",")
    lines.extend(["};", ""])
    return lines


def generate_logo_header(repo_root: Path):
    out_path = repo_root / "include" / "boot_logo.h"
    dark_img = load_or_render_rgba(repo_root, "dark")
    light_img = load_or_render_rgba(repo_root, "light")

    dark_565, dark_a8 = split_rgb565_a8(dark_img)
    light_565, light_a8 = split_rgb565_a8(light_img)

    lines = [
        "// =============================================================================",
        "// AUTO-GENERATED BY tools/gen_logo.py — DO NOT EDIT MANUALLY",
        "// Multi-color per-theme boot logo: straight RGB565 + A8 alpha (DESIGN.md Section 12)",
        "// =============================================================================",
        "#pragma once",
        "",
        "#include <stdint.h>",
        "#include <pgmspace.h>",
        "",
        "namespace BootLogo {",
        "",
        f"constexpr uint16_t WIDTH  = {LOGO_W};",
        f"constexpr uint16_t HEIGHT = {LOGO_H};",
        "",
    ]
    lines += array_lines("uint16_t", "kLogoDark565", dark_565, "0x{:04X}", 12)
    lines += array_lines("uint8_t", "kLogoDarkA8", dark_a8, "0x{:02X}", 16)
    lines += array_lines("uint16_t", "kLogoLight565", light_565, "0x{:04X}", 12)
    lines += array_lines("uint8_t", "kLogoLightA8", light_a8, "0x{:02X}", 16)
    lines.extend(["} // namespace BootLogo", ""])

    out_path.write_text("\n".join(lines))
    print(
        f"[gen_logo] Generated {out_path.relative_to(repo_root)} ({LOGO_W}x{LOGO_H} dark & light RGB565 + A8)"
    )


if __name__ == "__main__":
    root = Path(__file__).resolve().parent.parent
    generate_logo_header(root)
else:
    try:
        Import("env")  # type: ignore
    except NameError:
        pass
    else:
        generate_logo_header(Path(env.subst("$PROJECT_DIR")))  # type: ignore
