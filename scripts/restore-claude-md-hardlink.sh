#!/usr/bin/env bash
# Restore the CLAUDE.md <-> docs/CLAUDE.md hardlink (macOS / Linux).
#
# Background: CLAUDE.md is auto-loaded by Claude Code from the repo root,
# but the canonical file lives in docs/CLAUDE.md (gitignored, Syncthing'd
# across machines). The two are joined by a POSIX hardlink so an edit in
# either location updates both. Many editors -- including atomic
# write-temp-rename paths -- break the link by writing a fresh inode at
# one of the two paths, leaving the two files diverged.
#
# This script idempotently re-establishes the hardlink. It picks the
# newer (mtime-most-recent) file as the truth source, deletes the other,
# and re-links from the truth to the deleted path. If only one exists,
# it creates the missing peer. If neither exists, it warns and exits 0.
#
# Pair: scripts/restore-claude-md-hardlink.ps1 on Windows. Keep the two
# in sync. The dev launchers invoke this at startup so a broken link
# never persists across two launches.

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" || {
    echo 'error: not in a git checkout' >&2; exit 1; }
root_path="$repo_root/CLAUDE.md"
docs_path="$repo_root/docs/CLAUDE.md"

# stat -c is GNU; stat -f is BSD/macOS. Probe once.
if stat -c %i "$repo_root" >/dev/null 2>&1; then
    inode_of() { stat -c %i  "$1"; }
    mtime_of() { stat -c %Y  "$1"; }
else
    inode_of() { stat -f %i  "$1"; }
    mtime_of() { stat -f %m  "$1"; }
fi

root_exists=false; [[ -f "$root_path" ]] && root_exists=true
docs_exists=false; [[ -f "$docs_path" ]] && docs_exists=true

# Colors only when stdout is a terminal.
if [[ -t 1 ]]; then YELLOW=$'\033[33m'; RESET=$'\033[0m'
else                 YELLOW='';        RESET=''; fi

if ! $root_exists && ! $docs_exists; then
    echo "warning: neither $root_path nor $docs_path exists; nothing to restore." >&2
    exit 0
fi

if $root_exists && ! $docs_exists; then
    printf '%s\n' "${YELLOW}[restore-claude-md-hardlink] docs/CLAUDE.md missing; creating hardlink to root CLAUDE.md.${RESET}"
    mkdir -p "$(dirname "$docs_path")"
    ln "$root_path" "$docs_path"
    exit 0
fi

if $docs_exists && ! $root_exists; then
    printf '%s\n' "${YELLOW}[restore-claude-md-hardlink] root CLAUDE.md missing; creating hardlink from docs/CLAUDE.md.${RESET}"
    ln "$docs_path" "$root_path"
    exit 0
fi

# Both exist: same inode == already hardlinked, no-op.
root_inode="$(inode_of "$root_path")"
docs_inode="$(inode_of "$docs_path")"
if [[ "$root_inode" == "$docs_inode" ]]; then
    exit 0
fi

# Both exist but are separate files. Pick the newer one as truth.
root_mtime="$(mtime_of "$root_path")"
docs_mtime="$(mtime_of "$docs_path")"

if [[ "$root_mtime" -ge "$docs_mtime" ]]; then
    truth="$root_path"
    stale="$docs_path"
    printf '%s\n' "${YELLOW}[restore-claude-md-hardlink] link broken; root CLAUDE.md is newer -> rebuilding docs/CLAUDE.md as hardlink.${RESET}"
else
    truth="$docs_path"
    stale="$root_path"
    printf '%s\n' "${YELLOW}[restore-claude-md-hardlink] link broken; docs/CLAUDE.md is newer -> rebuilding root CLAUDE.md as hardlink.${RESET}"
fi

rm -f "$stale"
ln "$truth" "$stale"
