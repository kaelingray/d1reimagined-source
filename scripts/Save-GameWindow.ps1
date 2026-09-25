# Save the Destiny RPCS3 game window to a PNG via PrintWindow (works while the window is
# covered by other windows; captures only the game window, not the desktop).
param([Parameter(Mandatory)] [string]$Out)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Collections.Generic; using System.Runtime.InteropServices; using System.Text;
public static class GameWin {
  public struct R { public int L, T, Rr, B; }
  public delegate bool P(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(P p, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
}
"@
$ids = @((Get-Process rpcs3 -ErrorAction Stop).Id)
$script:win = [IntPtr]::Zero
[GameWin]::EnumWindows({ param($h, $l) $p = 0; [void][GameWin]::GetWindowThreadProcessId($h, [ref]$p)
    if ($ids -contains $p -and [GameWin]::IsWindowVisible($h)) {
        $sb = New-Object Text.StringBuilder 256; [void][GameWin]::GetWindowText($h, $sb, 256)
        if ($sb.ToString() -match 'FPS|Destiny|BLES') { $script:win = $h } }
    $true }, [IntPtr]::Zero) | Out-Null
if ($script:win -eq [IntPtr]::Zero) { throw "Destiny game window not found" }
$r = New-Object GameWin+R
[void][GameWin]::GetWindowRect($script:win, [ref]$r)
$bmp = New-Object Drawing.Bitmap ($r.Rr - $r.L), ($r.B - $r.T)
$g = [Drawing.Graphics]::FromImage($bmp)
$dc = $g.GetHdc(); [void][GameWin]::PrintWindow($script:win, $dc, 2); $g.ReleaseHdc($dc); $g.Dispose()
$bmp.Save($Out); $bmp.Dispose()
$Out
