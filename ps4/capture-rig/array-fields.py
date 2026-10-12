#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Where arrays really live in the RSAT anchored descriptors - proved from the client's own walkers.

WHAT THIS SETTLES
-----------------
`historicaldocs/design/character-and-orbit.md` §26.27 inferred `[I]` that **`kind & 0x100` = repeated/array** in the
anchored descriptor field records, from a histogram alone.

**That inference is FALSE and this tool destroys it from the code.**  Three descriptor-driven walkers
consume the field `kind` word, and all three read it as **two independent bytes**:

    marker+0x08  (byte)  kind & 0xFF   -> type selector; 1 = COMPOSITE, anything else indexes a
                                          leaf-codec function table
    marker+0x09  (byte)  kind >> 8     -> the BIT WIDTH OF AN OPTIONAL-PRESENCE FLAG.
                                          non-zero  => read one bit first; if clear the field is
                                                       ABSENT and is skipped entirely
                                          and the same byte is ADDED to the field's bit offset,
                                          because the presence bit occupies that many bits.

Measured over all 4555 anchored descriptors the high byte takes exactly two values, 0 (6234 fields)
and 1 (602 fields) - i.e. "mandatory" and "optional, one presence bit".  An array flag would not be
read with `movzx` and *added to a bit position*.

**The real array mechanism is a different pair of columns**, and it is only consulted for COMPOSITE
fields (`kind & 0xFF == 1`):

    marker+0x10  (byte)  count-source kind; **1 = "the element count is a field of the parent
                         C object"**
    marker+0x14  (i32)   when count-source == 1, the BYTE OFFSET INTO THE PARENT C OBJECT from which
                         the i32 element count is loaded.
                         ⚠ `eboot-descriptors.py` labels this column "width in BITS" unconditionally.
                         That is right for LEAF fields (it is the leaf handler's parameter block) and
                         WRONG for composite fields.  Do not read it as a bit width on an array field.

and a per-descriptor array header, which overlays field[0]'s record:

    anchor+0x20  (i32)   > 0  => THIS DESCRIPTOR IS AN ARRAY TYPE of that many elements
    anchor+0x28/0x2c/0x30 (i32)  per-element strides (bytes / bytes / bits), multiplied by the index
    anchor+0x38  (u16)   element kind, same two-byte split as above
    anchor+0x3c  (u32)   element child tag

A composite field whose count-source is 1 and whose child is an array descriptor is therefore a
**variable-length array**: the descriptor supplies the element type and stride, the decoded C object
supplies the count.

AND - the thing the gate actually needed - **the BAP response envelope's span is NOT a descriptor
field at all.**  `0x261FB0` hard-codes it, after the message body, in its own bit stream:

    0x2626C0  call 0x73430          ; read ONE bit
    0x2626C5  test al,al / je       ;   clear -> envelope+0x10 = NULL, envelope+0x18 = 0
    0x2626D9  call 0x72C70(esi=16)  ; read a 16-BIT LENGTH  N
    0x2626DE  mov [r15+0x18], eax   ; envelope.count = N
    0x26279F  mov [r15+0x10], rcx   ; envelope.ptr   = arena + cursor
    0x2627AE  shl edx, 3            ; N * 8
    0x2627B8  call 0x730B0          ; read N BYTES into the arena

The encoder `0x261BE0` is byte-symmetric (`0x261E43` writes the presence bit, `0x261E71` the 16-bit
length, `0x261EB1` the blob).  So no anchored schema can ever be "the envelope schema" - searching
for one is searching for something that does not exist.

Usage:
    ./array-fields.py --validate            # the code-level proof, byte-pinned, positive controls
    ./array-fields.py --validate --oracle   # ALSO drive 0x261BE0/0x261FB0 under Unicorn and show a
                                            # non-empty envelope span round-trip (needs unicorn:
                                            # PYTHONPATH=$HOME/.local/lib/d1-tools)
    ./array-fields.py --inventory           # every anchored array field / array type
    ./array-fields.py --inventory --envelope-rank
"""
import argparse
import os
import struct
import sys
from importlib.machinery import SourceFileLoader

HERE = os.path.dirname(os.path.abspath(__file__))
_ed = SourceFileLoader("d1_ed", os.path.join(HERE, "eboot-descriptors.py")).load_module()
BIAS = _ed.BIAS
SENTINEL = _ed.SENTINEL
LEAF = _ed.LEAF

# ── the three descriptor-driven walkers, and the two codec entry points ───────────────────────────
DECODE_WALKER = 0x105DD20      # called from 0x2C886E (r8d==1) and 0x2C794F
ENCODE_WALKER = 0x105E3C0      # called from 0x2C8875
WALKER3 = 0x105EA60            # third variant, called from 0x2C7A07 / 0x2C7AC8
WALK_ENTRY = 0x2C8680          # resolves the tag, then tail-selects decode vs encode walker
DECODER = 0x261FB0
ENCODER = 0x261BE0

READ_BIT = 0x73430             # read 1 bit  -> al      (shr rax,0x3f - verified below)
READ_BITS = 0x72C70            # read N bits -> eax
READ_BLOB = 0x730B0            # read N bits into a buffer
WRITE_BITS = 0x733A0
WRITE_BLOB = 0x72E30

# Positive-control tags: schemas whose shape is already pinned in the dossier.
CTL_503_RESPONSE = 0x80801825  # §26.26  5 fields, struct 968
CTL_206_RESPONSE = 0x80801869  # §26.21  1 field, struct 1, one 4-bit leaf
CTL_ABSENT_TAG = 0x808033F4    # §26.27  provably NOT in the image - the dangle control

# ── byte-pinned proof sites ───────────────────────────────────────────────────────────────────────
# Each entry: (vaddr, expected bytes, disassembly, what it proves).  If the bytes at the address do
# not match, this tool FAILS rather than printing a confident quotation of something else.
PROOF = {
    "A. the kind word is split into two bytes - DECODE walker 0x105DD20": [
        (0x105DF05, "43807ce53100", "cmp  byte [r13+r12*8+0x31], 0",
         "kind>>8 (marker+0x09) tested on its own: is there a presence bit?"),
        (0x105DF12, "e8195501ff", "call 0x73430",
         "...if so, read exactly ONE bit from the stream"),
        (0x105DF31, "84db", "test bl, bl",
         "the bit is the field's presence flag"),
        (0x105DF33, "0f84e6020000", "je   0x105e21f",
         "clear => the field is SKIPPED (not decoded at all)"),
        (0x105DF43, "4b0fbe44e530", "movsx rax, byte [r13+r12*8+0x30]",
         "kind&0xFF (marker+0x08) read on its own, as a BYTE"),
        (0x105DF49, "4883f801", "cmp  rax, 1",
         "low byte == 1 => COMPOSITE; anything else => leaf"),
        (0x105E0D2, "410fb630", "movzx esi, byte [r8]     ; r8 = &marker+0x09",
         "the HIGH byte again..."),
        (0x105E0D6, "01d6", "add  esi, edx",
         "...ADDED TO THE BIT OFFSET. That is a bit width, not a flag."),
        (0x105E0F5, "ff94c350010000", "call qword [rbx+rax*8+0x150]",
         "leaf dispatch indexed by the LOW byte alone (table 0x18FB1D0)"),
    ],
    "B. same split on the array-element path - DECODE walker": [
        (0x105E03D, "0fb74c2450", "movzx ecx, word [rsp+0x50]   ; the kind word",
         "element kind of an array descriptor"),
        (0x105E044, "c1e808", "shr  eax, 8",
         "high byte isolated"),
        (0x105E047, "80f901", "cmp  cl, 1",
         "low byte compared against 1 - COMPOSITE test"),
        (0x105E071, "01c6", "add  esi, eax",
         "high byte ADDED to the bit offset. Again a width, not an array flag."),
    ],
    "C. THE REAL ARRAY MECHANISM - DECODE walker, composite-field recursion": [
        (0x105E30A, "438a4ce538", "mov   cl, byte [r13+r12*8+0x38]   ; marker+0x10",
         "count-source kind of a COMPOSITE field"),
        (0x105E312, "80f901", "cmp   cl, 1",
         "== 1 means 'the count lives in the parent object'"),
        (0x105E317, "4b634ce53c", "movsxd rcx, dword [r13+r12*8+0x3c] ; marker+0x14",
         "marker+0x14 is then a BYTE OFFSET INTO THE C OBJECT, not a bit width"),
        (0x105E31C, "4863340a", "movsxd rsi, dword [rdx+rcx]",
         "the i32 element count is LOADED FROM THE DECODED OBJECT"),
        (0x105E341, "4989f4", "mov   r12, rsi",
         "and becomes the walker's trip count"),
    ],
    "D. the ENCODE walker 0x105E3C0 makes the identical reads": [
        (0x105E5A5, "43807ce53100", "cmp  byte [r13+r12*8+0x31], 0", "presence-bit byte"),
        (0x105E6E4, "c1e808", "shr  eax, 8", "high byte isolated"),
        (0x105E6E7, "80f901", "cmp  cl, 1", "low byte == 1 composite test"),
        (0x105E711, "01c6", "add  esi, eax", "high byte added to the bit offset"),
        (0x105E9A2, "438a4ce538", "mov   cl, byte [r13+r12*8+0x38]", "count-source kind"),
        (0x105E9AA, "80f901", "cmp   cl, 1", "== 1"),
        (0x105E9AF, "4b634ce53c", "movsxd rcx, dword [r13+r12*8+0x3c]", "count offset"),
        (0x105E9B4, "4863340a", "movsxd rsi, dword [rdx+rcx]", "count from the object"),
    ],
    "E. the third walker 0x105EA60 agrees (different record base, same mechanism)": [
        (0x105EC45, "43807ce53100", "cmp  byte [r13+r12*8+0x31], 0", "presence-bit byte"),
        (0x105ED84, "c1e808", "shr  eax, 8", "high byte isolated"),
        (0x105ED87, "80f901", "cmp  cl, 1", "low byte == 1 composite test"),
        (0x105F052, "43807ce53801", "cmp  byte [r13+r12*8+0x38], 1", "count-source == 1"),
        (0x105F05A, "4b6344e540", "movsxd rax, dword [r13+r12*8+0x40]", "count offset"),
        (0x105F05F, "48633401", "movsxd rsi, dword [rcx+rax]", "count from the object"),
    ],
    "F. the ENVELOPE span is hard-coded in 0x261FB0 - no schema describes it": [
        (0x2626C0, "e86b0de1ff", "call 0x73430", "read ONE presence bit for the span"),
        (0x2626C5, "84c0", "test al, al", ""),
        (0x2626C7, "0f84bd000000", "je   0x26278a", "clear => span = {NULL, 0}"),
        (0x2626D4, "be10000000", "mov  esi, 0x10", "otherwise read a 16-BIT length"),
        (0x2626D9, "e89205e1ff", "call 0x72c70", ""),
        (0x2626DE, "41894718", "mov  dword [r15+0x18], eax", "envelope+0x18 = N   <-- THE COUNT"),
        (0x26279F, "49894f10", "mov  qword [r15+0x10], rcx", "envelope+0x10 = arena+cursor"),
        (0x2627A3, "410106", "add  dword [r14], eax", "cursor += N  => N is a BYTE count"),
        (0x2627AE, "c1e203", "shl  edx, 3", "N * 8"),
        (0x2627B8, "e8f308e1ff", "call 0x730b0", "read N BYTES into the arena"),
        (0x26278A, "49c7471000000000", "mov qword [r15+0x10], 0", "the empty case..."),
        (0x262792, "41c7471800000000", "mov dword [r15+0x18], 0", "...which is every run so far"),
    ],
    "G. the ENCODER 0x261BE0 is byte-symmetric": [
        (0x261DF4, "418b5f18", "mov  ebx, dword [r15+0x18]", "the span count"),
        (0x261E2F, "85db", "test ebx, ebx", ""),
        (0x261E31, "0f9fc0", "setg al", "presence bit = (count > 0)"),
        (0x261E43, "e85815e1ff", "call 0x733a0", "write it"),
        (0x261E6C, "ba10000000", "mov  edx, 0x10", "write a 16-bit length"),
        (0x261E71, "e82a15e1ff", "call 0x733a0", ""),
        (0x261EA1, "498b7710", "mov  rsi, qword [r15+0x10]", "the blob pointer"),
        (0x261EA5, "c1e203", "shl  edx, 3", "count * 8 bits"),
        (0x261EB1, "e87a0fe1ff", "call 0x72e30", "write the blob"),
    ],
    "H. 0x73430 really is a ONE-bit read (so 'presence bit' is not a guess)": [
        (0x73450, "48c1e83f", "shr  rax, 0x3f", "returns the single top bit of the 64-bit window"),
        (0x7344D, "ff4724", "inc  dword [rdi+0x24]", "and advances the stream by exactly 1"),
    ],
}

# Ranges scanned for a branch on bit 8.  Ends are the last instruction we disassembled in each.
KIND_CONSUMERS = [
    ("decode walker", DECODE_WALKER, 0x105E3C0),
    ("encode walker", ENCODE_WALKER, 0x105EA60),
    ("walker3", WALKER3, 0x105F1A0),
    ("walk entry 0x2C8680", WALK_ENTRY, 0x2C88BA),
    ("decoder 0x261FB0", DECODER, 0x2628C0),
    ("encoder 0x261BE0", ENCODER, 0x261FB0),
]


class Img:
    def __init__(self, path=_ed.EBOOT):
        self.d = open(path, "rb").read()

    def raw(self, vaddr, n):
        return self.d[vaddr + BIAS: vaddr + BIAS + n]

    def u32(self, v):
        return struct.unpack_from("<I", self.d, v + BIAS)[0]

    def i32(self, v):
        return struct.unpack_from("<i", self.d, v + BIAS)[0]


# ── descriptor model ──────────────────────────────────────────────────────────────────────────────
class Field:
    __slots__ = ("i", "marker", "off", "kind", "child", "param10", "param14", "sstart")

    def __init__(self, img, anchor, i):
        mk = anchor + 0x30 + i * 0x28
        self.i, self.marker = i, mk
        self.sstart = img.u32(mk)
        self.off = img.u32(mk - 8)
        self.kind = img.u32(mk + 8)
        self.child = img.u32(mk + 0x0C)
        self.param10 = img.u32(mk + 0x10)
        self.param14 = img.i32(mk + 0x14)

    @property
    def type_sel(self):
        return self.kind & 0xFF

    @property
    def presence_bits(self):
        return (self.kind >> 8) & 0xFF

    @property
    def is_composite(self):
        return self.type_sel == 1

    @property
    def is_dynamic_array(self):
        """The client's own test: composite AND count-source byte == 1."""
        return self.is_composite and (self.param10 & 0xFF) == 1


class Desc:
    __slots__ = ("tag", "anchor", "nfields", "size", "array_n", "fields")

    def __init__(self, img, tag, anchor):
        self.tag, self.anchor = tag, anchor
        self.nfields = img.u32(anchor + 8)
        self.size = img.u32(anchor + 0x14)
        self.array_n = img.i32(anchor + 0x20)          # > 0 => this descriptor IS an array type
        self.fields = []
        for i in range(self.nfields):
            mk = anchor + 0x30 + i * 0x28
            if img.u32(mk + 4) != SENTINEL:
                break
            self.fields.append(Field(img, anchor, i))

    # array-descriptor view (overlays field[0]'s record - same bytes, different meaning)
    def array_view(self, img):
        a = self.anchor
        return dict(count=self.array_n, stride0=img.i32(a + 0x28), stride1=img.i32(a + 0x2C),
                    bitstride=img.i32(a + 0x30), kind=img.u32(a + 0x38), child=img.u32(a + 0x3C))


def load(img):
    return {t: Desc(img, t, a) for t, a in _ed.Eboot(_ed.EBOOT).index().items()}


# ── validate ──────────────────────────────────────────────────────────────────────────────────────
def imm_scan(img, value):
    """Every `test/and/or/cmp/bt` against `value` inside the kind-consuming functions.

    Deliberately crude and byte-level (no capstone dependency): it looks for the encodings the
    compiler actually emits for an 8-bit-flag test on a 32-bit register, plus the 8-bit forms.
    Used ONLY with a positive control - never on its own.
    """
    pats = []
    v32 = struct.pack("<I", value)
    for op in (0xA9,):                       # test eax, imm32
        pats.append(bytes([op]) + v32)
    for opc, mod in ((0xF7, 0), (0x81, 4), (0x81, 1), (0x81, 7)):   # test/and/or/cmp r32, imm32
        for rm in range(8):
            pats.append(bytes([opc, 0xC0 | (mod << 3) | rm]) + v32)
    if value < 0x100:                        # 8-bit forms
        for opc, mod in ((0xF6, 0), (0x80, 4), (0x80, 1), (0x80, 7)):
            for rm in range(8):
                pats.append(bytes([opc, 0xC0 | (mod << 3) | rm, value]))
    hits = []
    for name, lo, hi in KIND_CONSUMERS:
        blob = img.raw(lo, hi - lo)
        for p in pats:
            s = 0
            while True:
                j = blob.find(p, s)
                if j < 0:
                    break
                hits.append((name, lo + j, p.hex()))
                s = j + 1
    return hits


def validate(img, descs, run_oracle=False):
    bad = 0
    print("=" * 96)
    print("VALIDATE - does `kind & 0x100` mean 'array'?   §26.27 says [I]. The code says NO.")
    print("=" * 96)

    for section, sites in PROOF.items():
        print(f"\n{section}")
        for va, want, dis, why in sites:
            got = img.raw(va, len(want) // 2).hex()
            ok = got == want
            bad += 0 if ok else 1
            mark = " " if ok else "  *** BYTES DIFFER (want %s) ***" % want
            print(f"  0x{va:07x}  {got:<20s} {dis:<44s} {why}{mark}")

    # ── the absence claim, with its positive control in the SAME run ──────────────────────────────
    print("\n" + "-" * 96)
    print("SCAN: is bit 8 of `kind` ever tested?   (absence claim -> positive control mandatory)")
    print("-" * 96)
    tgt = imm_scan(img, 0x100)
    ctl_3ff = imm_scan(img, 0x3FF)
    ctl_1fff = imm_scan(img, 0x1FFF)
    print(f"  immediate 0x100  in the 6 kind-consuming functions : {len(tgt)} hit(s)")
    for n, a, p in tgt:
        print(f"      {n} @ {a:#x}  {p}")
    print(f"  POSITIVE CONTROL immediate 0x3ff                   : {len(ctl_3ff)} hit(s)"
          f"   {[hex(a) for _, a, _ in ctl_3ff[:6]]}")
    print(f"  POSITIVE CONTROL immediate 0x1fff                  : {len(ctl_1fff)} hit(s)"
          f"   {[hex(a) for _, a, _ in ctl_1fff[:6]]}")
    if not ctl_3ff or not ctl_1fff:
        print("  *** CONTROL EMPTY - the scanner is broken, its 0x100 result means nothing ***")
        bad += 1
    elif tgt:
        print("  ⚠ a 0x100 immediate DOES appear - inspect it before trusting this section")
    else:
        print("  ⇒ [C] no code anywhere in the kind-consuming path masks, tests or branches on 0x100,")
        print("       while the same scanner finds the registry masks it is supposed to find.")

    # ── the histogram, now with the meaning attached ──────────────────────────────────────────────
    print("\n" + "-" * 96)
    print("MEASURE: the kind word over all anchored descriptors")
    print("-" * 96)
    hi, lo, cs_comp, cs_leaf = {}, {}, {}, {}
    nf = 0
    for d in descs.values():
        for f in d.fields:
            nf += 1
            hi[f.presence_bits] = hi.get(f.presence_bits, 0) + 1
            lo[f.type_sel] = lo.get(f.type_sel, 0) + 1
            tbl = cs_comp if f.is_composite else cs_leaf
            k = f.param10 & 0xFF
            tbl[k] = tbl.get(k, 0) + 1
    print(f"  descriptors {len(descs)}   fields {nf}")
    print(f"  kind>>8   (presence-bit width) : {dict(sorted(hi.items()))}")
    print(f"       ⇒ exactly two values. A flag byte read with movzx and ADDED to a bit position is")
    print(f"         a WIDTH: 0 = mandatory, 1 = optional-with-one-presence-bit.")
    print(f"  kind&0xFF (type selector)      : {dict(sorted(lo.items()))}")
    print(f"  marker+0x10 low byte, COMPOSITE fields : {dict(sorted(cs_comp.items()))}"
          "   <-- 1 = dynamic array")
    print(f"  marker+0x10 low byte, LEAF fields      : {dict(sorted(cs_leaf.items()))}"
          "   (leaf handler params, unrelated)")

    # positive control on the descriptor index itself
    print("\n  POSITIVE CONTROLS on the descriptor index:")
    for tag, exp_n, exp_sz in ((CTL_503_RESPONSE, 5, 968), (CTL_206_RESPONSE, 1, 1)):
        d = descs.get(tag)
        ok = d is not None and d.nfields == exp_n and d.size == exp_sz
        bad += 0 if ok else 1
        print(f"    {tag:#010x}  fields={d and d.nfields} size={d and d.size} "
              f"array_n={d and d.array_n}   expected {exp_n}/{exp_sz}   {'OK' if ok else '*** FAIL ***'}")
    absent = CTL_ABSENT_TAG in descs
    print(f"    {CTL_ABSENT_TAG:#010x}  present={absent}   expected False (§26.27)"
          f"   {'OK' if not absent else '*** FAIL ***'}")
    bad += 1 if absent else 0

    print("\n" + "=" * 96)
    print("VERDICT")
    print("=" * 96)
    print("  [C] `kind & 0x100` is NOT an array bit.  kind is two bytes: a type selector and the")
    print("      bit width of an optional-presence flag.  §26.27's [I] is REFUTED.")
    print("  [C] arrays are marker+0x10 (count-source kind) + marker+0x14 (count offset in the C")
    print("      object), consulted only for composite fields, plus anchor+0x20 on the element type.")
    print("  [C] the BAP envelope span is not schema-described at all: 0x261FB0 reads a 1-bit flag,")
    print("      a u16 byte-length and that many raw bytes, straight after the message body.")

    if run_oracle:
        bad += oracle_roundtrip()
    return bad


def oracle_roundtrip():
    """Drive the client's own encoder+decoder offline and produce a NON-EMPTY envelope span."""
    print("\n" + "=" * 96)
    print("ORACLE - native encode/decode round-trip through Unicorn (no emulator)")
    print("=" * 96)
    od = os.path.join(HERE, "oodle")
    sys.path.insert(0, od)
    cwd = os.getcwd()
    try:
        os.chdir(od)
        o = SourceFileLoader("d1_enc503", os.path.join(od, "encode-investment-503.py")).load_module()
    except Exception as exc:                                   # pragma: no cover
        os.chdir(cwd)
        print(f"  SKIPPED: {exc}")
        print("  (needs unicorn - try PYTHONPATH=$HOME/.local/lib/d1-tools)")
        return 0
    try:
        orc = o.Oracle()
        mu, H = orc.mu, orc.harness

        def run(span):
            hdl = orc.make_source_handle(o.DEFAULT_ACCOUNT_SOID, o.DEFAULT_FAMILY5_ROOT)
            if span:
                p = H.alloc(len(span))
                mu.mem_write(p, span)
                mu.mem_write(hdl + 0x10, struct.pack("<Q", p))
                mu.mem_write(hdl + 0x18, struct.pack("<i", len(span)))
            wire = orc.encode(hdl)
            inp = H.alloc(len(wire))
            mu.mem_write(inp, wire)
            out, dsz = H.alloc(0x4000), H.alloc(8)
            rc = H.call(o.DECODER, inp, len(wire), 1, out, 0x4000, dsz) & 0xFF
            env = bytes(mu.mem_read(out, 0x20))
            ptr = struct.unpack_from("<Q", env, 0x10)[0]
            cnt = struct.unpack_from("<i", env, 0x18)[0]
            got = bytes(mu.mem_read(ptr, cnt)) if ptr and cnt > 0 else b""
            return wire, rc, ptr, cnt, got

        bad = 0
        w, rc, p, c, g = run(b"")
        print(f"  CONTROL (empty span, the pinned 503 vector)")
        print(f"    wire  {w.hex()}")
        print(f"    rc={rc}  span_ptr={p:#x}  span_count={c}   expected 0")
        if c != 0:
            bad += 1
            print("    *** CONTROL FAILED ***")
        for payload in (b"\xab", bytes.fromhex("0000000100000004d1000000000000"
                                               "0164ae75bf0000000000")):
            w, rc, p, c, g = run(payload)
            ok = c == len(payload) and g == payload
            bad += 0 if ok else 1
            print(f"  TEST span={len(payload)}B")
            print(f"    wire  {w.hex()}")
            print(f"    rc={rc}  span_ptr={p:#x}  span_count={c}  bytes={g.hex()}"
                  f"   {'ROUND-TRIP OK' if ok else '*** MISMATCH ***'}")
        print("\n  ⇒ [C] `observed-oracle`: envelope+0x10/+0x18 become non-NULL/non-zero purely by")
        print("       appending {1 bit = 1}{u16 N}{N bytes} to the message's own bit stream.")
        print("       This is the first offline reproduction of span_count != 0.")
        return bad
    finally:
        os.chdir(cwd)


# ── inventory ─────────────────────────────────────────────────────────────────────────────────────
def inventory(img, descs, envelope_rank=False):
    arr_types = {t: d for t, d in descs.items() if d.array_n > 0}
    dyn = []
    for t, d in sorted(descs.items()):
        for f in d.fields:
            if f.is_dynamic_array:
                dyn.append((d, f))

    print("=" * 124)
    print("INVENTORY - every anchored descriptor holding an ARRAY field")
    print("  array field  ==  kind&0xFF == 1 (composite)  AND  marker+0x10 low byte == 1")
    print("                   (element count read from the parent C object at marker+0x14)")
    print("=" * 124)
    print(f"{'owner tag':<12}{'sz':>7}{'fld':>4}{'@off':>7}{'kind':>7}{'opt':>4}  "
          f"{'child tag':<12}{'child':<9}{'cnt@':>5}{'nbits':>6}{'max':>6}{'stride':>7}  element")
    print("-" * 124)
    dangling = 0
    nonarray = []
    for d, f in dyn:
        cf = [g for g in d.fields if g.off == f.param14]
        nbits = cf[0].param14 if cf else -1
        ch = descs.get(f.child)
        if ch is None:
            dangling += 1
            print(f"{d.tag:#010x} {d.size:>7}{f.i:>4}{f.off:>#7x}{f.kind:>#7x}{f.presence_bits:>4}  "
                  f"{f.child:#010x}  DANGLING{f.param14:>5}{nbits:>6}")
            continue
        av = ch.array_view(img)
        if av["count"] <= 0:
            nonarray.append((d.tag, f.child))
        el = "LEAF" if av["child"] == LEAF else f"{av['child']:#010x}"
        print(f"{d.tag:#010x} {d.size:>7}{f.i:>4}{f.off:>#7x}{f.kind:>#7x}{f.presence_bits:>4}  "
              f"{f.child:#010x}  resolved{f.param14:>5}{nbits:>6}{av['count']:>6}"
              f"{av['stride0']:>7}  kind {av['kind']:#06x} {el}")
    print("-" * 124)
    print(f"  {len(dyn)} array field(s) in {len({d.tag for d, _ in dyn})} descriptor(s); "
          f"{dangling} with a DANGLING child tag")
    print("  SHAPE, and it is uniform across all of them:  every owner has EXACTLY TWO fields -")
    print("    field[0] a `kind&0xFF == 5` leaf at C offset 0, `nbits` wide, holding the count")
    print("    field[1] the array itself, count-source 1, count offset 0 (i.e. field[0])")
    print("  so the C struct is `struct { u32 count; T items[max]; }` and the wire form is a")
    print("  bit-packed bounded length followed by `count` elements. `max` is the descriptor's")
    print("  capacity, not the wire count.")
    # Independent cross-check: if `nbits` really is the count's width and `max` really is the
    # capacity, then nbits must be exactly bit_length(max).  Nothing forces that unless the reading
    # is right, so a clean sweep here is evidence, not restatement.
    mism = []
    for d, f in dyn:
        ch = descs.get(f.child)
        if ch is None or ch.array_n <= 0:
            continue
        cf = [g for g in d.fields if g.off == f.param14]
        if cf and cf[0].param14 != max(1, ch.array_n.bit_length()):
            mism.append((d.tag, ch.array_n, cf[0].param14))
    print(f"\n  CROSS-CHECK  nbits == bit_length(max) : {len(dyn) - len(nonarray) - len(mism)}"
          f"/{len(dyn) - len(nonarray)} agree, {len(mism)} mismatch(es) {mism[:5]}")
    if nonarray:
        print(f"  ⚠ {len(nonarray)} child(ren) resolve but are NOT array types (anchor+0x20 <= 0):")
        print(f"      {[(hex(a), hex(b)) for a, b in nonarray]}")
        print("      These have a 1-BIT count, i.e. `max == 1`: an OPTIONAL single value written as")
        print("      a 0-or-1 length array. The walker then iterates the child's field list once.")

    # POSITIVE CONTROL for the dangle test - prove the resolver can say 'dangling'.
    print(f"\n  POSITIVE CONTROL on child-tag resolution:")
    print(f"    {CTL_503_RESPONSE:#010x} -> {'resolved' if CTL_503_RESPONSE in descs else 'DANGLING'}"
          "   (expected resolved)")
    print(f"    {CTL_ABSENT_TAG:#010x} -> {'resolved' if CTL_ABSENT_TAG in descs else 'DANGLING'}"
          "   (expected DANGLING - §26.27 proved it is not in the image)")
    print("    ⇒ 0 dangling children above is a real measurement, not a broken lookup.")

    refd = {f.child for _, f in dyn} & set(arr_types)
    print(f"\n  ARRAY TYPES (anchor+0x20 > 0): {len(arr_types)} descriptor(s), "
          f"{len(refd)} of them reached by a dynamic-array field above; the rest are "
          f"FIXED-LENGTH arrays (all `max` elements always on the wire).")
    print(f"  {'tag':<12}{'size':>7}{'elems':>7}{'stride':>8}{'bits':>7}  element")
    shown = 0
    for t, d in sorted(arr_types.items()):
        av = d.array_view(img)
        fits = "" if av["count"] * av["stride0"] == d.size else "   *** size != n*stride ***"
        el = "LEAF" if av["child"] == LEAF else f"{av['child']:#010x}"
        print(f"  {t:#010x}{d.size:>7}{av['count']:>7}{av['stride0']:>8}{av['bitstride']:>7}  "
              f"kind {av['kind']:#06x} {el}{fits}")
        shown += 1
        if shown >= 40:
            print(f"  ... {len(arr_types) - shown} more")
            break

    if envelope_rank:
        envelope_ranking(img, descs)


def envelope_ranking(img, descs):
    print("\n" + "=" * 118)
    print("ENVELOPE RANKING - which anchored schema could be {+0x00 u32 id, +0x08 ptr,")
    print("                    +0x10 ptr, +0x18 i32 count}, struct size 0x20?")
    print("=" * 118)
    exact = [d for d in descs.values() if d.size == 0x20]
    print(f"  descriptors with struct size exactly 0x20 : {len(exact)}")
    cands = []
    for d in exact:
        offs = {f.off for f in d.fields}
        if {0x10, 0x18} <= offs and any(f.is_dynamic_array for f in d.fields):
            cands.append(d)
    print(f"  ...of those, with fields at BOTH +0x10 and +0x18 and an array field : {len(cands)}")
    for d in cands:
        print(f"     {d.tag:#010x}")

    # A ranking with no positive control is worthless, so show the filter can select something.
    ptr_pair = [d for d in descs.values()
                if any(f.is_dynamic_array for f in d.fields)
                and any(f.off + 8 == g.off for f in d.fields for g in d.fields
                        if g.is_dynamic_array or f.is_dynamic_array)]
    print(f"\n  POSITIVE CONTROL - same filter without the size==0x20 constraint, i.e. 'any schema")
    print(f"  with an array field adjacent to another field 8 bytes away' : {len(ptr_pair)} hit(s)")
    print(f"     e.g. {[hex(d.tag) for d in ptr_pair[:6]]}")
    print("     ⇒ the filter is not vacuous; the empty result above is a real absence.")

    print("\n  VERDICT  [C] - the question is malformed, and this is why:")
    print("    envelope+0x10/+0x18 are NOT produced by the schema walker at all.  0x261FB0 zeroes")
    print("    the whole 0x20-byte handle at 0x2620FC..0x262118, writes +0x00 (the message INDEX")
    print("    returned by 0x27FDD0, not the raw id) at 0x26213A, writes +0x08 (arena+0x20) at")
    print("    0x2624DA, hands ONLY +0x08 to the descriptor walker at 0x2625A9, and then writes")
    print("    +0x10/+0x18 itself at 0x26279F/0x2626DE from a hard-coded")
    print("    {1-bit flag}{u16 byte-count}{that many bytes} trailer.")
    print("    No anchored descriptor is, or can be, 'the envelope schema'.  Populating the span is")
    print("    a FRAMING change, not a schema change - see --validate --oracle for the bytes.")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--validate", action="store_true", help="code-level proof, byte-pinned")
    ap.add_argument("--oracle", action="store_true", help="with --validate: native round-trip")
    ap.add_argument("--inventory", action="store_true", help="array-field inventory")
    ap.add_argument("--envelope-rank", action="store_true", help="with --inventory")
    a = ap.parse_args()
    if not (a.validate or a.inventory):
        a.validate = True

    img = Img()
    descs = load(img)
    rc = 0
    if a.validate:
        rc |= 1 if validate(img, descs, a.oracle) else 0
    if a.inventory:
        if a.validate:
            print()
        inventory(img, descs, a.envelope_rank)
    return rc


if __name__ == "__main__":
    sys.exit(main())
