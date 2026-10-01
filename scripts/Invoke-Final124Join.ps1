# One unattended 01.24 activity-join experiment (about 8 minutes): boot through
# Invoke-Final124Run.ps1, log in with the stored character (setup screens are skipped once the
# store holds its account user data), answer the post-prologue matchmaking poll with a match,
# point the client at an activity host proxy stand-in (127.1.1.128:12851) and sample the
# bootflow while the join runs. The stack is left running (-KeepRpcs3); -Stop closes it,
# including the proxy, which is not in the run's ownership manifest.
#
#   pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 -Name FINAL124_JOIN1 -Local <runtime> -DataRepo <data>
#   pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 -Name FINAL124_JOIN2 -World 231 -ProxyExtra '--ah-zone;112:0x811c9dc5'
#   pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 -Name FINAL124_JOIN1 -Stop
#
# Defaults are the furthest recipe so far (FINAL124_MM8, notes/checkpoint-2026-10-01-profile.md):
# empty mm rsp field 4, ahp rsp to the proxy, all unlock flags, universe flag 215 = 1, and a
# proxy that joins, grants entity indices and sends the activity state for
# act_cosmo_m0_highway, world (activity) 231. -BapExtra / -ProxyExtra are semicolon
# separated and appended last, so single-value options override the defaults.
param(
    [Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9][A-Za-z0-9_.-]*$')][string]$Name,
    [string]$Local,
    [string]$DataRepo,
    [string]$Map = 'act_cosmo_m0_highway',
    [int]$World = 231,
    [string]$BapExtra = '',
    [string]$ProxyExtra = '',
    [ValidateRange(5, 3600)][int]$AfterMatchSeconds = 60,
    [switch]$Stop
)
$ErrorActionPreference = 'Stop'
if ($Local) { $env:D1_LOCAL = (Resolve-Path -LiteralPath $Local).Path }
. "$PSScriptRoot\Paths.ps1"
$env:D1_RPCS3 = $D1Rpcs3 = Join-Path $D1Local 'RPCS3_FINAL124_SMOKE001'
$D1Shots = Join-Path $D1Rpcs3 'screenshots\BLES01857'
. "$PSScriptRoot\Final124RunHelpers.ps1"
if (-not $DataRepo) { $DataRepo = if ($env:D1_DATA_REPO) { $env:D1_DATA_REPO } else { Join-Path (Split-Path $D1Repo) 'd1xenia-data' } }
$DataRepo = (Resolve-Path -LiteralPath $DataRepo).Path
$logs = Join-Path $D1Local 'SIGNON_LOCAL\logs'
$research = Join-Path $D1Local 'RESEARCH\ps3_updates'
$proxyPidFile = Join-Path $logs "$Name-proxy.pid"

if ($Stop) {
    $owned = Read-D1ProcessManifest (Join-Path $logs "$Name-processes.json")
    foreach ($process in ($owned.processes | Sort-Object { $_.tag -ne 'gdbd' })) { Stop-D1OwnedProcess $process }
    if (Test-Path $proxyPidFile) {
        $proxyPid = [int](Get-Content $proxyPidFile)
        $proxy = Get-CimInstance Win32_Process -Filter "ProcessId=$proxyPid"
        if ($proxy -and $proxy.CommandLine -match 'bap_server\.py.*127\.1\.1\.128') { Stop-Process -Id $proxyPid -Confirm:$false }
    }
    Copy-D1SharedFileSnapshot -Source (Join-Path $D1Rpcs3 'log\RPCS3.log') -Destination (Join-Path $logs "$Name-rpcs3-final.log") | Out-Null
    "stopped $Name"
    return
}

$types = Join-Path $DataRepo 'ps3_updates\types_01.24.json'
$services = Join-Path $DataRepo 'ps3_updates\services_01.24.json'
$bapArgs = '--mm-rsp;4=;--ahp-reply;00000000000000007f01018000003233;--self-flags;2;--universe-flag-values;215=1'
if ($BapExtra) { $bapArgs += ";$BapExtra" }
# c->ah mgr rsp: 02 | u64 host id 1 | char[128] host name (137 bytes, decoder 0x415be4 in 01.00)
$mgr = [byte[]](@(2, 0, 0, 0, 0, 0, 0, 0, 1) + [Text.Encoding]::ASCII.GetBytes('d1local-host') + (,0 * 116))
$proxyArgs = @('--ah-mgr-reply', [Convert]::ToHexString($mgr).ToLower(), '--ah-join', '--ah-entities', '8192:12288',
    '--ah-state', '--ah-map', $Map, '--ah-world', "$World", '--ah-checksum', '0x79130968', '--ah-bubbles', '22',
    '--activity-schema', $types, '--schema', $types, '--services-schema', $services) +
    @($ProxyExtra -split ';' | Where-Object { $_ })

