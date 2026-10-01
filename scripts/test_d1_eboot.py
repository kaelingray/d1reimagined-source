import struct
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1_eboot  # noqa: E402

CODE, DATA, TOC = 0x10000, 0x20000, 0x28400


def lis(rd, hi): return (15 << 26) | (rd << 21) | (hi & 0xffff)
def addic(rd, ra, v): return (12 << 26) | (rd << 21) | (ra << 16) | (v & 0xffff)
def ori(ra, rs, v): return (24 << 26) | (rs << 21) | (ra << 16) | (v & 0xffff)
def lwz(rd, ra, v): return (32 << 26) | (rd << 21) | (ra << 16) | (v & 0xffff)
def addi(rd, ra, v): return (14 << 26) | (rd << 21) | (ra << 16) | (v & 0xffff)
def bl(pc, target): return (18 << 26) | ((target - pc) & 0x3fffffc) | 1
NOP, BLR = 0x60000000, 0x4e800020


def synthetic_eboot(words, slots=(), toc=TOC):
    """Small code segment and independent 32-bit pointer slots, without a private ELF."""
    eb = d1_eboot.Eboot.__new__(d1_eboot.Eboot)
    eb.words = np.array(words, dtype=np.uint32)
    eb.code_va, eb.toc = CODE, toc
    code = b"".join(struct.pack(">I", word) for word in words)
    eb.data = code
    eb.segments = [(CODE, 0, len(code), 5)]
    for address, target in slots:
        eb.segments.append((address, len(eb.data), 4, 6))
        eb.data += struct.pack(">I", target)
    return eb


def build(path):
    s1, s2 = DATA + 0x10, DATA + 0x8010  # s2's low half has bit 15 set
    code = [
        lis(3, (s1 + 0x8000) >> 16), addic(3, 3, s1), bl(CODE + 8, CODE + 0x40), BLR,   # f0: refs s1
        lis(4, (s2 + 0x8000) >> 16), NOP, addic(5, 4, s2 & 0xffff), BLR,                 # refs s2 signed
        lis(6, s1 >> 16), ori(6, 6, s1 & 0xffff), BLR, NOP,                             # refs s1 via ori
        lis(7, (s1 + 0x8000) >> 16), addi(7, 0, 5), addic(8, 7, s1), BLR,              # r7 overwritten
        lwz(9, 2, (TOC + 0x10 - TOC)), bl(CODE + 0x44, CODE + 0x40), BLR, NOP,          # f @0x40: TOC ref
        0x7C0802A6, BLR,                                  # @0x50: pointer-only function (mflr r0)
    ]
    code_bytes = b"".join(struct.pack(">I", w) for w in code)
    data = bytearray(0x9000)
    data[0x10:0x10 + 15] = b"objective test\0"
    data[0x8010:0x8010 + 12] = b"second one\0\0"
    struct.pack_into(">I", data, TOC + 0x10 - DATA, s1)          # TOC slot -> s1
    struct.pack_into(">II", data, 0x100, CODE + 0x10, d1_eboot.TOC_BASE)  # an .opd-style pair
    ehdr = bytearray(0x40)
    ehdr[:6] = b"\x7fELF\x02\x02"
    struct.pack_into(">QHH", ehdr, 0x20, 0x40, 0, 0)
    struct.pack_into(">HH", ehdr, 0x36, 56, 2)
    ph = b""
    code_off, data_off = 0x100, 0x100 + len(code_bytes)
    for vaddr, off, size, flags in ((CODE, code_off, len(code_bytes), 5), (DATA, data_off, len(data), 6)):
        ph += struct.pack(">IIQQQQQQ", 1, flags, off, vaddr, vaddr, size, size, 0x10)
    blob = bytes(ehdr) + ph
    blob += bytes(code_off - len(blob)) + code_bytes + bytes(data)
    Path(path).write_bytes(blob)
    return s1, s2


class EbootTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = Path(self.dir.name) / "EBOOT.elf"
        self.s1, self.s2 = build(self.path)
        self.eb = d1_eboot.Eboot(self.path, toc=TOC)

    def tearDown(self):
        self.dir.cleanup()

    def test_strings(self):
        found = dict(self.eb.strings(6))
        self.assertEqual(found[self.s1], "objective test")
        self.assertEqual([va for va, _ in self.eb.find_string("test")], [self.s1])

    def test_code_refs_signed_ori_and_kill(self):
        refs = self.eb.code_refs(self.s1)
        self.assertIn((CODE + 4, CODE), refs)            # lis + addic
        self.assertIn((CODE + 0x24, CODE + 0x20), refs)  # lis + ori
        self.assertNotIn(CODE + 0x38, [pc for pc, _ in refs])  # r7 was overwritten first
        self.assertEqual(self.eb.code_refs(self.s2), [(CODE + 0x18, CODE + 0x10)])

    def test_toc_and_data_refs(self):
        self.assertEqual(self.eb.toc_refs(self.s1), [(CODE + 0x40, TOC + 0x10)])
        self.assertEqual(self.eb.data_refs(self.s1), [TOC + 0x10])

    def test_toc_refs_decode_ds_subtypes_and_signed_offsets(self):
        # lwz/ld/ldu/lwa at TOC+16 and TOC-8, followed by reserved ld subtypes,
        # a store and a valid load through a different base register.
        positive = [0x80620010, 0xE8620010, 0xE8620011, 0xE8620012]
        negative = [0x8062FFF8, 0xE862FFF8, 0xE862FFF9, 0xE862FFFA]
        excluded = [0xE8620013, 0xE862FFFB, 0xF8620010, 0xE8630010]
        eb = synthetic_eboot(positive + negative + excluded,
                             [(TOC + 16, self.s1), (TOC - 8, self.s2)])
        self.assertEqual(eb.toc_refs(self.s1), [(CODE + 4 * i, TOC + 16) for i in range(4)])
        self.assertEqual(eb.toc_refs(self.s2), [(CODE + 4 * i, TOC - 8) for i in range(4, 8)])

    def test_toc_refs_signed_reach_excludes_wrapped_slots(self):
        words = [0x80628000, 0xE8628002, 0x80627FFC, 0xE8627FFD]
        eb = synthetic_eboot(words, [(TOC - 0x8000, self.s1), (TOC + 0x7FFC, self.s2)])
        self.assertEqual(eb.toc_refs(self.s1), [(CODE, TOC - 0x8000), (CODE + 4, TOC - 0x8000)])
        self.assertEqual(eb.toc_refs(self.s2), [(CODE + 8, TOC + 0x7FFC), (CODE + 12, TOC + 0x7FFC)])
        wrapped = synthetic_eboot(words, [(TOC + 0x8000, self.s1), (TOC - 0x8004, self.s2)])
        self.assertEqual(wrapped.toc_refs(self.s1), [])
        self.assertEqual(wrapped.toc_refs(self.s2), [])

    def test_code_refs_decode_ds_subtypes_and_reject_reserved_forms(self):
        for target in (0x247CC, 0x2FFF8):
            lo = target & 0xFFFF
            for opcode, subtypes in ((58, range(4)), (62, range(4))):
                for subtype in subtypes:
                    with self.subTest(target=hex(target), opcode=opcode, subtype=subtype):
                        word = (opcode << 26) | (4 << 21) | (3 << 16) | lo | subtype
                        eb = synthetic_eboot([lis(3, (target + 0x8000) >> 16), word, BLR])
                        valid = subtype < (3 if opcode == 58 else 2)
                        self.assertEqual(eb.code_refs(target), [(CODE + 4, CODE)] if valid else [])

    def test_calls_and_functions(self):
        self.assertEqual(self.eb.callers(CODE + 0x40), [CODE + 8, CODE + 0x44])
        self.assertEqual(self.eb.function_of(CODE + 0x48)[0], CODE + 0x40)
        self.assertEqual(self.eb.function_of(CODE + 0x54)[0], CODE + 0x50)  # prologue after nop

    def test_disasm_ppc64_memory_instructions(self):
        try:
            import capstone  # noqa: F401
        except ImportError:
            self.skipTest("capstone not installed")
        # Test the public disassembler with PPC64 DS forms and a signed displacement.
        raw = bytes.fromhex("e86447cce86447cde86447cef864fff9")
        eb = d1_eboot.Eboot.__new__(d1_eboot.Eboot)
        eb.data = raw
        eb.segments = [(CODE, 0, len(raw), 5)]
        eb.names = {}
        lines = list(eb.disasm(CODE, 4))
        self.assertEqual([line.split()[2] for line in lines], ["ld", "ldu", "lwa", "stdu"])
        self.assertIn("0x47cc(r4)", lines[2])
        self.assertIn("-8(r4)", lines[3])


if __name__ == "__main__":
    unittest.main()
