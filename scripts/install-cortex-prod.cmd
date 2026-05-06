@echo off
REM Cortex prod installer (Windows).
REM Builds release-mode warp-oss and copies the EXE to a stable location at
REM   %LOCALAPPDATA%\Cortex\Cortex.exe
REM independent of target/, so Cloud agents and dev rebuilds never lock the
REM running prod EXE. Run this whenever you want prod to catch up to main.
REM
REM Pair with:
REM   scripts\launch-cortex.cmd       - daily-driver launcher (delegates here on R-rebuild)
REM   scripts\launch-cortex-dev.bat   - live-rebuild dev loop (sibling lane)
REM
REM This script and launch-cortex-dev.bat are intentionally near-mirror
REM images of each other in section structure (header, identity, log
REM capture, retention, log header, heartbeat, cargo invocation w/ --timings,
REM duration line, footer). Prod-only steps (refuse-while-running,
REM EXE/asset copy, build stamp) come after the build; everything before
REM the build is dev/prod-symmetric.
REM
REM Each prod build captures cargo's combined output (with --timings) to
REM   .cortex-logs\cortex-prod-<TS>.log
REM so build-time regressions can be studied without cmd-window copy/paste.
REM Open the latest target\cargo-timings\cargo-timing-*.html for a per-crate
REM Gantt + slowest-units table. See CLAUDE.md "Live dev-session logs" for
REM the convention agents use to find these logs.
REM
REM See CLAUDE.md "The dev loop" for the full two-lane workflow.

setlocal enableextensions
title Cortex (build prod + install)
cd /d C:\Users\Michael\cortex

set INSTALL_DIR=%LOCALAPPDATA%\Cortex
set INSTALL_PATH=%INSTALL_DIR%\Cortex.exe

REM --- Refuse to clobber a running prod EXE (Windows holds an exclusive
REM     lock on the binary while it executes; copy /Y would fail mid-step
REM     and leave a half-installed Cortex.exe). Bail early with a clear
REM     message instead.
tasklist /FI "IMAGENAME eq Cortex.exe" 2>nul | find /I "Cortex.exe" >nul
if not errorlevel 1 (
    echo.
    echo Cortex is currently running. Close it before re-installing prod.
    echo   ^(Right-click the taskbar icon -^> Close window, or run:
    echo      taskkill /IM Cortex.exe /F^)
    echo.
    if not defined CORTEX_NONINTERACTIVE pause
    exit /b 1
)

REM --- Bake "Cortex" into the embedded Windows resources for this build.
REM     build.rs reads WARP_APP_NAME and emits it as the FileDescription in
REM     the .rc file. Pair with rerun-if-env-changed=WARP_APP_NAME (added
REM     in build.rs) so toggling between prod and dev triggers a resource
REM     rebuild on incremental cargo runs.
set WARP_APP_NAME=Cortex

REM --- Tell build.rs which target subdir to drop conpty.dll / OpenConsole.exe
REM     / dxcompiler.dll / dxil.dll into. Cargo's built-in PROFILE env var only
REM     resolves to "debug" or "release", but build.rs needs the full profile
REM     name (which can be a custom one). Without this set, build.rs defaults
REM     to "debug" and copies the runtime ConPTY assets to target\debug\
REM     instead of target\release\ - so a `cargo build --release` produces
REM     a target\release\warp-oss.exe with NO sibling DLL/exe. At runtime,
REM     LoadLibraryW("conpty.dll") then falls through Windows' DLL search
REM     order to whatever conpty.dll happens to be in PATH (WezTerm bundles
REM     one at C:\Program Files\WezTerm\conpty.dll, for instance), and the
REM     ABI-mismatched OpenConsole.exe spawned by that foreign DLL crashes
REM     immediately on CreatePseudoConsole - leaving every Git Bash tab stuck
REM     on "Starting Git Bash..." with bash orphaned at 100% kernel CPU.
REM     See app/build.rs:141 and the copy_windows_assets call site.
set CARGO_FULL_PROFILE=release

