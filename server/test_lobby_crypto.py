import unittest
import lobby_probe as lp
import bdbuffer


class LobbyCryptoTest(unittest.TestCase):
    def test_seal_open_round_trip(self):
        keys = {"bddata_20": [b"c" * 20, b"s" * 20], "bddata_16": [b"C" * 16, b"S" * 16]}
        server = lp.LobbyCrypto(keys)
        client_view = lp.LobbyCrypto({"bddata_20": keys["bddata_20"][::-1],
                                      "bddata_16": keys["bddata_16"][::-1]})
        frame = server.seal(b"\x86\x8a" + bytes(40))
        self.assertEqual(frame[4:6], b"\xab\x85")
        self.assertEqual(int.from_bytes(frame[:4], "little"), len(frame) - 4)
        self.assertEqual(client_view.open(frame), (1, b"\x86\x8a" + bytes(40)))
        with self.assertRaises(ValueError):
            client_view.open(frame[:-1] + bytes([frame[-1] ^ 1]))

    def test_task_reply_layout(self):
        reply = lp.task_reply(0x01, 1)
        self.assertEqual(reply[0], 0x01)
        values = bdbuffer.Reader(reply[1:]).values()
        self.assertEqual(values, [(bdbuffer.UINT64, 0x8000000000000000), (bdbuffer.UINT32, 0),
                                  (bdbuffer.UINT8, 1), (bdbuffer.UINT32, 0), (bdbuffer.UINT32, 0)])


if __name__ == "__main__":
    unittest.main()
