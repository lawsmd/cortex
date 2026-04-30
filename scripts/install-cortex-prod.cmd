@echo off
REM Cortex prod installer (Windows).
REM Builds release-mode warp-oss and copies the EXE to a stable location at
REM   %LOCALAPPDATA%\Cortex\Cortex.exe
REM independent of target/, so Cloud agents and dev rebuilds never lock the
REM running prod EXE. Run this whenever you want prod to catch up to main.
REM
REM Pair with:
REM   scripts\launch-cortex.cmd       - daily-driver launcher
REM   scripts\launch-cortex-dev.bat   - live-rebuild dev loop
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
    pause
    exit /b 1
)

REM --- Bake "Cortex" into the embedded Windows resources for this build.
REM     build.rs reads WARP_APP_NAME and emits it as the FileDescription in
REM     the .rc file. Pair with rerun-if-env-changed=WARP_APP_NAME (added
REM     in build.rs) so toggling between prod and dev triggers a resource
REM     rebuild on incremental cargo runs.
set WARP_APP_NAME=Cortex

echo === Building release Cortex (this is slow on Windows; ~5-10 min cold) ===
echo Started: %date% %time%
echo.

cargo build --release --bin warp-oss --features gui,skip_login
if errorlevel 1 (
    echo.
    echo Build failed. Prod EXE not updated.
    pause
    exit /b 1
)

echo.
echo === Installing to %INSTALL_PATH% ===

if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"
copy /Y "target\release\warp-oss.exe" "%INSTALL_PATH%"
if errorlevel 1 (
    echo.
    echo Copy failed. Prod EXE not updated.
    pause
    exit /b 1
)

REM --- Write build-stamp so launch-cortex.cmd can detect when prod is
REM     stale relative to the current working tree. Format: simple key=val
REM     lines (commit/branch/dirty/built) so the launcher can parse with
REM     `findstr "^commit=" | for /f "tokens=2 delims==" ...`. If git is
REM     unavailable for any reason we just skip the stamp - the launcher
REM     handles a missing stamp by launching unconditionally.
set BUILD_COMMIT=
set BUILD_BRANCH=
for /f "delims=" %%I in ('git rev-parse HEAD 2^>nul') do set BUILD_COMMIT=%%I
for /f "delims=" %%I in ('git rev-parse --abbrev-ref HEAD 2^>nul') do set BUILD_BRANCH=%%I
set BUILD_DIRTY=no
git diff --quiet 2>nul
if errorlevel 1 set BUILD_DIRTY=yes
set BUILD_TIME=
for /f "delims=" %%I in ('powershell -NoProfile -Command "Get-Date -Format o"') do set BUILD_TIME=%%I

if defined BUILD_COMMIT (
    (
        echo commit=%BUILD_COMMIT%
        echo branch=%BUILD_BRANCH%
        echo dirty=%BUILD_DIRTY%
        echo built=%BUILD_TIME%
    ) > "%INSTALL_DIR%\Cortex.build-info"
)

echo.
echo === Done ===
echo Prod EXE:    %INSTALL_PATH%
echo Build stamp: %INSTALL_DIR%\Cortex.build-info  ^(commit %BUILD_COMMIT:~0,7%^)
echo Launcher:    scripts\launch-cortex.cmd
echo.
echo To set up Desktop and Start Menu shortcuts (with custom DEV-overlay
echo icon for the dev shortcut), run once:
echo     powershell -NoProfile -ExecutionPolicy Bypass -File scripts\install-shortcuts.ps1
echo Idempotent - re-run anytime to refresh shortcut targets/icons.
echo.
echo Finished: %date% %time%
REM CORTEX_NONINTERACTIVE=1 from the launcher's [R]ebuild path skips the
REM trailing pause so the launcher can immediately start the new EXE.
if not defined CORTEX_NONINTERACTIVE pause
endlocal
