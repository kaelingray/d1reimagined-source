# Run one local Destiny PS3 comparison end to end (loopback services only).
# Makes a fresh RPCS3 copy from the TEST017 baseline (keyboard pad, game CA + unsigned
# manifest patches), restarts server/bap_server.py with -BapArgs, boots the game, presses
# X at the title screen, waits, takes an in-emulator screenshot and prints the BAP log.
# DNS, STUN, RPCN, signon_probe.py and lobby_probe.py must already be running.
param(
    [Parameter(Mandatory)] [string]$Name,
    [string[]]$BapArgs = @(),
    [int]$WaitSeconds = 60
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$root = 'D:\Destiny360'
$logs = "$root\SIGNON_LOCAL\logs"
$dest = "$root\RPCS3_$Name"
Set-Location $repo

Get-Process rpcs3 -ErrorAction SilentlyContinue | Stop-Process -Confirm:$false
Get-CimInstance Win32_Process -Filter "Name like 'python%'" |
    Where-Object CommandLine -like '*bap_server.py*' | ForEach-Object { Stop-Process -Id $_.ProcessId -Confirm:$false }
Start-Sleep 2
python server/prepare_rpcs3_copy.py "$root/RPCS3_TEST017_LOBBY_HEADER" $dest `
    --game-ca "$root/SIGNON_LOCAL/ca_v4/ca.cert.pem" --unsigned-manifest | Out-Host
$bapLog = "$logs\$Name-bap.log"
Start-Process -FilePath (Get-Command python).Source -ArgumentList (@('-u', 'server/bap_server.py') + $BapArgs) `
    -WorkingDirectory $repo -WindowStyle Hidden -RedirectStandardOutput $bapLog -RedirectStandardError "$logs\$Name-bap.err"
Start-Sleep 2
Start-Process -FilePath "$dest\rpcs3.exe" -ArgumentList "$root\01_WORKING\ps3_destiny\PS3_GAME\USRDIR\EBOOT.BIN" -WorkingDirectory $dest

# The title screen is up shortly after the game's STUN probe. RPCS3 locks its own log, so
# watch the STUN stub's log (newest stun_stub*/test*-stun.log) for a line after launch.
$launched = Get-Date
$stunLog = Get-ChildItem $logs -Filter '*stun.log' | Sort-Object LastWriteTime | Select-Object -Last 1
$deadline = $launched.AddMinutes(4)
do {
    Start-Sleep 2
    $last = Get-Content $stunLog.FullName -Tail 1
    $stamp = [datetime]::MinValue
    $ready = $last -and [datetime]::TryParse($last.Split(' ')[0], [ref]$stamp) -and $stamp -gt $launched
} until ($ready -or (Get-Date) -gt $deadline)
Start-Sleep 15
& "$PSScriptRoot\Send-RpcsKey.ps1" -Count 1 | Out-Null
Start-Sleep $WaitSeconds
& "$PSScriptRoot\Send-RpcsKey.ps1" -Count 1 -Vk 0x7B | Out-Null
Start-Sleep 2
$shot = Get-ChildItem "$dest\screenshots" -Recurse -File -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 1
"screenshot: $($shot.FullName)"
Get-Content $bapLog, "$logs\$Name-bap.err" | ForEach-Object { if ($_.Length -gt 220) { $_.Substring(0, 220) } else { $_ } }
