# Drive BLES01857 01.24 from the title screen to character select, unattended.
#
#   pwsh -NoProfile -File scripts\Invoke-Final124Login.ps1 -Name L01
#
# This is the 01.24 counterpart of Invoke-TitanLogin.ps1, and the difference matters.
# Invoke-TitanLogin watches the BAP log for 'sent sign-in rsp' and
# 'select-character rsp'. On 01.24 the client never reaches BAP (it waits in
# network_bap_nat_punch_queue), so those lines never appear and TitanLogin would time
# out on a client that has actually signed on. Here the progress signals are:
#
#   sign-on   the SignOn listener logging 'replied 200 SignOn success'
#   manifest  'replied 200 content manifest'
#   menu      a screenshot whose 'BETA BUILD' watermark is present (Test-ScreenReady)
#
# The setup screens between the title and character select (Set Screen Bounds,
# Brightness, Subtitles) are navigated exactly as TitanLogin does: Cross, Cross,
# left-stick down, Cross. Character select takes input only once the watermark is
# drawn, so the cursor moves wait for it.
#
# Always reports what it saw, so a run that does not reach the end is still evidence.
param(
    [Parameter(Mandatory)][string]$Name,
    [int]$TitleTimeoutMinutes = 12,
    [int]$SetupPauseSeconds = 5,
    [string]$Local = ''
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\Paths.ps1"
if ($Local) { $env:D1_LOCAL = $Local; $D1Local = $Local }
$logs = "$D1Local\SIGNON_LOCAL\logs"
$shots = "$D1Local\RPCS3_FINAL124_SMOKE001\screenshots\BLES01857"

function Keys([string[]]$K, [int]$Hold = 150) { & "$PSScriptRoot\Send-RpcsKey.ps1" -Keys $K -HoldMs $Hold | Out-Null }
function TakeShot { try { & "$PSScriptRoot\Invoke-RpcsStep.ps1" -WaitSeconds 0 } catch { } }
function SignOnSeen  { (Test-Path "$logs\$Name-signon.log") -and (Select-String -Path "$logs\$Name-signon.log" -Pattern 'SignOn success' -Quiet) }
function ManifestSeen { (Test-Path "$logs\$Name-signon.log") -and (Select-String -Path "$logs\$Name-signon.log" -Pattern 'content manifest' -Quiet) }
function BapSeen([string]$P) { (Test-Path "$logs\$Name-bap.log") -and (Select-String -Path "$logs\$Name-bap.log" -Pattern $P -Quiet) }
function LatestShot {
    Get-ChildItem $shots -File -ErrorAction SilentlyContinue | Where-Object Length -gt 0 |
        Sort-Object LastWriteTime | Select-Object -Last 1
}
function MenuReady {
    $s = LatestShot
    ($s) -and (& "$PSScriptRoot\Test-ScreenReady.ps1" -Path $s.FullName)
}


# ---- stage 1: the title screen, until sign-on -------------------------------
Write-Host "== stage 1: title screen, pressing Cross until sign-on" -ForegroundColor Cyan
$deadline = (Get-Date).AddMinutes($TitleTimeoutMinutes)
$presses = 0
while (-not (SignOnSeen) -and (Get-Date) -lt $deadline) {
    Keys X; Start-Sleep 10; $presses++
    Write-Host ("   {0}  Cross #{1}" -f (Get-Date -Format HH:mm:ss), $presses)
}
if (-not (SignOnSeen)) {
    Write-Warning "no sign-on within $TitleTimeoutMinutes min ($presses presses)"
} else {
    Write-Host "   sign-on OK after $presses presses" -ForegroundColor Green
}

# ---- stage 2: the first-run setup screens -----------------------------------
# TitanLogin's sequence: Set Screen Bounds, Brightness, then Subtitles (the cursor
# starts on 2, so a left-stick down lands on an option, then Cross).
Write-Host "== stage 2: setup screens" -ForegroundColor Cyan
Start-Sleep $SetupPauseSeconds
Keys X; Start-Sleep $SetupPauseSeconds      # Set Screen Bounds
TakeShot
Keys X; Start-Sleep $SetupPauseSeconds      # Adjust Brightness
Keys S 200; Start-Sleep 1
Keys X; Start-Sleep $SetupPauseSeconds      # Subtitles
TakeShot
if (ManifestSeen) { Write-Host "   manifest fetched" -ForegroundColor DarkGray }

# ---- stage 3: wait for the menu to accept input -----------------------------
Write-Host "== stage 3: waiting for a menu screen to accept input" -ForegroundColor Cyan
$ready = (Get-Date).AddSeconds(120)
do {
    TakeShot
    if (MenuReady) { break }
    Start-Sleep 3
} until ((Get-Date) -gt $ready)
if (MenuReady) { Write-Host "   menu ready (BETA BUILD watermark present)" -ForegroundColor Green }
else { Write-Warning "no BETA BUILD watermark yet" }

# ---- stage 4: character select ----------------------------------------------
Write-Host "== stage 4: character select" -ForegroundColor Cyan
Keys D 520; Start-Sleep 1        # cursor onto the first character tile
Keys W 90;  Start-Sleep 1
TakeShot
$selected = $false
foreach ($try in 1..3) {          # a first press can be lost while loading
    Keys X
    Start-Sleep 6
    if (BapSeen 'select-character rsp') { $selected = $true; break }
}
if ($selected) { Write-Host "   character select reached the BAP stand-in" -ForegroundColor Green }
else { Write-Warning "character select not confirmed (no BAP 'select-character rsp')" }

# ---- report -----------------------------------------------------------------
$shot = LatestShot
Write-Host ''
Write-Host "== $Name ==" -ForegroundColor Cyan
Write-Host ("   presses at title : {0}" -f $presses)
Write-Host ("   sign-on          : {0}" -f (SignOnSeen))
Write-Host ("   manifest         : {0}" -f (ManifestSeen))
Write-Host ("   menu watermark   : {0}" -f (MenuReady))
Write-Host ("   character select : {0}" -f $selected)
if ($shot) { Write-Host ("   last screenshot  : {0}" -f $shot.FullName) }
$bf = "$logs\$Name-bap.log"
if (Test-Path $bf) {
    $n = @(Get-Content $bf).Count
    Write-Host ("   BAP log lines    : {0}" -f $n)
    if ($n -gt 1) { Get-Content $bf | Select-Object -Skip 1 -First 6 | ForEach-Object { "      $_" } }
}
