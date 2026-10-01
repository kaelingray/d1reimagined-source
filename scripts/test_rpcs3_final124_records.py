import struct
import unittest

import rpcs3_final124_records as reader


class ReplicationOccupancy(unittest.TestCase):
    def fixture(self):
        header = bytearray(0x40)
        for off, value in ((0, 0x3000), (8, 0x5000), (0x10, 4),
                           (0x14, 139), (0x18, 0x44), (0x1c, 0x48)):
            struct.pack_into(">I", header, off, value)
        values = {(0x1000, 4): struct.pack(">I", 0x2000), (0x2000, 0x40): header,
                  (0x2000, 4): struct.pack(">I", 0x3000),
                  (0x3008, 4): struct.pack(">I", 0x4000),
                  (0x4000, 12): struct.pack(">III", 4, 32, 0x4010),
                  (0x4010, 4): struct.pack(">I", 0xa)}
        for slot in (1, 3):
            raw = bytearray(0x44)
            struct.pack_into(">IBxHII", raw, 0, 0x12345678, 0xc, slot, 0xffffffff, 0x808022a5)
            raw[0x15] = int(slot == 3)
            struct.pack_into(">i", raw, 0x3c, -1)
            values[0x5000 + slot * 0x48, 0x44] = raw
        return values, lambda address, size: bytes(values[address, size])

    def test_sparse_occupied_slots_are_not_pool_word_14_records(self):
        _, read = self.fixture()
        result = reader.snapshot(read, 0x1000)
        self.assertEqual(result["occupied"], 2)
        self.assertEqual(result["pool_word_14"], 139)
        self.assertEqual(result["awaiting_baseline"], 1)
        self.assertEqual([r["pool_index"] for r in result["records"]], [1, 3])
        self.assertEqual(result["records"][1]["baseline"], "0x12345678/0xc/3")

    def test_empty_bitmap_is_not_a_filled_record_pool(self):
        values, read = self.fixture()
        values[0x4010, 4] = bytes(4)
        self.assertEqual(reader.snapshot(read, 0x1000)["records"], [])

    def test_invalid_occupied_header_and_changed_bitmap_are_rejected(self):
        values, read = self.fixture()
        values[0x5048, 0x44] = bytes(0x44)
        with self.assertRaisesRegex(ValueError, "invalid replication"):
            reader.snapshot(read, 0x1000)
        _, read = self.fixture()
        calls = 0
        def changing(address, size):
            nonlocal calls
            if address == 0x4010:
                calls += 1
                if calls > 1:
                    return bytes(size)
            return read(address, size)
        with self.assertRaisesRegex(ValueError, "changed"):
            reader.snapshot(changing, 0x1000)


if __name__ == "__main__":
    unittest.main()