REM --- Setup capture path (mirrors launch-cortex-dev.bat) ---
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
REM
REM     The `cortex-prod-` prefix segregates prod-build logs from
REM     `cortex-dev-` dev-session logs so each lane retains independently.
if not exist .cortex-logs mkdir .cortex-logs
powershell -NoProfile -Command "[Console]::OutputEncoding=[Text.Encoding]::ASCII; Get-Date -Format yyyy-MM-dd-HHmmss" > "%TEMP%\cortex-ts.txt"
set /p LOG_TS=<"%TEMP%\cortex-ts.txt"
del /q "%TEMP%\cortex-ts.txt" >nul 2>&1
set LOG_PATH=%CD%\.cortex-logs\cortex-prod-%LOG_TS%.log

REM --- Retention: keep only 10 newest prod-build logs. Done at start (not
REM     exit) so a crash on the previous run can't postpone cleanup.
REM     Dev-session logs use a separate `cortex-dev-*.log` prefix and rotate
REM     independently in launch-cortex-dev.bat.
powershell -NoProfile -Command "Get-ChildItem '.cortex-logs\cortex-prod-*.log' | Sort-Object LastWriteTime -Descending | Select-Object -Skip 10 | Remove-Item -Force" >nul 2>&1

REM --- Gather build identity for the header ---
REM     Route git output through a temp file + `set /p` instead of
REM     `for /f ('git ...')`. The `for /f` capture path empirically
REM     injects a leading 0x0C (form feed) byte into the captured value
REM     on this machine's Git-for-Windows install -- the same bug the
REM     launch-cortex.cmd staleness check works around. With the bug,
REM     the build-stamp commit= line ends up `commit=^L<sha>`, which
REM     then makes the launcher's `git rev-list --count BUILD..HEAD`
REM     fail silently (BUILD is invalid) and the staleness prompt fires
REM     on every launch even when prod is current. Temp file + set /p
REM     gives a clean, CRLF-stripped string with no embedded 0x0C.
set GIT_REV=unknown
set GIT_BRANCH=unknown
set _TMP=%TEMP%\cortex-prod.tmp
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
REM     UTF-8-via-StreamWriter (below) read identically.
(
  echo === Cortex prod build started %date% %time% ===
  echo Branch:    %GIT_BRANCH%
  echo Commit:    %GIT_REV%%GIT_DIRTY%
  echo OS:        Windows
  echo Profile:   release  ^(--features gui,skip_login --timings^)
  echo Launcher:  scripts\install-cortex-prod.cmd
  echo.
) > "%LOG_PATH%"

echo === Building release Cortex (release mode is slow on Windows) ===
echo Started: %date% %time%
echo Log:     %LOG_PATH%
echo.
echo Note: cargo's progress bar tops out *before* the MSVC linker runs.
echo Release builds spend additional silent minutes in the linker phase
echo writing PDBs (debug=true is on for sentry stacktraces). The
echo [heartbeat] lines below confirm the build is still alive during that
echo phase. If heartbeats keep firing for many minutes and Task Manager
echo shows no `link.exe` / `lld-link.exe` / `rustc.exe` activity, it's
echo actually stuck.
echo.

REM --- Heartbeat (mirrors launch-cortex-dev.bat) ---
REM     Background heartbeat: prints elapsed time every 30s while cargo
REM     runs. Started via `start /b` so it shares this console (and dies
REM     with it). A stop file at %TEMP%\cortex-hb.stop lets cleanup be
REM     immediate when cargo exits. Heartbeat lines stay in the cmd
REM     window only - they don't get teed to the log file (different
REM     process; not part of cargo's pipe). That's fine: heartbeats are
REM     dev-loop noise, not bug-diagnostic signal.
del /q "%TEMP%\cortex-hb.stop" >nul 2>&1
start "" /b powershell -NoProfile -Command "$s=Get-Date; while(-not (Test-Path '%TEMP%\cortex-hb.stop')){Start-Sleep 30; if(Test-Path '%TEMP%\cortex-hb.stop'){break}; Write-Host ('[heartbeat] '+((Get-Date)-$s).ToString('hh\:mm\:ss')+' elapsed') -ForegroundColor DarkGray}"

