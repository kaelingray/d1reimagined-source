#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""The RSAT **type registry**: how runtime schema tags (`0x8080xxxx`) are assigned, read
statically out of `eboot.bin`.  No emulator, no live client.

WHY THIS EXISTS
---------------
`historicaldocs/design/character-and-orbit.md` §26.17 measured the span element tag `0x808033F4` in a live run and
§26.27 established that the value appears **nowhere** in the image.  §26.28 found a 16-byte table at
`0x19D0BD8` keyed by a name hash and concluded "tags are assigned at load, only the name hash is
static", leaving `[U]` *who assigns them* and *whether they are stable*.

Both are now answered, and the answer is better than hoped: **the tag is a pure function of two
static inputs.**  Nothing is counted, nothing is hashed, nothing is allocated in arrival order:

    tag = 0x80000000 | (container_slot << 13) | (element_index & 0x1FFF)

`container_slot` is one of three **hardcoded immediates** and `element_index` is a `u16` in a static
array.  Tags are therefore identical in every run of this build and can be computed offline.

⚠ TWO CORRECTIONS TO §26.28, both structural, both with the evidence below:

1. **The record is `{u64 ptr; u32 name_hash; u32 pad}`, not `{u32 name_hash; pad; u64 ptr}`** - the
   table starts at `0x19D0BD0`, eight bytes *earlier* than §26.28's `0x19D0BD8`, and the hash sits at
   `+0x08`.  §26.28 paired every hash with the *next* record's pointer, so every (hash → slot) pair
   it printed is off by one.  Proof, three ways, all in `--verify`:
     * a relocation exists at `0x19D0BD0` and none at `0x19D0BC0`;
     * the array ends exactly at `0x19D5F10`, which `proc_param+0x38` names as `sceLibcParam`;
     * the registrar at `0xC2B2B0` passes `esi = 0x534 = 1332`, and `(0x19D5F10-0x19D0BD0)/16 = 1332`.
   Under the corrected layout the tag holder that `0x19CC800` points at pairs with name hash
   **`0xE2E25C2C`** - which is the *same* hash the field-pool record carries (§26.19/§26.29).  So
   §26.28's open question "why does one type appear under two hashes" dissolves: **it never did.**
   `0xE2B769E8` belongs to the neighbouring entry.
2. §26.28's `.bss` addresses do not reproduce.  `0x19CC800`'s RELA addend is **`0x28FD0B0`**
   (`--verify` prints it), not `0x2900BF0`.

THE MECHANISM, end to end (all `[C]`, `static-client-inference`)
---------------------------------------------------------------
`DT_INIT` is `0x20` and walks a 4735-entry `.ctors` array downward from `0x1A06C58` (terminator `-1`
at `0x19FD860`).  Somewhere in that startup path is a straight-line routine containing `0x1100`.
`[U]` how `0x1100` itself is entered is *not* established here - it carries no `call`/`jmp`/`lea`/RELA
reference and no `.ctors` entry lands between `0xD00` and `0x1200`; it is fall-through inside a
larger routine whose head this script does not name.  Nothing below depends on that: the binder call
is direct and unconditional.

    0x1120..0x1148   nine calls, each of which
                       - constructs that module's .bss slot array (tag field := 0xFFFFFFFF)
                       - tail-calls 0xEE0D0(table_ptr, entry_count) to enlist its name-hash table
    0x114D           lea rdi, [0x129F950]          <- the global element list
    0x1154           call 0xEE100                  <- THE BINDER

`0xEE0D0` appends `{ptr, u32 count, u32 cursor := 0}` to a 16-byte-stride array at `0x25AB410`,
count at `0x25AB520`.

