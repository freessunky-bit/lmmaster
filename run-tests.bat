@echo off
chcp 65001 >nul 2>&1
REM ============================================================
REM  LMmaster - Full verification (cargo + vitest + tsc + clippy + fmt)
REM  Expected: cargo 845 + vitest 251 = 1096 tests / 0 failed.
REM ============================================================

setlocal

cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%LOCALAPPDATA%\pnpm;%PATH%"

echo.
echo ====================================================
echo   LMmaster Full Verification
echo ====================================================
echo.

echo [1/6] cargo fmt --all -- --check
cargo fmt --all -- --check
if errorlevel 1 (
    echo [warn] fmt diff detected. Run: cargo fmt --all
)

echo.
echo [2/6] cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets -- -D warnings
if errorlevel 1 (
    echo [error] clippy failed.
    pause
    exit /b 1
)

echo.
echo [3/6] cargo test --workspace
cargo test --workspace
if errorlevel 1 (
    echo [error] cargo test failed.
    pause
    exit /b 1
)

echo.
echo [4/6] frontend type-check (tsc -b)
cd apps\desktop
call pnpm exec tsc -b --clean
call pnpm exec tsc -b
if errorlevel 1 (
    echo [error] TypeScript errors.
    pause
    exit /b 1
)

echo.
echo [5/6] vitest run
call pnpm exec vitest run
if errorlevel 1 (
    echo [error] vitest failed.
    pause
    exit /b 1
)

echo.
echo [6/6] cleanup stale .js artifacts under src/
for /r src %%f in (*.js) do (
    echo %%f | findstr /V "vite.config.js vitest.config.js" >nul && del "%%f"
)

cd ..\..

echo.
echo ====================================================
echo   All verification passed. 0 failed.
echo ====================================================
pause

endlocal
