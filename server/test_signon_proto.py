import struct
import tempfile
import unittest
from pathlib import Path

import signon_proto as sp


class TitleConfigAndManifest(unittest.TestCase):
    def test_success_reply_carries_title_config(self):
        config = sp.title_config(b"http://content.local.test/manifest", b"guid-1")
        reply = sp.success_response(b"token", b"a" * 16, b"b" * 16, b"c" * 16, 1,
                                    sp.ipv4_symmetric("127.1.1.127"), 12850, config=config)
        fields = sp.decode(reply)
        self.assertEqual([(n, w) for n, w, _ in fields], [(1, 0), (2, 2), (4, 2)])
        block = sp.decode(fields[2][2])
        self.assertEqual(block[:2], [(1, 0, 1), (2, 0, 1)])
        content = sp.decode(sp.decode(block[2][2])[0][2])
        self.assertEqual(content, [(2, 2, b"http://content.local.test/manifest"), (3, 2, b"guid-1")])

    def test_manifest_lists_disc_packages(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for name, pid, value in (("ps3_bootstrap_01a3_0.pkg", 0x1A3, 7),
                                     ("ps3_intro_0204_en_0.pkg", 0x204, 8),
                                     ("ps3_intro_0204_de_0.pkg", 0x204, 9),
                                     ("ps3_client_bootstrap_unpatchable_0.pkg", 0x179, 10)):
                root.joinpath(name).write_bytes(struct.pack(">4HQ", 0x18, 4, pid, 1, value) + b"body")
            packages = sp.disc_packages(root)
            self.assertEqual(packages, [(0x1A3, b"ps3_bootstrap_01a3_0", 7), (0x204, b"ps3_intro_0204_en_0", 8)])
        fields = sp.decode(sp.content_manifest(b"guid-1", packages))
        self.assertEqual(fields[-1], (5, 2, b"guid-1"))
        self.assertEqual(sp.decode(fields[0][2]),
                         [(1, 0, 0), (2, 2, b"ps3_bootstrap_01a3_0"), (3, 0, 0x1A3), (4, 0, 7)])
        with self.assertRaises(ValueError):
            sp.content_manifest(b"guid-1", [(5, b"too-low-id", 0)])


if __name__ == "__main__":
    unittest.main()
