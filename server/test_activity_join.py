import unittest

import bap_server as b
import investment


def prim(off, kind, width, min_=0, opt=0):
    return dict(off=off, bit=0, kind=kind, opt=opt, nested=0xFFFFFFFF, arr=0, min=min_, width=width, extra=0)


def nested(off, handle):
    return dict(off=off, bit=0, kind=1, opt=0, nested=handle, arr=0, min=0, width=0, extra=0)


# Cut-down activity schema: the request is {i32, i64}, the response echoes it, then the result
# (3 bits, min 1), the char[128] session id and a trailing u16.
SCHEMA = {
    f"{b.JOIN_REQUEST_TYPE:#x}": {"size": 0x10, "fields": [prim(0, 9, 32), prim(8, 10, 64)]},
    "0x80804008": {"size": 0x80, "fields": [prim(i, 3, 8, 0x80) for i in range(128)]},
    "0x80804056": {"size": 0x80, "fields": [nested(0, 0x80804008)]},
    f"{b.JOIN_RESPONSE_TYPE:#x}": {"size": 0x198, "fields": [
        nested(0, b.JOIN_REQUEST_TYPE), prim(0xF8, 3, 3, 1), nested(0xF9, 0x80804056), prim(0x18E, 8, 16)]},
}


class JoinResponse(unittest.TestCase):
    def test_echoes_request_and_reports_joined(self):
        request = investment.encode(SCHEMA, b.JOIN_REQUEST_TYPE, {0: 0x00DAE848, 8: 0x1234})
        data = b.join_response(SCHEMA, request, "act")
        values = investment.decode(SCHEMA, b.JOIN_RESPONSE_TYPE, data)
        self.assertEqual(values[0], {0: 0x00DAE848, 8: 0x1234})
        self.assertEqual(values[0xF8], 0)
        self.assertEqual(bytes(values[0xF9][0][i] for i in range(4)), b"act\0")
        # The response bit stream starts with the request's bits (both are 96 bits here).
        self.assertEqual(data[:12], request[:12])
        # Result 0 is stored as 1 (bits - min) in 3 bits.
        self.assertEqual(data[12] >> 5, 1)



class EntityIndices(unittest.TestCase):
    def test_bitmap_words(self):
        schema = {
            f"{b.ENTITY_INDICES_TYPE:#x}": {"size": 0x600, "fields": [nested(0, 0x808043A7)]},
            "0x808043a7": {"size": 0x600, "fields": [prim(4 * i, 9, 32) for i in range(b.ENTITY_WORDS)]},
        }
        data = b.entity_indices(schema, 64, 128)
        self.assertEqual(len(data), 0x600)
        self.assertEqual(data[:8], bytes(8))
        self.assertEqual(data[8:16], b"\xff" * 8)
        self.assertEqual(data[16:], bytes(0x600 - 16))

class ActivityState(unittest.TestCase):
    def test_zone_record_none(self):
        schema = {
            f"{b.STATE_TYPE:#x}": {"size": 0xA20, "fields": [nested(0, 0x80800B8F)]},
            "0x80800b8f": {"size": 0xA20, "fields": [
                prim(0, 2, 1), prim(0x10, 5, 32, 0x80000000), prim(0x54, 5, 32, 0x80000000), prim(0x58, 9, 32)]},
        }
        data = b.activity_state(schema, zone=(-1, 0x811C9DC5))
        values = investment.decode(schema, b.STATE_TYPE, data)[0]
        self.assertEqual(values[0x54] & 0xFFFFFFFF, 0xFFFFFFFF)
        self.assertEqual(values[0x58] & 0xFFFFFFFF, 0x811C9DC5)
        # bool, bubble count 1 (biased), index -1 biased to 0x7fffffff, hash
        self.assertEqual(data, bytes.fromhex("c0000000bfffffffc08e4ee2") + bytes([0x80]))

    def test_bubble_states(self):
        schema = {
            f"{b.STATE_TYPE:#x}": {"size": 0xA20, "fields": [nested(0, 0x80800B8F)]},
            "0x80800b8f": {"size": 0xA20, "fields": [
                prim(0, 2, 1), prim(0x10, 5, 32, 0x80000000), nested(0x14, 0x80804050)]},
            "0x80804050": {"size": 0x40, "fields": [prim(i, 7, 8) for i in range(4)]},
        }
        data = b.activity_state(schema, bubbles=4, bubble_states={None: 0, 2: -1})
        values = investment.decode(schema, b.STATE_TYPE, data)[0]
        self.assertEqual(values[0x10], 4)
        self.assertEqual([values[0x14][i] & 0xFF for i in range(4)], [0, 0, 0xFF, 0])


ID_TYPE = {"0x8080405e": {"size": 8, "fields": [prim(i, 7, 8) for i in range(8)]}}


