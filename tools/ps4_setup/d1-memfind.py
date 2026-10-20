#!/usr/bin/env python3
"""d1-memfind - find a byte pattern in the live client's committed, readable memory. Read-only.

    d1-memfind.py <hex pattern> [--max N]
"""
import ctypes, ctypes.wintypes as wt, sys, os
HERE = os.path.dirname(os.path.abspath(__file__))
g = {}; exec(open(os.path.join(HERE, 'd1-winguest.py'), encoding='utf-8').read().split('def main():')[0], g)

class MBI(ctypes.Structure):
    _fields_ = [('BaseAddress', ctypes.c_void_p), ('AllocationBase', ctypes.c_void_p), ('AllocationProtect', wt.DWORD),
                ('PartitionId', wt.WORD), ('RegionSize', ctypes.c_size_t), ('State', wt.DWORD), ('Protect', wt.DWORD), ('Type', wt.DWORD)]

def main():
    pat = bytes.fromhex(sys.argv[1]); mx = int(sys.argv[sys.argv.index('--max') + 1]) if '--max' in sys.argv else 20
    p, base = g['open_client']()
    k32 = g['k32']; k32.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
    addr, hits, m = 0, 0, MBI()
    while addr < 0x7FFFFFFF0000 and k32.VirtualQueryEx(p.h, ctypes.c_void_p(addr), ctypes.byref(m), ctypes.sizeof(m)):
        size = m.RegionSize; start = m.BaseAddress or 0
        if m.State == 0x1000 and (m.Protect & 0xEE) and not (m.Protect & 0x100):
            off = 0
            while off < size:
                n = min(0x4000000, size - off)
                try:
                    blob = p.read(start + off, n)
                except OSError:
                    break
                i = blob.find(pat)
                while i >= 0:
                    print(hex(start + off + i)); hits += 1
                    if hits >= mx: return 0
                    i = blob.find(pat, i + 1)
                off += n - len(pat) if n > len(pat) else n
        addr = start + size
    print(f'{hits} hit(s)', file=sys.stderr)
    return 0

if __name__ == '__main__':
    sys.exit(main())
