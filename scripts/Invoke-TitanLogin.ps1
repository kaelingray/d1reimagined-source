# Drive a freshly started local Destiny run (Start-LocalStack.ps1, keyboard pad) from the title
# screen to "Log In" with the stored character: Cross at the title until the BAP stand-in
# answers sign-in, the three first-run setup screens (they return after every hard restart),
# then the character-select cursor onto the first character tile and Cross.
# Prints a screenshot path after each stage; throws if sign-in or character select fails.
param([Parameter(Mandatory)] [string]$Name, [int]$TitleTimeoutMinutes = 15)
. "$PSScriptRoot\Paths.ps1"
$ErrorActionPreference = 'Stop'
$bap = "$D1Logs\$Name-bap.log"
function Seen([string]$Pattern) { (Test-Path $bap) -and (Select-String -Path $bap -Pattern $Pattern -Quiet) }
function Keys([string[]]$K, [int]$Hold = 150) { & "$PSScriptRoot\Send-RpcsKey.ps1" -Keys $K -HoldMs $Hold | Out-Null }

$deadline = (Get-Date).AddMinutes($TitleTimeoutMinutes)
while (-not (Seen 'sent sign-in rsp') -and (Get-Date) -lt $deadline) { Keys X; Start-Sleep 10 }
if (-not (Seen 'sent sign-in rsp')) { throw "no sign-in after $TitleTimeoutMinutes minutes" }
"signed in"
Start-Sleep 10
& "$PSScriptRoot\Invoke-RpcsStep.ps1" -WaitSeconds 0
Keys X; Start-Sleep 4          # Set Screen Bounds
Keys X; Start-Sleep 4          # Adjust Brightness (cursor starts on 2)
Keys S 200; Start-Sleep 1      # Subtitles: cursor down onto an option
Keys X; Start-Sleep 8
# Character select takes input only once it has finished loading (the "BETA BUILD" watermark is
# drawn); a slower PC is still loading here, and the cursor move below would then be lost.
$ready = (Get-Date).AddSeconds(90)
do {
    $shot = & "$PSScriptRoot\Invoke-RpcsStep.ps1" -WaitSeconds 0
    if (& "$PSScriptRoot\Test-ScreenReady.ps1" -Path $shot) { break }
    Start-Sleep 2
} until ((Get-Date) -gt $ready)
Keys D 520; Keys W 90          # cursor onto the first character tile
& "$PSScriptRoot\Invoke-RpcsStep.ps1" -WaitSeconds 1
foreach ($try in 1..3) {       # Cross opens "Log In" on the tile; press again if it was missed
    Keys X
    Start-Sleep 6
    if (Seen 'select-character rsp') { break }
}
if (-not (Seen 'select-character rsp')) { throw 'character select did not reach the BAP stand-in' }
"select-character: True"
