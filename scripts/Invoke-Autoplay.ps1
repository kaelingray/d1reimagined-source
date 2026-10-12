# Unattended local Destiny run: start the loopback stack (Start-LocalStack.ps1), press Cross
# (keyboard pad, X key) until the stored character is selected (service 504 in the BAP log),
# then keep capturing game-window screenshots while the run continues.
# Screenshots: SIGNON_LOCAL\screens\<Name>\NNN.png. Needs the keyboard pad config in the
# RPCS3 copy (Cross = X).
param(
    [Parameter(Mandatory)] [string]$Name,
    [string[]]$BapArgs = @('--universe-time', 's', '--self-flags', '1;2@B0,C0-1023'),
    [int]$WatchSeconds = 240,
    [int]$ShotEvery = 15,
    [switch]$NoRestart
)
. "$PSScriptRoot\Paths.ps1"
$ErrorActionPreference = 'Stop'
$logs = $D1Logs
$shots = "$D1Signon\screens\$Name"
New-Item -ItemType Directory -Force $shots | Out-Null
if (-not $NoRestart) { & "$PSScriptRoot\Start-LocalStack.ps1" -Name $Name -BapArgs $BapArgs | Out-Host }
$bap = "$logs\$Name-bap.log"

function Shot([string]$Tag) {
    $n = (Get-ChildItem $shots -Filter *.png).Count
    & "$PSScriptRoot\Save-GameWindow.ps1" -Out ("$shots\{0:000}-{1}.png" -f $n, $Tag) | Out-Null
}
function Seen([string]$Pattern) { (Test-Path $bap) -and (Select-String -Path $bap -Pattern $Pattern -Quiet) }

$deadline = (Get-Date).AddMinutes(5)
while (-not (Seen 'restored stored character|sent sign-in rsp') -and (Get-Date) -lt $deadline) { Start-Sleep 3 }
"signed in: $(Seen 'sent sign-in rsp')"
Start-Sleep 8
$deadline = (Get-Date).AddMinutes(4)
while (-not (Seen 'select-character rsp') -and (Get-Date) -lt $deadline) {
    & "$PSScriptRoot\Send-RpcsKey.ps1" -Keys X | Out-Null
    Start-Sleep 6
    Shot 'menu'
}
"character selected: $(Seen 'select-character rsp')"
$end = (Get-Date).AddSeconds($WatchSeconds)
while ((Get-Date) -lt $end) {
    Start-Sleep $ShotEvery
    Shot 'run'
    if (Seen '10054|closed with') { Start-Sleep 3; Shot 'end'; break }
}
Select-String -Path $bap -Pattern 'echo|payload=1080|payload=2119|service 70[12]' -NotMatch |
    Select-Object -Last 30 | ForEach-Object { $l = $_.Line; $l.Substring(0, [Math]::Min(200, $l.Length)) }
"screenshots: $shots"
