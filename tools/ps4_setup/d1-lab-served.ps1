param(
    [Parameter(Mandatory)][string]$Name,
    [int]$Minutes = 90,
    # The world the Activity Host publishes (global_activity_state+0x688).
    [string]$World = 'pvp_moonbase',
    # The world's sensor-client list (d1-winguest.py sensor-clients --all --bundle).
    [string]$SensorClients = 'F:\d1\d1preservation-main\cfw-staging\sensor-clients-pvp_moonbase-bundle7-team0.txt',
    # Extra NAME=VALUE server variables appended after the served set.
    [string[]]$Extra = @()
)
# The fully served 56578 lab session: geared Guardian, Director flags served (profile completions +
# D1_UNLOCK_FLAGS gates), and the Activity Host following the launched activity. No memory writes.
$B = 'F:\d1\d1preservation-main\capture-rig\builds\later-56578'
$S = 'F:\d1\d1preservation-main\cfw-staging'
$follow = "$S\$Name-launched-activity.txt"
Remove-Item $follow -ErrorAction SilentlyContinue
$envs = @(
    'D1_ANSWER_FAMILIES=0,2',
    "D1_ROSTER_DIR=$B\rosters-f0-geared",
    'D1_AH_SENSOR_PHASE=3', 'D1_AH_SENSOR_REPEAT_S=10',
    "D1_AH_SENSOR_CLIENTS=$SensorClients",
    "D1_ACTIVITY_FOLLOW_FILE=$follow", "D1_AH_ACTIVITY_FOLLOW_FILE=$follow",
    "D1_AH_ACTIVITY_TABLE=$B\activity-table.tsv",
    "D1_PROFILE_FILE=$B\profile-geared-385-director-c.bin",
    'D1_UNLOCK_FLAGS=98,308,419,440,834,2668,3391,5021,5998,6029,433,2539,2549,3558,3587',
    "D1_INVENTORY=$B\inventory", 'D1_FAMILY42=1', 'D1_UNLOCKS=key'
) + $Extra
& F:\d1\tools\Start-D1ContainedBoot.ps1 -Name $Name -LabIp 10.77.0.2 -Minutes $Minutes -OperatorInput -ActivityHost `
    -Activity $World -ClientPatch F:\d1\d1preservation-main\patches\CUSA00219-56578-lab-orbit.xml `
    -ExtraServerEnv $envs *> "$S\$Name.console"
"done exit=$LASTEXITCODE"
