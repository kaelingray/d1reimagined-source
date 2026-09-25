# Drive a freshly started local Destiny run (Start-LocalStack.ps1, keyboard pad) from the title
# screen to "Log In" with the stored character: Cross at the title until the BAP stand-in
# answers sign-in, the three first-run setup screens (they return after every hard restart),
# then the character-select cursor onto the first character tile and Cross.
# Prints an F12 screenshot path after each stage.
param([Parameter(Mandatory)] [string]$Name, [int]$TitleTimeoutMinutes = 15)
$ErrorActionPreference = 'Stop'
$bap = "D:\Destiny360\SIGNON_LOCAL\logs\$Name-bap.log"
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
& "$PSScriptRoot\Invoke-RpcsStep.ps1" -WaitSeconds 0
Keys D 520; Keys W 90          # cursor onto the first character tile
& "$PSScriptRoot\Invoke-RpcsStep.ps1" -WaitSeconds 1
Keys X
Start-Sleep 5
"select-character: $(Seen 'select-character rsp')"
