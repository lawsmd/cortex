@echo off
REM Cortex Windows installer driver.
REM
REM Builds the release-mode Cortex EXE and wraps it in an Inno Setup
REM installer that's the artifact you actually hand to an external user.
REM Output: dist\Cortex-Setup.exe (next to repo root).
REM
REM Distinct from scripts\install-cortex-prod.cmd, which is the contributor's
REM "build locally and copy to %LOCALAPPDATA%\Cortex" fast path. That script
REM stays the daily driver for iterating; THIS script is strictly for
REM producing a shareable artifact.
REM
REM What it does:
REM   1. cargo build --release --bin warp-oss --features gui,skip_login
REM   2. Rename target\release\warp-oss.exe -> target\release\Cortex.exe
REM      (the .iss references {#MyAppExeName} which we pass as Cortex.exe).
REM   3. script\windows\prepare_bundled_resources.ps1 populates
REM      target\release\resources\, which the .iss recurses into.
REM   4. iscc.exe script\windows\windows-installer.iss with the right /D
REM      preprocessor flags (MyAppName=Cortex, ReleaseChannel=oss, etc.).
REM   5. Move Output\Cortex-Setup.exe -> dist\Cortex-Setup.exe.
REM
REM Inno Setup 6 is required. Install from https://jrsoftware.org/isdl.php
REM (free), or via winget: `winget install JRSoftware.InnoSetup`.
REM
REM Code-signing: if you have an Authenticode cert and want to sign the
REM installer + bundled uninstaller, set CORTEX_SIGN_TOOL to a full SignTool
REM invocation before running this script, e.g.:
REM
REM   set CORTEX_SIGN_TOOL=signtool sign /fd sha256 /a /tr http://timestamp.digicert.com /td sha256 $f
REM   scripts\build-cortex-installer.cmd
REM
REM Leaving CORTEX_SIGN_TOOL unset skips signing (the installer still works,
REM but Windows SmartScreen will warn on first launch).

setlocal enableextensions
title Cortex (build Windows installer)

REM cd to repo root via the script's own location.
cd /d "%~dp0.."

set REPO_ROOT=%CD%
set INSTALLER_NAME=Cortex-Setup.exe
set DIST_DIR=%REPO_ROOT%\dist
set TARGET_DIR=%REPO_ROOT%\target\release
set ISS_PATH=%REPO_ROOT%\script\windows\windows-installer.iss

echo === Cortex installer build ===
echo Repo:        %REPO_ROOT%
echo .iss:        %ISS_PATH%
echo Output dir:  %DIST_DIR%
echo.

REM --- Locate iscc.exe.
REM     %ISCC% override wins if set. Otherwise probe the standard install
REM     locations (Inno Setup 6 64-bit on x64 Windows, 32-bit registry tree).
if defined ISCC (
    set ISCC_EXE=%ISCC%
    goto :iscc_found
)
set ISCC_EXE=
for %%P in (
    "%ProgramFiles(x86)%\Inno Setup 6\ISCC.exe"
    "%ProgramFiles%\Inno Setup 6\ISCC.exe"
    "%ProgramFiles(x86)%\Inno Setup 5\ISCC.exe"
    "%ProgramFiles%\Inno Setup 5\ISCC.exe"
) do (
    if exist "%%~P" set ISCC_EXE=%%~P
)
if "%ISCC_EXE%"=="" (
    where iscc.exe >nul 2>&1
    if not errorlevel 1 (
        for /f "delims=" %%P in ('where iscc.exe') do set ISCC_EXE=%%P
    )
)
:iscc_found
if "%ISCC_EXE%"=="" (
    echo error: Inno Setup compiler ^(iscc.exe^) not found.
    echo Install Inno Setup 6 from https://jrsoftware.org/isdl.php
    echo or set %%ISCC%% to the full path of ISCC.exe.
    exit /b 1
)
echo Inno Setup:  %ISCC_EXE%

REM --- Derive a version string.
REM     Honor CORTEX_INSTALLER_VERSION if set (CI passes the release tag).
REM     Otherwise `git describe --tags` if there's a tag in history.
REM     Otherwise fall back to "0.0.0-<short-sha>".
set CORTEX_VERSION=
if defined CORTEX_INSTALLER_VERSION (
    set CORTEX_VERSION=%CORTEX_INSTALLER_VERSION%
    goto :have_version
)
set _TMP=%TEMP%\cortex-installer-ver.tmp
git describe --tags --always --dirty > "%_TMP%" 2>nul
if exist "%_TMP%" (
    set /p CORTEX_VERSION=<"%_TMP%"
    del /q "%_TMP%" >nul 2>&1
)
if "%CORTEX_VERSION%"=="" (
    git rev-parse --short HEAD > "%_TMP%" 2>nul
    if exist "%_TMP%" (
        set /p _SHA=<"%_TMP%"
        del /q "%_TMP%" >nul 2>&1
    )
    if "%_SHA%"=="" set _SHA=unknown
    set CORTEX_VERSION=0.0.0-%_SHA%
)
:have_version
REM Strip a leading "v" so the AppVerName reads "Cortex 1.2.3" not "Cortex v1.2.3".
if "%CORTEX_VERSION:~0,1%"=="v" set CORTEX_VERSION=%CORTEX_VERSION:~1%
echo Version:     %CORTEX_VERSION%
echo.

REM --- Build release Cortex.
REM     Same flags as scripts\install-cortex-prod.cmd. Sets WARP_APP_NAME so
REM     the embedded Windows resource reads "Cortex" in file properties, and
REM     CARGO_FULL_PROFILE so build.rs drops conpty.dll/OpenConsole.exe under
REM     target\release\ (the location iscc expects them, via {#AssetsDir}).
set WARP_APP_NAME=Cortex
set CARGO_FULL_PROFILE=release

echo === cargo build --release --bin warp-oss --features gui,skip_login ===
cargo build --release --bin warp-oss --features gui,skip_login
if errorlevel 1 (
    echo.
    echo error: cargo build failed.
    exit /b 1
)
echo.

if not exist "%TARGET_DIR%\warp-oss.exe" (
    echo error: %TARGET_DIR%\warp-oss.exe missing after build.
    exit /b 1
)

REM --- Rename warp-oss.exe -> Cortex.exe in target\release\.
REM     The .iss [Files] copies "{#TargetProfileDir}\{#MyAppExeName}", and
REM     we pass MyAppExeName=Cortex.exe so all shortcut/registry references
REM     end up pointing at Cortex.exe. copy + del rather than move so an
REM     interrupted run doesn't leave target\release\ without warp-oss.exe.
echo === Staging Cortex.exe ===
copy /Y "%TARGET_DIR%\warp-oss.exe" "%TARGET_DIR%\Cortex.exe" >nul
if errorlevel 1 (
    echo error: copy warp-oss.exe -^> Cortex.exe failed.
    exit /b 1
)

REM --- Populate target\release\resources\ for the .iss recursive copy.
REM     The .iss has `Source: "{#TargetProfileDir}\resources\*"` which fails
REM     hard if the directory is missing. Upstream Warp's release pipeline
REM     invokes this script via script\windows\bundle.ps1; we call it
REM     directly here. Empty `resources/bundled/` is acceptable - the
REM     installer just won't ship those files.
echo === Staging resources ===
powershell -NoProfile -ExecutionPolicy Bypass -File "%REPO_ROOT%\script\windows\prepare_bundled_resources.ps1" -DestinationDir "%TARGET_DIR%\resources" -Channel oss -CargoProfile release
if errorlevel 1 (
    echo error: prepare_bundled_resources.ps1 failed.
    exit /b 1
)
echo.

REM --- Output directory (Inno writes to .iss-relative "Output\" by default;
REM     OutputDir override on the ISCC line redirects to our dist\).
if not exist "%DIST_DIR%" mkdir "%DIST_DIR%"

REM --- Invoke iscc.
REM     Architecture: hard-coded x64 (Cortex doesn't ship 32-bit anymore).
REM     Quoting: ISCC's /D switch wants the value bare; quotes are stripped
REM     by cmd before ISCC sees them only if the value has no spaces.
REM     Version strings contain dashes/dots but no spaces -> safe.
REM
REM     /Qp suppresses ISCC's per-line "Compiling..." chatter; errors still
REM     surface. Drop /Qp if you want full output for debugging.
set SIGN_FLAG=
if defined CORTEX_SIGN_TOOL (
    echo Signing tool: %CORTEX_SIGN_TOOL%
    set SIGN_FLAG=/Scodesign=%CORTEX_SIGN_TOOL% /DSIGN_TOOL=codesign
)

echo === Compiling installer ===
"%ISCC_EXE%" %SIGN_FLAG% ^
    /DMyAppName=Cortex ^
    /DMyAppVersion=%CORTEX_VERSION% ^
    /DMyAppExeName=Cortex.exe ^
    /DReleaseChannel=oss ^
    /DTargetProfileDir=target\release ^
    /DArch=x64 ^
    /DOutputName=Cortex-Setup ^
    /O"%DIST_DIR%" ^
    "%ISS_PATH%"
if errorlevel 1 (
    echo.
    echo error: iscc.exe failed.
    exit /b 1
)
echo.

if not exist "%DIST_DIR%\%INSTALLER_NAME%" (
    echo error: %DIST_DIR%\%INSTALLER_NAME% not produced.
    exit /b 1
)

echo === Done ===
echo Installer: %DIST_DIR%\%INSTALLER_NAME%
echo Version:   %CORTEX_VERSION%
echo.
echo To smoke-test on this machine:
echo     "%DIST_DIR%\%INSTALLER_NAME%"
echo and walk through the wizard. The installer respects the AppMutex, so
echo it'll prompt to close a running Cortex.exe first.

endlocal
