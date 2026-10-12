#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
r"""The type-name-hash registries, read STATICALLY from the eboot - plus a hash oracle.

WHAT THIS IS
------------
`historicaldocs/design/character-and-orbit.md` §26.28 found a sorted table at `0x19D0BD8` keyed by a u32
"name hash", and asked for the hash function so that 1332 runtime types could be named.

This tool is the apparatus for that question. It:
  * locates BOTH registries (§26.28 saw only one) and reads them with the CORRECTED record
    layout - see GEOMETRY below, the published `{hash, pad, ptr}` reading is off by 8;
  * cross-checks the geometry against the two static initialisers that register the tables,
    which is where the element COUNTS come from rather than from a heuristic scan;
  * runs a candidate-name list against the registries through 24 hash functions x 12 string
    transforms, with a positive control, and reports hits against the noise floor.  (The
    exhaustive 52 x 20 C sweep whose result is recorded under RULED OUT is not carried here;
    this set is the useful subset for interactive use.)
  * diffs the registries between two eboots (01.00 vs 01.29 is informative - see below).

THE HASH IS NOT CRACKED. `--test` is the oracle you use when you have a *name corpus*; every
corpus reachable from this eboot has already been run through it and come back at noise.
Read `RULED OUT` before spending time here.

GEOMETRY  `[C]`
--------------
Record is 16 bytes and the POINTER COMES FIRST:

    +0x00  u64  -> .bss word that receives the runtime tag (relocated; 0 in the file)
    +0x08  u32  name hash
    +0x0c  u32  pad (always 0)

§26.28's `{u32 hash, pad4, u64 ptr}` reading is the same bytes framed 8 low; it yields an
entry [0] with hash 0 and a final entry whose pointer is not relocated. Both artefacts vanish
with the layout above, and every one of the 1332 (resp. 6338) pointers is then relocated.

Two registries, both sorted ascending by hash, both registered by a static initialiser that
calls `0xEE0D0(table, count)` (which appends `{ptr, count, 0}` to a global list at `0x25AB410`):

    registry A   base 0x19B3530   count 0x18C2 = 6338   registrar 0xC262D0
    registry B   base 0x19D0BD0   count 0x0534 = 1332   registrar 0xC2B2B0

Registry B is §26.28's table. Registry A was not previously known.

RULED OUT - do not re-run these  `[C]`
--------------------------------------
Corpus: every printable run in the 01.29 eboot (113,180 distinct strings), *and every
substring of every one of them up to 96 chars* (~10^8 trials), *and* 129,527 names derived
from the Bungie D1 `world_sql_content` manifest (table names, JSON keys, string values, plus
snake_case / `c_` / `s_` / `c_investment_` prefixed forms).

Hashes: FNV-1, FNV-1a (basis 0x811C9DC5 and 0, 32-bit, 64-bit folded and truncated),
djb2, djb2-xor, sdbm, x65599, x31, x37, x131, x17, Larson-101, Python-1000003, ELF/PJW,
Jenkins one-at-a-time (with and without finalisation), Jenkins lookup2 and lookup3,
CRC-32 (IEEE reflected, Castagnoli, MSB-first 0x04C11DB7, 0x1EDC6F41; each with and without
the initial/final inversion), Adler-32, MurmurHash2 (seeds 0, 1, -1), MurmurHash3-x86-32
(seeds 0, 1, -1, 0x9747B28C), MurmurHash64A (folded and truncated), SuperFastHash/Hsieh,
xxHash32 (seeds 0, 1), rot5-xor, rot7-add, rot3-xor, byte-sum, length-seeded sdbm/FNV-1a,
FNV-1a + Murmur3 finaliser.

Transforms: as-is, +NUL, lower, upper, reversed, strip `c_`, strip `s_`, last `::` component,
first `::` component, `::`->`_`, drop underscores, strip trailing `_x`, prepend `c_`,
prepend `s_`, UTF-16LE (raw and lowercased), u32 length prefix, Itanium `<len><name>` prefix,
backslash->slash, lower+NUL.

Every combination came back at the noise floor, verified against a same-run control in which
the corpus letters are Caesar-rotated (`--test --control`). The self-test path
(`--test --selftest`) injects known hashes and must report them, so an empty result is
distinguishable from a broken scanner.

Independent corroboration that the answer is not in this binary: a control-backed sweep of
every tight byte-loop in the image (6.9M instructions, 833 candidate mixing loops) finds
exactly TWO string-hash routines in the whole client - FNV-1 with `tolower` and `\`->`/`
normalisation (the *file path* hash, e.g. `0xBC980`, `0x57BED0`), and zlib's table CRC-32
(`0x2AD370`). Neither produces these values. The name hashes are baked by the schema compiler;
the client only ever compares them.

⚠ CORRECTION TO §26.28 - the span element's stable id is `0xE2E25C2C`, not `0xE2B769E8`  `[C]`
------------------------------------------------------------------------------------------
Each entry's `.bss` pointer is a RELA relocation, so the entry -> slot map is readable from the
file with no live client (`--slot`).  Doing that:

    global 0x19CC800  (the one 0x570A71 loads on the crash path)  -> .bss 0x28FD0B0
    .bss 0x28FD0B0 == registry B index 1167, name hash 0xE2E25C2C
    .bss 0x2904C70 == registry B index 1166, name hash 0xE2B769E8   <- nothing references it
    .bss 0x2900BF0 == registry B index 1224, name hash 0xED22E277   <- §26.28's quoted slot
    .bss 0x28FFA10 == registry B index   71, name hash 0x0D45C10A

§26.28 paired index 1166 with slot `0x2900BF0` and concluded the span element type is registered
under `0xE2B769E8`.  The relocations say `0x19CC800` resolves to index **1167 = 0xE2E25C2C**, and
that is independently corroborated: `0xE2E25C2C` is field-pool record 2147's *own* name hash
(§26.19), and record 2147 is the span element descriptor.  Two routes agree on 1167.
`[U]` §26.28's slot numbers came from a live read and mine from static RELA; the discrepancy is
not explained, so re-read the live slot before treating 1166 as dead.

CROSS-VERSION  `[C]`
--------------------
Registry B: 998 entries in 01.00, 1332 in 01.29; 814 shared, 518 new in 01.29.
`0xE2E25C2C` (field-pool record 2147's own hash) is present in BOTH.
`0xE2B769E8` - the span element type the `0x570AAE` crash is blocked on - is **new in 01.29**.
So the gate's element type did not exist at launch; whatever names it, it is 01.29-era.

Usage:
    ./name-hash-tables.py --dump B | head
    ./name-hash-tables.py --lookup 0xE2B769E8
    ./name-hash-tables.py --slot 0x19CC800          # the crash path's global -> which entry
    ./name-hash-tables.py --diff dumps/base-CUSA00219/eboot.bin
    ./name-hash-tables.py --test names.txt              # the oracle
    ./name-hash-tables.py --test names.txt --control    # same run, scrambled corpus
    ./name-hash-tables.py --test names.txt --selftest   # proves the oracle can see a hit
"""
import argparse
import os
import struct
import sys
import zlib

