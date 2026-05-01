#!/usr/bin/env bash
# Cortex prod launcher (macOS). Daily-driver entry point.
# Either run directly, or — easier — launch via Raycast/Spotlight by typing
# "Cortex" once ~/Applications/Cortex.app exists. Both reach the smart launch
# below either way: Raycast indexes ~/Applications and runs the .app's main
# binary, but for the *staleness check* you want this script in front of the
# .app. See docs/development/raycast-shortcuts.md for the wrapper-app pattern.
#
# Smart-launch behavior: before opening prod, compare the commit prod was
# built from (read from Cortex.build-info, written by install-cortex-prod.sh)
# to the current HEAD. If they match, launch silently. If prod is behind,
# show an osascript dialog:
#   [Launch existing]  [Rebuild then launch]  [Cancel]
#
# Why: prod is a *copy* of the .app, decoupled from target/. Cloud agents
# and dev rebuilds never touch it — which means it can also silently fall
# behind. This check makes drift visible without forcing a rebuild every
# launch.

set -euo pipefail

REPO="$HOME/cortex"
PROD_APP="$HOME/Applications/Cortex.app"
SUPPORT_DIR="$HOME/Library/Application Support/Cortex"
STAMP="$SUPPORT_DIR/Cortex.build-info"

if [[ ! -d "$PROD_APP" ]]; then
    osascript -e 'display dialog "Cortex prod isn'"'"'t installed yet.\n\nRun:\n    ~/cortex/scripts/install-cortex-prod.sh" buttons {"OK"} default button "OK" with title "Cortex"' >/dev/null 2>&1 || true
    echo "Cortex prod isn't installed. Run scripts/install-cortex-prod.sh first." >&2
    exit 1
fi

# --- Determine staleness ---
CURRENT="$(git -C "$REPO" rev-parse HEAD 2>/dev/null || true)"
CURRENT_SHORT="$(git -C "$REPO" rev-parse --short HEAD 2>/dev/null || true)"

BUILD=""
if [[ -f "$STAMP" ]]; then
    BUILD="$(grep '^commit=' "$STAMP" 2>/dev/null | head -1 | cut -d= -f2- || true)"
fi

# Bail out of the staleness check if we can't determine either side. Just launch.
if [[ -z "$CURRENT" || -z "$BUILD" || "$CURRENT" == "$BUILD" ]]; then
    open -a "$PROD_APP" "$@"
    exit 0
fi

# --- Stale: count commits behind, get short hash for display ---
AHEAD="$(git -C "$REPO" rev-list --count "$BUILD..HEAD" 2>/dev/null || echo "?")"
BUILD_SHORT="$(git -C "$REPO" rev-parse --short "$BUILD" 2>/dev/null || echo "$BUILD")"

DIALOG_TEXT="Prod is ${AHEAD} commit(s) behind your working tree.

  prod commit:   ${BUILD_SHORT}
  current HEAD:  ${CURRENT_SHORT}

Launch the existing prod, rebuild and launch fresh, or cancel?"

# AppleScript returns the button label as the result; capture it.
# The "with title" controls the dialog window title shown by macOS.
CHOICE="$(osascript <<EOF
display dialog "$DIALOG_TEXT" \
    buttons {"Cancel", "Launch existing", "Rebuild then launch"} \
    default button "Launch existing" \
    with title "Cortex — prod is stale"
return button returned of result
EOF
)"

case "$CHOICE" in
    "Launch existing")
        open -a "$PROD_APP" "$@"
        ;;
    "Rebuild then launch")
        # Open Terminal and run the installer there so the user sees the build
        # output. CORTEX_NONINTERACTIVE=1 skips the installer's trailing
        # "press any key" prompt so Terminal closes (or stays for review per
        # user's Terminal prefs) right after the build, with the prod app
        # already opened by the chained command.
        SCRIPT="$REPO/scripts/install-cortex-prod.sh"
        osascript <<APPLESCRIPT
tell application "Terminal"
    activate
    do script "export CORTEX_NONINTERACTIVE=1 && \"$SCRIPT\" && open -a \"$PROD_APP\""
end tell
APPLESCRIPT
        ;;
    *)
        # Cancel or unknown — exit cleanly.
        exit 0
        ;;
esac
