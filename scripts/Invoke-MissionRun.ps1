# The current best unattended run: sign in, launch A Guardian Rises (act_cosmo_m0_highway) through
# the stand-in activity host and spawn the guardian (TEST194). Wraps Invoke-JoinCycle.ps1 with the
# full argument set; extra activity-host or BAP arguments can be appended.
#   Invoke-MissionRun.ps1 -Name TEST196
#   Invoke-MissionRun.ps1 -Name TEST197 -Trace 0x358210 -Dumps 'r3:0' -Hits 20 -Seconds 120
param(
    [Parameter(Mandatory)][string]$Name,
    [string]$Baselines = 'D:\Destiny360\SIGNON_LOCAL\baselines_highway_z104.txt',
    [string[]]$ExtraAh = @(),
    [string[]]$ExtraBap = @(),
    [string]$Trace = '0x2bef68',
    [string]$Dumps = 'r3:0',
    [int]$Hits = 2,
    [int]$Seconds = 60
)
$bap = @('--self-flags', '2', '--universe-flag-values', '215=1', '--peer-family') + $ExtraBap
$ah = @('--ah-join', '--ah-entities', '8192:12288', '--ah-state', '--ah-map', 'act_cosmo_m0_highway',
        '--ah-world', '126', '--ah-checksum', '0x79130968', '--ah-bubbles', '22', '--ah-bubble-states', '-1,13=0',
        '--ah-zone', '104:0x811c9dc5', '--ah-world-state', '104:1', '--ah-roster', '0',
        '--ah-sim-player', '0x4786c0e0:5:0x0a3d4a2c8fbea6ba:1', '--ah-game-mode', '0x4786c0e0:3:3',
        '--ah-baselines', "@$Baselines", '--ah-heartbeat', '1') + $ExtraAh
& "$PSScriptRoot\Invoke-JoinCycle.ps1" -Name $Name -LobbyArgs @('--reply-type', '0x01', '--session-result') `
    -AhpReply 00000000000000007f01018000003233 -AhHost d1local-host -ExtraBap $bap -AhArgs $ah `
    -TraceAfter 'ahp req' -Trace $Trace -Dumps $Dumps -Hits $Hits -Seconds $Seconds
