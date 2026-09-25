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


if __name__ == "__main__":
    unittest.main()
