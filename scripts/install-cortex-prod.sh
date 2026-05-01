#!/usr/bin/env bash
# Cortex prod installer (macOS).
# Builds a release-mode warp-oss bundle and copies it to a stable location at
#   ~/Applications/Cortex.app
# decoupled from target/, so dev rebuilds (./scripts/launch-cortex-dev.sh) and
# Cloud agents never touch the daily-driver bundle. Run this whenever you want
# prod to catch up to main.
#
# Pair with:
#   scripts/launch-cortex.sh        - daily-driver smart launcher (Raycast)
#   scripts/launch-cortex-dev.sh    - live-rebuild dev loop
#
# See docs/development/macos-prod-dev.md for the full two-lane workflow.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# --- Preflight: catch the two fresh-machine deps most likely to be missing
#     and surface a clear pointer instead of letting the build fail mid-flight
#     with a cryptic message. Both are installed by script/macos/bootstrap.
if ! [ -d "/Applications/Xcode.app" ]; then
    echo "error: full Xcode is required (Command Line Tools alone isn't enough)." >&2
    echo "Install Xcode from the Mac App Store, then run:" >&2
    echo "    sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer" >&2
    echo "    xcodebuild -runFirstLaunch" >&2
    echo "    xcodebuild -downloadComponent MetalToolchain" >&2
    echo "(Metal shader compilation needs Xcode's metal toolchain.)" >&2
    exit 1
fi
XCODE_SELECT_PATH="$(xcode-select -p 2>/dev/null || true)"
if [[ "$XCODE_SELECT_PATH" != *"/Xcode.app/"* ]]; then
    echo "error: xcode-select points at CommandLineTools (no metal compiler there)." >&2
    echo "Switch to the full Xcode and download the Metal toolchain:" >&2
    echo "    sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer" >&2
    echo "    xcodebuild -runFirstLaunch" >&2
    echo "    xcodebuild -downloadComponent MetalToolchain" >&2
    exit 1
fi
if ! xcrun -f metal >/dev/null 2>&1; then
    echo "error: Metal toolchain not found." >&2
    echo "Run:  xcodebuild -downloadComponent MetalToolchain" >&2
    exit 1
fi
if ! PATH="$HOME/.cargo/bin:$PATH" command -v cargo-bundle >/dev/null 2>&1; then
    echo "error: cargo-bundle isn't installed." >&2
    echo "Install it (matches script/macos/bootstrap):" >&2
    echo "    cargo install cargo-bundle --git=https://github.com/burtonageo/cargo-bundle --rev ae4c76e92c08774bf54ff077b1c52e3d1cd6c16d" >&2
    echo "Or run the full bootstrap once: ./script/bootstrap" >&2
    exit 1
fi
# protoc is a real build dep but script/macos/bootstrap doesn't install it
# (the Linux install_build_deps does). See docs/development/macos-prod-dev.md
# "Bootstrap gaps" for the list of brew installs upstream forgets.
if ! command -v protoc >/dev/null 2>&1; then
    echo "error: protoc isn't installed (upstream macOS bootstrap forgets it)." >&2
    echo "Install:  brew install protobuf" >&2
    exit 1
fi

PROD_APP="$HOME/Applications/Cortex.app"
SUPPORT_DIR="$HOME/Library/Application Support/Cortex"
STAMP="$SUPPORT_DIR/Cortex.build-info"
BUILT_APP="$REPO_ROOT/target/release/bundle/osx/Cortex.app"

# --- Refuse to clobber a running prod app. Unlike Windows, macOS lets you
#     replace a running binary's file — but the *running* prod process keeps
#     using the old inode until it exits, which is confusing ("I just rebuilt,
#     why isn't my change live?"). Bail with a clear message instead.
#     Match against the prod bundle's MacOS executable path specifically so
#     a running "Cortex Dev" doesn't trip this guard.
if pgrep -fl "$PROD_APP/Contents/MacOS" >/dev/null 2>&1; then
    echo
    echo "Cortex is currently running. Quit it before re-installing prod."
    echo "  (Cmd+Q the Cortex window, or run:"
    echo "     pkill -f \"$PROD_APP/Contents/MacOS\")"
    echo
    exit 1
