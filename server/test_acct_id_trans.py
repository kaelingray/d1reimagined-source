import struct, unittest
import bap_server as b

class AcctIdTrans(unittest.TestCase):
    def test_reply_echoes_request_and_appends_ids(self):
        req = bytes.fromhex("000107ff") + b"d1localtest".ljust(36, b"\0")
        rsp = b.acct_id_trans_reply(req, [0x0a3d4a2c8fbea6b9])
        self.assertEqual(rsp[:40], req)
        self.assertEqual(struct.unpack(">Q", rsp[40:])[0], 0x0a3d4a2c8fbea6b9)
        with self.assertRaises(ValueError):
            b.acct_id_trans_reply(bytes.fromhex("00010000") + bytes(36), [1])

if __name__ == "__main__":
    unittest.main()

class ClientConfigAnd104(unittest.TestCase):
    def test_client_config_reply(self):
        self.assertEqual(b.client_config_reply(), bytes.fromhex("08011001"))

    def test_104_reply_carries_universe_root(self):
        import investment
        from pathlib import Path
        if not investment.DEFAULT_SCHEMA.exists():
            self.skipTest("private schema not available")
        schema = investment.load_schema(b.SIGNON / "investment_schema_character.json")
        body = b.default_ws_response(schema, 104, b.WS_REPLY_VALUES[104])
        values = investment.decode(schema, int(schema["services"]["104"][1], 16), body[2:])
        self.assertEqual(values[8][0], 0x7FFFFFFFFFFFFFFF)


if __name__ == "__main__":
    unittest.main()
