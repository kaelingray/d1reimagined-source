# Run one BLES01857 01.24 experiment and clean up the processes it owns.
#
#   pwsh -NoProfile -File scripts/Invoke-Final124Run.ps1 -Name TEST -Local <private-runtime> -DataRepo <data-checkout>
#   pwsh -NoProfile -File scripts/Invoke-Final124Run.ps1 -Name TEST -BootSeconds 300 -PressSeconds 150
#
# Paths default to Paths.ps1 / D1_LOCAL, D1_RPCS3, D1_USRDIR and D1_DATA_REPO.
# Pass -Local explicitly when using an imported runtime outside this checkout.
# -KeepRpcs3 retains the whole successful stack for interactive work; failures
# always clean up. Shared RPCN and existing sessions are left running.
# -PressSeconds enables Cross after 45 seconds and continues until BAP or the
# press deadline. SignOn alone does not end the experiment: BAP is the next gate.
# -ContinueAfterBap observes the full BootSeconds and keeps scheduled input
# active through profile setup; the default stops at the first BAP connection.
param(
    [Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9][A-Za-z0-9_.-]*$')][string]$Name,
    [string]$StunArgs = '',
    [string]$DnsArgs = '',
    [string]$BapExtra = '',
    [ValidateRange(1, 86400)][int]$BootSeconds = 300,
    [ValidateRange(0, 86400)][int]$PressSeconds = 0,
    [ValidateRange(1, 3600)][int]$PressEverySeconds = 10,
    [switch]$Shot,
    [string]$Local,
    [string]$DataRepo,
    [switch]$KeepRpcs3,
    [switch]$ContinueAfterBap,
    [string]$Rpcs3Dir,
    [string]$GameUsrdir,
    [string]$PpuHash,
    [string]$ManifestFile
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Final124RunHelpers.ps1"
$previousLocal = $env:D1_LOCAL
$previousRpcs3 = $env:D1_RPCS3
$previousUsrdir = $env:D1_USRDIR
$launcherIdentity = $null
$processManifest = $null
$summaryPath = $null
$completed = $false
$runFailure = $null
$progress = $null
$presses = 0
$launcherCompleted = $false
$bapReported = $false
$logCapture = [ordered]@{ status = 'not_started'; source = $null; destination = $null; bytes = $null; error = $null }

function Get-LaunchFailureDetail {
    $details = foreach ($path in @("$logs\$Name-launch.err", "$logs\$Name-launch.log")) {
        if (Test-Path -LiteralPath $path) { Get-Content -LiteralPath $path -Tail 8 }
    }
    $details -join [Environment]::NewLine
}

try {
    if ($Local) { $env:D1_LOCAL = (Resolve-Path -LiteralPath $Local).Path }
    . "$PSScriptRoot\Paths.ps1"
    $repo = $D1Repo
    $Local = (Resolve-Path -LiteralPath $D1Local).Path
    if (-not $DataRepo) {
        $DataRepo = if ($env:D1_DATA_REPO) { $env:D1_DATA_REPO } else { Join-Path (Split-Path $repo -Parent) 'd1xenia-data' }
    }
    $DataRepo = (Resolve-Path -LiteralPath $DataRepo).Path
    if (-not $Rpcs3Dir) {
        $Rpcs3Dir = if ($env:D1_RPCS3) { $D1Rpcs3 } else { Join-Path $Local 'RPCS3_FINAL124_SMOKE001' }
    }
    if (-not $GameUsrdir) { $GameUsrdir = $D1Usrdir }
    $Rpcs3Dir = (Resolve-Path -LiteralPath $Rpcs3Dir).Path
    $GameUsrdir = (Resolve-Path -LiteralPath $GameUsrdir).Path
    $logs = Join-Path $Local 'SIGNON_LOCAL\logs'
    New-Item -ItemType Directory -Path $logs -Force | Out-Null
    # Reusing a log name would mix evidence or overwrite an active session's
    # ownership record. A distinct Name is required for each experiment.
    if (Get-ChildItem -LiteralPath $logs -Filter "$Name-*" -File) {
        throw "Run name '$Name' already has logs in $logs. Choose a new -Name."
    }
    $processManifest = Join-Path $logs "$Name-processes.json"
    $summaryPath = Join-Path $logs "$Name-summary.json"
    $env:D1_LOCAL = $Local
    $env:D1_RPCS3 = $Rpcs3Dir
    $env:D1_USRDIR = $GameUsrdir
    $arguments = @(
        '-NoProfile', '-File', "$PSScriptRoot\Start-FinalBuildSmoke.ps1",
        '-DataRepo', $DataRepo, '-Rpcs3Dir', $Rpcs3Dir, '-GameUsrdir', $GameUsrdir,
        '-Name', $Name, '-Run', '-PreserveExistingProcesses', '-ProcessManifest', $processManifest
    )
    # A leading -- value otherwise binds as another pwsh -File parameter.
    # Attach option strings to their named parameter before Windows argv quoting.
    if ($StunArgs) { $arguments += "-StunArgs:$StunArgs" }
    if ($DnsArgs) { $arguments += "-DnsArgs:$DnsArgs" }
    if ($BapExtra) { $arguments += "-BapExtra:$BapExtra" }
    if ($PpuHash) { $arguments += @('-PpuHash', $PpuHash) }
    if ($ManifestFile) { $arguments += @('-ManifestFile', (Resolve-Path -LiteralPath $ManifestFile).Path) }
    $quotedArguments = @($arguments | ForEach-Object { ConvertTo-D1ProcessArgument $_ })
    $pwsh = (Get-Command pwsh -ErrorAction Stop).Source
    $launcher = Start-Process -FilePath $pwsh -ArgumentList $quotedArguments -WorkingDirectory $repo -WindowStyle Hidden `
        -RedirectStandardOutput "$logs\$Name-launch.log" -RedirectStandardError "$logs\$Name-launch.err" -PassThru
    try { $launcherIdentity = Get-D1ProcessIdentity $launcher 'launcher' $pwsh }
    catch {
        Stop-Process -InputObject $launcher -Force -Confirm:$false -ErrorAction SilentlyContinue
        throw
    }
    Write-Host "$Name launch requested; observing for up to $BootSeconds s. Logs: $logs" -ForegroundColor Cyan

    $started = Get-Date
    $deadline = $started.AddSeconds($BootSeconds)
    $nextPress = $started.AddSeconds(45)
    $pressUntil = $started.AddSeconds($PressSeconds)
    $lastReport = $started
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Seconds ([Math]::Min(5, [Math]::Max(1, [Math]::Ceiling(($deadline - (Get-Date)).TotalSeconds))))
        $launcher.Refresh()
        $manifest = Read-D1ProcessManifest $processManifest
        if ($manifest -and $manifest.state -eq 'failed') { throw "Stack launch failed: $($manifest.error)" }
        if ($launcher.HasExited -and -not $launcherCompleted) {
            if ($launcher.ExitCode -ne 0) {
                throw "01.24 launch/preflight failed (exit $($launcher.ExitCode)):$([Environment]::NewLine)$(Get-LaunchFailureDetail)"
            }
            if (-not $manifest -or $manifest.state -ne 'ready') { throw 'Launcher exited without confirming a ready stack.' }
            $launcherCompleted = $true
            Write-Host '  preflight and stack launch succeeded' -ForegroundColor DarkCyan
        }
        if ($manifest -and $manifest.state -eq 'ready') {
            foreach ($identity in $manifest.processes) {
                if (-not (Get-D1OwnedProcess $identity)) { throw "Owned $($identity.tag) exited during the experiment. Check $logs\$Name-$($identity.tag).err." }
                $errorLog = Join-Path $logs "$Name-$($identity.tag).err"
                if ((Test-Path -LiteralPath $errorLog) -and (Get-Item -LiteralPath $errorLog).Length -gt 0) {
                    throw "Owned $($identity.tag) reported an error. Check $errorLog."
                }
            }
        }
        $progress = Read-Final124Progress -LogRoot $logs -Name $Name
        if ($launcherCompleted -and $progress.BapConnections -gt 0) {
            if (-not $bapReported) {
                Write-Host '  BAP TCP connection observed' -ForegroundColor Green
                $bapReported = $true
            }
            if (-not $ContinueAfterBap) { break }
        }
        if ($PressSeconds -gt 0 -and (Get-Date) -lt $pressUntil -and (Get-Date) -ge $nextPress -and $manifest) {
            $emulatorIdentity = @($manifest.processes | Where-Object tag -EQ 'rpcs3') | Select-Object -First 1
            if ($emulatorIdentity) {
                $emulator = Get-D1OwnedProcess $emulatorIdentity
                if (-not $emulator) { throw 'The owned RPCS3 process is no longer running.' }
                & "$PSScriptRoot\Send-RpcsKey.ps1" -Keys X -HoldMs 150 -ProcessId $emulator.Id | Out-Null
                $presses++
                $nextPress = (Get-Date).AddSeconds($PressEverySeconds)
                Write-Host ("  {0}  Cross #{1}" -f (Get-Date -Format HH:mm:ss), $presses) -ForegroundColor DarkCyan
                if ($presses -eq 1 -and $Shot) {
                    & "$PSScriptRoot\Invoke-RpcsStep.ps1" -WaitSeconds 1 -ProcessId $emulator.Id | Out-Null
                }
            }
        }
        if (((Get-Date) - $lastReport).TotalSeconds -ge 60) {
            Write-Host ("  probes={0} replies={1} dns={2} signon-requests={3} signon-replies={4} bap-connections={5} bap-frames={6}" -f `
                $progress.StunProbes, $progress.StunReplies, $progress.DnsQueries, $progress.SignonRequests,
                $progress.SignonReplies, $progress.BapConnections, $progress.BapFrames)
            $lastReport = Get-Date
        }
    }
    if (-not $launcherCompleted) { throw "Launch did not finish successfully within $BootSeconds seconds. Check $logs\$Name-launch.*." }
    $progress = Read-Final124Progress -LogRoot $logs -Name $Name
    Write-Host "$Name : $(Get-Final124Verdict $progress)" -ForegroundColor Yellow
    Write-Host ("  probes={0} replies={1} dns={2} signon-requests={3} signon-replies={4} manifest-replies={5} bap-connections={6} bap-frames={7}" -f `
        $progress.StunProbes, $progress.StunReplies, $progress.DnsQueries, $progress.SignonRequests,
        $progress.SignonReplies, $progress.ManifestReplies, $progress.BapConnections, $progress.BapFrames)
    $completed = $true
} catch {
    $runFailure = $_.Exception.Message
    throw
} finally {
    if (-not ($KeepRpcs3 -and $completed)) {
        # Allow a child launched just before interruption to be registered and
        # cleaned by the launcher. Killing it immediately could leave that
        # child alive without a PID record. Verify direct children before any
        # forced termination if cooperative shutdown exceeds the grace period.
        $launcherChildren = @()
        if ($launcherIdentity) {
            try {
                if (Get-D1OwnedProcess $launcherIdentity) {
                    if ($processManifest) { [IO.File]::WriteAllText("$processManifest.stop", '') }
                    $stopDeadline = (Get-Date).AddSeconds(10)
                    while ((Get-D1OwnedProcess $launcherIdentity) -and (Get-Date) -lt $stopDeadline) {
                        Start-Sleep -Milliseconds 200
                    }
                    if (Get-D1OwnedProcess $launcherIdentity) {
                        $launcherChildren = @(Get-D1LauncherChildren $launcherIdentity)
                        Stop-D1OwnedProcess $launcherIdentity
                    }
                }
            } catch { Write-Warning "Launcher cleanup failed: $_" }
        }
        if ($processManifest) {
            try {
                $manifest = Read-D1ProcessManifest $processManifest
                $cleanupIdentities = @($manifest.processes) + $launcherChildren
                foreach ($identity in $cleanupIdentities | Sort-Object { $_.tag -ne 'gdbd' }) {
                    if ($identity) {
                        try { Stop-D1OwnedProcess $identity } catch { Write-Warning "Could not stop owned $($identity.tag): $_" }
                    }
                }
                Write-Host 'cleaned up the processes owned by this run' -ForegroundColor DarkGray
            } catch { Write-Warning "Owned-process cleanup failed: $_" }
        }
    } else {
        Write-Host "leaving this run's stack up (-KeepRpcs3); ownership record: $processManifest" -ForegroundColor DarkGray
    }
    # Capture after cleanup so crashes and service errors keep their emulator
    # evidence too. A retained emulator may deny every live reader; defer that
    # capture explicitly without turning a successful retained run into failure.
    if ($processManifest) {
        try {
            $manifest = Read-D1ProcessManifest $processManifest
            if (@($manifest.processes | Where-Object tag -EQ 'rpcs3').Count -gt 0) {
                $logCapture.destination = Join-Path $logs "$Name-rpcs3.log"
                $sources = @(Join-Path $Rpcs3Dir 'log\RPCS3.log'; Join-Path $Rpcs3Dir 'RPCS3.log')
                $logCapture.source = $sources | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
                if (-not $logCapture.source) { throw "RPCS3 log not found at $($sources -join ' or ')." }
                $snapshot = Copy-D1SharedFileSnapshot -Source $logCapture.source -Destination $logCapture.destination
                $logCapture.status = 'captured'
                $logCapture.bytes = $snapshot.bytes
                Write-Host "RPCS3 log snapshot: $($snapshot.bytes) bytes -> $($snapshot.destination)" -ForegroundColor DarkGray
            }
        } catch {
            $logCapture.status = if ($KeepRpcs3 -and $completed) { 'deferred' } else { 'failed' }
            $logCapture.error = $_.Exception.Message
            Write-Warning "RPCS3 log capture $($logCapture.status): $($logCapture.error)"
        }
    }
    if ($summaryPath) {
        try {
            [ordered]@{
                name = $Name; completed = $completed; error = $runFailure
                verdict = if ($progress) { Get-Final124Verdict $progress } else { 'no protocol evidence collected' }
                local = $Local; dataRepo = $DataRepo; rpcs3Dir = $Rpcs3Dir; gameUsrdir = $GameUsrdir
                presses = $presses; progress = $progress; keptStack = [bool]($KeepRpcs3 -and $completed)
                continuedAfterBap = [bool]$ContinueAfterBap
                logCapture = $logCapture
                finishedAt = (Get-Date).ToString('o')
            } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $summaryPath -Encoding utf8
            Write-Host "run summary: $summaryPath" -ForegroundColor DarkGray
        } catch { Write-Warning "Could not save the run summary: $_" }
    }
    $env:D1_LOCAL = $previousLocal
    $env:D1_RPCS3 = $previousRpcs3
    $env:D1_USRDIR = $previousUsrdir
}
