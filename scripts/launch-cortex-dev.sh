#!/usr/bin/env bash
# Cortex rapid-iteration launcher (macOS) — DEV lane.
# Each run does an incremental cargo bundle + open, so any source edits
# Claude (or you) made since the last launch are picked up automatically.
#
# Pair with the prod lane (scripts/install-cortex-prod.sh +
# scripts/launch-cortex.sh). Both lanes share the warp-oss channel state at
# ~/Library/Application Support/dev.warp.WarpOSS/, so theme/setting changes
# made in dev appear in prod after prod restarts.
#
# Each launch captures the build's combined output to
#   <repo>/.cortex-logs/cortex-dev-<TS>.log
# (sibling prefix to prod-build logs `cortex-prod-*.log` written by the
# Windows installer; macOS prod doesn't tee yet) so Claude Code agents can
# read the current/last session log without copy/pasting from the terminal.
# See docs/logging/dev-loop-capture.md for the full agent-readable convention.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

# --- Secure storage backend: file-encrypted, not macOS Keychain.
#     Set once for the user's launchd session so the `open -a` below inherits
#     it. The app reads this in app/src/lib.rs and switches from Keychain to
#     an AES-256-GCM file in ~/Library/Application Support/<bundle-id>/.
#     Default macOS builds (no env set) still use Keychain, so shared
#     installs are unaffected. See scripts/launch-cortex.sh for the same
#     line on the prod lane.
launchctl setenv WARP_SECURE_STORAGE_FILE 1

# --- Preflight: see install-cortex-prod.sh for rationale.
if ! [ -d "/Applications/Xcode.app" ]; then
    echo "error: full Xcode is required. Install from the Mac App Store, then:" >&2
    echo "    sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer" >&2
    echo "    xcodebuild -runFirstLaunch" >&2
    echo "    xcodebuild -downloadComponent MetalToolchain" >&2
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
if ! command -v protoc >/dev/null 2>&1; then
    echo "error: protoc isn't installed (upstream macOS bootstrap forgets it)." >&2
    echo "Install:  brew install protobuf" >&2
    exit 1
fi

DEV_APP_LINK="$HOME/Applications/Cortex Dev.app"
SUPPORT_DIR="$HOME/Library/Application Support/Cortex"
DEV_ICNS="$SUPPORT_DIR/Cortex-Dev.icns"

# --- Setup capture path ---
LOGS_DIR="$REPO_ROOT/.cortex-logs"
mkdir -p "$LOGS_DIR"
LOG_TS="$(date +%Y-%m-%d-%H%M%S)"
LOG_PATH="$LOGS_DIR/cortex-dev-$LOG_TS.log"

# --- Retention: keep only the 10 newest dev-session logs. Done at launch
#     start (not exit) so a crash on the previous run can't postpone cleanup.
#     `ls -t` orders by mtime descending; tail -n +11 yields everything past
#     the 10th newest. xargs -r is GNU-only; on macOS BSD xargs we substitute
#     with a guard to avoid running rm with no args. Glob narrowed to the
#     `cortex-dev-` prefix so future prod-build logs (`cortex-prod-*.log`)
#     rotate on a separate counter.
old_logs="$(ls -t "$LOGS_DIR"/cortex-dev-*.log 2>/dev/null | tail -n +11 || true)"
if [[ -n "$old_logs" ]]; then
    # shellcheck disable=SC2086
    rm -f $old_logs
fi

# --- Gather build identity for the header ---
GIT_REV="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
GIT_BRANCH="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo unknown)"
GIT_DIRTY=""
if ! git diff --quiet 2>/dev/null; then GIT_DIRTY="+dirty"; fi

# --- Write log header. Same shape as the Windows dev launcher's header
#     (see scripts/launch-cortex-dev.bat) so agents can correlate across OSes.
{
    echo "=== Cortex launch started $(date) ==="
    echo "Branch:    $GIT_BRANCH"
    echo "Commit:    $GIT_REV$GIT_DIRTY"
    echo "OS:        macOS"
    echo "Launcher:  scripts/launch-cortex-dev.sh"
    echo
} > "$LOG_PATH"

echo "=== Building Cortex Dev (incremental) ==="
echo "Started: $(date)"
echo "Log:     $LOG_PATH"
echo

# --- Build & patch & launch ---
# CARGO_TERM_COLOR=never strips SGR escape codes so the log is readable as
# plain text by agents. CARGO_TERM_PROGRESS_WHEN=never silences the progress
# bar (non-TTY pipes usually do this anyway, but belt-and-suspenders).
#
# `script/run --dont-open` drives cargo bundle, the upstream codesign step,
# Info.plist setup, and resource bundling. We post-process its output below.
# `--features skip_login` enables Cortex's bypass of the warp-account login
# splash so a fresh dev profile lands directly in the terminal. The Windows
# dev launcher passes the same feature. `script/run` parses `--features` and
# forwards it through to both `cargo bundle` and the build's `FEATURES` env
# (see ./script/macos/run:99); an earlier note that `script/run` couldn't
# forward features cleanly was outdated.
export CARGO_TERM_COLOR=never
export CARGO_TERM_PROGRESS_WHEN=never

