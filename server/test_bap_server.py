import struct
import unittest

import bap_server as bap


class ShelloReply(unittest.TestCase):
    key, mac_key = b"k" * 16, b"m" * 16

    def test_sealed_reply_fits_client_limits(self):
        blob = bap.shello_rsp_body(self.key, self.mac_key, b"n" * 12, b"s" * 16, b"i" * 16)
        self.assertEqual(len(blob), 84)
        self.assertEqual(struct.unpack_from(">I", blob)[0], 80)  # codec: first u32 == size - 4
        self.assertEqual(blob[4:20], b"i" * 16)
        self.assertEqual(bap.unseal(self.key, self.mac_key, blob), b"n" * 12 + b"s" * 16)
        tampered = blob[:30] + bytes([blob[30] ^ 1]) + blob[31:]
        with self.assertRaises(ValueError):
            bap.unseal(self.key, self.mac_key, tampered)

    def test_plain_frame_layout(self):
        frame = bap.plain_frame(bap.SHELLO_RSP, 0, 200, b"xyz")
        self.assertEqual(frame[:6], bytes.fromhex("01020000000b"))
        self.assertEqual(frame[6:14], bytes.fromhex("001a0000000000c8"))
        self.assertEqual(frame[14:], b"xyz")


    def test_gcm_channels_mirror_the_client(self):
        nonce, key = bytes(range(12)), b"s" * 16
        server_rx, server_tx = bap.channels(nonce, key)
        client_tx = bap.GcmChannel(key, nonce[:11] + bytes([nonce[11] ^ 1]))
        for msg in (bytes.fromhex("007900000001"), b"second"):
            sealed = client_tx.seal(msg)
            self.assertEqual(len(sealed), len(msg) + 16)
            self.assertEqual(server_rx.open(sealed), msg)
        channel = bap.GcmChannel(key, bytes.fromhex("ffff") + bytes(10))
        channel.seal(b"")
        self.assertEqual(bytes(channel.nonce), bytes.fromhex("000001") + bytes(9))  # LE carry
        frame = bap.encrypted_frame(server_tx, bap.QZ_REG_RSP, 1, 200, b"")
        self.assertEqual(frame[:6], bytes.fromhex("010100000018"))


if __name__ == "__main__":
    unittest.main()


class ClientConfigReply(unittest.TestCase):
    def test_fields_are_protobuf_varints(self):
        self.assertEqual(bap.client_config_reply(), bytes.fromhex("08011001"))
        self.assertEqual(bap.client_config_reply(0, 1), bytes.fromhex("08001001"))
        self.assertEqual(bap.client_config_reply(300, 2), bytes.fromhex("08ac021002"))
