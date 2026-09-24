"""Encoder for Destiny (BLES01857 01.00) investment objects, driven by the engine's own
runtime type definitions.

The schema is exported from a running local test instance with
`scripts/rpcs3_tiger_types.py schema OUT.json HANDLE...` (offsets, kinds and bit widths only;
it is kept outside Git). Encoding mirrors the client's deserializer (0x25d2dc -> 0xd0ecd8):

  * fields in order, MSB-first bit stream;
  * an optional field is preceded by one presence bit;
  * kind 1 is a nested struct; with array mode 1 its element count is the value of the
    field at struct offset `width` in the enclosing struct;
  * primitives (readers at 0x175d5c8) store `value + min` in `width` bits: kind 2 bool
    (1 bit), 3/7 u8, 4/8 u16, 5/9 u32, 6/10 u64, 11 real32 (32 bits).

`values` maps a struct offset to a value (or, for nested fields, a dict or list of dicts for
that struct) at each level; everything not given is encoded as absent (optional) or zero.

Object and family wire formats (qz update, 0x51c7ec / 0x41b9ec): an object is a 16-byte
header - u32 type hash (the type definition's +4), u64 object soid, u32 payload length -
followed by the payload; a family is u32 type, u64 root soid, u32 version, u8 flags,
u32 object count, objects. 0x41b75c accepts a family only for an existing subscription
slot: a new subscription needs flags bit 0 (full snapshot); later deltas (bit 0 clear)
must carry version = previous + 1, otherwise sign-in reason 6 is set.
"""

import json
import struct
from pathlib import Path

DEFAULT_SCHEMA = Path(r"D:\Destiny360\SIGNON_LOCAL\investment_schema.json")


class BitWriter:
    def __init__(self):
        self.bits = []

    def put(self, value, width):
        self.bits += [(value >> (width - 1 - i)) & 1 for i in range(width)]

    def data(self):
        bits = self.bits + [0] * (-len(self.bits) % 8)
        return bytes(int("".join(map(str, bits[i:i + 8])), 2) for i in range(0, len(bits), 8))


def load_schema(path=DEFAULT_SCHEMA):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def _primitive(w, field, value):
    kind = field["kind"]
    if kind == 2:
        w.put(1 if value else 0, 1)
        return
    width = 32 if kind == 11 else field["width"]
    if kind == 11:
        value = struct.unpack(">I", struct.pack(">f", float(value or 0)))[0]
        w.put(value, 32)
        return
    w.put((int(value or 0) + field["min"]) & ((1 << width) - 1), width)


def encode_into(w, schema, handle, values=None):
    values = values or {}
    for field in schema[f"{handle:#x}"]["fields"]:
        off = field["off"]
        present = off in values
        if field["opt"]:
            w.put(1 if present else 0, 1)
            if not present:
                continue
        value = values.get(off)
        if field["kind"] != 1:
            _primitive(w, field, value)
        elif field["arr"] == 1:
            items = value or []
            if len(items) != int(values.get(field["width"], 0)):
                raise ValueError(f"array at {off:#x} needs its count at {field['width']:#x}")
            for item in items:
                encode_into(w, schema, field["nested"], item)
        else:
            encode_into(w, schema, field["nested"], value)


def encode(schema, handle, values=None):
    w = BitWriter()
    encode_into(w, schema, handle, values)
    return w.data()


def obj(type_hash, soid, payload):
    return struct.pack(">IQI", type_hash, soid, len(payload)) + payload


def family(family_type, root, objects, version=1, flags=0):
    out = struct.pack(">IQIBI", family_type, root, version, flags, len(objects))
    return out + b"".join(objects)


def qz_payload(families):
    return struct.pack(">I", len(families)) + b"".join(families)
