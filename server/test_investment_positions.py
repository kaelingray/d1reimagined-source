"""Independent wire checks for raw positions and following marker references."""
import struct
import unittest

import investment as inv


def field(off, kind, width=0, minimum=0, nested=0xFFFFFFFF):
    return {"off": off, "kind": kind, "width": width, "min": minimum,
            "nested": nested, "opt": 0, "arr": 0, "bit": 0, "extra": 0}


SCHEMA = {
    "0x101": {"size": 16, "fields": [field(0, 13, minimum=128)]},
    "0x102": {"size": 8, "fields": [field(0, 9, 32),
        field(4, 3, 7, 1), field(6, 4, 16, 0x8000)]},
    "0x103": {"size": 64, "fields": [field(0, 3, 5, 1),
        field(1, 3, 3, 1), field(4, 1, nested=0x102),
        field(12, 5, 32, 0x80000000), field(16, 13, minimum=128),
        field(32, 1, nested=0x102)]},
    "0x104": {"size": 640, "fields": [field(i*64, 1, nested=0x103)
                                                   for i in range(10)]},
}


def pack_bits(bits):
    bits += "0" * (-len(bits) % 8)
    return bytes(int(bits[i:i+8], 2) for i in range(0, len(bits), 8))


def ref_bits(group, kind, index):
    return f"{group:032b}{kind+1:07b}{(index+0x8000)&0xFFFF:016b}"


class Positions(unittest.TestCase):
    def test_default_position_consumes_three_raw_floats(self):
        self.assertEqual(inv.encode(SCHEMA, 0x101), b"\0"*12)
        self.assertEqual(inv.decode(SCHEMA, 0x101, b"\0"*12), {0: [0., 0., 0.]})

    def test_exact_nonzero_wire(self):
        values = {0: [354.25, -796.5, 14.125]}
        wire = struct.pack(">fff", *values[0])
        self.assertEqual(inv.encode(SCHEMA, 0x101, values), wire)
        self.assertEqual(inv.decode(SCHEMA, 0x101, wire), values)

    def test_following_reference_keeps_its_bits(self):
        values = {0: 1, 12: 14, 16: [1.25, -2.5, 3.75],
                  32: {0: 0x12345678, 4: 44, 6: 87}}
        position = ''.join(f'{b:08b}' for b in struct.pack('>fff', *values[16]))
        bits = ('00010'+'001'+ref_bits(0, 0, 0)+f'{0x8000000E:032b}'
                +position+ref_bits(0x12345678, 44, 87))
        wire = pack_bits(bits)
        self.assertEqual(inv.encode(SCHEMA, 0x103, values), wire)
        result = inv.decode(SCHEMA, 0x103, wire)
        self.assertEqual(result[16], values[16])
        self.assertEqual(result[32], values[32])

    def test_all_ten_default_positions_preserve_later_marker(self):
        values = {9*64: {0: 1, 32: {0: 0xABCDEF01, 4: 44, 6: 88}}}
        wire = inv.encode(SCHEMA, 0x104, values)
        # 8 enum bits, two 55-bit references, i32 and raw xyz per entry.
        self.assertEqual(len(wire), (10*(8+55+32+96+55)+7)//8)
        result = inv.decode(SCHEMA, 0x104, wire)
        for index in range(10):
            self.assertEqual(result[index*64][16], [0., 0., 0.])
        self.assertEqual(result[9*64][32], values[9*64][32])

    def test_invalid_positions_are_rejected(self):
        for value in (0, [], [1, 2], [1, 2, 3, 4], [1, 2, float('nan')],
                      [1, 2, float('inf')], [1, 2, 1e50], [1, 2, {}]):
            with self.subTest(value=value), self.assertRaises(ValueError):
                inv.encode(SCHEMA, 0x101, {0: value})

    def test_truncated_position_is_rejected(self):
        for size in (0, 4, 8, 11):
            with self.subTest(size=size), self.assertRaises(ValueError):
                inv.decode(SCHEMA, 0x101, b'\0'*size)


if __name__ == '__main__':
    unittest.main()