$launch = Start-Job -ScriptBlock { param($script, $n, $l, $d, $b)
    $env:D1_LOCAL = $l
    & pwsh -NoProfile -File $script -Name $n -Local $l -DataRepo $d -BootSeconds 250 -PressSeconds 110 `
        -ContinueAfterBap -KeepRpcs3 -BapExtra $b 2>&1 | Select-Object -Last 5
} -ArgumentList (Join-Path $PSScriptRoot 'Invoke-Final124Run.ps1'), $Name, $D1Local, $DataRepo, $bapArgs
$bap = Join-Path $logs "$Name-bap.log"
$deadline = (Get-Date).AddMinutes(8)
while (-not ((Test-Path $bap) -and (Select-String -Path $bap -Pattern 'upo rsp' -Quiet))) {
    if ((Get-Date) -gt $deadline -or $launch.State -eq 'Failed') {
        Receive-Job $launch -Wait -AutoRemoveJob
        throw 'no sign-in within 8 minutes'
    }
    Start-Sleep 3
}
Start-Sleep 25   # the wrapper presses Cross until 110 s after launch

# Started only now: Invoke-Final124Run refuses to launch while any bap_server is running.
$proxy = Start-Process -FilePath $D1Python -PassThru -WindowStyle Hidden -WorkingDirectory $D1Repo `
    -ArgumentList (@('-u', 'server/bap_server.py', '--bind', '127.1.1.128', '--port', '12851') + $proxyArgs) `
    -RedirectStandardOutput (Join-Path $logs "$Name-ahl.log") -RedirectStandardError (Join-Path $logs "$Name-ahl.err")
Set-Content $proxyPidFile $proxy.Id
"proxy pid $($proxy.Id)"

$owned = Read-D1ProcessManifest (Join-Path $logs "$Name-processes.json")
$emulator = (Get-D1OwnedProcess ($owned.processes | Where-Object tag -EQ 'rpcs3')).Id
# Character select uses a free cursor: right and up onto the stored character, then Log In.
& pwsh -NoProfile -File "$PSScriptRoot\Invoke-RpcsStep.ps1" -ProcessId $emulator -Keys D -HoldMs 500 -WaitSeconds 0 | Out-Null
& pwsh -NoProfile -File "$PSScriptRoot\Invoke-RpcsStep.ps1" -ProcessId $emulator -Keys W -HoldMs 250 -WaitSeconds 1
& pwsh -NoProfile -File "$PSScriptRoot\Invoke-RpcsStep.ps1" -ProcessId $emulator -Keys X -WaitSeconds 3
$deadline = (Get-Date).AddMinutes(7)   # the prologue video runs about 3 minutes
while (-not (Select-String -Path $bap -Pattern 'mm rsp' -Quiet)) {
    if ((Get-Date) -gt $deadline) { 'no mm req within 7 minutes'; break }
    Start-Sleep 3
}
$start = Get-Date
for ($i = 1; ((Get-Date) - $start).TotalSeconds -lt $AfterMatchSeconds; $i++) {
    Start-Sleep ($i -le 6 ? 1 : 8)   # the join outcome arrives within seconds
    $line = & $D1Python "$PSScriptRoot\rpcs3_final124_bootflow.py" --out (Join-Path $research "$Name-bootflow-$i.json") 2>&1 |
        Select-Object -Last 1
    "$(Get-Date -Format HH:mm:ss) $line"
}
& pwsh -NoProfile -File "$PSScriptRoot\Invoke-RpcsStep.ps1" -ProcessId $emulator -WaitSeconds 0
# The kept stack can hold the launcher job's output pipe open; do not wait on it indefinitely.
Wait-Job $launch -Timeout 60 | Out-Null
Remove-Job $launch -Force
"done; close with: scripts/Invoke-Final124Join.ps1 -Name $Name -Stop"
