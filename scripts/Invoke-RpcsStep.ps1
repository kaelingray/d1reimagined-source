# One scripted input step for the local Destiny RPCS3 run: post keys (keyboard pad: X Cross,
# C Circle, Return Start, arrows D-pad, WASD left stick), wait, then take an in-emulator
# screenshot and print its path. It captures the real frame even when the desktop is locked or
# the game window is in the background, unlike PrintWindow.
#   Invoke-RpcsStep.ps1 -Keys X
#   Invoke-RpcsStep.ps1 -Keys D -HoldMs 520 -WaitSeconds 1
param(
    [string[]]$Keys = @(),
    [int]$HoldMs = 150,
    [int]$WaitSeconds = 4,
    [int]$ProcessId = 0
)
. "$PSScriptRoot\Paths.ps1"
$shots = $D1Shots
if ($Keys) { & "$PSScriptRoot\Send-RpcsKey.ps1" -Keys $Keys -HoldMs $HoldMs -ProcessId $ProcessId | Out-Null }
Start-Sleep $WaitSeconds
$before = (Get-ChildItem $shots -File -ErrorAction SilentlyContinue | Where-Object Length -gt 0 | Measure-Object).Count
function Wait-Shot {
    $deadline = (Get-Date).AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 500
        $script:valid = @(Get-ChildItem $shots -File -ErrorAction SilentlyContinue | Where-Object Length -gt 0)
    } until (($script:valid.Count -gt $before) -or (Get-Date) -gt $deadline)
}
# The diagnostic build takes a screenshot when d1/d1shot.on appears (F12 is a window shortcut
# and only fires while the game window is active); fall back to F12 for other builds.
$trigger = (Join-Path $D1Control 'd1shot.on')
[IO.File]::WriteAllText($trigger, '')
Wait-Shot
if ($valid.Count -le $before) {
    if (Test-Path $trigger) { [IO.File]::Delete($trigger) }
    & "$PSScriptRoot\Send-RpcsKey.ps1" -Keys F12 -ProcessId $ProcessId | Out-Null
    Wait-Shot
}
if ($valid.Count -le $before) { throw 'RPCS3 did not save a complete screenshot within 20 seconds' }
($valid | Sort-Object LastWriteTime | Select-Object -Last 1).FullName