`0xEE100(X)` walks `X+0x20 -> {i64 count; record[16]}` - 14816 records, each
`{i64 rel_ptr -> descriptor object; u16 container_index; u16 element_index; u32 pad}` - and for each:

    r12d = 0x6E9B0(container_base + container_index*0x80, element_index)      <- ALLOCATES THE TAG
    if r12d != -1: 0x6F5A0(r12d, descriptor_object, ...)                      <- registry bind
    for each enlisted table t:                                               <- sorted merge join
        if table[t][cursor[t]].name_hash == descriptor_object->name_hash:
            slot = table[t][cursor[t]].ptr
            if slot->object == NULL:
                slot->object = descriptor_object
                slot->tag    = r12d
                slot->companion->slot = slot
                slot->companion->tag  = r12d
            cursor[t] += 1

Both sides are sorted ascending by name hash, which is *why* the tables are sorted.  Replaying that
merge offline binds **100 % of every table's entries** (`--verify`), which is the check that the
reading is right.

`0x6E9B0` is the whole of tag assignment:

    eax = [container + 0x34]
    ebx = eax << 13
    if eax & 0x40000000:  ebx = (ebx & 0x07FFE000) | (index & 0x1FFF) | 0x80000000
    else:                 ebx = (ebx & 0x007FE000) | (index & 0x1FFF)
    return ebx

and `[container+0x34]` is written once, by `0x70C60` at `0x70E08`:  `ecx = slot_id; ecx |= 0x40000000;
[rbx+0x34] = ecx`.  `0xE9770` creates exactly three containers with **literal** slot ids:

    0x70C60(0x256F220 + 0x000, "schema handle table 0", 0x400, 0x2000, 0)
    0x70C60(0x256F220 + 0x080, "schema handle table 1", 0x401, 0x2000, 0)
    0x70C60(0x256F220 + 0x100, "schema handle table 2", 0x402, 0x2000, 0)

⇒ `[C]` **Tags are deterministic.**  Not "probably stable" - closed form, no state.

THE `.bss` SLOT RECORD is 0x20 bytes (1332 of them, contiguous, for `spasaha_investment`):

    +0x00  u32 tag        0xFFFFFFFF until bound   <- `0x570A82` reads exactly this word
    +0x04  u32 pad
    +0x08  ptr  descriptor object (also the "already bound" guard)
    +0x10  ptr  next, intrusive list, head 0x256EF28
    +0x18  ptr  companion `{ptr slot; u32 tag; u32 pad}` in .data.rel.ro, pre-set by the module ctor
                (placeholder value `0xC00000xx` before binding - note the `0xC0` prefix, versus
                `0x80` for a live tag)

DESCRIPTOR OBJECTS.  `object + 0x50` is the `0x80800050` anchor word.  A composite carries
`0x8080010A` at `+0x54` and is what `capture-rig/eboot-descriptors.py` indexes; a leaf carries `0`
there, which is why that tool cannot see leaves such as `0x808033F4`.  Common header:

    +0x00 u32 kind   +0x08 u32 name_hash   +0x10 u32 child tag (0xFFFFFFFF = leaf)   +0x14 u32 width-in-bits

METHOD RULE OBSERVED: every scan in this file that can report "absent" also searches for something
known to be present in the same pass and prints both.  `--verify` fails loudly if a control is empty.

Usage:
    ./type-registry.py --verify                 # structural checks + positive controls (run this first)
    ./type-registry.py --stats                  # the nine schema modules and their containers
    ./type-registry.py --tag 0x808033F4         # tag -> module, name hash, descriptor object, slot
    ./type-registry.py --hash 0xE2E25C2C        # name hash -> tag
    ./type-registry.py --object 0x1492550       # descriptor object -> tag
    ./type-registry.py --module spasaha_investment --list
    ./type-registry.py --dump map.tsv           # every element: tag, hash, object, module, slot
