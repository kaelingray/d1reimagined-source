# Which RPCS3 threads are burning CPU: samples each thread's CPU time over -Seconds and prints
# the busiest with their names (RPCS3 names its PPU/SPU/RSX threads, e.g. "PPU[0x1000003] Thread
# (fios mediathread 2)"). Used by Invoke-JoinCycle.ps1 when the emulator freezes (the login-cinematic
# livelock spins several threads at 100 %).
#   Get-RpcsHotThreads.ps1 [-Seconds 5] [-Top 16] [-ProcessName rpcs3]
param([int]$Seconds = 5, [int]$Top = 16, [string]$ProcessName = 'rpcs3')
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class RpcsThreads {
  [DllImport("kernel32.dll", SetLastError = true)] public static extern IntPtr OpenThread(uint access, bool inherit, uint id);
  [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
  [DllImport("kernel32.dll")] public static extern int GetThreadDescription(IntPtr h, out IntPtr desc);
  [DllImport("kernel32.dll")] public static extern IntPtr LocalFree(IntPtr p);
  public static string Name(uint id) {
    IntPtr h = OpenThread(0x0040, false, id);  // THREAD_QUERY_INFORMATION (the limited right is refused)
    if (h == IntPtr.Zero) return "";
    try {
      IntPtr p; if (GetThreadDescription(h, out p) < 0 || p == IntPtr.Zero) return "";
      string s = Marshal.PtrToStringUni(p); LocalFree(p); return s;
    } finally { CloseHandle(h); }
  }
}
"@
$proc = Get-Process $ProcessName -ErrorAction Stop | Select-Object -First 1
$before = @{}
foreach ($t in $proc.Threads) { try { $before[$t.Id] = $t.TotalProcessorTime.TotalMilliseconds } catch {} }
Start-Sleep $Seconds
$proc = Get-Process -Id $proc.Id
$rows = foreach ($t in $proc.Threads) {
    try { $ms = $t.TotalProcessorTime.TotalMilliseconds - $before[$t.Id] } catch { continue }
    [pscustomobject]@{ Cpu = [math]::Round(100 * $ms / (1000 * $Seconds)); Id = $t.Id; State = $t.ThreadState; Name = [RpcsThreads]::Name([uint32]$t.Id) }
}
"$ProcessName pid $($proc.Id), $($proc.Threads.Count) threads, sampled $Seconds s (Cpu = % of one core)"
$rows | Sort-Object Cpu -Descending | Select-Object -First $Top | Format-Table -AutoSize | Out-String -Width 200
