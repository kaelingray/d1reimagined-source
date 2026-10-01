import struct
import unittest
from rpcs3_final124_activities import ACTIVITY_CACHE, activities


class ActivityTableTest(unittest.TestCase):
    def test_relative_definition_and_build_fields(self):
        blocks = {ACTIVITY_CACHE: struct.pack(">I", 0x1000),
                  0x1004: struct.pack(">II", 1, 0x20),
                  0x103c: struct.pack(">I", 0x100)}
        definition = bytearray(0x50)
        struct.pack_into(">I", definition, 0, 0xD03EC63D)
        struct.pack_into(">I", definition, 0x40, 0x80A585CC)
        struct.pack_into(">I", definition, 0x48, 1)
        blocks[0x113c] = definition
        result = activities(lambda address, size: bytes(blocks[address][:size]))
        self.assertEqual(result["count"], 1)
        self.assertEqual(result["activities"][0]["scenario"], "0x80a585cc")
        self.assertEqual(result["activities"][0]["required_level"], 1)

    def test_uninitialized_cache_is_not_an_empty_catalog(self):
        with self.assertRaisesRegex(ValueError, "not initialized"):
            activities(lambda address, size: bytes(size))
