"""Dump Destiny (BLES01857 01.00) runtime type definitions and web services from a running
local RPCS3 test instance (read-only guest memory access via rpcs3_guest_read.py).

Tag handles resolve through the instance table at *(*0x0175c1f0) (as in 0x1c13a0):
  idx = (h >> 13) masked, entry = table + idx*64,
  p = [entry+0x10] + (h & 0x1fff) * [entry+0x30]; def = p - ([p+8] & [entry+0x34]).
A type definition holds its struct size at +0x14 and a field list at def+0x54+[def+0x54].
Primitive readers (table 0x175d5c8) decode bits - min: kind 2 bool (1 bit), 3/7 u8,
4/8 u16, 5/9 u32, 6/10 u64 (width bits each), 11 real32 (32 bits when +0x20 is 0).
u32 count, 16-byte header, then 0x28-byte entries:
  +0 struct offset, +8 presence-bit index, +0x10 kind (1 nested struct, 3/4/5/6/7/9/0xa
  primitives), +0x11 optional (1 presence bit precedes the field), +0x14 nested type
  handle, +0x18 array mode (1: element count read from the parent struct), +0x1c bit width.
The bit stream is MSB-first (0x25d2dc -> 0xd0ecd8).

Web services: table from registry *(0x0175cbcc) -> resolve([reg+8]) -> resolve([+0x20c]) + 0xc;
record i at table + [table+4] + i*36 + 0x14: +8 u16 wire id, +0xc request type, +0x10
response type. Most responses start with type 0x80801796 = {u4 result, i32 detail}; result 0
is error_none (0x41a2f8).

Usage: python scripts/rpcs3_tiger_types.py services | wire <id> | type <handle>
       python scripts/rpcs3_tiger_types.py schema OUT.json HANDLE... (type trees as JSON)
"""

import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
_argv, sys.argv = sys.argv, sys.argv[:1]
import rpcs3_guest_read as g  # noqa: E402
sys.argv = _argv

B = g.find_base()


def rd(a, n):
    data = g.rpm(B + a, n)
    if data is None:
        raise ValueError(f"guest address {a:#x} is not readable")
    return data


def u32(a):
    return struct.unpack(">I", rd(a, 4))[0]


def u16(a):
    return struct.unpack(">H", rd(a, 2))[0]


def resolve(h):
    r6 = ((h - (1 << 32) if h & 0x80000000 else h) >> 13) & 0xFFFFFFFF
    idx = r6 & ((r6 >> 18) | 0x3FF)
    entry = u32(u32(0x175C1F0)) + idx * 64
    p = u32(entry + 0x10) + (h & 0x1FFF) * u32(entry + 0x30)
    return (p - (u32(p + 8) & u32(entry + 0x34))) & 0xFFFFFFFF


def fields(h):
    p = resolve(h)
    fl = p + 0x54 + u32(p + 0x54)
    out = []
    for i in range(u32(fl)):
        e = fl + 0x14 + i * 0x28
        # For primitives, +0x18 is the minimum value (decoded = bits - min, 0xd1c428) and
        # +0x1c the bit width; for nested fields the byte at +0x18 is the array mode.
        out.append(dict(off=u32(e), bit=u32(e + 8), kind=rd(e + 0x10, 1)[0], opt=rd(e + 0x11, 1)[0],
                        nested=u32(e + 0x14), arr=rd(e + 0x18, 1)[0], min=u32(e + 0x18),
                        width=u32(e + 0x1C), extra=u32(e + 0x20)))
    return p, u32(p + 0x14), out


def dump(h, depth=0, seen=None):
    seen = set() if seen is None else seen
    p, size, fs = fields(h)
    print("  " * depth + f"type {h:#x} (def {p:#x}) size {size:#x}, {len(fs)} fields")
    for f in fs:
        print("  " * depth + "  - " + " ".join(f"{k}={v:#x}" for k, v in f.items()))
        if f["kind"] == 1 and f["nested"] not in seen and depth < 8:
            seen.add(f["nested"])
            dump(f["nested"], depth + 2, seen)


def services():
    t = resolve(u32(resolve(u32(u32(0x175CBCC) + 8)) + 0x20C)) + 0xC
    for i in range(u32(t)):
        d = t + u32(t + 4) + i * 36 + 0x14
        yield i, u16(d + 8), u32(d + 0xC), u32(d + 0x10)


def schema(handles, out=None):
    """Collect type trees as {handle: {"size": n, "fields": [...]}} for server-side encoders."""
    out = {} if out is None else out
    todo = list(handles)
    while todo:
        h = todo.pop()
        key = f"{h:#x}"
        if key in out:
            continue
        _, size, fs = fields(h)
        out[key] = {"size": size, "fields": fs}
        todo += [f["nested"] for f in fs if f["kind"] == 1]
    return out


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "services"
    if cmd == "services":
        for i, wire, req, rsp in services():
            print(f"{i:3} wire {wire:5} request {req:#x} response {rsp:#x}")
    elif cmd == "wire":
        for i, wire, req, rsp in services():
            if wire == int(sys.argv[2], 0):
                print("request:"); dump(req)
                print("response:"); dump(rsp)
    elif cmd == "type":
        dump(int(sys.argv[2], 16))
    elif cmd == "schema":
        # Types for the given handles plus every web-service request and response type
        # (requests name what each service does); "services" maps wire id -> [request
        # handle, response handle].
        import json
        svc = {wire: [req, rsp] for _, wire, req, rsp in services()}
        out = schema([int(h, 16) for h in sys.argv[3:]] + [h for pair in svc.values() for h in pair])
        out["services"] = {str(w): [f"{a:#x}", f"{b:#x}"] for w, (a, b) in svc.items()}
        Path(sys.argv[2]).write_text(json.dumps(out, indent=0))
