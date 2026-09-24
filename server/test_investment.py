import struct
import unittest

import investment as inv

SCHEMA = {
    "0x1": {"size": 0x20, "fields": [
        {"off": 0, "kind": 6, "opt": 1, "nested": -1, "arr": 0, "min": 0, "width": 64, "extra": 0},
        {"off": 8, "kind": 3, "opt": 0, "nested": -1, "arr": 0, "min": 1, "width": 4, "extra": 0},
        {"off": 0xc, "kind": 5, "opt": 0, "nested": -1, "arr": 0x80, "min": 0x80000000, "width": 32, "extra": 0},
        {"off": 0x10, "kind": 5, "opt": 0, "nested": -1, "arr": 0, "min": 0, "width": 3, "extra": 0},
        {"off": 0x14, "kind": 1, "opt": 0, "nested": 2, "arr": 1, "min": 1, "width": 0x10, "extra": 0},
    ]},
    "0x2": {"size": 2, "fields": [
        {"off": 0, "kind": 2, "opt": 0, "nested": -1, "arr": 0, "min": 0, "width": 0, "extra": 0},
    ]},
}


class Encoder(unittest.TestCase):
    def test_defaults_and_bias(self):
        # absent optional (1 bit), enum 0 -> 1 (4 bits), i32 0 -> 0x80000000, count 0 (3 bits)
        bits = inv.encode(SCHEMA, 1)
        self.assertEqual(bits, bytes.fromhex("0c00000000"))

    def test_values_and_arrays(self):
        data = inv.encode(SCHEMA, 1, {0: 0x1122334455667788, 0x10: 2, 0x14: [{0: 1}, {0: 0}]})
        w = inv.BitWriter()
        w.put(1, 1); w.put(0x1122334455667788, 64); w.put(1, 4); w.put(0x80000000, 32)
        w.put(2, 3); w.put(1, 1); w.put(0, 1)
        self.assertEqual(data, w.data())
        with self.assertRaises(ValueError):
            inv.encode(SCHEMA, 1, {0x14: [{}]})

    def test_family_layout(self):
        o = inv.obj(0x88190FE1, 7, b"xy")
        self.assertEqual(o, struct.pack(">IQI", 0x88190FE1, 7, 2) + b"xy")
        fam = inv.family(4, 7, [o])
        self.assertEqual(fam[:21], struct.pack(">IQIBI", 4, 7, 1, 0, 1))
        self.assertEqual(inv.qz_payload([fam])[:4], b"\x00\x00\x00\x01")


if __name__ == "__main__":
    unittest.main()
