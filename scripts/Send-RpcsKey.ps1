# Post a key (default X, 0x58) to the running Destiny RPCS3 window, e.g. to press Cross
# with the keyboard pad handler, or F12 (-Vk 0x7B) for an in-emulator screenshot.
param([int]$Count = 1, [int]$Vk = 0x58)
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Text; using System.Collections.Generic;
public static class W {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
}
"@
$ids = (Get-Process rpcs3 -ErrorAction SilentlyContinue).Id
$found = @()
[W]::EnumWindows({ param($h,$l) $p=0; [void][W]::GetWindowThreadProcessId($h,[ref]$p)
  if ($ids -contains $p -and [W]::IsWindowVisible($h)) { $sb=New-Object Text.StringBuilder 256; [void][W]::GetWindowText($h,$sb,256); $script:found += [pscustomobject]@{H=$h;T=$sb.ToString()} }; $true }, [IntPtr]::Zero) | Out-Null
foreach ($f in $found) { "window: $($f.T)" }
$game = $found | Where-Object { $_.T -match 'FPS|Destiny|BLES' }
foreach ($g in $game) { for ($i=0;$i -lt $Count;$i++) {
  [void][W]::PostMessage($g.H,0x100,[IntPtr]$Vk,[IntPtr]0x002D0001); Start-Sleep -Milliseconds 150
  [void][W]::PostMessage($g.H,0x101,[IntPtr]$Vk,[IntPtr]0xC02D0001); Start-Sleep -Milliseconds 600 }
  "posted key 0x{0:X} x{1} to: {2}" -f $Vk,$Count,$g.T }
