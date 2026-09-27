# One mission boot with extra activity-host arguments (e.g. --ah-object / --ah-object-at state
# experiments) in a chosen zone set, plus what every such run needs: the bubble's replication
# records fed back to the BAP stand-in (--ah-baselines-watch) until the run ends, one Cross on the
# URGENT MESSAGE after the join, and screenshots every -Every seconds for -Seconds. Keyboard pad
# during the run, XInput restored afterwards; RPCS3 is closed at the end.
# -Watch writes PPU hook watches (d1\d1ppu.txt lines, "ADDR [MAXHITS] [u] [rN[+OFF]:LEN ...]") for
# the instrumented build before the boot and empties the list afterwards; the D1P lines end up in
# the copied RPCS3 log.
#   Invoke-ObjectExperiment.ps1 -Name TEST361 -Bubble 14 -Zone 113 -ExtraAh @('--ah-object', '...')
#   Invoke-ObjectExperiment.ps1 -Name TEST367 -Bubble 14 -ExtraAh @('--ah-bubble-states', '-1,14=0,3=0', '--ah-zone-schedule', '24@150') `
#     -Watch @('42a498 4000 u r3:48 r4:48 r5:48', '258778 4000 u r3:32 r4:32 r5:32')
param(
    [Parameter(Mandatory)][string]$Name,
    [Parameter(Mandatory)][int]$Bubble,
    [int]$Zone = -1,              # default: the bubble's base zone set (8 * Bubble)
    [string[]]$ExtraAh = @(),
    [int]$Seconds = 180,
    [int]$Every = 5,
    [string[]]$Watch = @()
)
. "$PSScriptRoot\Paths.ps1"
if ($Bubble -lt 0 -or $Bubble -gt 21) { throw 'bubble must be 0-21' }
if ($Zone -lt 0) { $Zone = 8 * $Bubble }
$watch = "$D1Signon\$Name-baselines.txt"
if (Test-Path $watch) { [IO.File]::Delete($watch) }
$ppuWatch = Join-Path $D1Control 'd1ppu.txt'
if ($Watch) { [IO.File]::WriteAllLines($ppuWatch, $Watch) }
& "$PSScriptRoot\Set-PadConfig.ps1" -Mode keyboard
try {
    # Feed the client's replication records back every 15 s from the start: an object without a
    # baseline blocks all replicated state (0x1ce1b8), including the experiment's own objects.
    $feeder = Start-Job -ScriptBlock {
        param($py, $script, $watch, $summary)
        while ($true) {
            $out = & $py $script 2> $summary
            if ($out) { [IO.File]::WriteAllText($watch, ($out -join '')) }
            Start-Sleep 15
        }
    } -ArgumentList $D1Python, "$PSScriptRoot\rpcs3_replicated_objects.py", $watch, "$D1Signon\$Name-objects.txt"
    & "$PSScriptRoot\Invoke-MissionRun.ps1" -Name $Name -Seconds 30 -ExtraAh (@(
        '--ah-bubble-states', "-1,$Bubble=0", '--ah-zone', "${Zone}:0x811c9dc5", '--ah-world-state', "${Zone}:1",
        '--ah-baselines-watch', $watch) + $ExtraAh) 2>&1 | Out-Null
    "joined; dismissing the urgent message"
    & "$PSScriptRoot\Invoke-RpcsStep.ps1" -Keys X -WaitSeconds 2 2>&1 | Select-Object -Last 1
    $end = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $end) {
        $shot = & "$PSScriptRoot\Invoke-RpcsStep.ps1" -WaitSeconds $Every 2>&1 | Select-Object -Last 1
        "{0} {1}" -f (Get-Date -Format HH:mm:ss), $shot
    }
    & $D1Python "$PSScriptRoot\rpcs3_replicated_objects.py" --json-out "$D1Signon\$Name-objects.json" --schema-out "$D1Signon\$Name-object-types.json" 2> "$D1Signon\$Name-objects-final.txt" | Out-Null
} finally {
    if ($feeder) { Stop-Job $feeder; Remove-Job $feeder -Force }
    if ($Watch) { [IO.File]::WriteAllText($ppuWatch, '') }
    Copy-Item "$D1Rpcs3\log\RPCS3.log" "$D1Logs\$Name-rpcs3.log" -ErrorAction SilentlyContinue
    Get-Process rpcs3 -ErrorAction SilentlyContinue | Stop-Process -Force -Confirm:$false
    Start-Sleep 3
    & "$PSScriptRoot\Set-PadConfig.ps1" -Mode xinput
}
