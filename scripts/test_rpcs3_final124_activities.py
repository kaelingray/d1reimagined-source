import struct
import unittest
from rpcs3_final124_activities import ACTIVITY_CACHE, activities, activity_blob


class ActivityTableTest(unittest.TestCase):
    def test_relative_definition_and_build_fields(self):
        blocks = {ACTIVITY_CACHE: struct.pack(">I", 0x1000),
                  0x1004: struct.pack(">II", 1, 0x20),
                  0x103c: struct.pack(">I", 0x100)}
        definition = bytearray(0x50)
        struct.pack_into(">I", definition, 0, 0xD03EC63D)
        struct.pack_into(">I", definition, 0x44, 0x80A585CC)
        struct.pack_into(">I", definition, 0x48, 1)
        blocks[0x113c] = definition
        result = activities(lambda address, size: bytes(blocks[address][:size]))
        self.assertEqual(result["count"], 1)
        self.assertEqual(result["activities"][0]["tag_44"], "0x80a585cc")
        self.assertEqual(result["activities"][0]["required_level"], 1)

    def test_uninitialized_cache_is_not_an_empty_catalog(self):
        with self.assertRaisesRegex(ValueError, "not initialized"):
            activities(lambda address, size: bytes(size))

    def test_static_offsets_and_bounds(self):
        blob = bytearray(0x100)
        struct.pack_into(">II", blob, 4, 1, 0x20)
        struct.pack_into(">I", blob, 0x3c, 0x40)
        struct.pack_into(">I", blob, 0x7c, 0xD03EC63D)
        struct.pack_into(">I", blob, 0x7c + 0x44, 0x80A585CC)
        struct.pack_into(">I", blob, 0x7c + 0x48, 1)
        row = activity_blob(blob)["activities"][0]
        self.assertEqual(row["offset"], "0x0000007c")
        self.assertEqual(row["hash"], "0xd03ec63d")
        self.assertEqual(row["required_level"], 1)
        with self.assertRaisesRegex(ValueError, "outside"):
            activity_blob(blob[:0x90])
