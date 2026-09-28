import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import game_ca_patch as g  # noqa: E402

PEM = "-----BEGIN CERTIFICATE-----\nMIIB\n-----END CERTIFICATE-----\n"


class GameCaPatch(unittest.TestCase):
    def test_01_00_sites(self):
        text = g.patch_yaml(PEM, unsigned_manifest=True)
        self.assertIn(g.PPU_HASH + ":", text)
        self.assertIn("[ utf8, 0x011b7218, ", text)
        self.assertIn(f"[ byte, 0x{0x011b7218 + len(PEM):08x}, 0x00 ]", text)
        self.assertIn("[ byte, 0x0179d5f4, 0x00 ]", text)
        self.assertTrue(g.config_yaml().startswith(g.PPU_HASH + ":"))

    def test_final_build_needs_its_hash(self):
        with self.assertRaises(ValueError):
            g.build_sites("01.24")
        with self.assertRaises(ValueError):
            g.build_sites("01.25", "PPU-00")
        sites = g.build_sites("01.24", "PPU-1234")
        text = g.patch_yaml(PEM, unsigned_manifest=True, sites=sites)
        self.assertTrue(text.split("\n")[2] == "PPU-1234:")
        self.assertIn("[ utf8, 0x012d3740, ", text)
        self.assertIn("[ byte, 0x018dca64, 0x00 ]", text)
        self.assertIn("PPU-1234:", g.config_yaml(True, sites))


if __name__ == "__main__":
    unittest.main()
