"""Dump the 56578 client's unlock-flag table {u32 hash, u8 kind, u16 sub} from live memory.

The rows are found by scanning committed guest memory for row 98's hash with row 2539's hash
exactly (2539-98)*8 bytes later (4 s; the getter 0x25FA30's cache at [0x26527F8+0x270] was empty
in world). 8 bytes per row: hash @0, kind byte @4, sub word @6 (0x28D000 reads kind at +4 and sub
at +6). Controls: row 98 = 3638915056 and row 2539 =
1681748763, the hashes documented for 01.29 ordinals 98 and 2538.

  d1-flagtable.py [--out FILE.tsv] [--idx 1,2,3]
"""
import argparse, os, struct, sys
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'd1-winguest.py')).read()
wg = {'__name__': 'lib'}; exec(compile(src, 'd1-winguest.py', 'exec'), wg)
ap = argparse.ArgumentParser(); ap.add_argument('--out'); ap.add_argument('--idx'); ap.add_argument('--count', type=int, default=7731)
a = ap.parse_args()
p, base = wg['open_client']()
import ctypes, ctypes.wintypes as wt
k32 = ctypes.WinDLL('kernel32', use_last_error=True)
class MBI(ctypes.Structure):
    _fields_ = [('BaseAddress', ctypes.c_void_p), ('AllocationBase', ctypes.c_void_p), ('AllocationProtect', wt.DWORD),
                ('PartitionId', wt.WORD), ('RegionSize', ctypes.c_size_t), ('State', wt.DWORD), ('Protect', wt.DWORD), ('Type', wt.DWORD)]
k32.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
needle, want, gap = struct.pack('<I', 3638915056), struct.pack('<I', 1681748763), (2539 - 98) * 8
hits, addr, mbi = [], 0x100000000, MBI()
while addr < 0x1000000000 and k32.VirtualQueryEx(p.h, ctypes.c_void_p(addr), ctypes.byref(mbi), ctypes.sizeof(mbi)):
    b0, sz = mbi.BaseAddress or 0, mbi.RegionSize
    if mbi.State == 0x1000 and mbi.Protect in (0x02, 0x04, 0x08, 0x20, 0x40):
        for c in range(0, sz, 1 << 24):
            try: buf = p.read(b0 + c, min((1 << 24) + gap + 4, sz - c))
            except OSError: continue
            i = buf.find(needle)
            while i >= 0:
                if buf[i + gap:i + gap + 4] == want: hits.append(b0 + c + i - 98 * 8)
                i = buf.find(needle, i + 1)
    addr = b0 + sz
hits = sorted(set(hits))
if not hits: sys.exit('flag table not found in guest memory')
rows_at = hits[0]
tab = rows_at
raw = p.read(rows_at, a.count * 8)
rows = [struct.unpack_from('<IBBH', raw, i * 8) for i in range(a.count)]
ok = rows[98][0] == 3638915056 and rows[2539][0] == 1681748763
print(f'table {tab:#x} rows {rows_at:#x}  control {"PASS" if ok else "FAIL"} (98={rows[98][0]}, 2539={rows[2539][0]})')
if not ok:
    sys.exit(2)
if a.idx:
    for i in map(int, a.idx.split(',')):
        h, k, x, s = rows[i]; print(i, h, k, s)
if a.out:
    with open(a.out, 'w') as f:
        f.write('index\thash\tkind\tsub\n')
        for i, (h, k, x, s) in enumerate(rows):
            f.write(f'{i}\t{h}\t{k}\t{s}\n')
    print('wrote', a.out)
