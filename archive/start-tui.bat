@echo off
chcp 65001 >nul
title Clarity TUI
cd /d "%~dp0"

echo [Clarity TUI Launcher]
echo Reading Kimi credentials...

:: Read OAuth token from Kimi CLI credentials
for /f "usebackq delims=" %%a in (`powershell -NoProfile -Command "(Get-Content '%USERPROFILE%\.kimi\credentials\kimi-code.json' | ConvertFrom-Json).access_token"`) do set "KIMI_CODE_API_KEY=%%a"

set "KIMI_CODE_BASE_URL=https://api.kimi.com/coding/v1"

echo Launching clarity-tui...
echo.

:: Run TUI directly (inherits current console for alternate screen buffer)
"%~dp0target\release\clarity-tui.exe"

echo.
echo TUI exited.
pause
