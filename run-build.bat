@echo off
chcp 65001 >nul 2>&1
REM ============================================================
REM  LMmaster - Production build (Windows NSIS installer)
REM
REM  Notes:
REM   - No code signing certificate yet (SmartScreen will warn).
REM   - If minisign pubkey is placeholder, build may fail.
REM     Workaround: set plugins.updater.active=false in tauri.conf.json
REM     and bundle.createUpdaterArtifacts=false.
REM   - First build: 5-15 minutes.
REM   - Output: apps/desktop/src-tauri/target/release/bundle/nsis/
REM ============================================================

setlocal

cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%LOCALAPPDATA%\pnpm;%PATH%"

echo.
echo ====================================================
echo   LMmaster Installer Build
echo ====================================================
echo.

call pnpm install
if errorlevel 1 (
    echo [error] pnpm install failed.
    pause
    exit /b 1
)

echo.
echo [build] tauri:build starting...
echo (Rust release compile may pause output for several minutes - this is normal.)
echo.

call pnpm --filter @lmmaster/desktop tauri:build
if errorlevel 1 (
    echo.
    echo [error] tauri:build failed. Check the log above.
    echo.
    echo Common causes:
    echo   1. plugins.updater.pubkey is placeholder (TODO_REPLACE_...).
    echo      Fix: in tauri.conf.json set plugins.updater.active=false
    echo           and bundle.createUpdaterArtifacts=false.
    echo   2. NSIS auto-download failed (check internet).
    echo   3. icons/icon.ico missing.
    pause
    exit /b 1
)

echo.
echo ====================================================
echo   Build complete.
echo ====================================================
echo.
echo Output:
echo   apps\desktop\src-tauri\target\release\bundle\nsis\
echo.
echo Installer files:
dir /B "apps\desktop\src-tauri\target\release\bundle\nsis\*.exe" 2>nul
echo.
echo Double-click the .exe to install. SmartScreen warning is expected
echo (unsigned bundle). Click "More info" then "Run anyway".
echo.
pause

endlocal
