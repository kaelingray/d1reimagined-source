"""Independent wire fixtures for the null dynamic reference in 01.24."""
import unittest

import investment as inv


def field(off, kind, width=0):
    return {"off": off, "kind": kind, "width": width, "min": 0,
            "opt": 0, "arr": 0, "nested": 0xFFFFFFFF}


SCHEMA = {"0x100": {"size": 16, "fields": [field(0, 25), field(8, 2),
                                            field(12, 9, 32)]}}


class NullDynamicReference(unittest.TestCase):
    def test_default_null_keeps_following_fields_aligned(self):
        # Six-bit selector 22, absent handle, true, raw u32 = exactly 40 bits.
        expected = bytes.fromhex("5912345678")
        self.assertEqual(inv.encode(SCHEMA, 0x100, {8: True, 12: 0x12345678}), expected)
        self.assertEqual(inv.decode(SCHEMA, 0x100, expected),
                         {0: -1, 8: True, 12: 0x12345678})

    def test_explicit_null_forms_share_the_native_null_encoding(self):
        for value in (None, -1, 0xFFFFFFFF):
            with self.subTest(value=value):
                self.assertEqual(inv.encode(SCHEMA, 0x100, {0: value}), b"\x58\0\0\0\0")

    def test_nullable_selector_aliases_decode_without_losing_the_sentinel(self):
        for selector in (21, 22, 23):
            raw = bytes([(selector << 2) | 1]) + bytes.fromhex("89abcdef")
            with self.subTest(selector=selector):
                self.assertEqual(inv.decode(SCHEMA, 0x100, raw),
                                 {0: -1, 8: True, 12: 0x89ABCDEF})

    def test_non_null_references_are_explicitly_unsupported(self):
        for value in (0, 0x816564AA, {}, {"kind": 22, "value": 0}, True):
            with self.subTest(value=value), self.assertRaises(ValueError):
                inv.encode(SCHEMA, 0x100, {0: value})
        with self.assertRaises(ValueError):
            inv.decode(SCHEMA, 0x100, bytes.fromhex("5a0000000000"))

    def test_unknown_selectors_and_truncated_handles_are_rejected(self):
        for raw in (b"", b"\x5a", bytes.fromhex("0012345678"),
                    bytes.fromhex("fc12345678")):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                inv.decode(SCHEMA, 0x100, raw)


if __name__ == "__main__":
    unittest.main()
