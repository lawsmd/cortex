@echo off
REM Cortex prod launcher (Windows). Thin wrapper around the installed EXE.
REM
REM Most users launch Cortex via the Desktop / Start Menu shortcut created
REM by scripts\install-shortcuts.ps1, which targets Cortex.exe directly
REM (no cmd window). This script exists for the cases where someone wants
REM to launch from a terminal, and as a delegate for any tooling that
REM still references it.
REM
REM The staleness check that used to live here moved to
REM scripts\check-cortex-staleness.cmd. End users don't have a git
REM checkout, so a staleness prompt on every launch was meaningless to
REM the prod audience. Run that script manually if you want to verify
REM prod matches your working tree.

setlocal enableextensions

set INSTALL_DIR=%LOCALAPPDATA%\Cortex
set PROD_EXE=%INSTALL_DIR%\Cortex.exe

if not exist "%PROD_EXE%" (
    echo.
    echo Cortex prod isn't installed yet.
    echo Run:  scripts\install-cortex-prod.cmd
    echo.
    pause
    exit /b 1
)

REM `start ""` detaches Cortex.exe from this cmd process so the script can
REM exit immediately. Cortex.exe is GUI-subsystem (verified at install
REM time by install-cortex-prod.cmd), so no console window is spawned.
REM
REM Any args passed to this script are forwarded to Cortex.exe.
start "" "%PROD_EXE%" %*
exit /b 0
