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

    def test_chains_register_block_patch_files_oldest_first_and_shared_manifests(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            disc, update = root / "disc", root / "update"
            disc.mkdir()
            update.mkdir()
            # 0172: overlay _2 keeps blocks in _0 (disc) and _1; _3 is stale (inventory)
            tiger(disc / "ps3_client_bootstrap_patchabl_0172_0.pkg", 0x172, 0, [0], value=10)
            tiger(update / "ps3_client_bootstrap_patchabl_0172_1.pkg", 0x172, 1, [0, 1], value=11)
            tiger(update / "ps3_client_bootstrap_patchabl_0172_2.pkg", 0x172, 2, [0, 1, 2], value=12)
            tiger(update / "ps3_client_bootstrap_patchabl_0172_3.pkg", 0x172, 3, [0, 3], value=13)
            # shared manifest (marker 0): disc _0, update _4 overlay with blocks in _1
            tiger(disc / "ps3_shared_manifest_0430_0.pkg", 0x430, 0, [0], marker=0, value=20)
            tiger(update / "ps3_shared_manifest_0430_1.pkg", 0x430, 1, [1], marker=0, value=21)
            tiger(update / "ps3_shared_manifest_0430_4.pkg", 0x430, 4, [1, 4], marker=0, value=24)
            names = ["ps3_client_bootstrap_patchabl_0172_1.pkg", "ps3_client_bootstrap_patchabl_0172_2.pkg",
                     "ps3_client_bootstrap_patchabl_0172_3.pkg", "ps3_shared_manifest_0430_1.pkg",
                     "ps3_shared_manifest_0430_4.pkg"]
            inventory = root / "inventory.json"
            inventory.write_text(json.dumps({"packages": {n: {} for n in names},
                                             "stale": ["ps3_client_bootstrap_patchabl_0172_3.pkg"]}),
                                 encoding="utf-8")
            _, plain = probe.build(disc, [], "guid", update, inventory)
            self.assertEqual([e[1] for e in plain], [b"ps3_client_bootstrap_patchabl_0172_2",
                                                     b"ps3_shared_manifest_0430_0"])
            _, entries = probe.build(disc, [], "guid", update, inventory, chains=True)
            self.assertEqual(entries, [
                (0x172, b"ps3_client_bootstrap_patchabl_0172_0", 10),
                (0x172, b"ps3_client_bootstrap_patchabl_0172_1", 11),
                (0x172, b"ps3_client_bootstrap_patchabl_0172_2", 12),
                (0x430, b"ps3_shared_manifest_0430_1", 21),
                (0x430, b"ps3_shared_manifest_0430_4", 24),
            ])

    def test_chains_fail_when_a_block_patch_file_is_missing(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            disc, update = root / "disc", root / "update"
            disc.mkdir()
            update.mkdir()
            tiger(disc / "ps3_globals_0151_0.pkg", 0x151, 0, [0])
            tiger(update / "ps3_globals_0151_4.pkg", 0x151, 4, [0, 2, 4])
            with self.assertRaisesRegex(ValueError, r"patch 2, but ps3_globals_0151_2\.pkg is not available"):
                probe.build(disc, [], "guid", update, chains=True)


def tiger(path, package_id, patch, block_patches, marker=1, value=0):
    """Minimal v24 header (+0x20 patch id, block table at +0xd0) with one block per patch id."""
    header = bytearray(0x100)
    struct.pack_into(">4HQ", header, 0, 0x18, 4, package_id, marker, value)
    struct.pack_into(">H", header, 0x20, patch)
    struct.pack_into(">II", header, 0xD0, len(block_patches), 0x100)
    blocks = b"".join(struct.pack(">IIHH", 0, 0, p, 0) + bytes(20) for p in block_patches)
    path.write_bytes(bytes(header) + blocks)


if __name__ == "__main__":
    unittest.main()
