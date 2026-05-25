#!/usr/bin/env bash
# Cortex pre-rebase audit (macOS / Linux).
#
# Run before kicking off `git rebase --onto upstream/master ...` to front-load
# the divergence survey. Without this, you discover Frankenstein conflicts
# mid-rebase, which is the worst time to make judgment calls.
#
# What it does:
#   1. Resolves the fork point: `git merge-base HEAD upstream/master`.
#      This is the last upstream commit Cortex landed on top of -- the
#      rebase target from the previous merge, or the original fork point
#      if no merge has happened yet. (Why not upstream/stable_release/*?
#      Post-rebase, HEAD contains all upstream commits the rebase
#      absorbed, so diffing against the stable_release branch
#      double-counts them and reports every upstream-changed file as
#      Cortex-changed. The merge-base is the true divergence point.)
#   2. Diffs upstream/master vs <merge-base>     -> upstream-changed files
#      (i.e. the upstream commits we haven't absorbed yet).
#   3. Diffs HEAD            vs <merge-base>     -> Cortex-changed files
#      (i.e. our customization commits since the fork point).
#   4. Intersects the two sets                   -> conflict candidates.
#   5. For each candidate, looks up the divergence registry
#      (docs/divergence-registry.md, gitignored / Syncthing-synced).
#   6. Prints a sorted report; unregistered candidates are flagged.
#
# Read the output before starting the rebase. Anything tagged
# UNREGISTERED is a divergence the registry forgot to record; add an
# entry to docs/divergence-registry.md before resolving the conflict.
#
# Pair: scripts/pre-rebase-audit.ps1 on Windows. Keep the two in sync.

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null)" || {
    echo 'error: not in a git checkout' >&2; exit 1; }
cd "$repo_root"

upstream_master='upstream/master'

# Sanity-check the upstream ref resolves.
git rev-parse --verify --quiet "$upstream_master" >/dev/null || {
    echo "error: ref $upstream_master not found. Run \`git fetch upstream\` first." >&2; exit 1; }

# --- Find the fork point: most recent commit common to HEAD and upstream/master.
fork_point="$(git merge-base HEAD "$upstream_master")"
if [[ -z "$fork_point" ]]; then
    echo 'error: could not determine merge-base; histories are unrelated?' >&2
    exit 1
fi

fork_short="$(git rev-parse --short "$fork_point")"
upstream_sha="$(git rev-parse --short "$upstream_master")"
head_sha="$(git rev-parse --short HEAD)"
head_ref="$(git rev-parse --abbrev-ref HEAD)"

# Colors only when stdout is a terminal.
if [[ -t 1 ]]; then
    CYAN=$'\033[36m'; YELLOW=$'\033[33m'; GREEN=$'\033[32m'; RESET=$'\033[0m'
else
    CYAN=''; YELLOW=''; GREEN=''; RESET=''
fi

echo
echo "${CYAN}=== Cortex pre-rebase audit ===${RESET}"
printf '  Fork point (merge-base):  %s\n'      "$fork_short"
printf '  Upstream HEAD:            %s  @ %s\n' "$upstream_master" "$upstream_sha"
printf '  Local HEAD:               %s  @ %s\n' "$head_ref"        "$head_sha"
echo

# --- Per-side change sets vs the fork point.
upstream_changes_file="$(mktemp)"
cortex_changes_file="$(mktemp)"
trap 'rm -f "$upstream_changes_file" "$cortex_changes_file"' EXIT

git diff --name-only "$fork_point..$upstream_master" > "$upstream_changes_file"
git diff --name-only "$fork_point..HEAD"             > "$cortex_changes_file"

upstream_count="$(wc -l < "$upstream_changes_file" | tr -d ' ')"
cortex_count="$(wc -l < "$cortex_changes_file"   | tr -d ' ')"

# Intersection -> conflict candidates.
candidates="$(comm -12 <(sort -u "$upstream_changes_file") <(sort -u "$cortex_changes_file"))"
if [[ -z "$candidates" ]]; then
    candidate_count=0
else
    candidate_count="$(printf '%s\n' "$candidates" | wc -l | tr -d ' ')"
fi

printf '  Upstream-changed since fork: %s file(s)\n' "$upstream_count"
printf '  Cortex-changed since fork:   %s file(s)\n' "$cortex_count"
printf '  Intersection (candidates):   %s file(s)\n' "$candidate_count"
echo

if [[ "$candidate_count" -eq 0 ]]; then
    echo "${GREEN}No conflict candidates. Clean rebase looks likely.${RESET}"
    exit 0
fi

