import tempfile
import threading
import unittest
from pathlib import Path

from player_db import ConflictError, ID_BASE, PlayerDB


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


if __name__ == "__main__":
    unittest.main()