"""
import argparse
import struct
import sys

EBOOT_DEFAULT = "dumps/patch-01.29-CUSA00219/eboot.bin"
BIAS = 0x4000                      # guest vaddr = file offset - 0x4000 (both loadable segments)

TEXT_LO, TEXT_HI = 0x0, 0x18BF0DC
DATA_LO, DATA_HI = 0x18C0000, 0x18C0000 + 0x149088
BSS_LO, BSS_HI = DATA_HI, 0x18C0000 + 0x1690DC0

DYNLIBDATA = 0x1A0D100             # file offset of PT_SCE_DYNLIBDATA
RELA_OFF, RELA_SZ, RELA_ENT = DYNLIBDATA + 0xBDD0, 0x1B0918, 0x18

ELEMENT_LIST = 0x129F950           # the argument to 0xEE100, from `lea rdi` at 0x114D
ELEMENT_LIST_FIELD = 0x20          # X+0x20 -> {i64 count; record[]}
CONTAINER_BASE_PTR = 0x1A06D80     # -> 0x256F220, three 0x80-byte "schema handle table" containers
CONTAINER_STRIDE = 0x80

BINDER = 0xEE100
TAG_ALLOC = 0x6E9B0
TABLE_REGISTER = 0xEE0D0

# container_index -> slot id, from the three literal immediates at 0xE9791 / 0xE97B1 / 0xE97D1
CONTAINER_SLOT = {0: 0x400, 1: 0x401, 2: 0x402}

# module name -> (name-hash table vaddr, entry count, .bss slot array vaddr)
# read off the nine register/unregister call pairs at 0x1120..0x1181; --verify re-checks every one
MODULES = [
    ("well_known",         0x197F170,  300, 0x28C1D00),
    ("foundation",         0x1959320,  308, 0x289DEF0),
    ("engine",             0x190E710, 2992, 0x267C720),
    ("ai",                 0x1961670, 1189, 0x28A0960),
    ("investment",         0x196D9A0, 2007, 0x28A9E40),
    ("renderer",           0x1979F20, 1014, 0x28B9E40),
    ("spasaha",            0x19B3530, 6338, 0x28C8DD0),
    ("spasaha_investment", 0x19D0BD0, 1332, 0x28FA850),
    ("audio",              0x1980760,   40, 0x28C4340),
]

TABLE_STRIDE = 0x10                # {u64 ptr -> .bss slot; u32 name_hash; u32 pad}
SLOT_STRIDE = 0x20

# --- controls -------------------------------------------------------------------------------
# every one of these was established independently of this script; --verify re-derives them.
CONTROLS = [
    # (tag, descriptor object, provenance)
    (0x80801796, 0x139D1D8, "eboot-descriptors.py: anchor 0x139D228 = object+0x50"),
    (0x80801825, 0x1324020, "eboot-descriptors.py: anchor 0x1324070 = object+0x50 (503 response)"),
    (0x808033F4, 0x1492550, "live registry walk, character-and-orbit §26.19 (g_eboot+0x1492550)"),
]
CONTROL_GLOBAL = (0x19CC800, 0x28FD0B0)     # the span element tag holder; 0x570A82 reads [[0x19CC800]]


class Eboot:
    def __init__(self, path):
        with open(path, "rb") as f:
            self.d = f.read()
        if self.d[:4] != b"\x7fELF":
            sys.exit("%s: not an ELF" % path)
        self._rela = None

    # ---- address helpers
    @staticmethod
    def infile(v):
        return TEXT_LO <= v < TEXT_HI or DATA_LO <= v < DATA_HI

    def off(self, v):
        if not self.infile(v):
            raise KeyError("vaddr %#x is not file-backed (bss/heap)" % v)
        return v + BIAS

    def u16(self, v):
        return struct.unpack_from("<H", self.d, self.off(v))[0]

    def u32(self, v):
        return struct.unpack_from("<I", self.d, self.off(v))[0]

    def u64(self, v):
        return struct.unpack_from("<Q", self.d, self.off(v))[0]

    def i64(self, v):
        return struct.unpack_from("<q", self.d, self.off(v))[0]

    def raw(self, v, n):
        return self.d[self.off(v):self.off(v) + n]

    def cstr(self, v, n=128):
        return self.raw(v, n).split(b"\0")[0].decode("latin1")

    def relptr(self, v):
        """self-relative pointer: value = &field + *(i64*)&field, 0 meaning NULL."""
        d = self.i64(v)
        return 0 if d == 0 else v + d

    # ---- relocations (R_X86_64_RELATIVE carries its value in the addend; the file word is 0)
    def rela(self):
        if self._rela is None:
            by_off, by_add = {}, {}
            for o in range(RELA_OFF, RELA_OFF + RELA_SZ, RELA_ENT):
                r_off, r_info, r_add = struct.unpack_from("<QQq", self.d, o)
                by_off[r_off] = (r_info, r_add)
                by_add.setdefault(r_add, []).append(r_off)
            self._rela = (by_off, by_add)
        return self._rela

    def reloc_at(self, v):
        by_off, _ = self.rela()
        e = by_off.get(v)
        return e[1] if e else None


def tag_of(slot_id, index):
    """0x6E9B0's arithmetic verbatim, for a container whose +0x34 is 0x40000000|slot_id."""
    eax = 0x40000000 | slot_id
    ebx = (eax << 13) & 0xFFFFFFFF
    if eax & 0x40000000:
        return (ebx & 0x07FFE000) | (index & 0x1FFF) | 0x80000000
    return (ebx & 0x007FE000) | (index & 0x1FFF)


