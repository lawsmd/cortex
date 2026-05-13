#!/usr/bin/env python3
"""Generate macOS Raycast/Spotlight shortcut icons for Cortex prod + dev lanes.

Mac variants composite the brain glyph onto a **black squircle plate** that
fills the icon canvas (modern macOS app-icon convention; Apple's HIG since
Big Sur). Without the plate, the line-art brain reads as "missing background"
against Mac's light Dock/Finder. Windows is unaffected — its in-tree dev .ico
keeps the transparent-brain style because a black plate would vanish into a
dark Windows taskbar.

Outputs:
  ~/Library/Application Support/Cortex/Cortex.icns       — multi-res .icns of
                                                            the brain on a
                                                            black squircle plate.
  ~/Library/Application Support/Cortex/Cortex-Dev.icns   — same plate + brain,
                                                            plus a BRAIN_PINK
                                                            "DEV" label below
                                                            the brain (no
                                                            outline; the plate
                                                            provides contrast).
  ~/Library/Application Support/Cortex/Cortex-Dev.png    — intermediate 2048x2048
                                                            RGBA used to derive
                                                            the dev .icns. Kept
                                                            for visual reference.
  scripts/cortex-icon.icns                               — committed reference
                                                            .icns (mirror of the
                                                            prod .icns above) so
                                                            the in-repo asset
                                                            matches what ships.
  app/channels/local/icon/no-padding/icon-dev.ico        — in-tree, committed.
                                                            Embedded into the
                                                            Windows warp-oss.exe
                                                            dev build by
                                                            app/build.rs. Stays
                                                            transparent (no
                                                            plate) — see header
                                                            note above.

Mirror of scripts/build-shortcut-icons.py (which produces the .ico equivalents
on Windows). The transparent "DEV" composite logic lives in
_shortcut_icons_core.py and is imported by both entry points; the Mac plate
compositor below is a separate code path because the layout differs.

Run from repo root:
    python3 scripts/build-shortcut-icons-macos.py

Requires Pillow 9.x+ and the macOS-bundled `iconutil`.
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

_CORE_PATH = Path(__file__).resolve().parent / "_shortcut_icons_core.py"
_spec = importlib.util.spec_from_file_location("_shortcut_icons_core", _CORE_PATH)
_core = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_core)

REPO_ROOT = Path(__file__).resolve().parent.parent
MASTER_PNG = REPO_ROOT / "app" / "assets" / "cortex" / "CortexIcon.png"
COMMITTED_REFERENCE_ICNS = REPO_ROOT / "scripts" / "cortex-icon.icns"
IN_TREE_DEV_ICO = REPO_ROOT / "app" / "channels" / "local" / "icon" / "no-padding" / "icon-dev.ico"
ICO_SIZES = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]

# Mac plate ratios. Apple's HIG (confirmed by inspecting Terminal.app's .icns)
# expects the rounded-square tile to be INSET from the canvas to ~87.5% of
# canvas width, with transparent margin around it. macOS / Raycast / Spotlight
# render their own tile backdrop into that transparent margin; if the icon
# fills the full canvas instead, downstream renderers paint over the corners
# (giving a "white box around black plate" look in Raycast). The plate corner
# radius is ≈22.5% of the *plate* size (not the canvas).
#
# The brain glyph sits centered inside the plate, sized small enough to
# breathe inside it. Dev variant shrinks the brain further and shifts it up
# to make room for a "DEV" label below.
MAC_CANVAS = 2048
MAC_PLATE_COLOR = (0, 0, 0, 255)
MAC_PLATE_SIZE_RATIO = 0.875            # plate side / canvas side (Apple HIG)
MAC_PLATE_RADIUS_RATIO = 0.225          # corner radius / plate side
MAC_PROD_FILL_RATIO = 0.60              # brain content / canvas (prod)
MAC_DEV_FILL_RATIO = 0.48               # brain content / canvas (dev)
MAC_DEV_BRAIN_Y_OFFSET_RATIO = -0.06    # brain y offset / canvas (negative = up)
MAC_DEV_TEXT_TARGET_W_RATIO = 0.42      # DEV text width / canvas
MAC_DEV_TEXT_BASELINE_Y_RATIO = 0.80    # DEV text baseline y / canvas

# Apple's canonical iconset layout. Each tuple is (filename, pixel_size).
# @2x variants share a logical size with their non-@2x sibling but render at
# double the pixel resolution (Retina). iconutil reads this exact naming.
ICONSET_LAYOUT = [
    ("icon_16x16.png", 16),
    ("icon_16x16@2x.png", 32),
    ("icon_32x32.png", 32),
    ("icon_32x32@2x.png", 64),
    ("icon_128x128.png", 128),
    ("icon_128x128@2x.png", 256),
    ("icon_256x256.png", 256),
    ("icon_256x256@2x.png", 512),
    ("icon_512x512.png", 512),
    ("icon_512x512@2x.png", 1024),
]


def find_bold_font(size_px: int) -> ImageFont.FreeTypeFont:
    """Best-effort bold sans-serif font lookup for macOS."""
    candidates = [
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "/System/Library/Fonts/Supplemental/Impact.ttf",
        "/Library/Fonts/Arial Bold.ttf",
        "/System/Library/Fonts/HelveticaNeue.ttc",
        "/System/Library/Fonts/Helvetica.ttc",
        "/System/Library/Fonts/SFNS.ttf",
    ]
    for p in candidates:
        if os.path.isfile(p):
            return ImageFont.truetype(p, size_px)
    return _core.fallback_default_font(size_px)


def _autosize_font(text: str, target_w: int) -> ImageFont.FreeTypeFont:
    """Walk font sizes upward until the text width meets target_w."""
    font_size = 8
    font = find_bold_font(font_size)
    while font_size <= 1200:
        next_font = find_bold_font(font_size + 8)
        bbox = next_font.getbbox(text)
        if (bbox[2] - bbox[0]) >= target_w:
            return next_font
        font = next_font
        font_size += 8
    return font


def compose_macos_plate(brain_path: Path, dev_text: str | None = None) -> Image.Image:
    """Composite the brain glyph onto a black squircle plate (macOS-style tile).

    `dev_text=None` produces the prod variant (brain centered, ~65% canvas fill).
    Passing `dev_text="DEV"` (or any short label) shrinks the brain, shifts it
    up, and adds a BRAIN_PINK label below it within the plate.
    """
    canvas = Image.new("RGBA", (MAC_CANVAS, MAC_CANVAS), (0, 0, 0, 0))
    draw = ImageDraw.Draw(canvas)
    plate_size = int(round(MAC_CANVAS * MAC_PLATE_SIZE_RATIO))
    plate_x0 = (MAC_CANVAS - plate_size) // 2
    plate_y0 = plate_x0
    plate_x1 = plate_x0 + plate_size
    plate_y1 = plate_y0 + plate_size
    radius = int(round(plate_size * MAC_PLATE_RADIUS_RATIO))
    draw.rounded_rectangle((plate_x0, plate_y0, plate_x1, plate_y1), radius=radius, fill=MAC_PLATE_COLOR)

    src = Image.open(brain_path).convert("RGBA")
    bbox = src.getbbox()
    if bbox is None:
        raise ValueError(f"{brain_path} is fully transparent — cannot composite")
    cropped = src.crop(bbox)
    cw, ch = cropped.size
    fill_ratio = MAC_DEV_FILL_RATIO if dev_text else MAC_PROD_FILL_RATIO
    target = int(MAC_CANVAS * fill_ratio)
    scale = min(target / cw, target / ch)
    nw, nh = int(round(cw * scale)), int(round(ch * scale))
    scaled = cropped.resize((nw, nh), Image.LANCZOS)

    y_offset = int(MAC_CANVAS * MAC_DEV_BRAIN_Y_OFFSET_RATIO) if dev_text else 0
    canvas.paste(scaled, ((MAC_CANVAS - nw) // 2, (MAC_CANVAS - nh) // 2 + y_offset), scaled)

    if dev_text:
        target_w = int(MAC_CANVAS * MAC_DEV_TEXT_TARGET_W_RATIO)
        font = _autosize_font(dev_text, target_w)
        bbox = font.getbbox(dev_text)
        tw = bbox[2] - bbox[0]
        text_x = (MAC_CANVAS - tw) // 2 - bbox[0]
        text_y = int(MAC_CANVAS * MAC_DEV_TEXT_BASELINE_Y_RATIO) - bbox[1]
        draw.text((text_x, text_y), dev_text, font=font, fill=_core.BRAIN_PINK)

    return canvas


def _strip_icns_info_chunk(icns_path: Path) -> None:
    """Remove the trailing 'info' chunk iconutil adds since recent macOS.

    iconutil writes an NSKeyedArchiver bplist `info` chunk containing an
    `assetcatalog-reference` payload. When the icon ships standalone (not in
    an asset catalog), some modern renderers — confirmed: Raycast on macOS
    Sequoia — try to resolve that reference, fail, and paint a placeholder
    white tile under the icon. Apple's own .icns files (e.g. Terminal.app)
    don't carry this chunk; stripping it makes ours behave the same.
    """
    data = icns_path.read_bytes()
    if data[:4] != b"icns":
        return
    pos = 8
    kept: list[bytes] = []
    while pos < len(data):
        clen = int.from_bytes(data[pos + 4 : pos + 8], "big")
        if clen < 8 or pos + clen > len(data):
            break
        if data[pos : pos + 4] != b"info":
            kept.append(data[pos : pos + clen])
        pos += clen
    body = b"".join(kept)
    new_total = 8 + len(body)
    icns_path.write_bytes(b"icns" + new_total.to_bytes(4, "big") + body)


def img_to_icns(img: Image.Image, out_icns: Path) -> None:
    """Render a multi-res .icns from a 2048x2048 RGBA Image via iconutil."""
    out_icns.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as tmp:
        iconset_dir = Path(tmp) / "Cortex.iconset"
        iconset_dir.mkdir()
        for name, px in ICONSET_LAYOUT:
            resized = img.resize((px, px), Image.LANCZOS)
            resized.save(iconset_dir / name, format="PNG", optimize=True)
        subprocess.run(
            ["iconutil", "--convert", "icns", "--output", str(out_icns), str(iconset_dir)],
            check=True,
        )
    _strip_icns_info_chunk(out_icns)
    sizes_str = ", ".join(f"{px}x{px}" for _, px in ICONSET_LAYOUT)
    print(f"  wrote {out_icns}  (sizes: {sizes_str})")


def main() -> int:
    if sys.platform != "darwin":
        print(f"error: this script is macOS-only (sys.platform={sys.platform!r})", file=sys.stderr)
        return 1

    install_dir = Path.home() / "Library" / "Application Support" / "Cortex"
    install_dir.mkdir(parents=True, exist_ok=True)

    print(f"Output: {install_dir}")
    print()
    print("=== Prod icon: brain on black squircle plate ===")
    prod_img = compose_macos_plate(MASTER_PNG)
    img_to_icns(prod_img, install_dir / "Cortex.icns")
    img_to_icns(prod_img, COMMITTED_REFERENCE_ICNS)

    print()
    print("=== Dev icon: brain on plate + 'DEV' label below ===")
    dev_img = compose_macos_plate(MASTER_PNG, dev_text="DEV")
    dev_png_intermediate = install_dir / "Cortex-Dev.png"
    dev_png_intermediate.parent.mkdir(parents=True, exist_ok=True)
    dev_img.save(dev_png_intermediate, optimize=True)
    print(f"  wrote {dev_png_intermediate}  ({dev_img.size[0]}x{dev_img.size[1]} RGBA)")
    img_to_icns(dev_img, install_dir / "Cortex-Dev.icns")

    print()
    print("=== In-tree Windows dev .ico (transparent brain + DEV text — no plate) ===")
    win_dev_png = Path(tempfile.gettempdir()) / "cortex-windev.png"
    _core.build_dev_master(MASTER_PNG, win_dev_png, find_bold_font)
    Image.open(win_dev_png).convert("RGBA").save(IN_TREE_DEV_ICO, format="ICO", sizes=ICO_SIZES)
    print(f"  wrote {IN_TREE_DEV_ICO}  (sizes: {', '.join(f'{w}x{h}' for w, h in ICO_SIZES)})")

    print()
    print("Done. Mac .icns files include a black squircle plate; the committed")
    print("scripts/cortex-icon.icns mirrors the prod plate variant. The in-tree")
    print("icon-dev.ico stays transparent so Windows dark taskbars don't swallow")
    print("the icon. Both Mac .icns are referenced by ~/Applications/Cortex.app +")
    print("~/Applications/Cortex Dev.app via CFBundleIconFile in Info.plist.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
