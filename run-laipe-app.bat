@echo off
setlocal

rem ============================================================
rem  laipe app launcher (Tauri 2 desktop)
rem
rem  Usage:
rem    run-laipe-app.bat
rem
rem  One-time prereq: tauri-cli
rem    cargo install tauri-cli --version "^2.0" --locked
rem
rem  First build takes 5-10 min (Tauri's dep tree is large).
rem  Subsequent builds are fast (incremental).
rem ============================================================

cd /d "%~dp0"

rem Check for tauri-cli
where cargo-tauri >nul 2>&1
if errorlevel 1 (
    echo [setup] cargo-tauri not found.
    echo         Install with:  cargo install tauri-cli --version "^2.0" --locked
    echo         ^(one-time, ~5-10 min^)
    echo.
    set /p INSTALL="Install now? [y/N]: "
    if /I "%INSTALL%"=="Y" (
        cargo install tauri-cli --version "^2.0" --locked
        if errorlevel 1 (
            echo [error] tauri-cli install failed.
            exit /b 1
        )
    ) else (
        echo [error] tauri-cli required. Aborting.
        exit /b 1
    )
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
