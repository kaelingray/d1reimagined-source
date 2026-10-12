#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Read the eboot's type-descriptor pool - the table §26.19/§26.29 called "the 0x58 field pool".

WHAT IT ACTUALLY IS, AND HOW §26.29's FRAMING NEEDS CORRECTING
--------------------------------------------------------------
§26.29 described "a flat FIELD pool, 0x1464340..0x14A2DF8, 2918 records, stride 0x58".  Measured
here, that window is the *tail* of a much larger, VARIABLE-LENGTH, self-delimiting record chain:

    leaf pool     0x13D8A40 .. 0x14A2E00     9188 objects, lengths 0x58 / 0x60 / 0x68
    and it continues backwards into the aggregate (struct-descriptor) records, which are the
    same record type carrying an extra inline block - i.e. the same table `eboot-descriptors.py`
    already parses.  The "field pool" and the "anchored descriptor table" are ONE table.

⚠ EIGHT-BYTE PHASE ERROR IN §26.19.  It read the record as starting at the constant word
0x80800050 and numbered every field from there.  The registry disagrees: it resolves tag
0x808033F4 to `0x801492550` = marker+8 [C, live, §26.19's own registry dump].  Measured here:

    object O = marker + 8;   O+0x50 == 0x80800050 and O+0x54 == 0
    ⇒ that constant word is the LAST ENTRY OF **THIS** OBJECT'S TYPE-TAG LIST, and the next word
      is the list's NUL terminator.  It is a trailer, not a header.

Every offset in §26.19's table is therefore 8 too high.  This module uses object-relative offsets.

OBJECT LAYOUT - every one of the 0x58 bytes accounted for   [C]
---------------------------------------------------------------
Counts below are over the 9188-object leaf pool; "(window: n)" is the §26.29 2917-object window.

    O+0x00  u32  size        offset of the object's last u32.  See LENGTH RULE.  Only ever
                             0x54 (6714), 0x5C (2472), 0x64 (2).
    O+0x04  u32  0           zero in 9188/9188
    O+0x08  u32  name hash   UNIQUE across all 9188; each block is SORTED by it
    O+0x0c  u32  aux         nonzero in 2913 (window: 185); nonzero values are 0x0084xxxx,
                             densely packed in 0x842001..0x845A30, not monotone.  [U]
    O+0x10  u32  child tag   0xFFFFFFFF = leaf (6113), else 0x8080xxxx = the element/underlying
                             type (3075; window: 1214)
    O+0x14  u32  width       SERIALIZED width IN BITS.  323 distinct values (window: 188) -
                             NOT the 10-value set §26.29 reported; 2384 of 9188 (window: 886 of
                             2917) fall outside {1,4,8,12,16,24,32,40,48,64}.  It is a *wire*
                             width, not a C size: e.g. a child whose struct is 48 bytes is
                             carried in 48 bits.  Large values (4096, 128000) are fixed arrays.
    O+0x18  u32  align       bytes; only ever 1,2,4,8,16
    O+0x1c  u32  flags A     0x40000 / 0x40004 / 0x60000 / 0 / 0x40008 / 0x30000 / 0x50000 …
    O+0x20  u32  flags B     bitfield; bits 12 and 13 set in 2917/2917 of the window
    O+0x24  u32  0           always
    O+0x28  u64  0           always
    O+0x30  u32  0           always
    O+0x34  u32  0           always
    O+0x38  u64  blobA       nonzero in 5971 (window: 1771), ALL 8-byte aligned, range
                             0x18..0x27B300.  NOT code addresses - only 2.7% are call/jmp
                             targets against a control that finds 0x56FFB0 and 0xBF25A0.  [U]
    O+0x40  u64  blobB       same shape; nonzero in 4267 (window: 1625), all 8-aligned,
                             range 0x1F28..0x22BA70.  [U]
    O+0x48  u64  csize       0 in 9188/9188 leaf objects; 24/16/40/48/… on aggregate records,
                             which reads like a C object size for non-trivial types.  [I]
    O+0x50  u32[] tag list   NUL-terminated; ALWAYS contains 0x80800050.  The 0x58 block has
                             exactly [0x80800050]; the 0x60 block has [0x8080001d, T,
                             0x80800050]; two objects have five entries.

NOT ONE BYTE OF THIS TABLE IS RELOCATED.  All 73825 RELA entries have r_offset >= 0x18C0038; the
table ends at 0x14A2E00, ~4 MB below the first relocation.  Positive control for that scan: the
§26.28 name-hash table at 0x19D0BD8 carries 1331 relocations at +0x08 and they are found.
⇒ every field above is a literal.  blobA/blobB cannot be load-time pointers.

LENGTH RULE   [C], 9187 consecutive steps landing exactly on 0x14A2E00
    length = size + 4   when u32(O + size) == 0   - the tag list's NUL; every leaf object
    length = size       otherwise                 - an aggregate ending in its field array

BLOCKS.  The table is hash-sorted in blocks, not globally.  The leaf pool is exactly two:
    0x13D8A40 .. 0x14128E0   2472 objects, every length 0x60   (tag list of three)
    0x1412940 .. 0x14A2DA8   6716 objects, length 0x58 x6714 / 0x68 x2
Below them the same chain continues into the aggregate records (mixed lengths, carrying members).
The §26.29 window 0x1464348+k*0x58 is the last 2917 of that second block.  Its "2918th" record is
NOT in the table: 0x14A2E00 is where a different record family begins (first tag 0x80800073).
That object's first word is 0xC0 - which is what §26.29 read as "one record with kind 192".

WHY MOST CHILD TAGS DANGLE.  Only 95 of the 1214 typed child tags in the §26.29 window resolve to
an aggregate self tag.  `--graph` shows why, and it is measured, not assumed: every occurrence of
a dangling tag anywhere in the image sits at object+0x10 - those tags are only ever *referenced*,
never *declared*.  Aggregates declare themselves (0x80801825 appears exactly once image-wide, at
its own anchor+0x10); leaf objects carry no self tag at all.  So a leaf type's tag exists only at
runtime, exactly as §26.27/§26.28 concluded, and no static edge can name it.

Usage:
    ./field-pool.py --stats
    ./field-pool.py --record 5946            # index within the leaf pool (0x13D8A40 = 0)
    ./field-pool.py --pool-record 2147       # legacy §26.19 numbering off 0x1464348
    ./field-pool.py --addr 0x1492550
    ./field-pool.py --hash 0xE2E25C2C
    ./field-pool.py --graph [--all]
"""
import argparse
import bisect
import collections
import struct
import sys

EBOOT = "$HOME/dev/destiny1-preservation/dumps/patch-01.29-CUSA00219/eboot.bin"
BIAS = 0x4000                       # vaddr = file_off - 0x4000

LEAF_POOL_START = 0x13D8A40         # first object with no aggregate part
POOL_END = 0x14A2E00                # first byte past the table; another record family starts here
LEGACY_POOL_BASE = 0x1464348        # §26.19's "record 0" (its 0x1464340 + the 8-byte phase error)

MARKER = 0x80800050                 # always present in an object's tag list
AGGREGATE = 0x8080010A              # tag that introduces the inline struct-descriptor block
LEAF = 0xFFFFFFFF

DYNLIBDATA = 0x1A0D100
RELA_OFF, RELA_SZ = DYNLIBDATA + 0xBDD0, 0x1B0918
NAMEHASH_LO, NAMEHASH_HI = 0x19D0BD8, 0x19D5F08     # the §26.28 type-name-hash table


class Eboot:
    def __init__(self, path):
        self.d = open(path, "rb").read()

    def u32(self, v):
        o = v + BIAS
        return struct.unpack_from("<I", self.d, o)[0] if 0 <= o <= len(self.d) - 4 else None

    def u64(self, v):
        o = v + BIAS
        return struct.unpack_from("<Q", self.d, o)[0] if 0 <= o <= len(self.d) - 8 else None

    def raw(self, v, n):
        return self.d[v + BIAS: v + BIAS + n]

    def relocations(self):
        out = {}
        for off in range(RELA_OFF, RELA_OFF + RELA_SZ, 0x18):
            ro, ri, ra = struct.unpack_from("<QQq", self.d, off)
            out[ro] = (ri & 0xFFFFFFFF, ra)
        return out


class Record:
    __slots__ = ("o", "size", "length", "hash", "aux", "child", "width", "align", "flags_a",
                 "flags_b", "blob_a", "blob_b", "csize", "tags", "anchor", "tag", "fields",
                 "struct_size", "index")

    def __repr__(self):
        return f"<rec {self.index} @{self.o:#x}>"


def parse(eb, o):
    size = eb.u32(o)
    if size is None or size < 0x50 or size > 0x20000 or (size & 3):
        return None
    if eb.u32(o + 4) != 0:
        return None
    tags, off = [], 0x50
    while off <= size and len(tags) < 64:
        v = eb.u32(o + off)
        if v is None or v == 0:
            break
        tags.append(v)
        off += 4
    if MARKER not in tags:
        return None
    r = Record()
    r.o, r.size = o, size
    r.length = size + 4 if eb.u32(o + size) == 0 else size
    r.hash = eb.u32(o + 0x08)
    r.aux = eb.u32(o + 0x0C)
    r.child = eb.u32(o + 0x10)
    r.width = eb.u32(o + 0x14)
    r.align = eb.u32(o + 0x18)
    r.flags_a = eb.u32(o + 0x1C)
    r.flags_b = eb.u32(o + 0x20)
    r.blob_a = eb.u64(o + 0x38)
    r.blob_b = eb.u64(o + 0x40)
    r.csize = eb.u64(o + 0x48)
    r.tags = tags
    r.anchor = r.tag = r.fields = r.struct_size = None
    if AGGREGATE in tags and tags.index(AGGREGATE) and tags[tags.index(AGGREGATE) - 1] == MARKER:
        r.anchor = o + 0x50 + 4 * (tags.index(AGGREGATE) - 1)
        r.tag = eb.u32(r.anchor + 0x10)
        r.fields = eb.u32(r.anchor + 0x08)
        r.struct_size = eb.u32(r.anchor + 0x14)
    r.index = -1
    return r


def walk(eb, start=LEAF_POOL_START, stop=POOL_END):
    out, o = [], start
    while o < stop:
        r = parse(eb, o)
        if r is None:
            break
        r.index = len(out)
        out.append(r)
        o += r.length
    return out, o


def anchors(eb):
    """tag -> anchor, by an independent scan for the MARKER+AGGREGATE pair.  Same anchor
    definition `eboot-descriptors.py` uses, so the two tools can be cross-checked."""
    pat = struct.pack("<II", MARKER, AGGREGATE)
    out, s = {}, 0
    while True:
        j = eb.d.find(pat, s)
        if j < 0:
            break
        a = j - BIAS
        s = j + 8
        tag, cnt = eb.u32(a + 0x10), eb.u32(a + 8)
        if tag is not None and (tag & 0xFFFF0000) == 0x80800000 and cnt is not None and cnt <= 64:
            out.setdefault(tag, a)
    return out


# ---------------------------------------------------------------------------- controls

def control(label, ok, detail=""):
    print(f"  [{'PASS' if ok else 'FAIL'}] {label}{('  ' + detail) if detail else ''}")
    return ok


def run_controls(eb, recs, tagmap):
    """Printed by every mode.  A scan whose controls fail is a broken scanner, not a discovery -
    the defect §26.10 records having cost two sessions."""
    print("controls")
    by_addr = {r.o: r for r in recs}
    by_hash = {r.hash: r for r in recs}
    ok = True
    ok &= control("+ 0x1492550 (what the live registry resolved 0x808033F4 to) is in the pool",
                  0x1492550 in by_addr,
                  f"index {by_addr[0x1492550].index}" if 0x1492550 in by_addr else "")
    ok &= control("+ name hash 0xE2E25C2C present", 0xE2E25C2C in by_hash)
    ok &= control("- name hash 0xDEADBEEF absent", 0xDEADBEEF not in by_hash)
    ok &= control("+ aggregate self tag 0x80801825 (503 response) found by the anchor scan",
                  0x80801825 in tagmap)
    ok &= control("- bogus tag 0xDEADBEEF not found by the anchor scan", 0xDEADBEEF not in tagmap)
    ok &= control("+ the walk terminates exactly at 0x14A2E00", walk(eb)[1] == POOL_END,
                  f"{len(recs)} objects")
    rel = eb.relocations()
    n_ctrl = sum(1 for v in range(NAMEHASH_LO + 8, NAMEHASH_HI, 16) if v in rel)
    ok &= control("+ the RELA scan sees the name-hash table", n_ctrl > 1300, f"{n_ctrl} relocs")
    n_pool = sum(1 for r in recs for k in range(0, r.length, 4) if (r.o + k) in rel)
    ok &= control("- no relocation anywhere in the pool", n_pool == 0, f"{n_pool} found")
    print()
    return ok


# ---------------------------------------------------------------------------- printers

def show(eb, r, raw=False):
    kind = f"AGGREGATE (self tag {r.tag:#010x})" if r.tag is not None else "leaf"
    print(f"record[{r.index}]  object {r.o:#x}  marker {r.o - 8:#x}  "
          f"size {r.size:#x}  length {r.length:#x}  {kind}")
    if LEGACY_POOL_BASE <= r.o and (r.o - LEGACY_POOL_BASE) % 0x58 == 0:
        print(f"           legacy §26.19 index {(r.o - LEGACY_POOL_BASE) // 0x58}")
    print(f"  +0x08 name hash   {r.hash:#010x}")
    print(f"  +0x0c aux         {r.aux:#010x}")
    print(f"  +0x10 child tag   {'LEAF' if r.child == LEAF else f'{r.child:#010x}'}")
    print(f"  +0x14 width       {r.width} bits ({(r.width + 7) // 8} bytes)")
    print(f"  +0x18 align       {r.align}")
    print(f"  +0x1c flags A     {r.flags_a:#010x}")
    print(f"  +0x20 flags B     {r.flags_b:#010x}   bits "
          f"{[b for b in range(32) if r.flags_b >> b & 1]}")
    print(f"  +0x38 blobA       {r.blob_a:#x}")
    print(f"  +0x40 blobB       {r.blob_b:#x}")
    print(f"  +0x48 csize       {r.csize:#x}")
    print(f"  +0x50 tag list    {[hex(t) for t in r.tags]}  (NUL at +{r.size:#x})")
    if r.tag is not None:
        print(f"        anchor {r.anchor:#x}  {r.fields} field(s)  struct {r.struct_size} bytes")
    if raw:
        for k in range(0, r.length, 16):
            print(f"    +{k:#05x}  {eb.raw(r.o + k, 16).hex(' ')}")


def stats(eb, recs, end, tagmap):
    print(f"leaf pool: {len(recs)} objects, {LEAF_POOL_START:#x} .. {end:#x}"
          f"  ({'terminates cleanly' if end == POOL_END else '*** STOPPED EARLY ***'})")
    hs = [r.hash for r in recs]
    print(f"name hashes: {len(set(hs))} distinct of {len(hs)}  "
          f"({'all unique' if len(set(hs)) == len(hs) else 'COLLISIONS'})")
    bounds = [0] + [i for i in range(1, len(hs)) if hs[i] < hs[i - 1]] + [len(hs)]
    print(f"hash-sorted blocks: {len(bounds) - 1}")
    for k in range(len(bounds) - 1):
        a, b = bounds[k], bounds[k + 1]
        blk = recs[a:b]
        print(f"   block {k}: idx {a}..{b-1}  {blk[0].o:#x}..{blk[-1].o:#x}  n={len(blk)}  "
              f"lengths={[(hex(s), n) for s, n in collections.Counter(r.length for r in blk).most_common(3)]}")
    print()
    for name, get in (("size (+0x00)", lambda r: r.size),
                      ("aux (+0x0c) != 0", lambda r: bool(r.aux)),
                      ("align (+0x18)", lambda r: r.align),
                      ("flags A (+0x1c)", lambda r: r.flags_a),
                      ("tag list", lambda r: tuple(hex(t) for t in r.tags))):
        print(f"  {name}: {collections.Counter(get(r) for r in recs).most_common(6)}")
    w = collections.Counter(r.width for r in recs)
    canon = {1, 4, 8, 12, 16, 24, 32, 40, 48, 64}
    print(f"  width (+0x14): {len(w)} distinct, top {w.most_common(10)}")
    print(f"                 {sum(n for v, n in w.items() if v not in canon)} objects outside "
          f"{sorted(canon)}  <-- §26.29 reported that set as exhaustive")
    print(f"  child tag: {sum(1 for r in recs if r.child == LEAF)} leaf, "
          f"{sum(1 for r in recs if r.child != LEAF)} typed")
    print(f"  always-zero columns hold: " + ", ".join(
        f"+{o:#04x}" for o in (0x04, 0x24, 0x28, 0x2C, 0x30, 0x34)
        if all(eb.u32(r.o + o) == 0 for r in recs)))
    print(f"  blobA/blobB/csize nonzero in the leaf pool: "
          f"{sum(1 for r in recs if r.blob_a)}/{sum(1 for r in recs if r.blob_b)}/"
          f"{sum(1 for r in recs if r.csize)}")
    print()
    sub = [r for r in recs if r.o >= LEGACY_POOL_BASE]
    print(f"§26.29 window (0x1464348 + k*0x58): {len(sub)} objects, "
          f"{sub[0].o:#x}..{sub[-1].o:#x}  (it claimed 2918; the 2918th is at {POOL_END:#x}, "
          f"outside the table)")
    print(f"aggregate self tags found by the independent anchor scan: {len(tagmap)}")
    print(f"first object past the table, {POOL_END:#x}: "
          f"size {eb.u32(POOL_END):#x}, +0x0c {eb.u32(POOL_END + 0x0C):#010x} "
          f"(tag 0x80800073 - a different record family; this is §26.29's 'kind 192')")


def occurrences(eb, recs, tag):
    addrs = [r.o for r in recs]
    pat = struct.pack("<I", tag)
    offs, s = collections.Counter(), 0
    while True:
        j = eb.d.find(pat, s)
        if j < 0:
            break
        v, s = j - BIAS, j + 1
        if not (LEAF_POOL_START <= v < POOL_END):
            continue
        i = bisect.bisect_right(addrs, v) - 1
        if i >= 0:
            offs[v - addrs[i]] += 1
    return offs


def graph(eb, recs, tagmap, everything=False):
    pool = recs if everything else [r for r in recs if r.o >= LEGACY_POOL_BASE]
    label = "whole leaf pool" if everything else "§26.29 window"
    typed = [r for r in pool if r.child != LEAF]
    hit = [r for r in typed if r.child in tagmap]
    miss = [r for r in typed if r.child not in tagmap]
    print(f"child-tag linkage over the {label}: {len(pool)} objects")
    print(f"  {len(typed)} typed, {len(hit)} resolve into the anchored descriptor index, "
          f"{len(miss)} dangle")
    print("  top dangling targets: "
          f"{[(hex(t), n) for t, n in collections.Counter(r.child for r in miss).most_common(8)]}")
    print()
    print("  are the danglers declared anywhere?  offsets-in-object of every image-wide hit:")
    for t, _ in collections.Counter(r.child for r in miss).most_common(3):
        print(f"    {t:#010x}: {dict(occurrences(eb, recs, t))}   (+0x10 = the child-tag slot)")
    ctl = 0x80801825
    print(f"    control {ctl:#010x} (a declared aggregate): "
          f"{len([1 for _ in [0] if ctl in tagmap])} anchor, image-wide hits in the leaf pool "
          f"{dict(occurrences(eb, recs, ctl))}")
    print()
    for r in hit[:25]:
        a = tagmap[r.child]
        print(f"  [{r.index}] hash {r.hash:#010x} w={r.width:<5} -> {r.child:#010x}  "
              f"anchor {a:#x} struct {eb.u32(a + 0x14)}B {eb.u32(a + 8)} fields")
    if len(hit) > 25:
        print(f"  … {len(hit) - 25} more")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--eboot", default=EBOOT)
    ap.add_argument("--record", type=int, help="index within the leaf pool")
    ap.add_argument("--pool-record", type=int, help="legacy §26.19 index off 0x1464348")
    ap.add_argument("--addr", type=lambda s: int(s, 0))
    ap.add_argument("--hash", type=lambda s: int(s, 0))
    ap.add_argument("--stats", action="store_true")
    ap.add_argument("--graph", action="store_true")
    ap.add_argument("--all", action="store_true", help="--graph over the whole leaf pool")
    ap.add_argument("--raw", action="store_true")
    ap.add_argument("--no-controls", action="store_true")
    a = ap.parse_args()

    eb = Eboot(a.eboot)
    recs, end = walk(eb)
    tagmap = anchors(eb)
    if not a.no_controls and not run_controls(eb, recs, tagmap):
        print("CONTROLS FAILED - treat every number below as suspect.", file=sys.stderr)

    picked = []
    if a.record is not None:
        picked.append(recs[a.record] if 0 <= a.record < len(recs) else None)
    if a.pool_record is not None:
        want = LEGACY_POOL_BASE + a.pool_record * 0x58
        picked.append(next((r for r in recs if r.o == want), None))
    if a.addr is not None:
        picked.append(next((r for r in recs if r.o == a.addr), None))
    if a.hash is not None:
        picked.append(next((r for r in recs if r.hash == a.hash), None))
    for r in picked:
        show(eb, r, a.raw) if r else print("no such record")
        print()
    if a.graph:
        graph(eb, recs, tagmap, a.all)
    if a.stats or not (picked or a.graph):
        stats(eb, recs, end, tagmap)


if __name__ == "__main__":
    main()
