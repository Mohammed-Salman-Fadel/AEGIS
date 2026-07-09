@echo off
REM scripts/build-cli-release.bat — Build the AEGIS CLI as a standalone .exe
REM Usage: scripts\build-cli-release.bat

setlocal enabledelayedexpansion

echo ===== AEGIS CLI Release Build =====
echo.

REM 1. Build the CLI binary in release mode
echo [1/4] Compiling CLI (release)...
cd /d "%~dp0..\cli"
cargo build --release || exit /b 1
echo CLI compiled successfully.
echo.

REM 2. Locate the binary
set CLI_EXE=%~dp0..\cli\target\release\aegis.exe
if not exist "%CLI_EXE%" (
    echo ERROR: Expected CLI binary at %CLI_EXE%
    exit /b 1
)

REM 3. Show binary info
echo [2/4] Binary created at:
echo   %CLI_EXE%
for %%I in ("%CLI_EXE%") do echo   Size: %%~zI bytes
echo.

REM 4. Copy to staging
set STAGING_DIR=%~dp0..\build\release
echo [3/4] Copying to staging: %STAGING_DIR%
mkdir "%STAGING_DIR%" 2>nul
copy /Y "%CLI_EXE%" "%STAGING_DIR%\" || exit /b 1
echo Done.
echo.

REM 5. Verify
echo [4/4] Verifying binary...
"%STAGING_DIR%\aegis.exe" --help || exit /b 1
echo.
echo ===== CLI release build complete =====
echo Output: %STAGING_DIR%\aegis.exe
