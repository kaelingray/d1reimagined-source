"""Mission "loot" drops: the announcing incident body, the pending record and its claim."""
import unittest
from unittest import mock

import bap_server as bap
from mission_script import LootDropReaction

NULL = 0xFFFFFFFF


def fields(*rows):
    return [{"off": off, "kind": kind, "width": width, "nested": nested, "min": minimum,
             "opt": opt, "arr": arr} for off, kind, width, nested, minimum, opt, arr in rows]


# Measured 01.24 layout of the selector-2 body 0x80800cb6 and its nested types.
SCHEMA = {
    "0x80800cb6": {"size": 128, "fields": fields(
        (0, 1, 0, 0x8080056F, 0, 0, 0), (24, 1, 0, 0x80800E70, 0, 0, 0), (88, 9, 32, NULL, 0, 0, 0),
        (96, 13, 0, NULL, 128, 0, 0), (112, 5, 32, NULL, 0x80000000, 0, 128))},
    "0x8080056f": {"size": 24, "fields": fields((0, 9, 32, NULL, 0, 0, 0), (8, 1, 0, 0x808009B8, 0, 0, 0))},
    "0x808009b8": {"size": 16, "fields": fields((0, 3, 3, NULL, 1, 0, 0), (8, 6, 64, NULL, 0, 0, 0))},
    "0x80800e70": {"size": 64, "fields": fields(
        (0, 25, 0, NULL, 0, 0, 0), (8, 2, 0, NULL, 0, 0, 0), (9, 2, 0, NULL, 0, 0, 0),
        (10, 2, 0, NULL, 0, 0, 0), (11, 3, 2, NULL, 1, 0, 0), (12, 9, 32, NULL, 0, 0, 0),
        (16, 9, 32, NULL, 0, 0, 0), (20, 9, 32, NULL, 0, 0, 0), (24, 35, 0, NULL, 0, 0, 0),
        (32, 3, 6, NULL, 3, 0, 0), (40, 10, 64, NULL, 0, 0, 0), (48, 3, 3, NULL, 1, 0, 0),
        (49, 3, 2, NULL, 1, 0, 0), (56, 10, 64, NULL, 0, 0, 0))},
}
CHARACTER = 0x0A3D4A2C8FBEA6BA
RIFLE = {"item": 3598, "position": [410.644, -794.385, 17.723], "world": 3}
# JOIN222, third send: the body that reached handler 0x5aede4 (recipient 1, world 3, nonce
# {1, 0x0a3d000000002170}).
ACCEPTED_BODY = bytes.fromhex(
    "000000014147a0000000042e0b02000000000000000000000000000000000000000018000000000000000940"
    "000000000000000000000010f3549bf111a6291063722d20000000c0")


class MissionLoot(unittest.TestCase):
    def test_drop_body_matches_the_body_the_client_accepted(self):
        loot = bap.mission_loot({"loot": {"rifle": RIFLE}})["rifle"]
        with mock.patch.object(bap, "loot_nonce", return_value=(1, 0x0A3D000000002170)):
            self.assertEqual(bap.loot_drop_body(SCHEMA, loot, CHARACTER, "rifle"), ACCEPTED_BODY)

    def test_loot_table_defaults_and_validation(self):
        self.assertEqual(bap.mission_loot({"loot": {"rifle": RIFLE}}), {"rifle": {
            "item": 3598, "position": (410.644, -794.385, 17.723), "world": 3,
            "source_kind": 3, "recipient": 1}})
        self.assertEqual(bap.mission_loot({}), {})
        for bad in ({**RIFLE, "item": 1}, {**RIFLE, "position": [1, 2]}, {**RIFLE, "world": -1},
                    {**RIFLE, "recipient": True}, {**RIFLE, "colour": 1}, {"item": 3598}):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                bap.mission_loot({"loot": {"rifle": bad}})

    def test_nonce_is_stable_per_character_and_name(self):
        key, nonce = bap.loot_nonce(CHARACTER, "rifle")
        self.assertEqual(key, 1)
        self.assertEqual(nonce >> 48, CHARACTER >> 48)
        self.assertEqual(bap.loot_nonce(CHARACTER, "rifle"), (key, nonce))
        self.assertNotEqual(bap.loot_nonce(CHARACTER, "other")[1], nonce)
        self.assertNotEqual(bap.loot_nonce(CHARACTER ^ 1 << 60, "rifle")[1], nonce)

    def test_pending_record_is_claimed_by_its_drop_nonce(self):
        loot = bap.mission_loot({"loot": {"rifle": RIFLE}})["rifle"]
        overlay = bap.add_pending_loot(None, [bap.pending_loot_record(loot, CHARACTER, "rifle")])
        key, nonce = bap.loot_nonce(CHARACTER, "rifle")
        cleared, item = bap.claim_pending_loot(overlay, (3, key, nonce, 1))
        self.assertEqual(item, 3598)
        self.assertEqual(cleared["self"][bap.PENDING_LOOT_0124][8][0], {0: -1})

    def test_records_fill_free_slots_after_existing_ones(self):
        used = {0: 3598, 8: 3, 16: {0: {0: 1, 8: 5}, 16: 1}}
        overlay = {"self": {bap.PENDING_LOOT_0124: {0: 1, 8: {0: used}}}}
        added = bap.add_pending_loot(overlay, [{0: 1}, {0: 2}])
        slots = added["self"][bap.PENDING_LOOT_0124][8]
        self.assertEqual((slots[0], slots[0x38], slots[0x70], slots[0xA8]), (used, {0: 1}, {0: 2}, {0: -1}))
        self.assertEqual(len(slots), 64)
        self.assertIs(bap.add_pending_loot(overlay, []), overlay)
        # after a grant the cleared slots are still published (JOIN228)
        empty = bap.add_pending_loot(None, [], publish_empty=True)["self"][bap.PENDING_LOOT_0124]
        self.assertEqual((empty[0], set(map(str, empty[8].values()))), (1, {str({0: -1})}))
        self.assertEqual(len(empty[8]), 64)
        with self.assertRaises(ValueError):
            bap.add_pending_loot(None, [{0: 1}] * 65)

    def test_mission_steps_announce_named_drops_reactively(self):
        mission = {"loot": {"rifle": RIFLE},
                   "steps": [{"on": {"type": 18}, "delay": 30, "loot_drop": "rifle"}]}
        timed, reactive = bap.expand_mission(mission, SCHEMA)
        self.assertEqual(timed, [])
        self.assertEqual([(r[0], r[2], r[3]) for r in reactive], [(18, 30.0, LootDropReaction("rifle"))])
        for step in ({"on": {"type": 18}, "loot_drop": "other"}, {"at": 5, "loot_drop": "rifle"}):
            with self.subTest(step=step), self.assertRaises(ValueError):
                bap.expand_mission({"loot": {"rifle": RIFLE}, "steps": [step]}, SCHEMA)


if __name__ == "__main__":
    unittest.main()
