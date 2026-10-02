"""Durable player database for the preservation backend (multiplayer-first).

Replaces the single-client JSON store (`SIGNON_LOCAL/characters-*.json`, one character per
account, read-modify-write) step by step; see notes/architecture-multiplayer.md (debt register)
and notes/player-database.md.

Design rules:
- Every row is owned by an explicit scope (account, character); nothing is "the current player".
- Identifiers are allocated by the database and globally unique within an installation
  (characters and item instances use 64-bit ids from one allocator).
- Writes run in transactions. Grants carry an idempotency key: replaying the same grant (network
  retry, duplicate message, reconnect) returns the original result instead of granting twice.
- Rows that clients mutate carry a version; updates check it (optimistic concurrency).
- The schema is created and upgraded only by the numbered MIGRATIONS below, recorded in
  `schema_migrations`, so every installation can be rebuilt and audited.

SQLite is the first backend (one file per installation, WAL mode so several server components
and connections can read while one writes). The API is narrow so it can move to a server
database later without touching callers.
"""
import json
import sqlite3
import threading
import time
from contextlib import contextmanager
from pathlib import Path

# 64-bit ids: a fixed installation prefix keeps them distinct from ids that 01.24 assigns itself
# and from test fixtures; the counter lives in the database, never in process memory.
ID_BASE = 0x0A3D_0000_0000_0000

MIGRATIONS = [
    # 1: core ownership graph and the grant ledger
    """
    CREATE TABLE id_allocator (name TEXT PRIMARY KEY, next INTEGER NOT NULL);
    INSERT INTO id_allocator VALUES ('object', 1);
    CREATE TABLE accounts (
        account_id   INTEGER PRIMARY KEY,           -- from sign-in (platform identity)
        display_name TEXT NOT NULL,
        created_at   REAL NOT NULL,
        version      INTEGER NOT NULL DEFAULT 1,
        account_data BLOB                            -- service 701 user data (settings)
    );
    CREATE TABLE characters (
        character_id INTEGER PRIMARY KEY,           -- allocated here, globally unique
        account_id   INTEGER NOT NULL REFERENCES accounts(account_id),
        slot         INTEGER NOT NULL,              -- character select slot 0..2
        create_request BLOB NOT NULL,               -- the client's service 501 request (class, appearance)
        created_at   REAL NOT NULL,
        deleted_at   REAL,
        version      INTEGER NOT NULL DEFAULT 1
    );
    CREATE UNIQUE INDEX characters_slot ON characters(account_id, slot) WHERE deleted_at IS NULL;
    CREATE TABLE items (
        item_instance_id INTEGER PRIMARY KEY,       -- allocated here, globally unique
        item_hash    INTEGER NOT NULL,              -- definition (item index/hash in the build's item list)
        account_id   INTEGER NOT NULL REFERENCES accounts(account_id),
        character_id INTEGER REFERENCES characters(character_id),  -- NULL = account-scoped (vault)
        location     TEXT NOT NULL,                 -- 'inventory', 'equipped', 'vault', 'postmaster'
        equip_slot   INTEGER,
        quantity     INTEGER NOT NULL DEFAULT 1,
        state        TEXT NOT NULL DEFAULT '{}',    -- JSON: perks, dyes, levels... (per instance)
        version      INTEGER NOT NULL DEFAULT 1
    );
    CREATE INDEX items_owner ON items(account_id, character_id, location);
    CREATE TABLE grants (
        idempotency_key TEXT PRIMARY KEY,           -- e.g. 'reward:<instance>:<event>:<character>'
        account_id   INTEGER NOT NULL,
        character_id INTEGER,
        reason       TEXT NOT NULL,                 -- activity/kill/chest/vendor/starter...
        result       TEXT NOT NULL,                 -- JSON of what was granted (item instance ids)
        created_at   REAL NOT NULL
    );
    """,
]


class ConflictError(Exception):
    """A row changed since the caller read it (optimistic concurrency)."""


