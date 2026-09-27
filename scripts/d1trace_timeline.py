"""Summarise the D1T timeline written by the instrumented RPCS3 build (patches/
rpcs3-destiny-main-memory-blit.patch, active while d1/d1trace.on next to rpcs3.exe exists) from an RPCS3.log:
per frame (split at flips), the ordered RSX blits/copies, semaphore acquires (with wait time)
and releases, texture uploads from the SPU-shared region and SPU DMA runs (consecutive gets or
puts by the same SPU collapsed into an address range).

Usage: python scripts/d1trace_timeline.py RPCS3.log [FRAMES]
       python scripts/d1trace_timeline.py --graph RPCS3.log [FRAMES]   (render targets/textures only)
"""

import re
import sys

LINE = re.compile(r"D1T (\d+) (.*)")
SPU = re.compile(r"spu(\S+) (get|put) eal=0x([0-9a-f]+) size=0x([0-9a-f]+)")


def events(path):
    with open(path, encoding="utf-8", errors="replace") as f:
        for line in f:
            m = LINE.search(line)
            if m:
                yield int(m.group(1)), m.group(2).strip()


def frames(evs):
    cur = []
    for t, e in evs:
        if e == "flip":
            yield cur
            cur = []
        else:
            cur.append((t, e))
    if cur:
        yield cur


REGIONS = [("normals", 0x44813A80, 1024 * 624 * 4), ("depth", 0x445A3A80, 1024 * 624 * 4),
           ("mask", 0x444EFA80, 1024 * 624)]


def region(eal):
    for name, base, size in REGIONS:
        if base <= eal < base + size:
            return name
    return None


def spans(frame, t0):
    """Per region and direction: SPU DMA count, first/last time, lowest/highest address."""
    acc = {}
    for t, e in frame:
        m = SPU.match(e)
        if not m:
            continue
        eal = int(m.group(3), 16)
        r = region(eal)
        if not r:
            continue
        key = (r, m.group(2))
        a = acc.setdefault(key, [t, t, eal, eal, 0])
        a[0], a[1], a[2], a[3], a[4] = min(a[0], t), max(a[1], t), min(a[2], eal), max(a[3], eal), a[4] + 1
    return {k: ((v[0] - t0) / 1000, (v[1] - t0) / 1000, v[2], v[3], v[4]) for k, v in acc.items()}


def collapse(frame):
    out, run = [], None
    for t, e in frame:
        m = SPU.match(e)
        if m:
            spu, kind, eal, size = m.group(1), m.group(2), int(m.group(3), 16), int(m.group(4), 16)
            if run and run[1] == spu and run[2] == kind and eal <= run[4] + 0x10000 and eal >= run[3] - 0x10000:
                run[3], run[4], run[5] = min(run[3], eal), max(run[4], eal + size), run[5] + 1
                run[6] = t
                continue
            if run:
                out.append(run)
            run = [t, spu, kind, eal, eal + size, 1, t]
            continue
        if run:
            out.append(run)
            run = None
        out.append((t, e))
    if run:
        out.append(run)
    return out


CALL = re.compile(r"fifo (call|jump) io=0x([0-9a-f]+) addr=0x([0-9a-f]+)")


def command_writers(frame, t0):
    """For each FIFO call/jump target, the SPU puts into [target, target+64K) in the same frame."""
    out = []
    for t, e in frame:
        m = CALL.match(e)
        if not m:
            continue
        target = int(m.group(3), 16)
        puts = [(tt, int(mm.group(3), 16)) for tt, ee in frame
                if (mm := SPU.match(ee)) and mm.group(2) == "put" and target <= int(mm.group(3), 16) < target + 0x10000]
        if puts:
            last = max(tt for tt, _ in puts)
            out.append(f"  {m.group(1)} 0x{target:x} at +{(t - t0) / 1000:.2f}ms: {len(puts)} SPU puts, last at +{(last - t0) / 1000:.2f}ms"
                       + ("  <-- written AFTER the RSX got there" if last > t else ""))
    return out


GRAPH = ("rtt[", "zeta ", "tex ", "blit ", "memcpy ")


def graph(path, limit):
    """Render graph per frame: render/depth target setups, texture samples and copies, with
    consecutive duplicates collapsed."""
    shown = 0
    for frame in frames(events(path)):
        items = [(t, e) for t, e in sorted(frame) if e.startswith(GRAPH)]
        if len(items) < 5:
            continue
        t0 = items[0][0]
        print(f"=== frame ({len(items)} graph events)")
        prev, count = None, 0
        for t, e in items:
            if e == prev:
                count += 1
                continue
            if prev is not None and count > 1:
                print(f"            (x{count})")
            print(f"  +{(t - t0) / 1000:8.2f}ms {e}")
            prev, count = e, 1
        shown += 1
        if shown >= limit:
            break


def main():
    if sys.argv[1] == "--graph":
        graph(sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 2)
        return
    path = sys.argv[1]
    limit = int(sys.argv[2]) if len(sys.argv) > 2 else 3
    shown = 0
    for frame in frames(events(path)):
        if len(frame) < 5:
            continue
        frame.sort()
        t0 = frame[0][0]
        print(f"=== frame ({len(frame)} events, {(frame[-1][0] - t0) / 1000:.1f} ms)")
        for t, e in frame:
            if not SPU.match(e):
                print(f"  +{(t - t0) / 1000:8.2f}ms {e}")
        for (r, kind), (a, b, lo, hi, n) in sorted(spans(frame, t0).items(), key=lambda kv: kv[1][0]):
            print(f"  spu {kind:3} {r:8} x{n:<5} +{a:7.2f} .. +{b:7.2f}ms  0x{lo:x}-0x{hi:x}")
        for line in command_writers(frame, t0):
            print(line)
        shown += 1
        if shown >= limit:
            break


if __name__ == "__main__":
    main()
