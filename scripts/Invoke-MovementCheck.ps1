# Standard in-mission rendering check (keyboard pad): dismiss the urgent-message dialog, take a
# still frame, walk (W, A, S, D), take a frame after each move, then one after standing still.
# Prints "label path" per F12 screenshot. Used to compare RPCS3 settings (TEST204 onwards).
param([int]$SettleSeconds = 45)
$steps = @(
    @{ Label = 'dismissed'; Keys = @('X'); Hold = 150; Wait = 8 },
    @{ Label = 'w800'; Keys = @('W'); Hold = 800; Wait = 4 },
    @{ Label = 'w2500'; Keys = @('W'); Hold = 2500; Wait = 3 },
    @{ Label = 'a1500'; Keys = @('A'); Hold = 1500; Wait = 3 },
    @{ Label = 's2000'; Keys = @('S'); Hold = 2000; Wait = 3 },
    @{ Label = 'd2500'; Keys = @('D'); Hold = 2500; Wait = 3 },
    @{ Label = "still$SettleSeconds"; Keys = @(); Hold = 150; Wait = $SettleSeconds }
)
foreach ($s in $steps) {
    $shot = & "$PSScriptRoot\Invoke-RpcsStep.ps1" -Keys $s.Keys -HoldMs $s.Hold -WaitSeconds $s.Wait 2>&1 | Select-Object -Last 1
    "$($s.Label) $shot"
}
