@echo off
rem Right-click > Run as administrator. Sets up the isolated D1 lab address (10.77.0.2) for shadPS4.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0d1-lab-net.ps1" -Mode apply
