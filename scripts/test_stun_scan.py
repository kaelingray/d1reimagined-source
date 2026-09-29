import sys
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import stun_scan  # noqa: E402


class StunScanTest(unittest.TestCase):
    """The scanners are pure word-array searches, so they are tested on a synthetic code
    segment rather than on a decrypted EBOOT (which stays in the private `local\\` folder)."""

    BASE = 0x10000

    def words(self, *values):
        return np.array(values, dtype=np.uint32)

    def test_find_li_matches_the_opcode_and_addresses_the_word_index(self):
        # li r3, 0x1d  ->  0x3800001d ; li r5, 0x0e  ->  0x3800000e
        words = self.words(0x3800001D, 0x60000000, 0x3800000E)
        self.assertEqual(stun_scan.find_li(words, self.BASE, 0x1D), [0x10000])
        self.assertEqual(stun_scan.find_li(words, self.BASE, 0x0E), [0x10008])

    def test_find_word_is_an_exact_32_bit_match(self):
        words = self.words(0x01000100, 0x00010001)
        self.assertEqual(stun_scan.find_word(words, self.BASE, 0x01000100), [0x10000])
        self.assertEqual(stun_scan.find_word(words, self.BASE, 0x0BFA), [])

    def test_find_cmpwi_uses_the_low_immediate_bits(self):
        # capstone: 0x2c030004 -> cmpwi r3, 4 ; 0x2c850005 -> cmpwi cr1, r5, 5 ;
        # 0x2f040010 -> cmpwi cr6, r4, 0x10. The immediate is bits 0-15 and the CR
        # field is bits 6-10, so the same immediate must be found for any register/CR.
        words = self.words(0x2C03001E, 0x2C05000E, 0x2F04001E)
        self.assertEqual(stun_scan.find_cmpwi(words, self.BASE, 0x1E), [0x10000, 0x10008])
        self.assertEqual(stun_scan.find_cmpwi(words, self.BASE, 0x0E), [0x10004])
        self.assertEqual(stun_scan.find_cmpwi(words, self.BASE, 0x14), [])

    def test_find_cmpwi_agrees_with_capstone(self):
        """The word patterns come from the real EBOOT, so check them against capstone."""
        try:
            import capstone
        except ImportError:  # capstone is only needed for disasm, per requirements.txt
            self.skipTest("capstone not installed")
        md = capstone.Cs(capstone.CS_ARCH_PPC, capstone.CS_MODE_64 | capstone.CS_MODE_BIG_ENDIAN)
        for word in (0x2C030004, 0x2C850005, 0x2F040010, 0x2C1E0000, 0x2C0E0000):
            ins = next(md.disasm(word.to_bytes(4, "big"), self.BASE))
            self.assertEqual(ins.mnemonic, "cmpwi")
            immediate = int(ins.op_str.rsplit(",", 1)[1].strip(), 0)
            self.assertEqual(stun_scan.find_cmpwi(self.words(word), self.BASE, immediate), [self.BASE])

    def test_report_counts_and_filters_by_range(self):
        import io
        import contextlib

        # 0x10000 and 0x900000: only the first is inside the default Demonware range.
        addresses = [0x10000, 0x900000]
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            stun_scan.report("li 1", addresses, 0xA00000, 0xC00000)
        text = out.getvalue()
        self.assertIn("2 total, 0 in", text)
        self.assertNotIn("0x00010000", text)

        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            stun_scan.report("li 1", addresses, 0x10000, 0xC00000)
        self.assertIn("0x00010000", out.getvalue())


if __name__ == "__main__":
    unittest.main()
