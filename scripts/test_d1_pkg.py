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

    def test_mapped_package_matches_normal_extraction(self):
        p = d1_pkg.Package(self.path, mapped=True)
        try:
            self.assertEqual(p.entry_data(1), d1_pkg.Package(self.path).entry_data(1))
            self.assertTrue(p.table_hash_ok("entries"))
        finally:
            p.data.close()

    def test_entry_within_and_across_blocks(self):
        p = d1_pkg.Package(self.path)
        self.assertEqual(p.entry_data(0), bytes(range(0x20, 0x60)))
        self.assertEqual(p.entry_data(1), bytes(range(0xf0, 0x100)) + b"tail of the entry")

    def test_compressed_block_needs_oodle(self):
        p = d1_pkg.Package(self.path)
        with self.assertRaisesRegex(RuntimeError, "Kraken"):
            p.entry_data(2)

    def test_compressed_blocks_decode_to_their_used_size(self):
        p = d1_pkg.Package(self.path)
        self.assertEqual(p.raw_size(0), d1_pkg.BLOCK_SIZE)  # entry 1 runs to the block's end
        self.assertEqual(p.raw_size(1), 0x4000)  # 17 bytes of entry 1
        self.assertEqual(p.raw_size(2), 0x4000)  # entry 2: 50 bytes

        class FakeOodle:
            sizes = []

            def decompress(self, raw, size):
                self.sizes.append(size)
                return b"z" * size

        oodle = FakeOodle()
        self.assertEqual(p.entry_data(2, oodle), b"z" * 50)
        self.assertEqual(oodle.sizes, [0x4000])
        self.assertIn("0xB7", d1_pkg.oodle_codec(b"\xb7\x43\xe2\x90"))

    def test_tag_hash_round_trip(self):
        self.assertEqual(d1_pkg.tag_hash(0x5b, 0), 0x808b6000)  # PS4 study sample
        for pkg_id, index in ((0x10, 5), (0x3ff, 0x1fff), (0x400, 1), (0xbff, 42)):
            self.assertEqual(d1_pkg.split_tag_hash(d1_pkg.tag_hash(pkg_id, index)), (pkg_id, index))
        self.assertIsNone(d1_pkg.split_tag_hash(0x12345678))

    def test_parse_int(self):
        self.assertEqual(d1_pkg.parse_int("12"), 12)
        self.assertEqual(d1_pkg.parse_int("80800000"), 0x80800000)
        self.assertEqual(d1_pkg.parse_int("0x1f"), 0x1f)

    def test_empty_entity_group_has_no_array_header(self):
        data = bytearray(40)
        struct.pack_into(">I", data, 8, 0x96E0A5E5)
        self.assertEqual(d1_pkg.entity_group(data), (0x96E0A5E5, []))

    def test_split_patch_chain_uses_verified_block(self):
        p = d1_pkg.Package(self.path)
        p.path = Path(self.dir.name) / "update" / "ps3_test_0123_1.pkg"
        p.patch_id = 1
        wrong = Path(self.dir.name) / "wrong.pkg"
        wrong.write_bytes(bytes(len(self.path.read_bytes())))
        p.patch_sources = {self.path.name: [wrong, self.path]}
        self.assertEqual(p.entry_data(0), bytes(range(0x20, 0x60)))
        p.patch_sources = {self.path.name: [wrong]}
        with self.assertRaises(FileNotFoundError):
            p.entry_data(0)


class LocalizedStrings(unittest.TestCase):
    def test_split_parts_are_joined_before_hashes_are_assigned(self):
        index = bytearray(0x60)
        struct.pack_into(">I", index, 4, 2)
        struct.pack_into(">II", index, 0x40, 0x80800184, 2)
        struct.pack_into(">II", index, 0x54, 0x11111111, 0x22222222)
        table = bytearray(0x100)
        struct.pack_into(">II", table, 4, 3, 0x3c)
        struct.pack_into(">III", table, 0x40, 0x80800184, 3, 0x8080058c)
        struct.pack_into(">II", table, 0x24, 2, 0x7c)
        struct.pack_into(">III", table, 0xa0, 0x80800184, 2, 0x8080033e)
        text_at = 0xd0
        for i, text in enumerate((b"hello ", b"world", b"!")):
            part = 0x54 + i * 0x14
            struct.pack_into(">IIIHHHH", table, part, 0, text_at - part - 4, 0,
                             len(text), len(text), 0x1f, 0)
            table[text_at:text_at + len(text)] = bytes((c - 0x1f) & 255 for c in text)
            text_at += len(text)
        struct.pack_into(">iI", table, 0xb4, 0x54 - 0xb4, 2)
        struct.pack_into(">iI", table, 0xbc, 0x7c - 0xbc, 1)
        self.assertEqual(d1_pkg.string_table(index, table),
                         {0x11111111: "hello world", 0x22222222: "!"})
        struct.pack_into(">iI", table, 0xb4, 0x55 - 0xb4, 2)
        with self.assertRaisesRegex(ValueError, "outside"):
            d1_pkg.string_table(index, table)


