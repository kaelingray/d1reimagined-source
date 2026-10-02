"""Find the local player's position in a running 01.24 session (read-only memory scan).

Takes snapshots of the guest's committed memory between scripted moves (keyboard pad: hold W,
stay still, hold W, hold S) and keeps aligned big-endian float32 values that change during every
move and stay put while standing. Candidates are reported as clusters (position vectors are three
neighbouring floats). Reads only; the moves go through Send-RpcsKey.ps1 (the owned emulator).

    python scripts/rpcs3_position_scan.py --manifest <run>-processes.json [--hold-ms 1500]
"""
import argparse
import ctypes
import subprocess
import time
from ctypes import wintypes as wt
from pathlib import Path

import numpy as np

from rpcs3_owned_memory import OwnedMemory

ROOT = Path(__file__).resolve().parent


class MBI(ctypes.Structure):
    _fields_ = [("BaseAddress", ctypes.c_void_p), ("AllocationBase", ctypes.c_void_p),
                ("AllocationProtect", wt.DWORD), ("PartitionId", wt.WORD), ("RegionSize", ctypes.c_size_t),
                ("State", wt.DWORD), ("Protect", wt.DWORD), ("Type", wt.DWORD)]


def guest_regions(mem, low=0x00010000, high=0xF0000000):
    """Committed, readable guest ranges [(guest address, size)] between low and high."""
    k = mem.kernel
    k.VirtualQueryEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.POINTER(MBI), ctypes.c_size_t]
    k.VirtualQueryEx.restype = ctypes.c_size_t
    regions, address = [], mem.base + low
    while address < mem.base + high:
        info = MBI()
        if not k.VirtualQueryEx(mem.handle, ctypes.c_void_p(address), ctypes.byref(info), ctypes.sizeof(info)):
            break
        start, size = info.BaseAddress or 0, info.RegionSize
        readable = info.Protect & 0xEE and not info.Protect & 0x100  # any read access, not a guard page
        if info.State == 0x1000 and readable:
            regions.append((start - mem.base, size))
        address = start + size
    merged = []
    for start, size in regions:
        if merged and merged[-1][0] + merged[-1][1] == start:
            merged[-1] = (merged[-1][0], merged[-1][1] + size)
        else:
            merged.append((start, size))
    return [(max(s, low), min(s + n, high) - max(s, low)) for s, n in merged if s + n > low and s < high]


def snapshot(mem, regions, chunk=16 << 20):
    parts = []
    for start, size in regions:
        for offset in range(0, size, chunk):
            n = min(chunk, size - offset)
            try:
                data = mem.read(start + offset, n)
            except OSError:
                data = bytes(n)
            parts.append(np.frombuffer(data, dtype=">f4", count=n // 4))
    return np.concatenate(parts)


def addresses(regions):
    return np.concatenate([np.arange(s, s + n - 3, 4, dtype=np.uint32)[: n // 4] for s, n in regions])


def key(keys, hold_ms):
    subprocess.run(["pwsh", "-NoProfile", "-File", str(ROOT / "Send-RpcsKey.ps1"), "-Keys", keys,
                    "-HoldMs", str(hold_ms)], check=True, capture_output=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--manifest", required=True, type=Path)
    ap.add_argument("--hold-ms", type=int, default=1500)
    ap.add_argument("--limit", type=float, default=50000.0, help="largest plausible coordinate")
    args = ap.parse_args()
    with OwnedMemory(args.manifest) as mem:
        regions = guest_regions(mem)
        print(f"{len(regions)} regions, {sum(n for _, n in regions) >> 20} MiB")
        addr = addresses(regions)
        # moves: (key, direction sign relative to the first move); None = stand still
        steps = [None, ("W", 1), (None, 0), ("W", 1), ("S", -1)]
        idx, first_sign, previous = None, None, None
        for step in steps:
            if step and step[0]:
                key(step[0], args.hold_ms)
            elif step:
                time.sleep(args.hold_ms / 1000)
            time.sleep(0.4)
            current = snapshot(mem, regions)
            if previous is None:
                with np.errstate(invalid="ignore", over="ignore"):
                    idx = np.flatnonzero(np.isfinite(current) & (np.abs(current) < args.limit)
                                         & (np.abs(current) > 1))
                previous = current[idx]
                print(f"start: {len(idx)} candidates")
                continue
            now = current[idx]
            with np.errstate(invalid="ignore", over="ignore"):
                delta = now - previous
                if step[1] == 0:
                    keep = now == previous
                else:
                    keep = np.isfinite(now) & (np.abs(delta) > 0.05) & (np.abs(delta) < 100)
                    sign = np.sign(delta)
                    if first_sign is None:
                        first_sign = sign
                    else:
                        keep &= sign == first_sign * step[1]
            idx, previous = idx[keep], now[keep]
            if first_sign is not None and len(first_sign) != len(idx):
                first_sign = first_sign[keep] if len(first_sign) == len(keep) else first_sign
            print(f"after {step}: {len(idx)} candidates")
        hits = idx
        values = previous
        cluster = [int(np.sum(np.abs(hits.astype(np.int64) - int(i)) <= 2)) for i in hits]
        order = sorted(range(len(hits)), key=lambda i: -cluster[i])[:80]
        for i in sorted(order, key=lambda i: hits[i]):
            print(f"{addr[hits[i]]:#010x} {values[i]:12.3f} cluster {cluster[i]}")


if __name__ == "__main__":
    main()
