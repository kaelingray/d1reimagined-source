import unittest
import rpcs3_owned_memory as reader


class OwnedMemoryTest(unittest.TestCase):
    def test_identity_requires_unique_emulator_with_creation_time(self):
        good = {"tag": "rpcs3", "id": 12, "executable": "C:/runtime/rpcs3.exe",
                "startTimeUtcTicks": "639264680749309780"}
        self.assertEqual(reader.emulator_identity({"processes": [good]}), good)
        for processes in ([], [good, good], [{**good, "startTimeUtcTicks": None}],
                          [{**good, "executable": None}], [{**good, "id": 0}]):
            with self.subTest(processes=processes), self.assertRaises(ValueError):
                reader.emulator_identity({"processes": processes})

    def test_filetime_matches_dotnet_ownership_ticks(self):
        ticks = 639264680749309780
        filetime = ticks - reader.FILETIME_EPOCH_TICKS
        self.assertEqual(reader.creation_ticks(filetime >> 32, filetime & 0xFFFFFFFF), ticks)

    def test_read_rejects_guest_wraparound_and_closed_handle(self):
        memory = reader.OwnedMemory.__new__(reader.OwnedMemory)
        memory.handle = 1
        for address, size in ((0xFFFFFFFF, 2), (-1, 4), (0, -1), (0, 65 * 1024 * 1024)):
            with self.subTest(address=address, size=size), self.assertRaises(ValueError):
                memory.read(address, size)
        memory.handle = None
        with self.assertRaises(ValueError):
            memory.read(0, 4)
