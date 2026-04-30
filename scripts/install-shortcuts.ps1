# Cortex shortcut installer (Windows). Creates two .lnk files on the Desktop
# and in the Start Menu, both with the cmd-wrapper target pattern so they
# can be pinned to the taskbar (raw .bat targets aren't pinnable; cmd.exe
# is, and `cmd /c <script>` survives the pin).
#
#   Cortex.lnk       — daily-driver prod, custom Cortex.ico
#   Cortex Dev.lnk   — live-rebuild dev loop, Cortex-Dev.ico (with "DEV"
#                       overlay text in pink #F000D0 with dark purple
#                       #200040 outline, sampled from the master icon)
#
# Idempotent — re-running just refreshes the targets and icons. Run once
# after install-cortex-prod.cmd completes (so prod EXE exists), or run
# anytime to re-point shortcuts after moving things.
#
# If the icons haven't been generated yet, this script invokes
# build-shortcut-icons.py first.

$ErrorActionPreference = 'Stop'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$InstallDir = Join-Path $env:LOCALAPPDATA 'Cortex'
$ProdIco = Join-Path $InstallDir 'Cortex.ico'
$DevIco = Join-Path $InstallDir 'Cortex-Dev.ico'
$ProdScript = Join-Path $RepoRoot 'scripts\launch-cortex.cmd'
$DevScript = Join-Path $RepoRoot 'scripts\launch-cortex-dev.bat'

# --- Generate icons if missing -------------------------------------------
if (-not (Test-Path $ProdIco) -or -not (Test-Path $DevIco)) {
    Write-Host "Icons missing - running build-shortcut-icons.py..."
    $py = Join-Path $RepoRoot 'scripts\build-shortcut-icons.py'
    & python $py
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Icon generation failed."
        exit 1
    }
}

# --- Validate prerequisites ----------------------------------------------
if (-not (Test-Path $ProdScript)) {
    Write-Error "Missing $ProdScript - did you intend to run install-cortex-prod.cmd first?"
    exit 1
}
if (-not (Test-Path $DevScript)) {
    Write-Error "Missing $DevScript"
    exit 1
}

# --- Helper: create a .lnk via WScript.Shell COM -------------------------
function New-Shortcut {
    param(
        [string]$Path,
        [string]$Target,
        [string]$Arguments,
        [string]$IconLocation,
        [string]$Description,
        [string]$WorkingDirectory
    )
    $ws = New-Object -ComObject WScript.Shell
    $sc = $ws.CreateShortcut($Path)
    $sc.TargetPath = $Target
    $sc.Arguments = $Arguments
    $sc.IconLocation = $IconLocation
    $sc.Description = $Description
    $sc.WorkingDirectory = $WorkingDirectory
    # Window style 7 = "minimized" so the cmd wrapper window doesn't grab focus
    # for prod (which detaches the EXE immediately via `start ""` and exits).
    # Dev keeps the default 1 (normal) so the build output is visible.
    $sc.WindowStyle = 1
    $sc.Save()
}

# --- Where to drop shortcuts ---------------------------------------------
$Desktop = [Environment]::GetFolderPath('Desktop')
$StartMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'

$cmdExe = "$env:WINDIR\System32\cmd.exe"

$prodArgs = "/c `"$ProdScript`""
$devArgs = "/c `"$DevScript`""

$prodDesc = 'Cortex Terminal (daily driver)'
$devDesc = 'Cortex Terminal - dev (live rebuild + launch)'

foreach ($Dir in @($Desktop, $StartMenu)) {
    Write-Host "Writing shortcuts to: $Dir"

    New-Shortcut `
        -Path (Join-Path $Dir 'Cortex.lnk') `
        -Target $cmdExe `
        -Arguments $prodArgs `
        -IconLocation "$ProdIco,0" `
        -Description $prodDesc `
        -WorkingDirectory $env:USERPROFILE

    New-Shortcut `
        -Path (Join-Path $Dir 'Cortex Dev.lnk') `
        -Target $cmdExe `
        -Arguments $devArgs `
        -IconLocation "$DevIco,0" `
        -Description $devDesc `
        -WorkingDirectory $RepoRoot
}

Write-Host ""
Write-Host "Done. To pin to taskbar:"
Write-Host "  1. Right-click 'Cortex' on Desktop -> 'Show more options' -> 'Pin to taskbar'"
Write-Host "  2. Same for 'Cortex Dev'"
Write-Host ""
Write-Host "Targets:"
Write-Host "  Cortex.lnk      -> cmd /c $ProdScript     (icon: Cortex.ico)"
Write-Host "  Cortex Dev.lnk  -> cmd /c $DevScript      (icon: Cortex-Dev.ico)"
