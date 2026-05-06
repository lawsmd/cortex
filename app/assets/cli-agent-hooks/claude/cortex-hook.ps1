#requires -Version 5.1
<#
Cortex bridge hook for vanilla `clauded` (claude --dangerously-skip-permissions).

Translates claude's first-class hook lifecycle events into the OSC 777 wire
format that Cortex's CLIAgentSessionsModel already understands. Lets claude
sessions without the warp@claude-code-warp plugin participate in the rich-
status pipeline (tab animation, badges, notifications).

Schema reference: app/src/terminal/cli_agent_sessions/event/mod.rs +
                  app/src/terminal/cli_agent_sessions/event/v1.rs

Wired into ~/.claude/settings.json by Cortex on first claude detection.
The first positional arg is the cortex-side event name (NOT the claude
event name) — Notification is split upstream by `matcher` into two
distinct entries so this script doesn't have to discriminate subtypes:
    powershell -NoProfile -File cortex-hook.ps1 user_prompt_submit
    powershell -NoProfile -File cortex-hook.ps1 stop
    powershell -NoProfile -File cortex-hook.ps1 permission_request
    powershell -NoProfile -File cortex-hook.ps1 idle_prompt
    powershell -NoProfile -File cortex-hook.ps1 session_end
    powershell -NoProfile -File cortex-hook.ps1 pre_compact

Claude pipes the rest of the event payload (session_id, transcript path,
event-specific fields) as JSON over stdin.

The script is intentionally permissive — every error is swallowed so a hook
failure can't break claude. Diagnostics go to:
    %USERPROFILE%\.claude\cortex-hook.log

This script must work on Windows PowerShell 5.1 (the version baked into
every Windows install). Don't introduce 7+ syntax (?? operator, ternary,
etc.).
#>

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [string]$HookEvent = ''
)

$ErrorActionPreference = 'Continue'

$logPath = Join-Path $env:USERPROFILE '.claude\cortex-hook.log'

function Write-CortexHookLog {
    param([string]$Message)
    try {
        $line = "[{0}] {1}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss.fff'), $Message
        Add-Content -Path $logPath -Value $line -Encoding utf8 -ErrorAction SilentlyContinue
    } catch {
        # If we can't even log, give up quietly. We never want a hook to throw.
    }
}

# ---- Cortex detection ---------------------------------------------------
# Short-circuit cleanly when claude is running outside Cortex (e.g. Windows
# Terminal, vscode integrated terminal). The user's claude config is shared
# across terminals; we must no-op there to avoid spurious side effects.
$protocolVersion = $env:WARP_CLI_AGENT_PROTOCOL_VERSION
if (-not $protocolVersion) {
    # Don't even log — runs outside Cortex are the common case for users with
    # multiple terminals, and we don't want to grow a log file in the no-op path.
    exit 0
}

# ---- Read claude's stdin JSON -------------------------------------------
$stdinJson = ''
$stdinObj  = $null
try {
    $stdinJson = [Console]::In.ReadToEnd()
    if ($stdinJson) {
        $stdinObj = $stdinJson | ConvertFrom-Json -ErrorAction Stop
    }
} catch {
    Write-CortexHookLog ("stdin read/parse failed: {0}" -f $_.Exception.Message)
}

# ---- Discovery probe -----------------------------------------------------
# Append every hook invocation's positional arg + raw stdin to a discovery
# log. Used to confirm whether claude ever fires Notification subtypes we
# don't yet handle (e.g. follow-up questions). Errors are swallowed so we
# never break a hook firing.
try {
    $discoveryLog = Join-Path $env:USERPROFILE '.claude\cortex-hook-discovery.log'
    $argDisplay = if ($HookEvent) { $HookEvent } else { '<none>' }
    $stdinFlat  = if ($stdinJson) { ($stdinJson -replace "[`r`n]+", ' ') } else { '' }
    $line = '{0} arg={1} stdin={2}' -f
        (Get-Date -Format 'o'),
        $argDisplay,
        $stdinFlat
    Add-Content -LiteralPath $discoveryLog -Value $line -Encoding utf8 -ErrorAction SilentlyContinue
} catch {
    # Discovery is best-effort; never let it block the real hook path.
}