class PlayerDB:
    def __init__(self, path):
        self.path = str(path)
        self._local = threading.local()
        conn = self._conn()
        conn.execute("CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY, applied_at REAL)")
        for number, script in enumerate(MIGRATIONS, 1):
            if conn.execute("SELECT 1 FROM schema_migrations WHERE version=?", (number,)).fetchone():
                continue
            # one transaction per migration (executescript cannot run inside an open one); the
            # INSERT's primary key makes a concurrent second applier fail and roll back
            try:
                conn.executescript(f"BEGIN IMMEDIATE;\n{script}\n"
                                   f"INSERT INTO schema_migrations VALUES ({number}, {time.time()});\nCOMMIT;")
            except sqlite3.Error:
                if conn.in_transaction:
                    conn.execute("ROLLBACK")
                if not conn.execute("SELECT 1 FROM schema_migrations WHERE version=?", (number,)).fetchone():
                    raise

    # one connection per thread: SQLite connections are not shared across threads
    def _conn(self):
        conn = getattr(self._local, "conn", None)
        if conn is None:
            conn = sqlite3.connect(self.path, timeout=30, isolation_level=None)
            conn.execute("PRAGMA journal_mode=WAL")
            conn.execute("PRAGMA foreign_keys=ON")
            self._local.conn = conn
        return conn

    @contextmanager
    def _tx(self):
        conn = self._conn()
        conn.execute("BEGIN IMMEDIATE")
        try:
            yield conn
            conn.execute("COMMIT")
        except BaseException:
            conn.execute("ROLLBACK")
            raise

    def close(self):
        conn = getattr(self._local, "conn", None)
        if conn is not None:
            conn.close()
            self._local.conn = None

    def schema_version(self):
        return self._conn().execute("SELECT MAX(version) FROM schema_migrations").fetchone()[0]

    @staticmethod
    def _allocate(db):
        while True:
            (counter,) = db.execute("SELECT next FROM id_allocator WHERE name='object'").fetchone()
            db.execute("UPDATE id_allocator SET next = next + 1 WHERE name='object'")
            candidate = ID_BASE + counter
            # imported legacy ids live in the same 64-bit space: never hand one out twice
            taken = db.execute("SELECT 1 FROM characters WHERE character_id=? UNION ALL "
                               "SELECT 1 FROM items WHERE item_instance_id=?", (candidate, candidate)).fetchone()
            if not taken:
                return candidate

    # accounts
    def ensure_account(self, account_id, display_name):
        with self._tx() as db:
            db.execute("INSERT OR IGNORE INTO accounts (account_id, display_name, created_at) VALUES (?, ?, ?)",
                       (account_id, display_name, time.time()))
        return self.account(account_id)

    def account(self, account_id):
        row = self._conn().execute("SELECT account_id, display_name, version, account_data FROM accounts "
                                   "WHERE account_id=?", (account_id,)).fetchone()
        return None if row is None else {"account_id": row[0], "display_name": row[1], "version": row[2],
                                         "account_data": row[3]}

    def set_account_data(self, account_id, data, expected_version=None):
        with self._tx() as db:
            if expected_version is None:
                cur = db.execute("UPDATE accounts SET account_data=?, version=version+1 WHERE account_id=?",
                                 (data, account_id))
            else:
                cur = db.execute("UPDATE accounts SET account_data=?, version=version+1 "
                                 "WHERE account_id=? AND version=?", (data, account_id, expected_version))
            if cur.rowcount != 1:
                raise ConflictError(f"account {account_id:#x} changed or missing")

    # characters
    def create_character(self, account_id, create_request, idempotency_key):
        """Create a character in the account's first free slot; replaying the same key returns the
        same character instead of creating another (a resent service 501)."""
        with self._tx() as db:
            done = db.execute("SELECT result FROM grants WHERE idempotency_key=?", (idempotency_key,)).fetchone()
            if done:
                return json.loads(done[0])["character_id"]
            used = {row[0] for row in db.execute(
                "SELECT slot FROM characters WHERE account_id=? AND deleted_at IS NULL", (account_id,))}
            free = [s for s in range(3) if s not in used]
            if not free:
                raise ValueError(f"account {account_id:#x} has no free character slot")
            character_id = self._allocate(db)
            db.execute("INSERT INTO characters (character_id, account_id, slot, create_request, created_at) "
                       "VALUES (?, ?, ?, ?, ?)", (character_id, account_id, free[0], create_request, time.time()))
            db.execute("INSERT INTO grants VALUES (?, ?, ?, ?, ?, ?)",
                       (idempotency_key, account_id, character_id, "create_character",
                        json.dumps({"character_id": character_id}), time.time()))
            return character_id

    def import_character(self, account_id, character_id, create_request, idempotency_key):
        """Adopt a character created elsewhere (e.g. the legacy JSON store) with its existing id,
        so running sessions and recipes that name that id keep working. Idempotent by key."""
        with self._tx() as db:
            done = db.execute("SELECT result FROM grants WHERE idempotency_key=?", (idempotency_key,)).fetchone()
            if done:
                return json.loads(done[0])["character_id"]
            if db.execute("SELECT 1 FROM characters WHERE character_id=?", (character_id,)).fetchone():
                raise ValueError(f"character id {character_id:#x} already exists")
            used = {row[0] for row in db.execute(
                "SELECT slot FROM characters WHERE account_id=? AND deleted_at IS NULL", (account_id,))}
            free = [slot for slot in range(3) if slot not in used]
            if not free:
                raise ValueError(f"account {account_id:#x} has no free character slot")
            db.execute("INSERT INTO characters (character_id, account_id, slot, create_request, created_at) "
                       "VALUES (?, ?, ?, ?, ?)", (character_id, account_id, free[0], create_request, time.time()))
            db.execute("INSERT INTO grants VALUES (?, ?, ?, ?, ?, ?)",
                       (idempotency_key, account_id, character_id, "import_character",
                        json.dumps({"character_id": character_id}), time.time()))
            return character_id

    def characters(self, account_id):
        return [{"character_id": r[0], "slot": r[1], "create_request": r[2], "version": r[3]}
                for r in self._conn().execute(
                    "SELECT character_id, slot, create_request, version FROM characters "
                    "WHERE account_id=? AND deleted_at IS NULL ORDER BY slot", (account_id,))]

    def delete_character(self, account_id, character_id):
        with self._tx() as db:
            cur = db.execute("UPDATE characters SET deleted_at=?, version=version+1 "
                             "WHERE character_id=? AND account_id=? AND deleted_at IS NULL",
                             (time.time(), character_id, account_id))
            return cur.rowcount == 1

    # items
    def grant_items(self, account_id, character_id, items, reason, idempotency_key):
        """Create item instances [{item_hash, location, equip_slot?, quantity?, state?}] for one
        owner in one transaction. Replaying the key returns the original instance ids."""
        with self._tx() as db:
            done = db.execute("SELECT result FROM grants WHERE idempotency_key=?", (idempotency_key,)).fetchone()
            if done:
                return json.loads(done[0])["items"]
            owner = db.execute("SELECT account_id FROM characters WHERE character_id=? AND deleted_at IS NULL",
                               (character_id,)).fetchone() if character_id is not None else (account_id,)
            if owner is None or owner[0] != account_id:
                raise ValueError("character does not belong to the account")
            ids = []
            for item in items:
                instance_id = self._allocate(db)
                db.execute("INSERT INTO items (item_instance_id, item_hash, account_id, character_id, location, "
                           "equip_slot, quantity, state) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                           (instance_id, item["item_hash"], account_id, character_id, item.get("location", "inventory"),
                            item.get("equip_slot"), item.get("quantity", 1), json.dumps(item.get("state", {}))))
                ids.append(instance_id)
            db.execute("INSERT INTO grants VALUES (?, ?, ?, ?, ?, ?)",
                       (idempotency_key, account_id, character_id, reason, json.dumps({"items": ids}), time.time()))
            return ids

    def items(self, account_id, character_id=None, location=None):
        query = "SELECT item_instance_id, item_hash, character_id, location, equip_slot, quantity, state, version " \
                "FROM items WHERE account_id=?"
        params = [account_id]
        if character_id is not None:
            query += " AND character_id=?"
            params.append(character_id)
        if location is not None:
            query += " AND location=?"
            params.append(location)
        return [{"item_instance_id": r[0], "item_hash": r[1], "character_id": r[2], "location": r[3],
                 "equip_slot": r[4], "quantity": r[5], "state": json.loads(r[6]), "version": r[7]}
                for r in self._conn().execute(query + " ORDER BY item_instance_id", params)]

    def move_item(self, account_id, item_instance_id, expected_version, character_id, location, equip_slot=None):
        """Move one instance (transfer, vault, equip) if it is unchanged since the caller read it."""
        with self._tx() as db:
            cur = db.execute("UPDATE items SET character_id=?, location=?, equip_slot=?, version=version+1 "
                             "WHERE item_instance_id=? AND account_id=? AND version=?",
                             (character_id, location, equip_slot, item_instance_id, account_id, expected_version))
            if cur.rowcount != 1:
                raise ConflictError(f"item {item_instance_id:#x} changed or not owned by {account_id:#x}")


