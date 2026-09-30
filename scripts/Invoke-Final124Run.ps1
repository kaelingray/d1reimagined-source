# Run one BLES01857 01.24 experiment end to end and ALWAYS clean up afterwards.
#
#   pwsh -NoProfile -File run-final124.ps1 -Name TEST -StunArgs '--shape;mirror'
#   pwsh -NoProfile -File run-final124.ps1 -Name TEST -BootSeconds 300 -KeepRpcs3
#
# The point of the wrapper: every run used to be left with RPCS3 and the stand-ins
# still running, which wastes RAM (the emulator sits at ~3.2 GB) and means the next
# run inherits a half-torn-down stack. This closes the emulator and the services
# before returning, whether the run succeeded, failed or was interrupted - the
# cleanup is in a `finally`, so Ctrl-C does not leak the process either.
#
# -BootSeconds is how long to let the client run before judging it. ~300 s is
# enough to get past shader compilation and well into the STUN loop.
#
# -PressX drives the title screen. 01.24 gets as far as the startup network work
# on its own, but the title screen is a Taken King / "Press X" prompt and the
# stand-ins only get contacted *after* Cross, exactly as Invoke-TitanLogin.ps1 does
# for 01.00. Without it the run measures the client sitting at a prompt. The press
# loop stops as soon as the BAP stand-in shows sign-in traffic, so it costs nothing
# once the client is past that point.
param(
    [Parameter(Mandatory)][string]$Name,
    [string]$StunArgs = '',
    [string]$DnsArgs = '',
    [int]$BootSeconds = 300,
    [int]$PressSeconds = 0,      # 0 = not used; else keep pressing Cross for this long
    [int]$PressEverySeconds = 10,
    [switch]$Shot,               # take a screenshot after the first Cross
    [string]$Local = 'F:\d1\lukespc-import\d1xenia\local',
    [string]$DataRepo = 'F:\d1\d1xenia-data',
    [switch]$KeepRpcs3          # leave the emulator up for interactive poking
)

$ErrorActionPreference = 'Continue'
$repo = 'F:\d1\d1xenia'
$rpcs3dir = "$Local\RPCS3_FINAL124_SMOKE001"
$usrdir = "$Local\01_WORKING\ps3_destiny\PS3_GAME\USRDIR"
$logs = "$Local\SIGNON_LOCAL\logs"
$env:D1_LOCAL = $Local

function Stop-Stack {
    <#
      .SYNOPSIS
      Close the emulator and every stand-in this stack started.
      .DESCRIPTION
      Stops RPCS3 and the python services by matching their command lines rather
      than by name, because a parallel run would otherwise be killed too. Leaves
      rpcn.exe alone: it is a shared local service started once, not per run.
    #>
    Get-Process rpcs3 -ErrorAction SilentlyContinue |
        Stop-Process -Force -Confirm:$false -ErrorAction SilentlyContinue
    Get-CimInstance Win32_Process -Filter "Name like 'python%'" -ErrorAction SilentlyContinue |
        Where-Object { $_.CommandLine -match 'dns_sinkhole|stun_stub|signon_probe|lobby_probe|bap_server|rpcs3_gdb_daemon' } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -Confirm:$false -ErrorAction SilentlyContinue }
    Start-Sleep 3
}

# clear anything left over from a previous run before starting
Stop-Stack

