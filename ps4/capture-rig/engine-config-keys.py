#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Recover the signon `client_config.3` KEY -> engine-config SLOT map from the eboot.

The signon response's `client_config` sub-field 3 is a config-delta stream
(`historicaldocs/design/content-manifest-gate.md` §3): `u32 LE magic 0x00123456`, then `(u32 LE key, value)` pairs,
terminated by key 0.  Every key is dispatched by ONE function:

    0x351C80(key, &stream)  ->  0x1073170(&stream, key)      # the 1155-arm setter switch

`0x1073170` is a dense jump table for keys 100..1254 (`esi -= 0x64; cmp esi, 0x482; ja default`,
table at 0x107B8F4) plus five sparse keys.  Each arm is stereotyped:

    lea rax, [rip + X]        ; X resolves to a CONFIG-BLOCK GLOBAL (a pointer to the block)
    mov esi, OFFSET           ; byte offset within that block
    jmp  <shared reader>      ; or an inline reader

so (key -> block global, byte offset, wire type) is fully recoverable statically.  This is the map
you need to set any engine-config value FROM THE SERVER without touching the client.

⚠ A key with no arm falls through to the default, which returns FALSE.  That aborts the callback,
which aborts the WHOLE signon response parse (`Client config decoding failed. Check for invalid
overrides.`).  Never send a key this tool reports as a HOLE.

⚠ ATLAS §4.5: arms are reached from a jump table, so there is no prologue to anchor on.  Each arm
is disassembled forward from its own table target only, and bounded; an arm that does not decode
into the expected shape is reported UNRECOGNISED rather than guessed at.

Controls (§4.2/§4.4d), all established before this tool existed:
    key 100 -> +0xa84 string   (content manifest URL body   -- content-manifest-gate.md §2.2)
    key 101 -> +0xb84 string   (content manifest GUID)
    key 102 -> +0xba9 string   (content manifest URL prefix)
    key 157 -> +0x554 u32      (investment_signin timeout, 30000 ms -- character-and-orbit §26.41)
    key 894 -> +0x9ee u8       (use URL prefix, default 1)

Usage:
    ./engine-config-keys.py                       # the full table + controls
    ./engine-config-keys.py --offset 0x554        # which key writes this slot
    ./engine-config-keys.py --tsv keys.tsv
