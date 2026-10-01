# Wait for the owned 01.24 mission to be stable, then capture a short walking
# and camera sequence. This uses the existing keyboard-pad bindings. It stops
# when loading/cleanup returns, and never sends input to a different emulator.
param(
    [Parameter(Mandatory)][ValidatePattern('^[A-Za-z0-9][A-Za-z0-9_.-]*$')][string]$Name,
    [Parameter(Mandatory)][string]$Local,
    [ValidateRange(0, 3600)][int]$NotBeforeSeconds = 150,
    [ValidateRange(10, 3600)][int]$TimeoutSeconds = 600
)
$ErrorActionPreference = 'Stop'
$env:D1_LOCAL = (Resolve-Path -LiteralPath $Local).Path
. "$PSScriptRoot\Paths.ps1"
$env:D1_RPCS3 = $D1Rpcs3 = Join-Path $D1Local 'RPCS3_FINAL124_SMOKE001'
. "$PSScriptRoot\Final124RunHelpers.ps1"
$manifest = Join-Path $D1Logs "$Name-processes.json"
$research = Join-Path $D1Local 'RESEARCH\ps3_updates'
$audit = Join-Path $D1Logs "$Name-movement.jsonl"
$stateFile = Join-Path $research "$Name-movement-state.json"
$identity = (Read-D1ProcessManifest $manifest).processes | Where-Object tag -EQ 'rpcs3'
$matchLog = Join-Path $D1Logs "$Name-bap.log"
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$matchAt = $null
$stable = 0
$sawLoading = $false

function Get-MovementState {
    $process = Get-D1OwnedProcess $identity
    if (-not $process) { throw "Owned emulator for $Name has exited." }
    & $D1Python "$PSScriptRoot\rpcs3_final124_bootflow.py" --world --process-manifest $manifest --out $stateFile | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Unable to read the owned mission state.' }
    Get-Content -LiteralPath $stateFile -Raw | ConvertFrom-Json
}
function Test-MovementReady($state) {
    $state.current -eq 36 -and $state.stage_unchanged_during_sample -and
        $null -ne $state.world.engine_state_89 -and
        $state.world.engine_state_89 -eq 0
}

while ((Get-Date) -lt $deadline) {
    $state = Get-MovementState
    if ($state.current -ge 33 -and $state.current -le 36) { $sawLoading = $true }
    if ($sawLoading -and $state.name -eq 'cleanup') {
        "mission entered cleanup; no walking sequence was sent"
        return
    }
    if (-not $matchAt -and (Select-String -LiteralPath $matchLog -Pattern 'mm rsp' -Quiet)) {
        # Use the protocol log's timestamp, not the time this observer noticed it.
        $line = (Select-String -LiteralPath $matchLog -Pattern 'mm rsp' | Select-Object -First 1).Line
        $matchAt = [DateTime]::Parse($line.Substring(0, 23))
    }
    $stable = if (Test-MovementReady $state) { $stable + 1 } else { 0 }
    if ($stable -ge 3 -and $matchAt -and ((Get-Date) - $matchAt).TotalSeconds -ge $NotBeforeSeconds) { break }
    Start-Sleep -Seconds 2
}
if ($stable -lt 3 -or -not $matchAt -or ((Get-Date) - $matchAt).TotalSeconds -lt $NotBeforeSeconds) {
    "mission did not become ready for the walking sequence within $TimeoutSeconds seconds"
    return
}

$steps = @(
    @{ label = 'before'; keys = @(); hold = 0 },
    @{ label = 'forward800'; keys = @('W'); hold = 800 },
    @{ label = 'forward2500'; keys = @('W'); hold = 2500 },
    @{ label = 'left1500'; keys = @('A'); hold = 1500 },
    @{ label = 'back2000'; keys = @('S'); hold = 2000 },
    @{ label = 'right2500'; keys = @('D'); hold = 2500 },
    @{ label = 'camera300'; keys = @('L'); hold = 300 },
    @{ label = 'after'; keys = @(); hold = 0 }
)
foreach ($step in $steps) {
    $state = Get-MovementState
    if (-not (Test-MovementReady $state)) { "mission state changed; stopped before $($step.label)"; return }
    $process = Get-D1OwnedProcess $identity
    if (-not $process) { throw "Owned emulator for $Name has exited." }
    $shot = & "$PSScriptRoot\Invoke-RpcsStep.ps1" -ProcessId $process.Id -Keys $step.keys -HoldMs $step.hold -WaitSeconds 2 |
        Select-Object -Last 1
    [pscustomobject]@{ time = (Get-Date).ToUniversalTime().ToString('o'); label = $step.label;
        keys = $step.keys; holdMs = $step.hold; screenshot = $shot; state = $state.name } |
        ConvertTo-Json -Compress | Add-Content -LiteralPath $audit
    "$($step.label) $shot"
}
