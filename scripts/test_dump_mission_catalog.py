import struct
import unittest
import dump_mission_catalog as catalog


class ScenarioHeaderTest(unittest.TestCase):
    def test_final_scenario_checksum_and_bubbles(self):
        data = bytearray(0x80)
        struct.pack_into(">III", data, 0, len(data), 0x992BB471, 0x8FE07299)
        struct.pack_into(">I", data, 0x34, 22)
        self.assertEqual(catalog.scenario_header(data), {
            "entity_group": "0x992bb471", "checksum": "0x8fe07299", "bubble_count": 22})

    def test_truncated_and_wrong_layout_are_rejected(self):
        for data in (b"", bytes(0x38), struct.pack(">I", 5) + bytes(0x40)):
            with self.subTest(data=data), self.assertRaises(ValueError):
                catalog.scenario_header(data)
