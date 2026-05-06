@echo off
REM Cortex rapid-iteration launcher (Windows) - DEV lane.
REM Each click does an incremental cargo build + run, so any source edits
REM Claude (or you) made since the last launch are picked up automatically.
REM
REM Pair with the prod lane (scripts\install-cortex-prod.cmd +
REM scripts\launch-cortex.cmd). Both lanes share the warp-oss channel
REM state at %USERPROFILE%\.warp-oss\, so theme/setting changes made in
REM dev appear in prod after prod restarts.
REM
REM This is equivalent to `./script/run` on Windows minus the
REM install_channel_config SSH check (which always fails on the OSS fork).
REM See script/run:117-123 - the Windows dispatch is just `cargo run`.
REM
REM Each launch captures cargo's combined output to
REM   .cortex-logs\cortex-dev-<TS>.log
REM (sibling prefix to the prod-build logs `cortex-prod-*.log` written by
REM scripts\install-cortex-prod.cmd) so Claude Code agents can read the
REM current/last session log without cmd-window copy/paste.
REM See CLAUDE.md "Live dev-session logs".
REM
REM This script and install-cortex-prod.cmd are intentionally near-mirror
REM images of each other in section structure (header, identity, log
REM capture, retention, log header, heartbeat, cargo invocation w/ --timings,
REM duration line, footer). Differences are intrinsic to each lane's
REM purpose: dev uses `cargo run` and skips the prod-only EXE-copy / asset
REM copy / build-stamp steps.

REM --- Bake "Cortex Dev" into the embedded Windows resources so the dev
REM     EXE's file properties and FileDescription read distinctly from
REM     prod's "Cortex". build.rs reads WARP_APP_NAME at compile time;
REM     rerun-if-env-changed=WARP_APP_NAME (in build.rs) makes incremental
REM     toggles between prod ("Cortex") and dev ("Cortex Dev") rebuild
REM     the resource without a `cargo clean`.
set WARP_APP_NAME=Cortex Dev

REM --- Isolate dev's runtime state from prod. WARP_DATA_PROFILE is
REM     honored only in debug builds (gated on cfg!(debug_assertions)
REM     in crates/warp_core/src/channel/state.rs:132), so this has no
REM     effect on release-mode prod even if the env var leaks into its
REM     environment via the parent shell. Result: dev writes to
REM     %USERPROFILE%\.warp-oss-dev\ and %APPDATA%\warp\WarpOss-dev\*
REM     while prod stays on the un-suffixed paths. The one explicit
REM     exception is %USERPROFILE%\.warp-oss\projects.json, which is
REM     intentionally shared between lanes (see
REM     warp_home_projects_file_path in crates/warp_core/src/paths.rs).
REM     Mirrors scripts/launch-cortex-dev.sh:122-127 on macOS.
set WARP_DATA_PROFILE=dev

title Cortex Dev (rebuild + launch)
REM cd to repo root via the script's own location so this works regardless
REM of where Cortex is installed (or after a Windows reinstall onto a
REM different drive). %~dp0 is this script's directory with a trailing
REM backslash; cd resolves the .. immediately.
cd /d "%~dp0.."

REM --- Setup capture path ---
REM     Why this looks weird: capturing PowerShell stdout via `for /f` is fragile
REM     in PS 5.1 - the output can include a UTF-16 BOM or extra blank lines that
REM     leak into the captured value, producing a multi-line LOG_TS that breaks
REM     `>` redirection downstream ("filename syntax is incorrect"). Writing PS
REM     output to a temp file with ASCII encoding and reading it back via
REM     `set /p` strips trailing CRLF and dodges the BOM entirely.
REM
REM     LOG_PATH is absolute (%CD% prefix) because the inner PowerShell pipeline
REM     uses [IO.StreamWriter], which resolves relative paths via .NET's current
REM     directory rather than PS's location - they don't always agree.
if not exist .cortex-logs mkdir .cortex-logs
powershell -NoProfile -Command "[Console]::OutputEncoding=[Text.Encoding]::ASCII; Get-Date -Format yyyy-MM-dd-HHmmss" > "%TEMP%\cortex-ts.txt"
set /p LOG_TS=<"%TEMP%\cortex-ts.txt"
del /q "%TEMP%\cortex-ts.txt" >nul 2>&1
set LOG_PATH=%CD%\.cortex-logs\cortex-dev-%LOG_TS%.log

