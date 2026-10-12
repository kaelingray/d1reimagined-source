#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Drive codec 0x7B's transform (0x568F30) offline -- the consumer of the envelope span.

The envelope-span sweep answers "what makes the span non-empty".  This answers the question one
step downstream, which the sweep's recommendation depends on: GIVEN a non-empty span, does the
client's QueueZ codec ACCEPT its contents?

Call shape, read from the crash site 0x570AAE..0x570AC2:
    0x568F30(rdi = codec object, rsi = span ptr, edx = span LENGTH,
             ecx = 0x10000 capacity, r8 = &out_len, r9 = 64KB output buffer)
    returns AL  (0x5690E5 `mov al, bl` -- bl is the accept flag)

Acceptance rules, read from the disassembly and then CONFIRMED by running it:
    memcpy(dst, src, len)                     0x568F5D -- NO minimum-length check
    [out_len] = len                           0x568F62
    count = bswap32(dst[0:4])                 0x568F65/0x568F68
    count == 0  -> "invalid queuez transaction count %d", bl = 0, RETURN FALSE   0x568F6D
    per element, stride 0x15 from dst+4:
        +0x00 u32, +0x04 u64, +0x0C u32, +0x11 u32   all byte-swapped in place
        +0x0C == 0xFFFFFFFF is a sentinel; with it, +0x11 != 0 sets bl = 0   0x568FC4/0x568FD1
        +0x0C != -1 and +0x11 != 0 walks a trailing payload chain of 16-byte headers  0x569020

⚠ NOTE the missing length check: a span shorter than 4 bytes makes 0x568F65 read whatever is in
the caller's uninitialised 64 KB stack buffer.  That is why a 1-byte span is a SHAPE probe only
and must never be proposed as server bytes.

Everything here is SYNTHETIC -- our harness executing the client's native code offline.  No
emulator is launched.
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

QUEUEZ_TRANSFORM = 0x568F30
OUT_CAPACITY = 0x10000


class QueuezCodec:
    def __init__(self):
        self.harness = trace.h.Harness()
        self.mu = self.harness.mu
        # The codec object is only dereferenced by its CALLER (0x570AB4 loads the vtable); the
        # transform itself never touches rdi.  Confirmed by disassembly: no [rdi] reference in
        # 0x568F30..0x5690F4.  A dummy is therefore sound, and the run would fault if it were not.
        self.codec = self.harness.alloc(0x40)

    def transform(self, span: bytes):
        src = self.harness.alloc(max(len(span), 16))
        if span:
            self.mu.mem_write(src, span)
        out = self.harness.alloc(OUT_CAPACITY)
        # Poison the output buffer so a short-span over-read is VISIBLE rather than reading our
        # own zeroed heap and silently looking like count 0.
        self.mu.mem_write(out, b"\xa5" * 64)
        out_len = self.harness.alloc(8)
        rc = self.harness.call(
            QUEUEZ_TRANSFORM, self.codec, src, len(span), OUT_CAPACITY, out_len, out
        ) & 0xFF
        n = struct.unpack("<I", self.mu.mem_read(out_len, 4))[0]
        return {
            "accepted": bool(rc & 1),
            "out_len": n,
            "out": bytes(self.mu.mem_read(out, min(max(n, 4), 128))),
        }


VECTORS = [
    ("count 0 (the 'cheapest safe shot' both prior analyses proposed)",
     "00000000"),
    ("count 1 + one 21-byte record, +0x0c=0x64ae75bf, +0x11=0",
     "00000001" "00000004" "d100000000000001" "64ae75bf" "00" "00000000"),
    ("count 1, +0x0c = 0xFFFFFFFF sentinel, +0x11 = 0",
     "00000001" "00000004" "d100000000000001" "ffffffff" "00" "00000000"),
    ("count 1, +0x0c = 0xFFFFFFFF sentinel, +0x11 = 1  (the sentinel reject path)",
     "00000001" "00000004" "d100000000000001" "ffffffff" "00" "00000001"),
    ("count 2 + two clean 21-byte records",
     "00000002"
     "00000004" "d100000000000001" "64ae75bf" "00" "00000000"
     "00000004" "d100000000000002" "64ae75c0" "00" "00000000"),
    ("d1_bap::queuez_object_body's 50-byte family-4 shape (§26.23's flagged tension)",
     "00000004" "7fffffffffffffff" "00000001" "00000001" "64ae75bf"
     "d100000000000001" "00000000"),
    ("1 byte only -- SHORT SPAN, reads past the memcpy (poison 0xa5 should show)",
     "aa"),
]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--span", help="hex span to test instead of the built-in vectors")
    a = ap.parse_args()
    c = QueuezCodec()

    if a.span:
        span = bytes.fromhex(a.span)
        r = c.transform(span)
        print(f"span ({len(span)}B) {span.hex()}")
        print(f"  accepted={r['accepted']}  out_len={r['out_len']}  out={r['out'].hex()}")
        return 0

    print("codec 0x7B transform 0x568F30, driven offline.  accepted = AL (0x5690E5 `mov al,bl`)\n")
    worst = 0
    for label, hexs in VECTORS:
        span = bytes.fromhex(hexs)
        try:
            r = c.transform(span)
            print(f"  accepted={str(r['accepted']):<5} out_len={r['out_len']:<5} "
                  f"len={len(span):<3} {label}")
            print(f"      span {span.hex()}")
            print(f"      out  {r['out'].hex()}")
        except Exception as e:                                       # noqa: BLE001
            worst = 1
            print(f"  RAISED  {type(e).__name__}: {e}   [{label}]")
    print("\nPOSITIVE CONTROL: the count-1 clean-record vector must be accepted while the count-0")
    print("vector is refused.  If BOTH come back the same, this oracle proves nothing.")
    return worst


if __name__ == "__main__":
    sys.exit(main())