def split_tag(tag):
    """inverse of tag_of, transcribed from `0x6F5A0` (the registry setter):

        ecx = tag >> 13                  (shr, logical)
        eax = ((sar(tag,31) & 0x3C00) | 0x3FF) & ecx        -> slot
        edi = tag & 0x1FFF                                  -> index

    ⚠ §26.11 renders this as `((ecx>>13 u>>0x12) | 0x3ff) & (ecx>>13)`, which is not the same
    expression and yields slot 0x001 for `0x808033F4`.  The sign-mask form below yields 0x401,
    which is what `dump-registry.py` measured live.
    """
    sign = 0xFFFFFFFF if tag & 0x80000000 else 0
    mask = (sign & 0x3C00) | 0x3FF
    return (tag >> 13) & mask, tag & 0x1FFF


class Registry:
    def __init__(self, eb):
        self.eb = eb
        self.elements = self._elements()
        self.tables = {name: self._table(tv, n) for name, tv, n, _ in MODULES}
        self.binding = self._merge()

    # every element in the global list, in list order (== ascending name hash)
    def _elements(self):
        eb = self.eb
        lst = eb.relptr(ELEMENT_LIST + ELEMENT_LIST_FIELD)
        n = eb.i64(lst)
        base = lst + 8
        out = []
        for i in range(n):
            r = base + i * 16
            obj = eb.relptr(r)
            cont, idx = struct.unpack_from("<HH", eb.d, eb.off(r) + 8)
            h = eb.u32(obj + 8) if obj else None
            out.append(dict(i=i, rec=r, obj=obj, cont=cont, idx=idx, hash=h,
                            tag=tag_of(CONTAINER_SLOT[cont], idx)))
        return out

    def _table(self, tv, n):
        eb = self.eb
        out = []
        for i in range(n):
            a = tv + i * TABLE_STRIDE
            out.append(dict(entry=a, hash=eb.u32(a + 8), slot=eb.reloc_at(a)))
        return out

    # replay 0xEE100's inner loop exactly: independent per-table cursor, advanced only on a match
    def _merge(self):
        cursor = {name: 0 for name, *_ in MODULES}
        bound = {}
        for e in self.elements:
            for name, *_ in MODULES:
                t = self.tables[name]
                c = cursor[name]
                if c >= len(t) or t[c]["hash"] != e["hash"]:
                    continue
                bound[(name, c)] = e
                cursor[name] += 1
        self.cursor = cursor
        return bound

    # ---- lookups
    def by_tag(self, tag):
        return [e for e in self.elements if e["tag"] == tag]

    def by_hash(self, h):
        return [e for e in self.elements if e["hash"] == h]

    def by_object(self, o):
        return [e for e in self.elements if e["obj"] == o]

    def modules_of(self, element):
        out = []
        for (name, c), e in self.binding.items():
            if e is element:
                out.append((name, c, self.tables[name][c]))
        return out


