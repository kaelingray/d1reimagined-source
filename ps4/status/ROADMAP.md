# ROADMAP - from here to a 3v3 private Elimination match

> ## 🎉 2026-08-11 - **THE IN-WORLD MILESTONE IS REACHED.** A **rendered Guardian** stands in
> `pvp_moonbase` (type-9 biped, Ghost out). ⇒ `design/GUARDIAN-IN-WORLD.md`. The route below is
> therefore past "get in-world" and into **make it legitimate and playable**: earn tasks 9/10 onto
> the wire (drop the 2 code hooks), then possession/control, then fps (6-7 in-game), then the
> mission/enemy/objective layer, then 3v3. ⚠ "3v3" is still NOT attested in the owned build.

**The honest map of what is left.** Written 2026-08-06 because "how much further?" had no answer in
the tree - `status/shadps4.md` had the boot ladder, but nothing covered the route past orbit to the
actual goal.

Every stage below is named **from the client's own strings**, not invented: 22 `bootflow:` state
literals, 260 `error.*` reason strings, and the `networking:*` subsystem prefixes. So the route is
`[C]` as a *list of things that exist*; the difficulty estimates are `[I]`.

---

## Where we are

```
STAGE 1  BOOT ─────────────────────────────────────────── 12 of 14 gates DONE
  media_check → bootload → intro_screen_display → start
    → platform_signin ✓            Layer A (our ShadNet server)
    → platform_account_warnings ✓
    → enumerate_content_rights ✓
    → account_signin ✓             PSN OAuth + /SignOn protobuf
    → content_check ✓              manifest URL via signon f4.3
    → package_registration ✓       1033-package manifest
    → bap_signin        ✓          full BAP setup + AES-GCM secured channel
    → investment_signin ◀── HERE   error.no_investment_connection ("chicken")
    → profile_setup                first-run DISPLAY SETTINGS - *not* character select
    → prepare_for_orbit            ← ORBIT (goal condition 2)
        → character:signin (23)    ← CHARACTER SELECT, reached FROM prepare_for_orbit

STAGE 2  IN-WORLD ── orbit → destination → activity launch
STAGE 3  PRIVATE MATCH ── fireteam → private activity host → 3v3 Elimination
```

**Conditional branches** (exist, not on our path unless something goes wrong): `eula`, `email`,
`upsell`, `queuing`, `content_install`, `confirm_account_transfer`, `illegal_dlc_media`,
`insufficient_content_space`.

---

## Stage 1 - the remaining three gates

| gate | what it needs | state |
|---|---|---|
| **`bap_signin`** | ✅ **DONE.** `0x1e→0x1f`, `0x19→0x1a`, `0x79→0x7a`, `0x12e→0x12f`, then `_connected`. Transport: TCP + AES-128-GCM. | `historicaldocs/design/bap-protocol.md`, `d1-server/crates/d1-bap` |
| **`investment_signin`** | Layer B2, and the real remaining work. Exit predicate `[C]`: `0x79BFF0` needs `h[+0x30]==7 && h[+0x34]==3` - an account record with `tag==3` bound to the local platform user, a non-null account object with its ready byte set, and a profile blob. **Critical path is hand-rolled bit/byte format, not protobuf.** | **in progress** - `historicaldocs/design/investment-profile-contract.md` |
| **`profile_setup`** | ⚠ **NOT character select.** It is the three first-run display screens (screen bounds → brightness → subtitles) - a 7-step *local* UI wizard at `0x79E830` that sends no message and reads no character. `[C]` | **no server work at all** |
| **`prepare_for_orbit`** | `[C]` strings exist: `orbit_map_loader: Unable to load Orbit scenario`, `The character's last-orbited-destination is invalid`, `no slice sets loaded, we should teleport you into space here`. | `[U]`, not started |

### ⚠ The biggest scope risk in this document was WRONG, and it is now retired `[C]`

This file previously said `profile_setup` was character select and "likely needs authoritative
character state, which means persistence" - and called that the single largest scope risk. **It is
not character select.** Character select is `character:signin` (state 23), reached *from*
`prepare_for_orbit`, one gate later. `profile_setup` sends no message and reads no character.

