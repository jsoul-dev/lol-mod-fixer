@echo off
title League Mod Fixer (Rose Skins Auto-Repair)
setlocal

set PWSH="%SYSTEMROOT%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -Command

:: Check if already running as Administrator
%PWSH% "([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)" | findstr "True" >nul

if %errorLevel% == 0 (
    goto :run
) else (
    echo Requesting Administrator privileges via UAC to unlock restricted Rose folders...
    %PWSH% "Start-Process cmd -ArgumentList '/k \"\"%~dpnx0\" %*\"' -Verb RunAs"
    exit /b
)

:run
cd /d "%~dp0"
echo =========================================================
echo  League Mod Fixer - Standalone LTK Manager Repair Engine
echo =========================================================
echo.

set "EXE="

if exist "%~dp0lol-mod-fixer.exe" (
    set "EXE=%~dp0lol-mod-fixer.exe"
) else if exist "%~dp0lol-mod-fixer-static.exe" (
    set "EXE=%~dp0lol-mod-fixer-static.exe"
) else if exist "%~dp0..\lol-mod-fixer.exe" (
    set "EXE=%~dp0..\lol-mod-fixer.exe"
) else if exist "%~dp0..\dist\lol-mod-fixer.exe" (
    set "EXE=%~dp0..\dist\lol-mod-fixer.exe"
) else if exist "%~dp0..\target\release\lol-mod-fixer.exe" (
    set "EXE=%~dp0..\target\release\lol-mod-fixer.exe"
) else (
    for %%F in ("%~dp0lol-mod-fixer*.exe") do (
        set "EXE=%%F"
    )
)

if defined EXE (
    "%EXE%" repair --pause %*
    if errorlevel 1 (
        echo.
        echo =========================================================
        echo  lol-mod-fixer finished with an error or warning code.
        echo =========================================================
        pause
    )
) else (
    echo Error: Could not find lol-mod-fixer.exe in:
    echo   "%~dp0"
    echo.
    echo Please make sure lol-mod-fixer.exe is placed in the same folder as this script.
    echo.
    pause
)
