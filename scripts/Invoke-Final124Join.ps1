# One unattended 01.24 activity-join experiment (about 8 minutes): boot through
# Invoke-Final124Run.ps1, log in with the stored character (setup screens are skipped once the
# store holds its account user data), answer the post-prologue matchmaking poll with a match,
# point the client at an activity host proxy stand-in (127.1.1.128:12851) and sample the
# bootflow while the join runs. The stack is left running (-KeepRpcs3); -Stop closes it,
# including the proxy, whose full process identity is saved separately.
#
#   pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 -Name FINAL124_JOIN1 -Local <runtime> -DataRepo <data>
#   & scripts/Invoke-Final124Join.ps1 -Name FINAL124_JOIN2 -ProxyExtra '--ah-zone;112:0x811c9dc5'
#   & scripts/Invoke-Final124Join.ps1 -Name FINAL124_JOIN3 -SkipOpeningCinematic # after watching the Mars intro
#   pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 -Name FINAL124_JOIN1 -Stop
#
# Defaults use the level-1 intro activity and the final-build scenario checksum
# (FINAL124_JOIN10/11, notes/checkpoint-2026-10-01-profile.md):
# empty mm rsp field 4, ahp rsp to the proxy, all unlock flags, universe flags 3390 and 309 = 1 (the
# intro activity's access rules NOT flag[3390] and NOT flag[309], FINAL124_JOIN3/5; 01.00 used 215), and a
# proxy that joins, grants entity indices and sends the activity state for
# act_cosmo_m0_highway, world (activity) 229. -BapExtra / -ProxyExtra are semicolon
# separated and appended last, so single-value options override the defaults.
param(
    [Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9][A-Za-z0-9_.-]*$')][string]$Name,
    [string]$Local,
    [string]$DataRepo,
    [string]$Map = 'act_cosmo_m0_highway',
    [int]$World = 229,
    [ValidatePattern('^0x[0-9a-fA-F]{1,8}$')][string]$Checksum = '0x8fe07299',
    [string]$BapExtra = '',
    [string]$ProxyExtra = '',
    [ValidateRange(5, 3600)][int]$AfterMatchSeconds = 60,
    [switch]$CaptureTimeline,
    [switch]$SkipOpeningCinematic,
    [switch]$Stop
)
$ErrorActionPreference = 'Stop'
if ($Local) { $env:D1_LOCAL = (Resolve-Path -LiteralPath $Local).Path }
. "$PSScriptRoot\Paths.ps1"
$env:D1_RPCS3 = $D1Rpcs3 = Join-Path $D1Local 'RPCS3_FINAL124_SMOKE001'
$D1Shots = Join-Path $D1Rpcs3 'screenshots\BLES01857'
. "$PSScriptRoot\Final124RunHelpers.ps1"
if (-not $DataRepo) { $DataRepo = if ($env:D1_DATA_REPO) { $env:D1_DATA_REPO } else { Get-D1DefaultDataRepo -Repository $D1Repo } }
$DataRepo = (Resolve-Path -LiteralPath $DataRepo).Path
$logs = Join-Path $D1Local 'SIGNON_LOCAL\logs'
$research = Join-Path $D1Local 'RESEARCH\ps3_updates'
$proxyManifest = Join-Path $logs "$Name-proxy-process.json"

