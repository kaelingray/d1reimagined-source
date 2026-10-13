param(
    [Parameter(Mandatory)][string]$Out,
    [int]$ActivityId = 416,   # 56578: 416 = Elimination (407 is Salvage here; 01.29 numbering differs)
    [int]$WatchMinutes = 10,
    [int]$NudgeSeconds = 45
)
$ErrorActionPreference = 'Stop'
# From setup:orbit (a client already returned there), launch the activity again and reach
# activity:in_world with the same SYNTHETIC steps as d1-drive-in-world.ps1, then hold off the
# in-world AFK timer (error.inactivity_timed_out) with a short left-stick nudge every
# $NudgeSeconds while logging the sensor/spawn state. The log is memory-polled; the emulator log
# may be past its cap.
New-Item -ItemType Directory -Force $Out | Out-Null
$log = Join-Path $Out 'watch.log'
$guest = Join-Path $PSScriptRoot 'd1-winguest.py'
function Say([string]$m) { $l = "$([DateTime]::UtcNow.ToString('o')) $m"; Add-Content -LiteralPath $log $l; $l }
function Rung { (python $guest session 2>$null | Select-String 'goal').ToString() }
Say 'SYNTHETIC launch poke'; python $guest launch --activity-id $ActivityId --confirm | ForEach-Object { Say $_ }
Start-Sleep 60
Say 'SYNTHETIC task-7 suppress'; python $guest suppress --confirm | ForEach-Object { Say $_ }
Start-Sleep 30
Say 'SYNTHETIC rung-34 done-mask force'; python $guest masks --force-done --confirm | ForEach-Object { Say $_ }
$end = (Get-Date).AddMinutes($WatchMinutes)
while ((Get-Date) -lt $end) {
    Start-Sleep -Seconds $NudgeSeconds
    & (Join-Path $PSScriptRoot 'd1-pad.ps1') -Keys 'Hold:W:250' | Out-Null
    Say ("rung " + (Rung))
    Say ("pending " + ((python $guest sensor-clients 2>&1 | Select-String 'pending clients').ToString()))
    python $guest lifetime 2>$null | Select-Object -Last 1 | ForEach-Object { Say $_ }
    python $guest players 2>$null | Select-Object -Index 2 | ForEach-Object { Say $_ }
}
& (Join-Path $PSScriptRoot 'd1-shot.ps1') -Out (Join-Path $Out 'end.png') | ForEach-Object { Say $_ }
Say 'watch done'
