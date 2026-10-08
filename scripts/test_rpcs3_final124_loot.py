import math
import struct
import unittest

import rpcs3_final124_loot as loot


class LootSnapshotTest(unittest.TestCase):
    def setUp(self):
        self.manager, self.character, self.character_id = 0x1000, 0x10000, 0xA3D000000000001
        self.controller, self.world_handle = 0x20000, 0xABCD0000 | (7 << 3)
        self.raw = bytearray(loot.MANAGER_SIZE)
        struct.pack_into(">III", self.raw, 0x200, 64, 63, 2)
        struct.pack_into(">I", self.raw, 0x1A10, 0x12345678)  # signature is not the world ID
        self.records = bytearray(64 * 0x38)
        for i in range(64):
            struct.pack_into(">h", self.records, i * 0x38, -1)
        struct.pack_into(">h", self.records, 0, 3598)
        self.records[8] = 3
        struct.pack_into(">i", self.records, 0x10, 123)
        struct.pack_into(">Q", self.records, 0x18, 456)
        self.cache(63, 123, 456, 7, (410.6, -794.4, 17.7, 1))
        self.cache(0, 987, 654, 7, (10, 20, 30, 1))

    def cache(self, index, key, identifier, world, position, slot=False):
        offset = (loot.SLOT_CACHE_OFFSET if slot else loot.CACHE_OFFSET) + index * loot.CACHE_STRIDE
        struct.pack_into(">i", self.raw, offset, key)
        struct.pack_into(">Q", self.raw, offset + 8, identifier)
        struct.pack_into(">4f", self.raw, offset + 0x10, *position)
        struct.pack_into(">i", self.raw, offset + 0x20, world)

    def read(self, address, size):
        if address == loot.MANAGER_POINTER:
            return struct.pack(">I", self.manager)
        if address == loot.WORLD_CONTROLLER_POINTER:
            return struct.pack(">I", self.controller)
        if address == self.controller + 0x110C:
            return struct.pack(">I", self.world_handle)
        if address == self.character:
            return struct.pack(">Q", self.character_id)
        if address == self.manager:
            return bytes(self.raw[:size])
        if address == self.character + loot.LOOT_OFFSET:
            return bytes(self.records[:size])
        self.fail(f"unexpected read {address:#x}/{size:#x}")

    def test_ring_wrap_and_exact_nonce_pair(self):
        result = loot.snapshot(self.read, self.character, self.character_id)
        self.assertEqual([c["cache_index"] for c in result["cache"]], [63, 0])
        rifle, = result["records"]
        self.assertEqual(rifle["item_index"], 3598)
        self.assertEqual([m["cache_index"] for m in rifle["ring_matches"]], [63])
        self.assertTrue(rifle["world_spawn_source"])
        self.assertTrue(rifle["position_matches_current_world"])
        self.cache(63, 123, 457, 7, (1, 2, 3, 1))
        self.assertFalse(loot.snapshot(self.read, self.character, self.character_id)["records"][0]
                         ["position_matches_current_world"])

    def test_newest_ring_match_precedes_slot_fallback(self):
        self.cache(0, 123, 456, 8, (1, 2, 3, 1))
        self.cache(0, 123, 456, 7, (4, 5, 6, 1), slot=True)
        rifle, = loot.snapshot(self.read, self.character, self.character_id)["records"]
        self.assertEqual([m["cache_index"] for m in rifle["ring_matches"]], [0, 63])
        self.assertFalse(rifle["position_matches_current_world"])
        struct.pack_into(">I", self.raw, 0x208, 0)
        rifle, = loot.snapshot(self.read, self.character, self.character_id)["records"]
        self.assertEqual(rifle["ring_matches"], [])
        self.assertTrue(rifle["position_matches_current_world"])

    def test_bad_identity_bounds_and_position_fail_closed(self):
        with self.assertRaisesRegex(ValueError, "identity"):
            loot.snapshot(self.read, self.character, self.character_id + 1)
        struct.pack_into(">I", self.raw, 0x208, 65)
        with self.assertRaisesRegex(ValueError, "bounds"):
            loot.snapshot(self.read, self.character, self.character_id)
        struct.pack_into(">I", self.raw, 0x208, 2)
        self.cache(63, 123, 456, 7, (math.nan, 2, 3, 1))
        with self.assertRaisesRegex(ValueError, "non-finite"):
            loot.snapshot(self.read, self.character, self.character_id)

    def test_mutating_input_rejects_observation(self):
        reads = 0

        def changing_read(address, size):
            nonlocal reads
            if address == self.character + loot.LOOT_OFFSET:
                reads += 1
                if reads == 2:
                    self.records[8] = 5
            return self.read(address, size)

        with self.assertRaisesRegex(ValueError, "changed during"):
            loot.snapshot(changing_read, self.character, self.character_id)

    def test_absent_world_and_nonworld_source_are_not_spawn_evidence(self):
        self.world_handle = 0xFFFFFFFF
        self.records[8] = 2
        result = loot.snapshot(self.read, self.character, self.character_id)
        self.assertEqual(result["current_world_id"], -1)
        self.assertFalse(result["records"][0]["world_spawn_source"])
        self.assertFalse(result["records"][0]["position_matches_current_world"])


if __name__ == "__main__":
    unittest.main()
