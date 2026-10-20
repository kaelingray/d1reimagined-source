"""Dump the 56578 client's activity table (index -> activity hash) from live memory.

The table is content (block 1063 of the investment globals): elements of 0x10 bytes
`{u32 hash, u32 pad, i64 self-relative -> definition}`. It is found by scanning committed guest
memory for Skirmish's hash 0xFB5D13A3 with Elimination's 0x6D714C3A exactly 6 elements later
(56578 indices 410 and 416, the two Private-Match/launch indices measured live). Control: the
element count back to the table start must put Skirmish at 410.

  d1-activitytable.py [--out FILE.tsv] [--hash 0x...]
"""
import argparse, ctypes, ctypes.wintypes as wt, os, struct, sys
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'd1-winguest.py')).read()
wg = {'__name__': 'lib'}; exec(compile(src, 'd1-winguest.py', 'exec'), wg)
ap = argparse.ArgumentParser(); ap.add_argument('--out'); ap.add_argument('--hash', action='append', default=[])
ap.add_argument('--count', type=int, default=0, help='rows to dump (default: until the pattern breaks)')
a = ap.parse_args()
p, base = wg['open_client']()
k32 = ctypes.WinDLL('kernel32', use_last_error=True)
class MBI(ctypes.Structure):
    _fields_ = [('BaseAddress', ctypes.c_void_p), ('AllocationBase', ctypes.c_void_p), ('AllocationProtect', wt.DWORD),
                ('PartitionId', wt.WORD), ('RegionSize', ctypes.c_size_t), ('State', wt.DWORD), ('Protect', wt.DWORD), ('Type', wt.DWORD)]
k32.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
SK, EL, GAP = struct.pack('<I', 0xFB5D13A3), struct.pack('<I', 0x6D714C3A), 6 * 0x10
hits, addr, mbi = [], 0x100000000, MBI()
while addr < 0x1000000000 and k32.VirtualQueryEx(p.h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
    b0, sz = mbi.BaseAddress or 0, mbi.RegionSize
    if mbi.State == 0x1000 and mbi.Protect in (0x02, 0x04, 0x08, 0x20, 0x40):
        for c in range(0, sz, 1 << 24):
            try: buf = p.read(b0 + c, min((1 << 24) + GAP + 4, sz - c))
            except OSError: continue
            i = buf.find(SK)
            while i >= 0:
                if buf[i + GAP:i + GAP + 4] == EL: hits.append(b0 + c + i)
                i = buf.find(SK, i + 1)
    addr = b0 + sz
print('Skirmish/Elimination pairs at', [hex(h) for h in hits])
if not hits: sys.exit('activity table not found')
start = hits[0] - 410 * 0x10
rows = []
n = a.count or 1024
raw = p.read(start, n * 0x10)
for i in range(n):
    h, pad, rel = struct.unpack_from('<IIq', raw, i * 0x10)
    if not a.count and (pad != 0 or abs(rel) > 1 << 32): break
    rows.append(h)
ok = len(rows) > 416 and rows[410] == 0xFB5D13A3 and rows[416] == 0x6D714C3A
print(f'table {start:#x}: {len(rows)} rows  control {"PASS" if ok else "FAIL"}')
if not ok: sys.exit(2)
for hx in a.hash:
    h = int(hx, 0); print(hx, '->', [i for i, r in enumerate(rows) if r == h])
if a.out:
    with open(a.out, 'w') as f:
        f.write('index\thash\n')
        for i, h in enumerate(rows): f.write(f'{i}\t{h:#010x}\n')
    print('wrote', a.out)