def opt_nested(off, handle):
    return dict(nested(off, handle), opt=1)


class SimulationPacket(unittest.TestCase):
    def test_roster_slot(self):
        schema = dict(ID_TYPE)
        schema["0x80800b83"] = {"size": 0xC, "fields": [
            opt_nested(0, 0x8080405E), prim(8, 4, 16, 0, 1), prim(0xA, 2, 1)]}
        schema["0x8080540e"] = {"size": 0x30C, "fields": [nested(0xC * i, 0x80800B83) for i in range(65)]}
        schema[f"{b.ROSTER_TYPE:#x}"] = {"size": 0x310, "fields": [
            prim(0, 5, 32, 0x80000000, 1), nested(4, 0x8080540E)]}
        peer = {i: i + 1 for i in range(8)}
        data = b.sim_packet(schema, peer, slot=2, generation=7, epoch=3, tick=9)
        r = investment.BitReader(data)
        self.assertEqual(r.get(8), 3)
        self.assertEqual(r.get(1), 1)
        self.assertEqual([r.get(1) for _ in range(65)], [int(i == 2) for i in range(65)])
        self.assertEqual(r.get(1), 1)
        roster = investment.decode_from(r, schema, b.ROSTER_TYPE)
        self.assertEqual(roster[4][0x18], {0: peer, 8: 7, 0xA: True})
        self.assertEqual(roster[4][0], {0xA: False})
        self.assertEqual(r.get(32), 9)
        self.assertEqual(r.get(1), 0)

    def test_replicated_sim_player(self):
        schema = dict(ID_TYPE)
        schema["0x80800b83"] = {"size": 0xC, "fields": [opt_nested(0, 0x8080405E)]}
        schema["0x8080540e"] = {"size": 0x30C, "fields": [nested(0xC * i, 0x80800B83) for i in range(65)]}
        schema[f"{b.ROSTER_TYPE:#x}"] = {"size": 0x310, "fields": [nested(4, 0x8080540E)]}
        schema[f"{b.OBJECT_ID_TYPE:#x}"] = {"size": 8, "fields": [
            prim(0, 9, 32), prim(4, 3, 6, 1), prim(6, 4, 16, 0x8000)]}
        schema["0x80800540"] = {"size": 0x10, "fields": [prim(0, 5, 32, 0x80000000), prim(8, 2, 0), prim(9, 2, 0)]}
        schema["0x80800646"] = {"size": 0x30, "fields": [prim(0, 0x23, 0)]}
        schema[f"{b.SIM_PLAYER_TYPE:#x}"] = {"size": 0x50, "fields": [
            nested(0, 0x80800540), nested(0x18, 0x80800646), prim(0x4C, 5, 32, 0x80000000)]}
        peer = {i: 0 for i in range(8)}
        obj = b.sim_player(schema, 0x4786C0E0, 5, 0xAF769890229AB8FB)
        data = b.sim_packet(schema, peer, objects=[obj, b.default_baseline(1, 0x10, 3)])
        r = investment.BitReader(data)
        r.get(8 + 1 + 65 + 1)
        investment.decode_from(r, schema, b.ROSTER_TYPE)
        r.get(32)
        self.assertEqual(r.get(1), 1)
        self.assertEqual(r.get(1), 1)
        self.assertEqual(investment.decode_from(r, schema, b.OBJECT_ID_TYPE), {0: 0x4786C0E0, 4: 0xC, 6: 5})
        length = r.get(32)
        start = r.position
        self.assertEqual((r.get(1), r.get(1)), (1, 1))
        state = investment.decode_from(r, schema, b.SIM_PLAYER_TYPE)
        self.assertEqual(state[0], {0: -1, 8: True, 9: True})
        self.assertEqual(state[0x18], {0: 0xAF769890229AB8FB})
        self.assertEqual(r.get(1), 0)
        self.assertEqual(r.position - start, length)
        self.assertEqual(r.get(1), 1)
        self.assertEqual(investment.decode_from(r, schema, b.OBJECT_ID_TYPE), {0: 1, 4: 0x10, 6: 3})
        self.assertEqual(r.get(32), 2)
        self.assertEqual((r.get(1), r.get(1)), (1, 0))
        self.assertEqual(r.get(1), 0)



class ObjectSpec(unittest.TestCase):
    def test_parse(self):
        obj, state_type, state, has_b = b.object_spec('0x992bb471:0x20:7:0x80802a1b:1={"0": 1, "0x10": {"4": 2}}')
        self.assertEqual(obj, {0: 0x992BB471, 4: 0x20, 6: 7})
        self.assertEqual(state_type, 0x80802A1B)
        self.assertEqual(state, {0: 1, 0x10: {4: 2}})
        self.assertTrue(has_b)
        self.assertEqual(b.object_spec("1:2:3:0x80800463")[2], {})


if __name__ == "__main__":
    unittest.main()
