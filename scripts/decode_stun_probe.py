"""Decode the 01.24 Demonware STUN probe from local run logs.

The 01.00-era stub only understood two fixed binary replies (types 30/20 -> 31/21,
see stun_stub.py). 01.24 sends something else entirely: two shapes captured on
this machine from RUN A01:

    len=4   00 01 00 01
    len=29  0e 02 00 00 ... ff 00 ff 00 00 00 ff 00 ff 00 00

Decoding the second one by hand: byte 0 is 0x0e, which in MessagePack is a
fixmap with 14 elements, i.e. seven key/value pairs. Every following byte is a
MessagePack integer (positive fixint 0..127, negative fixint -1), so the body is
seven integer pairs. The -1 values are almost certainly "not bound / unset"
port fields: the client is probing before it has a socket address to report.

So 01.24's STUN request is msgpack, not the 01.00 fixed layout. That is why
stun_stub.py answers neither shape ("unknown type 00" / "unknown type 0e").

This script prints the decoded map from a stun log so the reply can be shaped to
match. It does not guess the reply format - the client's own parser is the only
authority for that (notes/final-build.md, "01.24 STUN response format remains
unknown"; the request-side codec sits behind the vtable slots called from
0xb3012c).

    python decode-stun.py FINAL124_S01-stun.log
    python decode-stun.py FINAL124_S01-stun.log --unique
"""

import argparse
import re
from collections import OrderedDict

LINE = re.compile(r"len=(\d+)\s+((?:[0-9a-f]{2}\s+)+[0-9a-f]{2})")


def decode_msgpack_map(buf, pos=0):
    """Decode a MessagePack map of ints -> ints.

    Returns (pairs, next_pos) where pairs is a list of (key, value) in wire
    order. A list, not a dict: these maps repeat keys (several distinct sockets
    reported in one request), and collapsing them would lose the real structure.
    """
    b = buf[pos]
    if b >> 4 == 0x0:                      # fixmap, 0..15 pairs
        pairs = b & 0x0F
        pos += 1
    elif b == 0x8F:                        # map16
        pairs = int.from_bytes(buf[pos + 1:pos + 3], "big")
        pos += 3
    elif b == 0xDE:                        # map32
        pairs = int.from_bytes(buf[pos + 1:pos + 5], "big")
        pos += 5
    else:
        raise ValueError(f"not a map at {pos}: 0x{b:02x}")

    out = []
    for _ in range(pairs):
        k, pos = _int(buf, pos)
        v, pos = _int(buf, pos)
        out.append((k, v))
    return out, pos


def _int(buf, pos):
    b = buf[pos]
    if b <= 0x7F:                          # positive fixint
        return b, pos + 1
    if b >= 0xE0:                          # negative fixint
        return b - 0x100, pos + 1
    if b == 0xCC:
        return buf[pos + 1], pos + 2
    if b == 0xCD:
        return int.from_bytes(buf[pos + 1:pos + 3], "big"), pos + 3
    if b == 0xCE:
        return int.from_bytes(buf[pos + 1:pos + 5], "big"), pos + 5
    raise ValueError(f"not an int at {pos}: 0x{b:02x}")


def describe(pkt):
    """A short, human-readable shape for one probe."""
    head = f"len={len(pkt):<3} {pkt[:2].hex(' ')}"
    if len(pkt) == 4 and pkt == bytes([0, 1, 0, 1]):
        return head + "  fixed 4-byte probe: type=0x00 version=0x01 f=0x0001"
    if pkt[0] & 0xF0 == 0x00 and pkt[0] <= 0x0F:
        try:
            m, end = decode_msgpack_map(pkt)
            pairs = ", ".join(f"{k}={v}" for k, v in m)
            tail = "" if end == len(pkt) else f" (+{len(pkt) - end} trailing bytes)"
            return head + f"  msgpack map({len(m)}): {pairs}{tail}"
        except ValueError as exc:
            return head + f"  looks like msgpack but decode failed: {exc}"
    return head + "  unrecognised shape"


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("log", help="stun stub log from a run")
    ap.add_argument("--unique", action="store_true", help="one line per distinct shape")
    ap.add_argument("--limit", type=int, default=0)
    args = ap.parse_args()

    seen = OrderedDict()
    with open(args.log, "r", encoding="utf-8", errors="replace") as fh:
        for raw in fh:
            m = LINE.search(raw)
            if not m:
                continue
            pkt = bytes(int(x, 16) for x in m.group(2).split())
            if len(pkt) != int(m.group(1)):
                continue                      # a reply, not a probe
            seen.setdefault(pkt, 0)
            seen[pkt] += 1

    if not seen:
        print("no probes found in", args.log)
        return

    print(f"{args.log}: {len(seen)} distinct probe shape(s)\n")
    for i, (pkt, count) in enumerate(seen.items()):
        if args.limit and i >= args.limit:
            print(f"  ... {len(seen) - args.limit} more")
            break
        print(f"  x{count:<5} {describe(pkt)}")
        print(f"          raw: {pkt.hex(' ')}")


if __name__ == "__main__":
    main()
