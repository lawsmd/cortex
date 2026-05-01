#!/usr/bin/env bash
# Backwards-compat alias for the macOS dev loop.
#
# This used to be the single-lane macOS installer (build + symlink
# ~/Applications/Cortex.app to target/debug/bundle/osx/Cortex.app). It's been
# superseded by a two-lane setup that mirrors the Windows workflow:
#
#   scripts/install-cortex-prod.sh    - build release + copy to ~/Applications/Cortex.app
#   scripts/launch-cortex.sh          - daily-driver smart launcher (use from Raycast)
#   scripts/launch-cortex-dev.sh      - live-rebuild dev loop  (this file delegates here)
#   scripts/install-shortcuts-macos.sh - one-shot setup of icons + dev symlink
#
# Full docs: docs/development/macos-prod-dev.md
#
# This shim stays around so that muscle memory (`./scripts/install-cortex.sh`)
# and any stale doc references still work. It just forwards to the new dev
# launcher.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "note: install-cortex.sh has been superseded by the two-lane workflow."
echo "      forwarding to scripts/launch-cortex-dev.sh (the dev loop)."
echo "      see docs/development/macos-prod-dev.md for the full setup."
echo

exec "$REPO_ROOT/scripts/launch-cortex-dev.sh" "$@"