def describe(eb, e):
    obj = e["obj"]
    kind = eb.u32(obj)
    child = eb.u32(obj + 0x10)
    width = eb.u32(obj + 0x14)
    anchor2 = eb.u32(obj + 0x54)
    shape = "composite" if anchor2 == 0x8080010A else ("leaf" if anchor2 == 0 else "?%#x" % anchor2)
    return ("tag %#010x  hash %08X  object %#010x  container %d (slot %#x)  index %#06x\n"
            "    kind %#x  %s  child %s  width %d bits  anchor %#x" %
            (e["tag"], e["hash"], obj, e["cont"], CONTAINER_SLOT[e["cont"]], e["idx"],
             kind, shape, "LEAF" if child == 0xFFFFFFFF else "%#010x" % child, width, obj + 0x50))


def cmd_verify(eb, reg):
    ok = True

    def check(label, cond, detail=""):
        nonlocal ok
        print("  [%s] %-58s %s" % ("PASS" if cond else "FAIL", label, detail))
        ok = ok and cond

    print("== table geometry (the §26.28 correction) ==")
    start_rel = eb.reloc_at(0x19D0BD0)
    before_rel = eb.reloc_at(0x19D0BC0)
    check("relocation at table start 0x19D0BD0", start_rel is not None, "-> %s" % (hex(start_rel) if start_rel else None))
    check("no relocation one record earlier (0x19D0BC0)", before_rel is None,
          "CONTROL: the line above must be non-empty")
    end = 0x19D0BD0 + 1332 * TABLE_STRIDE
    libc = eb.reloc_at(0x18C0038)
    check("table ends at sceLibcParam (proc_param+0x38)", end == libc == 0x19D5F10,
          "end=%#x libcparam=%s size=%#x" % (end, hex(libc) if libc else None, eb.u64(0x19D5F10)))
    check("registrar immediate esi == 0x534 == 1332", eb.raw(0xC2B2C0, 5) == b"\xbe\x34\x05\x00\x00",
          "0xC2B2C0: %s" % eb.raw(0xC2B2C0, 5).hex())
    check("slot-array ctor loop bound == 1332*0x20", eb.raw(0xC269BB, 5) == b"\x3d\x80\xa6\x00\x00",
          "0xC269BB cmp eax,0xA680")

    print("== the three containers (hardcoded slot ids) ==")
    cbase = eb.reloc_at(CONTAINER_BASE_PTR)
    check("container base pointer 0x1A06D80 relocates", cbase == 0x256F220, "-> %s" % hex(cbase or 0))
    for i, (site, want) in enumerate(((0xE9791, 0x400), (0xE97B1, 0x401), (0xE97D1, 0x402))):
        imm = struct.unpack_from("<I", eb.d, eb.off(site) + 1)[0]
        check("0xE9770 creates container %d with slot %#x" % (i, want), imm == want,
              "immediate at %#x = %#x  name %r" % (site, imm, eb.cstr(0x1608A65 + i * 22)))

    print("== the tag holder the crash site reads ==")
    g = eb.reloc_at(CONTROL_GLOBAL[0])
    check("0x19CC800 relocates to the .bss slot", g == CONTROL_GLOBAL[1],
          "-> %s (§26.28 printed 0x2900BF0; that does not reproduce)" % hex(g or 0))
    idx = (g - 0x28FA850) // SLOT_STRIDE if g else -1
    check("that slot is inside spasaha_investment's array", 0 <= idx < 1332, "slot index %d" % idx)
    ent = [t for t in reg.tables["spasaha_investment"] if t["slot"] == g]
    check("exactly one table entry points at it", len(ent) == 1,
          "entry %#x  name hash %08X" % (ent[0]["entry"], ent[0]["hash"]) if ent else "")
    if ent:
        check("its name hash is 0xE2E25C2C (field-pool record's own hash)", ent[0]["hash"] == 0xE2E25C2C,
              "§26.28 paired it with 0xE2B769E8 = the previous entry")

    print("== every module's table is well formed and binds 100% ==")
    for name, tv, n, slots in MODULES:
        t = reg.tables[name]
        hs = [x["hash"] for x in t]
        rel = sum(1 for x in t if x["slot"] is not None)
        srt = hs == sorted(hs)
        uniq = len(set(hs)) == n
        # the slot array is contiguous; the TABLE is sorted by hash, so entry i's slot is not
        # slots+i*0x20 -- compare as sets, which is what "one slot per entry" actually means
        want = {slots + i * SLOT_STRIDE for i in range(n)}
        contig = {x["slot"] for x in t} == want
        check("%-19s %4d entries" % (name, n),
              rel == n and srt and uniq and contig and reg.cursor[name] == n,
              "relocs %d/%d sorted=%s unique=%s slots=%s bound=%d" %
              (rel, n, srt, uniq, "contiguous" if contig else "MISMATCH", reg.cursor[name]))

    print("== positive controls: tags derived here vs tags established elsewhere ==")
    for tag, obj, prov in CONTROLS:
        hits = reg.by_tag(tag)
        check("tag %#010x -> object %#010x" % (tag, obj),
              len(hits) == 1 and hits[0]["obj"] == obj,
              (("got %#010x  " % hits[0]["obj"]) if hits else "NO ELEMENT  ") + prov)
    bogus = reg.by_tag(0x8080FFFF)
    check("negative control: bogus tag 0x8080FFFF resolves to nothing", not bogus,
          "CONTROL: the three lines above must be PASS, or this proves nothing")

    print("== the tag formula is invertible ==")
    bad = [e for e in reg.elements
           if split_tag(e["tag"]) != (CONTAINER_SLOT[e["cont"]], e["idx"])]
    check("split_tag() round-trips all %d elements" % len(reg.elements), not bad,
          "%d mismatches" % len(bad))

    # The decisive determinism argument, and it comes from DATA rather than from reading code:
    # descriptors reference each other by tag, statically, in the image.  If those baked-in words
    # are exactly the tags this script computes, the toolchain knew them at build time -- so there
    # is nothing left for the runtime to choose.
    print("== static child-tag references == computed tags (compile-time constants) ==")
    tags = {e["tag"] for e in reg.elements}
    hdr = [eb.u32(e["obj"] + 0x10) for e in reg.elements]
    hdr = [c for c in hdr if c != 0xFFFFFFFF]
    fld = []
    for e in reg.elements:
        a = e["obj"] + 0x50
        if eb.u32(a + 4) != 0x8080010A:
            continue
        for i in range(eb.u32(a + 8)):
            m = a + 0x30 + i * 0x28
            if eb.u32(m + 4) != 0x3F000000:
                continue
            c = eb.u32(m + 0xC)
            if c != 0xFFFFFFFF:
                fld.append(c)
    check("all %d header child tags resolve" % len(hdr), all(c in tags for c in hdr),
          "%d unresolved" % sum(1 for c in hdr if c not in tags))
    check("all %d composite field child tags resolve" % len(fld), all(c in tags for c in fld),
          "%d unresolved" % sum(1 for c in fld if c not in tags))
    check("CONTROL: the tag set is not trivially everything", len(tags) == len(reg.elements)
          and 0x8080FFFF not in tags, "%d distinct tags for %d elements" % (len(tags), len(reg.elements)))

    print("\n%s" % ("ALL CHECKS PASSED" if ok else "*** FAILURES ABOVE ***"))
    return 0 if ok else 1