# ---- Map cortex event → CLIAgentEvent envelope --------------------------
# The positional arg IS the cortex-side event name. The Notification slot
# in ~/.claude/settings.json is split into two matcher-specific entries
# (`permission_prompt` → `permission_request`, `idle_prompt` →
# `idle_prompt`), so this script doesn't have to substring-match the
# message body. Falling back to claude's `hook_event_name` field handles
# rare cases where the arg got dropped — e.g. a stale entry from before
# the matcher split.
$eventName = $HookEvent
if (-not $eventName -and $stdinObj -and $stdinObj.hook_event_name) {
    # Claude sends e.g. "UserPromptSubmit" — normalize to snake_case.
    $eventName = ($stdinObj.hook_event_name -creplace '([a-z])([A-Z])', '$1_$2').ToLower()
}

$payload = [ordered]@{}
$cortexEvent = $null

switch -Regex ($eventName) {
    '^user_prompt_submit$|^userpromptsubmit$' {
        $cortexEvent = 'prompt_submit'
        if ($stdinObj -and $stdinObj.prompt) { $payload['query'] = [string]$stdinObj.prompt }
        break
    }
    '^stop$' {
        $cortexEvent = 'stop'
        # Stop hook input doesn't include the assistant response text; that
        # lives in the transcript file. Status state machine maps Stop →
        # Success regardless of payload content, so we leave response empty.
        break
    }
    '^session_end$|^sessionend$' {
        # Treat session-end as a final stop — clears any lingering Tier 1
        # InProgress state when claude exits cleanly.
        $cortexEvent = 'stop'
        break
    }
    '^permission_request$' {
        $cortexEvent = 'permission_request'
        if ($stdinObj -and $stdinObj.message)   { $payload['summary']   = [string]$stdinObj.message }
        if ($stdinObj -and $stdinObj.tool_name) { $payload['tool_name'] = [string]$stdinObj.tool_name }
        break
    }
    '^idle_prompt$' {
        # IdlePrompt is benign for status (apply_event ignores it) but we
        # still emit so the discovery log captures it and so a future
        # consumer can react if needed.
        $cortexEvent = 'idle_prompt'
        if ($stdinObj -and $stdinObj.message) { $payload['summary'] = [string]$stdinObj.message }
        break
    }
    '^pre_compact$' {
        # `/compact` (manual) and auto-compaction. Map to prompt_submit so
        # apply_event flips status to InProgress for the duration of the
        # compaction API call. There's no PostCompact hook in current claude;
        # the running animation clears on the next Stop or UserPromptSubmit.
        # We do NOT block compaction (no `decision: "block"` JSON on stdout —
        # we exit 0 with stdout empty; the OSC 777 goes to the TTY directly).
        $cortexEvent = 'prompt_submit'
        if ($stdinObj -and $stdinObj.trigger) {
            # Surface "manual" vs "auto" for the discovery log; not load-
            # bearing for the Rust state machine but useful for grepping.
            $payload['query'] = ('compact ({0})' -f [string]$stdinObj.trigger)
        } else {
            $payload['query'] = 'compact'
        }
        break
    }
    '^notification$' {
        # Stale invocation: an old matcher-less Notification entry left
        # over from a pre-Phase-C settings.json. Fall back to the legacy
        # substring discrimination so we don't break the bridge while the
        # next install rewrites the entries.
        $msg = ''
        if ($stdinObj -and $stdinObj.message) { $msg = [string]$stdinObj.message }
        if ($msg -match '(?i)permission') {
            $cortexEvent = 'permission_request'
            $payload['summary']   = $msg
            if ($stdinObj.tool_name) { $payload['tool_name'] = [string]$stdinObj.tool_name }
        } elseif ($msg) {
            $cortexEvent = 'idle_prompt'
            $payload['summary'] = $msg
        }
        break
    }
    '^pre_tool_use$|^post_tool_use$|^pretooluse$|^posttooluse$' {
        # Tool events are noisy and not load-bearing for the comet animation
        # in v1. Reserved for future enrichment.
        Write-CortexHookLog ("skipping tool event: {0}" -f $eventName)
        exit 0
    }
    default {
        Write-CortexHookLog ("unhandled event '{0}'; skipping" -f $eventName)
        exit 0
    }
}

