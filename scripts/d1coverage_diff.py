"""Compare first-call coverage (D1C lines of the instrumented RPCS3 build, watch file `coverage`)
between two runs: functions reached only in run B, in B's first-call order, with their callers.

Used to see how far a code path gets in one configuration compared with another, e.g. the
intro cinematic slice set (zone set 113) against the gameplay slice set (112).

Usage: python scripts/d1coverage_diff.py A-rpcs3.log B-rpcs3.log [--after SECONDS] [--limit N]
  --after: only B functions first called at least SECONDS into the emulation (skips boot)
  --lr ADDR: only calls with this return address (e.g. 0x276230, the replicated-state apply site)
"""

import argparse
import re

LINE = re.compile(r"^\S+ (\d+):(\d+):([\d.]+) \{([^}]*)\} PPU: D1C \d+ pc=(0x[0-9a-f]+) lr=(0x[0-9a-f]+)")


def coverage(path):
    """{pc: (seconds, lr, thread)} for the first call of each function."""
    first = {}
    with open(path, "rb") as f:
        for raw in f:
            if b" D1C " not in raw:
                continue
            m = LINE.match(raw.decode("utf-8", "replace"))
            if not m:
                continue
            h, mi, s, thread, pc, lr = m.groups()
            pc = int(pc, 16)
            if pc not in first:
                first[pc] = (int(h) * 3600 + int(mi) * 60 + float(s), int(lr, 16), thread)
    return first


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("--after", type=float, default=0)
    ap.add_argument("--limit", type=int, default=200)
    ap.add_argument("--lr", type=lambda v: int(v, 0))
    args = ap.parse_args()
    a, b = coverage(args.a), coverage(args.b)
    print(f"A: {len(a)} functions, B: {len(b)} functions, only in B: {len(set(b) - set(a))}")
    rows = sorted((t, pc, lr, th) for pc, (t, lr, th) in b.items()
                  if pc not in a and t >= args.after and (args.lr is None or lr == args.lr))
    for t, pc, lr, th in rows[:args.limit]:
        print(f"{t:10.3f}  pc={pc:#x}  lr={lr:#x}  {th}")


if __name__ == "__main__":
    main()
