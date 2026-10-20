param(
    [Parameter(Mandatory)][string]$Out,
    [int]$Notices = 12,
    [switch]$SkipNotices,
    [switch]$NoLaunch
)
$ErrorActionPreference = 'Stop'
# Orbit -> Set Destination -> Earth -> The Dark Within -> Restart -> LAUNCH, by pad. Needs the Director
# and Earth-page flags served (profile-geared-385-director-d.bin + the 30-flag D1_UNLOCK_FLAGS list).
# Cursor moves are stick holds measured at the default window size; each stage saves a checkpoint.
$pad = Join-Path $PSScriptRoot 'd1-pad.ps1'
$shot = Join-Path $PSScriptRoot 'd1-shot.ps1'
function Say($m) { "$(Get-Date -Format o) $m" | Tee-Object -FilePath (Join-Path $Out 'driver.log') -Append | Write-Host }
function Pad([string[]]$k, [int]$gap = 700) { & $pad -Keys $k -GapMilliseconds $gap | Out-Null }
function Shot($name) { & $shot -Out (Join-Path $Out "$name.png") | Out-Null; Say "checkpoint $name.png" }
New-Item -ItemType Directory -Force $Out | Out-Null

if (-not $SkipNotices) {
    Say "clearing $Notices orbit notices"
    foreach ($i in 1..$Notices) { Pad @('NumPad2') 2200 }
    Pad @('NumPad6') 3000
}
Say 'late-notice pass'
Pad @('Hold:A:2200', 'Hold:W:1400') 400
foreach ($i in 1..4) { Pad @('NumPad2') 2500 }
Shot '1-orbit'

Pad @('Hold:A:2200', 'Hold:S:1400') 400
Pad @('Hold:D:280', 'Hold:W:190') 400
Pad @('NumPad2') 6000
Shot '2-director'

# Earth: park bottom-left, then up-right onto the planet.
Pad @('Hold:A:2200', 'Hold:S:1400') 400
Pad @('Hold:D:520', 'Hold:W:180') 800
Pad @('Hold:D:400', 'Hold:W:50') 1200
Shot '3-earth-hover'
Pad @('NumPad2') 5000
Shot '4-earth-map'

# The Dark Within: the star node below Skywatch, left of centre.
Pad @('Hold:A:500', 'Hold:W:120') 1500
Pad @('Hold:D:200', 'Hold:W:30') 1500
Shot '5-node-hover'
Pad @('NumPad4') 5000            # Restart (Square): starts the mission from the beginning
Shot '6-lobby'
if ($NoLaunch) { Say 'stopping in the lobby'; return }
Pad @('Hold:D:400', 'Hold:D:400', 'Hold:S:300') 500
Pad @('NumPad2') 2500            # LAUNCH
Pad @('NumPad2') 300             # confirm "Restart activity?"
Say 'LAUNCH pressed'
