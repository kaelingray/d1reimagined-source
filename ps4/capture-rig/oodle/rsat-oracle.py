#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Generalised offline oracle for Destiny's RSAT message codecs.

This drives the client's OWN encoder (`0x261BE0`) and decoder (`0x261FB0`) under Unicorn, for ANY
message id / schema tag pair, and exposes the FULL 0x20-byte decoded envelope rather than only the
object pointer.  It never launches shadPS4 and never touches dumps/ or runs/.

It generalises `encode-investment-503.py`, which hardcoded message 503 and threw away exactly the
envelope fields this gate turns on.

WHAT THE ENVELOPE ACTUALLY IS  (read out of 0x261FB0/0x261BE0, see §"WIRE FORMAT" below)
----------------------------------------------------------------------------------------
    +0x00  u16   definition INDEX  -- the return of 0x27FDD0(message_id).  *** NOT the message id
                                      and NOT a BAP message class. ***  The harness stubs 0x27FDD0
                                      so this reads back as whatever `index` you pass (default 0);
                                      that is why the offline envelope showed 0 where the live one
                                      showed 0x4C / 0x3B.  Those live values are definition indices.
    +0x08  ptr   decoded object    -- arena + aligned offset (alignment from the descriptor)
    +0x10  ptr   span bytes        -- into the same arena, or NULL
    +0x18  u32   span LENGTH IN BYTES  (not an element count)

WIRE FORMAT of an RSAT message, read from both directions of the client's code  `[C]`
-------------------------------------------------------------------------------------
    u16   message id                       decode 0x26212E read16 / encode 0x261C93 write16
    ...   object bits, per schema tag       0x2C9000 decode / 0x2C8FF0 encode
    1 bit span-present flag                decode 0x2626C0 (0x73430 = read one bit)
                                           encode 0x261E11 `test ebx,ebx / setg` on handle+0x18
    if the flag is set:
      u16 span length in BYTES             decode 0x2626D9 / encode 0x261E59
      that many raw bytes                  decode 0x730B0 (len*8 bits) / encode 0x72E30

    So the span is INSIDE the RSAT bitstream, immediately after the object's bits, gated by one
    bit.  Appending whole bytes after a byte-aligned body cannot ever set that bit -- which is why
    every "tail" experiment (character-and-orbit.md §26.22 test 5, §26.24) read back as empty.

    The span is WRAPPER-LEVEL and SCHEMA-INDEPENDENT: nothing in 0x261FB0 writes envelope+0x10 or
    +0x18 except this one code path (enumerate every r15 reference in 0x261FB0..0x2628A1 to check),
    and the object decoder 0x2C9000 never sees the envelope.  So a schema does NOT need an array
    field to produce a non-empty span, and a sweep of response tags looking for one answers a
    question that has no bearing on the span.

    Three states are distinguishable at envelope+0x10/+0x18, and the client can tell them apart:
        flag 0                 -> { NULL,     0 }      (0x26278A)
        flag 1, length 0       -> { non-NULL, 0 }      (0x26279C, pointer still assigned)
        flag 1, length N       -> { ptr,      N }
    The native encoder collapses length 0 back to flag 0 (`setg` on handle+0x18 at 0x261E11), so it
    never emits the middle state -- but the decoder accepts it.

USAGE
-----
    ./rsat-oracle.py --self-test
    ./rsat-oracle.py --schema 0x80801869 --msgid 206 --body 10
    ./rsat-oracle.py --schema 0x80801869 --msgid 206 --encode --span 0000000100000004d1...

The self-test is the POSITIVE CONTROL.  Every other mode runs it first unless you pass
--no-control, because a sweep that reports span_count == 0 everywhere is indistinguishable from a
broken harness -- a class of false negative this project has already been burned by twice
(character-and-orbit.md §26.10, status/LESSONS.md).

