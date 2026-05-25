@echo off
REM Cortex prod staleness check (Windows). Compares the commit prod was
REM built from (read from %LOCALAPPDATA%\Cortex\Cortex.build-info, written
REM by install-cortex-prod.cmd) to the current HEAD. Reports state; if
REM prod is behind, prompts whether to rebuild.
REM
REM Was previously folded into launch-cortex.cmd, but that script is now
REM the end-user launch path and must not prompt or pause - end users
REM don't have a git checkout, so a staleness prompt is meaningless to
REM them. Run this manually from a dev terminal when you want to check
REM whether prod matches your working tree.

setlocal enableextensions

REM Resolve repo root from the script's own location.
for %%I in ("%~dp0..") do set "REPO=%%~fI"
set INSTALL_DIR=%LOCALAPPDATA%\Cortex
set PROD_EXE=%INSTALL_DIR%\Cortex.exe
set STAMP=%INSTALL_DIR%\Cortex.build-info

if not exist "%PROD_EXE%" (
    echo.
    echo Cortex prod isn't installed yet.
    echo Run:  scripts\install-cortex-prod.cmd
    echo.
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
set _TMP=%TEMP%\cortex-staleness.tmp

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

if not exist "%STAMP%" goto no_stamp
findstr "^commit=" "%STAMP%" > "%_TMP%" 2>nul
if not exist "%_TMP%" goto no_stamp
set _STAMP_LINE=
set /p _STAMP_LINE=<"%_TMP%"
del "%_TMP%" >nul 2>&1
REM "commit=" is 7 characters; everything after that is the hash. Done at
REM the top level (not inside a parens block) so the substring expansion
REM happens after `set /p` has populated _STAMP_LINE.
if defined _STAMP_LINE set BUILD=%_STAMP_LINE:~7%

if "%CURRENT%"=="" goto no_git
if "%BUILD%"=="" goto no_git
if /i "%CURRENT%"=="%BUILD%" goto current

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
choice /c RC /m "[R]ebuild now  [C]ancel"
if errorlevel 2 exit /b 0

call "%REPO%\scripts\install-cortex-prod.cmd"
exit /b %ERRORLEVEL%

:current
echo.
echo Prod is up to date with HEAD ^(%CURRENT_SHORT%^).
echo.
exit /b 0

:no_stamp
echo.
echo No build stamp found at %STAMP%.
echo Prod was likely built from a snapshot or older script. Can't determine
echo staleness.
echo.
exit /b 0

:no_git
echo.
echo Couldn't determine git state. Skipping staleness check.
echo.
exit /b 0