# --- Load divergence registry (gitignored / Syncthing-synced).
registry_path="$repo_root/docs/divergence-registry.md"
registry_dump=''
if [[ -f "$registry_path" ]]; then
    # awk: track current "### `path`" or "### path" heading, emit "path<TAB>resolution"
    # whenever a "- **Resolution:** ..." bullet appears within that section.
    registry_dump="$(awk '
        /^### / {
            sub(/^### /, "", $0)
            gsub(/`/, "", $0)
            sub(/[[:space:]]+$/, "", $0)
            path = $0
            next
        }
        /^[[:space:]]*-[[:space:]]+\*\*Resolution:\*\*/ {
            if (path != "") {
                sub(/^[[:space:]]*-[[:space:]]+\*\*Resolution:\*\*[[:space:]]*/, "", $0)
                sub(/[[:space:]]+$/, "", $0)
                printf "%s\t%s\n", path, $0
                path = ""
            }
        }
    ' "$registry_path")"
else
    printf "${YELLOW}  (divergence registry not found at %s; classifications will be UNREGISTERED)${RESET}\n" "$registry_path"
    echo
fi

# --- Build rows: path<TAB>classification<TAB>cortex_add<TAB>cortex_del<TAB>upstream_add<TAB>upstream_del<TAB>cortex_delta
numstat_for() {
    # $1 = revspec, $2 = path
    git diff --numstat "$1" -- "$2" 2>/dev/null | head -1
}

rows_file="$(mktemp)"
trap 'rm -f "$upstream_changes_file" "$cortex_changes_file" "$rows_file"' EXIT

while IFS= read -r path; do
    [[ -z "$path" ]] && continue
    c_line="$(numstat_for "$fork_point..HEAD"             "$path")"
    u_line="$(numstat_for "$fork_point..$upstream_master" "$path")"
    parse_added()   { local l="$1"; [[ -z "$l" ]] && { echo 0; return; }; local f; f="$(echo "$l" | awk '{print $1}')"; [[ "$f" == '-' ]] && echo 0 || echo "$f"; }
    parse_removed() { local l="$1"; [[ -z "$l" ]] && { echo 0; return; }; local f; f="$(echo "$l" | awk '{print $2}')"; [[ "$f" == '-' ]] && echo 0 || echo "$f"; }
    c_add="$(parse_added   "$c_line")"
    c_del="$(parse_removed "$c_line")"
    u_add="$(parse_added   "$u_line")"
    u_del="$(parse_removed "$u_line")"
    cortex_delta=$(( c_add + c_del ))

    classification="UNREGISTERED"
    if [[ -n "$registry_dump" ]]; then
        match="$(printf '%s\n' "$registry_dump" | awk -F '\t' -v p="$path" '$1 == p {print $2; exit}')"
        if [[ -n "$match" ]]; then classification="$match"; fi
    fi

    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$path" "$classification" "$c_add" "$c_del" "$u_add" "$u_del" "$cortex_delta" \
        >> "$rows_file"
done <<<"$candidates"

# --- Sort by Cortex delta size descending. Field 7 is cortex_delta.
sorted_file="$(mktemp)"
trap 'rm -f "$upstream_changes_file" "$cortex_changes_file" "$rows_file" "$sorted_file"' EXIT
sort -t $'\t' -k7,7nr "$rows_file" > "$sorted_file"

echo "${CYAN}--- Conflict candidates (sorted by Cortex delta size desc) ---${RESET}"
echo

fmt='%-60s %-28s %12s %12s\n'
# shellcheck disable=SC2059
printf "$fmt" 'Path' 'Classification' 'Cortex +/-' 'Upstream +/-'
# shellcheck disable=SC2059
printf "$fmt" "$(printf '%.0s-' {1..60})" "$(printf '%.0s-' {1..28})" "$(printf '%.0s-' {1..12})" "$(printf '%.0s-' {1..12})"

unregistered=0
while IFS=$'\t' read -r path classification c_add c_del u_add u_del _; do
    cortex_col="+${c_add}/-${c_del}"
    upstream_col="+${u_add}/-${u_del}"
    if [[ "$classification" == 'UNREGISTERED' ]]; then
        # shellcheck disable=SC2059
        printf "${YELLOW}$fmt${RESET}" "$path" "$classification" "$cortex_col" "$upstream_col"
        unregistered=$((unregistered + 1))
    else
        # shellcheck disable=SC2059
        printf "$fmt" "$path" "$classification" "$cortex_col" "$upstream_col"
    fi
done < "$sorted_file"

echo
if [[ "$unregistered" -gt 0 ]]; then
    printf "${YELLOW}  %s UNREGISTERED divergence(s) above.${RESET}\n" "$unregistered"
    printf "${YELLOW}  Add entries to docs/divergence-registry.md before resolving the rebase.${RESET}\n"
else
    printf '%s\n' "${GREEN}  All candidates are registered. You have a pre-decided rule for each.${RESET}"
fi
echo

# --- Check for Warp-internal workflows that the rebase may re-introduce.
warp_workflows=()
for f in .github/workflows/*.yml; do
    [[ ! -f "$f" ]] && continue
    base="$(basename "$f")"
    case "$base" in
        ci.yml|cortex-*) continue ;;
        *) warp_workflows+=("$base") ;;
    esac
done
if [[ ${#warp_workflows[@]} -gt 0 ]]; then
    echo "${YELLOW}--- Warp-internal workflows detected (purge after rebase) ---${RESET}"
    for wf in "${warp_workflows[@]}"; do
        printf "${YELLOW}  .github/workflows/%s${RESET}\n" "$wf"
    done
    printf "${YELLOW}  These cause daily failure-notification emails. Delete them before pushing.${RESET}\n"
    printf "${YELLOW}  See: docs/upstream-updates.md § Post-merge: purge re-introduced Warp workflows${RESET}\n"
    echo
fi
