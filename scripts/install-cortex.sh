#!/usr/bin/env bash
# Build a fresh Cortex dev bundle and ensure ~/Applications/Cortex.app
# points at it (so Raycast / Spotlight launches the latest code).
#
# Run after code changes:
#   ./scripts/install-cortex.sh
# Then CMD+Q the running Cortex and relaunch via Raycast.
#
# Branding (name, bundle id, icon) is baked into app/Cargo.toml's
# [package.metadata.bundle.bin.warp-oss] section, so cargo bundle produces
# Cortex.app directly — no post-processing here.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

PATH="$HOME/.cargo/bin:$PATH" ./script/run --dont-open

BUILT="$REPO_ROOT/target/debug/bundle/osx/Cortex.app"
LINK="$HOME/Applications/Cortex.app"

if [[ ! -d "$BUILT" ]]; then
  echo "error: $BUILT not found after build" >&2
  exit 1
fi

mkdir -p "$(dirname "$LINK")"
ln -sfn "$BUILT" "$LINK"

# One-time cleanup: remove the legacy Quest.app symlink left from the
# pre-rename install path. Safe to delete this block once it's run on every
# machine that ever had Quest installed.
if [[ -L "$HOME/Applications/Quest.app" ]]; then
  rm "$HOME/Applications/Quest.app"
  echo "removed legacy ~/Applications/Quest.app symlink"
fi

echo
echo "linked: $LINK -> $BUILT"
echo "next: CMD+Q the running Cortex, then Raycast → Cortex"
