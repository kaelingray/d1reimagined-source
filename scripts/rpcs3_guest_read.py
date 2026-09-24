"""Read Destiny guest memory from the running local RPCS3 test instance (read-only).

Finds the guest base by locating a known EBOOT string, then prints each VA:LEN given,
e.g. `python scripts/rpcs3_guest_read.py 1986228:100` shows the last fatal-halt message
(release_vhalt buffer). Windows only; BLES01857 01.00 addresses.
"""
import ctypes, ctypes.wintypes as wt, subprocess, sys
k32 = ctypes.WinDLL("kernel32", use_last_error=True)
class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p), ("AllocationProtect", wt.DWORD),
                ("PartitionId", wt.WORD), ("RegionSize", ctypes.c_size_t), ("State", wt.DWORD), ("Protect", wt.DWORD), ("Type", wt.DWORD)]
k32.OpenProcess.restype = wt.HANDLE
k32.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
k32.ReadProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
pid = int(subprocess.check_output(["powershell", "-NoProfile", "-Command", "(Get-Process rpcs3).Id"]).split()[0])
h = k32.OpenProcess(0x0410, False, pid)
def rpm(addr, n):
    buf = ctypes.create_string_buffer(n); got = ctypes.c_size_t()
    if not k32.ReadProcessMemory(h, ctypes.c_void_p(addr), buf, n, ctypes.byref(got)): return None
    return buf.raw[:got.value]
def find_base():
    pat = b"The game disc cannot be read.  Please quit the game using the PS button.\x00\x00\x00\x00XStringVerify"
    addr, mbi = 0, MBI()
    while k32.VirtualQueryEx(h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
        base, size = mbi.BaseAddress or 0, mbi.RegionSize
        if mbi.State == 0x1000 and mbi.Protect & 0xEE and not mbi.Protect & 0x100 and size < 1 << 30:
            data = rpm(base, size)
            if data:
                i = data.find(pat)
                while i != -1:
                    cand = base + i - 0x122dec8
                    if rpm(cand + 0x11b9710, 4) == b"NONE":  # second EBOOT string as a check
                        return cand
                    i = data.find(pat, i + 1)
        addr = base + size
    return None
if __name__ == "__main__":
    b = find_base(); print("guest base", hex(b) if b else None)
    for a in sys.argv[1:]:
        va, n = (int(x, 16) for x in a.split(":"))
        print(hex(va), rpm(b + va, n))
