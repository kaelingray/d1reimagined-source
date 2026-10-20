param([Parameter(Mandatory)][string]$Out)
$ErrorActionPreference = 'Stop'
# Save a PNG of the running shadPS4 window (brought to the front first so the capture is the game,
# not whatever covers it).
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
namespace D1Shot { public static class W {
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
} }
'@
$emu = Get-Process shadPS4 -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1
if (-not $emu) { throw 'No shadPS4 window' }
$h = $emu.MainWindowHandle
[D1Shot.W]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [D1Shot.W]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
[void][D1Shot.W]::ShowWindow($h, 9); [void][D1Shot.W]::SetForegroundWindow($h)
Start-Sleep -Milliseconds 400
$r = New-Object D1Shot.W+RECT
[void][D1Shot.W]::GetWindowRect($h, [ref]$r)
$bmp = New-Object Drawing.Bitmap ($r.Right - $r.Left), ($r.Bottom - $r.Top)
$g = [Drawing.Graphics]::FromImage($bmp)
try { $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size); $bmp.Save($Out, [Drawing.Imaging.ImageFormat]::Png); "saved $Out" }
finally { $g.Dispose(); $bmp.Dispose() }
