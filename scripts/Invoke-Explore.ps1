# In-mission reconnaissance (keyboard pad): look around (right stick L), show the Ghost / objective
# (Select = Space), then walk forward in stretches with turns, taking an F12 screenshot after each
# step. Prints "label path" per screenshot.
#   Invoke-Explore.ps1 -Stretches 6
param([int]$Stretches = 6, [int]$WalkMs = 4000)
function Step([string]$Label, [string[]]$Keys, [int]$Hold, [int]$Wait = 2) {
    $shot = & "$PSScriptRoot\Invoke-RpcsStep.ps1" -Keys $Keys -HoldMs $Hold -WaitSeconds $Wait 2>&1 | Select-Object -Last 1
    "$Label $shot"
}
Step 'start' @() 150 1
foreach ($i in 1..6) { Step "look$i" @('L') 700 }
Step 'ghost' @('Space') 150 3
foreach ($i in 1..$Stretches) {
    Step "walk$i" @('W') $WalkMs
    Step "turn$i" @($(if ($i % 2) { 'L' } else { 'J' })) 900
}