def cmd_stats(eb, reg):
    from collections import Counter
    print("element list %#x -> %d elements, hashes strictly ascending, all unique" %
          (ELEMENT_LIST, len(reg.elements)))
    c = Counter(e["cont"] for e in reg.elements)
    for k in sorted(c):
        print("  container %d  slot %#05x  %5d elements   tags %#010x..%#010x" %
              (k, CONTAINER_SLOT[k], c[k],
               min(e["tag"] for e in reg.elements if e["cont"] == k),
               max(e["tag"] for e in reg.elements if e["cont"] == k)))
    print()
    print("%-20s %-11s %6s %-11s %s" % ("module", "hash table", "count", "slot array", "containers used"))
    for name, tv, n, slots in MODULES:
        used = Counter()
        for (m, c), e in reg.binding.items():
            if m == name:
                used[e["cont"]] += 1
        print("%-20s %#011x %6d %#011x %s" %
              (name, tv, n, slots, dict(sorted(used.items()))))
    return 0


def cmd_lookup(eb, reg, hits, what):
    if not hits:
        print("%s: no element" % what)
        return 1
    for e in hits:
        print(describe(eb, e))
        for name, c, ent in reg.modules_of(e):
            print("    module %-20s table entry #%d @%#x  slot %#x  companion %s" %
                  (name, c, ent["entry"], ent["slot"], "(see +0x18 at runtime)"))
        print()
    return 0


