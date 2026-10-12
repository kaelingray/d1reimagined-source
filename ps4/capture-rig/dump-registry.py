#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Read the live client's package registry, and resolve content/schema tags through it.

WHY: registry slot `0x400` (package id 0) is **eboot-resident** - there is no `.pkg` file for it, and
neither a static rebuild nor a Unicorn trace can reconstruct its entry table
(`historicaldocs/design/character-and-orbit.md` §15.1/§15.2: route (a) found zero candidates; route (b) faults
because the field walker re-resolves *every* field's sub-type tag through the same table). The
running client has already built it, so read it from there.

HOW: shadPS4 maps the guest at `g_eboot_address + vaddr` (the same convention the patch XML uses:
`Address = eboot vaddr + 0x400000`, and PatchMemory computes `g_eboot_address + (Address-0x400000)`).
We recover `g_eboot_address` by searching the process for a byte signature taken from a unique,
non-relocated spot in our own eboot copy, then read:

    registry = *(*(base + 0x1A0B420))          # both resolution sites lea this same global
    slot     = registry + tag_slot(tag)*0x40   # +0x10 data base · +0x30 entry size · +0x34 mask
    entry    = slot_data + index*entry_size
    object   = (slot_data + index*entry_size) - (mask & qword[entry+8])

Usage:
    ./dump-registry.py                       # summarise the registry and slot 0x400
    ./dump-registry.py --tag 0x80801825      # resolve one tag and hexdump its object
"""
import argparse
import ctypes
import os
import re
import struct
import subprocess
import sys

EBOOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                     "dumps", "patch-01.29-CUSA00219", "eboot.bin")
BIAS = 0x4000
REGISTRY_GLOBAL = 0x1A0B420
SIG_VADDR = 0x1324060          # near the 503 response descriptor; distinctive, no relocations here
SIG_LEN = 48


def emu_pid():
    out = subprocess.run(["pgrep", "-f", "[s]hadps4 -p "], capture_output=True, text=True).stdout
    pids = [int(x) for x in out.split()]
    if not pids:
        raise SystemExit("no running shadps4 (pgrep -f '[s]hadps4 -p ')")
    return pids[0]


class Mem:
    def __init__(self, pid):
        self.pid = pid
        self.f = open(f"/proc/{pid}/mem", "rb", 0)
        self.regions = []
        for line in open(f"/proc/{pid}/maps"):
            m = re.match(r"([0-9a-f]+)-([0-9a-f]+) (\S{4})", line)
            if m and "r" in m.group(3):
                self.regions.append((int(m.group(1), 16), int(m.group(2), 16), line.strip()))

    def read(self, addr, n):
        self.f.seek(addr)
        return self.f.read(n)

    def find(self, sig):
        hits = []
        for lo, hi, _ in self.regions:
            if hi - lo > 0x40000000:      # skip enormous reservations
                continue
            try:
                self.f.seek(lo)
                blob = self.f.read(hi - lo)
            except Exception:
                continue
            start = 0
            while True:
                j = blob.find(sig, start)
                if j < 0:
                    break
                hits.append(lo + j)
                start = j + 1
                if len(hits) > 8:
                    return hits
        return hits


def tag_slot(tag):
    eax = ctypes.c_int32(tag).value
    esi = eax >> 13
    edx = (ctypes.c_uint32(esi).value >> 0x12) | 0x3FF
    return (ctypes.c_int32(edx).value & esi) & 0xFFFFFFFF, tag & 0x1FFF


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0],
                                 formatter_class=argparse.RawDescriptionHelpFormatter,
                                 epilog=__doc__)
    ap.add_argument("--tag", type=lambda s: int(s, 0), action="append", default=[])
    ap.add_argument("--bytes", type=int, default=256, help="hexdump length per resolved object")
    ap.add_argument("--schema", action="store_true",
                    help="walk the field tree instead of hexdumping (see FIELD RECORD below)")
    ap.add_argument("--depth", type=int, default=3)
    args = ap.parse_args()

    eb = open(EBOOT, "rb").read()
    sig = eb[SIG_VADDR + BIAS: SIG_VADDR + BIAS + SIG_LEN]

    pid = emu_pid()
    mem = Mem(pid)
    print(f"shadps4 pid {pid}, {len(mem.regions)} readable regions")
    hits = mem.find(sig)
    if not hits:
        raise SystemExit("eboot signature not found in the process - is the game loaded yet?")
    bases = sorted({h - SIG_VADDR for h in hits})
    print(f"signature at {[hex(h) for h in hits]}  =>  candidate eboot base(s) {[hex(b) for b in bases]}")
    base = bases[0]

    g = mem.read(base + REGISTRY_GLOBAL, 8)
    p1 = struct.unpack("<Q", g)[0]
    print(f"registry global @ {base + REGISTRY_GLOBAL:#x} -> {p1:#x}")
    if not p1:
        raise SystemExit("registry pointer is NULL - the client has not built it yet")
    registry = struct.unpack("<Q", mem.read(p1, 8))[0]
    print(f"registry = {registry:#x}")

    # FIELD RECORD (40 bytes, array starts at object+0x80; count at object+0x58):
    #   +0x00 marker 0x3f00000000000000 · +0x08 u32 kind · +0x0c u32 type_tag (0xffffffff = leaf)
    #   +0x14 u32 width_in_bits · +0x20 u32 byte offset within the decoded C struct
    def resolve(tag):
        slot, index = tag_slot(tag)
        rec = mem.read(registry + slot * 0x40, 0x40)
        data_base = struct.unpack_from("<Q", rec, 0x10)[0]
        esize = struct.unpack_from("<i", rec, 0x30)[0]
        mask = struct.unpack_from("<i", rec, 0x34)[0]
        if not data_base or esize <= 0:
            return None
        ent = data_base + index * esize
        loc = struct.unpack("<Q", mem.read(ent + 8, 8))[0]
        return ent - ctypes.c_int64(mask & loc).value

    def walk(tag, depth=0, seen=None):
        seen = seen if seen is not None else set()
        pad = "   " * depth
        obj = resolve(tag)
        if obj is None:
            print(f"{pad}{tag:#010x}  <slot empty>"); return
        hdr = mem.read(obj, 0x80)
        size = struct.unpack_from("<I", hdr, 0x64)[0]
        n = struct.unpack_from("<I", hdr, 0x58)[0]
        # ⚠ SANITY GATE. Depth-2 composites were printing "struct 0 bytes, 2155872336 field(s)" -
        # `n` there is a 0x8080xxxx TAG, i.e. `resolve()` returned an address that is not a
        # descriptor at all. Printing it as a field count produced authoritative-looking garbage
        # inside otherwise-correct output, which is exactly how a bad reading gets believed.
        # Refuse to describe an object we cannot show is a descriptor.
        if n > 64 or size == 0 or (n & 0xFFFF0000) == 0x80800000:
            print(f"{pad}{tag:#010x}  <UNRESOLVED - bad descriptor at {obj:#x}: "
                  f"size={size} count={n:#x}; do NOT trust nested output for this tag>")
            return
        print(f"{pad}{tag:#010x}  struct {size} bytes, {n} field(s)")
        if tag in seen or depth >= args.depth or n > 64:
            if tag in seen: print(f"{pad}   (already expanded)")
            return
        seen.add(tag)
        # ⚠ THE C-OBJECT OFFSET LIVES AT marker-8, NOT marker+0x20. The client's own field walker
        # reads it from marker-8; reading marker+0x20 yields the NEXT field's offset, and on the
        # LAST field that lands in the neighbouring descriptor (the bogus "@struct+0xc8" that made
        # an 8-byte struct look impossible). Verified two ways: it gives 0x80801796 = {+0x00 4-bit,
        # +0x04 32-bit} = 8 bytes exactly as stated, and for 0x80801825 the final field lands at
        # +0x18 with 0x18 + 944 == 968 == the struct size.
        # Read one qword early so marker-8 is in range for field 0.
        raw = mem.read(obj + 0x78, 8 + n * 0x28)
        for i in range(n):
            o = 8 + i * 0x28           # o indexes the MARKER within `raw`
            kind = struct.unpack_from("<I", raw, o + 0x08)[0]
            ttag = struct.unpack_from("<I", raw, o + 0x0c)[0]
            bits = struct.unpack_from("<I", raw, o + 0x14)[0]
            soff = struct.unpack_from("<I", raw, o - 8)[0]
            if ttag == 0xFFFFFFFF:
                print(f"{pad}   [{i}] kind {kind:#x}  LEAF  {bits} bits   @struct+{soff:#x}")
            else:
                print(f"{pad}   [{i}] kind {kind:#x}  -> {ttag:#010x}   @struct+{soff:#x}")
                walk(ttag, depth + 1, seen)

    if args.schema:
        for tag in (args.tag or [0x80801825]):
            walk(tag)
        return 0

    for tag in (args.tag or [0x80801825, 0x80801726, 0x80A5E37A]):
        slot, index = tag_slot(tag)
        s = registry + slot * 0x40
        rec = mem.read(s, 0x40)
        data_base, esize, mask = struct.unpack_from("<Q", rec, 0x10)[0], \
            struct.unpack_from("<i", rec, 0x30)[0], struct.unpack_from("<i", rec, 0x34)[0]
        print(f"\ntag {tag:#010x}: slot {slot:#x} index {index}  "
              f"data_base={data_base:#x} entry_size={esize} mask={mask:#x}")
        if not data_base or esize <= 0:
            print("   slot is empty in this process")
            continue
        ent = data_base + index * esize
        loc = struct.unpack("<Q", mem.read(ent + 8, 8))[0]
        # `and rbx,[rsi+rax+8]` then `sub rsi,rbx` - rbx is a sign-extended i32 mask and the
        # subtraction is SIGNED, so the masked loc must be read as a signed 64-bit back-offset.
        masked = ctypes.c_int64(mask & loc).value
        obj = ent - masked
        print(f"   entry @ {ent:#x}  loc={loc:#x}  -> object @ {obj:#x}")
        blob = mem.read(obj, args.bytes)
        for i in range(0, min(len(blob), args.bytes), 16):
            print(f"     +{i:04x}  {blob[i:i+16].hex(' ')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
