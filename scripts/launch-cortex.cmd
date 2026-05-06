@echo off
REM Cortex prod launcher (Windows). Daily-driver entry point.
REM Pin this (or %LOCALAPPDATA%\Cortex\Cortex.exe directly) to the taskbar.
REM
REM Smart-launch behavior: before opening prod, compares the commit prod
REM was built from (read from Cortex.build-info, written by
REM install-cortex-prod.cmd) to the current HEAD. If they match, launches
REM silently. If prod is behind, prompts:
REM   [L]aunch existing prod  [R]ebuild then launch  [C]ancel
REM
REM Why: prod is a copy of the EXE, decoupled from target/. Cloud agents
REM and dev rebuilds never touch it - which means it can also silently
REM fall behind. This check makes drift visible without forcing a rebuild
REM every launch.

setlocal enableextensions

REM Resolve repo root from the script's own location so the launcher works
REM regardless of where the user installed Cortex (or after a Windows
REM reinstall onto a different drive). %~dp0 is this script's directory
REM with a trailing backslash; %%~fI canonicalizes the parent path.
for %%I in ("%~dp0..") do set "REPO=%%~fI"
set INSTALL_DIR=%LOCALAPPDATA%\Cortex
set PROD_EXE=%INSTALL_DIR%\Cortex.exe
set STAMP=%INSTALL_DIR%\Cortex.build-info

if not exist "%PROD_EXE%" (
    echo.
    echo Cortex prod isn't installed yet.
    echo Run:  scripts\install-cortex-prod.cmd
    echo.
    pause
    exit /b 1
)

REM --- Determine staleness ---
REM We avoid cmd's `for /f ('git ...')` capture pattern - empirically it
REM injects a leading 0x0C byte into the captured value on Git for Windows
REM output, breaking subsequent `==` equality checks. Routing through a
REM temp file + `set /p` gives a clean, CRLF-stripped string.
set CURRENT=
set CURRENT_SHORT=
set BUILD=
set BUILD_SHORT=
set _TMP=%TEMP%\cortex-launch.tmp

git -C "%REPO%" rev-parse HEAD > "%_TMP%" 2>nul
if exist "%_TMP%" (
    set /p CURRENT=<"%_TMP%"
    del "%_TMP%" >nul 2>&1
)

git -C "%REPO%" rev-parse --short HEAD > "%_TMP%" 2>nul
if exist "%_TMP%" (
    set /p CURRENT_SHORT=<"%_TMP%"
    del "%_TMP%" >nul 2>&1
)

if not exist "%STAMP%" goto skip_stamp
findstr "^commit=" "%STAMP%" > "%_TMP%" 2>nul
if not exist "%_TMP%" goto skip_stamp
set _STAMP_LINE=
set /p _STAMP_LINE=<"%_TMP%"
del "%_TMP%" >nul 2>&1
REM "commit=" is 7 characters; everything after that is the hash. Done at
REM the top level (not inside a parens block) so the substring expansion
REM happens after `set /p` has populated _STAMP_LINE.
if defined _STAMP_LINE set BUILD=%_STAMP_LINE:~7%
:skip_stamp

REM Bail out of the staleness check if we can't determine either side - just launch.
if "%CURRENT%"=="" goto launch
if "%BUILD%"=="" goto launch
if /i "%CURRENT%"=="%BUILD%" goto launch

REM --- Stale: count how many commits behind, get short hash for display ---
set AHEAD=

git -C "%REPO%" rev-list --count "%BUILD%..HEAD" > "%_TMP%" 2>nul
if exist "%_TMP%" (
    set /p AHEAD=<"%_TMP%"
    del "%_TMP%" >nul 2>&1
)

git -C "%REPO%" rev-parse --short "%BUILD%" > "%_TMP%" 2>nul
if exist "%_TMP%" (
    set /p BUILD_SHORT=<"%_TMP%"
    del "%_TMP%" >nul 2>&1
)

if "%AHEAD%"=="" set AHEAD=?
if "%BUILD_SHORT%"=="" set BUILD_SHORT=%BUILD%

echo.
echo Prod is %AHEAD% commit(s) behind your working tree.
echo   prod commit:   %BUILD_SHORT%
echo   current HEAD:  %CURRENT_SHORT%
echo.
choice /c LRC /m "[L]aunch existing prod  [R]ebuild then launch  [C]ancel"
if errorlevel 3 exit /b 0
if errorlevel 2 goto rebuild

:launch
start "" "%PROD_EXE%" %*
exit /b 0

:rebuild
REM CORTEX_NONINTERACTIVE skips the installer's trailing `pause` so we
REM can immediately launch after a successful rebuild.
set CORTEX_NONINTERACTIVE=1
call "%REPO%\scripts\install-cortex-prod.cmd"
if errorlevel 1 (
    echo.
    echo Rebuild failed; not launching.
    pause
    exit /b 1
)
start "" "%PROD_EXE%" %*
exit /b 0
