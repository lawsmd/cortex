#!/usr/bin/env pwsh
# Cortex pre-rebase audit (Windows / cross-platform PowerShell).
#
# Run before kicking off `git rebase --onto upstream/master ...` to front-load
# the divergence survey. Without this, you discover Frankenstein conflicts
# mid-rebase, which is the worst time to make judgment calls.
#
# What it does:
#   1. Resolves the fork point: `git merge-base HEAD upstream/master`.
#      This is the last upstream commit Cortex landed on top of — the
#      rebase target from the previous merge, or the original fork point
#      if no merge has happened yet. (Why not `upstream/stable_release/*`?
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
# Pair: scripts/pre-rebase-audit.sh on macOS/Linux. Keep the two in sync.

$ErrorActionPreference = 'Stop'

$repoRoot = (& git rev-parse --show-toplevel 2>$null)
if (-not $repoRoot) {
    Write-Error 'not in a git checkout'
    exit 1
}
Set-Location $repoRoot

$upstreamMaster = 'upstream/master'

# Sanity-check the upstream ref resolves.
& git rev-parse --verify --quiet $upstreamMaster > $null
if ($LASTEXITCODE) { Write-Error "ref $upstreamMaster not found. Run ``git fetch upstream`` first."; exit 1 }

# --- Find the fork point: the most recent commit common to HEAD and upstream/master.
$forkPoint = (& git merge-base HEAD $upstreamMaster).Trim()
if (-not $forkPoint) { Write-Error 'could not determine merge-base; histories are unrelated?'; exit 1 }

$forkShort   = (& git rev-parse --short $forkPoint).Trim()
$upstreamSha = (& git rev-parse --short $upstreamMaster).Trim()
$headSha     = (& git rev-parse --short HEAD).Trim()
$headRef     = (& git rev-parse --abbrev-ref HEAD).Trim()

Write-Host ''
Write-Host '=== Cortex pre-rebase audit ===' -ForegroundColor Cyan
Write-Host ("  Fork point (merge-base):  {0}" -f $forkShort)
Write-Host ("  Upstream HEAD:            {0}  @ {1}" -f $upstreamMaster, $upstreamSha)
Write-Host ("  Local HEAD:               {0}  @ {1}" -f $headRef, $headSha)
Write-Host ''

# --- Per-side change sets vs the fork point.
$upstreamChanges = @(& git diff --name-only "$forkPoint..$upstreamMaster")
$cortexChanges   = @(& git diff --name-only "$forkPoint..HEAD")

$upstreamSet = [System.Collections.Generic.HashSet[string]]::new()
$upstreamChanges | ForEach-Object { [void]$upstreamSet.Add($_) }
$cortexSet   = [System.Collections.Generic.HashSet[string]]::new()
$cortexChanges   | ForEach-Object { [void]$cortexSet.Add($_) }

$candidates = @($cortexChanges | Where-Object { $upstreamSet.Contains($_) })

Write-Host ("  Upstream-changed since fork: {0} file(s)" -f $upstreamSet.Count)
Write-Host ("  Cortex-changed since fork:   {0} file(s)" -f $cortexSet.Count)
Write-Host ("  Intersection (candidates):   {0} file(s)" -f $candidates.Count)
Write-Host ''

if ($candidates.Count -eq 0) {
    Write-Host 'No conflict candidates. Clean rebase looks likely.' -ForegroundColor Green
    exit 0
}

# --- Load the divergence registry (gitignored / Syncthing-synced).
$registryPath = Join-Path $repoRoot 'docs/divergence-registry.md'
$registry = @{}
if (Test-Path $registryPath) {
    $lines = Get-Content -Encoding UTF8 $registryPath
    $currentPath = $null
    foreach ($line in $lines) {
        # H3 headings either bare path or backtick-wrapped: "### `path/to/file`"
        $m = [regex]::Match($line, '^###\s+`?([^`]+?)`?\s*$')
        if ($m.Success) {
            $currentPath = $m.Groups[1].Value.Trim()
            continue
        }
        $r = [regex]::Match($line, '^\s*-\s*\*\*Resolution:\*\*\s*(.+?)\s*$')
        if ($r.Success -and $currentPath) {
            $registry[$currentPath] = $r.Groups[1].Value.Trim()
        }
    }
} else {
    Write-Host ("  (divergence registry not found at {0}; classifications will be UNREGISTERED)" -f $registryPath) -ForegroundColor Yellow
    Write-Host ''
}

