@echo off
REM Console shim for the GUI-subsystem prod Cortex.exe (cmd / PowerShell).
REM
REM Installed by scripts\install-cortex-prod.cmd to %LOCALAPPDATA%\Cortex\bin\
REM (which the installer also adds to the user PATH), so that the documented
REM `cortex orchestrate ...` invocation resolves from any shell.
REM
REM Why it exists: Cortex.exe is a Windows GUI-subsystem binary, so
REM interactive cmd/PowerShell launch it without waiting and never see its
REM exit code. Running it from inside a batch script makes cmd wait
REM synchronously and propagate %ERRORLEVEL%, while stdin/stdout/stderr pass
REM straight through to the child.
REM
REM Why it lives in bin\ rather than next to Cortex.exe: PATHEXT resolves
REM .EXE before .CMD, so a cortex.cmd sitting beside Cortex.exe would lose
REM to the GUI EXE when a shell looks up `cortex`. bin\ contains only the
REM shims, so `cortex` resolves here.
"%~dp0..\Cortex.exe" %*
exit /b %ERRORLEVEL%
