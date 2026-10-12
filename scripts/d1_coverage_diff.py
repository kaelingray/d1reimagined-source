"""Diff the first-call coverage (D1C lines) of two RPCS3 runs of the diagnostic build.

With `coverage` in d1/d1ppu.txt the diagnostic RPCS3 logs one line per PPU function on its first
call: `<level> <emu time> {thread} PPU: D1C <tick> pc=0x.. lr=0x..`. This lists the functions only
one of two runs reached (with their first-call time, thread and caller), optionally limited to
an address range and to calls after a time, e.g. to see where a passing boot diverges from a
stuck one:

  python scripts/d1_coverage_diff.py STUCK-rpcs3.log PASS-rpcs3.log --after 0:01:00
"""
import argparse
import re
import sys

LINE = re.compile(r"^\S+ (\d+):(\d+):(\d+(?:\.\d+)?) \{([^}]*)\} PPU: D1C \d+ pc=0x([0-9a-f]+) lr=0x([0-9a-f]+)")


def seconds(text):
    parts = [float(p) for p in text.split(":")]
    while len(parts) < 3:
        parts.insert(0, 0.0)
    return parts[0] * 3600 + parts[1] * 60 + parts[2]


def first_calls(path):
    """{pc: (emu seconds, thread, lr)} for the first D1C line of each pc."""
    calls = {}
    with open(path, encoding="utf-8", errors="replace") as log:
        for line in log:
            if "D1C" not in line:
                continue
            m = LINE.match(line)
            if not m:
                continue
            pc = int(m.group(5), 16)
            if pc not in calls:
                t = int(m.group(1)) * 3600 + int(m.group(2)) * 60 + float(m.group(3))
                thread = m.group(4).split(" [")[0]
                calls[pc] = (t, thread, int(m.group(6), 16))
    return calls


def only_in(a, b, after=0.0, low=0, high=0xFFFFFFFF):
    rows = [(t, pc, thread, lr) for pc, (t, thread, lr) in a.items()
            if pc not in b and t >= after and low <= pc <= high]
    return sorted(rows)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("first")
    parser.add_argument("second")
    parser.add_argument("--after", default="0", help="only first calls at or after this emulator time (h:m:s)")
    parser.add_argument("--range", default=None, help="LOW-HIGH hex address range of the pc")
    parser.add_argument("--limit", type=int, default=200)
    args = parser.parse_args(argv)
    low, high = 0, 0xFFFFFFFF
    if args.range:
        lo, hi = args.range.split("-")
        low, high = int(lo, 16), int(hi, 16)
    a, b = first_calls(args.first), first_calls(args.second)
    print(f"{args.first}: {len(a)} functions; {args.second}: {len(b)} functions; common {len(set(a) & set(b))}")
    for label, x, y in (("only in first", a, b), ("only in second", b, a)):
        rows = only_in(x, y, seconds(args.after), low, high)
        print(f"\n{label}: {len(rows)}")
        for t, pc, thread, lr in rows[:args.limit]:
            print(f"  {t:9.3f}  pc=0x{pc:08x}  lr=0x{lr:08x}  {thread}")
        if len(rows) > args.limit:
            print(f"  ... {len(rows) - args.limit} more")
    return 0


if __name__ == "__main__":
    sys.exit(main())