fi

echo "=== Building release Cortex (slow on a cold cache; ~5-10 min) ==="
echo "Started: $(date)"
echo

# ./script/run --release --dont-open drives cargo bundle --release, runs the
# upstream's update_plist + prepare_bundled_resources + compile_icon helpers,
# and codesigns (auto-detects an Apple Development cert via security
# find-identity, falls back to ad-hoc). It produces target/release/bundle/osx/Cortex.app.
PATH="$HOME/.cargo/bin:$PATH" ./script/run --release --dont-open

if [[ ! -d "$BUILT_APP" ]]; then
    echo
    echo "error: $BUILT_APP not found after build" >&2
    exit 1
fi

echo
echo "=== Installing to $PROD_APP ==="
mkdir -p "$(dirname "$PROD_APP")"
rm -rf "$PROD_APP"
cp -R "$BUILT_APP" "$PROD_APP"

# --- Custom icon for the prod bundle if it exists.
#     The cargo-bundle output already has CortexIcon.icns embedded, but if the
#     user has run install-shortcuts-macos.sh (which generates a multi-res
#     Cortex.icns under ~/Library/Application Support/Cortex/) we copy that
#     into Resources/ so the prod bundle's icon stays in sync with the dev
#     overlay icon's source.
PROD_ICNS="$SUPPORT_DIR/Cortex.icns"
if [[ -f "$PROD_ICNS" ]]; then
    cp "$PROD_ICNS" "$PROD_APP/Contents/Resources/Cortex.icns"
    /usr/libexec/PlistBuddy -c "Set :CFBundleIconFile Cortex" "$PROD_APP/Contents/Info.plist" 2>/dev/null \
        || /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string Cortex" "$PROD_APP/Contents/Info.plist"
    # Re-sign ad-hoc since modifying Info.plist invalidated the signature
    # script/macos/run applied. Re-finding the cert keeps parity with the
    # original sign command.
    # See install-shortcuts-macos.sh for the rationale on `|| true` here.
    SIGNING_CERT="$(security find-identity -p codesigning -v 2>/dev/null | grep "Apple Development" | awk '{print $2}' | head -1 || true)"
    codesign --force --deep --options runtime --sign "${SIGNING_CERT:--}" "$PROD_APP" >/dev/null 2>&1
fi

# --- Build stamp: lets launch-cortex.sh detect when prod is behind HEAD.
#     Same key=val format the Windows installer writes to Cortex.build-info,
#     so any future tooling can read either OS's stamp identically.
mkdir -p "$SUPPORT_DIR"
BUILD_COMMIT="$(git rev-parse HEAD 2>/dev/null || true)"
BUILD_BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || true)"
BUILD_DIRTY=no
if ! git diff --quiet 2>/dev/null; then BUILD_DIRTY=yes; fi
BUILD_TIME="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

if [[ -n "$BUILD_COMMIT" ]]; then
    {
        printf "commit=%s\n" "$BUILD_COMMIT"
        printf "branch=%s\n" "$BUILD_BRANCH"
        printf "dirty=%s\n" "$BUILD_DIRTY"
        printf "built=%s\n" "$BUILD_TIME"
    } > "$STAMP"
fi

echo
echo "=== Done ==="
echo "Prod app:    $PROD_APP"
echo "Build stamp: $STAMP  (commit ${BUILD_COMMIT:0:7})"
echo "Launcher:    scripts/launch-cortex.sh"
echo
echo "To set up the Cortex Dev shortcut and DEV-overlay icon, run once:"
echo "    scripts/install-shortcuts-macos.sh"
echo "Idempotent — re-run anytime."
echo
echo "Finished: $(date)"

# CORTEX_NONINTERACTIVE=1 from launch-cortex.sh's [R]ebuild path skips the
# trailing pause so the launcher can immediately start the new app.
if [[ -z "${CORTEX_NONINTERACTIVE:-}" ]]; then
    if [[ -t 0 ]]; then
        echo "Press any key to close..."
        read -r -n 1 -s
        echo
    fi
fi