Three separate minimums, now measured rather than feared (`historicaldocs/design/investment-profile-contract.md`):

| milestone | durable state needed |
|---|---|
| **gate-minimal** - leaves `profile_setup` | **none.** An in-memory synthetic account record + profile blob satisfies every predicate. No database. |
| **orbit-minimal** - a controllable character at orbit | **one** durable character. The only boot-path field is `last_orbited_destination` (u32 hash, character view `+0x480`), and **unset is a tolerated failure** that falls back to two configured default destinations. |
| **PvP-minimal** - a private match | **six** durable, distinct account SOIDs - `networking:investment:peer` blocks while any peer lacks one. |

**Realistic read:** `investment_signin` is the real work - a whole second protocol layer, and its
critical path is **hand-rolled bit/byte format, not protobuf**. But the persistence cliff we were
bracing for does not exist between here and orbit.

---

## Stage 2 - in-world

`[U]` mostly unexplored. Named subsystems that must come alive: `world_controller:activity_manager`,
`world_controller:orbit_map_loader`, `networking:session:{create,channel,election,handoff,
handle_player_add}`, `networking:activity_client:*`.

The `world_controller` strings show the shape: `Change world (activity_name=…, symbol=…)`,
`Register package set`, `Change slice-set`, `failed to activate activity`.

---

## Stage 3 - and the goal is now defined by Bungie's own data `[C]`

> ### ⚠ READ THIS BEFORE THE TABLE BELOW - "3v3" is **not** attested in the owned build
>
> This section's era caveat is real but easy to skim past, and the document's **title** and sizing
> table both read as though 3v3 were settled. It is not.
> `historicaldocs/design/package-container-pvp-content.md` §8.4, from the owned 01.29 estate:
>
> > **team count and team size - `[U]`. "3v3" is not represented in any data recovered here.** It is
> > not in the eboot, not in any symbol, not in any readable entry. **Do not assume it.**
>
> `maxPlayers 6` comes from the **Nov 2020** manifest - four years newer than our Oct 2016 install.
> By this file's own rule (*"where the manifest and the owned install disagree, the install wins"*),
> the number is a **lead**, not a fact. The mode rules that would settle it live in
> `globals_pvp_0169`, where **721 of 998 blocks are Oodle-compressed** (§8.3) - so this is
> `[U]` until Oodle comes down or a retail capture answers it.
>
> ⇒ Keep "3v3 private Elimination" as the *goal*; do not let the **6** leak into a fixture, a
> struct, or an acceptance criterion as though it were measured.

**The manifest archive (2026-08-06) settles what "3v3 Elimination" actually is.** Agent A reported
that `"3v3" is not represented in any data I recovered` from the package corpus - correct for the
packages, which are 76% Oodle-compressed. It is represented in the manifest:

```
activityHash     2263964303
activityName     "Elimination"
activityType     "Elimination"           (typeHash 39921727)
destinationHash  2777041980              ("The Crucible")
placeHash        4088006058
maxPlayers       6                       ← 3v3
minParty         1
isPlaylist       true      isMatchmade   true
activityLevel    1         tier          -1
description      "Your allies are your lifeblood. No second chances except the ones you give."
```

Four Elimination activities exist in total: this playlist, a 12-player variant, another 6-player
playlist (`1415179315`), and **Inferno Elimination** (`1378430574`, type `1765876615`, 12 players).

⚠ **Era caveat, and it matters.** The archived manifest is version `88857.20.11.05.1145-2`
(**Nov 2020**) - four years newer than our 01.29 install (Oct 2016). Hashes and definitions may have
moved. Treat these as a **naming corpus and a strong lead**, not as era-correct truth: where the
manifest and the owned install disagree, **the install wins**. Confirming `2263964303` against the
01.29 packages is still required before it becomes contract.

