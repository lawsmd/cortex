#requires -Version 5.1
<#
Phase 0 Windows TTY-bypass smoke test for the Cortex claude-hook bridge.

Invoked as a claude `Stop` hook. Tries four techniques in sequence to write
an OSC 0 (window-title) sequence back to the controlling terminal — the
title flip is the visible signal that bytes reached Cortex's ANSI parser.

Each technique sets a uniquely-numbered title (T1..T4) with a 1500ms pause
between attempts, so the user can watch the title progression in real time.
The final visible title is whichever technique succeeded last; the log file
captures definitive PASS/FAIL per technique with exception detail.

Output log: $env:USERPROFILE\.claude\cortex-smoketest.log

This script is dev-only scaffolding. Production hook lives at
app/assets/cli-agent-hooks/claude/cortex-hook.ps1 (Phase 2).
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Continue'

$logPath = Join-Path $env:USERPROFILE '.claude\cortex-smoketest.log'

function Write-SmoketestLog {
    param([string]$Message)
    $line = "[{0}] {1}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'), $Message
    Add-Content -Path $logPath -Value $line -Encoding utf8
}

# Drain claude's stdin so we don't hang the hook. We don't use the JSON for
# the smoke test, but reading it keeps claude's IPC happy.
try {
    $stdinJson = [Console]::In.ReadToEnd()
    Write-SmoketestLog ("=== smoketest run begin (stdin len={0}) ===" -f $stdinJson.Length)
} catch {
    Write-SmoketestLog ("=== smoketest run begin (stdin read failed: {0}) ===" -f $_.Exception.Message)
}

Write-SmoketestLog ("env WARP_CLI_AGENT_PROTOCOL_VERSION={0}" -f $env:WARP_CLI_AGENT_PROTOCOL_VERSION)
Write-SmoketestLog ("env TERM_PROGRAM={0}" -f $env:TERM_PROGRAM)
Write-SmoketestLog ("env WARP_IS_LOCAL_SHELL_SESSION={0}" -f $env:WARP_IS_LOCAL_SHELL_SESSION)

function Get-OscTitleBytes {
    param([int]$Technique)
    $title = "CortexHookTest-T$Technique"
    $bytes = [byte[]]@(0x1B, 0x5D, 0x30, 0x3B) +
             [System.Text.Encoding]::ASCII.GetBytes($title) +
             [byte[]]@(0x07)
    return $bytes
}

# T1 — cmd /c redirect to CON via temp binary
function Invoke-T1-CmdConRedirect {
    $bytes = Get-OscTitleBytes 1
    $tmp = [System.IO.Path]::GetTempFileName()
    try {
        [System.IO.File]::WriteAllBytes($tmp, $bytes)
        $cmd = 'type "{0}" > CON' -f $tmp
        $output = & cmd /c $cmd 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw "cmd /c exited with $LASTEXITCODE; output=$output"
        }
        Write-SmoketestLog "T1 cmd /c CON: PASS"
    } catch {
        Write-SmoketestLog ("T1 cmd /c CON: FAIL ({0})" -f $_.Exception.Message)
    } finally {
        Remove-Item $tmp -ErrorAction SilentlyContinue
    }
}

# T2 — PowerShell open CONOUT$ via FileStream
function Invoke-T2-FileStreamConout {
    $bytes = Get-OscTitleBytes 2
    try {
        $stream = [System.IO.FileStream]::new('CONOUT$', [System.IO.FileMode]::Open, [System.IO.FileAccess]::Write, [System.IO.FileShare]::ReadWrite)
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush()
        $stream.Dispose()
        Write-SmoketestLog "T2 FileStream CONOUT`$: PASS"
    } catch {
        Write-SmoketestLog ("T2 FileStream CONOUT`$: FAIL ({0})" -f $_.Exception.Message)
    }
}

