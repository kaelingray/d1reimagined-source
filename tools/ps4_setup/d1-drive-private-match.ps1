param(
    # Folder for checkpoint screenshots.
    [Parameter(Mandatory)][string]$Out,
    # Cross presses to clear the orbit notices (Armsday, Eververse, Rise of Iron, Age of Triumph).
    [int]$Notices = 9,
    # Start at a clear orbit screen (notices already dismissed). The notice set varies with the
    # served unlocks (e.g. "Destination revealed" popups), so clearing by count is not reliable.
    [switch]$SkipNotices
)
$ErrorActionPreference = 'Stop'
# Orbit -> Set Destination -> Crucible -> Private Match -> LAUNCH, by pad, for a contained run started
# with -OperatorInput. Needs the Director flags served (profile completions + D1_UNLOCK_FLAGS gates).
# Cursor moves are stick holds tuned at the default window size; each stage saves a checkpoint.
# The Destiny 2 advert is dismissed with Circle, never Cross (Cross presses Pre-order).
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
Shot '1-orbit'

# Park the cursor in the bottom-left corner, then step onto Set Destination.
Pad @('Hold:A:2200', 'Hold:S:1400') 400
    Pad @('Hold:D:280', 'Hold:W:190') 400
Pad @('NumPad2') 6000
Shot '2-director'

# Crucible sits up and right of the Tower.
Pad @('Hold:D:325', 'Hold:W:310') 1500
Pad @('Hold:D:120', 'Hold:W:180') 1500
Pad @('NumPad2') 4500
Shot '3-crucible'

# Private Match is the bottom node of the Crucible page. The page opens with the cursor wherever the
# Crucible node was, so park in the bottom-left corner first, then step onto it.
Pad @('Hold:A:2200', 'Hold:S:1400') 400
Pad @('Hold:D:520', 'Hold:W:150') 500
Pad @('Hold:D:300', 'Hold:W:170') 500
Pad @('Hold:D:130') 1500
Shot '4-private-hover'
Pad @('NumPad2') 6000
Shot '5-lobby'

Pad @('Hold:D:640', 'Hold:S:285') 900
Pad @('NumPad2') 300
Say 'LAUNCH pressed'