Also `[C]` from the manifest, closing an open question about `last_orbited_destination`: real
destination hashes include `2612139675` The Last City · `2777041980` The Crucible · `2468353467` Old
Russia · `4233735899` Ocean of Storms, Moon · `518553403` Ishtar Sink, Venus · `4072959335` Meridian
Bay, Mars · `3393905939` The Reef · `2897855902` Dreadnaught. **There is no "Orbit" destination** -
which supports the caution against putting the orbit *activity* tag `0x80B52001` into a field that
takes a *destination* hash.

## Stage 3 - the honest part

This is **Layer C**, peer-to-peer, and it is the largest remaining piece. The client has a complete
vocabulary for it already:

- `networking:activity_client:` - `send_reservation_request`, `receive_message_from_host`,
  `activity_host_changed`, `send_client_authoritative_peer_identity_update`
- `error.private_activity_host_*` - ~30 distinct reasons: `connecting_to_bap`,
  `error_initializing_ipahp`, `rejected_host_join`, `sending_host_startup_request`,
  `session_host_migration`, `timed_out_waiting_for_entity_indices`,
  `timed_out_waiting_for_all_session_peers`, `session_parameter_mismatch`, …
- `error.starting_private_activity_instance_*` - the same family for instance startup
- `networking:session:election` / `handoff` - host election and migration

**What that vocabulary tells us `[I]`:** a private match is not "a server hosts a game". One *client*
becomes the **activity host** (`ipahp`), peers **reserve** slots against it, entity indices are
distributed, and hosting can **migrate**. That is the Aldridge/Tribes model in
`historicaldocs/design/networking-architecture.md`. Our server's job is matchmaking/brokering and BAP-side session
bookkeeping - the match itself runs peer-to-peer.

**Consequence for scope:** with a single console/emulator you can reach a private match *lobby*, but
"3v3" needs **six participants**. The two-instance harness in `status/harness.md` stops being a
nicety and becomes a requirement.

---

## Sizing, stated plainly

| stage | remaining |
|---|---|
| Stage 1 → orbit | 2 gates. `investment_signin` is a protocol layer; `profile_setup` needs **no server work**; `prepare_for_orbit` tolerates an unset destination. |
| Stage 2 → in-world | unexplored, but the client narrates itself, which has been worth a lot |
| Stage 3 → 3v3 private | largest. P2P host election, reservations, entity indices, 6 participants |

**This will not be finished in one session.** What makes it tractable rather than open-ended:

1. **Every gate is named before it is reached.** We have the complete `bootflow:` and `error.*`
   vocabularies, so we always know what the next wall is called.
2. **The client is a precise oracle.** It refuses with a specific complaint, and `RELEASE_VHALT`
   crash reports state the cause in prose.
3. **Five gates fell in one session** once the method settled (read the binary, answer minimally,
   read the complaint). The rate is not the problem; the remaining surface is.
4. **The hard cryptographic walls are behind us.** The two RSA envelopes (`sku_config`, CDN manifest)
   were unforgeable and were routed around. BAP's MAC is keyed by material **we** issue.

**Biggest unknown, restated (the old one was retired - see above).** Two things now carry the most
risk, and neither is persistence:

1. **Content decompression.** 76% of package blocks are **Oodle LZH/LZHLW**, with no public decoder.
   Most game *content* is therefore unreadable, which is what currently blocks recovering an
   Elimination activity definition. `OodleLZDecoder_DecodeSome` is at vaddr `0x12672e0` in our own
   decrypted eboot; lifting it into a harness is self-checking and is the highest-value experiment.
2. **Stage 3 needs six participants.** That is a hardware and harness problem, not a protocol one.

---

## Reach warning

Everything past `/SignOn` has been reached on **bytes we invented**, and everything past
`content_check` additionally requires `patches/CUSA00219-lab-manifest*.xml`, a **client
modification**. All of it is `synthetic_ancestor`. A working private match reached this way proves the
*client* can be satisfied; it does **not** establish what Bungie's services actually did. Only a
retail capture can do that, and `panther-*` is already NXDOMAIN - see `status/TODO.md` → *Blocked on
hardware*.