if ($Stop) {
    $stackManifest = Join-Path $logs "$Name-processes.json"
    if (-not (Test-Path -LiteralPath $stackManifest) -and -not (Test-Path -LiteralPath $proxyManifest)) {
        throw "No ownership records for run '$Name' in $logs. Use the exact original -Name."
    }
    $owned = Read-D1ProcessManifest $stackManifest
    if ($owned) { [IO.File]::WriteAllText("$stackManifest.stop", '') }
    $ownedEmulator = $owned.processes | Where-Object tag -EQ 'rpcs3'
    $captureFinal = $ownedEmulator -and (Get-D1OwnedProcess $ownedEmulator)
    foreach ($process in (Get-D1ShutdownOrder $owned.processes)) { Stop-D1OwnedProcess $process }
    if (Test-Path -LiteralPath $proxyManifest) {
        Stop-D1OwnedProcess (Get-Content -LiteralPath $proxyManifest -Raw | ConvertFrom-Json)
    }
    # The emulator log is shared across runs. An already stopped run must not
    # capture a later run's log under its own name.
    if ($captureFinal) {
        Copy-D1SharedFileSnapshot -Source (Join-Path $D1Rpcs3 'log\RPCS3.log') -Destination (Join-Path $logs "$Name-rpcs3-final.log") | Out-Null
    }
    "stopped $Name"
    return
}

$types = Join-Path $DataRepo 'ps3_updates\types_01.24.json'
$services = Join-Path $DataRepo 'ps3_updates\services_01.24.json'
# Universe flags 3390/309 open the highway activity. The in-world "Urgent Message" and
# "Message of the Day" dialogs are not controlled by universe flags 1321/1327 (JOIN229) or
# account flags A1262-A1327 (JOIN230); see notes/guardian-rises-reference-script.md.
$bapArgs = '--mm-rsp;4=;--ahp-reply;00000000000000007f01018000003233;--self-flags;2;--universe-flag-values;3390=1,309=1'
if ($BapExtra) { $bapArgs += ";$BapExtra" }
# c->ah mgr rsp: 02 | u64 host id 1 | char[128] host name (137 bytes, decoder 0x415be4 in 01.00)
$mgr = [byte[]](@(2, 0, 0, 0, 0, 0, 0, 0, 1) + [Text.Encoding]::ASCII.GetBytes('d1local-host') + (,0 * 116))
$proxyArgs = @('--ah-mgr-reply', [Convert]::ToHexString($mgr).ToLower(), '--ah-join', '--ah-entities', '8192:12288',
    '--ah-state', '--ah-map', $Map, '--ah-world', "$World", '--ah-checksum', $Checksum, '--ah-bubbles', '22',
    '--activity-schema', $types, '--schema', $types, '--services-schema', $services) +
    @($ProxyExtra -split ';' | Where-Object { $_ })

# Every input below uses Send-RpcsKey's keyboard mapping. A previous run's
# restored XInput profile otherwise leaves this unattended join at the title.
& "$PSScriptRoot\Set-PadConfig.ps1" -Mode keyboard -Rpcs3Dir $D1Rpcs3

