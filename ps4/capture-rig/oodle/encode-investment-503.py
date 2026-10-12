#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Emit and verify Destiny's message-503 response with the client's native codecs.

This is an offline oracle.  It constructs the decoded 0x80801825 C object, invokes the eboot's
encoder at 0x261BE0, decodes the emitted bytes through 0x261FB0, and re-encodes the decoded handle.
Success requires both the semantic field checks and byte-identical encode/decode/encode output.

It never launches shadPS4 and never writes to dumps/ or runs/.
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
MESSAGE_ID = 503
REQUEST_TAG = 0x80801726
RESPONSE_TAG = 0x80801825
RESPONSE_OBJECT_SIZE = 0x3C8
DECODED_HANDLE_SIZE = 0x20

DEFAULT_ACCOUNT_SOID = 0xD100000000000001
DEFAULT_FAMILY5_ROOT = 0x7FFFFFFFFFFFFFFF

# Native-oracle output for the defaults above.  Pinning it catches harness or descriptor drift.
EXPECTED_BODY_HEX = (
    "180000000d1000000000000010000000000000000bfffffffffffffffc"
    "00000000000000020810220000000100"
)


def parse_u64(value: str) -> int:
    parsed = int(value, 0)
    if not 0 <= parsed <= 0xFFFFFFFFFFFFFFFF:
        raise argparse.ArgumentTypeError("value must fit in an unsigned 64-bit integer")
    return parsed


class Oracle:
    def __init__(self) -> None:
        self.harness = trace.h.Harness()
        self.mu = self.harness.mu

        # 0x25C9A0 returns an investment-globals manager whose +0x4D0 accessor returns a
        # definition pointer already biased by +0x10.  The native encoder reads the message id at
        # returned+0x10, so leaving this zero emits message id 0 even if handle+0 is 503.
        self.definition = self.harness.alloc(0x80)
        self.mu.mem_write(self.definition + 0x10, struct.pack("<H", MESSAGE_ID))
        self.mu.mem_write(self.definition + 0x14, struct.pack("<I", REQUEST_TAG))
        self.mu.mem_write(self.definition + 0x18, struct.pack("<I", RESPONSE_TAG))

        vtable = self.harness.alloc(0x600)
        self.mu.mem_write(vtable + 0x4D0, struct.pack("<Q", trace.STUB_ACCESSOR))
        self.manager = self.harness.alloc(0x40)
        self.mu.mem_write(self.manager, struct.pack("<Q", vtable))

        eboot = trace._ed.Eboot(trace._ed.EBOOT)
        trace.build_registry(
            self.harness, eboot, trace.DESC_INDEX, RESPONSE_TAG, log=False
        )

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
                message_id = uc.reg_read(trace.UC_X86_REG_RDI) & 0xFFFF
                if message_id != MESSAGE_ID:
                    raise RuntimeError(
                        f"decoder requested unexpected message id {message_id}"
                    )
                self.harness._ret(0)
                return
            return original_code_hook(uc, address, size, user_data)

        # The harness owns one late-bound hook.  Adding a second hook double-executes import stubs
        # and pops return addresses twice; see d1-oodle-harness.py.
        self.harness._code = code_hook

    def make_source_handle(self, account_soid: int, family5_root: int) -> int:
        obj = self.harness.alloc(RESPONSE_OBJECT_SIZE)
        # alloc() zeroes the object.  These are the only non-default semantic values.
        self.mu.mem_write(obj + 0x08, struct.pack("<Q", account_soid))
        self.mu.mem_write(obj + 0x18, struct.pack("<Q", family5_root))

        handle = self.harness.alloc(DECODED_HANDLE_SIZE)
        self.mu.mem_write(handle + 0x00, struct.pack("<H", MESSAGE_ID))
        self.mu.mem_write(handle + 0x08, struct.pack("<Q", obj))
        return handle

    def encode(self, handle: int) -> bytes:
        capacity = 0x4000
        output = self.harness.alloc(capacity)
        output_len = self.harness.alloc(8)
        rc = self.harness.call(ENCODER, handle, 1, output, capacity, output_len) & 0xFF
        if rc != 1:
            raise RuntimeError(f"native encoder returned {rc}")

        length = struct.unpack("<I", self.mu.mem_read(output_len, 4))[0]
        if not 2 <= length <= capacity:
            raise RuntimeError(f"native encoder returned invalid length {length}")
        return bytes(self.mu.mem_read(output, length))

    def decode(self, wire: bytes) -> tuple[int, bytes]:
        input_ptr = self.harness.alloc(len(wire))
        self.mu.mem_write(input_ptr, wire)
        handle = self.harness.alloc(0x4000)
        decoded_size = self.harness.alloc(8)

        rc = self.harness.call(
            DECODER, input_ptr, len(wire), 1, handle, 0x4000, decoded_size
        ) & 0xFF
        if rc != 1:
            raise RuntimeError(f"native decoder returned {rc}")

        size = struct.unpack("<Q", self.mu.mem_read(decoded_size, 8))[0]
        expected_size = DECODED_HANDLE_SIZE + RESPONSE_OBJECT_SIZE
        if size != expected_size:
            raise RuntimeError(
                f"decoded allocation is {size:#x}, expected {expected_size:#x}"
            )

        # The output is an opaque decoded-object handle, not a wire-message record.  In
        # particular, decoder 0x261FB0 leaves handle+0 zero; the message id was consumed to select
        # the definition and is not copied here.  The encoder gets 503 from that definition.
        object_ptr = struct.unpack("<Q", self.mu.mem_read(handle + 8, 8))[0]
        if object_ptr == 0:
            raise RuntimeError("decoded handle has a null object pointer")
        obj = bytes(self.mu.mem_read(object_ptr, RESPONSE_OBJECT_SIZE))
        return handle, obj


