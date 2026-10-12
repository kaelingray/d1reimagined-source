import struct
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1_types  # noqa: E402


def definition(handle, size, fields, link=0x54, counted=False):
    """A type definition as compiled into the EBOOT: size at +0x14, a relative pointer at +0x54
    (+0x34 in the title updates) to the field list {count, handle, size, 8 bytes, then 0x28-byte
    field entries}."""
    head = bytearray(link + 4)
    struct.pack_into(">I", head, 0x14, size)
    struct.pack_into(">I", head, link, 4)  # the list follows the pointer word
    body = struct.pack(">III", len(fields), handle, size) + bytes(8)
    for field in fields:
        off, kind, opt, nested, minimum, width = field[:6]
        repeat, shift = (field[6] if len(field) > 6 else 0), (4 if counted else 0)
        e = bytearray(0x28)
        if counted:
            struct.pack_into(">I", e, 0, repeat)  # the updates lead each entry with a repeat count
        struct.pack_into(">III", e, shift, off, off, 0)
        e[shift + 0x10], e[shift + 0x11] = kind, opt
        struct.pack_into(">III", e, shift + 0x14, nested, minimum, width)
        body += bytes(e)
    return bytes(head) + body


class ScanTypes(unittest.TestCase):
    def test_finds_definitions_and_signature(self):
        inner = definition(0x80801704, 4, [(0, 3, 1, 0xFFFFFFFF, 0x80, 8), (2, 4, 1, 0xFFFFFFFF, 0x8000, 16)])
        outer = definition(0x8080183C, 0x28, [(0, 6, 1, 0xFFFFFFFF, 0, 64), (0xE, 1, 1, 0x80801704, 0, 0)])
        data = bytes(12) + inner + bytes(8) + outer + bytes(64)
        types = d1_types.scan_types(data)
        self.assertEqual(set(types), {"0x80801704", "0x8080183c"})
        self.assertEqual(types["0x8080183c"]["size"], 0x28)
        self.assertEqual(types["0x8080183c"]["fields"][1]["nested"], 0x80801704)
        self.assertEqual(d1_types.signature(types, 0x8080183C), "u64? {u8? i16?}?")

    def test_title_update_layout(self):
        # the updates' definitions link from +0x34 and lead each field entry with a repeat count;
        # a fixed array is one entry whose offset is the stride
        a = definition(0x80800051, 8, [(0, 9, 0, 0xFFFFFFFF, 0, 32), (4, 3, 1, 0xFFFFFFFF, 0x80, 8)],
                       link=0x34, counted=True)
        b = definition(0x80800052, 0x80, [(1, 3, 0, 0xFFFFFFFF, 0x80, 8, 0x80)], link=0x34, counted=True)
        types = d1_types.scan_types(bytes(8) + a + b + bytes(0x60))
        self.assertEqual(set(types), {"0x80800051", "0x80800052"})
        self.assertEqual(d1_types.signature(types, 0x80800051), "u32 u8?")
        self.assertEqual(d1_types.signature(types, 0x80800052), "u8x128")
        self.assertEqual(types["0x80800052"]["fields"][127]["off"], 127)


if __name__ == "__main__":
    unittest.main()
