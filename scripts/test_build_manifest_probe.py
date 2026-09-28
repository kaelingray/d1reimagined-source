import struct
import tempfile
import unittest
from pathlib import Path

import build_manifest_probe as probe


def package(path, package_id, marker=1, value=0):
    path.write_bytes(struct.pack(">4HQ", 0x18, 4, package_id, marker, value) + b"body")


class ManifestProbeTests(unittest.TestCase):
    def test_latest_patchable_update_and_new_id_are_listed(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            disc, update = root / "disc", root / "update"
            disc.mkdir()
            update.mkdir()
            package(disc / "ps3_globals_0151_0.pkg", 0x151, value=1)
            package(update / "ps3_globals_0151_1.pkg", 0x151, value=2)
            package(update / "ps3_globals_0151_4.pkg", 0x151, value=4)
            package(update / "ps3_extra_0200_1.pkg", 0x200, value=8)
            package(update / "ps3_shared_manifest_0430_1.pkg", 0x430, marker=0)
            _, entries = probe.build(disc, [], "guid", update)
            self.assertEqual(entries, [(0x151, b"ps3_globals_0151_4", 4),
                                       (0x200, b"ps3_extra_0200_1", 8)])


if __name__ == "__main__":
    unittest.main()
