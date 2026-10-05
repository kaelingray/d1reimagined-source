# Start the full loopback stack for the local Destiny PS3 test and boot RPCS3.
# Services: RPCN, DNS sinkhole, STUN stub, SignOn (auth3 + manifest), lobby probe, BAP
# stand-in, then RPCS3 (TEST048 copy: XInput pad, async shaders) and the GDB daemon.
# Legacy calls stop already-running services first. -PreserveExistingProcesses
# refuses conflicts without stopping another run. Logs: SIGNON_LOCAL\logs\<Name>-*.log
param(
    [string]$Name = ("STACK_" + (Get-Date -Format 'yyyyMMdd_HHmmss')),
    [string[]]$BapArgs = @('--universe-time', 's', '--self-flags', '1;2@B0,C0-1023'),
    [string[]]$LobbyArgs = @(),
    [string[]]$SignonArgs = @(),
    [string[]]$StunArgs = @(),   # experimental 01.24 reply shapes; no dispatch layout is confirmed
    [string[]]$DnsArgs = @(),   # e.g. '--stun-distinct' to give each STUN name its own address
    [switch]$NoGame,
    [switch]$PreserveExistingProcesses,
    [string]$ProcessManifest
)
. "$PSScriptRoot\Paths.ps1"
. "$PSScriptRoot\Final124RunHelpers.ps1"
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$root = $D1Local
$logs = "$root\SIGNON_LOCAL\logs"
$py = $D1Python
$rpcs3 = $D1Rpcs3
$usrdir = $D1Usrdir

# Content failure must never prevent a game launch. The independent worker only
# observes ready run manifests and starts native recording after exact ownership checks.
try { & "$PSScriptRoot\Start-ContentCapture.ps1" | Out-Null }
catch { Write-Warning "Optional content capture startup: $_" }

