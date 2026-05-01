#!/usr/bin/env bash
# Cortex shortcut installer (macOS). Idempotent — re-run anytime.
#
# Sets up the two Raycast/Spotlight-discoverable entries:
#   ~/Applications/Cortex.app        — prod (real .app, copied from a release build)
#   ~/Applications/Cortex Dev.app    — dev  (symlink to target/debug/bundle/osx/Cortex.app)
# and the icon files referenced by both bundles' CFBundleIconFile:
#   ~/Library/Application Support/Cortex/Cortex.icns
#   ~/Library/Application Support/Cortex/Cortex-Dev.icns
#
# Run once after install-cortex-prod.sh (so prod exists), or run anytime to
# re-point shortcuts after moving things. If prod hasn't been installed yet
# this script invokes install-cortex-prod.sh for you.
#
# Pair with the launchers:
#   scripts/launch-cortex.sh        — daily-driver smart launcher
#   scripts/launch-cortex-dev.sh    — live-rebuild dev loop

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

SUPPORT_DIR="$HOME/Library/Application Support/Cortex"
PROD_ICNS="$SUPPORT_DIR/Cortex.icns"
DEV_ICNS="$SUPPORT_DIR/Cortex-Dev.icns"
PROD_APP="$HOME/Applications/Cortex.app"
DEV_APP_LINK="$HOME/Applications/Cortex Dev.app"
DEV_BUNDLE="$REPO_ROOT/target/debug/bundle/osx/Cortex.app"

# --- Generate icons if missing ------------------------------------------
if [[ ! -f "$PROD_ICNS" || ! -f "$DEV_ICNS" ]]; then
    echo "Icons missing - running build-shortcut-icons-macos.py..."
    if ! python3 "$REPO_ROOT/scripts/build-shortcut-icons-macos.py"; then
        echo "Icon generation failed. Install the script-side Python deps:" >&2
        echo "    pip3 install --user --break-system-packages -r scripts/requirements.txt" >&2
        echo "(--break-system-packages is needed on Homebrew/PEP 668 pythons; --user keeps it in ~/.local/.)" >&2
        exit 1
    fi
fi

# --- Ensure prod exists -------------------------------------------------
# Detect the legacy v1 install (symlink to target/debug/bundle/osx/Cortex.app)
# and refuse to apply prod icons through it — that would mutate the *debug*
# bundle rather than a stable prod copy. install-cortex-prod.sh handles the
# v1→v2 migration cleanly via `rm -rf` + `cp -R`.
if [[ -L "$PROD_APP" ]]; then
    echo "Detected legacy v1 install: $PROD_APP is a symlink to" >&2
    echo "    $(readlink "$PROD_APP")" >&2
    echo
    echo "v2 prod is a real release-build copy decoupled from target/. Migrate first:" >&2
    echo "    scripts/install-cortex-prod.sh" >&2
    echo "Then re-run this script." >&2
    exit 1
fi
if [[ ! -d "$PROD_APP" ]]; then
    echo "Prod app missing at $PROD_APP — running install-cortex-prod.sh first..."
    "$REPO_ROOT/scripts/install-cortex-prod.sh"
fi

# --- Apply the prod icon to the installed prod bundle. install-cortex-prod.sh
#     does this too if the .icns already existed at install time, but on a
#     first run the .icns didn't exist yet (we just generated it above), so
#     we re-apply here.
if [[ -d "$PROD_APP" && -f "$PROD_ICNS" ]]; then
    cp "$PROD_ICNS" "$PROD_APP/Contents/Resources/Cortex.icns"
    /usr/libexec/PlistBuddy -c "Set :CFBundleIconFile Cortex" "$PROD_APP/Contents/Info.plist" 2>/dev/null \
        || /usr/libexec/PlistBuddy -c "Add :CFBundleIconFile string Cortex" "$PROD_APP/Contents/Info.plist"
    # Trailing `|| true` because `grep "Apple Development"` exits 1 when there
    # is no Apple Development cert in the keychain. Without it, pipefail +
    # set -e abort the whole script silently right after the assignment.
    SIGNING_CERT="$(security find-identity -p codesigning -v 2>/dev/null | grep "Apple Development" | awk '{print $2}' | head -1 || true)"
    codesign --force --deep --options runtime --sign "${SIGNING_CERT:--}" "$PROD_APP" >/dev/null 2>&1 || true
fi

# --- Set up the dev symlink. Only meaningful if a dev bundle exists; if not,
#     point the user at the dev launcher (which builds and creates the bundle)
#     rather than failing. Dev .app is fully patched + icon-applied by
#     scripts/launch-cortex-dev.sh on each launch.
mkdir -p "$HOME/Applications"
if [[ -d "$DEV_BUNDLE" ]]; then
    ln -sfn "$DEV_BUNDLE" "$DEV_APP_LINK"
    echo "Dev symlink:   $DEV_APP_LINK -> $DEV_BUNDLE"
else
    echo "Dev bundle not built yet at $DEV_BUNDLE."
    echo "Run scripts/launch-cortex-dev.sh once to build it; the dev symlink"
    echo "will be created automatically by that script."
fi

# --- Bust LaunchServices cache so Raycast/Spotlight see the rebrand now,
#     not after the next reboot.
LSREG="/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister"
if [[ -x "$LSREG" ]]; then
    "$LSREG" -f "$PROD_APP" >/dev/null 2>&1 || true
    [[ -L "$DEV_APP_LINK" ]] && "$LSREG" -f "$DEV_APP_LINK" >/dev/null 2>&1 || true
fi

echo
echo "Done. Raycast and Spotlight index ~/Applications automatically."
echo
echo "  Cortex          -> $PROD_APP                (icon: Cortex.icns)"
if [[ -L "$DEV_APP_LINK" ]]; then
    echo "  Cortex Dev      -> $DEV_BUNDLE  (icon: Cortex-Dev.icns)"
fi
echo
echo "Type 'Cortex' or 'Cortex Dev' into Raycast and the entries will appear."
echo "For the staleness-aware prod launcher (recommended), bind the Raycast"
echo "Quicklink target to:  $REPO_ROOT/scripts/launch-cortex.sh"
echo "See docs/development/raycast-shortcuts.md for the Raycast setup."
