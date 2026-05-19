#!/usr/bin/env pwsh
# Cortex post-merge verification harness (Windows / cross-platform PowerShell).
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
#     - Skippable with -NoBuild for fast iteration on static checks.
#
# Exit code 0 = all green. Non-zero = first failure printed in red and
# the rest of the report still emitted so you can see the full picture.
#
# Pair: scripts/post-merge-verify.sh on macOS/Linux. Keep the two in sync.

param(
    [switch]$NoBuild
)

$ErrorActionPreference = 'Continue'

$repoRoot = (& git rev-parse --show-toplevel 2>$null)
if (-not $repoRoot) {
    Write-Error 'not in a git checkout'
    exit 1
}
Set-Location $repoRoot

$failures = New-Object System.Collections.ArrayList

function Check([string]$name, [scriptblock]$body) {
    Write-Host -NoNewline ("  - {0,-48}" -f $name)
    try {
        $detail = & $body
        if ($detail) {
            Write-Host ("ok ({0})" -f $detail) -ForegroundColor Green
        } else {
            Write-Host 'ok' -ForegroundColor Green
        }
    } catch {
        Write-Host ('FAIL: {0}' -f $_.Exception.Message) -ForegroundColor Red
        [void]$failures.Add($name)
    }
}

Write-Host ''
Write-Host '=== Cortex post-merge verification ===' -ForegroundColor Cyan
Write-Host ''
Write-Host 'Phase 1: static sanity checks' -ForegroundColor Cyan

Check 'theme yaml count >= 1000' {
    $count = (Get-ChildItem -Path 'app/src/themes/wezterm_bundle/yaml' -Filter '*.yaml' -ErrorAction Stop).Count
    if ($count -lt 1000) {
        throw "only $count theme yamls; expected >= 1000 (baseline ~1,078)"
    }
    "$count yamls"
}

Check 'CortexSettings module present' {
    $path = 'app/src/settings/cortex.rs'
    if (-not (Test-Path $path)) { throw "$path missing" }
    $content = Get-Content $path -Raw -ErrorAction Stop
    if ($content -notmatch 'pub\s+enum\s+TabsSelectedTitleAlignment') {
        throw 'TabsSelectedTitleAlignment enum missing from cortex.rs'
    }
    'cortex.rs + TabsSelectedTitleAlignment'
}

Check 'cortex_settings/ pane module present' {
    if (-not (Test-Path 'app/src/cortex_settings')) {
        throw 'app/src/cortex_settings/ directory missing'
    }
    if (-not (Test-Path 'app/src/cortex_settings/brand.rs')) {
        throw 'app/src/cortex_settings/brand.rs missing'
    }
    'brand.rs present'
}

Check 'divergence registry present' {
    if (-not (Test-Path 'docs/divergence-registry.md')) {
        throw 'docs/divergence-registry.md missing (Syncthing not running?)'
    }
    $size = (Get-Item 'docs/divergence-registry.md').Length
    "$size bytes"
}

Check 'CLAUDE.md hardlink intact' {
    $rootLinks = (& fsutil hardlink list 'CLAUDE.md' 2>$null) -join ';'
    if (-not $rootLinks) {
        throw "fsutil hardlink list 'CLAUDE.md' returned nothing"
    }
    if ($rootLinks -notmatch '\\docs\\CLAUDE\.md') {
        throw "root CLAUDE.md is not hardlinked to docs/CLAUDE.md -- run scripts/restore-claude-md-hardlink.ps1"
    }
    'shares inode'
}

Check 'pre-rebase audit script present' {
    if (-not (Test-Path 'scripts/pre-rebase-audit.ps1')) {
        throw 'scripts/pre-rebase-audit.ps1 missing'
    }
    'present'
}

if ($NoBuild) {
    Write-Host ''
    Write-Host 'Phase 2 skipped (-NoBuild).' -ForegroundColor DarkGray
} else {
    Write-Host ''
    Write-Host 'Phase 2: cargo check (slow)' -ForegroundColor Cyan
    Write-Host '  Running: cargo check --bin warp-oss --features gui,skip_login'
    Write-Host '  (use -NoBuild to skip)'
    Write-Host ''
    $env:CARGO_TERM_COLOR = 'always'
    & cargo check --bin warp-oss --features gui,skip_login
    if ($LASTEXITCODE -ne 0) {
        Write-Host ''
        Write-Host '  FAIL: cargo check exited with code' $LASTEXITCODE -ForegroundColor Red
        [void]$failures.Add('cargo check')
    } else {
        Write-Host ''
        Write-Host '  cargo check: ok' -ForegroundColor Green
    }
}

Write-Host ''
if ($failures.Count -eq 0) {
    Write-Host '=== All checks passed ===' -ForegroundColor Green
    exit 0
} else {
    Write-Host ('=== {0} failure(s): {1} ===' -f $failures.Count, ($failures -join ', ')) -ForegroundColor Red
    exit 1
}