HARNESS TRAPS (all previously paid for; do not re-discover them)
---------------------------------------------------------------
  * The Harness owns ONE late-bound code hook, dispatched through the `_code` ATTRIBUTE.  Adding a
    second Unicorn hook double-executes import stubs and pops return addresses twice, producing a
    garbage RIP that looks like a null import.  We chain through the existing hook.
  * AVX is unavailable in this Unicorn build and UC_X86_REG_XCR0 does not exist.  The harness has a
    fail-closed shim; do not "fix" it into silently skipping instructions.
  * The package registry must cover the TRANSITIVE CLOSURE of every tag the decode touches, not
    just the root, or a nested resolve faults.  We build the union of the request and response
    closures once, through trace-investment-schema.py's own fail-closed builder.
"""

import argparse
import struct
import sys
from importlib.machinery import SourceFileLoader
from pathlib import Path

HERE = Path(__file__).resolve().parent
trace = SourceFileLoader(
    "d1_investment_trace", str(HERE / "trace-investment-schema.py")
).load_module()

ENCODER = 0x261BE0
DECODER = trace.DECODER
ENVELOPE_SIZE = 0x20          # decoder zeroes exactly this much at 0x262100..0x262118
MIN_CAPACITY = 0x20           # 0x262063 `cmp ebx,0x1f / jg` -- capacity must exceed 0x1F

DIR_REQUEST = 0
DIR_RESPONSE = 1

# ---- the pinned 503 vector.  Drift here means the harness or the descriptors moved. --------------
CONTROL_MESSAGE_ID = 503
CONTROL_REQUEST_TAG = 0x80801726
CONTROL_RESPONSE_TAG = 0x80801825
CONTROL_ACCOUNT_SOID = 0xD100000000000001
CONTROL_FAMILY5_ROOT = 0x7FFFFFFFFFFFFFFF
CONTROL_BODY_HEX = (
    "180000000d1000000000000010000000000000000bfffffffffffffffc"
    "00000000000000020810220000000100"
)


class DescriptorError(RuntimeError):
    pass


def _eboot():
    return trace._ed.Eboot(trace._ed.EBOOT)


def descriptor_info(tag, eb=None, idx=None):
    """(struct_size, alignment, anchor, registry_object) for a schema tag, read from the eboot.

    struct_size is the descriptor's own `anchor+0x14`, as the brief specifies.  It is cross-checked
    against `registry_object+0x14`, which is the field the CLIENT actually reads at 0x262426 to size
    its arena allocation -- if those two ever disagree the size we hand out is a fiction and we say
    so rather than continuing.  Alignment is `registry_object+0x18` (read at 0x2622F0).
    """
    eb = eb or _eboot()
    idx = idx if idx is not None else trace.DESC_INDEX
    anchor = idx.get(tag)
    if anchor is None:
        raise DescriptorError(f"tag {tag:#010x} has no anchored descriptor in the eboot")
    obj = trace.object_start(eb, anchor)
    if obj is None:
        raise DescriptorError(
            f"tag {tag:#010x}: registry-object start not uniquely recoverable from anchor {anchor:#x}"
        )
    size_anchor = eb.u32(anchor + 0x14)
    size_client = eb.u32(obj + 0x14)
    if size_anchor != size_client:
        raise DescriptorError(
            f"tag {tag:#010x}: anchor+0x14 says {size_anchor} but the client reads "
            f"registry_object+0x14 = {size_client}; the descriptor model is wrong for this tag"
        )
    return size_anchor, eb.u32(obj + 0x18) & 0xFF, anchor, obj


def missing_children(tag, eb=None, idx=None):
    """Child tags reachable from `tag` that have no descriptor in the eboot.

    These are the tags that would resolve to NULL in the fail-closed registry and fault mid-decode,
    so a caller can tell "this schema is not offline-decodable" from "the harness is broken".
    """
    eb = eb or _eboot()
    idx = idx if idx is not None else trace.DESC_INDEX
    missing, seen, todo = set(), set(), [tag]
    while todo:
        t = todo.pop()
        if t in seen:
            continue
        seen.add(t)
        anchor = idx.get(t)
        if anchor is None:
            missing.add(t)
            continue
        for i in range(eb.u32(anchor + 8)):
            mk = anchor + 0x30 + i * 0x28
            if eb.u32(mk + 4) != trace._ed.SENTINEL:
                break
            child = eb.u32(mk + 0x0C)
            if child != trace._ed.LEAF:
                todo.append(child)
    return sorted(missing)


def _build_registry_for(harness, eb, roots, log=False):
    """Install ONE fail-closed registry covering the union of several closures.

    trace.build_registry() takes a single root and rewrites the registry global, so calling it twice
    silently discards the first root's tags.  Rather than copy its construction (the fail-closed
    entry encoding is the subtle part and must not diverge), we hand it the union by substituting
    the `closure` it looks up in its own module namespace, then restore.
    """
    roots = [r for r in roots if r is not None]
    if not roots:
        raise RuntimeError("no schema roots to build a registry for")
    union = set()
    for r in roots:
        union |= trace.closure(eb, trace.DESC_INDEX, r)
    original = trace.closure
    try:
        trace.closure = lambda _eb, _idx, _root: set(union)
        trace.build_registry(harness, eb, trace.DESC_INDEX, roots[0], log=log)
    finally:
        trace.closure = original
    return union


class Oracle:
    """Native encode/decode for one (message id, request tag, response tag) triple.

    One Oracle owns one Harness (one Unicorn instance, one eboot image, one registry).  Construction
    costs ~0.15 s; for a sweep build one Oracle per schema rather than trying to retarget, because
    the harness heap is a bump allocator and is never reclaimed.
    """

    def __init__(self, message_id, request_tag=None, response_tag=None,
                 definition_index=0, strict_message_id=True, log=False):
        if request_tag is None and response_tag is None:
            raise ValueError("at least one of request_tag / response_tag is required")
        self.message_id = message_id & 0xFFFF
        self.request_tag = request_tag
        self.response_tag = response_tag
        self.definition_index = definition_index & 0xFFFF
        self.strict_message_id = strict_message_id
        self.observed_message_ids = []

        eb = _eboot()
        self.request_size = self.request_align = None
        self.response_size = self.response_align = None
        if request_tag is not None:
            self.request_size, self.request_align, _, _ = descriptor_info(request_tag, eb)
        if response_tag is not None:
            self.response_size, self.response_align, _, _ = descriptor_info(response_tag, eb)

        self.harness = trace.h.Harness()
        self.mu = self.harness.mu

        # 0x25C9A0 returns an investment-globals manager whose vtable slot +0x4D0 returns a
        # definition pointer already biased by +0x10.  The native encoder reads the message id at
        # returned+0x10 (0x261C93) and the schema tag at +0x14 (request) / +0x18 (response).
        self.definition = self.harness.alloc(0x80)
        self.mu.mem_write(self.definition + 0x10, struct.pack("<H", self.message_id))
        if request_tag is not None:
            self.mu.mem_write(self.definition + 0x14, struct.pack("<I", request_tag))
        if response_tag is not None:
            self.mu.mem_write(self.definition + 0x18, struct.pack("<I", response_tag))

        vtable = self.harness.alloc(0x600)
        self.mu.mem_write(vtable + 0x4D0, struct.pack("<Q", trace.STUB_ACCESSOR))
        self.manager = self.harness.alloc(0x40)
        self.mu.mem_write(self.manager, struct.pack("<Q", vtable))

        _build_registry_for(self.harness, eb, [response_tag, request_tag], log=log)

        original_code_hook = self.harness._code

        def code_hook(uc, address, size, user_data):
            if address == 0:
                raise RuntimeError("native codec dispatched through a null function pointer")
            if address == trace.GET_MANAGER:
                self.harness._ret(self.manager)
                return
            if address == trace.STUB_ACCESSOR:
                self.harness._ret(self.definition)
                return
            if address == trace.ID_TO_INDEX:
                seen = uc.reg_read(trace.UC_X86_REG_RDI) & 0xFFFF
                self.observed_message_ids.append(seen)
                if self.strict_message_id and seen != self.message_id:
                    raise RuntimeError(
                        f"stream carries message id {seen} ({seen:#06x}), oracle is built for "
                        f"{self.message_id} ({self.message_id:#06x})"
                    )
                self.harness._ret(self.definition_index)
                return
            return original_code_hook(uc, address, size, user_data)

        # ONE hook.  The harness dispatches through this attribute; registering a second Unicorn
        # hook double-executes import stubs.  See d1-oodle-harness.py.__init__.
        self.harness._code = code_hook

    # ---- handles -------------------------------------------------------------------------------
    def object_size(self, direction=DIR_RESPONSE):
        size = self.response_size if direction == DIR_RESPONSE else self.request_size
        if size is None:
            raise RuntimeError(f"no schema tag configured for direction {direction}")
        return size

    def make_handle(self, object_bytes=None, span=b"", direction=DIR_RESPONSE,
                    definition_index=None):
        """Build a 0x20-byte envelope the native encoder can consume."""
        size = self.object_size(direction)
        obj = self.harness.alloc(max(size, 16))
        if object_bytes:
            if len(object_bytes) > size:
                raise ValueError(
                    f"object is {len(object_bytes)} bytes, schema struct is {size}")
            self.mu.mem_write(obj, bytes(object_bytes))
        handle = self.harness.alloc(ENVELOPE_SIZE)
        index = self.definition_index if definition_index is None else definition_index
        # handle+0x00 is the u16 the ENCODER feeds to the definition accessor (0x261C1D).  Our
        # accessor stub ignores its argument, so this value does not reach the wire -- the self-test
        # proves that by encoding the same object under two different values.
        self.mu.mem_write(handle + 0x00, struct.pack("<H", index & 0xFFFF))
        self.mu.mem_write(handle + 0x08, struct.pack("<Q", obj))
        if span:
            span_ptr = self.harness.alloc(len(span))
            self.mu.mem_write(span_ptr, bytes(span))
            self.mu.mem_write(handle + 0x10, struct.pack("<Q", span_ptr))
            self.mu.mem_write(handle + 0x18, struct.pack("<I", len(span)))
        return handle

    # ---- codecs --------------------------------------------------------------------------------
    def encode(self, handle, direction=DIR_RESPONSE, capacity=0x4000):
        output = self.harness.alloc(capacity)
        output_len = self.harness.alloc(8)
        rc = self.harness.call(ENCODER, handle, direction, output, capacity, output_len) & 0xFF
        if rc != 1:
            raise RuntimeError(f"native encoder returned {rc}")
        length = struct.unpack("<I", self.mu.mem_read(output_len, 4))[0]
        if not 2 <= length <= capacity:
            raise RuntimeError(f"native encoder returned invalid length {length}")
        return bytes(self.mu.mem_read(output, length))

    def decode(self, wire, direction=DIR_RESPONSE, capacity=0x4000):
        """Decode a full wire message (u16 message id first) and return the WHOLE envelope.

        Returns a dict:
            id            u32 at envelope+0x00 -- low 16 bits are the definition INDEX
            object_ptr    envelope+0x08
            span_ptr      envelope+0x10
            span_count    envelope+0x18   (BYTES, see the module docstring)
            object        the decoded C struct's bytes
            span          the span's bytes
            decoded_size  the arena high-water mark the decoder reported through its r9 out-param
            envelope      the raw 0x20 bytes
        """
        wire = bytes(wire)
        if len(wire) < 2:
            raise ValueError("wire must carry at least the u16 message id")
        if capacity <= MIN_CAPACITY:
            raise ValueError(f"capacity must exceed {MIN_CAPACITY:#x} (0x262063)")

        input_ptr = self.harness.alloc(len(wire) + 16)
        self.mu.mem_write(input_ptr, wire)
        arena = self.harness.alloc(capacity)
        decoded_size = self.harness.alloc(8)

        rc = self.harness.call(
            DECODER, input_ptr, len(wire), direction, arena, capacity, decoded_size
        ) & 0xFF
        if rc != 1:
            raise RuntimeError(f"native decoder returned {rc}")

        high = struct.unpack("<I", self.mu.mem_read(decoded_size + 4, 4))[0]
        if high:
            raise RuntimeError("decoder out-param is a u32; upper half is non-zero")
        size = struct.unpack("<I", self.mu.mem_read(decoded_size, 4))[0]

        env = bytes(self.mu.mem_read(arena, ENVELOPE_SIZE))
        ident = struct.unpack_from("<I", env, 0x00)[0]
        object_ptr = struct.unpack_from("<Q", env, 0x08)[0]
        span_ptr = struct.unpack_from("<Q", env, 0x10)[0]
        span_count = struct.unpack_from("<i", env, 0x18)[0]

        if object_ptr == 0:
            raise RuntimeError("decoded envelope has a null object pointer")
        obj_size = self.object_size(direction)
        obj = bytes(self.mu.mem_read(object_ptr, obj_size))
        span = b""
        if span_ptr and span_count > 0:
            span = bytes(self.mu.mem_read(span_ptr, span_count))
        elif span_count > 0:
            raise RuntimeError(f"span_count {span_count} with a NULL span pointer")

        return {
            "rc": rc,
            "id": ident,
            "definition_index": ident & 0xFFFF,
            "object_ptr": object_ptr,
            "span_ptr": span_ptr,
            "span_count": span_count,
            "span": span,
            "object": obj,
            "decoded_size": size,
            "arena": arena,
            "envelope": env,
            "handle": arena,          # the envelope IS the handle the encoder consumes
        }

    def decode_body(self, body, direction=DIR_RESPONSE, **kw):
        """Decode a body that does not already carry the u16 message id."""
        return self.decode(struct.pack(">H", self.message_id) + bytes(body), direction, **kw)


# ---- positive control ----------------------------------------------------------------------------
def self_test(verbose=True):
    """Reproduce the pinned 503 vector and exercise the span path in both directions.

    Raises on ANY divergence.  Nothing else in this file should run if this does not pass.
    """
    def say(*a):
        if verbose:
            print(*a)

    o = Oracle(CONTROL_MESSAGE_ID, CONTROL_REQUEST_TAG, CONTROL_RESPONSE_TAG)
    if o.response_size != 0x3C8:
        raise RuntimeError(
            f"503 response struct is {o.response_size:#x} from the descriptor, expected 0x3c8")

    obj = bytearray(o.response_size)
    struct.pack_into("<Q", obj, 0x08, CONTROL_ACCOUNT_SOID)
    struct.pack_into("<Q", obj, 0x18, CONTROL_FAMILY5_ROOT)

    handle = o.make_handle(bytes(obj))
    wire = o.encode(handle)
    expected = struct.pack(">H", CONTROL_MESSAGE_ID) + bytes.fromhex(CONTROL_BODY_HEX)
    if wire != expected:
        raise RuntimeError(
            f"pinned 503 vector drifted: got {wire.hex()}, expected {expected.hex()}")
    say(f"  [1] encode(503) == pinned vector          {wire.hex()}")

    # handle+0x00 must not reach the wire: our accessor stub ignores it.  Encoding the same object
    # under a different value proves the field is the definition index, not the message id.
    alt = o.make_handle(bytes(obj), definition_index=CONTROL_MESSAGE_ID)
    if o.encode(alt) != wire:
        raise RuntimeError("handle+0x00 changed the encoded bytes; the definition model is wrong")
    say("  [2] handle+0x00 does not reach the wire    (it is the definition index)")

    d = o.decode(wire)
    if d["decoded_size"] != ENVELOPE_SIZE + o.response_size:
        raise RuntimeError(
            f"decoded size {d['decoded_size']:#x}, expected "
            f"{ENVELOPE_SIZE + o.response_size:#x}")
    checks = {
        "byte+0x00": (d["object"][0], 0),
        "u32+0x04": (struct.unpack_from("<I", d["object"], 0x04)[0], 0),
        "u64+0x08": (struct.unpack_from("<Q", d["object"], 0x08)[0], CONTROL_ACCOUNT_SOID),
        "u32+0x10": (struct.unpack_from("<I", d["object"], 0x10)[0], 0),
        "u32+0x14": (struct.unpack_from("<I", d["object"], 0x14)[0], 0),
        "field4.root": (struct.unpack_from("<Q", d["object"], 0x18)[0], CONTROL_FAMILY5_ROOT),
    }
    bad = [f"{k}: got {v:#x} want {w:#x}" for k, (v, w) in checks.items() if v != w]
    if bad:
        raise RuntimeError("decoded field mismatch: " + "; ".join(bad))
    say("  [3] decode(503) semantic fields            OK")

    if d["span_ptr"] != 0 or d["span_count"] != 0:
        raise RuntimeError(
            f"503 span should be empty, got ptr={d['span_ptr']:#x} count={d['span_count']}")
    say("  [4] 503 span is empty                      (expected: 0x80801825 sets no span bit)")

    if o.encode(d["handle"]) != wire:
        raise RuntimeError("encode -> decode -> encode is not a fixed point")
    say("  [5] encode -> decode -> encode fixed point OK")

    # ---- the span path, in both directions.  Without this a sweep reporting span_count == 0
    # everywhere cannot be told apart from a harness that never populates the field at all.
    probe = bytes(range(0x11, 0x11 + 21))          # one codec-0x7B-sized record, distinctive bytes
    span_handle = o.make_handle(bytes(obj), span=probe)
    span_wire = o.encode(span_handle)
    if span_wire == wire:
        raise RuntimeError("attaching a span did not change the encoded bytes")
    sd = o.decode(span_wire)
    if sd["span_count"] != len(probe):
        raise RuntimeError(
            f"span round-trip: count {sd['span_count']}, expected {len(probe)}")
    if sd["span"] != probe:
        raise RuntimeError(
            f"span round-trip: bytes {sd['span'].hex()}, expected {probe.hex()}")
    if sd["span_ptr"] == 0:
        raise RuntimeError("span round-trip: non-zero count with a NULL pointer")
    if sd["decoded_size"] != ENVELOPE_SIZE + o.response_size + len(probe):
        raise RuntimeError(
            f"span round-trip: decoded size {sd['decoded_size']:#x}")
    say(f"  [6] span round-trip {len(probe)} bytes            "
        f"count={sd['span_count']} ptr={sd['span_ptr']:#x}  bytes match")
    if o.encode(sd["handle"]) != span_wire:
        raise RuntimeError("span encode -> decode -> encode is not a fixed point")
    say("  [7] span encode -> decode -> encode        fixed point OK")
    return True


def run_control(enabled):
    if not enabled:
        print("!! POSITIVE CONTROL SKIPPED -- any negative result from this run is UNINTERPRETABLE",
              file=sys.stderr)
        return
    try:
        self_test(verbose=False)
    except Exception as exc:
        print(f"POSITIVE CONTROL FAILED, refusing to run: {exc}", file=sys.stderr)
        raise SystemExit(2)
    print("positive control: PASS (pinned 503 vector + span round-trip)")


# ---- CLI -----------------------------------------------------------------------------------------
def _hexarg(s):
    s = s.strip().replace(" ", "").replace("_", "")
    return bytes.fromhex(s)


def _report(d):
    print("ENVELOPE (0x20 bytes)  " + d["envelope"].hex())
    print(f"  +0x00 id / definition index = {d['id']:#010x}  ({d['definition_index']})")
    print(f"  +0x08 object ptr            = {d['object_ptr']:#x}")
    print(f"  +0x10 span ptr              = {d['span_ptr']:#x}")
    print(f"  +0x18 span count (BYTES)    = {d['span_count']}")
    print(f"  reported decoded size       = {d['decoded_size']:#x}")
    if d["span"]:
        print(f"  span bytes                  = {d['span'].hex()}")
    print(f"  object ({len(d['object'])} bytes)  = {d['object'][:64].hex()}"
          + ("..." if len(d["object"]) > 64 else ""))


def main():
    ap = argparse.ArgumentParser(
        description=__doc__.splitlines()[0],
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="See the module docstring for the wire format and the harness traps.")
    ap.add_argument("--self-test", action="store_true",
                    help="reproduce the pinned 503 vector and the span round-trip, then exit")
    ap.add_argument("--schema", type=lambda s: int(s, 0),
                    help="response schema tag to decode/encode against")
    ap.add_argument("--request-tag", type=lambda s: int(s, 0))
    ap.add_argument("--msgid", type=lambda s: int(s, 0), default=0,
                    help="u16 message id written to / expected on the wire")
    ap.add_argument("--index", type=lambda s: int(s, 0), default=0,
                    help="value 0x27FDD0 is stubbed to return (envelope+0x00)")
    ap.add_argument("--direction", type=int, choices=(0, 1), default=1,
                    help="0 = request schema, 1 = response schema (default)")
    ap.add_argument("--wire", type=_hexarg, help="hex INCLUDING the u16 message id")
    ap.add_argument("--body", type=_hexarg, help="hex WITHOUT the message id; it is prepended")
    ap.add_argument("--encode", action="store_true",
                    help="encode instead of decode; use --object / --span")
    ap.add_argument("--object", type=_hexarg, default=b"",
                    help="decoded C struct bytes for --encode (zero-filled if short)")
    ap.add_argument("--span", type=_hexarg, default=b"",
                    help="span bytes for --encode")
    ap.add_argument("--capacity", type=lambda s: int(s, 0), default=0x4000)
    ap.add_argument("--any-msgid", action="store_true",
                    help="do not require the stream's message id to match --msgid")
    ap.add_argument("--no-control", action="store_true",
                    help="skip the positive control (results become uninterpretable)")
    args = ap.parse_args()

    if args.self_test:
        print("SELF-TEST  (positive control for every sweep built on this oracle)")
        self_test()
        print("PASS")
        return 0

    if args.schema is None:
        ap.error("one of --self-test or --schema is required")
    if args.direction == DIR_REQUEST and args.request_tag is None:
        ap.error("--direction 0 needs --request-tag (a zero tag resolves to garbage, not a fault)")

    run_control(not args.no_control)

    eb = _eboot()
    size, align, anchor, obj = descriptor_info(args.schema, eb)
    print(f"schema {args.schema:#010x}: struct {size} bytes, align {align}, "
          f"anchor {anchor:#x}, registry object {obj:#x}")
    absent = missing_children(args.schema, eb)
    if absent:
        print("  ⚠ child tags with no descriptor in the eboot (these fault mid-decode): "
              + " ".join(f"{t:#010x}" for t in absent))

    o = Oracle(args.msgid, args.request_tag, args.schema,
               definition_index=args.index, strict_message_id=not args.any_msgid)

    if args.encode:
        handle = o.make_handle(args.object, span=args.span, direction=args.direction)
        wire = o.encode(handle, direction=args.direction, capacity=args.capacity)
        print(f"wire = {wire.hex()}")
        print(f"body = {wire[2:].hex()}")
        d = o.decode(wire, direction=args.direction, capacity=args.capacity)
        print("-- decoded back --")
        _report(d)
        if d["span"] != args.span:
            print(f"  ⚠ span did not round-trip: {d['span'].hex()} != {args.span.hex()}")
        return 0

    if args.wire is None and args.body is None:
        ap.error("--schema without --encode needs --wire or --body")
    wire = args.wire if args.wire is not None else struct.pack(">H", args.msgid) + args.body
    print(f"decoding {len(wire)} byte(s): {wire.hex()}")
    d = o.decode(wire, direction=args.direction, capacity=args.capacity)
    _report(d)
    if o.observed_message_ids:
        print(f"  stream message id           = {o.observed_message_ids[0]} "
              f"({o.observed_message_ids[0]:#06x})")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except SystemExit:
        raise
    except Exception as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        sys.exit(1)