REM --- Retention: keep only the 10 newest dev-session logs. Done at launch
REM     start (not exit) so a crash on the previous run can't postpone cleanup.
REM     Prod-build logs (`cortex-prod-*.log`) rotate independently in
REM     install-cortex-prod.cmd so the two lanes don't fight over slots.
powershell -NoProfile -Command "Get-ChildItem '.cortex-logs\cortex-dev-*.log' | Sort-Object LastWriteTime -Descending | Select-Object -Skip 10 | Remove-Item -Force" >nul 2>&1

REM --- Gather build identity for the header ---
REM     Route git output through a temp file + `set /p` instead of
REM     `for /f ('git ...')`. The `for /f` capture path empirically
REM     injects a leading 0x0C (form feed) byte into the captured value
REM     on this machine's Git-for-Windows install -- the same bug the
REM     launch-cortex.cmd staleness check works around (and that
REM     install-cortex-prod.cmd's build-stamp section worked around
REM     after we got bitten by it). For the dev launcher this is only
REM     cosmetic (the affected fields land in the log-file header text,
REM     not in any parsed comparison) but matching the pattern keeps
REM     dev/prod symmetric.
set GIT_REV=unknown
set GIT_BRANCH=unknown
set _TMP=%TEMP%\cortex-dev.tmp
git rev-parse --short HEAD > "%_TMP%" 2>nul
if exist "%_TMP%" (
    set /p GIT_REV=<"%_TMP%"
    del /q "%_TMP%" >nul 2>&1
)
git rev-parse --abbrev-ref HEAD > "%_TMP%" 2>nul
if exist "%_TMP%" (
    set /p GIT_BRANCH=<"%_TMP%"
    del /q "%_TMP%" >nul 2>&1
)
set GIT_DIRTY=
git diff --quiet 2>nul
if errorlevel 1 set GIT_DIRTY=+dirty

REM --- Write log header. ASCII content, so cp1252-via-cmd-echo and
REM     UTF-8-via-StreamWriter (below) read identically. We use cmd's
REM     %date%/%time% directly - the human-readable format here is just for
REM     the header annotation; the agent-readable liveness sentinel is the
REM     `=== Cortex exited code=` footer prefix, not the timestamp format.
(
  echo === Cortex launch started %date% %time% ===
  echo Branch:    %GIT_BRANCH%
  echo Commit:    %GIT_REV%%GIT_DIRTY%
  echo OS:        Windows
  echo Launcher:  scripts\launch-cortex-dev.bat
  echo.
) > "%LOG_PATH%"

echo === Building Cortex Dev (incremental) ===
echo Started: %date% %time%
echo Log:     %LOG_PATH%
echo.
echo Note: cargo's progress bar tops out *before* the MSVC linker runs.
echo After the last `Compiling warp` line the linker takes 1-2 min silently
echo on a debug build of this size. The [heartbeat] lines below confirm
echo the build is still alive during that phase. If heartbeats keep
echo firing for 5+ min and Task Manager shows no `link.exe` / `lld-link.exe`
echo / `rustc.exe` activity, then it's actually stuck.
echo.

