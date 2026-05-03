@echo off
chcp 65001 >nul 2>&1
setlocal EnableDelayedExpansion

REM ============================================================
REM  LMmaster - Dev mode (Tauri 2 + Vite + Rust hot reload)
REM
REM  All .cmd-script invocations (pnpm) are wrapped in `call`
REM  to prevent the parent batch from terminating prematurely.
REM ============================================================

cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%LOCALAPPDATA%\pnpm;%PATH%"
set "LOG=%~dp0run-dev.log"

echo. > "%LOG%"
echo === LMmaster run-dev start === >> "%LOG%"

echo.
echo ====================================================
echo   LMmaster Dev Mode
echo   cwd: %CD%
echo   log: %LOG%
echo ====================================================
echo.

echo [step 1/4] check cargo
where cargo >> "%LOG%" 2>&1
cargo --version
if errorlevel 1 (
    echo [error] cargo not on PATH. Install Rust: https://rustup.rs
    echo cargo missing >> "%LOG%"
    goto :end
)

echo.
echo [step 2/4] check pnpm
where pnpm >> "%LOG%" 2>&1
call pnpm --version
if errorlevel 1 (
    echo [error] pnpm not on PATH. Run: corepack enable
    echo pnpm missing >> "%LOG%"
    goto :end
)

echo.
echo [step 3/4] pnpm install
echo --- pnpm install --- >> "%LOG%"
call pnpm install >> "%LOG%" 2>&1
if errorlevel 1 (
    echo [error] pnpm install failed. See %LOG%
    goto :end
)
echo pnpm install OK

echo.
echo [pre] cleanup leftover dev processes (vite port 1420 / lmmaster-desktop.exe)
echo --- pre cleanup --- >> "%LOG%"
REM If a previous batch did not exit cleanly, vite may still hold port 1420.
REM Find the PID listening on :1420 (5th token from netstat -ano) and taskkill.
for /f "tokens=5" %%a in ('netstat -ano 2^>nul ^| findstr :1420 ^| findstr LISTENING') do (
    echo killing PID %%a holding port 1420 >> "%LOG%"
    taskkill /F /PID %%a >nul 2>&1
)
taskkill /F /IM lmmaster-desktop.exe /T >nul 2>&1
REM Brief wait for OS to release the port.
timeout /t 1 /nobreak >nul

echo.
echo [step 4/4] tauri:dev
echo (First build: 5-15 minutes for Rust release compile.)
echo (Look for "gateway://ready port=NNNNN" + desktop window.)
echo.

call pnpm --filter @lmmaster/desktop tauri:dev
set "TAURI_EXIT=%errorlevel%"
echo.
echo tauri:dev exited with code %TAURI_EXIT%
echo tauri:dev exit %TAURI_EXIT% >> "%LOG%"

:end
echo.
echo ====================================================
echo   Done. Press any key to close.
echo ====================================================
pause
endlocal
