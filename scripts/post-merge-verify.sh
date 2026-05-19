#!/usr/bin/env bash
# Cortex post-merge verification harness (macOS / Linux).
#
# Run after a `git rebase --onto upstream/master ...` (or any large merge)
# to catch obvious regressions before the user discovers them by
# launching the dev lane.
#
# Two phases:
#   Phase 1: static sanity checks (fast, ~seconds)
#     - Theme YAML count is intact (>= 1000; current baseline ~1,078).
#     - CortexSettings module is present and parseable.
#     - Divergence registry exists at the expected path.
#     - CLAUDE.md hardlink is intact (root and docs/ share an inode).
#   Phase 2: cargo check (slow, ~30s-a few minutes on incremental)
#     - `cargo check --bin warp-oss --features gui,skip_login`
#     - Skippable with --no-build for fast iteration on static checks.
#
# Exit code 0 = all green. Non-zero = first failure printed in red and
# the rest of the report still emitted so you can see the full picture.
#
# Pair: scripts/post-merge-verify.ps1 on Windows. Keep the two in sync.

set -uo pipefail

no_build=false
for arg in "$@"; do
    case "$arg" in
        --no-build) no_build=true ;;
        -h|--help)
            grep -E '^# ' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *) echo "warning: unknown arg: $arg" >&2 ;;
    esac
done

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" || {
    echo 'error: not in a git checkout' >&2; exit 1; }
cd "$repo_root"

# Colors only when stdout is a terminal.
if [[ -t 1 ]]; then
    CYAN=$'\033[36m'; GREEN=$'\033[32m'; RED=$'\033[31m'; GRAY=$'\033[90m'; RESET=$'\033[0m'
else
    CYAN=''; GREEN=''; RED=''; GRAY=''; RESET=''
fi

failures=()

# Pretty per-check runner. $1 = name, $2 = shell expression that prints
# "<detail>" on success and any output to stderr on failure (or returns
# non-zero with no output to mean a generic failure).
check() {
    local name="$1"
    local detail
    printf '  - %-48s' "$name"
    if detail="$(eval "$2" 2>&1)"; then
        if [[ -n "$detail" ]]; then
            printf '%sok (%s)%s\n' "$GREEN" "$detail" "$RESET"
        else
            printf '%sok%s\n' "$GREEN" "$RESET"
        fi
    else
        printf '%sFAIL: %s%s\n' "$RED" "${detail:-no detail}" "$RESET"
        failures+=("$name")
    fi
}

inode_of() {
    if stat -c %i "$1" >/dev/null 2>&1; then stat -c %i "$1"
    else                                       stat -f %i "$1"; fi
}

echo
printf '%s=== Cortex post-merge verification ===%s\n' "$CYAN" "$RESET"
echo
printf '%sPhase 1: static sanity checks%s\n' "$CYAN" "$RESET"

check 'theme yaml count >= 1000' '
    count=$(find app/src/themes/wezterm_bundle/yaml -maxdepth 1 -name "*.yaml" 2>/dev/null | wc -l | tr -d " ")
    if [[ "$count" -lt 1000 ]]; then
        echo "only $count theme yamls; expected >= 1000 (baseline ~1,078)" >&2
        false
    else
        printf "%s yamls" "$count"
    fi
'

check 'CortexSettings module present' '
    path="app/src/settings/cortex.rs"
    if [[ ! -f "$path" ]]; then echo "$path missing" >&2; false
    elif ! grep -q "pub[[:space:]]\+enum[[:space:]]\+TabsSelectedTitleAlignment" "$path"; then
        echo "TabsSelectedTitleAlignment enum missing from cortex.rs" >&2; false
    else
        printf "cortex.rs + TabsSelectedTitleAlignment"
    fi
'

check 'cortex_settings/ pane module present' '
    if [[ ! -d "app/src/cortex_settings" ]]; then
        echo "app/src/cortex_settings/ directory missing" >&2; false
    elif [[ ! -f "app/src/cortex_settings/brand.rs" ]]; then
        echo "app/src/cortex_settings/brand.rs missing" >&2; false
    else
        printf "brand.rs present"
    fi
'

check 'divergence registry present' '
    if [[ ! -f "docs/divergence-registry.md" ]]; then
        echo "docs/divergence-registry.md missing (Syncthing not running?)" >&2; false
    else
        size=$(wc -c < "docs/divergence-registry.md" | tr -d " ")
        printf "%s bytes" "$size"
    fi
'

check 'CLAUDE.md hardlink intact' '
    root_inode=$('"$(declare -f inode_of)"'; inode_of CLAUDE.md 2>/dev/null)
    docs_inode=$('"$(declare -f inode_of)"'; inode_of docs/CLAUDE.md 2>/dev/null)
    if [[ -z "$root_inode" ]] || [[ -z "$docs_inode" ]]; then
        echo "could not stat one or both CLAUDE.md paths" >&2; false
    elif [[ "$root_inode" != "$docs_inode" ]]; then
        echo "different inodes -- run scripts/restore-claude-md-hardlink.sh" >&2; false
    else
        printf "shares inode"
    fi
'

check 'pre-rebase audit script present' '
    if [[ ! -f "scripts/pre-rebase-audit.sh" ]]; then
        echo "scripts/pre-rebase-audit.sh missing" >&2; false
    else
        printf "present"
    fi
'

if $no_build; then
    echo
    printf '%sPhase 2 skipped (--no-build).%s\n' "$GRAY" "$RESET"
else
    echo
    printf '%sPhase 2: cargo check (slow)%s\n' "$CYAN" "$RESET"
    echo "  Running: cargo check --bin warp-oss --features gui,skip_login"
    echo "  (use --no-build to skip)"
    echo
    if ! cargo check --bin warp-oss --features gui,skip_login; then
        echo
        printf '  %sFAIL: cargo check exited with code %s%s\n' "$RED" "$?" "$RESET"
        failures+=('cargo check')
    else
        echo
        printf '  %scargo check: ok%s\n' "$GREEN" "$RESET"
    fi
fi

echo
if [[ "${#failures[@]}" -eq 0 ]]; then
    printf '%s=== All checks passed ===%s\n' "$GREEN" "$RESET"
    exit 0
else
    printf '%s=== %s failure(s): %s ===%s\n' "$RED" "${#failures[@]}" "${failures[*]}" "$RESET"
    exit 1
fi