$owned = [System.Collections.Generic.List[object]]::new()
$launchedProcesses = [System.Collections.Generic.Dictionary[string, System.Diagnostics.Process]]::new()
$state = 'starting'
$failure = $null
function Save-ProcessManifest {
    if ($ProcessManifest) {
        $manifest = [ordered]@{ name = $Name; state = $state; error = $failure; processes = @($owned.ToArray()) }
        # Readers see either the previous complete manifest or the next one.
        $temporary = "$ProcessManifest.tmp"
        [IO.File]::WriteAllText($temporary, ($manifest | ConvertTo-Json -Depth 5))
        Move-Item -LiteralPath $temporary -Destination $ProcessManifest -Force
    }
}
function Register-Process([System.Diagnostics.Process]$Process, [string]$Tag, [string]$Executable) {
    try {
        # Keep the kernel handle from this exact launch for exit diagnostics;
        # later PID lookup alone cannot prove that the process actually exited.
        $null = $Process.Handle
        $launchedProcesses[$Tag] = $Process
        $identity = Get-D1ProcessIdentity -Process $Process -Tag $Tag -Executable $Executable
        $owned.Add($identity)
        Save-ProcessManifest
        $identity
    } catch {
        # Registration itself can fail before the PID is written to disk. This
        # handle came directly from this invocation's Start-Process call.
        $registrationFailure = $_.Exception.Message
        $detail = Get-D1LaunchedProcessDiagnostic $Process $Tag @("$logs\$Name-$Tag-stdout.log", "$logs\$Name-$Tag.log", "$logs\$Name-$Tag.err")
        Stop-Process -InputObject $Process -Force -Confirm:$false -ErrorAction SilentlyContinue
        throw "$registrationFailure$([Environment]::NewLine)$($detail.message)"
    }
}
function Start-StandIn([string]$Tag, [string[]]$ArgList) {
    Assert-D1StopNotRequested $ProcessManifest
    $arguments = @(@('-u') + $ArgList | ForEach-Object { ConvertTo-D1ProcessArgument $_ })
    $process = Start-Process -FilePath $py -ArgumentList $arguments -WorkingDirectory $repo -WindowStyle Hidden `
        -RedirectStandardOutput "$logs\$Name-$Tag.log" -RedirectStandardError "$logs\$Name-$Tag.err" -PassThru
    Register-Process $process $Tag $py | Out-Null
    Assert-D1StopNotRequested $ProcessManifest
}
function Assert-StandIns {
    Assert-D1StopNotRequested $ProcessManifest
    foreach ($identity in $owned) {
        $process = $launchedProcesses[$identity.tag]
        $process.Refresh()
        if ($process.HasExited -or -not (Get-D1OwnedProcess $identity)) {
            $detail = Get-D1LaunchedProcessDiagnostic $process $identity.tag @(
                "$logs\$Name-$($identity.tag)-stdout.log", "$logs\$Name-$($identity.tag).log", "$logs\$Name-$($identity.tag).err")
            $reason = if ($detail.hasExited) { 'exited during stack launch' } else { 'could not be verified by exact identity during stack launch' }
            throw "$($identity.tag) $reason.$([Environment]::NewLine)$($detail.message)"
        }
        $errorLog = Join-Path $logs "$Name-$($identity.tag).err"
        # RPCS3 can write nonfatal driver/compiler messages to stderr. Its
        # process exit or debugger startup deadline determines launch failure.
        if ($identity.tag -ne 'rpcs3' -and (Test-Path -LiteralPath $errorLog) -and (Get-Item -LiteralPath $errorLog).Length -gt 0) {
            $detail = Read-D1LaunchOutputTail $errorLog
            throw "$($identity.tag) reported a startup error:$([Environment]::NewLine)$detail"
        }
    }
}

try {
    New-Item -ItemType Directory -Path $logs -Force | Out-Null
    if ($ProcessManifest) {
        $ProcessManifest = [IO.Path]::GetFullPath($ProcessManifest)
        New-Item -ItemType Directory -Path (Split-Path $ProcessManifest -Parent) -Force | Out-Null
    }
    Save-ProcessManifest
    Assert-D1StopNotRequested $ProcessManifest
    if (-not $PreserveExistingProcesses) {
        Get-Process rpcs3 -ErrorAction SilentlyContinue | Stop-Process -Confirm:$false
        Get-CimInstance Win32_Process -Filter "Name like 'python%'" |
            Where-Object { $_.CommandLine -match 'dns_sinkhole|stun_stub|signon_probe|lobby_probe|bap_server|rpcs3_gdb_daemon' } |
            ForEach-Object { Stop-Process -Id $_.ProcessId -Confirm:$false }
        Start-Sleep 2
    } elseif (-not $NoGame) {
        # The debugger ports are fixed; attaching to an existing listener would
        # accidentally control another session even if this emulator launches.
        $debugListeners = @(Get-NetTCPConnection -State Listen -ErrorAction Stop |
            Where-Object LocalPort -In @(2345, 2346))
        if ($debugListeners) { throw 'Debugger port 2345 or 2346 is already in use; close that session explicitly before launching this run.' }
    }

    if (-not (Get-Process rpcn -ErrorAction SilentlyContinue)) {
        Start-Process -FilePath "$root\RPCN_LOCAL\rpcn.exe" -WorkingDirectory "$root\RPCN_LOCAL" -WindowStyle Hidden | Out-Null
    }
    # RPCN is shared and deliberately excluded from the run's cleanup manifest.
    Start-StandIn dns (@('server/dns_sinkhole.py', '--destiny-hosts', '--log', "$logs\$Name-dns-queries.log") + $DnsArgs)
    Start-StandIn stun (@('server/stun_stub.py') + $StunArgs)
    Start-StandIn signon (@('server/signon_probe.py', '--mode', 'auth3', '--destiny-hosts',
        '--manifest-url', 'http://content.local.test/manifest', '--manifest-packages', "$usrdir\packages") + $SignonArgs)
    Start-StandIn lobby (@('server/lobby_probe.py', '--timeout', '60') + $LobbyArgs)
    Start-StandIn bap (@('server/bap_server.py', '--ws-signin',
        '--schema', "$root\SIGNON_LOCAL\investment_schema_character.json",
        '--store', "$root\SIGNON_LOCAL\characters.json") + $BapArgs)
    Start-Sleep 3
    Assert-StandIns
    if (-not $NoGame) {
        Assert-D1StopNotRequested $ProcessManifest
        $emulator = Start-Process -FilePath "$rpcs3\rpcs3.exe" `
            -ArgumentList (ConvertTo-D1ProcessArgument "$usrdir\EBOOT.BIN") -WorkingDirectory $rpcs3 -PassThru -WindowStyle Hidden `
            -RedirectStandardOutput "$logs\$Name-rpcs3-stdout.log" -RedirectStandardError "$logs\$Name-rpcs3.err"
        Register-Process $emulator 'rpcs3' (Join-Path $rpcs3 'rpcs3.exe') | Out-Null
        Assert-D1StopNotRequested $ProcessManifest
        $deadline = (Get-Date).AddSeconds(60)
        do {
            Start-Sleep 1
            Assert-StandIns
            $debugListener = Get-NetTCPConnection -State Listen -LocalPort 2345 -OwningProcess $emulator.Id -ErrorAction SilentlyContinue
        } until ($debugListener -or (Get-Date) -ge $deadline)
        if (-not $debugListener) {
            $detail = Get-D1LaunchedProcessDiagnostic $emulator 'rpcs3' @("$logs\$Name-rpcs3-stdout.log", "$logs\$Name-rpcs3.err")
            throw "This RPCS3 process did not open debugger port 2345 within 60 seconds.$([Environment]::NewLine)$($detail.message)"
        }
        # RPCS3's GDB stub serves one connection per emulator session: the daemon holds it.
        Start-StandIn gdbd @('scripts/rpcs3_gdb_daemon.py')
        Start-Sleep 3
        Assert-StandIns
        Get-Content -LiteralPath "$logs\$Name-gdbd.log", "$logs\$Name-bap.log"
    }
    $state = 'ready'
    Save-ProcessManifest
} catch {
    $state = 'failed'
    $failure = $_.Exception.Message
    try { Save-ProcessManifest } catch { Write-Warning "Could not save failed launch ownership: $_" }
    # Failed launches clean up only processes started by this invocation.
    foreach ($identity in (Get-D1ShutdownOrder $owned.ToArray())) {
        try { Stop-D1OwnedProcess $identity } catch { Write-Warning "Could not stop owned $($identity.tag): $_" }
    }
    throw
}
