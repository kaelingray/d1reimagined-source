#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Trace an investment message through the client's own native decoder.

WHY THIS AND NOT STATIC READING
-------------------------------
`historicaldocs/design/character-and-orbit.md` §13.4 records failed attempts to read the descriptor format by
inspection. Running the decoder removes most of that guesswork, but the primitive-call log is not
the grammar by itself: it exposes leaf widths, not codec transformations, composite control bits,
or the decoded C-object layout. Always inspect the object reached through output_handle+8.

The decoder is `0x261FB0`. Its runtime dependencies are supplied offline:

* `0x25C9A0` (the Investment Globals manager singleton) -> a fake object whose vtable slot `+0x4D0`
  returns a fake definition record. Only `[rec+0x18]` (= record `+0x28`, the RESPONSE schema tag) and
  `[rec+0x14]` (= `+0x24`, REQUEST) are read.
* a minimal fail-closed package registry covering the transitive closure of the selected schema.
  This lets all native registry walks run unchanged and maps missing entries to null.

VALIDATION: run it on the **request** schema first and compare both primitive reads and the decoded
C object with the captured request. For canonical response bytes, use
`encode-investment-503.py`: it requires native encode -> decode -> encode byte identity and checks
the semantic response fields.

Usage:
    ./trace-investment-schema.py --request
    ./trace-investment-schema.py --response
