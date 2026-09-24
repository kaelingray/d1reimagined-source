import socket
import struct
import threading
import unittest

from lobby_probe import exact, header_reply


class LobbyFraming(unittest.TestCase):
    def test_header_wire_layout(self):
        hello = struct.pack("<5I8s", 200, 200, 210, 210, 65535, b"12345678")
        reply = header_reply(hello, 1, b"abcdefgh")
        self.assertEqual(reply[:10], bytes.fromhex("16000000ab81d2000000"))
        self.assertEqual(struct.unpack_from("<Q", reply, 10)[0], 1)
        self.assertEqual(reply[18:], b"abcdefgh")
        with self.assertRaises(ValueError):
            header_reply(struct.pack("<5I8s", 200, 200, 220, 220, 65535, b"12345678"), 1, b"abcdefgh")

    def test_fragmented_stream_and_eof(self):
        a, b = socket.socketpair()
        with a, b:
            a.settimeout(2)
            def sender():
                for part in (b"one", b"two", b"three"):
                    b.sendall(part)
                b.shutdown(socket.SHUT_WR)
            worker = threading.Thread(target=sender)
            worker.start()
            self.assertEqual(exact(a, 11), b"onetwothree" if False else b"onetwothree")
            with self.assertRaises(EOFError):
                exact(a, 4)
            worker.join()


if __name__ == "__main__":
    unittest.main()
