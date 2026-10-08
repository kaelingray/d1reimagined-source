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
| `loot_component_send_pickup_message` | `0x13cd260` | local pickup callback; service 601 downstream (corrected 2026-10-08) |
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

1. Trace a live service-601 request/response and identify which action creates the
   rifle's nonce and position cache. Services 304/305/1218 share a
   `u64 x2 + {0x8080547d, 0x80804792, 0x80804407} arrays` request shape and remain candidates for
   activity/loot reward reports.
2. Connect the now-mapped self-family pending-loot records to that native action.
   The callback name alone does not identify a host wire message or replicated kind.
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

- `loot_component_send_pickup_message`: used in **`0xcbb6c4`** (the pickup sender).
  Its deferred callback `0xcbb5d4` calls `0x472518`; that path selects service ID
  `0x259` (601) and queues the request. This is static code evidence; no native
  rifle claim has been observed yet.
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
  The loot table is now mapped below.
- **Correction, 2026-10-08:** `0x276b08` and `0x276c44` are local deferred
  callback queues. They copy the supplied bytes into a bounded arena and store
  a payload pointer plus a function descriptor. The diagnostic string is not a
  wire message name. `0x1853168` resolves to `0x2b2f74`, which fills the local
  nonce cache; `0x1886af8` resolves to `0xcbb5d4`, which calls the same
  service-601 path as the immediate pickup branch. The earlier claim that this
  proved a pickup message to the activity authority is withdrawn. The meaning
  of the dispatch-mode bit at `*0x1a93690` remains unnamed.
- Service 601 (`0x3bf631f4`): request {u3, {i32, u64}, i32}, response {status} {{i32, u64}, i32}
  - consistent with claiming a pending reward by source kind and nonce. The
  `{i32, u64}` pair is an opaque nonce, not an established item hash/instance
  pair. Remaining field meanings and a live request/response still need validation.

## Pending world loot (static mapping, 2026-10-08)

The full final-build schema and native `0x2b4070` agree on this layout:

| Location | Type / layout | Evidence |
| --- | --- | --- |
| Self character `0x80801714` +`0x3bd0` | `0x80801701` pending-loot state | Native manager reads the first slot at self +`0x3bd8` |
| Pending state +8 | `0x80804671`, 64 slots with stride `0x38` | Manager calls `0x2b36f8` once per slot |
| Each slot | `0x80801829`: item index +0, word +4, source kind +8, nonce record +`0x10`, optional state +`0x28` | Schema and native field reads |
| Nonce record | `0x808018ff`: `{i32, u64}` at +0/+8, another i32 at +`0x10` | `0xc434e4` compares the key and full 64-bit ID |
| Manager +`0x200` / `0x204` / `0x208` | Ring capacity / start / count | Native cache insertion and backwards search |
| Manager +`0x210` | 64 client-data records, stride `0x30` | Each holds nonce, position at +`0x10`, world ID at +`0x20` |
| Manager +`0xe10` | One fallback client-data record per pending slot | Cleared when its nonce no longer matches |

`0x2b36f8` considers source kinds **3 and 5** for world spawning. `0x2b3240`
looks up the item's definition, chooses the newest matching nonce in the ring
(or the slot fallback), and requires its world ID to match the current world.
Only then does it resolve world art, create a `loot-bauble`, attach the loot
record to its component and bind the character owner. A pending record alone
therefore does not prove that a pickup can appear.

The current world ID comes from `0x14dcf4 -> 0x158dbc`: controller
`*(0x1a1dc80)` +`0x110c`, bits 3..8, or -1 for an absent handle. Manager
+`0x1a10` contains a four-word world signature returned by `0x14de84`; its
first word must not be used as the world ID.

`0x2b279c` records an action's nonce and its source unit position, falling back
to action +`0x2a0`; `0x2b2a60` is another recording path. Their direct callers
are callback handlers `0x5aea30` and `0x5aede4`. The mission/backend mechanism
that creates the correct rifle action and nonce remains open. This mapping is
not a recovered drop table or a native pickup acceptance result.

JOIN217 checks this mapping in the native client using an unarmed starter and
two synthetic pending rifle records (source kinds 3 and 5). Both manager entry
paths read them. Three read-only samples, including one after ordinary movement
to the rifle, have an empty ring, unmatched slot caches and absent unit-reference
pairs. No position-recording callback or service-601 entry is observed, and no
601 request reaches the backend. Synthetic nonces cannot establish the original
reward identity; this result verifies the diagnostic and illustrates why pending
records alone are insufficient. See [the bounded run](rifle-pickup-0124.md#join217-pending-records-reach-the-manager-no-native-pickup).

Use `scripts/rpcs3_final124_loot.py --process-manifest <exact-run-manifest>
--self-character <observed-address> --character-id <owned-character-id> --out
<new-private-path>` to inspect the two halves. Derive the self address from the
first-slot argument to native entry `0x2b36f8`; do not treat a watch at an
arbitrary return instruction as supported. The probe checks the character ID,
cache bounds, finite positions and rereads its inputs. It rejects changing
samples and preserves earlier output. A position match is only a prerequisite;
unit occupancy, visible art, pickup, equipment and durable grants require
separate evidence.

JOIN218 locates the native incident catalog and correlates one cinematic record
with the client callback's selector. The position-recording callbacks are native
incident dispatch paths, not BAP reply handlers. The corrected script-event codec
is already present in the final schema. This narrows the missing rifle action
investigation, but supplies neither its body nor its nonce/position. See
[the catalog evidence and address correction](incident-diagnostics.md#join218-native-catalog-and-resource-address-correction).
