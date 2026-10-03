"""Bit-exact regression coverage for the two non-width-based primitive readers."""

import copy
import unittest

import investment as inv


def field(off, kind, width=0, minimum=0, optional=False, nested=0xFFFFFFFF):
    return {"off": off, "bit": 0, "kind": kind, "opt": int(optional),
            "nested": nested, "arr": 0, "min": minimum, "width": width, "extra": 0}


# These fixtures contain no game assets and retain the real offsets and widths.
# The outer host-object fields match in both builds; its nested object-reference
# kind grows from six to seven bits in 01.24 (adjusted in the cross-build test).
SCHEMA = {
    "0x80800046": {"size": 4, "fields": [field(0, 0x17)]},
    "0x808004b1": {"size": 8, "fields": [field(0, 9, 32),
        field(4, 3, 7, 1), field(6, 4, 16, 0x8000)]},
    "0x8080065c": {"size": 0x60, "fields": [field(0, 2),
        field(4, 5, 32, 0x80000000), field(8, 1, nested=0x808004B1), field(0x10, 0x22)]},
    "0x80800538": {"size": 12, "fields": [field(0, 9, 32),
        field(4, 9, 32), field(8, 5, 32, 0x80000000)]},
    "0x101": {"size": 0x60, "fields": [field(0, 0x17), field(4, 2),
        field(8, 0x22), field(0x58, 2)]},
    "0x102": {"size": 0x50, "fields": [field(0, 0x22, optional=True), field(0x40, 2)]},
}


def packed(bits):
    bits += "0" * (-len(bits) % 8)
    return bytes(int(bits[i:i + 8], 2) for i in range(0, len(bits), 8))


class TypedValues(unittest.TestCase):
    def test_nullable_handle_default_and_explicit_null(self):
        for value in (None, -1, 0xFFFFFFFF):
            with self.subTest(value=value):
                self.assertEqual(inv.encode(SCHEMA, 0x80800046, {0: value}), b"\0")
        self.assertEqual(inv.decode(SCHEMA, 0x80800046, b"\0"), {0: -1})

    def test_nullable_handle_zero_is_present_and_invalid_values_rejected(self):
        self.assertEqual(inv.encode(SCHEMA, 0x80800046, {0: 0}), bytes.fromhex("8000000000"))
        for value in (-2, 0x100000000):
            with self.assertRaises(ValueError):
                inv.encode(SCHEMA, 0x80800046, {0: value})

    def test_null_fields_do_not_consume_adjacent_sentinels(self):
        # null handle, true, null typed value, true = 0101.
        expected = b"\x50"
        values = {0: -1, 4: True, 8: None, 0x58: True}
        self.assertEqual(inv.encode(SCHEMA, 0x101, values), expected)
        self.assertEqual(inv.decode(SCHEMA, 0x101, expected), values)

    def test_present_handle_and_typed_payload_use_exact_nested_schema(self):
        payload = {0: 0x11223344, 4: 0x55667788, 8: -7}
        values = {0: 0x12345678, 4: False,
                  8: {"type": "0x80800538", "values": payload}, 0x58: True}
        bits = ("1" + f"{0x12345678:032b}" + "0" + "1" + f"{0x80800538:032b}" +
                f"{0x11223344:032b}" + f"{0x55667788:032b}" + f"{0x7FFFFFF9:032b}" + "1")
        expected = packed(bits)
        self.assertEqual(inv.encode(SCHEMA, 0x101, values), expected)
        decoded = inv.decode(SCHEMA, 0x101, expected)
        self.assertEqual(decoded, {**values, 8: {"type": 0x80800538, "values": payload}})

    def test_optional_absence_and_present_null_are_distinct(self):
        self.assertEqual(inv.encode(SCHEMA, 0x102, {0x40: True}), b"\x40")
        self.assertEqual(inv.encode(SCHEMA, 0x102, {0: None, 0x40: True}), b"\xa0")
        self.assertEqual(inv.decode(SCHEMA, 0x102, b"\x40"), {0x40: True})
        self.assertEqual(inv.decode(SCHEMA, 0x102, b"\xa0"), {0: None, 0x40: True})

    def test_required_host_object_null_is_one_bit_in_both_builds(self):
        for build, kind_width in (("01.00", 6), ("01.24", 7)):
            with self.subTest(build=build):
                schema = copy.deepcopy(SCHEMA)
                schema["0x808004b1"]["fields"][1]["width"] = kind_width
                # Active, generation1, null reference and null typed value:
                # 88 bits in 01.00, 89 in 01.24; no game blobs are required.
                expected = packed("1" + f"{0x80000001:032b}" + f"{0x811C9DC5:032b}" +
                                  "0" * kind_width + "0" * 16 + "0")
                values = {0: True, 4: 1, 8: {0: 0x811C9DC5, 4: -1, 6: -32768}, 0x10: None}
                writer = inv.BitWriter()
                inv.encode_into(writer, schema, 0x8080065C, values)
                self.assertEqual(len(writer.bits), 82 + kind_width)
                self.assertEqual(inv.encode(schema, 0x8080065C, values), expected)
                self.assertEqual(inv.decode(schema, 0x8080065C, expected), values)

    def test_unknown_or_malformed_typed_payload_is_rejected(self):
        for value in ({"type": 0xDEADBEEF, "values": {}}, {"type": 0x80800538},
                      {"type": 0x80800538, "values": []}, 0, []):
            with self.subTest(value=value), self.assertRaises(ValueError):
                inv.encode(SCHEMA, 0x101, {8: value})
        # A known 32-bit handle is required before the dynamic decoder can proceed.
        with self.assertRaises(ValueError):
            inv.decode(SCHEMA, 0x101, packed("0" + "0" + "1" + f"{0xDEADBEEF:032b}"))

    def test_truncated_present_handle_or_nested_payload_is_rejected(self):
        with self.assertRaises(ValueError):
            inv.decode(SCHEMA, 0x80800046, b"\x80")
        with self.assertRaises(ValueError):
            inv.decode(SCHEMA, 0x101, packed("0" + "0" + "1" + f"{0x80800538:032b}"))


if __name__ == "__main__":
    unittest.main()
