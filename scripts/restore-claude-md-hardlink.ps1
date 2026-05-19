#!/usr/bin/env pwsh
# Restore the CLAUDE.md <-> docs\CLAUDE.md hardlink (Windows / cross-platform).
#
# Background: CLAUDE.md is auto-loaded by Claude Code from the repo root,
# but the canonical file lives in docs/CLAUDE.md (gitignored, Syncthing'd
# across machines). The two are joined by an NTFS hardlink so an edit in
# either location updates both. Most editors -- including the Edit tool's
# atomic-write-temp-rename path -- break the link by writing a fresh
# inode at one of the two paths, leaving the two files diverged.
#
# This script idempotently re-establishes the hardlink. It picks the
# newer (mtime-most-recent) file as the truth source, deletes the other,
# and re-links from the truth to the deleted path. If only one exists,
# it creates the missing peer. If neither exists, it warns and exits 0.
#
# Pair: scripts/restore-claude-md-hardlink.sh on macOS/Linux. Keep the
# two in sync. The dev launchers invoke this at startup so a broken link
# never persists across two launches.

$ErrorActionPreference = 'Stop'

$repoRoot = (& git rev-parse --show-toplevel 2>$null)
if (-not $repoRoot) {
    Write-Error 'not in a git checkout'
    exit 1
}
$rootPath = Join-Path $repoRoot 'CLAUDE.md'
$docsPath = Join-Path $repoRoot 'docs\CLAUDE.md'

function Get-HardlinkSig([string]$path) {
    # NTFS doesn't expose POSIX inode numbers via Get-Item alone, but
    # `fsutil hardlink list` enumerates all hardlinks to the file. Two
    # paths are linked iff `fsutil hardlink list A` includes B (and
    # vice versa). Cheaper: BaseName + Length + LastWriteTime as a
    # quick "are these the same content" check, then confirm via
    # fsutil if they look identical.
    if (-not (Test-Path -LiteralPath $path)) { return $null }
    $info = Get-Item -LiteralPath $path
    return @{
        Length = $info.Length
        Mtime  = $info.LastWriteTimeUtc
        LinkTargets = (& fsutil hardlink list $path 2>$null) -join ';'
    }
}

function Is-Hardlinked([string]$a, [string]$b) {
    # `fsutil hardlink list` prints all paths sharing the inode.
    # Paths in its output are repo-relative to the file's volume root
    # ("\Users\Michael\cortex\CLAUDE.md"), so normalize for comparison.
    $aLinks = (& fsutil hardlink list $a 2>$null)
    if (-not $aLinks) { return $false }
    $bNorm = (Resolve-Path -LiteralPath $b).Path
    foreach ($line in $aLinks) {
        $line = $line.Trim()
        if (-not $line) { continue }
        # fsutil emits volume-relative; reconstruct absolute by prepending drive.
        $aDrive = (Get-Item -LiteralPath $a).PSDrive.Root  # "C:\"
        $absLink = Join-Path $aDrive ($line.TrimStart('\'))
        if ($absLink -ieq $bNorm) { return $true }
    }
    return $false
}

function Make-Hardlink([string]$existing, [string]$linkPath) {
    # `cmd /c mklink /H` is the lowest-friction way to create an NTFS
    # hardlink from PS 5.1 -- New-Item -ItemType HardLink exists but
    # has historically been finicky about long paths and elevation.
    & cmd /c "mklink /H `"$linkPath`" `"$existing`"" > $null
    if ($LASTEXITCODE -ne 0) {
        Write-Error "failed to hardlink $linkPath -> $existing"
        exit 1
    }
}

$rootExists = Test-Path -LiteralPath $rootPath
$docsExists = Test-Path -LiteralPath $docsPath

if (-not $rootExists -and -not $docsExists) {
    Write-Warning ('neither {0} nor {1} exists; nothing to restore.' -f $rootPath, $docsPath)
    exit 0
}

if ($rootExists -and -not $docsExists) {
    Write-Host '[restore-claude-md-hardlink] docs/CLAUDE.md missing; creating hardlink to root CLAUDE.md.' -ForegroundColor Yellow
    $docsDir = Split-Path -Parent $docsPath
    if (-not (Test-Path -LiteralPath $docsDir)) { New-Item -ItemType Directory -Path $docsDir | Out-Null }
    Make-Hardlink -existing $rootPath -linkPath $docsPath
    exit 0
}

if ($docsExists -and -not $rootExists) {
    Write-Host '[restore-claude-md-hardlink] root CLAUDE.md missing; creating hardlink from docs/CLAUDE.md.' -ForegroundColor Yellow
    Make-Hardlink -existing $docsPath -linkPath $rootPath
    exit 0
}

# Both exist: check if they're already linked.
if (Is-Hardlinked $rootPath $docsPath) {
    # Silent on the happy path so launcher startup isn't noisy.
    exit 0
}

# Both exist but are separate files. Pick the newer one as truth, replace the older.
$rootInfo = Get-Item -LiteralPath $rootPath
$docsInfo = Get-Item -LiteralPath $docsPath

if ($rootInfo.LastWriteTimeUtc -ge $docsInfo.LastWriteTimeUtc) {
    $truth = $rootPath
    $stale = $docsPath
    Write-Host '[restore-claude-md-hardlink] link broken; root CLAUDE.md is newer -> rebuilding docs/CLAUDE.md as hardlink.' -ForegroundColor Yellow
} else {
    $truth = $docsPath
    $stale = $rootPath
    Write-Host '[restore-claude-md-hardlink] link broken; docs/CLAUDE.md is newer -> rebuilding root CLAUDE.md as hardlink.' -ForegroundColor Yellow
}

Remove-Item -LiteralPath $stale -Force
Make-Hardlink -existing $truth -linkPath $stale