REM --- Build with --timings, teed to log (mirrors dev's StreamWriter pattern) ---
REM
REM     CARGO_TERM_COLOR=never strips SGR escape codes so the log file is
REM     readable as plain text by agents (we pass output through, so colors
REM     would otherwise survive into the file).
REM     CARGO_TERM_PROGRESS_WHEN=never disables the carriage-return progress
REM     bar; non-TTY pipes usually do this anyway, belt-and-suspenders.
REM
REM     `cmd /c '... 2>&1'` merges stderr into stdout *inside cmd*, so PowerShell
REM     never sees raw stderr (which PS 5.1 wraps as NativeCommandError
REM     ErrorRecords, polluting the file).
REM
REM     We can't use `Tee-Object -Encoding utf8` - that parameter doesn't exist
REM     on PS 5.1 (added in PS 6+). Instead we open an [IO.StreamWriter] with an
REM     explicit no-BOM UTF-8 encoding and ForEach-Object both Write-Host
REM     (live cmd console) and $sw.WriteLine (file). AutoFlush keeps the file
REM     live-tail-able while cargo runs.
REM
REM     [Console]::OutputEncoding=$enc makes PS interpret cargo's bytes as UTF-8
REM     (cargo emits UTF-8; PS 5.1 otherwise decodes via the OEM code page and
REM     mangles non-ASCII chars in compiler error messages).
REM
REM     The `& { ... ; $script:rc = $LASTEXITCODE }` pattern captures cargo's
REM     exit code from inside the script block before the pipeline finishes.
REM     The Build-duration line is emitted from inside the same PS process so
REM     there's no clock-skew / cmd %time% midnight-wraparound to handle.
REM     Final `exit $script:rc` propagates to %ERRORLEVEL%.
REM
REM     `--timings` writes a per-crate HTML report to
REM     target\cargo-timings\cargo-timing-<timestamp>.html.
REM     Open the latest one to see which crate dominated the build.
set CARGO_TERM_COLOR=never
set CARGO_TERM_PROGRESS_WHEN=never
powershell -NoProfile -Command "$enc=New-Object System.Text.UTF8Encoding $false; [Console]::OutputEncoding=$enc; $sw=New-Object System.IO.StreamWriter('%LOG_PATH%',$true,$enc); $sw.AutoFlush=$true; $start=Get-Date; try { & { cmd /c 'cargo build --release --bin warp-oss --features gui,skip_login --timings 2>&1' ; $script:rc=$LASTEXITCODE } | ForEach-Object { Write-Host $_; $sw.WriteLine($_) }; $line='=== Build duration: {0:hh\:mm\:ss} ===' -f ((Get-Date)-$start); Write-Host $line; $sw.WriteLine($line) } finally { $sw.Close() }; exit $script:rc"

set CARGO_EXIT=%ERRORLEVEL%

REM Signal the heartbeat to stop now that cargo is done.
type nul > "%TEMP%\cortex-hb.stop"

if not "%CARGO_EXIT%"=="0" (
    echo === Cortex prod build exited code=%CARGO_EXIT% at %date% %time% === >> "%LOG_PATH%"
    echo.
    echo Build failed ^(exit %CARGO_EXIT%^). Prod EXE not updated.
    echo Log: %LOG_PATH%
    echo Per-crate timings: target\cargo-timings\cargo-timing-*.html  ^(open the latest^)
    if not defined CORTEX_NONINTERACTIVE pause
    exit /b %CARGO_EXIT%
)

echo.
echo === Installing to %INSTALL_PATH% ===

if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"
copy /Y "target\release\warp-oss.exe" "%INSTALL_PATH%"
if errorlevel 1 (
    echo.
    echo Copy failed. Prod EXE not updated.
    if not defined CORTEX_NONINTERACTIVE pause
    exit /b 1
)

REM --- Copy the runtime ConPTY/DirectX assets next to Cortex.exe.
REM     Cortex.exe loads conpty.dll via LoadLibraryW("conpty.dll") at
REM     PTY-spawn time, and that DLL in turn launches x64\OpenConsole.exe
REM     relative to itself - so both must sit next to Cortex.exe in
REM     INSTALL_DIR or LoadLibrary falls back to a foreign conpty.dll on
REM     PATH (WezTerm/Windows Terminal/etc.) and the ABI mismatch crashes
REM     OpenConsole on every spawn. dxcompiler.dll and dxil.dll are wgpu's
REM     DXC shader compiler; without them, wgpu falls back to the older
REM     FXC compiler with degraded shader features.
REM
REM     We copy from the source-of-truth at app\assets\windows\x64\
REM     (checked into the repo) rather than target\release\, so this
REM     step is not coupled to whether build.rs's copy_windows_assets
REM     ran in the current cargo invocation. Incremental cargo builds
REM     skip build.rs entirely when nothing dirty triggers it, so a
REM     re-install after a no-op build would otherwise find empty
REM     target\release\ companion files.
set ASSET_SRC=%CD%\app\assets\windows\x64
if not exist "%INSTALL_DIR%\x64" mkdir "%INSTALL_DIR%\x64"
copy /Y "%ASSET_SRC%\conpty.dll"      "%INSTALL_DIR%\conpty.dll"      >nul
copy /Y "%ASSET_SRC%\dxcompiler.dll"  "%INSTALL_DIR%\dxcompiler.dll"  >nul
copy /Y "%ASSET_SRC%\dxil.dll"        "%INSTALL_DIR%\dxil.dll"        >nul
copy /Y "%ASSET_SRC%\OpenConsole.exe" "%INSTALL_DIR%\x64\OpenConsole.exe" >nul
if errorlevel 1 (
    echo.
    echo Runtime asset copy failed. Prod will fall through to a foreign
    echo conpty.dll on PATH and Git Bash tabs will hang.
    if not defined CORTEX_NONINTERACTIVE pause
    exit /b 1
)

REM --- Write build-stamp so launch-cortex.cmd can detect when prod is
REM     stale relative to the current working tree. Format: simple key=val
REM     lines (commit/branch/dirty/built) so the launcher can parse with
REM     `findstr "^commit=" | for /f "tokens=2 delims==" ...`. If git is
REM     unavailable for any reason we just skip the stamp - the launcher
REM     handles a missing stamp by launching unconditionally.
REM
REM     Same temp-file + set /p pattern as the GIT_REV/BRANCH header
REM     gathering above (and as launch-cortex.cmd's staleness check)
REM     to avoid the 0x0C-byte injection that `for /f ('git ...')` and
REM     `for /f ('powershell ...')` exhibit on this machine. Without
REM     this fix, the stamp file ends up with form-feed bytes on every
REM     value line, which makes the staleness check misfire on every
REM     launch.
set BUILD_COMMIT=
set BUILD_BRANCH=
set BUILD_TIME=
git rev-parse HEAD > "%_TMP%" 2>nul
if exist "%_TMP%" (
    set /p BUILD_COMMIT=<"%_TMP%"
    del /q "%_TMP%" >nul 2>&1
)
git rev-parse --abbrev-ref HEAD > "%_TMP%" 2>nul
if exist "%_TMP%" (
    set /p BUILD_BRANCH=<"%_TMP%"
    del /q "%_TMP%" >nul 2>&1
)
set BUILD_DIRTY=no
git diff --quiet 2>nul
if errorlevel 1 set BUILD_DIRTY=yes
powershell -NoProfile -Command "Get-Date -Format o" > "%_TMP%" 2>nul
if exist "%_TMP%" (
    set /p BUILD_TIME=<"%_TMP%"
    del /q "%_TMP%" >nul 2>&1
)

if defined BUILD_COMMIT (
    (
        echo commit=%BUILD_COMMIT%
        echo branch=%BUILD_BRANCH%
        echo dirty=%BUILD_DIRTY%
        echo built=%BUILD_TIME%
    ) > "%INSTALL_DIR%\Cortex.build-info"
)

REM --- Footer (matches dev's clean-exit sentinel format) ---
echo.
echo === Done ===
echo Prod EXE:    %INSTALL_PATH%
echo Build stamp: %INSTALL_DIR%\Cortex.build-info  ^(commit %BUILD_COMMIT:~0,7%^)
echo Log:         %LOG_PATH%
echo Per-crate timings: target\cargo-timings\cargo-timing-*.html  ^(open the latest^)
echo Launcher:    scripts\launch-cortex.cmd
echo.
echo To set up Desktop and Start Menu shortcuts (with custom DEV-overlay
echo icon for the dev shortcut), run once:
echo     powershell -NoProfile -ExecutionPolicy Bypass -File scripts\install-shortcuts.ps1
echo Idempotent - re-run anytime to refresh shortcut targets/icons.
echo.
echo Finished: %date% %time%
echo === Cortex prod build exited code=%CARGO_EXIT% at %date% %time% === >> "%LOG_PATH%"
REM CORTEX_NONINTERACTIVE=1 from launch-cortex.cmd's [R]ebuild path skips the
REM trailing pause so the launcher can immediately start the new EXE.
if not defined CORTEX_NONINTERACTIVE pause
endlocal
