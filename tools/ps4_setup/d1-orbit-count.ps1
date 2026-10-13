param([Parameter(Mandatory)][string]$Out, [Parameter(Mandatory)][string]$Addrs)
$ErrorActionPreference = 'Stop'
# Title -> orbit with ad-hoc entry/branch counters installed BEFORE orbit loads (D1_COUNT_ANY),
# so build-once code at orbit entry is counted. Reads at orbit, leaves the hooks for a later read.
New-Item -ItemType Directory -Force $Out | Out-Null
$log = Join-Path $Out 'driver.log'
$shad = 'F:\d1\tools\shadPS4\0.19.0\user\log\shad_log.txt'
function Say([string]$m) { $l = "$([DateTime]::UtcNow.ToString('o')) $m"; Add-Content -LiteralPath $log $l; $l }
function WaitFor([string]$pattern, [int]$seconds) {
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) {
        if (Select-String -LiteralPath $shad -Pattern $pattern -Quiet) { return $true }
        Start-Sleep -Seconds 2
    }
    Say "TIMEOUT waiting for '$pattern'"; return $false
}
if (-not (WaitFor "Entering state 'bootflow:start'" 300)) { exit 2 }
$env:D1_COUNT_ANY = '1'; $env:D1_COUNT_ADDRS = $Addrs
for ($try = 0; $try -lt 3; $try++) { $o = python (Join-Path $PSScriptRoot 'd1-wincount.py') install 2>&1; $o | ForEach-Object { Say $_.ToString() }; if ($o -match 'hooked') { break }; Start-Sleep 3 }
Start-Sleep 6; & (Join-Path $PSScriptRoot 'd1-pad.ps1') -Keys NumPad2 | ForEach-Object { Say $_ }
if (-not (WaitFor "Entering state 'setup:orbit' for" 600)) { exit 2 }
Start-Sleep 20
python (Join-Path $PSScriptRoot 'd1-wincount.py') read | ForEach-Object { Say $_ }
Say 'AT ORBIT'
