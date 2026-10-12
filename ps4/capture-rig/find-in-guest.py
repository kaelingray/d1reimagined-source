#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Search the running client's memory for a value, and dump context around each hit.

WHY: `error.no_investment_connection` is a 30-second timeout. It cannot distinguish "the handler
never ran" from "the handler ran and rejected our data" - and we have burned many runs on that
ambiguity (historicaldocs/design/character-and-orbit.md §15.1). The client is an ordinary Linux process, so the
question is directly observable instead.

Use a MAGIC account SOID (e.g. 0xCAFEBABEDEADBEEF) so every hit is unambiguously ours:

  no hits          -> our response never decoded into a struct; the handler never saw it
  hits             -> the value landed; the surrounding bytes say WHERE (decoded message struct?
                      account record?), and an account record is 0x30 bytes with a small state
                      byte at offset 0 and the SOID at +0x10 (§6 of PUZZLE-503-account-login.md)

Usage:
    ./find-in-guest.py 0xCAFEBABEDEADBEEF
    ./find-in-guest.py 0xCAFEBABEDEADBEEF --context 64
"""
import argparse
import re
import struct
import subprocess
import sys


def emu_pid():
    out = subprocess.run(["pgrep", "-f", "[s]hadps4 -p "], capture_output=True, text=True).stdout
    pids = [int(x) for x in out.split()]
    if not pids:
        raise SystemExit("no running shadps4")
    return pids[0]


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0],
                                 formatter_class=argparse.RawDescriptionHelpFormatter,
                                 epilog=__doc__)
    ap.add_argument("value", type=lambda s: int(s, 0), help="64-bit value to hunt for")
    ap.add_argument("--context", type=int, default=48, help="bytes of context each side of a hit")
    ap.add_argument("--max", type=int, default=20, help="stop after this many hits")
    args = ap.parse_args()

    pid = emu_pid()
    le = args.value.to_bytes(8, "little")
    be = args.value.to_bytes(8, "big")
    print(f"pid {pid}; hunting {args.value:#018x}  (LE {le.hex()} / BE {be.hex()})")

    # ⚠ Do NOT filter by region SIZE. shadPS4 reserves enormous sparse ranges (87 TiB of address
    # space in one measurement) and the guest heap lives INSIDE them - an earlier version skipped any
    # region over 1 GiB and thereby searched 0.01% of memory while reporting confident "0 hits".
    # Filter by RESIDENT pages instead: /proc/<pid>/smaps gives Rss per mapping, so we touch only
    # memory that is actually committed.
    regions = []
    cur = None
    for line in open(f"/proc/{pid}/smaps"):
        m = re.match(r"([0-9a-f]+)-([0-9a-f]+) (\S{4})", line)
        if m:
            cur = (int(m.group(1), 16), int(m.group(2), 16), m.group(3), line.rstrip())
            continue
        if cur and line.startswith("Rss:"):
            rss = int(line.split()[1]) * 1024
            if rss > 0 and "r" in cur[2]:
                regions.append((cur[0], cur[1], f"{cur[3]}  rss={rss//1024}kB"))
            cur = None
    total = sum(hi - lo for lo, hi, _ in regions)
    print(f"searching {len(regions)} resident region(s), {total/2**20:.0f} MiB")

    hits = []
    with open(f"/proc/{pid}/mem", "rb", 0) as f:
        for lo, hi, desc in regions:
            try:
                f.seek(lo)
                blob = f.read(hi - lo)
            except Exception:
                continue
            if not blob:
                continue
            for pat, order in ((le, "LE"), (be, "BE")):
                start = 0
                while True:
                    j = blob.find(pat, start)
                    if j < 0:
                        break
                    a = lo + j
                    c0 = max(0, j - args.context)
                    hits.append((a, order, blob[c0:j + 8 + args.context], j - c0, desc))
                    start = j + 1
                    if len(hits) >= args.max:
                        break
                if len(hits) >= args.max:
                    break
            if len(hits) >= args.max:
                break

    print(f"\n{len(hits)} hit(s)" + ("  - the value NEVER LANDED" if not hits else ""))
    for a, order, blob, off, desc in hits:
        print(f"\n  {a:#014x} [{order}]   {desc.split()[-1] if len(desc.split())>5 else ''}")
        base = a - off
        for i in range(0, len(blob), 16):
            mark = " <<<" if i <= off < i + 16 else ""
            print(f"    {base+i:#014x}  {blob[i:i+16].hex(' ')}{mark}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
