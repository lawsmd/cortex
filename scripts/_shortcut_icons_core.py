"""Shared compositing logic for build-shortcut-icons.py (Windows) and
build-shortcut-icons-macos.py (macOS).

The 2048x2048 RGBA "DEV" master is platform-agnostic; only the bold-font
lookup and the final container format (.ico vs .icns) differ between OSes.
Both entry points reuse `build_dev_master()` here, supplying their own
font-lookup callable.
"""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Callable

from PIL import Image, ImageDraw, ImageFont

PINK = (240, 0, 208, 255)
DARK_PURPLE = (32, 0, 64, 255)


def build_dev_master(
    master_png: Path,
    out_png: Path,
    font_lookup: Callable[[int], ImageFont.FreeTypeFont],
) -> None:
    """Composite the master icon + 'DEV' overlay onto a 2048x2048 RGBA canvas."""
    src = Image.open(master_png).convert("RGBA")
    W, H = 2048, 2048
    canvas = Image.new("RGBA", (W, H), (0, 0, 0, 0))

    icon_h = int(H * 0.72)
    icon_w = int(src.width * (icon_h / src.height))
    scaled = src.resize((icon_w, icon_h), Image.LANCZOS)

    icon_x = (W - icon_w) // 2
    icon_y = int(H * 0.02)
    canvas.paste(scaled, (icon_x, icon_y), scaled)

    draw = ImageDraw.Draw(canvas)
    text = "DEV"
    target_w = int(W * 0.62)
    font_size = 1
    font = font_lookup(font_size)
    while True:
        font = font_lookup(font_size + 8)
        bbox = font.getbbox(text)
        text_w = bbox[2] - bbox[0]
        if text_w >= target_w:
            break
        font_size += 8
        if font_size > 1200:
            break

    bbox = font.getbbox(text)
    text_w = bbox[2] - bbox[0]
    text_x = (W - text_w) // 2 - bbox[0]
    text_y = int(H * 0.75) - bbox[1]

    stroke = max(1, int(W * 0.012))

    draw.text(
        (text_x, text_y),
        text,
        font=font,
        fill=PINK,
        stroke_width=stroke,
        stroke_fill=DARK_PURPLE,
    )

    out_png.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(out_png, optimize=True)
    print(f"  wrote {out_png}  ({W}x{H} RGBA)")


def fallback_default_font(size_px: int) -> ImageFont.FreeTypeFont:
    print("[warn] no bold TTF found - falling back to PIL default", file=sys.stderr)
    return ImageFont.load_default()
