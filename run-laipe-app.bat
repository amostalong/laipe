@echo off
setlocal

rem ============================================================
rem  laipe app launcher (Tauri 2 desktop)
rem
rem  Usage:
rem    run-laipe-app.bat
rem
rem  Prerequisites (all already needed by the laipe stack):
rem    - bun >= 1.2  (frontend deps + JS-side tauri CLI)
rem    - Rust + the x86_64-pc-windows-msvc target
rem
rem  The JS-side tauri CLI (@tauri-apps/cli, installed by `bun install`)
rem  is used; we do NOT need the cargo-installed `cargo-tauri` binary.
rem
rem  First build takes 5-10 min (Tauri's dep tree is large).
rem  Subsequent builds are fast (incremental).
rem ============================================================

cd /d "%~dp0"

rem Check for bun
where bun >nul 2>&1
if errorlevel 1 (
    echo [error] bun not found in PATH.
    echo         Install from https://bun.sh and re-run.
    exit /b 1
)

if not exist "node_modules" (
    echo [setup] installing frontend deps...
    bun install
    if errorlevel 1 (
        echo [error] bun install failed.
        exit /b 1
    )
)

echo.
echo === laipe app (Tauri 2 desktop) ===
echo   Compiling Rust + Vite, opening native window.
echo   First build: ~5 min. Subsequent: seconds.
echo   Press Ctrl+C to stop.
echo.

cd laipe-app
bun run tauri:dev
set EXITCODE=%ERRORLEVEL%
endlocal & exit /b %EXITCODE%
