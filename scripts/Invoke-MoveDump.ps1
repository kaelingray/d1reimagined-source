# In-mission render-target dumps during movement (instrumented RPCS3 build, see
# patches/rpcs3-destiny-main-memory-blit.patch): for each step, hold a movement key and create the
# marker C:/dev/d1dump.on part-way through, so the build writes one frame's targets to
# D:\Destiny360\d1dump\<id>\; then take an F12 screenshot. Prints "label dumpdir screenshot".
# Scan a dump with scripts/d1dump_scan.py.
param([string[]]$Moves = @('W', 'A', 'S', 'D'), [int]$HoldMs = 3000, [int]$ArmAfterMs = 1500)
$root = 'D:\Destiny360\d1dump'
foreach ($key in $Moves) {
    $before = @(Get-ChildItem $root -Directory -ErrorAction SilentlyContinue | ForEach-Object Name)
    $arm = Start-Job -ScriptBlock { param($ms) Start-Sleep -Milliseconds $ms; New-Item -ItemType File C:\dev\d1dump.on -Force | Out-Null } -ArgumentList $ArmAfterMs
    $shot = & "$PSScriptRoot\Invoke-RpcsStep.ps1" -Keys $key -HoldMs $HoldMs -WaitSeconds 3 2>&1 | Select-Object -Last 1
    Receive-Job $arm -Wait -AutoRemoveJob | Out-Null
    $new = @(Get-ChildItem $root -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -notin $before } | ForEach-Object FullName)
    "$key $($new -join ',') $shot"
}
