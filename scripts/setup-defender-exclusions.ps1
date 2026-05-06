# Cortex Defender exclusions helper (Windows).
#
# Real-time scanning of the cargo target dir and crate registry is one of
# the largest hidden costs of building Rust on Windows — Defender opens
# every .rlib, .pdb, and intermediate object as cargo writes it. Excluding
# those paths typically claws back 20-40% on cold builds. This script
# adds the recommended path + process exclusions for Cortex.
#
# Usage (one-time, idempotent):
#   1. Right-click PowerShell → "Run as administrator"
#   2. cd <your cortex repo root>
#   3. powershell -NoProfile -ExecutionPolicy Bypass -File scripts\setup-defender-exclusions.ps1
#
# Re-run any time. Listing/adding exclusions both require admin.

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$IsElevated = ([Security.Principal.WindowsPrincipal](
    [Security.Principal.WindowsIdentity]::GetCurrent())).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)

# Resolve repo target/ relative to this script so the exclusion stays valid
# even if the user runs the script from a different cwd.
$RepoRoot   = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$RepoTarget = Join-Path $RepoRoot 'target'
$CargoHome  = if ($env:CARGO_HOME)  { $env:CARGO_HOME }  else { Join-Path $env:USERPROFILE '.cargo' }
$RustupHome = if ($env:RUSTUP_HOME) { $env:RUSTUP_HOME } else { Join-Path $env:USERPROFILE '.rustup' }

# Path exclusions: where rustc/cargo write build artifacts and where the
# crate cache lives. Both get scanned heavily on a cold build.
$Paths = @(
    $RepoTarget,
    (Join-Path $CargoHome 'registry'),
    (Join-Path $CargoHome 'git'),
    $RustupHome
) | Where-Object { $_ -and (Test-Path $_) -or $_ -eq $RepoTarget }

# Process exclusions: aggressive but very effective during clean builds,
# since each compiled crate spawns a fresh rustc.exe and the linker is
# invoked once at the end. Skipping AV on these processes specifically
# avoids paying the scan cost for transient temp files they emit, while
# keeping AV active for everything else.
$Procs = @('rustc.exe', 'cargo.exe', 'link.exe', 'lld-link.exe')

if (-not $IsElevated) {
    Write-Host 'Defender exclusion management requires admin.' -ForegroundColor Yellow
    Write-Host ''
    Write-Host 'Would add these path exclusions:'
    $Paths | ForEach-Object { Write-Host "  $_" }
    Write-Host ''
    Write-Host 'And these process exclusions:'
    $Procs | ForEach-Object { Write-Host "  $_" }
    Write-Host ''
    Write-Host 'Re-run from an elevated PowerShell:'
    Write-Host "  cd $RepoRoot"
    Write-Host '  powershell -NoProfile -ExecutionPolicy Bypass -File scripts\setup-defender-exclusions.ps1'
    exit 1
}

$current = Get-MpPreference

Write-Host 'Current ExclusionPath:' -ForegroundColor Cyan
if ($current.ExclusionPath) { ($current.ExclusionPath | Out-String).TrimEnd() } else { '  (none)' }
Write-Host ''
Write-Host 'Current ExclusionProcess:' -ForegroundColor Cyan
if ($current.ExclusionProcess) { ($current.ExclusionProcess | Out-String).TrimEnd() } else { '  (none)' }
Write-Host ''
Write-Host '--- Applying ---' -ForegroundColor Cyan

$added = $false

foreach ($p in $Paths) {
    if ($current.ExclusionPath -and ($current.ExclusionPath -contains $p)) {
        Write-Host "Already excluded:    $p" -ForegroundColor DarkGray
    } else {
        Add-MpPreference -ExclusionPath $p
        Write-Host "Added path:          $p" -ForegroundColor Green
        $added = $true
    }
}

foreach ($p in $Procs) {
    if ($current.ExclusionProcess -and ($current.ExclusionProcess -contains $p)) {
        Write-Host "Already excluded:    $p" -ForegroundColor DarkGray
    } else {
        Add-MpPreference -ExclusionProcess $p
        Write-Host "Added process:       $p" -ForegroundColor Green
        $added = $true
    }
}

Write-Host ''
if ($added) {
    Write-Host 'Done. Exclusions take effect immediately.' -ForegroundColor Green
    Write-Host 'Verify:  Get-MpPreference | Select-Object ExclusionPath, ExclusionProcess'
} else {
    Write-Host 'Nothing to add — all recommended exclusions already present.' -ForegroundColor Green
}
