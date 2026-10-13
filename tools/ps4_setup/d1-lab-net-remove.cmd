@echo off
rem Right-click > Run as administrator. Undoes d1-lab-net.cmd (back to 127.0.0.1 redirects, full block).
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0d1-lab-net.ps1" -Mode remove
