import struct
import unittest

import rpcs3_final124_transition as reader


class TransitionTests(unittest.TestCase):
    def fixture(self, target=112, slot=0):
        engine, table = 0x3073E780, 0x60155C80
        fields, groups, slices = bytearray(0x14C), bytearray(0x1B8), bytearray(0x48)
        fields[:4] = bytes([4, 6, 4, 0])
        struct.pack_into(">iIi", fields, 0x90 - 0x88, -1, 0x811C9DC5, -1)
        struct.pack_into(">iIi", fields, 0x130 - 0x88, target, 0x811C9DC5, target)
        fields[0x1D0 - 0x88] = 2
        struct.pack_into(">Ii", groups, 0, 3, slot)
        struct.pack_into(">i", groups, 8, 113)
        struct.pack_into(">i", groups, 0x490 - 0x3B0, -1)
        groups[8 + 8:8 + 11] = bytes([255, 1, 0])
        groups[8 + 0x38] = 2
        struct.pack_into(">ii", slices, 0, -1, -1)
        struct.pack_into(">I", slices, 8 + 3 * 4, 1 << 16)
        values = {(reader.ENGINE_PTR, 4): struct.pack(">I", engine),
                  (reader.SLICE_TABLE_PTR, 4): struct.pack(">I", table),
                  (engine + 0x88, len(fields)): fields,
                  (engine + 0x3B0, len(groups)): groups,
                  (table + 0x110C, len(slices)): slices}
        return values, lambda address, size: bytes(values[address, size])

    def test_loaded_target_without_active_slice_is_preserved(self):
        _, read = self.fixture()
        result = reader.snapshot(read)
        self.assertEqual(result["slices"], {"active": -1, "loading": -1, "target_loaded": True})
        self.assertEqual(result["target"]["slice_set"], 112)
        self.assertEqual(result["cached_world_version"], 3)
        self.assertEqual(result["group_current"]["local_peer_byte"], 2)
        self.assertEqual(result["group_current"]["sequence"], -1)
        self.assertEqual(result["group_switch"]["slice_set"], -1)
        self.assertTrue(result["fields_unchanged_during_sample"])
        self.assertFalse(result["atomic"])

    def test_invalid_target_and_peer_are_not_used_as_indexes(self):
        for target, slot in ((-1, -1), (512, 20)):
            with self.subTest(target=target, slot=slot):
                _, read = self.fixture(target, slot)
                result = reader.snapshot(read)
                self.assertIsNone(result["slices"]["target_loaded"])
                self.assertIsNone(result["group_current"]["local_peer_byte"])

    def test_changed_group_is_not_accepted_as_unchanged(self):
        values, fixture = self.fixture()
        calls = 0
        def read(address, size):
            nonlocal calls
            if size == 0x1B8:
                calls += 1
                if calls == 2:
                    values[address, size][0x490 - 0x3B0 + 3] = 112
            return fixture(address, size)
        self.assertFalse(reader.snapshot(read)["fields_unchanged_during_sample"])

    def test_null_engine_does_not_follow_other_pointers(self):
        def read(address, size):
            self.assertEqual((address, size), (reader.ENGINE_PTR, 4))
            return bytes(4)
        self.assertFalse(reader.snapshot(read)["initialized"])

    def test_short_and_overflowing_reads_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "short"):
            reader.snapshot(lambda address, size: bytes(size - 1))
        with self.assertRaisesRegex(ValueError, "range"):
            reader.snapshot(lambda address, size: struct.pack(">I", 0xFFFFFFF0))


if __name__ == "__main__":
    unittest.main()
