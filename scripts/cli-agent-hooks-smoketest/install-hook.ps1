#requires -Version 5.1
<#
Adds the Phase 0 smoke-test hook entry to ~/.claude/settings.json.

Idempotent — repeated runs are no-ops. The entry is marked with
_cortex_smoketest=true so uninstall-hook.ps1 can find and remove it
without touching the user's other hooks (e.g. SideQuest's claude-status.sh
hooks already in this user's config).

Wired to the Stop event because that's the most byte-capture-prone
lifecycle event per the investigation doc — if our write technique works
there, it works for everything else.
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$repoRoot     = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
# $PSScriptRoot is .../scripts/cli-agent-hooks-smoketest, so up two = repo root.
# But to be robust, locate run-smoketest.ps1 next to *this* script:
$smoketestPs1 = Join-Path $PSScriptRoot 'run-smoketest.ps1'
if (-not (Test-Path $smoketestPs1)) {
    throw "run-smoketest.ps1 not found at $smoketestPs1"
}

$settingsPath = Join-Path $env:USERPROFILE '.claude\settings.json'
if (-not (Test-Path $settingsPath)) {
    throw "settings.json not found at $settingsPath; run claude at least once first"
}

$raw = Get-Content -Path $settingsPath -Raw -Encoding utf8
$json = $raw | ConvertFrom-Json

if (-not $json.hooks) {
    $json | Add-Member -MemberType NoteProperty -Name hooks -Value (New-Object psobject)
}
if (-not $json.hooks.Stop) {
    $json.hooks | Add-Member -MemberType NoteProperty -Name Stop -Value @()
}

# settings.json is loaded by ConvertFrom-Json as PSCustomObjects; arrays of
# objects come through fine. Find any existing _cortex_smoketest entry; if
# present, we update its command in place. Otherwise append a new one.
$existing = @($json.hooks.Stop | Where-Object { $_._cortex_smoketest -eq $true })

$command = ('powershell -NoProfile -ExecutionPolicy Bypass -File "{0}"' -f $smoketestPs1)
$entry = [pscustomobject]@{
    _cortex_smoketest = $true
    hooks = @(
        [pscustomobject]@{
            type    = 'command'
            command = $command
        }
    )
}

if ($existing.Count -gt 0) {
    # Replace the existing entry's command (keeps array order).
    $stopArray = @($json.hooks.Stop)
    for ($i = 0; $i -lt $stopArray.Count; $i++) {
        if ($stopArray[$i]._cortex_smoketest -eq $true) {
            $stopArray[$i] = $entry
        }
    }
    $json.hooks.Stop = $stopArray
    Write-Host "Updated existing smoketest hook entry."
} else {
    $stopArray = @($json.hooks.Stop) + @($entry)
    $json.hooks.Stop = $stopArray
    Write-Host "Added smoketest hook entry."
}

$out = $json | ConvertTo-Json -Depth 32

$tmp = "$settingsPath.tmp"
[System.IO.File]::WriteAllText($tmp, $out, [System.Text.UTF8Encoding]::new($false))
Move-Item -Force -Path $tmp -Destination $settingsPath

Write-Host "Smoketest hook installed."
Write-Host "  settings.json : $settingsPath"
Write-Host "  hook command  : $command"
Write-Host "  output log    : $env:USERPROFILE\.claude\cortex-smoketest.log"
Write-Host ""
Write-Host "Next: run claude in Cortex (prod or dev), submit any prompt, wait for"
Write-Host "the response. The window title should cycle through CortexHookTest-T1..T4"
Write-Host "with a 1.5s delay between each. Read the log for definitive PASS/FAIL."
