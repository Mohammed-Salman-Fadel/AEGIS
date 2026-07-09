@echo off
REM installer\build-installer.bat — Build the AEGIS Windows NSIS installer
REM Prerequisites: NSIS installed, `makensis` on PATH

setlocal enabledelayedexpansion

echo ===== AEGIS Windows Installer Build =====
echo.

REM 1. Build engine release binary
echo [1/5] Building engine release binary...
call "%~dp0..\scripts\build-engine-release.bat" || exit /b 1
echo.

REM 2. Build CLI release binary
echo [2/5] Building CLI release binary...
call "%~dp0..\scripts\build-cli-release.bat" || exit /b 1
echo.

REM 3. Ensure frontend dist is built
echo [3/5] Building frontend...
cd /d "%~dp0..\frontend"
call npm install && call npm run build || exit /b 1
echo.

REM 4. Run NSIS
echo [4/5] Compiling NSIS installer...
cd /d "%~dp0"
makensis aegis-installer.nsi || exit /b 1
echo.

REM 5. Show result
set INSTALLER_EXE=%~dp0AEGIS-Windows-x64.exe
echo [5/5] Installer created:
if exist "%INSTALLER_EXE%" (
    echo   %INSTALLER_EXE%
    for %%I in ("%INSTALLER_EXE%") do echo   Size: %%~zI bytes
    echo.
    echo ===== Installer build complete =====
) else (
    echo   ERROR: Installer not found at expected path!
    exit /b 1
)