def require_fields(obj: bytes, account_soid: int, family5_root: int) -> None:
    actual = {
        "byte+0x00": obj[0],
        "u32+0x04": struct.unpack_from("<I", obj, 0x04)[0],
        "u64+0x08": struct.unpack_from("<Q", obj, 0x08)[0],
        "u32+0x10": struct.unpack_from("<I", obj, 0x10)[0],
        "u32+0x14": struct.unpack_from("<I", obj, 0x14)[0],
        "field4.root": struct.unpack_from("<Q", obj, 0x18)[0],
    }
    expected = {
        "byte+0x00": 0,
        "u32+0x04": 0,
        "u64+0x08": account_soid,
        "u32+0x10": 0,
        "u32+0x14": 0,
        "field4.root": family5_root,
    }
    failures = [
        f"{name}: decoded {actual[name]:#x}, expected {value:#x}"
        for name, value in expected.items()
        if actual[name] != value
    ]
    if failures:
        raise RuntimeError("decoded field mismatch: " + "; ".join(failures))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--account-soid", type=parse_u64, default=DEFAULT_ACCOUNT_SOID
    )
    parser.add_argument(
        "--family5-root", type=parse_u64, default=DEFAULT_FAMILY5_ROOT
    )
    args = parser.parse_args()

    oracle = Oracle()
    source_handle = oracle.make_source_handle(args.account_soid, args.family5_root)
    wire = oracle.encode(source_handle)
    decoded_handle, decoded_object = oracle.decode(wire)
    require_fields(decoded_object, args.account_soid, args.family5_root)

    second_wire = oracle.encode(decoded_handle)
    if second_wire != wire:
        raise RuntimeError(
            "native encode/decode/encode is not a fixed point: "
            f"first={wire.hex()} second={second_wire.hex()}"
        )

    expected_prefix = struct.pack(">H", MESSAGE_ID)
    if wire[:2] != expected_prefix:
        raise RuntimeError(
            f"wire message id is {wire[:2].hex()}, expected {expected_prefix.hex()}"
        )
    body = wire[2:]
    if (
        args.account_soid == DEFAULT_ACCOUNT_SOID
        and args.family5_root == DEFAULT_FAMILY5_ROOT
        and body.hex() != EXPECTED_BODY_HEX
    ):
        raise RuntimeError(
            "default native-oracle vector drifted: "
            f"got {body.hex()}, expected {EXPECTED_BODY_HEX}"
        )

    print("PASS native encode -> decode -> encode fixed point")
    print(f"decoded byte+0x00:  0x{decoded_object[0]:02x}")
    print(f"decoded u32+0x04:   0x{struct.unpack_from('<I', decoded_object, 4)[0]:08x}")
    print(f"decoded account SOID: 0x{struct.unpack_from('<Q', decoded_object, 8)[0]:016x}")
    print(f"decoded family5 root: 0x{struct.unpack_from('<Q', decoded_object, 0x18)[0]:016x}")
    print(f"full_503_payload={wire.hex()}")
    print(f"D1_503_RAW={body.hex()}")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        sys.exit(1)
