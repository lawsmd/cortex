#!/usr/bin/env bash
# Cortex macOS .dmg builder.
#
# Wraps the release-mode Cortex.app bundle into a drag-to-Applications .dmg
# that's the artifact you actually hand to an external user.
# Output: dist/Cortex.dmg (next to repo root).
#
# Distinct from scripts/install-cortex-prod.sh, which is the contributor's
# "build locally and copy to ~/Applications/Cortex.app" fast path. That
# script stays the daily driver for iterating; THIS script is strictly for
# producing a shareable artifact.
#
# Pipeline:
#   1. Build target/release/bundle/osx/Cortex.app via ./script/run --release
#      --dont-open (skipped if --no-build and a bundle already exists).
#   2. create-dmg (Homebrew tool) wraps it with an Applications shortcut and
#      a sensible window layout.
#   3. Output lands at dist/Cortex.dmg.
#
# Code-signing: the bundle is ad-hoc signed by ./script/run today, which is
# enough for the .dmg to mount and the .app to launch. It is NOT enough for
# Gatekeeper to skip the "from an unidentified developer" warning on a fresh
# download. Real notarization (Apple Developer ID + notarytool) is a
# separate, larger change — see docs/development/macos-prod-dev.md.
#
# Dependencies:
#   - create-dmg              brew install create-dmg
#   - the rest matches install-cortex-prod.sh (Xcode + Metal toolchain +
#     cargo-bundle + protoc).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

DIST_DIR="$REPO_ROOT/dist"
BUILT_APP="$REPO_ROOT/target/release/bundle/osx/Cortex.app"
DMG_PATH="$DIST_DIR/Cortex.dmg"

# --- Args. --no-build skips the cargo bundle step and uses an existing
#     bundle. Useful in CI when you've split build and packaging into
#     separate steps, or locally when iterating on the .dmg layout itself.
SKIP_BUILD=0
for arg in "$@"; do
    case "$arg" in
        --no-build) SKIP_BUILD=1 ;;
        -h|--help)
            sed -n '2,30p' "$0"
            exit 0
            ;;
        *)
            echo "error: unknown argument: $arg" >&2
            echo "usage: $(basename "$0") [--no-build]" >&2
            exit 1
            ;;
    esac
done

# --- Preflight: create-dmg lives outside the bootstrap script (it's
#     packaging-only), so flag it explicitly with the brew install hint
#     rather than letting the call below fail with a cryptic message.
if ! command -v create-dmg >/dev/null 2>&1; then
    echo "error: create-dmg isn't installed." >&2
    echo "Install: brew install create-dmg" >&2
    exit 1
fi

# --- Build the bundle (unless --no-build).
if [[ "$SKIP_BUILD" -eq 0 ]]; then
    if ! [ -d "/Applications/Xcode.app" ]; then
        echo "error: full Xcode is required (see install-cortex-prod.sh for setup)." >&2
        exit 1
    fi
    if ! PATH="$HOME/.cargo/bin:$PATH" command -v cargo-bundle >/dev/null 2>&1; then
        echo "error: cargo-bundle isn't installed (see install-cortex-prod.sh)." >&2
        exit 1
    fi
    if ! command -v protoc >/dev/null 2>&1; then
        echo "error: protoc isn't installed. brew install protobuf" >&2
        exit 1
    fi

    echo "=== Building release Cortex bundle ==="
    PATH="$HOME/.cargo/bin:$PATH" ./script/run --release --dont-open
fi

if [[ ! -d "$BUILT_APP" ]]; then
    echo "error: $BUILT_APP not found." >&2
    echo "Run without --no-build, or run scripts/install-cortex-prod.sh first." >&2
    exit 1
fi

mkdir -p "$DIST_DIR"
rm -f "$DMG_PATH"

# --- Stage the .app in a clean directory so create-dmg's window layout
#     reflects only what should appear in the mounted volume. (Pointing
#     create-dmg at target/release/bundle/osx/ would also pick up
#     anything else that lands there.)
STAGE_DIR="$(mktemp -d -t cortex-dmg-stage)"
trap 'rm -rf "$STAGE_DIR"' EXIT
cp -R "$BUILT_APP" "$STAGE_DIR/"

echo
echo "=== Building $DMG_PATH ==="

# --window-size / --icon-size / --icon / --app-drop-link give the standard
# drag-to-Applications layout. --no-internet-enable is a hardening flag that
# disables the "automount on download" behavior so the .dmg doesn't get
# extra Gatekeeper attention on first open. --hdiutil-quiet keeps the build
# output readable; drop it for verbose hdiutil diagnostics.
create-dmg \
    --volname "Cortex" \
    --window-pos 200 120 \
    --window-size 600 400 \
    --icon-size 100 \
    --icon "Cortex.app" 175 190 \
    --hide-extension "Cortex.app" \
    --app-drop-link 425 190 \
    --no-internet-enable \
    --hdiutil-quiet \
    "$DMG_PATH" \
    "$STAGE_DIR/"

echo
echo "=== Done ==="
echo "DMG:     $DMG_PATH"
echo "Volume:  Cortex"
echo
echo "Smoke test:"
echo "    open \"$DMG_PATH\""
echo "and drag Cortex.app to the Applications shortcut. On first launch the"
echo "ad-hoc signature triggers a Gatekeeper warning — right-click the app"
echo "in Finder and choose Open to bypass once. Notarization (out of scope"
echo "here) is what removes that warning on a fresh download."
