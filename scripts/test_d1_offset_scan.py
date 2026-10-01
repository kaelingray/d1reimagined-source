import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1_offset_scan  # noqa: E402


BASE = 0x10000
OFFSET = 0x47CC
# Encoded r3/f3, displacement(r4). Keep known instruction bytes here so the
# tests do not reconstruct the scanner's opcode masks to generate expectations.
ACCESS_WORDS = {
    "lwz": 0x806447CC, "lwzu": 0x846447CC,
    "lbz": 0x886447CC, "lbzu": 0x8C6447CC,
    "stw": 0x906447CC, "stwu": 0x946447CC,
    "stb": 0x986447CC, "stbu": 0x9C6447CC,
    "lhz": 0xA06447CC, "lhzu": 0xA46447CC,
    "lha": 0xA86447CC, "lhau": 0xAC6447CC,
    "sth": 0xB06447CC, "sthu": 0xB46447CC,
    "lmw": 0xB86447CC, "stmw": 0xBC6447CC,
    "lfs": 0xC06447CC, "lfsu": 0xC46447CC,
    "lfd": 0xC86447CC, "lfdu": 0xCC6447CC,
    "stfs": 0xD06447CC, "stfsu": 0xD46447CC,
    "stfd": 0xD86447CC, "stfdu": 0xDC6447CC,
    "ld": 0xE86447CC, "ldu": 0xE86447CD, "lwa": 0xE86447CE,
    "std": 0xF86447CC, "stdu": 0xF86447CD,
}


def eboot(*words):
    return SimpleNamespace(code_va=BASE, words=np.array(words, dtype=np.uint32))


class OffsetScanTest(unittest.TestCase):
    def test_finds_integer_float_and_multiple_register_accesses(self):
        eb = eboot(*ACCESS_WORDS.values())
        self.assertEqual(d1_offset_scan.scan(eb, OFFSET),
                         [BASE + 4 * i for i in range(len(ACCESS_WORDS))])

    def test_ds_subtype_bits_are_not_part_of_displacement(self):
        eb = eboot(0xE86447CC, 0xE86447CD, 0xE86447CE,
                   0xF86447CC, 0xF86447CD, 0x806447CD)
        self.assertEqual(d1_offset_scan.scan(eb, OFFSET),
                         [BASE + 4 * i for i in range(5)])
        self.assertEqual(d1_offset_scan.scan(eb, OFFSET + 1), [BASE + 20])
        self.assertEqual(d1_offset_scan.scan(eb, OFFSET + 2), [])

    def test_excludes_reserved_ds_subtypes(self):
        eb = eboot(0xE86447CF, 0xF86447CE, 0xF86447CF)
        self.assertEqual(d1_offset_scan.scan(eb, OFFSET), [])
        self.assertEqual(d1_offset_scan.scan(eb, OFFSET + 3), [])

    def test_excludes_other_immediates_and_indexed_accesses(self):
        eb = eboot(0x386447CC, 0x606447CC, 0x480047CC)
        self.assertEqual(d1_offset_scan.scan(eb, OFFSET), [])
        # lhax, stwx, ldarx, stdcx. have register indexes, despite the old
        # mnemonic allow-list classifying them as displacement accesses.
        for word in (0x7C6422AE, 0x7C64212E, 0x7C6420A8, 0x7C6421AD):
            with self.subTest(word=hex(word)):
                self.assertEqual(d1_offset_scan.scan(eboot(word), word & 0xFFFF), [])

    def test_signed_and_encoded_negative_offsets_match(self):
        eb = eboot(0x8064FFF8, 0xE864FFF8, 0xE864FFF9,
                   0xE864FFFA, 0xF864FFF8, 0xF864FFF9)
        expected = [BASE + 4 * i for i in range(6)]
        self.assertEqual(d1_offset_scan.scan(eb, -8), expected)
        self.assertEqual(d1_offset_scan.scan(eb, 0xFFF8), expected)

    def test_displacement_boundaries_and_no_modulo_wrap(self):
        eb = eboot(0x80648000, 0xE8648002, 0x80647FFF,
                   0x8064FFFF, 0xF8640001)
        self.assertEqual(d1_offset_scan.scan(eb, -0x8000), [BASE, BASE + 4])
        self.assertEqual(d1_offset_scan.scan(eb, 0x8000), [BASE, BASE + 4])
        self.assertEqual(d1_offset_scan.scan(eb, 0x7FFF), [BASE + 8])
        self.assertEqual(d1_offset_scan.scan(eb, -1), [BASE + 12])
        self.assertEqual(d1_offset_scan.scan(eb, 0), [BASE + 16])
        for offset in (-0x8001, 0x10000, 0x147CC):
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                d1_offset_scan.scan(eb, offset)

    def test_range_uses_inclusive_low_and_exclusive_high(self):
        eb = eboot(0x806447CC, 0x60000000, 0xE86447CE, 0xF86447CD)
        self.assertEqual(d1_offset_scan.scan(eb, OFFSET, BASE + 8, BASE + 12), [BASE + 8])
        self.assertEqual(d1_offset_scan.scan(eb, OFFSET, hi=BASE + 8), [BASE])
        self.assertEqual(d1_offset_scan.scan(eb, OFFSET, lo=BASE + 12), [BASE + 12])
        self.assertEqual(d1_offset_scan.scan(eb, OFFSET, BASE + 16, BASE + 16), [])
        self.assertEqual(d1_offset_scan.scan(eboot(), OFFSET), [])

    def test_known_instruction_words_agree_with_capstone(self):
        try:
            import capstone
        except ImportError:
            self.skipTest("capstone not installed")
        md = capstone.Cs(capstone.CS_ARCH_PPC,
                         capstone.CS_MODE_64 | capstone.CS_MODE_BIG_ENDIAN)
        md.detail = True
        for mnemonic, word in ACCESS_WORDS.items():
            with self.subTest(mnemonic=mnemonic):
                ins = next(md.disasm(word.to_bytes(4, "big"), BASE))
                self.assertEqual(ins.mnemonic, mnemonic)
                memory = [op.mem for op in ins.operands if op.type == capstone.ppc.PPC_OP_MEM]
                self.assertEqual([op.disp for op in memory], [OFFSET])


if __name__ == "__main__":
    unittest.main()