if (-not $cortexEvent) {
    exit 0
}

# ---- Build the CLIAgentEvent envelope ------------------------------------
# IMPORTANT: the wire format is FLAT. Even though the in-memory Rust
# struct groups query/response/summary/tool_name/etc. under a `payload`
# field for ergonomics (event/mod.rs CLIAgentEventPayload), the v1 parser
# (event/v1.rs RawEvent) expects every field at the top level of the JSON
# body. Nesting them under `payload` causes serde to silently parse them
# as None — the session is created but its `query` stays empty, which
# closes Tier 1's `prompted` gate and the comet animation never updates.
# Verified with the Tier1-probe diagnostic log on 2026-05-05.
$envelope = [ordered]@{
    v     = [int]$protocolVersion
    agent = 'claude'
    event = $cortexEvent
}
foreach ($key in $payload.Keys) {
    $envelope[$key] = $payload[$key]
}
if ($stdinObj) {
    if ($stdinObj.session_id)      { $envelope['session_id']      = [string]$stdinObj.session_id }
    if ($stdinObj.cwd)             { $envelope['cwd']             = [string]$stdinObj.cwd }
    if ($stdinObj.transcript_path) { $envelope['transcript_path'] = [string]$stdinObj.transcript_path }
}

# Compact JSON keeps the OSC sequence small. Depth covers tool_input
# blobs we might forward later; envelope is otherwise flat.
$json = $envelope | ConvertTo-Json -Compress -Depth 8

# ---- OSC 777 byte sequence -----------------------------------------------
# \x1b]777;notify;warp://cli-agent;<JSON>\x07
$prefix = [byte[]]@(0x1B, 0x5D) + [System.Text.Encoding]::ASCII.GetBytes('777;notify;warp://cli-agent;')
$body   = [System.Text.Encoding]::UTF8.GetBytes($json)
$suffix = [byte[]]@(0x07)
$bytes  = $prefix + $body + $suffix

# ---- Emit via AttachConsole(pane_shell_pid) + CONOUT$ -------------------
# Why this is the only technique that works:
#
# Claude on Windows spawns hook child processes with their own console
# (likely CREATE_NEW_CONSOLE, plus an intermediate `bash -c` wrapper layer
# even when the hook command is powershell). A naive WriteFile to CONOUT$
# from inside the hook child opens THAT child's private console buffer,
# not Cortex's ConPTY pseudoconsole — so the bytes evaporate when the hook
# exits. Verified empirically by writing 296-byte OSC 777 envelopes that
# Cortex's parser never logged. (See docs/ai/external-status-injection.md
# § Phase 0 results.)
#
# The fix: walk the process tree up to find the pane shell — the shell
# whose direct parent is `warp-oss.exe` (dev) or `Cortex.exe` (prod) —
# then FreeConsole() to detach from our private console and
# AttachConsole(pane_shell_pid) to attach to the pane shell's console,
# which IS the ConPTY pseudoconsole Cortex listens on. CONOUT$ writes
# from there flow through the ConPTY OUTPUT pipe back to Cortex's ANSI
# parser as if the pane shell itself emitted them.

$emitted = $false

