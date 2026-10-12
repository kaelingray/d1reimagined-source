# True when a 1280x720 in-emulator screenshot shows the "BETA BUILD" watermark in the top right,
# which the game draws once a menu screen (e.g. character select) has finished loading and accepts
# input. Counts light pixels in the watermark's box.
#   Test-ScreenReady.ps1 -Path shot.png
param([Parameter(Mandatory)][string]$Path, [int]$MinPixels = 60)
Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile($Path)
try {
    $light = 0
    for ($y = 120; $y -lt 144; $y += 2) {
        for ($x = 1015; $x -lt 1170; $x += 2) {
            $c = $bmp.GetPixel($x, $y)
            if (($c.R + $c.G + $c.B) / 3 -gt 110) { $light++ }
        }
    }
    $light -ge $MinPixels
} finally { $bmp.Dispose() }
