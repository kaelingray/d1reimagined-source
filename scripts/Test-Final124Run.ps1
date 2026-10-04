# Isolated wrapper regression checks. Uses temporary logs and sleeping PowerShell
# child processes; never starts RPCS3, RPCN or a network service.
#   pwsh -NoProfile -File scripts/Test-Final124Run.ps1
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Final124RunHelpers.ps1"

function Assert-Equal($Actual, $Expected, [string]$Label) {
    if ($Actual -ne $Expected) { throw "$Label expected '$Expected', got '$Actual'." }
    Write-Host "PASS $Label"
}

$fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) ('d1-final124-tests-' + [guid]::NewGuid().ToString())
$fixtureScripts = Join-Path $fixtureRoot 'scripts'
$fixtureLocal = Join-Path $fixtureRoot 'private runtime'
$fixtureData = Join-Path $fixtureRoot 'empty data'
$fixtureLogs = Join-Path $fixtureLocal 'SIGNON_LOCAL\logs'
$testIdentities = [System.Collections.Generic.List[object]]::new()
$previousLocal = $env:D1_LOCAL
$previousRpcs3 = $env:D1_RPCS3
$previousUsrdir = $env:D1_USRDIR
$previousMode = $env:D1_TEST_LAUNCH_MODE
try {
    foreach ($file in @('Final124RunHelpers.ps1', 'Invoke-Final124Run.ps1', 'Start-FinalBuildSmoke.ps1',
        'Start-LocalStack.ps1', 'Send-RpcsKey.ps1', 'Invoke-RpcsStep.ps1')) {
        $tokens = $null; $parseErrors = $null
        [void][System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $file), [ref]$tokens, [ref]$parseErrors)
        Assert-Equal @($parseErrors).Count 0 "$file syntax"
    }
    New-Item -ItemType Directory -Path $fixtureScripts, $fixtureLogs, $fixtureData,
        (Join-Path $fixtureLocal 'RPCS3_FINAL124_SMOKE001'),
        (Join-Path $fixtureLocal '01_WORKING\ps3_destiny\PS3_GAME\USRDIR') -Force | Out-Null

    @(
        '2026-09-30T12:00:00 [-] listening on 127.0.0.1:443 (TLS, mode=auth3)',
        '2026-09-30T12:00:01 [#1] TLS handshake failed',
        "2026-09-30T12:00:02 [#2] HTTP 'POST /auth/ HTTP/1.1' host='auth3'",
        '2026-09-30T12:00:03 [#2] replied 200 auth3 success'
    ) | Set-Content -LiteralPath (Join-Path $fixtureLogs 'EVIDENCE-signon.log')
    @('2026-09-30T12:00:00 BAP stand-in listening on 127.1.1.127:12850',
        '2026-09-30T12:00:01 schema loaded', '2026-09-30T12:00:02 startup warning') |
        Set-Content -LiteralPath (Join-Path $fixtureLogs 'EVIDENCE-bap.log')
    $progress = Read-Final124Progress $fixtureLogs 'EVIDENCE'
    Assert-Equal $progress.SignonRequests 0 'TLS/auth3 noise is not SignOn'
    Assert-Equal $progress.SignonReplies 0 'auth3 response is not SignOn success'
    Assert-Equal $progress.BapConnections 0 'BAP startup lines are not connections'
    Assert-Equal (Get-Final124Verdict $progress) 'no SignOn or BAP progress confirmed' 'startup-only verdict'

    @("2026-09-30T12:00:04 [#3] HTTP 'POST /SignOn HTTP/1.1' host='signon'",
        '2026-09-30T12:00:05 [#3] replied 200 SignOn success (100 bytes): session 127.1.1.127:12850',
        '2026-09-30T12:00:06 [#4] replied 200 content manifest (10 bytes)') |
        Add-Content -LiteralPath (Join-Path $fixtureLogs 'EVIDENCE-signon.log')
    $progress = Read-Final124Progress $fixtureLogs 'EVIDENCE'
    Assert-Equal $progress.SignonRequests 1 'actual SignOn HTTP request count'
    Assert-Equal $progress.SignonReplies 1 'actual SignOn success count'
    Assert-Equal $progress.ManifestReplies 1 'manifest response count'
    Assert-Equal (Get-Final124Verdict $progress) 'REACHED SIGN-ON (success reply sent; no BAP connection)' 'SignOn success verdict'
    '2026-09-30T12:00:07 [test] connection from 127.0.0.1:12345' |
        Add-Content -LiteralPath (Join-Path $fixtureLogs 'EVIDENCE-bap.log')
    $progress = Read-Final124Progress $fixtureLogs 'EVIDENCE'
    Assert-Equal (Get-Final124Verdict $progress) 'BAP TCP CONNECTED (no protocol frame confirmed)' 'BAP connection takes priority over SignOn'
    '2026-09-30T12:00:08 [test] frame 1 (kind 2): hello id=1 payload=10 bytes' |
        Add-Content -LiteralPath (Join-Path $fixtureLogs 'EVIDENCE-bap.log')
    $progress = Read-Final124Progress $fixtureLogs 'EVIDENCE'
    Assert-Equal $progress.BapFrames 1 'actual BAP protocol frame count'
    Assert-Equal (Get-Final124Verdict $progress) 'REACHED BAP (protocol frame received)' 'BAP frame takes priority over SignOn'

    # Simulate an emulator's open write handle. Readers must grant write sharing
    # even when the writer permits reads. An exclusive lock must still fail
    # clearly, preserve a previous destination, and leave no partial snapshots.
    $lockedSource = Join-Path $fixtureRoot 'active RPCS3.log'
    $lockedDestination = Join-Path $fixtureRoot 'snapshot.log'
    $payload = [Text.Encoding]::UTF8.GetBytes("first line`nsecond line`n")
    $writer = [IO.FileStream]::new($lockedSource, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::Read)
    try {
        $writer.Write($payload, 0, $payload.Length)
        $writer.Flush()
        $snapshot = Copy-D1SharedFileSnapshot $lockedSource $lockedDestination
        Assert-Equal $snapshot.bytes $payload.Length 'snapshot length with active writer'
        Assert-Equal ([Convert]::ToBase64String([IO.File]::ReadAllBytes($lockedDestination))) ([Convert]::ToBase64String($payload)) 'snapshot bytes with active writer'
    } finally { $writer.Dispose() }
    $exclusive = [IO.FileStream]::new($lockedSource, [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    try {
        $copyFailure = $null
        try { Copy-D1SharedFileSnapshot $lockedSource $lockedDestination | Out-Null } catch { $copyFailure = $_.Exception.Message }
        Assert-Equal ($copyFailure.Contains($lockedSource) -and $copyFailure.Contains($lockedDestination)) $true 'snapshot read error includes both paths'
        Assert-Equal ([Convert]::ToBase64String([IO.File]::ReadAllBytes($lockedDestination))) ([Convert]::ToBase64String($payload)) 'failed snapshot preserves previous destination'
        Assert-Equal @(Get-ChildItem -LiteralPath $fixtureRoot -Filter 'snapshot.log.tmp.*').Count 0 'failed snapshot leaves no temporary artifact'
    } finally { $exclusive.Dispose() }

    # Failed launches must not relabel a shared log from an older emulator.
    # Both existing file locations are checked against this launch's exact time.
    $logIdentity = [pscustomobject]@{ startTimeUtcTicks = [DateTime]::UtcNow.Ticks.ToString() }
    $staleLog = Join-Path $fixtureRoot 'previous RPCS3.log'
    $freshLog = Join-Path $fixtureRoot 'current RPCS3.log'
    Set-Content -LiteralPath $staleLog 'previous run evidence'
    [IO.File]::SetLastWriteTimeUtc($staleLog, [DateTime]::UtcNow.AddDays(-1))
    $staleFailure = $null
    try { Get-D1CurrentRunLog $logIdentity @($staleLog) | Out-Null } catch { $staleFailure = $_.Exception.Message }
    Assert-Equal ([bool]($staleFailure -like '*refusing to capture a previous run*')) $true 'stale shared log is rejected'
    Set-Content -LiteralPath $freshLog 'current run evidence'
    Assert-Equal (Get-D1CurrentRunLog $logIdentity @($staleLog, $freshLog)) $freshLog 'fresh fallback log is selected after stale first location'
    $largeOutput = Join-Path $fixtureRoot 'large stderr.log'
    [IO.File]::WriteAllText($largeOutput, ('x' * 20000) + 'tail marker')
    $tail = Read-D1LaunchOutputTail $largeOutput
    Assert-Equal ($tail.Length -le 8192 -and $tail.EndsWith('tail marker')) $true 'startup output is bounded even for a long line'

    $pwsh = (Get-Command pwsh -ErrorAction Stop).Source
    $sleeper = Start-Process -FilePath $pwsh -ArgumentList @('-NoProfile', '-Command', 'Start-Sleep -Seconds 300') -PassThru -WindowStyle Hidden
    $identity = Get-D1ProcessIdentity $sleeper 'unrelated-test-process' $pwsh
    $testIdentities.Add($identity)
    $liveDiagnostic = Get-D1LaunchedProcessDiagnostic $sleeper 'sleeping-test-process'
    Assert-Equal $liveDiagnostic.hasExited $false 'live retained launch handle does not claim exit'
    $wrong = [pscustomobject]@{ id = $identity.id; executable = $identity.executable; startTimeUtcTicks = '0' }
    Stop-D1OwnedProcess $wrong
    Assert-Equal ([bool](Get-D1OwnedProcess $identity)) $true 'wrong start time cannot authorize cleanup'
    $wrong = [pscustomobject]@{ id = $identity.id; executable = 'wrong.exe'; startTimeUtcTicks = $identity.startTimeUtcTicks }
    Stop-D1OwnedProcess $wrong
    Assert-Equal ([bool](Get-D1OwnedProcess $identity)) $true 'wrong executable cannot authorize cleanup'
    $inputFailure = $null
    try {
        & "$PSScriptRoot\Send-RpcsKey.ps1" -ProcessId $identity.id -ProcessIdentity $wrong -IfWindowReady | Out-Null
    } catch { $inputFailure = $_.Exception.Message }
    Assert-Equal ($inputFailure -like '*exact owned RPCS3 process could not be verified*') $true 'window readiness does not swallow wrong input ownership'

    # Verify actual Windows argv parsing, including empty strings, quotes and a
    # trailing backslash in a path containing spaces.
    $values = @('simple', 'path with spaces', '', 'embedded"quote', 'C:\path with spaces\', '--shape;mirror')
    $argvOutput = Join-Path $fixtureRoot 'argv.json'
    $pythonCode = 'import json,sys; print(json.dumps(sys.argv[1:]))'
    $argv = @(@('-c', $pythonCode) + $values | ForEach-Object { ConvertTo-D1ProcessArgument $_ })
    $echoProcess = Start-Process -FilePath (Get-Command python).Source -ArgumentList $argv -Wait -PassThru -WindowStyle Hidden -RedirectStandardOutput $argvOutput
    Assert-Equal $echoProcess.ExitCode 0 'argument echo child succeeds'
    $received = @(Get-Content -LiteralPath $argvOutput -Raw | ConvertFrom-Json)
    Assert-Equal $received.Count $values.Count 'argument count round trip'
    for ($index = 0; $index -lt $values.Count; $index++) { Assert-Equal $received[$index] $values[$index] "argument $index round trip" }

    # Python processes sometimes return a null Path immediately after launch.
    # Record the actual executable supplied to Start-Process, then verify both
    # monitoring and cleanup once the OS has finished starting the child.
    $python = (Get-Command python).Source
    $exitOutput = Join-Path $fixtureRoot 'exit stdout.log'
    $exitError = Join-Path $fixtureRoot 'exit stderr.log'
    $exitArguments = @('-c', 'import sys; print("startup stdout"); print("startup stderr", file=sys.stderr); sys.exit(23)' |
        ForEach-Object { ConvertTo-D1ProcessArgument $_ })
    $exitedChild = Start-Process -FilePath $python -ArgumentList $exitArguments -Wait -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput $exitOutput -RedirectStandardError $exitError
    $exitDiagnostic = Get-D1LaunchedProcessDiagnostic $exitedChild 'exited-test-process' @($exitOutput, $exitError)
    Assert-Equal $exitDiagnostic.hasExited $true 'retained launch handle confirms actual exit'
    Assert-Equal $exitDiagnostic.exitCode 23 'actual child exit code is reported'
    Assert-Equal ($exitDiagnostic.message.Contains('0x00000017') -and
        $exitDiagnostic.message.Contains('startup stdout') -and $exitDiagnostic.message.Contains('startup stderr')) $true 'exit diagnosis includes hexadecimal code and both redirected streams'
    $pythonArguments = @('-c', 'import time; time.sleep(90)' | ForEach-Object { ConvertTo-D1ProcessArgument $_ })
    for ($trial = 0; $trial -lt 8; $trial++) {
        $pythonChild = Start-Process -FilePath $python -ArgumentList $pythonArguments -WindowStyle Hidden -PassThru
        $pythonIdentity = Get-D1ProcessIdentity $pythonChild 'python-startup-test' $python
        $testIdentities.Add($pythonIdentity)
        Assert-Equal $pythonIdentity.executable $python "Python launch $trial records an explicit executable"
        Start-Sleep -Milliseconds 75
        Assert-Equal ([bool](Get-D1OwnedProcess $pythonIdentity)) $true "Python launch $trial is monitored by identity"
        Stop-D1OwnedProcess $pythonIdentity
        Assert-Equal ([bool](Get-D1OwnedProcess $pythonIdentity)) $false "Python launch $trial is cleaned up"
    }

    # Run the real wrapper against an isolated launcher fixture. Its stand-in is
    # a sleeping PowerShell process, tagged rpcs3 only in the ownership manifest.
    foreach ($file in @('Paths.ps1', 'Final124RunHelpers.ps1', 'Invoke-Final124Run.ps1')) {
        Copy-Item -LiteralPath (Join-Path $PSScriptRoot $file) -Destination (Join-Path $fixtureScripts $file)
    }
    @'
param([string]$Path)
$stream = [IO.FileStream]::new($Path, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::None)
try {
    $bytes = [Text.Encoding]::UTF8.GetBytes('exclusive retained log evidence')
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Flush()
    Start-Sleep -Seconds 90
} finally { $stream.Dispose() }
'@ | Set-Content -LiteralPath (Join-Path $fixtureScripts 'FixtureLockedLog.ps1')
    @'
param([string]$Path)
Start-Sleep -Seconds 7
Add-Content -LiteralPath $Path '2026-09-30T12:00:09 [fixture] frame 2 (kind 1): profile id=2 payload=10 bytes'
Start-Sleep -Seconds 90
'@ | Set-Content -LiteralPath (Join-Path $fixtureScripts 'FixtureLateFrame.ps1')
    @'
param([string[]]$Keys, [int]$HoldMs, [int]$ProcessId, [object]$ProcessIdentity, [switch]$IfWindowReady)
. "$PSScriptRoot\Final124RunHelpers.ps1"
if (-not $IfWindowReady -or $ProcessId -ne $ProcessIdentity.id -or -not (Get-D1OwnedProcess $ProcessIdentity)) {
    throw 'Fixture input must use readiness mode and the exact owned process.'
}
$attemptPath = Join-Path $env:D1_LOCAL 'SIGNON_LOCAL\logs\window-attempts.txt'
Add-Content -LiteralPath $attemptPath 'checked exact ownership before window lookup'
$attempts = @(Get-Content -LiteralPath $attemptPath).Count
# First lookup models PPU compilation with no game window. It posts nothing.
if ($attempts -eq 1) { return $false }
Add-Content -LiteralPath (Join-Path $env:D1_LOCAL 'SIGNON_LOCAL\logs\window-input.txt') 'posted only after game window became ready'
return $true
'@ | Set-Content -LiteralPath (Join-Path $fixtureScripts 'Send-RpcsKey.ps1')
    @'
param([string]$DataRepo, [string]$Rpcs3Dir, [string]$GameUsrdir, [string]$Name,
    [switch]$Run, [switch]$PreserveExistingProcesses, [string]$ProcessManifest)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Paths.ps1"
. "$PSScriptRoot\Final124RunHelpers.ps1"
if (-not $PreserveExistingProcesses) { throw 'Wrapper did not opt into process preservation' }
if ($env:D1_TEST_LAUNCH_MODE -eq 'preflight-failure') { throw 'synthetic preflight failed; no game was started' }
$manifest = @{ name = $Name; state = 'starting'; error = $null; processes = @() }
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $ProcessManifest
$logPath = Join-Path $Rpcs3Dir 'log\RPCS3.log'
New-Item -ItemType Directory -Path (Split-Path $logPath -Parent) -Force | Out-Null
if ($env:D1_TEST_LAUNCH_MODE -eq 'stale-log') {
    Set-Content -LiteralPath $logPath 'synthetic evidence from a previous run'
    [IO.File]::SetLastWriteTimeUtc($logPath, [DateTime]::UtcNow.AddDays(-1))
}
if ($env:D1_TEST_LAUNCH_MODE -eq 'retained-exclusive') {
    $arguments = @('-NoProfile', '-File', "$PSScriptRoot\FixtureLockedLog.ps1", '-Path', $logPath |
        ForEach-Object { ConvertTo-D1ProcessArgument $_ })
    $child = Start-Process -FilePath (Get-Command pwsh).Source -ArgumentList $arguments -WindowStyle Hidden -PassThru
    Start-Sleep -Seconds 1
} elseif ($env:D1_TEST_LAUNCH_MODE -eq 'continue-bap') {
    $arguments = @('-NoProfile', '-File', "$PSScriptRoot\FixtureLateFrame.ps1", '-Path', (Join-Path $D1Logs "$Name-bap.log") |
        ForEach-Object { ConvertTo-D1ProcessArgument $_ })
    $child = Start-Process -FilePath (Get-Command pwsh).Source -ArgumentList $arguments -WindowStyle Hidden -PassThru
} else {
    $child = Start-Process -FilePath (Get-Command pwsh).Source -ArgumentList @('-NoProfile', '-Command', 'Start-Sleep -Seconds 90') -WindowStyle Hidden -PassThru
}
$identity = Get-D1ProcessIdentity $child 'rpcs3' (Get-Command pwsh).Source
if ($env:D1_TEST_LAUNCH_MODE -notin @('retained-exclusive', 'stale-log')) {
    Set-Content -LiteralPath $logPath "synthetic RPCS3 evidence for $Name"
}
if ($env:D1_TEST_LAUNCH_MODE -like 'interrupted-*') {
    # Independent test record lets the parent verify that a child launched
    # before manifest registration never survives wrapper interruption.
    $identity | ConvertTo-Json | Set-Content -LiteralPath "$ProcessManifest.pending.json"
    if ($env:D1_TEST_LAUNCH_MODE -eq 'interrupted-registration') { Start-Sleep -Seconds 3 }
    else { Start-Sleep -Seconds 90 } # deliberately ignore stop; exercise fallback
}
$manifest.processes = @($identity)
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $ProcessManifest
try { Assert-D1StopNotRequested $ProcessManifest } catch {
    $manifest.state = 'failed'
    $manifest.error = $_.Exception.Message
    $manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $ProcessManifest
    Stop-D1OwnedProcess $identity
    throw
}
$state = if ($env:D1_TEST_LAUNCH_MODE -in @('stack-failure', 'stale-log')) { 'failed' } else { 'ready' }
$manifest = @{ name = $Name; state = $state; error = 'synthetic stack failure'; processes = @($identity) }
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $ProcessManifest
if ($state -eq 'failed') { throw 'synthetic stack failure' }
@('2026-09-30T12:00:00 BAP stand-in listening on 127.1.1.127:12850',
    '2026-09-30T12:00:01 [fixture] connection from 127.0.0.1:12345',
    '2026-09-30T12:00:02 [fixture] frame 1 (kind 2): hello id=1 payload=10 bytes') |
    Set-Content -LiteralPath (Join-Path $D1Logs "$Name-bap.log")
'@ | Set-Content -LiteralPath (Join-Path $fixtureScripts 'Start-FinalBuildSmoke.ps1')
    $env:D1_RPCS3 = $null
    $env:D1_USRDIR = $null
    foreach ($mode in @('success', 'continue-bap', 'window-delay', 'preflight-failure', 'stack-failure', 'stale-log',
        'interrupted-registration', 'interrupted-hung', 'retained-exclusive')) {
        $env:D1_TEST_LAUNCH_MODE = $mode
        $runName = $mode.Replace('-', '_')
        $successful = $mode -in @('success', 'continue-bap', 'window-delay', 'retained-exclusive')
        $keep = $mode -eq 'retained-exclusive'
        $bootSeconds = if ($mode -like 'interrupted-*') { 1 } elseif ($mode -eq 'window-delay') { 56 } elseif ($mode -eq 'continue-bap') { 12 } else { 15 }
        $pressSeconds = if ($mode -eq 'window-delay') { 1 } else { 0 }
        $caught = $null
        try {
            & (Join-Path $fixtureScripts 'Invoke-Final124Run.ps1') -Name $runName -Local $fixtureLocal -DataRepo $fixtureData -BootSeconds $bootSeconds -PressSeconds $pressSeconds -KeepRpcs3:($mode -notin @('success', 'continue-bap', 'window-delay')) -ContinueAfterBap:($mode -in @('continue-bap', 'window-delay'))
        } catch { $caught = $_.Exception.Message }
        $report = Get-Content -LiteralPath (Join-Path $fixtureLogs "$runName-summary.json") -Raw | ConvertFrom-Json
        Assert-Equal $report.completed $successful "$mode completed status"
        Assert-Equal ([bool]$caught) (-not $successful) "$mode error propagation"
        Assert-Equal $report.keptStack $keep "$mode retention status"
        if ($successful) {
            Assert-Equal $report.verdict 'REACHED BAP (protocol frame received)' 'wrapper BAP verdict'
        }
        Assert-Equal $report.continuedAfterBap ($mode -in @('continue-bap', 'window-delay')) "$mode continuation mode recorded"
        if ($mode -eq 'continue-bap') {
            Assert-Equal $report.progress.BapFrames 2 'continued observation includes a frame received after initial BAP connection'
        }
        if ($mode -eq 'window-delay') {
            Assert-Equal @(Get-Content -LiteralPath (Join-Path $fixtureLogs 'window-attempts.txt')).Count 2 'missing game window is retried within original boot budget'
            Assert-Equal @(Get-Content -LiteralPath (Join-Path $fixtureLogs 'window-input.txt')).Count 1 'delayed readiness posts exactly one input'
            Assert-Equal $report.presses 1 'missing game window does not count as a press'
        }
        $captureStatus = if ($mode -eq 'retained-exclusive') { 'deferred' }
            elseif ($mode -eq 'stale-log') { 'failed' }
            elseif ($mode -in @('preflight-failure', 'interrupted-hung')) { 'not_started' }
            else { 'captured' }
        Assert-Equal $report.logCapture.status $captureStatus "$mode log capture status"
        if ($mode -eq 'stale-log') {
            Assert-Equal (Test-Path -LiteralPath $report.logCapture.destination) $false 'failed launch does not publish previous emulator evidence'
            Assert-Equal ($report.logCapture.error.Contains('refusing to capture a previous run')) $true 'failed capture explains stale shared log'
        }
        $manifest = Read-D1ProcessManifest (Join-Path $fixtureLogs "$runName-processes.json")
        foreach ($ownedIdentity in @($manifest.processes)) {
            if ($ownedIdentity) {
                $testIdentities.Add($ownedIdentity)
                Assert-Equal ([bool](Get-D1OwnedProcess $ownedIdentity)) $keep "$mode owned child lifecycle"
                if ($keep) { Stop-D1OwnedProcess $ownedIdentity }
            }
        }
        if ($mode -like 'interrupted-*') {
            $pending = Get-Content -LiteralPath (Join-Path $fixtureLogs "$runName-processes.json.pending.json") -Raw | ConvertFrom-Json
            $testIdentities.Add($pending)
            Assert-Equal ([bool](Get-D1OwnedProcess $pending)) $false "$mode unregistered child cannot survive interruption"
        }
        if ($keep) {
            Assert-Equal ($report.logCapture.error.Contains($report.logCapture.source)) $true 'deferred capture reports source path'
            $finalSnapshot = Copy-D1SharedFileSnapshot $report.logCapture.source $report.logCapture.destination
            Assert-Equal ([IO.File]::ReadAllText($finalSnapshot.destination)) 'exclusive retained log evidence' 'deferred log can be captured after owned process stops'
        }
        Assert-Equal ([bool](Get-D1OwnedProcess $identity)) $true "$mode leaves unrelated process running"
        Assert-Equal $env:D1_LOCAL $previousLocal "$mode restores Local environment"
    }
    Stop-D1OwnedProcess $identity
    Assert-Equal ([bool](Get-D1OwnedProcess $identity)) $false 'correct identity permits cleanup'
    Write-Host 'All isolated 01.24 run checks passed.' -ForegroundColor Green
} finally {
    foreach ($testIdentity in $testIdentities) { Stop-D1OwnedProcess $testIdentity }
    $env:D1_LOCAL = $previousLocal
    $env:D1_RPCS3 = $previousRpcs3
    $env:D1_USRDIR = $previousUsrdir
    $env:D1_TEST_LAUNCH_MODE = $previousMode
    # Verify the exact temporary target before any recursive deletion.
    $resolvedRoot = [IO.Path]::GetFullPath($fixtureRoot)
    $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (-not $resolvedRoot.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path $resolvedRoot -Leaf) -notmatch '^d1-final124-tests-[0-9a-f-]{36}$') {
        throw "Refusing to remove unexpected fixture directory: $resolvedRoot"
    }
    if (Test-Path -LiteralPath $resolvedRoot) { Remove-Item -LiteralPath $resolvedRoot -Recurse -Force }
}