class InvestmentItems(unittest.TestCase):
    """investment_items on hand-built tags (no strings: names come out empty)."""

    @staticmethod
    def array(elements, stride, element_class=0x80801234):
        head = struct.pack(">IIIIIII", 0, len(elements), 8, 0x80800184, len(elements), element_class, 0)
        return head + bytes(4) + b"".join(e.ljust(stride, b"\0") for e in elements)

    def test_buckets_and_items(self):
        root, buckets, items = 0x80a58000, 0x80a58015, 0x80a5801a
        helmet, gauntlets = 0xcd887e07, 0xd3b5f9fc
        item_a = bytearray(0x60); struct.pack_into(">I", item_a, 0x48, gauntlets)
        item_b = bytearray(0x60); struct.pack_into(">I", item_b, 0x44, helmet)
        tags = {root: bytearray(0x1a4), buckets: self.array([struct.pack(">Ih", helmet, -1),
                                                            struct.pack(">Ih", gauntlets, -1)], 0x1c, 0x80803495),
                items: self.array([struct.pack(">II", 0, 0x80a5c6d1), struct.pack(">II", 0, 0x80a5c6d2)], 8, 0x808033E8),
                0x80a5c6d1: bytes(item_a), 0x80a5c6d2: bytes(item_b)}
        struct.pack_into(">I", tags[root], 0xb0, buckets)
        struct.pack_into(">I", tags[root], 0xd8, items)
        libraries = 0x80a5e1df
        tags[libraries] = self.array([], 12, 0x80803473)
        struct.pack_into(">I", tags[root], 0x170, libraries)
        struct.pack_into(">I", tags[root], 0x1a0, libraries)
        for item in (item_a, item_b):
            for offset in (0x24, 0x2c, 0x34):
                struct.pack_into(">h", item, offset, -1)
        tags[0x80a5c6d1], tags[0x80a5c6d2] = bytes(item_a), bytes(item_b)

        class FakeSet:
            by_id = {0x12c: type("P", (), {"named": [{"name": "investment_globals", "hash": root}],
                                           "entries": []})()}
            oodle = None

            def tag(self, value):
                return bytes(tags[value]) if value in tags else None

        bucket_list, item_list = d1_pkg.investment_items(FakeSet())
        self.assertEqual([h for h, _ in bucket_list], [helmet, gauntlets])
        self.assertEqual([(it["index"], it["tag"]) for it in item_list], [(0, 0x80a5c6d1), (1, 0x80a5c6d2)])
        self.assertEqual([it["slot"] for it in item_list], [1, 0])
        self.assertEqual([it["bucket"] for it in item_list], ["", ""])  # bucket names need strings
        # Same outer class, grown layout: +d8 now names a non-array tag.
        tags[root].extend(bytes(0x10))
        struct.pack_into(">I", tags[root], 0xe8, items)
        struct.pack_into(">I", tags[root], 0xc0, buckets)
        struct.pack_into(">I", tags[root], 0xd8, 0x80a5802a)
        tags[0x80a5802a] = bytes(32)
        struct.pack_into(">I", item_a, 0x58, gauntlets)
        struct.pack_into(">I", item_b, 0x54, helmet)
        for item in (item_a, item_b):
            for offset in (0x34, 0x3c, 0x44):
                struct.pack_into(">h", item, offset, -1)
        tags[0x80a5c6d1], tags[0x80a5c6d2] = bytes(item_a), bytes(item_b)
        self.assertEqual(d1_pkg.investment_items(FakeSet())[1], item_list)

    def test_same_hash_resolves_in_its_referenced_library(self):
        from unittest.mock import patch
        root = bytearray(0x1a4)
        struct.pack_into(">I", root, 0x1a0, 1)
        tags = {1: self.array([struct.pack(">III", 0, 0, 2),
                               struct.pack(">III", 0, 0, 3)], 12, 0x80803473)}
        for tag in (2, 3):
            index = bytearray(16)
            struct.pack_into(">I", index, 12, tag + 2)
            tags[tag], tags[tag + 2] = bytes(index), bytes([tag])
        ps = type("P", (), {"tag": lambda _, tag: tags.get(tag)})()
        with patch.object(d1_pkg, "string_table", side_effect=lambda i, t:
                          {0x12345678: "Lightmail" if t == b"\x02" else "Prototype"}) as decode:
            text, cache = d1_pkg.investment_strings(ps, root, True)
            self.assertEqual(text(struct.pack(">h2xI", 0, 0x12345678), 0), "Lightmail")
            self.assertEqual(text(struct.pack(">h2xI", 1, 0x12345678), 0), "Prototype")
            self.assertEqual(text(struct.pack(">h2xI", 0, 0x12345678), 0), "Lightmail")
            self.assertEqual(decode.call_count, 2)
            self.assertEqual(len(cache), 2)
            self.assertEqual(text(struct.pack(">h2xI", -1, 0), 0), "")
            with self.assertRaisesRegex(ValueError, "outside"):
                text(struct.pack(">h2xI", 2, 0), 0)


if __name__ == "__main__":
    unittest.main()