$launch = Start-Job -ScriptBlock { param($script, $n, $l, $d, $b)
    $env:D1_LOCAL = $l
    $ErrorActionPreference = 'Stop'
    # Keep title-screen input available for the full sign-in window, then let
    # the observer finish successfully once the user-profile reply is sent.
    # Cancelling a still-running observer invokes its failure cleanup and kills
    # the kept stack when a short post-match interval finishes before eight minutes.
    & $script -Name $n -Local $l -DataRepo $d -BootSeconds 480 -PressSeconds 110 `
        -ContinueAfterBap -ReturnAfterUserProfileReply -KeepRpcs3 -BapExtra $b 2>&1 | Select-Object -Last 5
} -ArgumentList (Join-Path $PSScriptRoot 'Invoke-Final124Run.ps1'), $Name, $D1Local, $DataRepo, $bapArgs
$bap = Join-Path $logs "$Name-bap.log"
$deadline = (Get-Date).AddMinutes(8)
while (-not ((Test-Path $bap) -and (Select-String -Path $bap -Pattern 'upo rsp' -Quiet))) {
    if ((Get-Date) -gt $deadline -or $launch.State -in @('Failed', 'Completed', 'Stopped')) {
        Receive-Job $launch -Wait -AutoRemoveJob
        throw 'no sign-in within 8 minutes'
    }
    Start-Sleep 3
}
Start-Sleep 25   # the wrapper presses Cross until 110 s after launch

# Started only now: Invoke-Final124Run refuses to launch while any bap_server is running.
$proxy = Start-Process -FilePath $D1Python -PassThru -WindowStyle Hidden -WorkingDirectory $D1Repo `
    -ArgumentList ((@('-u', 'server/bap_server.py', '--bind', '127.1.1.128', '--port', '12851') + $proxyArgs) |
        ForEach-Object { ConvertTo-D1ProcessArgument $_ }) `
    -RedirectStandardOutput (Join-Path $logs "$Name-ahl.log") -RedirectStandardError (Join-Path $logs "$Name-ahl.err")
Get-D1ProcessIdentity -Process $proxy -Tag 'proxy' -Executable $D1Python |
    ConvertTo-Json | Set-Content -LiteralPath $proxyManifest
"proxy pid $($proxy.Id)"

$owned = Read-D1ProcessManifest (Join-Path $logs "$Name-processes.json")
$emulatorIdentity = $owned.processes | Where-Object tag -EQ 'rpcs3'
function Get-JoinEmulatorId {
    $process = Get-D1OwnedProcess $emulatorIdentity
    if (-not $process) { throw "Owned emulator for $Name has exited." }
    $process.Id
}
# Character select uses a free cursor: right and up onto the stored character, then Log In.
try {
    Invoke-D1CheckedPowerShellStep -Path "$PSScriptRoot\Invoke-RpcsStep.ps1" -Arguments @('-ProcessId', (Get-JoinEmulatorId), '-Keys', 'D', '-HoldMs', 500, '-WaitSeconds', 0) | Out-Null
    Invoke-D1CheckedPowerShellStep -Path "$PSScriptRoot\Invoke-RpcsStep.ps1" -Arguments @('-ProcessId', (Get-JoinEmulatorId), '-Keys', 'W', '-HoldMs', 125, '-WaitSeconds', 1)
    Invoke-D1CheckedPowerShellStep -Path "$PSScriptRoot\Invoke-RpcsStep.ps1" -Arguments @('-ProcessId', (Get-JoinEmulatorId), '-Keys', 'X', '-WaitSeconds', 3)
    $deadline = (Get-Date).AddMinutes(7)   # the prologue video runs about 3 minutes
    $openingSkipSent = $false
    $loginRetryAttempts = 0
    $lastLoginPress = Get-Date
    while (-not (Select-String -Path $bap -Pattern 'mm rsp' -Quiet)) {
        if ((Get-Date) -gt $deadline) {
            # A timed-out character/login flow is not a post-match fixture.
            # Reuse exact ownership cleanup before returning this failed run.
            throw 'No matchmaking reply within 7 minutes; post-match fixture was not started.'
        }
        $selectionAnswered = Select-String -Path $bap -Pattern 'sent select-character rsp|pushed self .*after-501' -Quiet
        $retryDue = -not $selectionAnswered -and $loginRetryAttempts -lt 3 -and
            ((Get-Date) - $lastLoginPress).TotalSeconds -ge 6
        if ($retryDue -or ($SkipOpeningCinematic -and -not $openingSkipSent)) {
            Get-JoinEmulatorId | Out-Null
            $openingState = & $D1Python "$PSScriptRoot\rpcs3_final124_bootflow.py" `
                --process-manifest (Join-Path $logs "$Name-processes.json") | ConvertFrom-Json
            if ($LASTEXITCODE -ne 0) { throw 'Could not verify the join bootflow stage.' }
            $selectionAnswered = Select-String -Path $bap -Pattern 'sent select-character rsp|pushed self .*after-501' -Quiet
            # The first Cross can be lost while character select is loading. Retry
            # only its exact native wait, and stop once the service has answered.
            if ($retryDue -and (Test-D1CharacterLoginRetry -State $openingState `
                    -Attempts $loginRetryAttempts -SecondsSincePress ((Get-Date) - $lastLoginPress).TotalSeconds `
                    -SelectionAnswered $selectionAnswered)) {
                $openingState | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath `
                    (Join-Path $logs "$Name-login-retry-$($loginRetryAttempts + 1)-before.json") -Encoding utf8
                & "$PSScriptRoot\Send-RpcsKey.ps1" -ProcessId (Get-JoinEmulatorId) `
                    -ProcessIdentity $emulatorIdentity -Keys X -HoldMs 500 -GapMs 50 | Out-Null
                $loginRetryAttempts++
                $lastLoginPress = Get-Date
                "$(Get-Date -Format HH:mm:ss) stored-character login retry $loginRetryAttempts posted"
                # Resample on the next iteration before considering a Mars skip.
                continue
            }
            # Only the pre-matchmaking character prologue is eligible. Never post
            # skip input into an activity's revival, power beat or outro.
            if ($SkipOpeningCinematic -and -not $openingSkipSent -and
                $openingState.current -eq 24 -and $openingState.target -eq 24 -and
                $openingState.name -eq 'character:prologue' -and
                $openingState.stage_unchanged_during_sample) {
                $openingState | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath `
                    (Join-Path $logs "$Name-opening-skip-before.json") -Encoding utf8
                & "$PSScriptRoot\Send-RpcsKey.ps1" -ProcessId (Get-JoinEmulatorId) `
                    -ProcessIdentity $emulatorIdentity -Keys C -HoldMs 1500 -GapMs 50 | Out-Null
                $openingSkipSent = $true
                "$(Get-Date -Format HH:mm:ss) opening Mars cinematic skip input posted"
            }
        }
        Start-Sleep 3
    }
    $start = Get-Date
    for ($i = 1; ((Get-Date) - $start).TotalSeconds -lt $AfterMatchSeconds; $i++) {
        Start-Sleep ($i -le 30 ? 1 : 8)   # keep short in-world transitions in the timeline
        Get-JoinEmulatorId | Out-Null
        $line = & $D1Python "$PSScriptRoot\rpcs3_final124_bootflow.py" --world --process-manifest (Join-Path $logs "$Name-processes.json") `
            --out (Join-Path $research "$Name-bootflow-$i.json") 2>&1 |
            Select-Object -Last 1
        "$(Get-Date -Format HH:mm:ss) $line"
        if ($CaptureTimeline) {
            $shot = & "$PSScriptRoot\Invoke-RpcsStep.ps1" -ProcessId (Get-JoinEmulatorId) -WaitSeconds 0 |
                Select-Object -Last 1
            [pscustomobject]@{ time = (Get-Date).ToUniversalTime().ToString('o'); sample = "$Name-bootflow-$i.json"; screenshot = $shot } |
                ConvertTo-Json -Compress | Add-Content -LiteralPath (Join-Path $logs "$Name-screenshots.jsonl")
        }
    }
    Invoke-D1CheckedPowerShellStep -Path "$PSScriptRoot\Invoke-RpcsStep.ps1" -Arguments @('-ProcessId', (Get-JoinEmulatorId), '-WaitSeconds', 0)
    # The kept stack can hold the launcher job's output pipe open; do not wait on it indefinitely.
    Wait-Job $launch -Timeout 60 | Out-Null
    if ($launch.State -ne 'Completed') {
        throw "Sign-in observer did not complete its kept-stack handoff (state $($launch.State)); refusing to report a completed join."
    }
    Receive-Job $launch -ErrorAction Stop | Out-Null
    Get-JoinEmulatorId | Out-Null
    Remove-Job $launch -Force
    "done; close with: scripts/Invoke-Final124Join.ps1 -Name $Name -Stop"
} catch {
    $joinFailure = $_
    try { & $PSCommandPath -Name $Name -Local $D1Local -DataRepo $DataRepo -Stop | Out-Null }
    catch { Write-Warning "Join failure cleanup needs attention: $_" }
    if (Get-Job -Id $launch.Id -ErrorAction SilentlyContinue) { Remove-Job $launch -Force }
    throw $joinFailure
}
