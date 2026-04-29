#!/usr/bin/env python3
"""Generate Warp-format theme YAMLs from a wezterm scheme dump.

Reads scripts/wezterm-schemes.txt (pipe-delimited, 19 fields per line:
name|fg|bg|ansi[0..7]|bright[0..7]) and writes one YAML per scheme into
app/src/themes/wezterm_bundle/yaml/. The Cortex binary embeds those YAMLs at
build time via include_dir!.

Usage:
    python3 scripts/generate-wezterm-yaml.py [--clean]

--clean wipes the output dir before writing. Default behavior overwrites in
place; files in the output dir not produced by this run are left alone.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SOURCE = REPO_ROOT / "scripts" / "wezterm-schemes.txt"
OUT_DIR = REPO_ROOT / "app" / "src" / "themes" / "wezterm_bundle" / "yaml"

ANSI_KEYS = ("black", "red", "green", "yellow", "blue", "magenta", "cyan", "white")


def parse_hex(h: str) -> tuple[int, int, int]:
    h = h.lstrip("#")
    return int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16)


def relative_luminance(hex_color: str) -> float:
    r, g, b = (c / 255.0 for c in parse_hex(hex_color))

    def channel(c: float) -> float:
        return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4

    return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)


def slug(name: str) -> str:
    s = name.lower()
    s = re.sub(r"[^a-z0-9_-]+", "_", s)
    s = re.sub(r"_+", "_", s).strip("_")
    return s or "theme"


def yaml_quote(s: str) -> str:
    # Single-quote with the standard YAML escape (double single quotes inside).
    return "'" + s.replace("'", "''") + "'"


def render_yaml(name: str, fg: str, bg: str, accent: str, details: str,
                normal: list[str], bright: list[str]) -> str:
    lines = [
        f"name: {yaml_quote(name)}",
        f"accent: {yaml_quote(accent)}",
        f"background: {yaml_quote(bg)}",
        f"foreground: {yaml_quote(fg)}",
        f"details: {details}",
        "terminal_colors:",
        "  normal:",
    ]
    for key, value in zip(ANSI_KEYS, normal):
        lines.append(f"    {key}: {yaml_quote(value)}")
    lines.append("  bright:")
    for key, value in zip(ANSI_KEYS, bright):
        lines.append(f"    {key}: {yaml_quote(value)}")
    lines.append("")
    return "\n".join(lines)


def pick_accent(normal: list[str], bright: list[str], foreground: str) -> str:
    # bright[4] is bright blue; ansi[4] is blue. Fall back to blue if bright
    # blue collapses to the foreground (defensive against monochrome schemes).
    candidate = bright[4]
    if candidate.lower() == foreground.lower():
        candidate = normal[4]
    return candidate


def pick_details(foreground: str, background: str) -> str:
    # "darker" = light text on dark background (UI surfaces should be darker
    # than the bg). "lighter" = the inverse.
    return "darker" if relative_luminance(foreground) > relative_luminance(background) else "lighter"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clean", action="store_true",
                        help="Remove existing YAMLs in the output dir before writing.")
    args = parser.parse_args()

    if not SOURCE.exists():
        print(f"error: {SOURCE} not found", file=sys.stderr)
        return 1

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    if args.clean:
        for old in OUT_DIR.glob("*.yaml"):
            old.unlink()

    written = 0
    skipped = 0
    collisions = 0
    used_slugs: dict[str, int] = {}

    with SOURCE.open() as f:
        for lineno, raw in enumerate(f, start=1):
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            fields = line.split("|")
            if len(fields) != 19:
                print(f"warn: line {lineno}: expected 19 fields, got {len(fields)}; skipping",
                      file=sys.stderr)
                skipped += 1
                continue
            name = fields[0]
            fg = fields[1]
            bg = fields[2]
            normal = fields[3:11]
            bright = fields[11:19]

            if any(not v for v in (name, fg, bg, *normal, *bright)):
                print(f"warn: line {lineno}: scheme {name!r} has empty color field(s); skipping",
                      file=sys.stderr)
                skipped += 1
                continue

            base = slug(name)
            count = used_slugs.get(base, 0)
            if count == 0:
                filename = f"{base}.yaml"
            else:
                filename = f"{base}_{count + 1}.yaml"
                collisions += 1
            used_slugs[base] = count + 1

            accent = pick_accent(normal, bright, fg)
            details = pick_details(fg, bg)

            doc = render_yaml(name, fg, bg, accent, details, normal, bright)
            (OUT_DIR / filename).write_text(doc)
            written += 1

    print(f"wrote {written} themes ({collisions} collisions resolved, {skipped} skipped) to {OUT_DIR.relative_to(REPO_ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