def import_store(db, path):
    """Import the legacy single-client JSON store ({account: {id, request, account_data}}) into a
    PlayerDB; safe to run again (idempotency keys). Returns [(account, character)]."""
    store = json.loads(Path(path).read_text(encoding="utf-8"))
    imported = []
    for account_hex, entry in store.items():
        account_id = int(account_hex, 16)
        db.ensure_account(account_id, account_hex)
        if "id" in entry and "request" in entry:
            character_id = db.import_character(account_id, int(entry["id"]), bytes.fromhex(entry["request"]),
                                               f"import:{account_hex}:{int(entry['id']):#x}")
            imported.append((account_id, character_id))
        if entry.get("account_data") and not (db.account(account_id) or {}).get("account_data"):
            db.set_account_data(account_id, bytes.fromhex(entry["account_data"]))
    return imported


if __name__ == "__main__":
    import argparse
    ap = argparse.ArgumentParser(description="player database tools")
    sub = ap.add_subparsers(dest="cmd", required=True)
    imp = sub.add_parser("import-store", help="import the legacy JSON character store")
    imp.add_argument("db", type=Path)
    imp.add_argument("store", type=Path)
    show = sub.add_parser("show", help="list accounts and characters")
    show.add_argument("db", type=Path)
    args = ap.parse_args()
    database = PlayerDB(args.db)
    try:
        if args.cmd == "import-store":
            for account_id, character_id in import_store(database, args.store):
                print(f"account {account_id:#x}: character {character_id:#x}")
        else:
            for (account_id,) in database._conn().execute("SELECT account_id FROM accounts ORDER BY account_id"):
                chars = ", ".join(f"slot {c['slot']}: {c['character_id']:#x}" for c in database.characters(account_id))
                data = (database.account(account_id) or {}).get("account_data")
                print(f"account {account_id:#x} ({chars or 'no characters'}; account data "
                      f"{len(data) if data else 0} bytes; schema v{database.schema_version()})")
    finally:
        database.close()