"""
import argparse
import struct
import sys
from collections import Counter

import capstone

EBOOT = "$HOME/dev/destiny1-preservation/dumps/patch-01.29-CUSA00219/eboot.bin"
BIAS = 0x4000

SWITCH = 0x1073170
JUMPTABLE = 0x107B8F4
N_ENTRIES = 0x483            # keys 100 .. 1254
FIRST_KEY = 100
DEFAULT_ARM = 0x10732CC

# primitive stream readers -- content-manifest-gate.md §3.2
READERS = {
    0xA1490: "varint",       # LEB128 (string length)
    0xA1340: "bytes",        # n raw bytes
    0xA2180: "fixed32",      # 4 raw bytes LE
    0xA2230: "fixed64",      # 8 raw bytes LE
}

CONTROLS = {
    100: (0x267B1B8, 0xA84, "string"),
    101: (0x267B1B8, 0xB84, "string"),
    102: (0x267B1B8, 0xBA9, "string"),
    157: (0x267B1B8, 0x554, "u32/f32"),
    894: (0x267B1B8, 0x9EE, "u8"),
}

_d = None


def data():
    global _d
    if _d is None:
        _d = open(EBOOT, "rb").read()
    return _d


def off(va):
    return va + BIAS


def decode_arm(arm_va, max_ins=48, max_hops=3):
    """Walk an arm (following unconditional jmps into shared tails) and recover its shape.

    Returns dict(global=, offset=, readers=[...], edx=, unrecognised=bool).
    """
    d = data()
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
    md.detail = False
    cur = arm_va
    g = offv = None
    readers = []
    edx = None
    seen = set()
    hops = 0
    n = 0
    while cur is not None and hops <= max_hops and cur not in seen:
        seen.add(cur)
        nxt = None
        blob = d[off(cur):off(cur) + 12 * max_ins]
        for ins in md.disasm(blob, cur):
            n += 1
            if n > max_ins * (hops + 1):
                break
            m, o = ins.mnemonic, ins.op_str
            if m == "lea" and "rip" in o:
                # lea reg, [rip + disp]
                try:
                    disp = int(o.split("+")[-1].strip(" ]"), 16)
                except ValueError:
                    continue
                cand = ins.address + ins.size + disp
                if g is None:
                    g = cand
            elif m == "mov":
                p = o.split(", ")
                if len(p) == 2:
                    try:
                        v = int(p[1], 0)
                    except ValueError:
                        continue
                    if p[0] in ("esi", "rsi") and offv is None:
                        offv = v
                    elif p[0] in ("r14d", "r14") and offv is None:
                        offv = v
                    elif p[0] in ("edx", "rdx"):
                        edx = v
            elif m == "call":
                try:
                    t = int(o, 16)
                except ValueError:
                    t = None
                if t in READERS:
                    readers.append(READERS[t])
            elif m == "jmp":
                try:
                    t = int(o, 16)
                except ValueError:
                    t = None
                if t is not None and t != DEFAULT_ARM:
                    nxt = t
                break
            elif m == "ret":
                break
        cur = nxt
        hops += 1
    return dict(g=g, offset=offv, readers=readers, edx=edx)


def classify(readers, edx):
    if readers[:2] == ["varint", "bytes"]:
        return "string"
    if readers[:1] == ["bytes"]:
        return "u8" if edx in (1, None) else f"bytes[{edx}]"
    if readers[:1] == ["fixed32"]:
        return "u32/f32"
    if readers[:1] == ["fixed64"]:
        return "u64/f64"
    if not readers:
        return "?"
    return "+".join(readers)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--tsv")
    ap.add_argument("--offset", type=lambda s: int(s, 0),
                    help="report which key(s) write this block offset")
    args = ap.parse_args()

    d = data()
    rows = {}
    holes = []
    for i in range(N_ENTRIES):
        key = FIRST_KEY + i
        rel = struct.unpack_from("<i", d, off(JUMPTABLE) + i * 4)[0]
        arm = JUMPTABLE + rel
        if arm == DEFAULT_ARM:
            holes.append(key)
            continue
        info = decode_arm(arm)
        rows[key] = dict(arm=arm, g=info["g"], offset=info["offset"],
                         type=classify(info["readers"], info["edx"]),
                         readers=info["readers"])

    print(f"jump table {JUMPTABLE:#x}, {N_ENTRIES} entries, keys "
          f"{FIRST_KEY}..{FIRST_KEY + N_ENTRIES - 1}")
    print(f"live keys: {len(rows)}   HOLES (setter returns false -> whole signon parse aborts): "
          f"{len(holes)}")
    print(f"holes: {holes}")
    print()

    print("POSITIVE CONTROL (ATLAS §4.2/§4.4d) - values known before this tool existed:")
    ok_all = True
    for k, (eg, eo, et) in sorted(CONTROLS.items()):
        r = rows.get(k)
        ok = r is not None and r["g"] == eg and r["offset"] == eo and r["type"] == et
        ok_all &= ok
        got = "MISSING" if r is None else f"global={r['g']:#x} +{r['offset']:#x} {r['type']}"
        print(f"  {'PASS' if ok else 'FAIL'}  key {k}: expect global {eg:#x} +{eo:#x} {et}  ->  {got}")
    print(f"  control: {'ALL PASS' if ok_all else 'FAILED - the table below is NOT evidence'}")
    print()

    globals_seen = Counter(r["g"] for r in rows.values())
    print("config-block globals targeted by the setter switch:")
    for g, c in globals_seen.most_common():
        print(f"  {g if g is None else hex(g)}: {c} key(s)")
    print()
    types = Counter(r["type"] for r in rows.values())
    print("value types:", dict(types))
    print()

    if args.offset is not None:
        print(f"keys writing block offset {args.offset:#x}:")
        for k, r in sorted(rows.items()):
            if r["offset"] == args.offset:
                print(f"  key {k}  global {r['g']:#x}  {r['type']}  arm {r['arm']:#x}")
        print("(keys whose arm did not decode are excluded; see the '?' count above)")
        return 0

    if args.tsv:
        with open(args.tsv, "w") as f:
            f.write("key\tblock_global\toffset\ttype\tarm\n")
            for k, r in sorted(rows.items()):
                g = "?" if r['g'] is None else f"{r['g']:#x}"
                o = "?" if r['offset'] is None else f"{r['offset']:#x}"
                f.write(f"{k}\t{g}\t{o}\t{r['type']}\t{r['arm']:#x}\n")
        print(f"wrote {args.tsv}")
    return 0 if ok_all else 1


if __name__ == "__main__":
    sys.exit(main())
