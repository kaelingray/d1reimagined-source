# One scripted input step for the local Destiny RPCS3 run: post keys (keyboard pad: X Cross,
# C Circle, Return Start, arrows D-pad, WASD left stick), wait, then take an in-emulator F12
# screenshot and print its path. F12 captures the real frame even when the desktop is locked,
# unlike PrintWindow.
#   Invoke-RpcsStep.ps1 -Keys X
#   Invoke-RpcsStep.ps1 -Keys D -HoldMs 520 -WaitSeconds 1
param(
    [string[]]$Keys = @(),
    [int]$HoldMs = 150,
    [int]$WaitSeconds = 4
)
$shots = 'D:\Destiny360\RPCS3_TEST048_SETUP\screenshots\BLES01857'
if ($Keys) { & "$PSScriptRoot\Send-RpcsKey.ps1" -Keys $Keys -HoldMs $HoldMs | Out-Null }
Start-Sleep $WaitSeconds
$before = (Get-ChildItem $shots -File -ErrorAction SilentlyContinue | Where-Object Length -gt 0 | Measure-Object).Count
& "$PSScriptRoot\Send-RpcsKey.ps1" -Keys F12 | Out-Null
$deadline = (Get-Date).AddSeconds(10)
do {
    Start-Sleep -Milliseconds 500
    $valid = @(Get-ChildItem $shots -File -ErrorAction SilentlyContinue | Where-Object Length -gt 0)
} until (($valid.Count -gt $before) -or (Get-Date) -gt $deadline)
if ($valid.Count -le $before) { throw 'RPCS3 did not save a complete screenshot within 10 seconds' }
($valid | Sort-Object LastWriteTime | Select-Object -Last 1).FullName
