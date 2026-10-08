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
    %PWSH% "Start-Process cmd -ArgumentList '/c \"\"%~dpnx0\" %*\"' -Verb RunAs"
    exit /b
)

:run
cd /d "%~dp0"
echo ======================================================================
echo  League Mod Fixer - Standalone LTK Manager Repair Engine
echo ======================================================================
echo.

if exist "%~dp0lol-mod-fixer.exe" (
    "%~dp0lol-mod-fixer.exe" repair --pause %*
) else (
    echo Error: lol-mod-fixer.exe was not found in %~dp0
    pause
)