# Isolate dev's runtime state from prod. WARP_DATA_PROFILE is honored only
# in debug builds (gated on cfg!(debug_assertions) in
# crates/warp_core/src/channel/state.rs), so this has no effect on
# release-mode prod even if the env var leaks into its environment.
# Result: dev writes to ~/.warp-oss-dev/ while prod stays on ~/.warp-oss/.
export WARP_DATA_PROFILE=dev

set +e
PATH="$HOME/.cargo/bin:$PATH" ./script/run --features skip_login --dont-open 2>&1 | tee -a "$LOG_PATH"
CARGO_EXIT=${PIPESTATUS[0]}
set -e

if [[ "$CARGO_EXIT" -ne 0 ]]; then
    {
        echo
        echo "=== Cortex exited code=$CARGO_EXIT at $(date) ==="
    } | tee -a "$LOG_PATH"
    exit "$CARGO_EXIT"
fi

DEV_BUNDLE="$REPO_ROOT/target/debug/bundle/osx/Cortex.app"
if [[ ! -d "$DEV_BUNDLE" ]]; then
    echo "error: $DEV_BUNDLE not found after build" | tee -a "$LOG_PATH" >&2
    exit 1
fi

# --- Post-build Info.plist patch.
#     We rebrand the dev bundle in place so LaunchServices treats it as a
#     distinct app from prod (different name in Dock + Cmd+Tab, different
#     icon, different bundle id). Without the CFBundleIdentifier suffix,
#     macOS coalesces the two apps and routing for warposs:// URLs becomes
#     unpredictable.
PLIST="$DEV_BUNDLE/Contents/Info.plist"
PB="/usr/libexec/PlistBuddy"

CURRENT_ID="$("$PB" -c "Print :CFBundleIdentifier" "$PLIST" 2>/dev/null || echo "")"
if [[ "$CURRENT_ID" != *-dev ]]; then
    "$PB" -c "Set :CFBundleIdentifier ${CURRENT_ID}-dev" "$PLIST"
fi
"$PB" -c "Set :CFBundleName Cortex Dev" "$PLIST" 2>/dev/null \
    || "$PB" -c "Add :CFBundleName string Cortex Dev" "$PLIST"
"$PB" -c "Set :CFBundleDisplayName Cortex Dev" "$PLIST" 2>/dev/null \
    || "$PB" -c "Add :CFBundleDisplayName string Cortex Dev" "$PLIST"

if [[ -f "$DEV_ICNS" ]]; then
    cp "$DEV_ICNS" "$DEV_BUNDLE/Contents/Resources/Cortex-Dev.icns"
    "$PB" -c "Set :CFBundleIconFile Cortex-Dev" "$PLIST" 2>/dev/null \
        || "$PB" -c "Add :CFBundleIconFile string Cortex-Dev" "$PLIST"
fi

# --- Re-sign ad-hoc. The upstream script/macos/run signed with a real cert
#     (or `-` for ad-hoc) before we touched Info.plist; that signature is now
#     invalid. `--sign -` ad-hoc is sufficient for a locally-built dev bundle
#     and avoids a repeat keychain prompt.
codesign --force --deep --sign - "$DEV_BUNDLE" >/dev/null 2>&1 || {
    echo "warning: codesign failed; the dev bundle may refuse to launch" | tee -a "$LOG_PATH" >&2
}

# --- Symlink so Raycast/Spotlight see "Cortex Dev" in ~/Applications.
#     Symlink (not copy): each future build's patched bundle replaces the
#     same target. ln -sfn is idempotent.
mkdir -p "$HOME/Applications"
ln -sfn "$DEV_BUNDLE" "$DEV_APP_LINK"

# --- Register with LaunchServices so Raycast/Spotlight see "Cortex Dev"
#     immediately. We register the *symlink* path (in ~/Applications), not
#     the build-output target — LS keys off the path it's given, and
#     Raycast/Spotlight only surface apps whose registered path is in a
#     standard apps dir. Without this, the build-output bundle is
#     registered but invisible by name.
LSREG="/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister"
if [[ -x "$LSREG" ]]; then
    "$LSREG" -f "$DEV_APP_LINK" >/dev/null 2>&1 || true
fi

echo "Opening: $DEV_APP_LINK"
open -a "$DEV_APP_LINK"

# --- Footer (clean-exit sentinel agents look for) ---
{
    echo
    echo "=== Cortex exited code=0 at $(date) ==="
} >> "$LOG_PATH"

echo
echo "=== Done ==="
echo "Dev app: $DEV_APP_LINK"
echo "Log:     $LOG_PATH"
echo "Per-crate timings: target/cargo-timings/cargo-timing-*.html  (open the latest, if --timings was passed)"
