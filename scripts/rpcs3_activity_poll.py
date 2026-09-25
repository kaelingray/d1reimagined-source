"""Poll the activity-join state of the running local RPCS3 test instance (read-only guest
memory reads through rpcs3_guest_read.py; no GDB connection needed, so it can run beside a
GDB trace). Prints a line whenever a watched value changes.

Watched (BLES01857 01.00):
  rm        resource manager ("activity loader") active scenario name *(*(0x18b477c)) and
            its async load handle (+0x290, -1 = idle)
  sel       investment_activity_selection_manager *(0x1906960): i16 destination, u8 flag,
            u32 selected activity (0 = travel to the destination without an activity)
  req       engine request flags *(0x18a4328)+0x114 (slot 3 = change world; 1 posted,
            2/3 in progress)
  load      world-load byte 0x18a9670
  fireteam  group manager *(0x18ea68c) group 0: current record (+0x450) valid/value and
            target record (+0x7f8) valid/destination
  index     world controller activity index *(0x18ea60c)+0x94 and *(0x18ea570)+0x238

Usage: python scripts/rpcs3_activity_poll.py [SECONDS]
"""

import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
_argv, sys.argv = sys.argv, sys.argv[:1]
import rpcs3_guest_read as g  # noqa: E402
sys.argv = _argv

B = g.find_base()


def rd(a, n):
    return g.rpm(B + a, n) or b""


def u32(a):
    d = rd(a, 4)
    return struct.unpack(">I", d)[0] if len(d) == 4 else None


def snapshot():
    out = {}
    rm = u32(0x18B477C)
    if rm:
        out["rm"] = (rd(rm, 48).split(b"\0")[0].decode("latin1"), hex(u32(rm + 0x290) or 0))
    sel = u32(0x1906960)
    if sel:
        out["sel"] = rd(sel, 8).hex()
    req = u32(0x18A4328)
    if req:
        out["req"] = rd(req + 0x114, 0x1C).hex()
    out["load"] = rd(0x18A9670, 1).hex()
    mgr = u32(0x18EA68C)
    if mgr:
        cur = u32(mgr + 8)
        if cur in (0, 1):
            data = mgr + 0x10 + cur * 0x8C98 + 0x63B0
            c, t = rd(data + 0x450, 0x3A8), rd(data + 0x7F8, 0x3A8)
            if len(c) == 0x3A8 and len(t) == 0x3A8:
                out["fireteam"] = (c[0xD0], c[0xD4:0xD8].hex(), t[0xD0], t[0xD8:0xDC].hex())
    x = u32(0x18EA60C)
    wc = u32(0x18EA570)
    out["index"] = (hex(u32(x + 0x94) or 0) if x else None, hex(u32(wc + 0x238) or 0) if wc else None)
    return out


def main():
    end = time.time() + (float(sys.argv[1]) if len(sys.argv) > 1 else 900)
    last = {}
    while time.time() < end:
        snap = snapshot()
        changed = {k: v for k, v in snap.items() if last.get(k) != v}
        if changed:
            print(time.strftime("%H:%M:%S"), " ".join(f"{k}={v}" for k, v in changed.items()), flush=True)
        last = snap
        time.sleep(0.05)


if __name__ == "__main__":
    main()
