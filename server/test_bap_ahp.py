"""Proxy assignment correlation over the real encrypted BAP session; no private data needed."""

import socket
import struct
import tempfile
import threading
import unittest
from pathlib import Path
from unittest.mock import patch

import bap_server as bap
from test_ah_session import FakeClient, read_frame


class ProxyAssignment(unittest.TestCase):
    endpoint = bytes.fromhex("7f01018012343233")
    configured = bytes.fromhex("1020304050607080") + endpoint

    def start(self, configured):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        logging = patch.object(bap, "log")
        logging.start()
        self.addCleanup(logging.stop)
        args = bap.parse_args([
            "--capture-dir", folder.name,
            "--schema", str(Path(folder.name) / "absent.json"),
            "--activity-schema", str(Path(folder.name) / "absent-activity.json"),
            "--ahp-reply", configured.hex(),
        ])
        server, client = socket.socketpair()
        client.settimeout(2)
        thread = threading.Thread(target=bap.handle, args=(server, ("127.0.0.1", 0), args), daemon=True)
        thread.start()
        self.addCleanup(thread.join, 2)
        self.addCleanup(client.close)
        return FakeClient(client)

    def request(self, client, payload, message_id):
        inner = struct.pack(">HI", bap.AHP_REQ, message_id) + payload
        sealed = client.tx.seal(inner)
        client.sock.sendall(struct.pack(">BBI", 1, 1, len(sealed)) + sealed)
        kind, body = read_frame(client.sock)
        self.assertEqual(kind, 1)
        plain = client.rx.open(body)
        self.assertEqual(struct.unpack_from(">HIH", plain), (bap.AHP_RSP, message_id, 200))
        return plain[8:]

    def test_initial_and_reassigned_ids_are_echoed_without_changing_endpoint(self):
        client = self.start(self.configured)
        # The outer BAP message id is independent of the assignment's u64 request id.
        for message_id, request_id in enumerate((0, 1, 0xFEDCBA9876543210, 1), 21):
            payload = struct.pack(">Q", request_id)
            with self.subTest(request_id=request_id):
                self.assertEqual(self.request(client, payload, message_id), payload + self.endpoint)

    def test_existing_zero_id_recipe_remains_byte_for_byte_identical(self):
        configured = bytes(8) + self.endpoint
        self.assertEqual(self.request(self.start(configured), bytes(8), 32), configured)

    def test_malformed_requests_preserve_the_raw_configured_reply(self):
        client = self.start(self.configured)
        for message_id, payload in enumerate((b"", bytes(7), bytes(9)), 40):
            with self.subTest(length=len(payload)):
                self.assertEqual(self.request(client, payload, message_id), self.configured)

    def test_malformed_configured_replies_are_not_padded_or_truncated(self):
        for configured in (b"", bytes(range(15)), bytes(range(17))):
            with self.subTest(length=len(configured)):
                self.assertEqual(self.request(self.start(configured), struct.pack(">Q", 1), 60), configured)


if __name__ == "__main__":
    unittest.main()
