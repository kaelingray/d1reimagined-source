# Loot, engrams, world drops and rewards (research note, opened 2026-10-02)

Scope items 25-27 of [`preservation-scope.md`](preservation-scope.md). Status: **INVESTIGATING**.
Target build 01.24. Labels: **OBSERVED** (seen in a run or read directly from the binary/data),
**INFERRED** (reasoned from names or structure, not yet confirmed), **OPEN**.

## Evidence so far (01.24 EBOOT strings, OBSERVED)

| String | Address | Reading |
| --- | --- | --- |
| `investment_loot_object_manager` | `0x12f0ed8` | a manager for loot objects in the world, part of the investment (BAP item) system |
| `loot-bauble` | `0x12f0ecc` | the in-world pickup (engram/item "bauble") |
| `loot_manager_record_client_data_from_nonce` | `0x12f0ea0` | loot is keyed by a **nonce**; client data recorded per nonce |
| `loot_component_send_pickup_message` | `0x13cd260` | picking up sends a message (to the activity host or BAP) |
| `c_cui_loot_pickup_item_server_message_component` | `0x133a684` | UI for the server's answer to a pickup |
| `c_cui_investment_loot_pipe_component`, `c_cui_investment_character_loot_state_component` | `0x133a4e4`, `0x133a3a4` | loot "pipe" (delivery) and per-character loot state |
| `investment_activity_reward_tracker` | `0x13053e0` | activity completion rewards tracked through investment |
| `cui::c_loot_reward_item_container`, `..._progression_reward_...`, `..._currency_reward_...`, `..._quest_step_reward_...`, `..._record_completion_reward_...` | `0x133e...-0x133f...` | reward display kinds: loot, progression, currency, quest step, record |
| `host deterministic random`, `host nondeterministic random`, `client deterministic random`, `client nondeterministic random` | `0x13c5160`, `0x12ddc20` | four separate random streams: host vs client, deterministic (seeded, reproducible across peers) vs not |
| `Change world (activity_name='%s', symbol='%s', rebuild_omaha_options='%s', random_seed='%c 0x%08X')` | `0x12f4318` | the engine's change-world request carries a **random seed** |
| `item_loot`, `reward sheet`, `reward site`, `reward stream`, `reward mapping` | various | content/tag vocabulary for reward definitions |

## Working model (INFERRED, to be confirmed)

1. **Who decides drops.** In Destiny 1 every item instance lives on Bungie's investment servers
   (BAP services 2xx-9xx in [`investment-services.md`](investment-services.md)). The client never
   invents an item instance id. A world drop is therefore most likely:
   activity host or investment server decides "this kill/chest yields loot" -> a **loot nonce**
   (and probably a rarity/class hint so the bauble can be coloured) is delivered to the client ->
   the client spawns a `loot-bauble` -> on pickup, `loot_component_send_pickup_message` sends the
   nonce -> the investment server resolves the nonce into an item instance (or engram) and returns
   it through a character inventory update (qz family push) -> UI shows the pickup.
2. **RNG.** The "deterministic random" streams are seeded (the change-world seed) so that peers in
   a fireteam simulate the same world. Loot, being per-player, should come from a
   **non-deterministic or server-side** source; the server-side generator is not in the client
   and has to be re-created. Combat/AI randomness (spread, AI decisions) is likely the
   deterministic client stream and needs no server.
3. **Engrams.** Engrams are items (in the item list, 6,557 decoded). Decoding at the Cryptarch is a
   vendor/investment action (a BAP service) that replaces the engram instance with a generated
   item; the generation rules (loot pools, level/Light ranges) were server-side.
4. **Loot pools.** Item definitions, buckets, tiers, levels and the reward-related tags above are
   local data; the **tables that pick an item for a given activity/enemy/engram** are probably a
   mix of local content tags (`reward sheet`, `item_loot`) and server logic. Which part is local is
   the key OPEN question.

## Investigation plan

1. Find the code around the loot strings (`d1_eboot.py xref`) and the BAP service or host message
   the pickup uses; map the request/response types (services 304/305/1218 share a
   `u64 x2 + {0x8080547d, 0x80804792, 0x80804407} arrays` request shape and are candidates for
   activity/loot reward reports).
2. Find where loot nonces arrive: a host message type (candidates in the 01.24 host table:
   10/18 with 0x2cc-byte structs, 23/24 with large buffers, 38 with a blob) or a replicated object
   kind (loot objects as replicated entities).
3. Decode the reward-related content tags in the catalog (`reward sheet`, `item_loot`, activity
   definitions' reward fields) to see what loot tables exist locally.
4. Decode the Cryptarch / engram decode path (vendor purchase 901 and the item-action services
   902/903).
5. Only then design the backend: a reward service that owns item-instance creation, a seeded and
   logged RNG (reproducible for tests, configurable per installation), loot tables as data, and
   per-player rewards (each fireteam member rolls separately).

## Backend design constraints (decided now)

- Item instances are created only by the backend's investment service; every grant is a logged
  transaction (who, what, why: activity/kill/chest/vendor, RNG seed and draw).
- RNG: one seeded generator per grant source, seed recorded, so a test can replay a reward
  sequence exactly; a production installation can seed from time.
- Multiplayer: rewards are per player and never shared state; the activity host only reports
  events (kills, chest opens, completions) with the player who earned them.
- Loot tables are data files derived from the game's own definitions wherever they exist;
  invented tables (where the original data was server-only) are marked as reconstructions, with
  their source (community drop-rate research, recordings) cited.

## Code anchors (OBSERVED, 01.24)

- `loot_component_send_pickup_message`: used in **`0xcbb6c4`** (the pickup sender). It calls
  `0x47c1a0` and `0x47d684(…, 4)` in the BAP message block (0x47xxxx holds the BAP codec/request
  functions, e.g. the user-message decoder `0x47bf9c`), so a pickup most likely becomes a BAP
  request; `0x276c44` (activity-host messaging) is also called. To trace next.
- `investment_loot_object_manager`: registered in `0x2b3fb8`.
- `loot-bauble`: used in `0x2b2c00` (the bauble object, same module as the loot manager).
- `investment_activity_reward_tracker`: registered in `0x482268` (BAP/investment block).
- In `0xcbb6c4`: `0x47c1a0()` returns `*(0x1ad5fa0 + 4)` (an investment/BAP singleton) and
  `0x47d684(table, 4, &id)` scans a **257-entry table of 0x40-byte records** for kind byte 4 with
  a matching u64 id (+0x20) and returns the record's +0x3c field (+0x60 relative to the scan
  base). So a pickup first resolves the loot object against client-side investment data keyed by
  a 64-bit id (OBSERVED: code; the meaning of kind 4 is OPEN).
- Kind 4 in that table is investment **family 4 = "self"** (`FAMILY_TYPES` in `bap_server.py`,
  table `0xb6068c`): the 257 records are the client's family-object cache {family, u64 root}.
  So the pickup sender first looks up the player's own self-family object - the per-character
  investment state is where loot state lives (fits `c_cui_investment_character_loot_state_component`).
  INFERRED next step: loot nonces/baubles are fields of the self family (or a family-4 child
  object) that the backend pushes via qz updates; decode the self family's loot-related fields
  in the 01.24 schema.
