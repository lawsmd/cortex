# Phase 0 — Windows TTY-bypass smoke test

Dev-only scaffolding to settle the Gap 2 question from
`docs/ai/external-status-injection.md`: which technique can a claude hook
use to write OSC bytes that Cortex's ANSI parser actually receives, given
that claude captures hook stdout for everything except `UserPromptSubmit` /
`UserPromptExpansion` / `SessionStart`?

## What this does

Adds a `Stop` hook to `~/.claude/settings.json` that runs
`run-smoketest.ps1`. The runner attempts four candidate techniques in
sequence, each setting a uniquely-numbered window title
(`CortexHookTest-T1` … `T4`) with a 1500ms pause between each so progress
is visible in real time.

Final visible title = the last technique that succeeded. Definitive
PASS/FAIL per technique is written to
`%USERPROFILE%\.claude\cortex-smoketest.log` with exception detail.

## Techniques tested (in order)

1. **`cmd /c "type <tmpfile> > CON"`** — writes raw OSC bytes to a temp
   file, then has cmd type-redirect them to the `CON` device. Cleanest
   non-PowerShell path.
2. **PowerShell `[FileStream]` against `CONOUT$`** — opens the console-output
   device by its standard name, writes raw bytes. Most likely to succeed
   because it explicitly opens the console device, not stdout.
3. **`[Console]::Out.Write`** — uses .NET's stdout. Probably fails since
   that's the fd claude captures, but worth a recorded data point.
4. **P/Invoke `kernel32!CreateFileW("CONOUT$") + WriteFile`** — last resort,
   bypasses the .NET runtime entirely. Most reliable; also the ugliest.

## Usage

```powershell
# 1. Install
powershell -NoProfile -ExecutionPolicy Bypass -File .\install-hook.ps1

# 2. In a Cortex pane (prod or dev), run claude and submit any short prompt.
#    Wait for the response. The Stop hook fires when claude finishes its turn.

# 3. Read the result log
Get-Content $env:USERPROFILE\.claude\cortex-smoketest.log -Tail 30

# 4. Clean up
powershell -NoProfile -ExecutionPolicy Bypass -File .\uninstall-hook.ps1
```

## What "success" looks like

In the **window title bar** of the Cortex pane:

- See the title cycle through `CortexHookTest-T1`, `T2`, `T3`, `T4` →
  multiple techniques work; the title flip is reaching the parser. Pick the
  cleanest of the working set for the production hook.
- See only one or two of the four titles → only those techniques work.
- See no title change at all → all four techniques are captured by claude
  and we need to fall back to Alternative B (file-based polling) for
  Windows.

In the **log file**:

- `T<n> ...: PASS` means the technique completed without throwing.
- `T<n> ...: FAIL (...)` means the technique threw — exception text is
  appended for diagnosis.

> **Important:** PASS in the log does NOT prove visibility. A technique
> can complete its write but go to a fd claude captures, in which case
> the bytes never reach Cortex. The window-title flip is the visibility
> oracle; the log is the *liveness* oracle (did this code path run at
> all?).

## Coexistence with existing hooks

Idempotent. Marker field `_cortex_smoketest=true` lets the install/uninstall
scripts find their entry without disturbing other hooks. Tested on a config
that already had SideQuest's `claude-status.sh` hooks across all four
events.

## After the test

Append findings to `docs/ai/external-status-injection.md` under a new
"Phase 0 results" section. Then `uninstall-hook.ps1` and proceed to
Phase 2 with the chosen technique.