"""
import argparse
import ctypes
import os
import struct
import sys
from importlib.machinery import SourceFileLoader

HERE = os.path.dirname(os.path.abspath(__file__))
h = SourceFileLoader("d1h", os.path.join(HERE, "d1-oodle-harness.py")).load_module()
_ed = SourceFileLoader("d1ed", os.path.join(HERE, "..", "eboot-descriptors.py")).load_module()
DESC_INDEX = _ed.Eboot(_ed.EBOOT).index()   # tag -> anchor vaddr, 4555 entries
from unicorn.x86_const import (UC_X86_REG_R14, UC_X86_REG_RDI, UC_X86_REG_RSI, UC_X86_REG_RDX,
                               UC_X86_REG_RCX, UC_X86_REG_R8, UC_X86_REG_RIP,
                               UC_X86_REG_RAX, UC_X86_REG_RSP)

DECODER      = 0x261FB0
GET_MANAGER  = 0x25C9A0
RESOLVE_SITE = 0x2622D1          # `mov rsi, [rcx+rdx+0x10]` - first insn of the registry walk
RESOLVE_DONE = 0x2622F0          # `movzx ebx, byte [rsi+0x18]` - first insn after it
READ_BITS    = 0x72C70           # read N bits  -> eax           (rdi=stream, esi=n)
READ_WIDE    = 0x73170           # read N bits  -> rax           (rdi=stream, esi=n)
READ_BLOB    = 0x730B0           # read N bits into a buffer     (rdi=stream, rsi=dst, edx=n)
ID_TO_INDEX  = 0x27FDD0

# Descriptor record heads located by tag search; see §12.2 / §14.3.
DESC = {"request": 0x12FE950, "response": 0x1324080}

STUB_ACCESSOR = 0x2FF0000        # unused address we point the vtable slot at

# ── SECOND registry-resolution site (added 2026-08-07) ────────────────────────────────────────────
# `RESOLVE_SITE` above is not the only one. `0x2623E0..0x262423` inlines the SAME package-registry
# arithmetic a second time, for the child tag in `r14d`, and faults on `[rbx+rcx+0x10]` because
# `rbx` comes from the registry global `0x1A0B420` (`lea rdx,[rip+0x17a9022]` at `0x2623F7`).
# This is what "two stub sites, not N" meant - the walker re-resolves, it does not resolve per-field.
#
# We satisfy it with the REAL tag rather than bypassing it: read `r14d`, look the tag up in the
# static eboot descriptor index, and hand back the object. Registry-convention object address is
# `anchor - 0x50` (verified: tag 0x80801796 -> registry object vaddr 0x139d1d8, anchor 0x139d228).
# Per the research lane's guardrail this supplies a validated tag→exact-descriptor map ONLY; it does
# not reimplement any grammar.
# ── SYNTHETIC PACKAGE REGISTRY (2026-08-07) ───────────────────────────────────────────────────────
# Decoder 0x261FB0 inlines the registry walk THREE times -- RIP-refs to the global 0x1A0B420 at
# 0x2622C7, 0x2623FE, 0x2624EF -- each with different register allocation, and its callees resolve
# more. Per-site stubs are therefore the fragile choice: a site we have not found is a SILENT WRONG
# ANSWER until it happens to fault. We build the real structure once and let the client's own code
# run. Construction contract agreed with the research lane (clkm_539f123a):
#
#   *(0x1A0B420) = holder ;  *holder = registry
#   registry + slot*0x40 :  +0x10 data_base   +0x30 entry_size(24)   +0x34 mask(-1)
#   entry E = data_base + index*24 ;  object = E - (mask & qword[E+8]) = E - loc
#     occupied  -> loc = E - O   (resolves to O)
#     UNOCCUPIED-> loc = E       (resolves to NULL, not a plausible address)  <-- FAIL CLOSED
#
# ⚠ eboot-descriptors.py indexes ANCHORS, not registry-object starts, and the prefix VARIES
# (0x50 for 0x80801825/0x80801796, 0x58 for 0x808045f0). Recover O by the structural relation
#   O + 0x48 + qword[O+0x48] == anchor + 8
# which is unique for every tag tested. A fixed offset would silently mis-resolve.
REGISTRY_GLOBAL = 0x1A0B420
ENTRY_SIZE = 24


def tag_slot(tag):
    """(slot, index) - the client's own arithmetic. NEVER key on `tag & 0x1fff` alone: across the
    4555 descriptors that collapses 4555 tags onto 3873 indices with 656 collisions."""
    eax = ctypes.c_int32(tag).value
    esi = eax >> 13
    edx = (ctypes.c_uint32(esi).value >> 0x12) | 0x3FF
    return (ctypes.c_int32(edx).value & esi) & 0xFFFFFFFF, tag & 0x1FFF


def object_start(eb, anchor, span=0x300):
    """Registry-object start O for a descriptor anchor. Unique candidate, or None."""
    hits = [O for O in range(anchor - span, anchor)
            if O + 0x48 + eb.u64(O + 0x48) == anchor + 8]
    return hits[0] if len(hits) == 1 else None


def closure(eb, idx, root):
    """Transitive closure of child tags from `root`. Minimal by construction - we add a tag only
    because the schema reaches it, never because it exists in the image."""
    seen, todo = set(), [root]
    while todo:
        t = todo.pop()
        if t in seen or t not in idx:
            continue
        seen.add(t)
        a = idx[t]
        for i in range(eb.u32(a + 8)):
            mk = a + 0x30 + i * 0x28
            if eb.u32(mk + 4) != 0x3F000000:
                break
            child = eb.u32(mk + 0x0C)
            if child != 0xFFFFFFFF:
                todo.append(child)
    return seen


def build_registry(H, eb, idx, root, log=True):
    """Install a minimal fail-closed registry covering exactly the closure of `root`."""
    mu = H.mu
    tags = closure(eb, idx, root)
    slots = {}
    for t in sorted(tags):
        slot, index = tag_slot(t)
        O = object_start(eb, idx[t])
        if O is None:
            raise RuntimeError(f"tag {t:#010x}: registry-object start not uniquely recoverable")
        slots.setdefault(slot, {})[index] = O

    registry = H.alloc(0x40 * (max(slots) + 1))
    for slot, entries in sorted(slots.items()):
        n = max(entries) + 1
        data_base = H.alloc(n * ENTRY_SIZE)
        for i in range(n):                       # fail closed: every entry resolves to NULL...
            E = data_base + i * ENTRY_SIZE
            mu.mem_write(E + 8, struct.pack("<Q", E))
        for index, O in entries.items():         # ...except the ones the schema actually reaches
            E = data_base + index * ENTRY_SIZE
            mu.mem_write(E + 8, struct.pack("<Q", (E - O) & 0xFFFFFFFFFFFFFFFF))
        rec = registry + slot * 0x40
        mu.mem_write(rec + 0x10, struct.pack("<Q", data_base))
        mu.mem_write(rec + 0x30, struct.pack("<i", ENTRY_SIZE))
        mu.mem_write(rec + 0x34, struct.pack("<i", -1))

    holder = H.alloc(8)
    mu.mem_write(holder, struct.pack("<Q", registry))
    mu.mem_write(REGISTRY_GLOBAL, struct.pack("<Q", holder))
    if log:
        print(f"registry: {len(tags)} tags in closure of {root:#010x}, "
              f"{len(slots)} slot(s) {[hex(s) for s in sorted(slots)]}, "
              f"registry {registry:#x}")
    return tags


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0],
                                 formatter_class=argparse.RawDescriptionHelpFormatter,
                                 epilog=__doc__)
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--request", action="store_true")
    g.add_argument("--response", action="store_true")
    ap.add_argument("--body", help="hex argument bitstream to feed after the u16 id")
    ap.add_argument("--msgid", type=lambda s: int(s, 0), default=503)
    args = ap.parse_args()

    which = "request" if args.request else "response"
    desc = DESC[which]
    direction = 0 if args.request else 1

    H = h.Harness()
    mu = H.mu
    reads = []
    resolved = []
    recent = []          # ring of recent RIPs: names the insn that dispatched to 0

    # Fake manager + vtable so 0x25C9A0 and `call [rcx+0x4D0]` succeed.
    fake_rec = H.alloc(0x80)
    mu.mem_write(fake_rec + 0x14, struct.pack("<I", 0x80801726))   # record +0x24 = REQUEST schema
    mu.mem_write(fake_rec + 0x18, struct.pack("<I", 0x80801825))   # record +0x28 = RESPONSE schema
    fake_vt = H.alloc(0x600)
    mu.mem_write(fake_vt + 0x4D0, struct.pack("<Q", STUB_ACCESSOR))
    fake_mgr = H.alloc(0x40)
    mu.mem_write(fake_mgr, struct.pack("<Q", fake_vt))

    # The registry replaces BOTH former per-site stubs: every inlined walk now runs for real.
    _eb = _ed.Eboot(_ed.EBOOT)
    build_registry(H, _eb, DESC_INDEX, 0x80801726 if args.request else 0x80801825)

    orig_code = H._code

    def code(uc, address, size, user):
        recent.append(address)
        if len(recent) > 12:
            recent.pop(0)
        if address == 0:
            print('DISPATCH TO 0 - preceding RIPs: ' + ' '.join(hex(x) for x in recent[:-1]))
        if address == GET_MANAGER:
            H._ret(fake_mgr)
            return
        if address == STUB_ACCESSOR:
            H._ret(fake_rec)
            return
        if address == READ_BITS:
            reads.append(("bits", uc.reg_read(UC_X86_REG_RSI) & 0xFFFFFFFF))
        elif address == READ_WIDE:
            reads.append(("wide", uc.reg_read(UC_X86_REG_RSI) & 0xFFFFFFFF))
        elif address == READ_BLOB:
            reads.append(("blob", uc.reg_read(UC_X86_REG_RDX) & 0xFFFFFFFF))
        elif address == ID_TO_INDEX:
            # Its lookup table is populated at runtime from content; in a bare harness it returns
            # -1 and the decoder bails before touching the schema. Any non-(-1) index works here
            # because the accessor is stubbed to hand back our fake record regardless.
            reads.append(("id_to_index", uc.reg_read(UC_X86_REG_RDI) & 0xFFFF))
            H._ret(0)
            return
        return orig_code(uc, address, size, user)

    # Single hook: the harness dispatches through this attribute (see its __init__).
    H._code = code

    body = bytes.fromhex(args.body) if args.body else b"\x00" * 96
    payload = struct.pack(">H", args.msgid) + body
    inp = H.alloc(len(payload) + 16)
    mu.mem_write(inp, payload)
    out = H.alloc(0x4000)
    cnt = H.alloc(8)

    print(f"tracing {which} schema, descriptor {desc:#x}, {len(payload)} payload bytes")
    try:
        rc = H.call(DECODER, inp, len(payload), direction, out, 0x4000, cnt)
        print(f"decoder returned {rc & 0xFF}")
        # ⚠ INSPECT THE DECODED C OBJECT, not just the primitive-call log. The log shows read
        # WIDTHS only - it does NOT expose codec transformations or composite control bits. Reading
        # "no presence bits" off the call log was exactly that mistake: wire nibble 0 decodes to C
        # byte 0xff, wire nibble 1 decodes to C 0x00, and wire u32 0 decodes to C 0x80000000.
        # The object is the ground truth; the call log is a hint.
        # ⚠ `out` is a HANDLE, not the object. out+0x08 holds a pointer to the decoded C struct -
        # reading `out` directly yields that pointer where the SOID should be (0x05061820, plainly a
        # heap address) and would be reported as a decoded value. Follow it.
        objp = struct.unpack_from("<Q", bytes(mu.mem_read(out, 16)), 8)[0]
        print(f"decoded object at {objp:#x} (via handle {out:#x}+8)")
        obj = bytes(mu.mem_read(objp, 0x20))
        print("decoded object +0x00..0x20: " + " ".join(f"{b:02x}" for b in obj))
        print(f"   byte +0x00 = {obj[0]:#04x}   u32 +0x04 = {struct.unpack_from('<I', obj, 4)[0]:#010x}"
              f"   u64 +0x08 = {struct.unpack_from('<Q', obj, 8)[0]:#018x}")
        print(f"   u32 +0x10 = {struct.unpack_from('<I', obj, 0x10)[0]:#010x}"
              f"   u32 +0x14 = {struct.unpack_from('<I', obj, 0x14)[0]:#010x}")
    except Exception as e:
        rip = mu.reg_read(UC_X86_REG_RIP)
        print(f"stopped: {e}   at RIP={rip:#x}  "
              f"rsi={mu.reg_read(UC_X86_REG_RSI):#x} rdi={mu.reg_read(UC_X86_REG_RDI):#x} "
              f"rcx={mu.reg_read(UC_X86_REG_RCX):#x} rdx={mu.reg_read(UC_X86_REG_RDX):#x}")

    print(f"\n{len(reads)} bitstream operation(s):")
    for i, (kind, n) in enumerate(reads):
        print(f"  {i:3d}  {kind:12s} {n}" + (f"  ({n//8} bytes)" if kind == "blob" else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
