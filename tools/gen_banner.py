#!/usr/bin/env python3
"""
README banner (tools/gen_banner.py)

The boot screen's lockup (tools/gen_logo.py) at print size: the mark and
"OS" in Tailwind cyan-500, "Hawa" and the tagline in text colours for a dark
and a light page. The README picks one with <picture> and
prefers-color-scheme. Transparent background, rendered at about 3x the size
it is shown, so it stays sharp on high-DPI screens.

    tools/gen_banner.py   ->  docs/media/banner-dark.png, banner-light.png
"""

import shutil
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
CYAN_500 = (0x00, 0xB6, 0xD4)  # Tailwind cyan-500 (include/theme_tokens.h)
# (text, tagline) per page theme: the device's cyan dark and light palettes.
THEMES = {
    "dark": ((0xEE, 0xF5, 0xF7), (0x93, 0xAD, 0xB4)),
    "light": ((0x05, 0x19, 0x20), (0x3F, 0x5C, 0x66)),
}
TAGLINE = "Breezy Elegance, Materialised"

# Proportions of the boot lockup, scaled up.
MARK = 312
TITLE_PX = 168
TAGLINE_PX = 66
GAP = 72
PAD = 24


def mark_shape(size: int) -> Image.Image:
    """The mark's coverage at `size` px, from the SVG (or the PNG)."""
    svg, png = ROOT / "assets" / "logo.svg", ROOT / "assets" / "logo.png"
    magick = shutil.which("magick")
    if svg.exists() and magick:
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp) / "mark.png"
            subprocess.run(
                [magick, "-background", "none", "-density", "600", str(svg),
                 "-resize", f"{size}x{size}", str(out)],
                check=True,
            )
            return Image.open(out).convert("RGBA").getchannel("A").copy()
    return Image.open(png).convert("RGBA").resize((size, size), Image.Resampling.LANCZOS).getchannel("A")


def render(text_rgb, dim_rgb) -> Image.Image:
    fonts = ROOT / "assets" / "fonts"
    title = ImageFont.truetype(str(fonts / "Inter-Display-SemiBold.ttf"), TITLE_PX)
    tag = ImageFont.truetype(str(fonts / "Inter-Regular.ttf"), TAGLINE_PX)

    probe = ImageDraw.Draw(Image.new("L", (1, 1)))
    w_hawa = probe.textlength("Hawa", font=title)
    w_os = probe.textlength("OS", font=title)
    w_tag = probe.textlength(TAGLINE, font=tag)
    lockup_w = int(round(MARK + GAP + w_hawa + w_os))
    width = max(lockup_w, int(w_tag)) + 2 * PAD
    tag_y = PAD + MARK + 56
    height = tag_y + TAGLINE_PX + 24 + PAD

    img = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    x0 = (width - lockup_w) // 2

    mark = Image.new("RGBA", (MARK, MARK), CYAN_500 + (255,))
    mark.putalpha(mark_shape(MARK))
    img.alpha_composite(mark, (x0, PAD))

    # "HawaOS" centred on the mark: anchor "lm" is the left edge, middle of
    # the text's ascender-to-descender box.
    text_x = x0 + MARK + GAP
    mid_y = PAD + MARK // 2
    draw.text((text_x, mid_y), "Hawa", font=title, fill=text_rgb + (255,), anchor="lm")
    draw.text((text_x + w_hawa, mid_y), "OS", font=title, fill=CYAN_500 + (255,), anchor="lm")

    draw.text(((width - w_tag) / 2, tag_y), TAGLINE, font=tag, fill=dim_rgb + (255,))
    return img


def main():
    out_dir = ROOT / "docs" / "media"
    for name, (text_rgb, dim_rgb) in THEMES.items():
        path = out_dir / f"banner-{name}.png"
        img = render(text_rgb, dim_rgb)
        img.save(path, optimize=True)
        print(f"{path.relative_to(ROOT)}: {img.width}x{img.height}")


if __name__ == "__main__":
    main()
