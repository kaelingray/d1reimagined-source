import hashlib
import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1_pkg  # noqa: E402


def build_package(path, pkg_id=0x0123, blocks=(), entries=(), named=()):
    """A synthetic big-endian v24 package: blocks are (stored bytes, flags), entries
    (type, subtype, block, offset, size), named (hash, class, name)."""
    head = bytearray(0x1000)
    struct.pack_into(">HHHHQQIIHH", head, 0, 24, 4, pkg_id, 0, 0x1122334455667788, 0, 0, 0, 0, 1)
    head[0x24:0x24 + 9] = b"test tool"
    etab = b"".join(struct.pack(">IIQ", 0xffffffff, (t << 16) | s,
                                b | ((o >> 4) << 14) | (size << 28)) for t, s, b, o, size in entries)
    ntab = b"".join(struct.pack(">II", h, c) + n.encode().ljust(60, b"\0") for h, c, n in named)
    data_off = 0x1000 + len(etab) + len(ntab) + 32 * len(blocks)
    btab, payload = b"", b""
    for stored, flags in blocks:
        btab += struct.pack(">IIHH", data_off + len(payload), len(stored), 0, flags) + hashlib.sha1(stored).digest()
        payload += stored
    eoff, noff, boff = 0x1000, 0x1000 + len(etab), 0x1000 + len(etab) + len(ntab)
    for field, count, off, table in ((0xb4, len(entries), eoff, etab), (0xd0, len(blocks), boff, btab),
                                     (0xec, len(named), noff, ntab)):
        struct.pack_into(">II", head, field, count, off)
        head[field + 8:field + 28] = hashlib.sha1(table).digest() if count else bytes(20)
    body = bytes(head) + etab + ntab + btab + payload
    struct.pack_into(">I", head, 0x13c, len(body))
    Path(path).write_bytes(bytes(head) + etab + ntab + btab + payload)


class PackageTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = Path(self.dir.name) / "ps3_test_0123_0.pkg"
        first = bytes(range(256)) * (d1_pkg.BLOCK_SIZE // 256)
        second = b"tail of the entry" + bytes(100)
        build_package(self.path, blocks=[(first, 0), (second, 0), (b"\x8c\x06" + bytes(30), 0x100)],
                      entries=[(16, 0, 0, 0x20, 64), (8, 21, 0, d1_pkg.BLOCK_SIZE - 0x10, 0x21),
                               (32, 1, 2, 0, 50)],
                      named=[(d1_pkg.tag_hash(0x123, 0), 0x8080052e, "act_cosmo_m0_highway")])

    def tearDown(self):
        self.dir.cleanup()

    def test_header_and_tables(self):
        p = d1_pkg.Package(self.path)
        self.assertEqual((p.platform, p.pkg_id, p.language, p.reg_value), (4, 0x123, 1, 0x1122334455667788))
        self.assertEqual(p.tool, "test tool")
        for name in ("entries", "blocks", "named"):
            self.assertTrue(p.table_hash_ok(name))
        self.assertEqual(p.named[0]["name"], "act_cosmo_m0_highway")
        self.assertEqual((p.entries[1]["type"], p.entries[1]["subtype"]), (8, 21))

    def test_entry_within_and_across_blocks(self):
        p = d1_pkg.Package(self.path)
        self.assertEqual(p.entry_data(0), bytes(range(0x20, 0x60)))
        self.assertEqual(p.entry_data(1), bytes(range(0xf0, 0x100)) + b"tail of the entry")

    def test_compressed_block_needs_oodle(self):
        p = d1_pkg.Package(self.path)
        with self.assertRaisesRegex(RuntimeError, "Kraken"):
            p.entry_data(2)

    def test_tag_hash_round_trip(self):
        self.assertEqual(d1_pkg.tag_hash(0x5b, 0), 0x808b6000)  # PS4 study sample
        for pkg_id, index in ((0x10, 5), (0x3ff, 0x1fff), (0x400, 1), (0xbff, 42)):
            self.assertEqual(d1_pkg.split_tag_hash(d1_pkg.tag_hash(pkg_id, index)), (pkg_id, index))
        self.assertIsNone(d1_pkg.split_tag_hash(0x12345678))

    def test_parse_int(self):
        self.assertEqual(d1_pkg.parse_int("12"), 12)
        self.assertEqual(d1_pkg.parse_int("80800000"), 0x80800000)
        self.assertEqual(d1_pkg.parse_int("0x1f"), 0x1f)


if __name__ == "__main__":
    unittest.main()
