import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import stun_stub  # noqa: E402

# The two 01.24 probe shapes captured from run A01 (local/SIGNON_LOCAL/logs/
# FINAL124_S01-stun.log). The 29-byte one is a MessagePack fixmap(14).
PROBE4 = bytes.fromhex("00 01 00 01")
PROBE29 = bytes.fromhex(
    "0e 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 "
    "ff 00 ff 00 00 00 ff 00 ff 00 00"
)


class Args:
    byteorder = "little"
    echo_version = False
    reply_index = 0
    fill = "all"
    shape = "map"


PEER = ("127.0.0.1", 3074)
SERVER = ("127.0.2.2", 3074)


class ParseTest(unittest.TestCase):
    def test_msgpack_probe_parses_to_seven_integer_pairs(self):
        pairs = stun_stub.parse_mp_map(PROBE29)
        self.assertIsNotNone(pairs)
        self.assertEqual(len(pairs), 14)          # fixmap of 14 elements
        self.assertEqual(pairs[0], (2, 0))        # leading "request stage" key
        self.assertIn(-1, [v for _, v in pairs])  # unbound port sentinels

    def test_non_msgpack_is_rejected(self):
        self.assertIsNone(stun_stub.parse_mp_map(b""))
        self.assertIsNone(stun_stub.parse_mp_map(bytes([30, 3, 0, 0])))
        self.assertIsNone(stun_stub.parse_mp_map(bytes([0x0E, 0x02])))  # truncated


class IntCodecTest(unittest.TestCase):
    def test_round_trips_the_range_a_port_needs(self):
        for value in (-1, 0, 1, 2, 127, 128, 255, 3074, 65535):
            encoded = stun_stub._mp_int(value)
            got, pos = stun_stub._read_mp_int(encoded, 0)
            self.assertEqual(got, value, f"{value} encoded as {encoded.hex(' ')}")
            self.assertEqual(pos, len(encoded), "encoder and reader disagree on width")


class ReplyTest(unittest.TestCase):
    def test_0124_msgpack_reply_round_trips_and_fills_the_first_unbound_port(self):
        reply = stun_stub.build_reply(PROBE29, PEER, SERVER, Args())
        self.assertIsNotNone(reply)
        pairs = stun_stub.parse_mp_map(reply)
        self.assertIsNotNone(pairs, "the client must be able to walk our reply back")
        self.assertEqual(len(pairs), len(stun_stub.parse_mp_map(PROBE29)))
        self.assertIn(PEER[1], [v for _, v in pairs],
                      "the mapped port must be reported back")

    def test_short_probe_gets_the_dispatch_index_then_the_mapped_address(self):
        reply = stun_stub.build_reply(PROBE4, PEER, SERVER, Args())
        self.assertIsNotNone(reply)
        # the client reads the first word as the dispatch index (0..19 valid)
        self.assertEqual(reply[0], 0)
        # the rest is the mapped address in the same binary layout the 01.00
        # replies use: IPv4 in network order, then the port.
        self.assertEqual(reply[1:5], bytes([127, 0, 0, 1]))
        self.assertEqual(int.from_bytes(reply[5:7], "little"), PEER[1])

    def test_reply_index_selects_the_dispatch_word(self):
        for index in (0, 1, 19):
            args = Args()
            args.reply_index = index
            reply = stun_stub.build_reply(PROBE4, PEER, SERVER, args)
            self.assertEqual(reply[0], index)

    def test_reply_index_minus_one_stays_silent(self):
        args = Args()
        args.reply_index = -1
        self.assertIsNone(stun_stub.build_reply(PROBE29, PEER, SERVER, args))
        self.assertIsNone(stun_stub.build_reply(PROBE4, PEER, SERVER, args))

    def test_0100_replies_are_unchanged(self):
        ip_discovery = stun_stub.build_reply(bytes([30, 3, 0, 0]), PEER, SERVER, Args())
        self.assertEqual(ip_discovery[:3], bytes([31, 2, 0]))
        nat_discovery = stun_stub.build_reply(bytes([20, 2, 0, 0]), PEER, SERVER, Args())
        self.assertEqual(nat_discovery[:3], bytes([21, 2, 0]))


class ShapeTest(unittest.TestCase):
    """--shape picks the reply layout for the 01.24 map probe; all start with the index."""

    def test_binary_shape_is_index_then_mapped_address(self):
        args = Args()
        args.shape = "binary"
        reply = stun_stub.build_reply(PROBE29, PEER, SERVER, args)
        self.assertEqual(reply[0], args.reply_index)
        self.assertEqual(reply[1:5], bytes([127, 0, 0, 1]))
        self.assertEqual(int.from_bytes(reply[5:7], "little"), PEER[1])

    def test_index_shape_is_the_index_alone(self):
        args = Args()
        args.shape = "index"
        for index in (0, 1, 19):
            args.reply_index = index
            self.assertEqual(stun_stub.build_reply(PROBE29, PEER, SERVER, args), bytes([index]))

    def test_map_shape_still_round_trips(self):
        args = Args()
        args.shape = "map"
        self.assertIsNotNone(stun_stub.parse_mp_map(
            stun_stub.build_reply(PROBE29, PEER, SERVER, args)))


class FillTest(unittest.TestCase):
    """--fill decides how many of the request's unbound (-1) ports are answered."""

    def sentinels(self, pairs):
        return [i for i, (_, v) in enumerate(pairs) if v == -1]

    def test_all_fills_every_unbound_port(self):
        args = Args()
        args.fill = "all"
        pairs = stun_stub.parse_mp_map(stun_stub.build_reply(PROBE29, PEER, SERVER, args))
        holes = self.sentinels(stun_stub.parse_mp_map(PROBE29))
        self.assertTrue(holes, "the captured probe should contain -1 sentinels")
        for i in holes:
            self.assertEqual(pairs[i][1], PEER[1])

    def test_one_fills_only_the_first(self):
        args = Args()
        args.fill = "one"
        pairs = stun_stub.parse_mp_map(stun_stub.build_reply(PROBE29, PEER, SERVER, args))
        holes = self.sentinels(stun_stub.parse_mp_map(PROBE29))
        self.assertEqual(pairs[holes[0]][1], PEER[1])
        for i in holes[1:]:
            self.assertEqual(pairs[i][1], -1)

    def test_none_echoes_the_request(self):
        args = Args()
        args.fill = "none"
        reply = stun_stub.build_reply(PROBE29, PEER, SERVER, args)
        self.assertEqual(stun_stub.parse_mp_map(reply), stun_stub.parse_mp_map(PROBE29))


def struct_ints(buf):
    """Every integer the reader can pull out of buf, in order."""
    out, pos = [], 0
    while pos < len(buf):
        value, pos = stun_stub._read_mp_int(buf, pos)
        if value is None:
            break
        out.append(value)
    return out