try {
    if (-not ('Cortex.Hook.Native' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace Cortex.Hook {
    public static class Native {
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

    # Walk the ancestor chain up to a Cortex process. Treat the shell-named
    # ancestor closest to that root as the pane shell — that's the one whose
    # console is the ConPTY.
    $shellPattern = '^(powershell|pwsh|cmd|bash|fish|zsh|sh)\.exe$'
    $cortexPattern = '^(warp-oss|Cortex)\.exe$'
    $cur = [uint32]$PID
    $chain = @()
    for ($i = 0; $i -lt 16; $i++) {
        $proc = Get-WmiObject Win32_Process -Filter "ProcessId = $cur" -ErrorAction SilentlyContinue
        if (-not $proc) { break }
        $chain += [pscustomobject]@{ Pid = [uint32]$cur; Name = $proc.Name }
        if ($proc.Name -match $cortexPattern) { break }
        if ($proc.ParentProcessId -eq 0 -or $proc.ParentProcessId -eq $cur) { break }
        $cur = [uint32]$proc.ParentProcessId
    }

    $shellPid = $null
    $cortexIdx = -1
    for ($i = 0; $i -lt $chain.Count; $i++) {
        if ($chain[$i].Name -match $cortexPattern) { $cortexIdx = $i; break }
    }
    if ($cortexIdx -ge 0) {
        for ($j = $cortexIdx - 1; $j -ge 0; $j--) {
            if ($chain[$j].Name -match $shellPattern) {
                $shellPid = $chain[$j].Pid
                break
            }
        }
    } else {
        # No Cortex process found in chain — running outside Cortex (was the
        # WARP_CLI_AGENT_PROTOCOL_VERSION short-circuit somehow bypassed?) or
        # truncated walk. Fall back to deepest shell ancestor so we don't
        # silently no-op on edge cases.
        for ($j = $chain.Count - 1; $j -ge 0; $j--) {
            if ($chain[$j].Name -match $shellPattern) {
                $shellPid = $chain[$j].Pid
                break
            }
        }
    }

    if (-not $shellPid) {
        Write-CortexHookLog ("emit FAIL: no shell ancestor found in chain ({0})" -f (($chain | ForEach-Object { "$($_.Pid)=$($_.Name)" }) -join ' -> '))
        exit 0
    }

    [void][Cortex.Hook.Native]::FreeConsole()
    $attached = [Cortex.Hook.Native]::AttachConsole($shellPid)
    if (-not $attached) {
        $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
        Write-CortexHookLog ("emit FAIL: AttachConsole({0}) lasterror={1}" -f $shellPid, $err)
        exit 0
    }

    try {
        $h = [Cortex.Hook.Native]::CreateFileW('CONOUT$', [uint32]0x40000000, [uint32]0x00000003, [IntPtr]::Zero, [uint32]3, [uint32]0, [IntPtr]::Zero)
        $invalid = [IntPtr]::new(-1)
        if ($h -eq $invalid) {
            $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
            Write-CortexHookLog ("emit FAIL: CreateFileW(CONOUT`$) lasterror={0}" -f $err)
        } else {
            try {
                [uint32]$written = 0
                $ok = [Cortex.Hook.Native]::WriteFile($h, $bytes, [uint32]$bytes.Length, [ref]$written, [IntPtr]::Zero)
                if ($ok -and $written -gt 0) {
                    $emitted = $true
                } else {
                    $err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
                    Write-CortexHookLog ("emit FAIL: WriteFile lasterror={0}" -f $err)
                }
            } finally {
                [void][Cortex.Hook.Native]::CloseHandle($h)
            }
        }
    } finally {
        [void][Cortex.Hook.Native]::FreeConsole()
    }
} catch {
    Write-CortexHookLog ("emit FAIL ({0})" -f $_.Exception.Message)
}

if ($emitted) {
    Write-CortexHookLog ("emit ok event={0} bytes={1} pane_shell={2}" -f $cortexEvent, $bytes.Length, $shellPid)
}

exit 0
