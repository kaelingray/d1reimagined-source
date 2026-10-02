# Player database (`server/player_db.py`)

The durable store for all player state, replacing the single-client JSON store
(`SIGNON_LOCAL/characters-0124.json`: one character per account, non-transactional, starter item
ids `0x100 + slot`). Status: **schema v1 implemented and tested; not yet wired into the BAP
stand-in** (next step). Multiplayer readiness: MULTIPLAYER-AWARE (concurrency tested with
simulated sessions; no real two-client run yet).

## Principles

See [`architecture-multiplayer.md`](architecture-multiplayer.md). In short: explicit ownership
(account, character), database-allocated globally unique ids, transactions for every write,
idempotency keys for grants, optimistic versions for client-driven mutations, numbered migrations.

## Schema v1 (migration 1)

| Table | Key | Owner | Purpose |
| --- | --- | --- | --- |
| `schema_migrations` | version | installation | applied migrations and time |
| `id_allocator` | name | installation | 64-bit object id counter (`ID_BASE` 0x0A3D000000000000 + n) for characters and item instances |
| `accounts` | account_id (from sign-in) | account | display name, version, service-701 account data |
| `characters` | character_id (allocated) | account | slot (unique per live account slot), the client's 501 creation request, soft delete, version |
| `items` | item_instance_id (allocated) | account; character or NULL (vault) | definition hash, location (inventory/equipped/vault/postmaster), equip slot, quantity, per-instance JSON state, version |
| `grants` | idempotency_key | account/character | ledger of every grant (character creation, items): reason and result; a replayed key returns the original result |

Planned next tables (each through a new migration): currencies (per account/character, ledger
based), progression and reputation, quests/objectives/bounties, checkpoints (owner explicit:
character, fireteam or activity instance), unlocks and Grimoire, vendor state (global vs per
player), sessions and presence, fireteams and members, matchmaking tickets/groups, activity and
world instances with memberships, reward history.

## Migration from the legacy JSON store

`python server/player_db.py import-store DB characters-0124.json` adopts each stored character
with its existing id (`import_character`, idempotency key `import:<account>:<id>`, refuses an
id that exists) and the account's 701 user data, so recipes naming the character id keep
working. The allocator skips ids that imports occupy. `python server/player_db.py show DB` lists
accounts and characters. The local test database is `SIGNON_LOCAL/players-0124.db` (outside Git).

## Operational rules

- SQLite in WAL mode; one connection per thread (`PlayerDB` keeps a thread-local connection).
- Migrations run on open, one transaction each; a second process opening a fresh file cannot apply
  a migration twice (primary key on `schema_migrations`).
- Backups: copy the database file with SQLite's online backup API (tool to be added) before
  schema migrations and test sessions; reverse-engineering runs use a throwaway copy, never the
  only database.

## Tests (`server/test_player_db.py`)

Migrations recorded and not re-run; account/character isolation; three-slot limit and slot reuse
after deletion; a resent creation request returns the same character; grants idempotent and
refused for another account's character; optimistic-concurrency conflict on a stale item move;
eight concurrent sessions each granting 25 items twice (duplicate delivery) produce exactly 25
items per account, all owned correctly, all instance ids unique.
