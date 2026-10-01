# Post keys to the running Destiny RPCS3 game window (keyboard pad handler), e.g. Cross = X,
# Circle = C, Start = Return, D-pad = arrows, left stick = WASD; F12 (-Vk 0x7B) takes an
# in-emulator screenshot. Keys are posted, so the window does not need focus.
#   Send-RpcsKey.ps1                      one X press
#   Send-RpcsKey.ps1 -Keys X,X,Down -GapMs 1500
#   Send-RpcsKey.ps1 -Keys D -HoldMs 400  hold (e.g. move the menu cursor with the stick)
param(
    [string[]]$Keys = @(),
    [int]$Vk = 0,
    [int]$Count = 1,
    [int]$HoldMs = 150,
    [int]$GapMs = 600,
    [int]$ProcessId = 0          # 0 = all matching RPCS3 windows (legacy behavior)
)
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Text;
public static class RpcsKeys {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint type);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
}
"@
$named = @{ Left = 0x25; Up = 0x26; Right = 0x27; Down = 0x28; Return = 0x0D; Enter = 0x0D; Space = 0x20;
            Escape = 0x1B; F12 = 0x7B }
$vks = @()
if ($Vk) { $vks += , $Vk }
foreach ($k in $Keys) {
    if ($named.ContainsKey($k)) { $vks += $named[$k] }
    elseif ($k.Length -eq 1) { $vks += [int][char]$k.ToUpper() }
    else { throw "unknown key $k" }
}
if (-not $vks) { $vks = @(0x58) }

$ids = if ($ProcessId) {
    $process = Get-Process -Id $ProcessId -ErrorAction Stop
    if ($process.ProcessName -ne 'rpcs3') { throw "Process $ProcessId is not RPCS3" }
    @($ProcessId)
} else { @((Get-Process rpcs3 -ErrorAction SilentlyContinue).Id) }
$script:game = @()
[RpcsKeys]::EnumWindows({ param($h, $l) $p = 0; [void][RpcsKeys]::GetWindowThreadProcessId($h, [ref]$p)
    if ($ids -contains $p -and [RpcsKeys]::IsWindowVisible($h)) {
        $sb = New-Object Text.StringBuilder 256; [void][RpcsKeys]::GetWindowText($h, $sb, 256)
        if ($sb.ToString() -match 'FPS|Destiny|BLES') { $script:game += $h } }
    $true }, [IntPtr]::Zero) | Out-Null
if (-not $script:game) { throw "Destiny game window not found" }

foreach ($h in $script:game) {
    # a minimized game window presents no frames (F12 then saves nothing): restore it without
    # taking focus, so the user can keep using the desktop
    if ([RpcsKeys]::IsIconic($h)) { [void][RpcsKeys]::ShowWindow($h, 4); Start-Sleep -Milliseconds 500 }
    for ($i = 0; $i -lt $Count; $i++) {
        foreach ($v in $vks) {
            $scan = [RpcsKeys]::MapVirtualKey([uint32]$v, 0)
            $ext = if ($v -ge 0x21 -and $v -le 0x28) { 1 -shl 24 } else { 0 }
            $down = 1 -bor ($scan -shl 16) -bor $ext
            $up = $down -bor (1 -shl 30) -bor (1 -shl 31)
            [void][RpcsKeys]::PostMessage($h, 0x100, [IntPtr]$v, [IntPtr][int64]$down)
            Start-Sleep -Milliseconds $HoldMs
            [void][RpcsKeys]::PostMessage($h, 0x101, [IntPtr]$v, [IntPtr][int64]([uint32]$up))
            Start-Sleep -Milliseconds $GapMs
        }
    }
}
"posted {0} x{1}" -f (($vks | ForEach-Object { '0x{0:X2}' -f $_ }) -join ','), $Count
