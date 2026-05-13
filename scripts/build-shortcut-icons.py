#!/usr/bin/env python3
"""Generate Windows taskbar shortcut icons for Cortex prod + dev lanes.

Outputs:
  %LOCALAPPDATA%/Cortex/Cortex.ico       — copy of the master multi-res .ico
  %LOCALAPPDATA%/Cortex/Cortex-Dev.ico   — same icon with "DEV" overlay below
                                            (BRAIN_PINK #F4B6C2 letters, black
                                            outline — matches the master's
                                            pink-brain + black-halo palette)

Why this lives in scripts/: per-machine taskbar shortcut icons are a personal
artifact, but the *recipe* should be reproducible — if the master icon ever
changes, running this script regenerates both shortcuts in one shot. Same
precedent as install-cortex-prod.cmd.

Run from repo root:
    python scripts/build-shortcut-icons.py

Requires Pillow 9.x+ (for ImageDraw.text stroke_width/stroke_fill).
"""

from __future__ import annotations

import importlib.util
import os
import shutil
import sys
from pathlib import Path

from PIL import Image, ImageFont

# Sibling module imported by file path because the dashed filenames in
# scripts/ aren't valid Python module identifiers for `import`.
_CORE_PATH = Path(__file__).resolve().parent / "_shortcut_icons_core.py"
_spec = importlib.util.spec_from_file_location("_shortcut_icons_core", _CORE_PATH)
_core = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_core)

REPO_ROOT = Path(__file__).resolve().parent.parent
MASTER_PNG = REPO_ROOT / "app" / "assets" / "cortex" / "CortexIcon.png"
MASTER_ICO = REPO_ROOT / "app" / "channels" / "oss" / "icon" / "no-padding" / "icon.ico"

# In-tree dev .ico that app/build.rs embeds into warp-oss.exe when
# WARP_APP_NAME=Cortex Dev (i.e. the dev lane). Sits next to the prod
# icon.ico in the channel build.rs actually reads from. CARGO_BIN_NAME isn't
# set during build-script execution, so build.rs falls back to "local" --
# that's why this lives under channels/local/, not channels/oss/.
IN_TREE_DEV_ICO = REPO_ROOT / "app" / "channels" / "local" / "icon" / "no-padding" / "icon-dev.ico"

ICO_SIZES = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]


def find_bold_font(size_px: int) -> ImageFont.FreeTypeFont:
    """Best-effort bold sans-serif font lookup for Windows."""
    candidates = [
        r"C:\Windows\Fonts\arialbd.ttf",
        r"C:\Windows\Fonts\seguibl.ttf",
        r"C:\Windows\Fonts\segoeuib.ttf",
        r"C:\Windows\Fonts\Impact.ttf",
        r"C:\Windows\Fonts\calibrib.ttf",
    ]
    for p in candidates:
        if os.path.isfile(p):
            return ImageFont.truetype(p, size_px)
    return _core.fallback_default_font(size_px)


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
    _core.build_dev_master(MASTER_PNG, dev_png, find_bold_font)
    dev_ico = install_dir / "Cortex-Dev.ico"
    png_to_multires_ico(dev_png, dev_ico)

    print()
    print("=== In-tree dev .ico (for app/build.rs to embed in warp-oss.exe dev builds) ===")
    png_to_multires_ico(dev_png, IN_TREE_DEV_ICO)

    print()
    print("Done. Use these in shortcut .lnk files via the IconLocation field.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
