"""Wire-contract tests; synthetic data only, no game files or network needed."""
import base64
import json
import struct
import unittest

from auth3_proto import make_response


class Auth3Contract(unittest.TestCase):
    def test_packed_ticket_and_response(self):
        key = bytes(range(24))
        raw, state = make_response(b'{"auth_task":"18","title_id":"5378","iv_seed":"4294967295"}',
                                   now=1700000000, session_key=key, server_ticket=bytes(range(128)))
        reply = json.loads(raw)
        self.assertEqual((reply["auth_task"], reply["code"], reply["iv_seed"]),
                         ("19", "700", "4294967295"))
        client = base64.b64decode(reply["client_ticket"], validate=True)
        self.assertEqual(len(client), 145)
        self.assertEqual(client[:4], b"\xde\xad\xbd\xef")
        self.assertEqual(struct.unpack_from("<I", client, 5)[0], 5378)
        self.assertEqual(struct.unpack_from("<II", client, 9), (1700000000, 1700086400))
        self.assertEqual(client[33:44], b"d1localtest")
        self.assertEqual(client[0x61:0x79], key)
        self.assertEqual(client[128:], bytes(17))
        self.assertEqual(len(base64.b64decode(reply["server_ticket"])), 128)
        self.assertEqual(base64.b64decode(state["session_key"]), key)

    def test_numeric_request_and_rejections(self):
        request = dict(auth_task=18, title_id=5378, iv_seed=0)
        make_response(json.dumps(request))
        for field, value in (("auth_task", 19), ("title_id", 1), ("iv_seed", -1),
                             ("iv_seed", 2**32), ("iv_seed", True), ("iv_seed", 1.5),
                             ("iv_seed", "1e3"), ("iv_seed", None)):
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                make_response(json.dumps({**request, field: value}))


if __name__ == "__main__":
    unittest.main()