# T3 — [Console]::Out.Write (likely captured by claude, but worth checking)
function Invoke-T3-ConsoleOut {
    $bytes = Get-OscTitleBytes 3
    try {
        $s = [System.Text.Encoding]::ASCII.GetString($bytes)
        [Console]::Out.Write($s)
        [Console]::Out.Flush()
        Write-SmoketestLog "T3 [Console]::Out.Write: PASS (write returned without exception; visibility unverified)"
    } catch {
        Write-SmoketestLog ("T3 [Console]::Out.Write: FAIL ({0})" -f $_.Exception.Message)
    }
}

# T4 — P/Invoke kernel32!CreateFileW + WriteFile against CONOUT$
function Invoke-T4-PinvokeConout {
    $bytes = Get-OscTitleBytes 4
    try {
        if (-not ('Cortex.Smoketest.Native' -as [type])) {
            Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace Cortex.Smoketest {
    public static class Native {
        [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
        public static extern IntPtr CreateFileW(
            string lpFileName, uint dwDesiredAccess, uint dwShareMode,
            IntPtr lpSecurityAttributes, uint dwCreationDisposition,
            uint dwFlagsAndAttributes, IntPtr hTemplateFile);
        [DllImport("kernel32.dll", SetLastError=true)]
        public static extern bool WriteFile(
            IntPtr hFile, byte[] lpBuffer, uint nNumberOfBytesToWrite,
            out uint lpNumberOfBytesWritten, IntPtr lpOverlapped);
        [DllImport("kernel32.dll", SetLastError=true)]
        public static extern bool CloseHandle(IntPtr hObject);
    }
}
'@
        }
        $GENERIC_WRITE = [uint32]0x40000000
        $FILE_SHARE_RW = [uint32]0x00000003
        $OPEN_EXISTING = [uint32]3
        $invalid = [IntPtr]::new(-1)
        $h = [Cortex.Smoketest.Native]::CreateFileW('CONOUT$', $GENERIC_WRITE, $FILE_SHARE_RW, [IntPtr]::Zero, $OPEN_EXISTING, 0, [IntPtr]::Zero)
        if ($h -eq $invalid) {
            $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
            throw "CreateFileW(CONOUT`$) failed; lasterror=$err"
        }
        try {
            [uint32]$written = 0
            $ok = [Cortex.Smoketest.Native]::WriteFile($h, $bytes, [uint32]$bytes.Length, [ref]$written, [IntPtr]::Zero)
            if (-not $ok) {
                $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
                throw "WriteFile failed; lasterror=$err"
            }
            Write-SmoketestLog ("T4 P/Invoke CONOUT`$: PASS (wrote {0} bytes)" -f $written)
        } finally {
            [void][Cortex.Smoketest.Native]::CloseHandle($h)
        }
    } catch {
        Write-SmoketestLog ("T4 P/Invoke CONOUT`$: FAIL ({0})" -f $_.Exception.Message)
    }
}

# T5 — FreeConsole + walk to shell PID + AttachConsole + WriteFile to CONOUT$
# This is the technique that *should* work given the diagnostic finding that
# T1/T3/T4 all PASS the write but Cortex's parser never sees the bytes:
# claude spawns hook children with CREATE_NEW_CONSOLE, so CONOUT$ in the
# child opens its OWN private console buffer, not Cortex's ConPTY. Detaching
# from that and reattaching to the shell process (whose console IS Cortex's
# ConPTY) puts our subsequent CONOUT$ writes into Cortex's byte stream.
function Invoke-T5-AttachToShell {
    $bytes = Get-OscTitleBytes 5
    try {
        if (-not ('Cortex.Smoketest.NativeAttach' -as [type])) {
            Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace Cortex.Smoketest {
    public static class NativeAttach {
        [DllImport("kernel32.dll", SetLastError=true)]
        public static extern bool AttachConsole(uint dwProcessId);
        [DllImport("kernel32.dll", SetLastError=true)]
        public static extern bool FreeConsole();
        [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Unicode)]
        public static extern IntPtr CreateFileW(
            string lpFileName, uint dwDesiredAccess, uint dwShareMode,
            IntPtr lpSecurityAttributes, uint dwCreationDisposition,
            uint dwFlagsAndAttributes, IntPtr hTemplateFile);
        [DllImport("kernel32.dll", SetLastError=true)]
        public static extern bool WriteFile(
            IntPtr hFile, byte[] lpBuffer, uint nNumberOfBytesToWrite,
            out uint lpNumberOfBytesWritten, IntPtr lpOverlapped);
        [DllImport("kernel32.dll", SetLastError=true)]
        public static extern bool CloseHandle(IntPtr hObject);
    }
}
'@
        }

        # Walk the FULL ancestor chain up to warp-oss.exe (or until we run
        # out). Previous attempt stopped at the first shell, but on this
        # user's Windows setup claude invokes hooks via Git Bash (so the
        # immediate parent bash.exe is the short-lived hook-execution
        # shell with its own private console — not the pane shell).
        # The pane shell is the shell-named ancestor closest to warp-oss.exe.
        $cur = [uint32]$PID
        $chain = @()
        for ($i = 0; $i -lt 16; $i++) {
            $proc = Get-WmiObject Win32_Process -Filter "ProcessId = $cur" -ErrorAction SilentlyContinue
            if (-not $proc) { break }
            $chain += [pscustomobject]@{ Pid = [uint32]$cur; Name = $proc.Name }
            if ($proc.Name -match '^warp-oss\.exe$') { break }
            if ($proc.ParentProcessId -eq 0 -or $proc.ParentProcessId -eq $cur) { break }
            $cur = [uint32]$proc.ParentProcessId
        }
        $traceStr = ($chain | ForEach-Object { "$($_.Pid)=$($_.Name)" }) -join ' -> '
        Write-SmoketestLog ("T5 full chain: $traceStr")

        # Pick the pane-shell candidate: the shell-named ancestor closest
        # to warp-oss.exe. If warp-oss.exe is in the chain, walk back from
        # it and find the first shell-named entry. Otherwise fall back to
        # the last shell-named entry in the chain.
        $shellPattern = '^(powershell|pwsh|cmd|bash|fish|zsh|sh)\.exe$'
        $shellPid = $null
        $warpIdx = -1
        for ($i = 0; $i -lt $chain.Count; $i++) {
            if ($chain[$i].Name -match '^warp-oss\.exe$') { $warpIdx = $i; break }
        }
        if ($warpIdx -ge 0) {
            # Walk back from warp-oss toward $PID, take first shell.
            for ($j = $warpIdx - 1; $j -ge 0; $j--) {
                if ($chain[$j].Name -match $shellPattern) {
                    $shellPid = $chain[$j].Pid
                    Write-SmoketestLog ("T5 picked pane shell at chain idx $j (closest shell to warp-oss): $shellPid=$($chain[$j].Name)")
                    break
                }
            }
        }
        if (-not $shellPid) {
            # Fallback: warp-oss not in chain (truncated walk?). Take the
            # LAST shell-named entry, which is the deepest ancestor we found
            # that's a shell — most likely to be the pane shell rather than
            # a hook-spawn shell.
            for ($j = $chain.Count - 1; $j -ge 0; $j--) {
                if ($chain[$j].Name -match $shellPattern) {
                    $shellPid = $chain[$j].Pid
                    Write-SmoketestLog ("T5 picked deepest shell (no warp-oss in chain): $shellPid=$($chain[$j].Name)")
                    break
                }
            }
        }
        if (-not $shellPid) {
            Write-SmoketestLog "T5 AttachConsole: FAIL (no shell-named ancestor in chain)"
            return
        }

        # Detach from our own console (created by claude with NEW_CONSOLE),
        # then attach to the shell's. FreeConsole returns false when there's
        # no console to free; that's fine.
        [void][Cortex.Smoketest.NativeAttach]::FreeConsole()
        $attached = [Cortex.Smoketest.NativeAttach]::AttachConsole($shellPid)
        if (-not $attached) {
            $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
            Write-SmoketestLog ("T5 AttachConsole({0}) FAIL: lasterror={1}" -f $shellPid, $err)
            return
        }

        try {
            $GENERIC_WRITE = [uint32]0x40000000
            $FILE_SHARE_RW = [uint32]0x00000003
            $OPEN_EXISTING = [uint32]3
            $invalid = [IntPtr]::new(-1)
            $h = [Cortex.Smoketest.NativeAttach]::CreateFileW('CONOUT$', $GENERIC_WRITE, $FILE_SHARE_RW, [IntPtr]::Zero, $OPEN_EXISTING, 0, [IntPtr]::Zero)
            if ($h -eq $invalid) {
                $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
                Write-SmoketestLog ("T5 CreateFileW(CONOUT`$) after AttachConsole({0}) FAIL: lasterror={1}" -f $shellPid, $err)
                return
            }
            try {
                # Write the title OSC.
                [uint32]$written = 0
                $ok = [Cortex.Smoketest.NativeAttach]::WriteFile($h, $bytes, [uint32]$bytes.Length, [ref]$written, [IntPtr]::Zero)
                if ($ok -and $written -gt 0) {
                    Write-SmoketestLog ("T5 title-OSC: PASS (wrote {0} bytes to PID {1})" -f $written, $shellPid)
                } else {
                    $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
                    Write-SmoketestLog ("T5 title-OSC WriteFile FAIL: lasterror={0}" -f $err)
                }

                # Also write an OSC 777 with a uniquely-marked session_id.
                # OSC 0 (title) competes with claude's own title spam and may
                # be overwritten before we can observe it. OSC 777 doesn't.
                # Cortex's parser logs every received OSC 777 at INFO; we can
                # grep the runtime log for the marker to confirm receipt.
                $marker = "smoketest-t5-{0:HHmmss}" -f (Get-Date)
                $jsonBody = ('{{"v":1,"agent":"claude","event":"prompt_submit","session_id":"{0}","payload":{{"query":"smoketest visibility probe"}}}}' -f $marker)
                $oscPrefix = [byte[]]@(0x1B, 0x5D) + [System.Text.Encoding]::ASCII.GetBytes('777;notify;warp://cli-agent;')
                $oscBody = [System.Text.Encoding]::UTF8.GetBytes($jsonBody)
                $oscSuffix = [byte[]]@(0x07)
                $oscBytes = $oscPrefix + $oscBody + $oscSuffix
                [uint32]$written2 = 0
                $ok2 = [Cortex.Smoketest.NativeAttach]::WriteFile($h, $oscBytes, [uint32]$oscBytes.Length, [ref]$written2, [IntPtr]::Zero)
                if ($ok2 -and $written2 -gt 0) {
                    Write-SmoketestLog ("T5 OSC-777: PASS (wrote {0} bytes; marker={1})" -f $written2, $marker)
                } else {
                    $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
                    Write-SmoketestLog ("T5 OSC-777 WriteFile FAIL: lasterror={0}" -f $err)
                }
            } finally {
                [void][Cortex.Smoketest.NativeAttach]::CloseHandle($h)
            }
        } finally {
            [void][Cortex.Smoketest.NativeAttach]::FreeConsole()
        }
    } catch {
        Write-SmoketestLog ("T5 AttachConsole: FAIL ({0})" -f $_.Exception.Message)
    }
}

# T5 runs FIRST so its title flip (if it works) survives as the visible
# final title — T1..T4 writes in the post-T5 process state go to a fresh
# console (because T5's FreeConsole detached us, and cmd /c spawns a new
# console anyway), so they can't overwrite T5's title in Cortex's view.
$techniques = @(
    @{ Name = 'T5 AttachShell';     Action = ${function:Invoke-T5-AttachToShell} },
    @{ Name = 'T1 cmd /c CON';      Action = ${function:Invoke-T1-CmdConRedirect} },
    @{ Name = 'T2 FileStream';      Action = ${function:Invoke-T2-FileStreamConout} },
    @{ Name = 'T3 Console.Out';     Action = ${function:Invoke-T3-ConsoleOut} },
    @{ Name = 'T4 P/Invoke CONOUT'; Action = ${function:Invoke-T4-PinvokeConout} }
)

foreach ($t in $techniques) {
    Write-SmoketestLog ("--> attempting {0}" -f $t.Name)
    & $t.Action
    Start-Sleep -Milliseconds 1500
}

Write-SmoketestLog "=== smoketest run end ==="
exit 0
