# In-mission render-target dumps during movement (instrumented RPCS3 build, see
# patches/rpcs3-destiny-main-memory-blit.patch): for each step, hold a movement key and create the
# marker d1\d1dump.on (next to rpcs3.exe) part-way through, so the build writes one frame's
# targets to d1\dump\<id>\; then take an F12 screenshot. Prints "label dumpdir screenshot".
# Scan a dump with scripts/d1dump_scan.py.
param([string[]]$Moves = @('W', 'A', 'S', 'D'), [int]$HoldMs = 3000, [int]$ArmAfterMs = 1500)
. "$PSScriptRoot\Paths.ps1"
$root = Join-Path $D1Control 'dump'
foreach ($key in $Moves) {
    $before = @(Get-ChildItem $root -Directory -ErrorAction SilentlyContinue | ForEach-Object Name)
    $arm = Start-Job -ScriptBlock { param($ms, $marker) Start-Sleep -Milliseconds $ms; New-Item -ItemType File $marker -Force | Out-Null } -ArgumentList $ArmAfterMs, (Join-Path $D1Control 'd1dump.on')
    $shot = & "$PSScriptRoot\Invoke-RpcsStep.ps1" -Keys $key -HoldMs $HoldMs -WaitSeconds 3 2>&1 | Select-Object -Last 1
    Receive-Job $arm -Wait -AutoRemoveJob | Out-Null
    $new = @(Get-ChildItem $root -Directory -ErrorAction SilentlyContinue | Where-Object { $_.Name -notin $before } | ForEach-Object FullName)
    "$key $($new -join ',') $shot"
}
