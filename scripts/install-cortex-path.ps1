# Adds %LOCALAPPDATA%\Cortex\bin (the `cortex` console-shim directory) to the
# user PATH, idempotently. Called by scripts\install-cortex-prod.cmd after it
# copies the shims; safe to re-run standalone at any time.
#
# Edits HKCU\Environment\Path via the registry API rather than
# [Environment]::SetEnvironmentVariable so that:
#   - unexpanded %VAR% entries in an REG_EXPAND_SZ Path survive untouched
#     (SetEnvironmentVariable reads the *expanded* value and would flatten
#     them on write-back);
#   - the existing value kind (REG_SZ vs REG_EXPAND_SZ) is preserved.
# Broadcasts WM_SETTINGCHANGE afterwards so newly launched shells (and
# Explorer-spawned processes) pick the change up without a logoff. Already
# running shells keep their stale PATH — that includes panes inside a
# running Cortex, which inherit the env Cortex captured at its own launch.

$ErrorActionPreference = 'Stop'

$bin = Join-Path $env:LOCALAPPDATA 'Cortex\bin'

$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
try {
    $current = [string]$key.GetValue(
        'Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)

    $entries = $current -split ';' | Where-Object { $_ -ne '' }
    if ($entries -contains $bin) {
        Write-Host "User PATH already contains $bin"
        exit 0
    }

    $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
    if ($current -ne '') {
        try { $kind = $key.GetValueKind('Path') } catch {}
    }

    $updated = if ($current -eq '') { $bin } else { $current.TrimEnd(';') + ';' + $bin }
    $key.SetValue('Path', $updated, $kind)
    Write-Host "Added $bin to user PATH"
} finally {
    $key.Close()
}

# Tell running top-level windows the environment changed (same broadcast
# the System control panel sends). SMTO_ABORTIFHUNG = 0x2, WM_SETTINGCHANGE
# = 0x1A, HWND_BROADCAST = 0xffff.
$signature = @'
[DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
public static extern IntPtr SendMessageTimeout(
    IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam,
    uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
'@
$broadcaster = Add-Type -MemberDefinition $signature -Name 'CortexPathBroadcast' `
    -Namespace 'CortexInstall' -PassThru
$result = [UIntPtr]::Zero
[void]$broadcaster::SendMessageTimeout(
    [IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 0x2, 5000, [ref]$result)
