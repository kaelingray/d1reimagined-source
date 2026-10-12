#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Decode one of the D1 eboot's table-driven PROTOBUF field descriptors.

The client has no protoc-generated code: `0xa2020 parse(stream, descriptor, out)` is one generic
reader driven by static 23-byte field-descriptor arrays.  The record layout is specified in
`historicaldocs/design/signon-protocol.md` §2.1 and is reproduced here rather than re-derived.

WHY THIS EXISTS.  `historicaldocs/design/message-catalogue.md` §3.2 says `0x0021`'s body is "a protobuf, not an
RSAT schema" and stops there, and `investment-profile-contract.md` §5.1 lists 28 descriptor sites
without decoding them.  The one question that decides whether a header-only reply is legal is
whether the descriptor contains a **required** field (cardinality nibble `0x00`), because the codec
has a `missing required field` error and a failed parse makes `decode_payload` return 0.  That is a
property of the table, so read the table.

⚠ THE `aux` TRAP (`signon-protocol.md` §2.3).  Every `aux` pointer reads as ZERO in the file image;
the real value is an `R_X86_64_RELATIVE` addend.  Walking nested messages without applying
relocations concludes -- wrongly -- that sub-messages have no sub-descriptor.  This applies them.

POSITIVE CONTROL.  `--control` re-decodes the XETROV request descriptor `0x15B3A50`, whose shape is
independently pinned by `d1_bap::parse_xetrov_request` (top: 1 varint, 2 varint, 3 varint optional,
4 nested; nested: 1 varint, 2 nested).  A run without a passing control is not evidence of absence
(`historicaldocs/design/ATLAS.md` §4.2).

    ./proto-descriptor.py --control
    ./proto-descriptor.py 0x15ab800            # 0x0021 bap_to_client_user_message_response
    ./proto-descriptor.py 0x15ab3e0 --depth 3
"""
import argparse
import struct
import sys
from pathlib import Path

EBOOT = Path(__file__).resolve().parent.parent / "dumps" / "patch-01.29-CUSA00219" / "eboot.bin"
BIAS = 0x4000
DYN = 0x1A0D100
RELA_OFF = DYN + 0xBDD0
RELA_SZ = 0x1B0918
REC = 0x17  # 23 bytes, packed

CARD = {0x00: "REQUIRED", 0x10: "optional", 0x20: "repeated", 0x30: "callback"}
FTYPE = {0: "varint", 1: "zigzag", 2: "fixed32", 3: "fixed64", 4: "bytes",
         5: "string", 6: "message", 7: "INVALID"}

_d = None
_reloc = None


def data():
    global _d
    if _d is None:
        _d = EBOOT.read_bytes()
    return _d


def reloc_map():
    """slot_vaddr -> addend, for every R_X86_64_RELATIVE entry."""
    global _reloc
    if _reloc is None:
        d = data()
        m = {}
        for p in range(RELA_OFF, RELA_OFF + RELA_SZ, 24):
            off, info, add = struct.unpack_from("<QQq", d, p)
            if (info & 0xFFFFFFFF) == 8:
                m[off] = add & 0xFFFFFFFFFFFFFFFF
        _reloc = m
    return _reloc


def fields(desc_va, limit=64):
    """Decode the 23-byte records at `desc_va` until the field_number == 0 terminator."""
    d = data()
    out = []
    for i in range(limit):
        p = desc_va + BIAS + i * REC
        if p + REC > len(d):
            break
        num, typ, pre, fdelta, size, cap = struct.unpack_from("<IBBbII", d, p)
        if num == 0:
            break
        aux_slot = desc_va + i * REC + 0x0F
        aux = reloc_map().get(aux_slot, struct.unpack_from("<Q", d, p + 0x0F)[0])
        out.append(dict(n=num, card=typ & 0xF0, ftype=typ & 0x0F, pre=pre,
                        fdelta=fdelta, size=size, cap=cap, aux=aux, slot=aux_slot))
    return out


def walk(desc_va, depth=0, maxdepth=3, seen=None):
    seen = seen if seen is not None else set()
    pad = "    " * depth
    fl = fields(desc_va)
    req = [f for f in fl if f["card"] == 0x00]
    print(f"{pad}descriptor {desc_va:#09x}: {len(fl)} field(s), "
          f"{len(req)} REQUIRED -> "
          f"{'a zero-length body CANNOT parse' if req else 'a ZERO-LENGTH body parses clean'}")
    if desc_va in seen:
        print(f"{pad}  (already expanded)")
        return
    seen.add(desc_va)
    for f in fl:
        card = CARD.get(f["card"], f"?{f['card']:#x}")
        ft = FTYPE.get(f["ftype"], f"?{f['ftype']}")
        extra = f" cap {f['cap']}" if f["card"] == 0x20 else ""
        aux = f"  aux {f['aux']:#09x}" if f["aux"] else ("  aux NULL" if f["ftype"] == 6 else "")
        print(f"{pad}  #{f['n']:<3} {card:<8} {ft:<8} size {f['size']:<5}"
              f" pre {f['pre']:<3} flagd {f['fdelta']:<4}{extra}{aux}")
        if f["ftype"] == 6 and f["aux"] and depth + 1 < maxdepth:
            walk(f["aux"], depth + 1, maxdepth, seen)


def control() -> int:
    """XETROV request descriptor `0x15B3A50` - shape pinned INDEPENDENTLY by
    `d1_bap::parse_xetrov_request`, which was written from the wire bytes, not from this table:

        #1 REQUIRED varint   -> ok_or(MissingField("top.1"))
        #2 REQUIRED varint   -> ok_or(MissingField("top.2"))
        #3 optional varint   -> optional_varint(&top, 3)?      (no ok_or)
        #4 REQUIRED bytes    -> ok_or(MissingField("top.4"))   (parsed as a nested message by hand)

    Matching all four field numbers, types AND cardinalities against a parser derived from a
    different source is what makes a `REQUIRED`/`optional` verdict elsewhere trustworthy.
    ⚠ Field 4 is `bytes` (type 4), NOT `message` (type 6) - an earlier version of this control
    asserted type 6 and failed. The table was right and the control was wrong; recorded because
    "the control failed" is not automatically "the instrument is broken".
    """
    want = [(1, 0x00, 0), (2, 0x00, 0), (3, 0x10, 0), (4, 0x00, 4)]
    fl = fields(0x15B3A50)
    got = [(f["n"], f["card"], f["ftype"]) for f in fl]
    ok = got == want
    for n, card, ft in got:
        print(f"CONTROL 0x15b3a50  #{n} {CARD.get(card, card):<8} {FTYPE.get(ft, ft)}")
    print("CONTROL:", "PASS - 4/4 vs d1_bap::parse_xetrov_request"
          if ok else f"*** FAIL *** got {got} want {want}")
    return 0 if ok else 1


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("descriptors", nargs="*", type=lambda s: int(s, 0))
    ap.add_argument("--depth", type=int, default=3)
    ap.add_argument("--control", action="store_true")
    a = ap.parse_args()

    rc = control() if a.control or not a.descriptors else 0
    if not a.descriptors:
        return rc
    # Every run carries the control, always - a descriptor walk with no control cannot
    # distinguish "no required fields" from "I walked the wrong bytes".
    if not a.control:
        rc = control()
    for d in a.descriptors:
        print()
        walk(d, maxdepth=a.depth)
    return rc


if __name__ == "__main__":
    sys.exit(main())
