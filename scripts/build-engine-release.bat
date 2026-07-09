@echo off
REM scripts/build-engine-release.bat — Build the AEGIS engine as a standalone .exe
REM Usage: scripts\build-engine-release.bat

setlocal enabledelayedexpansion

echo ===== AEGIS Engine Release Build =====
echo.

REM 1. Ensure frontend is fresh
echo [1/5] Building frontend...
cd /d "%~dp0..\frontend"
call npm install || exit /b 1
call npm run build || exit /b 1
echo Frontend built successfully.
echo.

REM 2. Build the engine binary in release mode
echo [2/5] Compiling engine (release)...
cd /d "%~dp0..\engine"
cargo build --release || exit /b 1
echo Engine compiled successfully.
echo.

REM 3. Locate the output binary
set ENGINE_EXE=%~dp0..\engine\target\release\aegis-engine.exe
if not exist "%ENGINE_EXE%" (
    echo ERROR: Expected engine binary at %ENGINE_EXE%
    exit /b 1
)

REM 4. Show binary info
echo [3/5] Binary created at:
echo   %ENGINE_EXE%
for %%I in ("%ENGINE_EXE%") do echo   Size: %%~zI bytes
echo.

REM 5. Copy to staging directory
set STAGING_DIR=%~dp0..\build\release
echo [4/5] Copying to staging: %STAGING_DIR%
mkdir "%STAGING_DIR%" 2>nul
copy /Y "%ENGINE_EXE%" "%STAGING_DIR%\" || exit /b 1
echo Done.
echo.

REM 6. Verify the binary
echo [5/5] Verifying binary...
"%STAGING_DIR%\aegis-engine.exe" --version 2>nul || (
    echo NOTE: --version flag may not exist; verify by checking the binary runs.
)
echo.
echo ===== Engine release build complete =====
echo Output: %STAGING_DIR%\aegis-engine.exe
