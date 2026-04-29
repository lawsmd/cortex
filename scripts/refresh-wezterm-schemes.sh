#!/usr/bin/env bash
# Refresh scripts/wezterm-schemes.txt from a wezterm installation that has
# already produced ~/.config/terminal/wezterm/.theme-data via the SideQuest
# config's auto-generation logic (see base_config.lua in that repo).
#
# Contributors without WezTerm + that config don't need to run this — the
# committed scripts/wezterm-schemes.txt is the source of truth and the
# generator (generate-wezterm-yaml.py) reads from it directly.
#
# Override the source path with WEZTERM_THEME_DATA=/path/to/.theme-data.

set -euo pipefail

SOURCE="${WEZTERM_THEME_DATA:-$HOME/.config/terminal/wezterm/.theme-data}"
DEST="$(cd "$(dirname "$0")/.." && pwd)/scripts/wezterm-schemes.txt"

if [[ ! -f "$SOURCE" ]]; then
  echo "error: $SOURCE not found." >&2
  echo "Open WezTerm once with the ~/.config/terminal/wezterm config, or set" >&2
  echo "WEZTERM_THEME_DATA to a path produced by base_config.lua." >&2
  exit 1
fi

if cmp -s "$SOURCE" "$DEST"; then
  echo "no change ($DEST)"
  exit 0
fi

cp "$SOURCE" "$DEST"
echo "updated: $DEST ($(wc -l <"$DEST" | tr -d ' ') lines)"
echo "rerun: python3 scripts/generate-wezterm-yaml.py --clean"
