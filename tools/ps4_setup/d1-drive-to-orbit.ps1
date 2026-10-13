param([Parameter(Mandatory)][string]$Out)
$ErrorActionPreference = 'Stop'
# Title screen -> setup:orbit only, then a frame; no pokes. For driving the Director by pad input.
New-Item -ItemType Directory -Force $Out | Out-Null
$log = Join-Path $Out 'driver.log'
$shad = 'F:\d1\tools\shadPS4\0.19.0\user\log\shad_log.txt'
function Say([string]$m) { $l = "$([DateTime]::UtcNow.ToString('o')) $m"; Add-Content -LiteralPath $log $l; $l }
# Only a log written since this driver started counts: the previous run's log stays on disk until
# the new emulator truncates it, and its stale lines once made the driver press keys at no window.
$driverStart = Get-Date
function WaitFor([string]$pattern, [int]$seconds) {
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) {
        if ((Test-Path $shad) -and (Get-Item $shad).LastWriteTime -gt $driverStart -and
            (Select-String -LiteralPath $shad -Pattern $pattern -Quiet)) { return $true }
        Start-Sleep -Seconds 2
    }
    Say "TIMEOUT waiting for '$pattern'"; return $false
}
if (-not (WaitFor "Entering state 'bootflow:(start|intro_screen_display)'" 300)) { exit 2 }
Start-Sleep 6; & (Join-Path $PSScriptRoot 'd1-pad.ps1') -Keys NumPad2 | ForEach-Object { Say $_ }
if (-not (WaitFor "Entering state 'setup:orbit' for" 600)) { exit 2 }
Start-Sleep 20
& (Join-Path $PSScriptRoot 'd1-shot.ps1') -Out (Join-Path $Out 'orbit.png') | ForEach-Object { Say $_ }
Say 'AT ORBIT'