# --- Build a row per candidate: path, classification, Cortex delta size,
#     upstream delta size.
$rows = foreach ($path in $candidates) {
    $cortexNumstat   = (& git diff --numstat "$forkPoint..HEAD"            -- $path) -split "`n" | Select-Object -First 1
    $upstreamNumstat = (& git diff --numstat "$forkPoint..$upstreamMaster" -- $path) -split "`n" | Select-Object -First 1
    function ParseNumstat($line) {
        if (-not $line) { return @{ Added = 0; Removed = 0 } }
        $parts = $line.Trim() -split '\s+', 3
        if ($parts.Count -lt 2) { return @{ Added = 0; Removed = 0 } }
        $added   = if ($parts[0] -eq '-') { 0 } else { [int]$parts[0] }
        $removed = if ($parts[1] -eq '-') { 0 } else { [int]$parts[1] }
        return @{ Added = $added; Removed = $removed }
    }
    $c = ParseNumstat $cortexNumstat
    $u = ParseNumstat $upstreamNumstat

    $classification = if ($registry.ContainsKey($path)) { $registry[$path] } else { 'UNREGISTERED' }

    [pscustomobject]@{
        Path           = $path
        Classification = $classification
        CortexAdd      = $c.Added
        CortexDel      = $c.Removed
        UpstreamAdd    = $u.Added
        UpstreamDel    = $u.Removed
        CortexDelta    = $c.Added + $c.Removed
    }
}

# --- Sort by Cortex delta size descending (largest divergence first).
$rows = $rows | Sort-Object -Property CortexDelta -Descending

Write-Host '--- Conflict candidates (sorted by Cortex delta size desc) ---' -ForegroundColor Cyan
Write-Host ''

$fmt = '{0,-60} {1,-28} {2,12} {3,12}'
Write-Host ($fmt -f 'Path', 'Classification', 'Cortex +/-', 'Upstream +/-')
Write-Host ($fmt -f ('-' * 60), ('-' * 28), ('-' * 12), ('-' * 12))
foreach ($r in $rows) {
    $cortexCol   = '+{0}/-{1}' -f $r.CortexAdd,   $r.CortexDel
    $upstreamCol = '+{0}/-{1}' -f $r.UpstreamAdd, $r.UpstreamDel
    $line = $fmt -f $r.Path, $r.Classification, $cortexCol, $upstreamCol
    if ($r.Classification -eq 'UNREGISTERED') {
        Write-Host $line -ForegroundColor Yellow
    } else {
        Write-Host $line
    }
}

$unregisteredCount = ($rows | Where-Object { $_.Classification -eq 'UNREGISTERED' } | Measure-Object).Count
Write-Host ''
if ($unregisteredCount -gt 0) {
    Write-Host ("  {0} UNREGISTERED divergence(s) above." -f $unregisteredCount) -ForegroundColor Yellow
    Write-Host '  Add entries to docs/divergence-registry.md before resolving the rebase.' -ForegroundColor Yellow
} else {
    Write-Host '  All candidates are registered. You have a pre-decided rule for each.' -ForegroundColor Green
}
Write-Host ''

# --- Check for Warp-internal workflows that the rebase may re-introduce.
$warpWorkflows = @(Get-ChildItem -Path '.github/workflows/*.yml' -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -notmatch '^(ci|cortex-)' } |
    Select-Object -ExpandProperty Name)
if ($warpWorkflows.Count -gt 0) {
    Write-Host '--- Warp-internal workflows detected (purge after rebase) ---' -ForegroundColor Yellow
    foreach ($wf in $warpWorkflows) {
        Write-Host ("  .github/workflows/{0}" -f $wf) -ForegroundColor Yellow
    }
    Write-Host '  These cause daily failure-notification emails. Delete them before pushing.' -ForegroundColor Yellow
    Write-Host '  See: docs/upstream-updates.md § Post-merge: purge re-introduced Warp workflows' -ForegroundColor Yellow
    Write-Host ''
}
