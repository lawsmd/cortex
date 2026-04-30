@echo off
REM Cortex rapid-iteration launcher (Windows).
REM Each click does an incremental cargo build + run, so any source edits
REM Claude (or you) made since the last launch are picked up automatically.
REM
REM This is equivalent to `./script/run` on Windows minus the
REM install_channel_config SSH check (which always fails on the OSS fork).
REM See script/run:117-123 — the Windows dispatch is just `cargo run`.

title Cortex (rebuild + launch)
cd /d C:\Users\Michael\cortex

echo === Building Cortex (incremental) ===
echo Started: %date% %time%
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
del /q "%TEMP%\cortex-hb.stop" >nul 2>&1
start "" /b powershell -NoProfile -Command "$s=Get-Date; while(-not (Test-Path '%TEMP%\cortex-hb.stop')){Start-Sleep 30; if(Test-Path '%TEMP%\cortex-hb.stop'){break}; Write-Host ('[heartbeat] '+((Get-Date)-$s).ToString('hh\:mm\:ss')+' elapsed') -ForegroundColor DarkGray}"

REM `--timings` writes a per-crate HTML report to
REM target\cargo-timings\cargo-timing-<timestamp>.html.
REM Open the latest one to see which crate dominated the build.
REM
REM `skip_login` auto-authenticates as a test user (auth_state.rs:137).
REM Without it, the OSS channel shows the Warp login screen, and the
REM `warposs://` OAuth callback re-launches the binary — producing an
REM infinite restart loop because dev builds lack single-instance IPC.
cargo run --bin warp-oss --features gui,skip_login --timings

set CARGO_EXIT=%ERRORLEVEL%

REM Signal the heartbeat to stop now that cargo is done.
type nul > "%TEMP%\cortex-hb.stop"

echo.
echo === Cortex exited with code %CARGO_EXIT% ===
echo Finished: %date% %time%
echo Per-crate timings: target\cargo-timings\cargo-timing-*.html  (open the latest)
pause