def cmd_list(eb, reg, module):
    t = reg.tables[module]
    print("%s: %d entries" % (module, len(t)))
    for c, ent in enumerate(t):
        e = reg.binding.get((module, c))
        print("  [%4d] hash %08X  slot %#011x  tag %s  object %s" %
              (c, ent["hash"], ent["slot"] or 0,
               "%#010x" % e["tag"] if e else "UNBOUND",
               "%#010x" % e["obj"] if e else "-"))
    return 0


def cmd_dump(eb, reg, path):
    with open(path, "w") as f:
        f.write("tag\tname_hash\tobject\tcontainer\tslot_id\tindex\tkind\tchild\twidth\tmodules\n")
        for e in reg.elements:
            mods = ";".join("%s#%d@%#x" % (m, c, t["slot"] or 0) for m, c, t in reg.modules_of(e))
            f.write("%#010x\t%08X\t%#010x\t%d\t%#05x\t%#06x\t%#x\t%#010x\t%d\t%s\n" %
                    (e["tag"], e["hash"], e["obj"], e["cont"], CONTAINER_SLOT[e["cont"]], e["idx"],
                     eb.u32(e["obj"]), eb.u32(e["obj"] + 0x10), eb.u32(e["obj"] + 0x14), mods))
    print("wrote %d elements to %s" % (len(reg.elements), path))
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--eboot", default=EBOOT_DEFAULT)
    ap.add_argument("--verify", action="store_true")
    ap.add_argument("--stats", action="store_true")
    ap.add_argument("--tag")
    ap.add_argument("--hash")
    ap.add_argument("--object")
    ap.add_argument("--module")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--dump")
    a = ap.parse_args()

    eb = Eboot(a.eboot)
    reg = Registry(eb)

    if a.verify:
        return cmd_verify(eb, reg)
    if a.stats:
        return cmd_stats(eb, reg)
    if a.tag:
        return cmd_lookup(eb, reg, reg.by_tag(int(a.tag, 0)), a.tag)
    if a.hash:
        return cmd_lookup(eb, reg, reg.by_hash(int(a.hash, 0)), a.hash)
    if a.object:
        return cmd_lookup(eb, reg, reg.by_object(int(a.object, 0)), a.object)
    if a.module and a.list:
        return cmd_list(eb, reg, a.module)
    if a.dump:
        return cmd_dump(eb, reg, a.dump)
    ap.print_help()
    return 2


if __name__ == "__main__":
    sys.exit(main())
