@echo off
setlocal

set "SETUP_SCRIPT=%~dp0scripts\setup-and-run.ps1"
if not exist "%SETUP_SCRIPT%" (
    echo STATEFLOW setup script was not found:
    echo %SETUP_SCRIPT%
    pause
    exit /b 1
)

powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%SETUP_SCRIPT%"
set "EXIT_CODE=%ERRORLEVEL%"

if not "%EXIT_CODE%"=="0" (
    echo.
    echo STATEFLOW setup or startup failed. See the messages above.
    pause
)

exit /b %EXIT_CODE%
