import tempfile
import threading
import unittest
from pathlib import Path

import json

from player_db import ConflictError, ID_BASE, PlayerDB, import_store


class PlayerDBTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.path = Path(self.dir.name) / "players.db"
        self.db = PlayerDB(self.path)

    def tearDown(self):
        self.db.close()
        self.dir.cleanup()

    def test_migrations_recorded_and_idempotent(self):
        self.assertEqual(self.db.schema_version(), 1)
        again = PlayerDB(self.path)  # reopening does not re-run migrations
        self.assertEqual(again.schema_version(), 1)
        again.close()

    def test_accounts_and_characters_are_isolated(self):
        self.db.ensure_account(0xA, "alpha")
        self.db.ensure_account(0xB, "bravo")
        a1 = self.db.create_character(0xA, b"titan", "501:a:1")
        a2 = self.db.create_character(0xA, b"hunter", "501:a:2")
        b1 = self.db.create_character(0xB, b"warlock", "501:b:1")
        self.assertEqual(len({a1, a2, b1}), 3)
        self.assertTrue(all(c >= ID_BASE for c in (a1, a2, b1)))
        self.assertEqual([c["slot"] for c in self.db.characters(0xA)], [0, 1])
        self.assertEqual([c["character_id"] for c in self.db.characters(0xB)], [b1])
        # a resent 501 does not create a second character
        self.assertEqual(self.db.create_character(0xA, b"titan", "501:a:1"), a1)
        self.assertEqual(len(self.db.characters(0xA)), 2)

    def test_slots_limit_and_deletion_frees_slot(self):
        self.db.ensure_account(1, "p")
        ids = [self.db.create_character(1, b"x", f"k{i}") for i in range(3)]
        with self.assertRaises(ValueError):
            self.db.create_character(1, b"x", "k4")
        self.assertTrue(self.db.delete_character(1, ids[1]))
        self.assertEqual(self.db.characters(1)[1]["slot"], 2)
        new = self.db.create_character(1, b"x", "k5")
        self.assertEqual({c["character_id"]: c["slot"] for c in self.db.characters(1)}[new], 1)

    def test_grants_are_idempotent_and_owned(self):
        self.db.ensure_account(0xA, "alpha")
        self.db.ensure_account(0xB, "bravo")
        a = self.db.create_character(0xA, b"t", "ca")
        b = self.db.create_character(0xB, b"t", "cb")
        first = self.db.grant_items(0xA, a, [{"item_hash": 710, "location": "equipped", "equip_slot": 0}],
                                    "starter", "starter:a")
        again = self.db.grant_items(0xA, a, [{"item_hash": 710}], "starter", "starter:a")
        self.assertEqual(first, again)
        self.assertEqual(len(self.db.items(0xA)), 1)
        with self.assertRaises(ValueError):  # another account's character
            self.db.grant_items(0xA, b, [{"item_hash": 1}], "x", "bad")
        self.assertEqual(self.db.items(0xB), [])

    def test_optimistic_concurrency_on_moves(self):
        self.db.ensure_account(1, "p")
        c = self.db.create_character(1, b"t", "c")
        (item,) = self.db.grant_items(1, c, [{"item_hash": 5}], "test", "g")
        version = self.db.items(1)[0]["version"]
        self.db.move_item(1, item, version, None, "vault")
        with self.assertRaises(ConflictError):  # a second client acting on the stale version
            self.db.move_item(1, item, version, c, "inventory")
        self.assertEqual(self.db.items(1)[0]["location"], "vault")

    def test_concurrent_grants_from_many_sessions(self):
        accounts = list(range(1, 9))
        characters = {}
        for a in accounts:
            self.db.ensure_account(a, f"p{a}")
            characters[a] = self.db.create_character(a, b"t", f"c{a}")
        errors = []

        def session(account):
            db = PlayerDB(self.path)  # each "client session" with its own connection
            try:
                for n in range(25):
                    db.grant_items(account, characters[account], [{"item_hash": n}], "loot",
                                   f"loot:{account}:{n}")
                    db.grant_items(account, characters[account], [{"item_hash": n}], "loot",
                                   f"loot:{account}:{n}")  # duplicate delivery
            except Exception as error:  # noqa: BLE001 - surfaced through the assertion below
                errors.append(error)
            finally:
                db.close()

        threads = [threading.Thread(target=session, args=(a,)) for a in accounts]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
        self.assertEqual(errors, [])
        all_ids = []
        for a in accounts:
            items = self.db.items(a)
            self.assertEqual(len(items), 25)
            self.assertTrue(all(i["character_id"] == characters[a] for i in items))
            all_ids += [i["item_instance_id"] for i in items]
        self.assertEqual(len(all_ids), len(set(all_ids)))

    def test_import_legacy_store_keeps_ids(self):
        legacy = Path(self.dir.name) / "characters.json"
        legacy.write_text(json.dumps({"0xa3d4a2c8fbea6b9": {"id": 0x0A3D4A2C8FBEA6BA, "request": "c0e0",
                                                           "account_data": "e000"}}), encoding="utf-8")
        self.assertEqual(import_store(self.db, legacy), [(0xA3D4A2C8FBEA6B9, 0x0A3D4A2C8FBEA6BA)])
        self.assertEqual(import_store(self.db, legacy), [(0xA3D4A2C8FBEA6B9, 0x0A3D4A2C8FBEA6BA)])  # idempotent
        chars = self.db.characters(0xA3D4A2C8FBEA6B9)
        self.assertEqual([(c["character_id"], c["slot"], c["create_request"]) for c in chars],
                         [(0x0A3D4A2C8FBEA6BA, 0, bytes.fromhex("c0e0"))])
        self.assertEqual(self.db.account(0xA3D4A2C8FBEA6B9)["account_data"], bytes.fromhex("e000"))
        # a new character on the same account takes the next slot and a fresh id
        new = self.db.create_character(0xA3D4A2C8FBEA6B9, b"x", "501:new")
        self.assertNotEqual(new, 0x0A3D4A2C8FBEA6BA)
        self.assertEqual({c["character_id"]: c["slot"] for c in self.db.characters(0xA3D4A2C8FBEA6B9)}[new], 1)

    def test_allocator_skips_imported_ids(self):
        self.db.ensure_account(1, "p")
        self.db.import_character(1, ID_BASE + 1, b"x", "import:1")
        allocated = self.db.create_character(1, b"y", "c")
        self.assertNotEqual(allocated, ID_BASE + 1)
        with self.assertRaises(ValueError):
            self.db.import_character(1, ID_BASE + 1, b"x", "import:other-key")


if __name__ == "__main__":
    unittest.main()
