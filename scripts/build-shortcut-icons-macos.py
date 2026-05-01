#!/usr/bin/env python3
"""Generate macOS Raycast/Spotlight shortcut icons for Cortex prod + dev lanes.

Outputs:
  ~/Library/Application Support/Cortex/Cortex.icns       — multi-resolution
                                                            .icns from the
                                                            master CortexIcon.png
  ~/Library/Application Support/Cortex/Cortex-Dev.icns   — same icon with "DEV"
                                                            overlay (pink
                                                            #F000D0 letters,
                                                            dark purple #200040
                                                            outline)
  ~/Library/Application Support/Cortex/Cortex-Dev.png    — intermediate 2048x2048
                                                            RGBA used to derive
                                                            the dev .icns. Kept
                                                            for visual reference.

Mirror of scripts/build-shortcut-icons.py (which produces the .ico equivalents
on Windows). The shared 2048x2048 "DEV" composite logic lives in
_shortcut_icons_core.py and is imported by both entry points.

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

from PIL import Image, ImageFont

_CORE_PATH = Path(__file__).resolve().parent / "_shortcut_icons_core.py"
_spec = importlib.util.spec_from_file_location("_shortcut_icons_core", _CORE_PATH)
_core = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_core)

REPO_ROOT = Path(__file__).resolve().parent.parent
MASTER_PNG = REPO_ROOT / "app" / "assets" / "cortex" / "CortexIcon.png"

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


def png_to_icns(src_png: Path, out_icns: Path) -> None:
    """Render a multi-res .icns from a single high-res RGBA PNG via iconutil."""
    img = Image.open(src_png).convert("RGBA")
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
    print("=== Prod icon (multi-res .icns from CortexIcon.png) ===")
    prod_icns = install_dir / "Cortex.icns"
    png_to_icns(MASTER_PNG, prod_icns)

    print()
    print("=== Dev icon (with 'DEV' text overlay) ===")
    dev_png = install_dir / "Cortex-Dev.png"
    _core.build_dev_master(MASTER_PNG, dev_png, find_bold_font)
    dev_icns = install_dir / "Cortex-Dev.icns"
    png_to_icns(dev_png, dev_icns)

    print()
    print("Done. Both .icns files are referenced by ~/Applications/Cortex.app")
    print("and ~/Applications/Cortex Dev.app via CFBundleIconFile in Info.plist.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