BIAS = 0x4000
DEFAULT_EBOOT = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    "..", "dumps", "patch-01.29-CUSA00219", "eboot.bin")

# base, count, registrar vaddr - counts read out of the registrars, not guessed.
REGISTRIES = {
    "A": (0x19B3530, 6338, 0xC262D0),
    "B": (0x19D0BD0, 1332, 0xC2B2B0),
}

M32 = 0xFFFFFFFF


def load(path):
    with open(path, "rb") as f:
        return f.read()


def entries(img, base, count):
    """Yield (index, name_hash, bss_slot_vaddr). bss slot is 0 in-file (relocated at load)."""
    for i in range(count):
        off = base + i * 16 + BIAS
        ptr, h, pad = struct.unpack_from("<QII", img, off)
        yield i, h, ptr, pad


def find_registries(img):
    """Re-derive both registries from the file rather than trusting the constants.

    A registry is a maximal run of 16-byte records, stride 16, whose +8 u32 ascends strictly
    and whose +0xc is zero.  Returns [(base, count)].  Used by --verify.
    """
    out = []
    for name, (base, count, _) in REGISTRIES.items():
        lo = base
        while True:
            p = lo - 16
            a = struct.unpack_from("<I", img, p + 8 + BIAS)[0]
            b = struct.unpack_from("<I", img, lo + 8 + BIAS)[0]
            if a and a < b:
                lo = p
            else:
                break
        hi = base
        while True:
            n = hi + 16
            a = struct.unpack_from("<I", img, hi + 8 + BIAS)[0]
            b = struct.unpack_from("<I", img, n + 8 + BIAS)[0]
            if b and b > a:
                hi = n
            else:
                break
        out.append((name, lo, (hi - lo) // 16 + 1))
    return out


# --------------------------------------------------------------------------- hash battery

def _rotl(x, r):
    return ((x << r) | (x >> (32 - r))) & M32


def fnv1(b, off=0x811C9DC5, p=0x01000193):
    h = off
    for c in b:
        h = ((h * p) & M32) ^ c
    return h


def fnv1a(b, off=0x811C9DC5, p=0x01000193):
    h = off
    for c in b:
        h = ((h ^ c) * p) & M32
    return h


def _mul_add(b, mul, init=0):
    h = init
    for c in b:
        h = (h * mul + c) & M32
    return h


def djb2(b):
    return _mul_add(b, 33, 5381)


def djb2x(b):
    h = 5381
    for c in b:
        h = ((h * 33) & M32) ^ c
    return h


def sdbm(b):
    h = 0
    for c in b:
        h = (c + (h << 6) + (h << 16) - h) & M32
    return h


def elf_hash(b):
    h = 0
    for c in b:
        h = ((h << 4) + c) & M32
        g = h & 0xF0000000
        if g:
            h ^= g >> 24
        h &= ~g & M32
    return h


def joaat(b):
    h = 0
    for c in b:
        h = (h + c) & M32
        h = (h + (h << 10)) & M32
        h ^= h >> 6
    h = (h + (h << 3)) & M32
    h ^= h >> 11
    h = (h + (h << 15)) & M32
    return h


def crc32_ieee(b):
    return zlib.crc32(b) & M32


def crc32c(b):
    crc = M32
    for c in b:
        crc ^= c
        for _ in range(8):
            crc = (crc >> 1) ^ 0x82F63B78 if crc & 1 else crc >> 1
    return crc ^ M32


def crc32_msb(b, poly=0x04C11DB7):
    crc = M32
    for c in b:
        crc ^= c << 24
        for _ in range(8):
            crc = ((crc << 1) ^ poly) & M32 if crc & 0x80000000 else (crc << 1) & M32
    return crc ^ M32


def murmur2(b, seed=0):
    m, r = 0x5BD1E995, 24
    ln = len(b)
    h = (seed ^ ln) & M32
    i = 0
    while ln >= 4:
        k = struct.unpack_from("<I", b, i)[0]
        k = (k * m) & M32
        k ^= k >> r
        k = (k * m) & M32
        h = (h * m) & M32
        h ^= k
        i += 4
        ln -= 4
    if ln == 3:
        h ^= b[i + 2] << 16
    if ln >= 2:
        h ^= b[i + 1] << 8
    if ln >= 1:
        h ^= b[i]
        h = (h * m) & M32
    h ^= h >> 13
    h = (h * m) & M32
    h ^= h >> 15
    return h


def murmur3(b, seed=0):
    c1, c2 = 0xCC9E2D51, 0x1B873593
    h = seed
    n = len(b)
    nb = n // 4
    for i in range(nb):
        k = struct.unpack_from("<I", b, i * 4)[0]
        k = (k * c1) & M32
        k = _rotl(k, 15)
        k = (k * c2) & M32
        h ^= k
        h = _rotl(h, 13)
        h = (h * 5 + 0xE6546B64) & M32
    k = 0
    tail = n & 3
    i = nb * 4
    if tail == 3:
        k ^= b[i + 2] << 16
    if tail >= 2:
        k ^= b[i + 1] << 8
    if tail >= 1:
        k ^= b[i]
        k = (k * c1) & M32
        k = _rotl(k, 15)
        k = (k * c2) & M32
        h ^= k
    h ^= n
    h ^= h >> 16
    h = (h * 0x85EBCA6B) & M32
    h ^= h >> 13
    h = (h * 0xC2B2AE35) & M32
    h ^= h >> 16
    return h


HASHES = {
    "fnv1": fnv1,
    "fnv1a": fnv1a,
    "fnv1_z": lambda b: fnv1(b, 0),
    "fnv1a_z": lambda b: fnv1a(b, 0),
    "djb2": djb2,
    "djb2x": djb2x,
    "djb2_z": lambda b: _mul_add(b, 33),
    "sdbm_x65599": sdbm,
    "x31": lambda b: _mul_add(b, 31),
    "x37": lambda b: _mul_add(b, 37),
    "x131": lambda b: _mul_add(b, 131),
    "larson101": lambda b: _mul_add(b, 101),
    "elf": elf_hash,
    "joaat": joaat,
    "crc32": crc32_ieee,
    "crc32_noinv": lambda b: zlib.crc32(b, 0) & M32 ^ 0,
    "crc32c": crc32c,
    "crc32_msb": crc32_msb,
    "crc32_msb_1EDC": lambda b: crc32_msb(b, 0x1EDC6F41),
    "murmur2_s0": lambda b: murmur2(b, 0),
    "murmur2_s1": lambda b: murmur2(b, 1),
    "murmur3_s0": lambda b: murmur3(b, 0),
    "murmur3_s1": lambda b: murmur3(b, 1),
    "murmur3_sFF": lambda b: murmur3(b, M32),
}


def transforms(s):
    yield "plain", s.encode("latin1", "ignore")
    yield "nul", s.encode("latin1", "ignore") + b"\0"
    yield "lower", s.lower().encode("latin1", "ignore")
    yield "upper", s.upper().encode("latin1", "ignore")
    if s.startswith("c_"):
        yield "noc_", s[2:].encode("latin1", "ignore")
    if s.startswith("s_"):
        yield "nos_", s[2:].encode("latin1", "ignore")
    if "::" in s:
        yield "lastns", s.rsplit("::", 1)[1].encode("latin1", "ignore")
        yield "firstns", s.split("::", 1)[0].encode("latin1", "ignore")
        yield "ns2und", s.replace("::", "_").encode("latin1", "ignore")
    yield "pre_c_", ("c_" + s).encode("latin1", "ignore")
    yield "pre_s_", ("s_" + s).encode("latin1", "ignore")
    yield "utf16", s.encode("utf-16-le", "ignore")
    yield "manglen", (str(len(s)) + s).encode("latin1", "ignore")


def scramble(s):
    out = []
    for c in s:
        if "a" <= c <= "z":
            out.append(chr((ord(c) - 97 + 7) % 26 + 97))
        elif "A" <= c <= "Z":
            out.append(chr((ord(c) - 65 + 7) % 26 + 65))
        else:
            out.append(c)
    return "".join(out)


# --------------------------------------------------------------------------- commands

def cmd_dump(img, which):
    base, count, _ = REGISTRIES[which]
    for i, h, ptr, pad in entries(img, base, count):
        print(f"[{i:5d}] hash=0x{h:08X} slot=0x{ptr:X} pad={pad}")


def cmd_lookup(img, value):
    for which, (base, count, reg) in REGISTRIES.items():
        for i, h, ptr, _ in entries(img, base, count):
            if h == value:
                print(f"registry {which} (base 0x{base:X}, {count} entries, registrar 0x{reg:X})"
                      f"  index {i}  hash 0x{h:08X}  .bss slot 0x{ptr:X} (in-file; relocated at load)")
                return 0
    print(f"0x{value:08X} not present in either registry")
    return 1


def read_rela(img):
    """RELA + JMPREL, as {r_offset: r_addend}. dynlibdata at file offset 0x1A0D100."""
    dl = 0x1A0D100
    out = {}
    for off, size in ((0xBDD0, 0x1B0918), (0x8200, 0x3BD0)):
        for k in range(size // 24):
            r_off, _r_info, r_add = struct.unpack_from("<QQq", img, dl + off + k * 24)
            out[r_off] = r_add
    return out


def slot_map(img):
    """{.bss slot vaddr: (registry, index, name_hash)} from the relocations."""
    rela = read_rela(img)
    m = {}
    for name, (base, count, _) in REGISTRIES.items():
        for i in range(count):
            addr = base + i * 16
            if addr in rela:
                h = struct.unpack_from("<I", img, addr + 8 + BIAS)[0]
                m[rela[addr]] = (name, i, h)
    return m


def cmd_slot(img, value):
    m = slot_map(img)
    if value in m:
        name, i, h = m[value]
        print(f"0x{value:X} -> registry {name} index {i}  name hash 0x{h:08X}")
        return 0
    # maybe the caller passed a global that POINTS at a slot
    rela = read_rela(img)
    if value in rela and rela[value] in m:
        name, i, h = m[rela[value]]
        print(f"global 0x{value:X} -> .bss 0x{rela[value]:X} -> registry {name} index {i}  "
              f"name hash 0x{h:08X}")
        return 0
    print(f"0x{value:X} is neither a registry .bss slot nor a global pointing at one")
    return 1


def cmd_verify(img):
    print("registry geometry re-derived from the file (must match the registrar constants):")
    ok = True
    for name, lo, n in find_registries(img):
        base, count, reg = REGISTRIES[name]
        good = (lo == base and n == count)
        ok &= good
        print(f"  {name}: derived base=0x{lo:X} count={n} | registrar says base=0x{base:X} "
              f"count={count} ({'OK' if good else 'MISMATCH'})")
    # positive control: pad must be zero everywhere, hashes strictly ascending
    for name, (base, count, _) in REGISTRIES.items():
        prev = -1
        bad = 0
        for i, h, ptr, pad in entries(img, base, count):
            if pad or h <= prev:
                bad += 1
            prev = h
        print(f"  {name}: {bad} records violating (pad==0 and strictly ascending)")
        ok &= (bad == 0)
    return 0 if ok else 1


def cmd_diff(img, other_path):
    other = load(other_path)
    for name, (base, count, _) in REGISTRIES.items():
        mine = set(h for _, h, _, _ in entries(img, base, count))
        # Locate the same registry in the other image: probe with several shared hashes, grow
        # each candidate into its maximal strictly-ascending stride-16 run, keep the longest.
        # A single 16-byte coincidence is not a registry, so the longest run is the only safe
        # choice - an early-exit on the first probe finds 3-entry noise runs instead.
        ordered = sorted(mine)
        probes = [ordered[i * len(ordered) // 9] for i in range(1, 9)]
        best = (0, None)
        for pv in probes:
            probe = struct.pack("<I", pv)
            idx = other.find(probe)
            while idx >= 0:
                v = idx - BIAS - 8
                if v % 16 == 0 and v > 16:
                    lo = hi = v
                    try:
                        while True:
                            a = struct.unpack_from("<I", other, lo - 16 + 8 + BIAS)[0]
                            b = struct.unpack_from("<I", other, lo + 8 + BIAS)[0]
                            if a and a < b:
                                lo -= 16
                            else:
                                break
                        while True:
                            a = struct.unpack_from("<I", other, hi + 8 + BIAS)[0]
                            b = struct.unpack_from("<I", other, hi + 16 + 8 + BIAS)[0]
                            if b and b > a:
                                hi += 16
                            else:
                                break
                    except struct.error:
                        idx = other.find(probe, idx + 1)
                        continue
                    n = (hi - lo) // 16 + 1
                    if n > best[0]:
                        best = (n, lo)
                idx = other.find(probe, idx + 1)
        n, lo = best
        if lo is None or n < 64:
            print(f"registry {name}: not located in {other_path} "
                  f"(longest candidate run {n} entries - too short to be a registry)")
            continue
        theirs = set(h for _, h, _, _ in entries(other, lo, n))
        print(f"registry {name}: this={len(mine)}  other=0x{lo:X}/{n}  "
              f"shared={len(mine & theirs)}  new-here={len(mine - theirs)}  gone={len(theirs - mine)}")


def cmd_test(img, names_path, control, selftest):
    hashes = set()
    for _, (base, count, _) in REGISTRIES.items():
        hashes |= set(h for _, h, _, _ in entries(img, base, count))
    names = [ln.rstrip("\n") for ln in open(names_path, encoding="utf-8", errors="replace")]
    names = [n for n in names if 2 < len(n) < 300]
    if control:
        names = [scramble(n) for n in names]
    seeded = 0
    if selftest:
        # inject sdbm() of the first 200 plain names; the oracle MUST report them.
        for n in names[:200]:
            hashes.add(sdbm(n.encode("latin1", "ignore")))
            seeded += 1
    trials = 0
    res = {}
    for n in names:
        for tname, b in transforms(n):
            if not b:
                continue
            for hname, fn in HASHES.items():
                trials += 1
                try:
                    h = fn(b)
                except Exception:
                    continue
                if h in hashes:
                    key = (hname, tname)
                    res.setdefault(key, []).append(n)
    floor = trials * len(hashes) / 2 ** 32 / max(1, len(HASHES) * 12)
    print(f"names={len(names)} registries={len(hashes)} trials={trials} "
          f"seeded={seeded} per-combo noise floor ~ {floor:.3f}")
    if not res:
        print("no hits in any (hash, transform) combination")
        return 1
    for (hname, tname), hits in sorted(res.items(), key=lambda kv: -len(kv[1])):
        print(f"{len(hits):6d}  {hname:<16s} {tname:<10s}  e.g. {hits[0]!r}")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--eboot", default=DEFAULT_EBOOT)
    ap.add_argument("--dump", choices=["A", "B"])
    ap.add_argument("--lookup")
    ap.add_argument("--slot", metavar="VADDR",
                    help=".bss slot (or a global pointing at one) -> registry index + name hash")
    ap.add_argument("--verify", action="store_true")
    ap.add_argument("--diff", metavar="OTHER_EBOOT")
    ap.add_argument("--test", metavar="NAMES_FILE")
    ap.add_argument("--control", action="store_true",
                    help="with --test: Caesar-scramble the corpus; result is the noise floor")
    ap.add_argument("--selftest", action="store_true",
                    help="with --test: inject known hashes; a zero result means the oracle is broken")
    a = ap.parse_args()
    img = load(a.eboot)
    if a.dump:
        cmd_dump(img, a.dump)
    elif a.lookup:
        return cmd_lookup(img, int(a.lookup, 0))
    elif a.slot:
        return cmd_slot(img, int(a.slot, 0))
    elif a.verify:
        return cmd_verify(img)
    elif a.diff:
        cmd_diff(img, a.diff)
    elif a.test:
        return cmd_test(img, a.test, a.control, a.selftest)
    else:
        ap.print_help()
    return 0


if __name__ == "__main__":
    sys.exit(main())
