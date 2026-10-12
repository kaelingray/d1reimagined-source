# In-mission soak test (keyboard pad): every step moves a little (W/A/S/D in turn), waits,
# takes an F12 screenshot, scores it with frame_blocks.py and records RPCS3's working set.
# Stops early if RPCS3 exits. Prints one line per step.
#   Invoke-SoakTest.ps1 -Minutes 30
param([int]$Minutes = 30, [int]$StepSeconds = 60)
$keys = @('W', 'A', 'S', 'D')
$end = (Get-Date).AddMinutes($Minutes)
$i = 0
while ((Get-Date) -lt $end) {
    $p = Get-Process rpcs3 -ErrorAction SilentlyContinue
    if (-not $p) { "$(Get-Date -Format HH:mm:ss) RPCS3 exited"; break }
    $key = $keys[$i % $keys.Count]
    $shot = & "$PSScriptRoot\Invoke-RpcsStep.ps1" -Keys $key -HoldMs 1500 -WaitSeconds 3 2>&1 | Select-Object -Last 1
    $score = if ($shot -and (Test-Path $shot)) { python "$PSScriptRoot\frame_blocks.py" $shot } else { "no screenshot" }
    "$(Get-Date -Format HH:mm:ss) step $i $key ws=$([math]::Round($p.WorkingSet64 / 1GB, 1))GB $score"
    $i++
    Start-Sleep -Seconds ([math]::Max(0, $StepSeconds - 8))
}
