#requires -Version 5.1
<#
Removes the Phase 0 smoke-test hook entry from ~/.claude/settings.json.
Identified by the _cortex_smoketest=true marker, so unrelated hooks are
left intact.
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$settingsPath = Join-Path $env:USERPROFILE '.claude\settings.json'
if (-not (Test-Path $settingsPath)) {
    Write-Host "No settings.json at $settingsPath — nothing to do."
    return
}

$raw = Get-Content -Path $settingsPath -Raw -Encoding utf8
$json = $raw | ConvertFrom-Json

if (-not $json.hooks -or -not $json.hooks.Stop) {
    Write-Host "No Stop hooks in settings — nothing to do."
    return
}

$before = @($json.hooks.Stop).Count
$filtered = @($json.hooks.Stop | Where-Object { $_._cortex_smoketest -ne $true })
$after = $filtered.Count

if ($before -eq $after) {
    Write-Host "No smoketest hook entry found — nothing to do."
    return
}

$json.hooks.Stop = $filtered

$out = $json | ConvertTo-Json -Depth 32

$tmp = "$settingsPath.tmp"
[System.IO.File]::WriteAllText($tmp, $out, [System.Text.UTF8Encoding]::new($false))
Move-Item -Force -Path $tmp -Destination $settingsPath

Write-Host ("Removed {0} smoketest entry." -f ($before - $after))
