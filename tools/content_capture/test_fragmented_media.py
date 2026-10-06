from pathlib import Path
import struct
import tempfile
import unittest

from fragmented_media import completed_fragment_prefix


def box(kind, payload=b"", extended=False):
    if extended:
        return struct.pack(">I4sQ", 1, kind, len(payload) + 16) + payload
    return struct.pack(">I4s", len(payload) + 8, kind) + payload


class FragmentPrefixTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.path = Path(self.temp.name) / "source.mp4"
        self.header = box(b"ftyp", b"isom") + box(b"moov", b"initial metadata")
        self.fragment = box(b"moof", b"sample table") + box(b"mdat", b"complete samples")
        self.prefix = self.header + self.fragment

    def tearDown(self):
        self.temp.cleanup()

    def scan(self, data):
        self.path.write_bytes(data)
        result = completed_fragment_prefix(self.path)
        self.assertEqual(self.path.read_bytes(), data, "scanner changed original media")
        return result

    def test_complete_file_and_trailer_need_no_recovery(self):
        self.assertIsNone(self.scan(self.prefix + box(b"mfra", b"index")))

    def test_partial_mdat_keeps_only_prior_fragment(self):
        data = self.prefix + box(b"moof", b"next table") + struct.pack(">I4s", 4096, b"mdat") + b"partial"
        result = self.scan(data)
        self.assertEqual(result["retained_bytes"], len(self.prefix))
        self.assertEqual(result["discarded_tail_bytes"], len(data) - len(self.prefix))

    def test_partial_moof_keeps_prior_fragment(self):
        self.assertEqual(self.scan(self.prefix + b"\x00\x00")["retained_bytes"], len(self.prefix))

    def test_pending_moof_without_mdat_is_discarded(self):
        self.assertEqual(self.scan(self.prefix + box(b"moof"))["retained_bytes"], len(self.prefix))

    def test_zero_size_tail_cannot_prove_complete_media(self):
        tail = box(b"moof") + struct.pack(">I4s", 0, b"mdat") + b"partial"
        self.assertEqual(self.scan(self.prefix + tail)["retained_bytes"], len(self.prefix))

    def test_extended_length_and_malformed_tail(self):
        prefix = self.header + box(b"moof", extended=True) + box(b"mdat", b"samples", extended=True)
        self.assertEqual(self.scan(prefix + struct.pack(">I4s", 2, b"free"))["retained_bytes"], len(prefix))

    def test_no_initial_metadata_or_no_complete_fragment_is_unrecoverable(self):
        self.assertIsNone(self.scan(box(b"ftyp") + self.fragment + b"partial"))
        self.assertIsNone(self.scan(self.header + box(b"moof") + struct.pack(">I4s", 1000, b"mdat")))

    def test_regular_container_and_unrelated_data_are_not_recovered(self):
        self.assertIsNone(self.scan(box(b"ftyp") + box(b"mdat", b"samples") + b"partial"))
        self.assertIsNone(self.scan(b"unrelated raw input"))


if __name__ == "__main__":
    unittest.main()
