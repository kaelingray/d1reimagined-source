# The current best unattended run: sign in, launch A Guardian Rises (act_cosmo_m0_highway) through
# the stand-in activity host and spawn the guardian (TEST194) in bubble -Bubble: by default 14, the
# mission start on the highway outside the Wall (TEST342; 13 is Dock 13, the mission's end). Wraps
# Invoke-JoinCycle.ps1 with the full argument set; extra activity-host or BAP arguments can be
# appended (later single-value options override these).
#   Invoke-MissionRun.ps1 -Name TEST196
#   Invoke-MissionRun.ps1 -Name TEST197 -Trace 0x358210 -Dumps 'r3:0' -Hits 20 -Seconds 120
param(
    [Parameter(Mandatory)][string]$Name,
    [ValidateRange(0, 21)][int]$Bubble = 14,
    [string]$Baselines,   # default: SIGNON_LOCAL\baselines_highway_z<8*Bubble>.txt (z112, z104 exist)
    [string[]]$ExtraAh = @(),
    [string[]]$ExtraBap = @(),
    [string]$Trace = '0x2bef68',
    [string]$Dumps = 'r3:0',
    [int]$Hits = 2,
    [int]$Seconds = 60,
    [string]$TraceAfter = 'ahp req'   # '' starts the trace right after the login
)
. "$PSScriptRoot\Paths.ps1"
$zone = 8 * $Bubble   # the bubble's base zone (slice) set
if (-not $Baselines) {
    $Baselines = "$D1Signon\baselines_highway_z$zone.txt"
    if (-not (Test-Path $Baselines)) { $Baselines = "$D1Signon\baselines_highway_z112.txt" }
}
$bap = @('--self-flags', '2', '--universe-flag-values', '215=1', '--peer-family') + $ExtraBap
$ah = @('--ah-join', '--ah-entities', '8192:12288', '--ah-state', '--ah-map', 'act_cosmo_m0_highway',
        '--ah-world', '126', '--ah-checksum', '0x79130968', '--ah-bubbles', '22', '--ah-bubble-states', "-1,$Bubble=0",
        '--ah-zone', "${zone}:0x811c9dc5", '--ah-world-state', "${zone}:1", '--ah-roster', '0',
        '--ah-sim-player', '0x4786c0e0:5:0x0a3d4a2c8fbea6ba:1', '--ah-game-mode', '0x4786c0e0:3:3',
        '--ah-baselines', "@$Baselines", '--ah-heartbeat', '1') + $ExtraAh
& "$PSScriptRoot\Invoke-JoinCycle.ps1" -Name $Name -LobbyArgs @('--reply-type', '0x01', '--session-result') `
    -AhpReply 00000000000000007f01018000003233 -AhHost d1local-host -ExtraBap $bap -AhArgs $ah `
    -TraceAfter $TraceAfter -Trace $Trace -Dumps $Dumps -Hits $Hits -Seconds $Seconds