REM Background heartbeat: prints elapsed time every 30s while cargo runs.
REM Started via `start /b` so it shares this console (and dies with it).
REM We also use a stop file so cleanup is immediate when cargo exits.
REM Heartbeat lines stay in the cmd window only - they don't get teed to
REM the log file (different process; not part of cargo's pipe). That's
REM fine: heartbeats are dev-loop noise, not bug-diagnostic signal.
del /q "%TEMP%\cortex-hb.stop" >nul 2>&1
start "" /b powershell -NoProfile -Command "$s=Get-Date; while(-not (Test-Path '%TEMP%\cortex-hb.stop')){Start-Sleep 30; if(Test-Path '%TEMP%\cortex-hb.stop'){break}; Write-Host ('[heartbeat] '+((Get-Date)-$s).ToString('hh\:mm\:ss')+' elapsed') -ForegroundColor DarkGray}"

REM --- Build & run, with combined output teed to the log file ---
REM
REM CARGO_TERM_COLOR=never strips SGR escape codes so the log file is
REM readable as plain text by agents (we pass output through, so colors
REM would otherwise survive into the file).
REM CARGO_TERM_PROGRESS_WHEN=never disables the carriage-return progress
REM bar; non-TTY pipes usually do this anyway, but it's belt-and-suspenders.
REM
REM `cmd /c '... 2>&1'` merges stderr into stdout *inside cmd*, so PowerShell
REM never sees raw stderr (which PS 5.1 wraps as NativeCommandError
REM ErrorRecords, polluting the file).
REM
REM We can't use `Tee-Object -Encoding utf8` - that parameter doesn't exist
REM on PS 5.1 (added in PS 6+). Instead we open an [IO.StreamWriter] with an
REM explicit no-BOM UTF-8 encoding and ForEach-Object both Write-Host
REM (live cmd console) and $sw.WriteLine (file). AutoFlush keeps the file
REM live-tail-able while cargo runs.
REM
REM [Console]::OutputEncoding=$enc makes PS interpret cargo's bytes as UTF-8
REM (cargo emits UTF-8; PS 5.1 otherwise decodes via the OEM code page and
REM mangles non-ASCII chars in compiler error messages).
REM
REM The `& { ... ; $script:rc = $LASTEXITCODE }` pattern captures cargo's
REM exit code from inside the script block before the pipeline finishes.
REM The Session-duration line (build + run elapsed) is emitted from inside
REM the same PS process so there's no clock-skew / cmd %time% midnight-
REM wraparound to handle. Cargo's own "Finished `dev` profile ... in N"
REM line -- also captured in this log -- gives the pure build time.
REM Final `exit $script:rc` propagates that to %ERRORLEVEL%.
REM
REM `--timings` writes a per-crate HTML report to
REM target\cargo-timings\cargo-timing-<timestamp>.html.
REM Open the latest one to see which crate dominated the build.
REM
REM `skip_login` auto-authenticates as a test user (auth_state.rs:137).
REM Without it, the OSS channel shows the Warp login screen, and the
REM `warposs://` OAuth callback re-launches the binary - producing an
REM infinite restart loop because dev builds lack single-instance IPC.
set CARGO_TERM_COLOR=never
set CARGO_TERM_PROGRESS_WHEN=never
powershell -NoProfile -Command "$enc=New-Object System.Text.UTF8Encoding $false; [Console]::OutputEncoding=$enc; $sw=New-Object System.IO.StreamWriter('%LOG_PATH%',$true,$enc); $sw.AutoFlush=$true; $start=Get-Date; try { & { cmd /c 'cargo run --bin warp-oss --features gui,skip_login --timings 2>&1' ; $script:rc=$LASTEXITCODE } | ForEach-Object { Write-Host $_; $sw.WriteLine($_) }; $line='=== Session duration: {0:hh\:mm\:ss} ===' -f ((Get-Date)-$start); Write-Host $line; $sw.WriteLine($line) } finally { $sw.Close() }; exit $script:rc"

set CARGO_EXIT=%ERRORLEVEL%

REM Signal the heartbeat to stop now that cargo is done.
type nul > "%TEMP%\cortex-hb.stop"

REM --- Footer (clean-exit sentinel agents look for) ---
echo.
echo === Cortex exited with code %CARGO_EXIT% ===
echo Finished: %date% %time%
echo Log:      %LOG_PATH%
echo === Cortex exited code=%CARGO_EXIT% at %date% %time% === >> "%LOG_PATH%"

echo Per-crate timings: target\cargo-timings\cargo-timing-*.html  (open the latest)
pause
