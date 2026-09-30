# Start the full loopback stack for the local Destiny PS3 test and boot RPCS3.
# Services: RPCN, DNS sinkhole, STUN stub, SignOn (auth3 + manifest), lobby probe, BAP
# stand-in, then RPCS3 (TEST048 copy: XInput pad, async shaders) and the GDB daemon.
# Already-running service processes are stopped first. Logs: SIGNON_LOCAL\logs\<Name>-*.log
param(
    [string]$Name = ("STACK_" + (Get-Date -Format 'yyyyMMdd_HHmmss')),
    [string[]]$BapArgs = @('--universe-time', 's', '--self-flags', '1;2@B0,C0-1023'),
    [string[]]$LobbyArgs = @(),
    [string[]]$SignonArgs = @(),
    [string[]]$StunArgs = @(),   # 01.24 answers the MessagePack probes; --reply-index picks the dispatch word
    [string[]]$DnsArgs = @(),   # e.g. '--stun-distinct' to give each STUN name its own address
    [switch]$NoGame
)
. "$PSScriptRoot\Paths.ps1"
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$root = $D1Local
$logs = "$root\SIGNON_LOCAL\logs"
$py = $D1Python
$rpcs3 = $D1Rpcs3
$usrdir = $D1Usrdir

Get-Process rpcs3 -ErrorAction SilentlyContinue | Stop-Process -Confirm:$false
Get-CimInstance Win32_Process -Filter "Name like 'python%'" |
    Where-Object { $_.CommandLine -match 'dns_sinkhole|stun_stub|signon_probe|lobby_probe|bap_server|rpcs3_gdb_daemon' } |
    ForEach-Object { Stop-Process -Id $_.ProcessId -Confirm:$false }
Start-Sleep 2

function Start-Service([string]$Tag, [string[]]$ArgList) {
    Start-Process -FilePath $py -ArgumentList (@('-u') + $ArgList) -WorkingDirectory $repo -WindowStyle Hidden `
        -RedirectStandardOutput "$logs\$Name-$Tag.log" -RedirectStandardError "$logs\$Name-$Tag.err"
}

if (-not (Get-Process rpcn -ErrorAction SilentlyContinue)) {
    Start-Process -FilePath "$root\RPCN_LOCAL\rpcn.exe" -WorkingDirectory "$root\RPCN_LOCAL" -WindowStyle Hidden
}
Start-Service dns (@('server/dns_sinkhole.py', '--destiny-hosts', '--log', "$logs\$Name-dns-queries.log") + $DnsArgs)
Start-Service stun (@('server/stun_stub.py') + $StunArgs)
Start-Service signon (@('server/signon_probe.py', '--mode', 'auth3', '--destiny-hosts',
    '--manifest-url', 'http://content.local.test/manifest', '--manifest-packages', "$usrdir\packages") + $SignonArgs)
Start-Service lobby (@('server/lobby_probe.py', '--timeout', '60') + $LobbyArgs)
Start-Service bap (@('server/bap_server.py', '--ws-signin',
    '--schema', "$root\SIGNON_LOCAL\investment_schema_character.json",
    '--store', "$root\SIGNON_LOCAL\characters.json") + $BapArgs)
Start-Sleep 3
Get-ChildItem $logs -Filter "$Name-*.err" | Where-Object Length -gt 0 | ForEach-Object {
    Write-Warning "$($_.Name):"; Get-Content $_.FullName -Tail 5
}
if ($NoGame) { return }

Start-Process -FilePath "$rpcs3\rpcs3.exe" -ArgumentList "$usrdir\EBOOT.BIN" -WorkingDirectory $rpcs3
$deadline = (Get-Date).AddSeconds(60)
do { Start-Sleep 1 } until ((Get-NetTCPConnection -State Listen -LocalPort 2345 -ErrorAction SilentlyContinue) -or (Get-Date) -gt $deadline)
# RPCS3's GDB stub serves one connection per emulator session: the daemon holds it.
Start-Service gdbd @('scripts/rpcs3_gdb_daemon.py')
Start-Sleep 3
Get-Content "$logs\$Name-gdbd.log", "$logs\$Name-bap.log"
