#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Read a hash-keyed DEFINITION TABLE out of Destiny's content packages.

⭐ THIS IS WHAT ANSWERS "what ordinal does the client use for <content thing>?"

The client never carries a content hash as an immediate - `historicaldocs/design/level-light-customization.md`
§3.3 scanned eight of them against a positive control returning 11,895 hits and got 0 each. The
runtime key is a small **ordinal**, and the ordinal is the element's INDEX in a definition array
that lives in an investment-globals package. This tool reads that array.

    d1-pkg entries <pkg>                      # find the entry: `off` is the array's block offset
    content-table.py <pkg> <block> <off>      # decompress that block, parse the array

ARRAY LAYOUT (`[C]`, holds for every array class seen in investment globals)

    +0x00  u64  total byte size of the entry (matches the entry table's `size` exactly)
    +0x08  u64  element count
    +0x10  u64  16
    +0x18  u32  0 ; u32 tag 0x80800184
    +0x20  u64  element count, again
    +0x28  u32  element class tag ; u32 0
    +0x30  <count> x <element>            <- the ordinal is the index here
    ...    variable-length tails (steps arrays, curves) the elements point into

⚠ THE ELEMENT SIZE IS NOT A CONSTANT OF THE FORMAT - it is a constant of the CONTENT VERSION.
The progression definition is 40 bytes in the 2014 launch content, 48 in 2015, 56 in the 01.29
content. So this tool DERIVES it (`--element` overrides) by requiring that **every** record in the
array has a distinct non-zero hash and a self-relative tail pointer whose target's count word equals
the record's own count word. On the three progression tables that predicate selects exactly one
size each. A derivation that admits more than one size is reported, not silently resolved.

CONTROLS: `--controls` re-derives four pinned results and exits 2 on any failure.
"""

import argparse
import hashlib
import importlib.util
import os
import struct
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
D1PKG = os.path.join(ROOT, "d1-server/target/release/d1-pkg")
BLOCK_PLAIN_SIZE = 0x40000

_HARNESS = None
_HARNESS_MOD = None


def _harness():
    """One Unicorn harness for the whole process - loading the eboot dominates per-block cost."""
    global _HARNESS, _HARNESS_MOD
    if _HARNESS is None:
        spec = importlib.util.spec_from_file_location(
            "d1_oodle_harness", os.path.join(HERE, "d1-oodle-harness.py"))
        m = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(m)
        _HARNESS_MOD, _HARNESS = m, m.Harness()
    return _HARNESS_MOD, _HARNESS


def block_plain(pkg, blk, tmpdir="/tmp"):
    """Stored bytes via the Rust container reader, plain bytes via the client's own decoder."""
    tmp = os.path.join(tmpdir, f".content-table.{os.getpid()}.z")
    r = subprocess.run([D1PKG, "extract-block", pkg, str(blk), "-o", tmp],
                       capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f"d1-pkg extract-block {pkg} {blk}: {r.stderr.strip()[:200]}")
    stored = open(tmp, "rb").read()
    os.remove(tmp)
    if "compressed" not in r.stdout:
        return stored
    m, h = _harness()
    h.heap_next = m.HEAP
    h.mu.mem_write(m.INPUT, b"\0" * 0x100000)
    h.mu.mem_write(m.INPUT, stored)
    h.mu.mem_write(m.OUTPUT, b"\0" * 0x100000)
    need = h.call(0x1267120, 0)
    scratch = h.alloc(need, 16)
    decoder = h.call(0x12671A0, 0, BLOCK_PLAIN_SIZE, scratch)
    result = h.alloc(0x20, 16)
    dec = comp = 0
    # Terminate on INPUT exhaustion. A block decompresses to *up to* 0x40000, not always exactly;
    # looping on `dec < plain_size` calls DecodeSome with no input left and reads unmapped memory.
    while dec < BLOCK_PLAIN_SIZE and comp < len(stored):
        h.mu.mem_write(result, b"\0" * 0x20)
        ok = h.call(0x12672E0, decoder, m.OUTPUT, dec, BLOCK_PLAIN_SIZE, BLOCK_PLAIN_SIZE - dec,
                    m.INPUT + comp, len(stored) - comp, 0, 0, result)
        v = struct.unpack("<8I", h.mu.mem_read(result, 0x20))
        if not ok or v[0] == 0:
            break
        dec += v[0]
        comp += v[1]
    if comp != len(stored):
        raise RuntimeError(f"{pkg} block {blk}: decode consumed {comp}/{len(stored)} stored bytes")
    return bytes(h.mu.mem_read(m.OUTPUT, dec))


def header(d, arr):
    size, count, sixteen, _pad_tag, count2, elem_tag_lo = struct.unpack_from("<6Q", d, arr)
    return {
        "size": size,
        "count": count,
        "elem_tag": elem_tag_lo & 0xFFFFFFFF,
        "base": arr + 0x30,
        "well_formed": sixteen == 16 and count == count2,
    }


def element_sizes(d, arr, lo=8, hi=128, step=4):
    """Every element size for which ALL `count` records validate. Should be exactly one."""
    h = header(d, arr)
    out = []
    for e in range(lo, hi + 1, step):
        if h["base"] + h["count"] * e > arr + h["size"]:
            continue
        seen, ok = set(), 0
        for i in range(h["count"]):
            o = h["base"] + i * e
            hash32 = struct.unpack_from("<I", d, o)[0]
            if hash32 == 0 or hash32 in seen:
                break
            seen.add(hash32)
            if e < 0x18:                       # too small to carry a {count, tail-ptr} pair
                ok += 1
                continue
            n, rel = struct.unpack_from("<Qq", d, o + 8)
            if n == 0 and rel == 0:
                ok += 1
                continue
            t = o + 0x10 + rel
            if arr <= t < arr + h["size"] and struct.unpack_from("<Q", d, t)[0] == n:
                ok += 1
            else:
                break
        if ok == h["count"]:
            out.append(e)
    return out


def records(d, arr, elem):
    h = header(d, arr)
    for i in range(h["count"]):
        o = h["base"] + i * elem
        hash32, f4 = struct.unpack_from("<II", d, o)
        n = struct.unpack_from("<Q", d, o + 8)[0] if elem >= 0x10 else 0
        yield i, hash32, f4 & 0xFF, n


# ---------------------------------------------------------------------------------------------
# Controls. Each is (label, callable -> (ok, detail)). Exit 2 if any fails.
# ---------------------------------------------------------------------------------------------

PKG = "ps4_investment_globals_012f_3"


def _c_pipeline():
    """The Oodle path itself, pinned by the block's plain SHA-1 and the activity table's index 1."""
    d = block_plain(PKG, 1063)
    sha = hashlib.sha1(d).hexdigest()
    h1 = struct.unpack_from("<I", d, 0x9920 + 0x10)[0]
    ok = (len(d) == 262144 and sha == "b7af28cbb6410f85ebf1b110e9a58af30594c1f3"
          and h1 == 0xDF283AFB)
    return ok, f"block 1063 = {len(d)} B sha1 {sha[:12]}, activity[1] = 0x{h1:08X}"


def _c_progression():
    d = block_plain(PKG, 1066)
    sizes = element_sizes(d, 145616)
    h = header(d, 145616)
    idx = {hh: i for i, hh, _, _ in records(d, 145616, 56)} if sizes == [56] else {}
    ok = (h["count"] == 247 and h["well_formed"] and sizes == [56]
          and idx.get(0x6650C0F9) == 117 and idx.get(0x7ACF6FD7) == 11
          and idx.get(0x447CA1B4) == 0)
    return ok, (f"count {h['count']}, element sizes that validate {sizes}, "
                f"character_level -> {idx.get(0x6650C0F9)}")


def _c_singleton():
    """The content states the ordinal outright: a 1-element array of {u32 hash, u32 ordinal}."""
    d = block_plain(PKG, 1066)
    hh, ordinal = struct.unpack_from("<II", d, 145552 + 0x30)
    ok = hh == 0x6650C0F9 and ordinal == 117
    return ok, f"entry 8131 singleton = {{0x{hh:08X}, {ordinal}}}"


def _c_stats():
    d = block_plain(PKG, 515)
    sizes = element_sizes(d, 172176)
    h = header(d, 172176)
    idx = {hh: i for i, hh, _, _ in records(d, 172176, 16)} if 16 in sizes else {}
    ok = (h["count"] == 36 and h["well_formed"] and sizes == [16]
          and idx.get(0x8E8B4E10) == 0 and idx.get(0xE854FA8E) == 1
          and idx.get(0x15F5C563) == 11)
    return ok, (f"count {h['count']}, element sizes {sizes}, "
                f"Light -> {idx.get(0x8E8B4E10)}, Defense -> {idx.get(0xE854FA8E)}, "
                f"Attack -> {idx.get(0x15F5C563)}")


def _c_version_drift():
    """⚠ NEGATIVE-SIDE control: the ordinal is a property of the CONTENT VERSION, not the format.

    If this ever passes with all three equal, the singleton is not what we think it is."""
    a = struct.unpack_from("<II", block_plain("ps4_investment_globals_012c_0", 5), 15280 + 0x30)
    b = struct.unpack_from("<II", block_plain("ps4_investment_globals_012f_1", 387), 70992 + 0x30)
    c = struct.unpack_from("<II", block_plain(PKG, 1066), 145552 + 0x30)
    ok = (a[0] == b[0] == c[0] == 0x6650C0F9) and (a[1], b[1], c[1]) == (109, 116, 117)
    return ok, f"character_level ordinal by content version: 012c_0={a[1]} 012f_1={b[1]} 012f_3={c[1]}"


CONTROLS = [
    ("Oodle pipeline + activity table (POSITIVE CONTROL for the whole path)", _c_pipeline),
    ("progression definition array: 247 x 56 B, character_level = 117", _c_progression),
    ("progression ordinal stated by content: {character_level, 117}", _c_singleton),
    ("stat definition array: 36 x 16 B, Light = 0", _c_stats),
    ("ordinal drifts with content version (109 / 116 / 117)", _c_version_drift),
]


def run_controls():
    bad = 0
    for label, fn in CONTROLS:
        try:
            ok, detail = fn()
        except Exception as e:                                  # noqa: BLE001
            ok, detail = False, f"{type(e).__name__}: {e}"
        print(f"{'PASS' if ok else 'FAIL'}  {label}\n        {detail}")
        bad += not ok
    print()
    if bad:
        print(f"{bad} CONTROL(S) FAILED - output from this tool may not be cited")
        return 2
    print("ALL CONTROLS PASS")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0],
                                 formatter_class=argparse.RawDescriptionHelpFormatter,
                                 epilog=__doc__)
    ap.add_argument("pkg", nargs="?", help="package base name, e.g. ps4_investment_globals_012f_3")
    ap.add_argument("block", nargs="?", type=lambda x: int(x, 0))
    ap.add_argument("offset", nargs="?", type=lambda x: int(x, 0),
                    help="the entry's block-local offset, from `d1-pkg entries`")
    ap.add_argument("--element", type=lambda x: int(x, 0), help="override the derived element size")
    ap.add_argument("--hash", type=lambda x: int(x, 0), action="append", default=[],
                    help="report the ordinal of this content hash. Repeatable")
    ap.add_argument("--all", action="store_true", help="print every record")
    ap.add_argument("--controls", action="store_true")
    args = ap.parse_args()

    if args.controls:
        return run_controls()
    if args.pkg is None or args.block is None or args.offset is None:
        ap.error("pkg, block and offset are required (or use --controls)")

    d = block_plain(args.pkg, args.block)
    h = header(d, args.offset)
    print(f"{args.pkg} block {args.block} @0x{args.offset:x}: {h['size']} bytes, "
          f"{h['count']} elements, element class 0x{h['elem_tag']:08x}, "
          f"header {'well-formed' if h['well_formed'] else '⚠ MALFORMED'}")
    if not h["well_formed"]:
        print("  ⚠ this offset does not look like an array header - check `d1-pkg entries`")
        return 1

    sizes = element_sizes(d, args.offset)
    print(f"  element sizes that validate all {h['count']} records: {sizes}")
    elem = args.element or (sizes[0] if len(sizes) == 1 else None)
    if elem is None:
        print("  ⚠ element size not uniquely determined - pass --element")
        return 1

    want = set(args.hash)
    for i, hash32, f4, n in records(d, args.offset, elem):
        if args.all or hash32 in want:
            print(f"  idx {i:5d}  0x{hash32:08X}  +0x04 {f4:3d}  +0x08 {n}")
    for w in want:
        found = [i for i, hh, _, _ in records(d, args.offset, elem) if hh == w]
        print(f"  0x{w:08X} -> ordinal {found[0] if found else 'ABSENT'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
