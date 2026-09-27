# Visit bubbles of A Guardian Rises one boot at a time (keyboard pad; restores the XInput pad at
# the end): join with bubble N's base zone set (8*N), dump the bubble's replication records into a
# file the BAP stand-in watches (--ah-baselines-watch) so it sends their baselines and the
# guardian can spawn, then take a screenshot before and after opening the Ghost's nav mode (which
# names the location). Prints "bubble N label path" per screenshot and saves the record summary.
# A failed boot (e.g. the emulator freezing in the login cinematic) is retried up to -Attempts
# times, each attempt under the next TEST number.
#   Invoke-BubbleSweep.ps1 -Bubbles 1,2,3 -FirstTest 263
param([Parameter(Mandatory)][string[]]$Bubbles, [Parameter(Mandatory)][int]$FirstTest, [int]$Attempts = 3)
# accept "21,14" as one string too (pwsh -File passes it unsplit, and [int] would read 2114)
# (a new variable: assigning back to the [string[]] parameter would turn the numbers into strings)
[int[]]$bubbleList = @($Bubbles -split '[,\s]+' | Where-Object { $_ } | ForEach-Object { [int]$_ })
if ($bubbleList | Where-Object { $_ -lt 0 -or $_ -gt 21 }) { throw "bubbles must be 0-21 (act_cosmo_m0_highway has 22): $($bubbleList -join ',')" }
. "$PSScriptRoot\Paths.ps1"
$watch = "$D1Signon\sweep-baselines.txt"
& "$PSScriptRoot\Set-PadConfig.ps1" -Mode keyboard
try {
    $test = $FirstTest
    foreach ($n in $bubbleList) {
        $zone = 8 * $n
        foreach ($attempt in 1..$Attempts) {
            $name = "TEST$test"; $test++
            if (Test-Path $watch) { [IO.File]::Delete($watch) }
            $done = $false
            try {
                & "$PSScriptRoot\Invoke-MissionRun.ps1" -Name $name -Seconds 30 -ExtraAh @(
                    '--ah-bubble-states', "-1,$n=0", '--ah-zone', "${zone}:0x811c9dc5", '--ah-world-state', "${zone}:1",
                    '--ah-baselines-watch', $watch) 2>&1 | Out-Null
                foreach ($i in 1..4) {
                    $list = python "$PSScriptRoot\rpcs3_replicated_objects.py" 2> "$D1Signon\$name-objects.txt"
                    if ($list) { [IO.File]::WriteAllText($watch, ($list -join '')) }
                    Start-Sleep 20
                }
                foreach ($step in @(@('start', @('X'), 3), @('look', @('L'), 2), @('nav', @('Space'), 3))) {
                    $shot = & "$PSScriptRoot\Invoke-RpcsStep.ps1" -Keys $step[1] -HoldMs 300 -WaitSeconds $step[2] 2>&1 | Select-Object -Last 1
                    "bubble $n $($step[0]) $shot"
                }
                $done = $true
            } catch { "bubble $n $name attempt $attempt failed: $_" }
            Copy-Item "$D1Rpcs3\log\RPCS3.log" "$D1Logs\$name-rpcs3.log" -ErrorAction SilentlyContinue
            Get-Process rpcs3 -ErrorAction SilentlyContinue | Stop-Process -Force -Confirm:$false
            Start-Sleep 5
            if ($done) { break }
        }
    }
} finally {
    Get-Process rpcs3 -ErrorAction SilentlyContinue | Stop-Process -Force -Confirm:$false
    Start-Sleep 3
    & "$PSScriptRoot\Set-PadConfig.ps1" -Mode xinput
}
