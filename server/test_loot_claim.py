"""Service-601 world-loot claims: request decoding, pending-slot match and the granted gear."""
import unittest

import bap_server as bap


def field(off, kind, width=0, nested=0xFFFFFFFF, minimum=0, arr=0):
    return {"off": off, "kind": kind, "width": width, "min": minimum, "opt": 0,
            "arr": arr, "nested": nested}


# Measured 01.24 request type 0x80801757 and its nonce record 0x808018ff/0x808017e8.
SCHEMA = {
    "0x80801757": {"size": 32, "fields": [field(0, 3, 4, minimum=1), field(8, 1, nested=0x808018FF)]},
    "0x808018ff": {"size": 24, "fields": [field(0, 1, nested=0x808017E8),
                                          field(16, 5, 32, minimum=0x80000000, arr=128)]},
    "0x808017e8": {"size": 16, "fields": [field(0, 5, 32, minimum=0x80000000, arr=128),
                                          field(8, 6, 64)]},
    "0x808017ab": {"size": 32, "fields": [field(8, 1, nested=0x808018FF)]},
    "services": {"601": ["0x80801757", "0x808017ab"]},
}
NONCE = 0x0A3D000000002170
# JOIN223: the client's claim after walking onto the spawned rifle, without the service ID.
CLAIM_BODY = bytes.fromhex("4800000010a3d000000002170800000010")


def pending(*records):
    slots = {i * bap.PENDING_LOOT_SLOT: {0: -1} for i in range(64)}
    for i, record in enumerate(records):
        slots[i * bap.PENDING_LOOT_SLOT] = record
    return {"self": {bap.PENDING_LOOT_0124: {0: 1, 8: slots}}}


def rifle(kind=3, nonce=NONCE):
    return {0: 3598, 4: 1, 8: kind, 16: {0: {0: 1, 8: nonce}, 16: 1}}


class LootClaim(unittest.TestCase):
    def test_observed_claim_decodes_to_source_kind_and_nonce(self):
        self.assertEqual(bap.loot_claim_request(SCHEMA, CLAIM_BODY), (3, 1, NONCE, 1))

    def test_matching_slot_is_cleared_and_others_kept(self):
        other = rifle(nonce=NONCE + 1)
        overlay = pending(other, rifle())
        updated, item = bap.claim_pending_loot(overlay, (3, 1, NONCE, 1))
        self.assertEqual(item, 3598)
        slots = updated["self"][bap.PENDING_LOOT_0124][8]
        self.assertEqual(slots[bap.PENDING_LOOT_SLOT], {0: -1})
        self.assertEqual(slots[0], other)
        self.assertEqual(updated["self"][bap.PENDING_LOOT_0124][0], 1)
        # the input overlay is not modified
        self.assertEqual(overlay["self"][bap.PENDING_LOOT_0124][8][bap.PENDING_LOOT_SLOT], rifle())

    def test_claim_needs_the_same_source_kind_and_full_nonce(self):
        overlay = pending(rifle())
        for claim in ((5, 1, NONCE, 1), (3, 2, NONCE, 1), (3, 1, NONCE ^ 1 << 40, 1)):
            with self.subTest(claim=claim):
                self.assertEqual(bap.claim_pending_loot(overlay, claim), (overlay, None))
        self.assertEqual(bap.claim_pending_loot(None, (3, 1, NONCE, 1)), (None, None))
        self.assertEqual(bap.claim_pending_loot(pending(), (3, 1, NONCE, 1))[1], None)

    def test_granted_rifle_is_equipped_with_its_art(self):
        gear = bap.gear_spec("starter:01.24:titan")
        granted = bap.world_loot_gear(gear, 3598, 0x51)
        self.assertEqual(granted[6], {"item": 3598, "id": 0x51, "dyes": [(5, 6036), (6, 6037)],
                                      "appearance": 19, "arrangement": 2364})
        self.assertEqual({k: v for k, v in granted.items() if k != 6}, gear)
        self.assertEqual(bap.world_loot_gear(gear, 3598)[6]["id"], 0x106)
        self.assertIsNone(bap.world_loot_gear(gear, 1))

    def test_grant_reaches_the_self_inventory_and_equipment(self):
        layout = bap.LAYOUT_0100
        overlay, item = bap.claim_pending_loot(pending(rifle()), (3, 1, NONCE, 1))
        extras = bap.character_extras(bap.world_loot_gear({}, item, 0x51), overlay, layout)
        self_values = extras["self"]
        self.assertEqual(self_values[layout["self_equipped"]], {6 * 8: 0x51})
        inventory = self_values[layout["self_inventory"]]
        self.assertEqual(inventory[0], 1)
        self.assertEqual(list(inventory[layout["inventory_items"]].values()),
                         [{0: 3598, 8: 0x51, 0x10: 1}])
        self.assertEqual(self_values[bap.PENDING_LOOT_0124][8][0], {0: -1})


if __name__ == "__main__":
    unittest.main()
