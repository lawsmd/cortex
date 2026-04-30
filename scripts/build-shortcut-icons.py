#!/usr/bin/env python3
"""Generate Windows taskbar shortcut icons for Cortex prod + dev lanes.

Outputs:
  %LOCALAPPDATA%/Cortex/Cortex.ico       — copy of the master multi-res .ico
  %LOCALAPPDATA%/Cortex/Cortex-Dev.ico   — same icon with "DEV" overlay below
                                            (pink #F000D0 letters, dark purple
                                            #200040 outline — sampled from the
                                            master icon's dominant clusters)

Why this lives in scripts/: per-machine taskbar shortcut icons are a personal
artifact, but the *recipe* should be reproducible — if the master icon ever
changes, running this script regenerates both shortcuts in one shot. Same
precedent as install-cortex-prod.cmd.

Run from repo root:
    python scripts/build-shortcut-icons.py

Requires Pillow 9.x+ (for ImageDraw.text stroke_width/stroke_fill).
"""

from __future__ import annotations

import os
import shutil
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

REPO_ROOT = Path(__file__).resolve().parent.parent
MASTER_PNG = REPO_ROOT / "app" / "assets" / "cortex" / "CortexIcon.png"
MASTER_ICO = REPO_ROOT / "app" / "channels" / "oss" / "icon" / "no-padding" / "icon.ico"

# Brand colors — sampled from CortexIcon.png pixel-frequency clusters.
PINK = (240, 0, 208, 255)        # #F000D0 — primary brain color
DARK_PURPLE = (32, 0, 64, 255)   # #200040 — outline/shadow

# Multi-resolution .ico standard sizes. At 16/24/32, the "DEV" text is
# illegible — that's expected; small-size icons just read as Cortex. The
# taskbar typically renders 24-48px, where the overlay is visible.
ICO_SIZES = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]


def find_bold_font(size_px: int) -> ImageFont.FreeTypeFont:
    """Best-effort bold sans-serif font lookup for Windows."""
    candidates = [
        r"C:\Windows\Fonts\arialbd.ttf",       # Arial Bold
        r"C:\Windows\Fonts\seguibl.ttf",       # Segoe UI Black
        r"C:\Windows\Fonts\segoeuib.ttf",      # Segoe UI Bold
        r"C:\Windows\Fonts\Impact.ttf",        # Impact (extra-condensed bold)
        r"C:\Windows\Fonts\calibrib.ttf",      # Calibri Bold
    ]
    for p in candidates:
        if os.path.isfile(p):
            return ImageFont.truetype(p, size_px)
    print("[warn] no bold TTF found — falling back to PIL default", file=sys.stderr)
    return ImageFont.load_default()


def build_dev_master(master_png: Path, out_png: Path) -> None:
    """Composite the master icon scaled to 70% of canvas + 'DEV' band below.

    Canvas is 2048×2048 RGBA. Top-aligned brain occupies the upper ~75%
    (slightly cropped if needed); bottom ~25% holds the DEV text.
    """
    src = Image.open(master_png).convert("RGBA")
    W, H = 2048, 2048
    canvas = Image.new("RGBA", (W, H), (0, 0, 0, 0))

    # Scale icon to ~75% of canvas height, keep aspect.
    icon_h = int(H * 0.72)
    icon_w = int(src.width * (icon_h / src.height))
    scaled = src.resize((icon_w, icon_h), Image.LANCZOS)

    # Center the icon horizontally; nudge up so it sits in the top portion.
    icon_x = (W - icon_w) // 2
    icon_y = int(H * 0.02)  # small top margin
    canvas.paste(scaled, (icon_x, icon_y), scaled)

    # DEV text band centered in the bottom ~25%.
    draw = ImageDraw.Draw(canvas)
    text = "DEV"
    # Iteratively size the font so the text fills ~85% of canvas width.
    target_w = int(W * 0.62)
    font_size = 1
    font = find_bold_font(font_size)
    while True:
        font = find_bold_font(font_size + 8)
        bbox = font.getbbox(text)
        text_w = bbox[2] - bbox[0]
        if text_w >= target_w:
            break
        font_size += 8
        if font_size > 1200:
            break

    bbox = font.getbbox(text)
    text_w = bbox[2] - bbox[0]
    text_h = bbox[3] - bbox[1]
    text_x = (W - text_w) // 2 - bbox[0]
    text_y = int(H * 0.75) - bbox[1]

    # Outline thickness scaled to canvas. ~3% of canvas width reads cleanly
    # at 48-128px and survives a soft-edge antialias down to 24px.
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


def png_to_multires_ico(src_png: Path, out_ico: Path) -> None:
    img = Image.open(src_png).convert("RGBA")
    out_ico.parent.mkdir(parents=True, exist_ok=True)
    img.save(out_ico, format="ICO", sizes=ICO_SIZES)
    print(f"  wrote {out_ico}  (sizes: {', '.join(f'{w}x{h}' for w, h in ICO_SIZES)})")


def main() -> int:
    install_dir = Path(os.environ.get("LOCALAPPDATA", "")) / "Cortex"
    if not install_dir.parent.exists():
        print(f"error: %LOCALAPPDATA% not resolved ({install_dir.parent}); is this Windows?", file=sys.stderr)
        return 1
    install_dir.mkdir(parents=True, exist_ok=True)

    print(f"Output: {install_dir}")
    print()
    print("=== Prod icon (copy of master multi-res .ico) ===")
    prod_ico = install_dir / "Cortex.ico"
    shutil.copyfile(MASTER_ICO, prod_ico)
    print(f"  wrote {prod_ico}")

    print()
    print("=== Dev icon (with 'DEV' text overlay) ===")
    dev_png = install_dir / "Cortex-Dev.png"
    build_dev_master(MASTER_PNG, dev_png)
    dev_ico = install_dir / "Cortex-Dev.ico"
    png_to_multires_ico(dev_png, dev_ico)

    print()
    print("Done. Use these in shortcut .lnk files via the IconLocation field.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
