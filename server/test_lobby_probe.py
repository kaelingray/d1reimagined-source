import base64
import hashlib
import hmac
import json
import socket
import struct
import tempfile
import threading
import unittest
from pathlib import Path

from lobby_probe import (auth_done, client_auth_fields, derive_auth, exact, header_reply,
                         hkdf_expand, match_session)


def pack_bits(fields, total):
    """LSB-first bit writer: fields are (bit offset, bytes)."""
    out = bytearray((total + 7) // 8)
    for start, data in fields:
        for bit in range(len(data) * 8):
            if data[bit >> 3] >> (bit & 7) & 1:
                pos = start + bit
                out[pos >> 3] |= 1 << (pos & 7)
    return bytes(out)


class LobbyFraming(unittest.TestCase):
    def test_header_wire_layout(self):
        hello = struct.pack("<5I8s", 200, 200, 210, 210, 65535, b"12345678")
        reply = header_reply(hello, 1, b"abcdefgh")
        self.assertEqual(reply[:10], bytes.fromhex("16000000ab81d2000000"))
        self.assertEqual(struct.unpack_from("<Q", reply, 10)[0], 1)
        self.assertEqual(reply[18:], b"abcdefgh")
        with self.assertRaises(ValueError):
            header_reply(struct.pack("<5I8s", 200, 200, 220, 220, 65535, b"12345678"), 1, b"abcdefgh")

    def test_hkdf_expand_rfc5869_case4(self):
        prk = bytes.fromhex("9b6c18c432a7bf8f0e71c8eb88f4b30baa2ba243")
        okm = hkdf_expand(prk, bytes(range(0xF0, 0xFA)), 42)
        self.assertEqual(okm.hex(), "085a01ea1b10f36933068b56efa5ad81a4f14b82"
                                    "2f5b091568a9cdd4f155fda2c22e422478d305f3f896")

    def test_client_auth_round_trip(self):
        hello = struct.pack("<5I8s", 200, 200, 210, 210, 65535, b"12345678")
        server_hello = header_reply(hello, 7, b"abcdefgh")
        key, ticket = bytes(range(24)), bytes(range(100, 228))
        bits = pack_bits([(0, b"\x07"), (14, struct.pack("<I", 5378)),
                          (51, struct.pack("<I", 0x3DCC365F)), (83, ticket)], 1112)
        unsigned = struct.pack("<I", len(bits) + 10) + b"\xab\x82" + bits
        keys = derive_auth(hello, server_hello, unsigned + bytes(8), key)
        frame = unsigned + keys["challenge"]
        self.assertEqual(client_auth_fields(frame), (5378, 0x3DCC365F, ticket))
        with tempfile.TemporaryDirectory() as folder:
            session = {"session_key": base64.b64encode(key).decode(),
                       "server_ticket": base64.b64encode(ticket).decode()}
            Path(folder, "a.session.json").write_text(json.dumps(session), encoding="utf-8")
            name, found = match_session(hello, server_hello, frame, Path(folder))
            self.assertEqual(name, "a.session.json")
            self.assertEqual(found["verify"], keys["verify"])
            tampered = frame[:-1] + bytes([frame[-1] ^ 1])
            self.assertIsNone(match_session(hello, server_hello, tampered, Path(folder)))
        # The verify is the second half of the CLIENTCHAL expansion.
        transcript = hello[8:] + server_hello + unsigned
        prk = hmac.new(hashlib.sha1(transcript).digest(), key, hashlib.sha1).digest()
        self.assertEqual(hkdf_expand(prk, b"CLIENTCHAL", 16)[8:], keys["verify"])
        self.assertEqual(auth_done(keys["verify"])[:6], bytes.fromhex("0a000000ab83"))

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
