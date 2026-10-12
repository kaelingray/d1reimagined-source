import struct
import unittest

import rpcs3_final124_frames as reader


class FrameStatus(unittest.TestCase):
    def fixture(self):
        values = {(reader.GCM_CONTROL_DESCRIPTOR, 8): struct.pack(">II", 0x1000, 0x20000),
                  (0x1000, 12): reader.GCM_CONTROL_GETTER,
                  (0x18028, 4): struct.pack(">I", 0x3000),
                  (0x3010, 4): struct.pack(">I", 0x4000),
                  (0x4000, 12): struct.pack(">III", 0x4000100, 0x4000200, 0xffffffff),
                  (reader.FRAME_LABEL_PTR, 4): struct.pack(">I", 0x5000),
                  (0x5000, 4): struct.pack(">I", 0x12340106),
                  (reader.FRAME_INDEX, 4): struct.pack(">I", 0),
                  (0x34000200, 4): struct.pack(">I", 0x24000200)}
        return values, lambda address, size: values[address, size]

    def test_credit_wait_and_self_jump_are_observed_separately(self):
        values, read = self.fixture()
        result = reader.snapshot(read, (0x4000000, 0x34000000, 0x1000000))
        self.assertTrue(result["credit_wait_condition"])
        self.assertTrue(result["fifo_jump_to_self"])
        self.assertEqual(result["credit_delta"], 2)
        values[0x34000200, 4] = struct.pack(">I", 0x24000300)
        self.assertFalse(reader.snapshot(read, (0x4000000, 0x34000000, 0x1000000))["fifo_jump_to_self"])
        values[reader.FRAME_INDEX, 4] = struct.pack(">I", 1)
        self.assertFalse(reader.snapshot(read)["credit_wait_condition"])

    def test_native_signature_and_explicit_mapping_are_required(self):
        values, read = self.fixture()
        self.assertNotIn("fifo_jump_to_self", reader.snapshot(read))
        with self.assertRaisesRegex(ValueError, "outside"):
            reader.snapshot(read, (0, 0, 0x1000))
        values[0x1000, 12] = bytes(12)
        with self.assertRaisesRegex(ValueError, "signature"):
            reader.snapshot(read)


if __name__ == "__main__":
    unittest.main()
