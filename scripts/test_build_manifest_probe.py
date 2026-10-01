import json
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

    def test_inventory_excludes_stale_higher_number_and_unlisted_files(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            disc, update = root / "disc", root / "update"
            disc.mkdir()
            update.mkdir()
            package(disc / "ps3_investment_globals_012f_0.pkg", 0x12F, value=0)
            final = "ps3_investment_globals_012f_4.pkg"
            stale = "ps3_investment_globals_012f_5.pkg"
            package(update / final, 0x12F, value=4)
            package(update / stale, 0x12F, value=5)
            package(update / "ps3_investment_globals_012f_9.pkg", 0x12F, value=9)
            inventory = root / "inventory.json"
            inventory.write_text(json.dumps({
                "packages": {final: {"update": "01.23"}, stale: {"update": "01.19"}},
                "stale": [stale],
            }), encoding="utf-8")
            _, entries = probe.build(disc, [], "guid", update, inventory)
            self.assertEqual(entries, [(0x12F, b"ps3_investment_globals_012f_4", 4)])

    def test_missing_update_directory_fails_instead_of_building_disc_only_manifest(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            disc = root / "disc"
            disc.mkdir()
            package(disc / "ps3_globals_0151_0.pkg", 0x151, value=1)
            with self.assertRaisesRegex(ValueError, "update packages directory does not exist"):
                probe.build(disc, [], "guid", root / "missing-update")

    def test_inventory_without_update_directory_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "requires an update packages directory"):
            probe.build(Path("disc"), [], "guid", inventory_file=Path("inventory.json"))


if __name__ == "__main__":
    unittest.main()