try {
    $args = @(
        '-NoProfile', '-File', "$repo\scripts\Start-FinalBuildSmoke.ps1",
        '-DataRepo', $DataRepo, '-Rpcs3Dir', $rpcs3dir, '-GameUsrdir', $usrdir,
        '-Name', $Name, '-Run'
    )
    if ($StunArgs) { $args += @('-StunArgs', $StunArgs) }
    if ($DnsArgs)  { $args += @('-DnsArgs', $DnsArgs) }

    $pwsh = (Get-Command pwsh).Source
    Start-Process -FilePath $pwsh -ArgumentList $args -WorkingDirectory $repo -WindowStyle Hidden `
        -RedirectStandardOutput "$logs\$Name-launch.log" `
        -RedirectStandardError "$logs\$Name-launch.err" | Out-Null
    Write-Host "started $Name at $(Get-Date -Format HH:mm:ss) - letting it run $BootSeconds s" -ForegroundColor Cyan

    $deadline = (Get-Date).AddSeconds($BootSeconds)
    $probes = 0; $replies = 0; $signonReqs = 0; $bapLines = 0; $dnsNames = 0
    $lastReport = Get-Date
    $presses = 0
    $nextPress = (Get-Date).AddSeconds(45)   # let it boot past shader compilation first
    $pressUntil = if ($PressSeconds -gt 0) { (Get-Date).AddSeconds($PressSeconds) } else { $null }

    while ((Get-Date) -lt $deadline) {
        Start-Sleep 5
        $sf = "$logs\$Name-stun.log"
        if (Test-Path $sf) {
            $s = @(Get-Content $sf -ErrorAction SilentlyContinue)
            $probes = @($s | Select-String '<- ').Count
            $replies = @($s | Select-String ' -> ').Count
        }
        $gn = "$logs\$Name-signon.log"
        if (Test-Path $gn) {
            $signonReqs = @(Get-Content $gn -ErrorAction SilentlyContinue |
                            Select-String -NotMatch 'listening on').Count
        }
        $bf = "$logs\$Name-bap.log"
        if (Test-Path $bf) { $bapLines = @(Get-Content $bf -ErrorAction SilentlyContinue).Count }
        $df = "$logs\$Name-dns-queries.log"
        if (Test-Path $df) { $dnsNames = @(Get-Content $df -ErrorAction SilentlyContinue).Count }

        # the client leaving the STUN loop is the only thing worth stopping early for
        if ($signonReqs -gt 0 -or $bapLines -gt 1) {
            Write-Host "  progress: sign-on requests=$signonReqs bap=$bapLines" -ForegroundColor Green
            break
        }

        # Cross at the title screen. Stop once the stand-ins see anything, or when
        # the press window is over.
        if ($pressUntil -and (Get-Date) -lt $pressUntil -and (Get-Date) -ge $nextPress) {
            & "$repo\scripts\Send-RpcsKey.ps1" -Keys X -HoldMs 150 | Out-Null
            $presses++
            $nextPress = (Get-Date).AddSeconds($PressEverySeconds)
            Write-Host ("  {0}  Cross #{1}" -f (Get-Date -Format HH:mm:ss), $presses) -ForegroundColor DarkCyan
            if ($presses -eq 1 -and $Shot) {
                try { & "$repo\scripts\Invoke-RpcsStep.ps1" -WaitSeconds 1 | Out-Null } catch {}
            }
        }

        if (((Get-Date) - $lastReport).TotalSeconds -ge 120) {
            Write-Host ("  {0}  probes={1} replies={2} dns={3} signon={4} bap={5}" -f `
                (Get-Date -Format HH:mm:ss), $probes, $replies, $dnsNames, $signonReqs, $bapLines)
            $lastReport = Get-Date
        }
    }

    # the PPU-hook numbers are the real signal: the gate field and the dispatch count
    $verdict = if ($signonReqs -gt 0) { 'REACHED SIGN-ON' }
               elseif ($bapLines -gt 1) { 'reached BAP' }
               else { 'no progress (still in the STUN loop)' }
    Write-Host ""
    Write-Host "$Name : $verdict" -ForegroundColor Yellow
    Write-Host ("  probes={0} replies={1} dns={2} signon-requests={3} bap-log-lines={4}" -f `
        $probes, $replies, $dnsNames, $signonReqs, $bapLines)

    try { Copy-Item "$rpcs3dir\log\RPCS3.log" "$logs\$Name-rpcs3.log" -ErrorAction SilentlyContinue } catch {}
    $hooked = "$logs\$Name-rpcs3.log"
    if (Test-Path $hooked) {
        foreach ($a in @('0xb36fd8', '0xb370e0', '0xb371e0', '0xb69ddb0', '0x69ddb0')) {
            $n = @(Select-String -Path $hooked -Pattern "pc=$a " -ErrorAction SilentlyContinue).Count
            if ($n) { Write-Host ("  hook {0}: {1} hits" -f $a, $n) }
        }
    }
}
finally {
    if (-not $KeepRpcs3) {
        Stop-Stack
        $left = Get-Process rpcs3 -ErrorAction SilentlyContinue
        if ($left) { Write-Warning "RPCS3 still running (pid $($left.Id))" }
        else { Write-Host "cleaned up: RPCS3 and the stand-ins are closed" -ForegroundColor DarkGray }
    } else {
        Write-Host "leaving the stack up (-KeepRpcs3)" -ForegroundColor DarkGray
    }
}
