// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐⭐ **What the client's own team assigner does with an activity ID - and why NO Activity Host
//! can reach it.**
//!
//! `activities:team_picker:dump` prints `Multiplayer Team=-1` for our single player, and that
//! number is the sixth wall of the in-world chain. This module answers one question about it:
//!
//! > **For a given activity ID, which of `FUN_007ABB50`'s two non-matchmaking assigners runs?**
//!
//! For **407** the measured answer is COPY-FROM-FIRETEAM, which has no source on this band. For
//! **398, 399, 408, 412, 413** it is SEQUENTIAL, which needs none. The table is the deliverable.
//!
//! ## ⛔⛔ CORRECTION 2026-08-10, IN PLACE - **THE ROW IS KEYED BY AN ID WE DO NOT PUBLISH**
//!
//! > This module shipped for one pass claiming the row was keyed by *"the activity index this
//! > crate publishes"* and that `D1_AH_ACTIVITY_INDEX=413` was *"a one-variable change to a field
//! > this crate already owns"*. **Both sentences were wrong**, and an A/B built on them would have
//! > cost a boot and returned a false negative.
//!
//! `FUN_007ABB50` fetches the id from a **client-local session parameter**, measured at
//! instruction level (`d1-eboot disasm 0x7abb50`, `d1-eboot controls` ALL PASS):
//!
//! ```text
//! 0x7ABC23  xor  edi, edi
//! 0x7ABC25  call 0x773FC0            ; network_session_parameters(0)
//! 0x7ABC2A  lea  rdi, [rax+0x5A8]    ; parameter slot 0x5A8 = `desired_activity`
//! 0x7ABC31  call 0x1203040           ; c_generic_network_session_parameter<T>::vfunc[18]  (RTTI)
//! 0x7ABC39  test rbx, rbx
//! 0x7ABC3C  je   0x7ABCE1            ; ⛔ UNSET => the WHOLE assignment block is skipped
//! 0x7ABC42  movzx edi, word [rbx+2]  ; <-- THE ACTIVITY ID, and this is the only source of it
//! 0x7ABC46  call 0x5441C0            ; activity_def[+0xA9] == 0 ?
//! 0x7ABC53  movzx esi, word [rbx+2]  ; the same id, into FUN_00544AC0
//! ```
//!
//! `0x773FC0` is `network_session_parameters(idx)` beyond doubt: it reads `[0x26B1850]`, session
//! stride `0x131C8`, inner stride `0x98E0`, state at `+0x8C58` with mask `0x7F0`, parameter set at
//! `+0x6B38` - every constant identical to `capture-rig/session-params.py`, which pins
//! `0x05A8 -> ("desired_activity", "s_desired_activity_information", …)` under its own controls.
//!
//! ⛔ **`D1_AH_ACTIVITY_INDEX` writes `global_activity_state+0x680`** - see
//! [`PUBLISHED_ACTIVITY_INDEX_OFFSET`] - **a different object, in a different message, and nothing
//! in the evidence links the two.**
//!
//! ### ⭐⭐ BUT THE LAUNCH POKE **DOES** REACH IT - `[C]`, offline, and this closes a `[U]`
//!
//! ⛔ **A revision of this doc said *"neither of this project's two activity levers writes the
//! field the assigner reads"*, and filed *"does anything populate `desired_activity`?"* as the open
//! question that decides the lane. REFUTED - the answer is one function away.** Nobody writes
//! `+0x5A8` directly; a **setter** does, and it takes its id from the **`activity`** parameter:
//!
//! ```text
//! FUN_00789D90(v):  local_d0[1] = *(u16 *)(v + 2);  …  FUN_00B8C210(&params->desired_activity, local_d0)
//! ```
//!
//! `d1-eboot xref 0x789d90` reports **9 confirmed callers** (4 reference kinds, 4 positive controls
//! PASS), and two of them pass `value_of(&params->activity)` **literally**:
//!
//! | caller | what it passes |
//! |---|---|
//! | `c_world_controller_state_setup_activity_host_setup_handler::vfunc[7]` `0x7A5250` | `c_generic_network_session_parameter<…>::vfunc[18](&psVar8->activity)` |
//! | `0x795AF0` | `memcpy(local_b8, value_of(&psVar2->activity), 0x8C)` then `FUN_00789D90(local_b8)` |
//! | `c_world_controller_state_setup_orbit_handler::vfunc[7]` `0x7AAF60` (⭐ the sole caller of `0x7ABB50`) | `FUN_00573D50()`, guarded on `network_session_parameter_activity_id(&psVar12->activity)` |
//!
//! plus `0x7A7C00`, `0x7A8D10` (`…setup_matchmaking_handler::vfunc[7]`), `0x7A9C20`, `0x7AB9B0`,
//! `0x953850`, `0x953CE0`.
//!
//! ⭐ And `params->activity` is `params+0x9B8` = `c_network_session_parameter_activity`, whose
//! `m_value+2` is what `network_session_parameter_activity_id` (`0xB8C130`) returns - **the exact
//! word `capture-rig/session-params.py --launch-poke --activity-id N` writes** (`+0x9B8 + 0xEA`).
//!
//! ⇒ **the OPERATOR'S launch poke is the lever for this lane; this Activity Host is not.** The A/B
//! is `session-params.py --launch-poke --activity-id 413`, and it must be run with
//! `D1_AH_ACTIVITY_INDEX` **left at 407** - `global_activity_state+0x680` has a different consumer
//! set (`0x78C6D0`/`0x78C770`/`0x78C8B0`, the **rung-33 unlock evaluator**), so changing it
//! perturbs a solved gate while measuring nothing here.
//! ⇒ **THIS PHASE HAS NO SERVER SURFACE ON THE TEAM LANE AT ALL.**
//!
//! ⚠ What is still `[U]` is **narrower**: every one of those callers is guarded on a
//! world-controller state, so the open question is whether a guard opens on *our* route - not
//! whether a writer exists. **The live discriminator is a READ, not a poke** - with a client in
//! orbit, `session-params.py --session 0` before and after `--launch-poke`, and look at
//! `desired_activity`'s `has_value`. If it stays 0, `FUN_007ABB50`'s assignment block is
//! unreachable (`0x7ABC3C`) and the gametype mode below is moot.
//!
//! ## ⛔⛔ WHAT THIS MODULE IS NOT - read before quoting it
//!
//! Naming the mode does **not** put a team on the wire, and this crate cannot name it anyway. The
//! team byte lives in the client's own network-session player record -
//! `session + `[`SESSION_PLAYER_ARRAY_OFFSET`]` + idx*`[`SESSION_PLAYER_STRIDE`]` +
//! `[`POSSE_PLAYER_TEAM_OFFSET`] - and its writers are all client-local:
//!
//! | writer | what it does | reachable from |
//! |---|---|---|
//! | `FUN_00B97E40(tbl)` | **sequential** - `mov [r14+rax+0x1D08], bl` with `bl` starting at 0 and incremented, i.e. `0,1,2…` into each player's `+0xB0` | `0x7ABB50` **only** (`d1-eboot xref`: 1 confirmed ref, `call` @ `0x7ABCDC`) |
//! | `FUN_00B97EA0(ft, posse)` | sets every posse player's `+0xB0` to `0xFF` (`0xB97F42`), then **copies** `+0x7A` of the FIRETEAM player with the same id at `+0x04` (`0xB97F8B` -> `0xB97F93`) | `0x7ABB50` **only** (`call` @ `0x7ABCC5`) |
//! | `FUN_00B97FC0(tbl)` | the skill picker - `mov [r15+rdx+0x1D08], bl` @ `0xB9807D`, i.e. **also `+0xB0`** | `0x7A98C0`, rung 29's matchmaking-DONE branch |
//! | `FUN_00B98AB0(tbl)` | clear-all to `0xFF` | `0x7A183E`, `0x7A903F` |
//! | `FUN_00B805F0` `find_or_add_player` | at player-add: `+0x7A = 0xFF` (`0xB80974`) **and** `+0xB0 = 0xFFFFFFFF` as a **dword** (`0xB80890`, `0xB8097D`) | player add |
//!
//! ⭐ **The dump reads it as a SIGNED BYTE** - `FUN_00AE94D0` @ `0xAE966F`
//! `movsx r9d, byte [r15+rbx+0x1D08]` - so `0xFF` prints `-1` and the sequential counter prints
//! `0, 1, 2`. That is why `Multiplayer Team=-1` is the *initialised* value, not an error.
//!
//! and `0x7ABB50` itself has exactly **one** caller, `0x7AAF60` =
//! `c_world_controller_state_setup_orbit_handler::vfunc[7]` (`d1-eboot xref 0x7abb50`: 1 confirmed
//! ref, `call 0x7AB38A`). So the assignment is gated **three** times over, and **not one of the
//! three is a wire value**:
//!
//! 1. the **id** - `desired_activity` must have a value at all (`0x7ABC3C`, above). `[U]`;
//! 2. the **countdown** - `FUN_0078A070` `is_activity_launch_countdown_active` requires
//!    `FUN_00750A70(3)`, the world-controller state to be `0x1B setup:orbit`, and
//!    `network_session_parameters(0)->countdown_timer` to hold a value with
//!    `FUN_00B8DF20(countdown_timer) != -1`. Our launch route forges the goal instead, so
//!    `handler+0x40` stays `-1` and the block is never entered. The client says so in its own
//!    words - `world_controller:online: 'is_activity_launch_countdown_active' failed, because
//!    we're not in state 'setup:orbit'` - in **2** of 257 run logs, one of them the rung-36
//!    success `20260809-230309-FAM02-OLIVE`;
//! 3. the **mode**, which is what this table carries.
//!
//! ⇒ **All three are client-local. A green control here means *"the gametype for this ID can
//! assign"* and NOTHING about anything we send.** The honest conclusion is stronger than the one
//! this module first shipped: 407 cannot get a team on this route for a reason that does not
//! depend on matchmaking at all, and 413 cannot be selected by any environment variable we own.
//!
//! ## ⛔ RETRACTION, IN PLACE - "the positive case has never been observed" is FALSE
//!
//! > This project's own notes record `Multiplayer Team = -1` as an unbroken observation and treat
//! > the positive case as never once seen. A census of **all 257** `runs/*/emulator.log` says
//! > otherwise: the dump fired in **26** runs, **163** lines read `-1` **and 2 read `0`**, both in
//! > `runs/20260810-102004-TEAM-LANE-1`.
//!
//! ⛔⛔ **AND A REVISION OF THIS RETRACTION SHIPPED A TABLE ITS OWN DATA REFUTES.** It said the two
//! values are *"discriminated by the dump's own `call_site=`"* - `Exit Matchmaking -> -1`,
//! `host_migration_timing_notify_migration_ended -> 0`. **`call_site` is not a discriminator.** The
//! value is on the `Peer Index` line and `call_site=` on the `BEGIN` line above it; attributing each
//! peer row to its own BEGIN gives:
//!
//! ```text
//!  84  Multiplayer Team=-1   call_site=Exit Matchmaking
//!  79  Multiplayer Team=-1   call_site=host_migration_timing_notify_migration_ended
//!   2  Multiplayer Team=0    call_site=host_migration_timing_notify_migration_ended   <- SAME SITE
//! ```
//!
//! ⇒ **79 `-1` and 2 `0` share one call_site**, and both values appear under it *inside*
//! `TEAM-LANE-1` itself. What separates them is position in time within the run, not the site.
//! ⚠ **The integers drift while a run is live** - the tree gained a row between two measurements
//! ~1 h apart (25 runs / 162 `-1` → 26 / 163). Cite the **shape** (one site, two values); re-measure
//! by attributing each `Peer Index` line to the preceding `BEGIN`, never by grepping one line.
//!
//! ⚠⚠ **`0` IS NOT EVIDENCE THAT AN ASSIGNER RAN**, and an earlier revision of this doc called it
//! *"the exact signature of `FUN_00B97E40`"*. It is not a signature; it is a coincidence with at
//! least one simpler competing explanation that was never considered. Both `0` lines are
//! immediately preceded by `Ending host migration due to FAILURE 'disconnected'` /
//! `disconnecting 'fireteam' session` / `Deleting session.` / `idle-host-disband disconnecting the
//! session.`, and they dump a **freshly created posse** (`posse:7196F8E8`, `posse:D10F819E`). A
//! record that never went through `FUN_00B805F0` is simply zero, and zero reads as `0`. ⇒ `[O]`,
//! **undiscriminated**: what the census proves is that the printed value is not a constant, which
//! is exactly enough to retract "-1, always" and no more. ⚠ Log silence cannot settle it either -
//! the countdown-failure diagnostic did **not** fire in that run, and it fires in only 2 of 257
//! runs overall, so its absence is not evidence the countdown succeeded.
//!
//! ⛔ **RETRACTED IN PLACE, same day, before it was ever cited - `FUN_00B80F30` is a READER.** A
//! concurrent revision offered it as *"a third alternative … stores 8 bytes at element `+0x1D08`
//! … on an add/copy path"*. It is the **opposite direction**, which is this project's named
//! output-vs-input failure mode. The one access is `0xB817E9 mov rdx, [r8+r15+0x1D08]` - a **LOAD**
//! - and the very next instruction, `0xB817F1 mov [rbx+r12+0x2160], rdx`, files it into a
//! *different* structure on a *different* stride. The loop walks the player array
//! (`add r15, 0xE8` / `cmp r15d, 0x1220`) as its **source**; the atlas names the function
//! `networking:session:membership:dump: [%s] simple…`. **It marshals a dump. It writes no player
//! record.** ⇒ it is not an alternative explanation for `Multiplayer Team=0` at all.
//!
//! ⚠ **What that revision got RIGHT, and it is worth keeping:** `TEAM-LANE-1`'s
//! `activity-host.log` reads `⭐ publishing activity INDEX 407`, and 407 is gametype team-mode
//! **3** - so a run that reached the assignment block would have selected `FUN_00B97EA0`, **not**
//! `FUN_00B97E40`, and an inference requiring the table below to be wrong cannot be offered as
//! evidence *for* it. ⛔ But note the tension is weaker than it looks, for the reason at the top of
//! this doc: **what we publish is not what the assigner reads.** `FUN_007ABB50` keys on
//! `desired_activity`, so `publishing activity INDEX 407` constrains our wire and not its lookup.
//! ⇒ **two live explanations for the `0`, neither preferred, `[U]`.**
//!
//! ## How each row was measured - `d1-replay`, offline, on a banked snapshot
//!
//! `FUN_007ABB50` selects the assigner from two bytes, and this module records both:
//!
//! ```text
//! FUN_00544AC0(rec, activity_id, 0xFFFF)              // rec[0] = activity def, rec[1] = gametype
//! FUN_00545240(out, rec)                              // out[0..0x1D] = u32 copy from gametype+0x14
//! if (FUN_00544DE0(rec) == 0 || out[0] != 3) { if (out[0] == 2) FUN_00B97E40(posse); }
//! else                                               { FUN_00B97EA0(fireteam, posse); }
//! //  FUN_00544DE0(rec) == *(u8*)(rec[0] + 0x15)      // a leaf, 7 bytes
//! ```
//!
//! so `gametype_team_mode` is `*(u32*)(gametype_record + 0x14)` and `def_gametype_override` is
//! `*(u8*)(activity_def + 0x15)`. Both were read by executing the client's own
//! `FUN_00544AC0` under `d1-replay` against `runs/snapshots/orbit-all.snap` (**20 PASS / 0 FAIL /
//! 0 SKIP**, `--stack-canary auto`), once per index, and dumping the 0x20-byte output struct.
//!
//! ⭐ **The positive control for the lookup itself**: `rec[0]` differs on every one of the 21 ids
//! (`0x3808CA790` … `0x3808CAB50`, stride `0x30`) and `rec+0x18` echoes the id, so the call is
//! keyed and not returning a constant. ⭐ **And the inverted one**: activity **7**
//! (`Patrol Cosmodrome`, `def[+0xA9] == 0xFF`) returns `rec[1] == 0` - **no gametype record at
//! all** - which is the same refusal `FUN_005441C0` makes one line earlier.
//!
//! ⭐⭐ **Three independently measured fields partition the 21 ids the same way**, which is the
//! internal control this table would fail if one row were transcribed wrong:
//! `def+0x10 == 6` ⟺ `def+0x15 == 0` ⟺ `gametype+0x14 == 2` ⟺ [`TeamAssignment::Sequential`].
//!
//! ⚠ `[U]` **`def+0x10` is not named.** It reads 12 for the copy-from-fireteam set, 6 for the
//! sequential set and 4 for index 411. A party or team size is the obvious guess and this module
//! deliberately does not make it - it is carried only as the third leg of the agreement control.

// ─────────────────────────────────────────────────────────────────────────────────────────────
// ⭐⭐ THE OFFSETS, PINNED - so the two mislabellings this module already shipped cannot recur.
//
// Every one of these is a literal displacement in an instruction I decoded with `d1-eboot disasm`,
// not a number read off a decompile. They exist as constants rather than prose because the defect
// they guard against is a *wrong label on a right measurement*, which prose cannot fail.
// ─────────────────────────────────────────────────────────────────────────────────────────────

/// `network_session_parameters(0) + this` is the parameter `FUN_007ABB50` reads the activity ID
/// out of: `0x7ABC2A lea rdi, [rax+0x5A8]`. `capture-rig/session-params.py` names slot `0x5A8`
/// **`desired_activity`** (`s_desired_activity_information`) under its own controls.
///
/// ⛔ **No Activity Host writes this.** It is not [`PUBLISHED_ACTIVITY_INDEX_OFFSET`].
pub const ASSIGNER_ID_SESSION_PARAM_OFFSET: u32 = 0x5A8;

/// Byte offset of the `u16` activity ID **inside** that parameter's value:
/// `0x7ABC42 movzx edi, word ptr [rbx+2]`, and again at `0x7ABC53` for `FUN_00544AC0`.
pub const ASSIGNER_ID_VALUE_OFFSET: u32 = 0x02;

/// What this crate *does* publish - `global_activity_state+0x680`, the knob
/// `D1_AH_ACTIVITY_INDEX`. Carried here **only** so the two can be asserted DIFFERENT; the
/// assigner never reads it. Mirrors [`crate::messages::GlobalActivityState::ACTIVITY_INDEX`].
pub const PUBLISHED_ACTIVITY_INDEX_OFFSET: u32 = 0x680;

/// Start of the per-player array inside a network session: `0xB97F3A lea r12, [r14+rax+0x1C58]`.
pub const SESSION_PLAYER_ARRAY_OFFSET: u32 = 0x1C58;

/// Stride of one player record: `0xB97F33 imul rax, 0xE8`.
pub const SESSION_PLAYER_STRIDE: u32 = 0xE8;

/// ⭐ The team field the dump prints - `player + 0xB0`, i.e. the absolute displacement `0x1D08`.
/// **Destination** of [`TeamAssignment::CopyFromFireteam`]'s copy (`0xB97F93`), target of the
/// sequential counter (`0xB97E40` -> `0xB9807D`-shaped store), and what
/// `activities:team_picker:dump` reads as a **signed byte** (`FUN_00AE94D0` @ `0xAE966F`
/// `movsx r9d, byte ptr [r15+rbx+0x1D08]`).
///
/// ⚠ `FUN_00B805F0` initialises it as a **dword** `0xFFFFFFFF`, while every assigner writes only
/// the low byte. The dump's `movsx` of that low byte is why the printed value is `-1`.
pub const POSSE_PLAYER_TEAM_OFFSET: u32 = 0xB0;

/// ⛔⛔ **The SOURCE of the copy, and it is NOT [`POSSE_PLAYER_TEAM_OFFSET`]** - `player + 0x7A`,
/// absolute displacement `0x1CD2`: `0xB97F8B mov al, [r13+rcx+0x1CD2]` -> `0xB97F93
/// mov [r12+0xB0], al`. An earlier revision of this module conflated the two and then named
/// `FUN_00B97FC0` as *"the only writer of that source"* - `FUN_00B97FC0` writes `+0xB0`
/// (`0xB9807D mov [r15+rdx+0x1D08], bl`), the **destination**. That claim was a non-sequitur
/// about a different field, and so was the matchmaking run-log census attached to it.
///
/// ⚠ `[U]` **The writer set for this field is UNKNOWN, not empty.** It is an indexed heap element
/// (`base + idx*0xE8 + 0x7A`), which this project's own rule marks invisible to **both**
/// `d1-eboot xref` (no address in the instruction) and `d1-eboot field-xref` (not a fixed
/// object+offset) - so no scan with a positive control has been run against it. A folded-constant
/// grep of the decompiled corpus finds exactly one write, `FUN_00B805F0` setting it to `0xFF` at
/// player-add, and that grep is **not** a controlled scan: the same technique on the neighbouring
/// `0x1D08` returns dozens of unrelated files.
pub const FIRETEAM_PLAYER_TEAM_SOURCE_OFFSET: u32 = 0x7A;

/// ⛔⛔ **THE SCOPE OF EVERY CONTROL IN THIS MODULE, IN THE CRATE'S OWN OUTPUT.**
///
/// [`LAUNCHABLE`] is a **hand-transcribed** record of an offline measurement. Nothing in this crate
/// can reach `runs/snapshots/orbit-all.snap`, so the controls **re-check the transcription against
/// itself and against pinned instruction displacements - they do not re-execute the client.**
///
/// ⚠ **A uniformly wrong transcription passes all of them.** The three-field agreement bounds
/// single-row slips only; a *consistent whole-row* flip is invisible to it and is caught instead by
/// the id-specific 407/413/negative/receipt assertions. ⇒ **"all controls pass" must never be read
/// as "the measurement was re-run".** This constant is what a reader gets handed instead.
pub const REMEASURE_RECIPE: &str = "\
    NOT RE-MEASURED BY THESE CONTROLS - LAUNCHABLE is a hand-transcribed table. To re-measure a row: \
    `d1-replay replay runs/snapshots/orbit-all.snap --func 0x544ac0 --arg scratch --arg <id> \
    --arg 0xffff --stack-canary auto --dump a:0x700000000000:0x20`, then read rec[0] (activity def) \
    and rec[1] (gametype record); gametype+0x14 = team mode, def+0x15 = override, def+0x10 = the \
    unnamed third leg. ⭐ Positive control: rec[0] differs on all 21 ids and rec+0x18 echoes the id. \
    ⭐ Inverted control: id 7 (Patrol Cosmodrome) returns rec[1] == 0, no gametype record at all. \
    ⛔ --stack-canary auto is MANDATORY; without it the replay faults at a host-shaped 0x55… address \
    and reads like `this function cannot be replayed`. \
    ⛔⛔ THE BAND PREDICATE IS INHERITED, NOT DERIVED HERE, and the inherited phrasing names the \
    WRONG OBJECT AND THE WRONG SENSE. This project says `activity_def[+0xA9] == 0`; FUN_005441C0 \
    actually admits on `*(u8*)(slot_0x320_record + 0xA9) != -1` - 0x5441E4 `call [rcx+0x320]` with \
    the id, then 0x5441EF `cmp byte [rax+0xA9], 0xFF` / `setne al`. That record comes from vtable \
    slot 0x320, while the defs these rows measure come from FUN_00544AC0's slot-0x328 lookup \
    (0x544B2F). `== 0` is a strict SUBSET of `!= -1`, so the 393-413 band may be NARROWER than the \
    client's real admission set - re-derive it before treating 21 as exhaustive.";

/// ⭐⭐ **The only A/B that moves this lane, and it is the OPERATOR'S poke, not an env var.**
///
/// Named as a constant so the receipt, the controls and the module doc cannot drift apart - the
/// defect this guards against is *shipped operator-facing text recommending a lever that cannot
/// move the field*, which no assertion over the measurement can see.
pub const TEAM_LANE_AB_RECIPE: &str = "\
    python3 -B capture-rig/session-params.py --launch-poke --activity-id 413 \
    (WITH D1_AH_ACTIVITY_INDEX LEFT AT 407) - FUN_007ABB50 reads \
    network_session_parameters(0)+0x5A8 `desired_activity` +2, which FUN_00789D90 fills from \
    *(u16*)(value_of(&params->activity)+2), and params->activity is params+0x9B8 whose m_value+2 is \
    exactly what --launch-poke --activity-id writes (+0x9B8+0xEA). ⛔ D1_AH_ACTIVITY_INDEX writes \
    global_activity_state+0x680, whose consumers are the rung-33 unlock evaluator \
    (0x78C6D0/0x78C770/0x78C8B0) - changing it perturbs a SOLVED gate and measures nothing here.";

/// What `FUN_007ABB50` will do for one activity, once the launch countdown is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamAssignment {
    /// `gametype+0x14 == 2` and `def+0x15 == 0` ⇒ `FUN_00B97E40(posse)` writes `0,1,2…` into each
    /// posse player's `+0xB0`. ⭐ **Needs no source**: a solo posse gets team **0**.
    Sequential,
    /// `gametype+0x14 == 3` and `def+0x15 != 0` ⇒ `FUN_00B97EA0(fireteam, posse)` sets every posse
    /// player's [`POSSE_PLAYER_TEAM_OFFSET`] to `0xFF` and then, for the FIRETEAM player whose id
    /// at `+0x04` matches, copies that player's [`FIRETEAM_PLAYER_TEAM_SOURCE_OFFSET`] into it.
    ///
    /// ⛔⛔ **THE SOURCE AND THE DESTINATION ARE DIFFERENT FIELDS** - `+0x7A` in, `+0xB0` out - and
    /// an earlier revision of this module said the source's *"only writer is `FUN_00B97FC0`,
    /// reached from rung 29's matchmaking-DONE branch"*. **That is wrong**: `FUN_00B97FC0` writes
    /// `+0xB0` (`0xB9807D mov [r15+rdx+0x1D08], bl`), so it is a writer of the *destination*, and
    /// the `Matchmaking for activity '%d' done`-never-fired census attached to it was evidence
    /// about matchmaking, not about this field.
    ///
    /// ⚠ `[U]` **What we can actually say:** the only visible write to the source initialises it to
    /// `0xFF` at player-add (`FUN_00B805F0` @ `0xB80974`), so a fireteam player that nothing else
    /// touched carries `0xFF` and this mode copies `0xFF` - which the dump prints as `-1`. Whether
    /// anything else writes `+0x7A` is **unknown**, because an indexed heap element is invisible to
    /// every static scanner this project owns; see [`FIRETEAM_PLAYER_TEAM_SOURCE_OFFSET`].
    CopyFromFireteam,
    /// Measured, but neither shape: `FUN_007ABB50` falls through both branches and **no assigner
    /// runs at all**. No launchable index is in this state today; the variant exists so a future
    /// row cannot be silently rounded to one of the two above.
    NoAssigner,
    /// Not in [`LAUNCHABLE`]. ⛔ Deliberately **not** a guess - an invented mode reads exactly like
    /// a measured one at the call site that consumes it.
    Unmeasured,
}

impl TeamAssignment {
    /// Whether the client will end up with a **non-negative** team byte, given a working countdown.
    ///
    /// ⚠ This is a statement about the *gametype*, never about the run. Gate 1 (the countdown) is
    /// not represented here at all, and it is the one that has actually been failing.
    pub fn assigns_a_team(self) -> bool {
        matches!(self, TeamAssignment::Sequential)
    }
}

/// One launchable activity's team-assignment facts, as `FUN_007ABB50` reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchableActivity {
    /// ⭐ The `u16` **activity ID** - the key `FUN_005441C0` and `FUN_00544AC0` are called with.
    ///
    /// ⛔ **NOT the index this crate publishes.** `FUN_007ABB50` reads this id out of
    /// `network_session_parameters(0) + `[`ASSIGNER_ID_SESSION_PARAM_OFFSET`]` +
    /// `[`ASSIGNER_ID_VALUE_OFFSET`] (`desired_activity`), a client-local session parameter no
    /// Activity Host writes. `D1_AH_ACTIVITY_INDEX` writes
    /// [`PUBLISHED_ACTIVITY_INDEX_OFFSET`] of a different object. The two happen to share a number
    /// space; nothing in the evidence links them.
    pub index: u16,
    /// `*(u32*)(gametype_record + 0x14)` - what `FUN_00545240` copies into `local_b0[0]` and what
    /// `FUN_007ABB50` compares against `3` and then `2`.
    pub gametype_team_mode: u32,
    /// `*(u8*)(activity_def + 0x15)` - the whole of `FUN_00544DE0`, a 7-byte leaf. Non-zero is
    /// what *enables* the mode-3 branch; zero short-circuits it to the mode-2 test.
    pub def_gametype_override: u8,
    /// `*(u32*)(activity_def + 0x10)`. ⚠ `[U]` - unnamed on purpose. Carried as the third,
    /// independent leg of the agreement control; see the module doc.
    pub def_field_0x10: u32,
    /// The gametype record's absolute address in `runs/snapshots/orbit-all.snap`. Provenance only,
    /// never sent: it is what makes a stale row falsifiable against a fresh capture, and it is how
    /// the 21 ids were seen to share just **three** records.
    pub gametype_record: u64,
}

impl LaunchableActivity {
    /// The assigner `FUN_007ABB50` will pick for this row, derived from the two measured bytes in
    /// the client's own branch order rather than stored - so a row cannot disagree with itself.
    pub fn team_assignment(&self) -> TeamAssignment {
        // if (FUN_00544DE0(rec) == 0 || out[0] != 3) { if (out[0] == 2) sequential; }
        // else                                      { copy-from-fireteam; }
        if self.def_gametype_override == 0 || self.gametype_team_mode != 3 {
            if self.gametype_team_mode == 2 {
                TeamAssignment::Sequential
            } else {
                TeamAssignment::NoAssigner
            }
        } else {
            TeamAssignment::CopyFromFireteam
        }
    }
}

/// ⭐⭐ Every index the client will launch at all - `activity_def[+0xA9] == 0`, **393-413**, all
/// Crucible private-match modes - with the two bytes that decide its team assignment.
///
/// Measured 2026-08-10 by replaying `FUN_00544AC0` on `runs/snapshots/orbit-all.snap`. See the
/// module doc for the command, the positive control (per-id definition pointers) and the inverted
/// one (index 7 ⇒ no gametype record).
pub const LAUNCHABLE: &[LaunchableActivity] = &[
    row(393, 3, 1, 12, 0x3_80AA_FE80),
    row(394, 3, 1, 12, 0x3_80AA_FE80),
    row(395, 3, 1, 12, 0x3_80AA_FE80),
    row(396, 3, 1, 12, 0x3_80AA_FE80),
    row(397, 3, 1, 12, 0x3_80AA_FE80),
    row(398, 2, 0, 6, 0x3_80AA_FF40),
    row(399, 2, 0, 6, 0x3_80AA_FF40),
    row(400, 3, 1, 12, 0x3_80AA_FE80),
    row(401, 3, 1, 12, 0x3_80AA_FE80),
    row(402, 3, 1, 12, 0x3_80AA_FE80),
    row(403, 3, 1, 12, 0x3_80AA_FE80),
    row(404, 3, 1, 12, 0x3_80AA_FE80),
    row(405, 3, 1, 12, 0x3_80AA_FE80),
    row(406, 3, 1, 12, 0x3_80AA_FE80),
    // ⛔ THE DEFAULT. 407 = ELIMINATION, the only index this project has walked to
    // `activity:in_world`, and the one whose gametype cannot assign a team on this route.
    row(407, 3, 1, 12, 0x3_80AA_FE80),
    row(408, 2, 0, 6, 0x3_80AA_FF40),
    row(409, 3, 1, 12, 0x3_80AA_FE80),
    row(410, 3, 1, 12, 0x3_80AA_FE80),
    // 411 is the only index with its own gametype record, and the only `def+0x10 == 4`.
    row(411, 3, 1, 4, 0x3_80AB_0000),
    row(412, 2, 0, 6, 0x3_80AA_FF40),
    row(413, 2, 0, 6, 0x3_80AA_FF40),
];

/// `const fn` so [`LAUNCHABLE`] reads as a table rather than as twenty-one struct literals.
const fn row(
    index: u16,
    gametype_team_mode: u32,
    def_gametype_override: u8,
    def_field_0x10: u32,
    gametype_record: u64,
) -> LaunchableActivity {
    LaunchableActivity {
        index,
        gametype_team_mode,
        def_gametype_override,
        def_field_0x10,
        gametype_record,
    }
}

/// The measured row for `index`, or `None` if it is outside the launchable band.
pub fn lookup(index: u16) -> Option<&'static LaunchableActivity> {
    LAUNCHABLE.iter().find(|a| a.index == index)
}

/// What `FUN_007ABB50` will do for `index`. [`TeamAssignment::Unmeasured`] for anything not in
/// [`LAUNCHABLE`] - which is every non-launchable index, and those never reach rung 30 anyway.
pub fn team_assignment(index: u16) -> TeamAssignment {
    match lookup(index) {
        Some(a) => a.team_assignment(),
        None => TeamAssignment::Unmeasured,
    }
}

/// ⭐ The five activity IDs whose gametype assigns a team without needing a source: **398, 399,
/// 408, 412, 413**.
///
/// ⛔ **This is NOT an A/B THIS CRATE can run.** `FUN_007ABB50` keys on `desired_activity`
/// ([`ASSIGNER_ID_SESSION_PARAM_OFFSET`]), and `D1_AH_ACTIVITY_INDEX` writes
/// [`PUBLISHED_ACTIVITY_INDEX_OFFSET`] of a different object - a knob that cannot move it.
///
/// ⭐ **It IS an A/B the OPERATOR can run.** `session-params.py --launch-poke --activity-id N`
/// writes `params+0x9B8` (`activity`), and `FUN_00789D90` reads `value+2` of its argument
/// (`0x789EB9 mov ax,[rbx+2]`) and stores it into `params+0x5A8` (`0x789F12 add r14,0x5A8` ->
/// `0x789F23 call 0xB8C210`). `d1-eboot xref 0x789d90` = **9 confirmed callers, 4/4 positive
/// controls PASS**, and one of them is `0x7AAF60` - the *sole* caller of `0x7ABB50`, which calls
/// the setter at `0x7AB345` and the assigner at `0x7AB38A`, in that order, in the same handler.
///
/// ⚠ `[U]` **what remains open is the GUARD, not the writer**: every caller sits behind a
/// world-controller state test, so whether one opens on our route is unmeasured. The discriminator
/// is a READ - `session-params.py --session 0`, `desired_activity`'s `has_value`, before and after
/// the launch poke.
pub fn sequential_team_indices() -> Vec<u16> {
    LAUNCHABLE
        .iter()
        .filter(|a| a.team_assignment() == TeamAssignment::Sequential)
        .map(|a| a.index)
        .collect()
}

/// What the Activity Host says at startup about the index it is about to publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamReceipt {
    pub index: u16,
    pub assignment: TeamAssignment,
    /// True when the operator should read this as a warning rather than a receipt.
    pub is_warning: bool,
    /// The whole sentence, already naming the diagnostic the operator will see instead.
    pub message: String,
}

/// ⭐⭐ Explain, in one startup line, what the published index means for `Multiplayer Team`.
///
/// This exists because the failure it describes is **silent by construction**: nothing refuses,
/// nothing errors, and the only evidence is a `-1` inside a dump that is itself gated on
/// `FUN_000E4DA0(6,2)` *and* on `player_idx != -1`, so a run where the player slot is unset prints
/// nothing at all and reads exactly like "the dump did not run".
pub fn receipt(index: u16) -> TeamReceipt {
    let assignment = team_assignment(index);
    let (is_warning, message) = match assignment {
        TeamAssignment::Sequential => (
            false,
            format!(
                "⭐ team assignment: activity ID {index} is gametype team-mode 2 (SEQUENTIAL) - \
                 FUN_007ABB50 calls FUN_00B97E40, which writes 0,1,2… straight into each posse \
                 player's +0x{POSSE_PLAYER_TEAM_OFFSET:02X} and needs no source, so a solo posse \
                 gets team 0 (the dump movsx-es that byte, so 0xFF prints -1 and 0 prints 0). \
                 ⛔ THIS SAYS NOTHING ABOUT WHAT WE PUBLISH. FUN_007ABB50 takes its activity ID \
                 from network_session_parameters(0)+0x{ASSIGNER_ID_SESSION_PARAM_OFFSET:03X} \
                 (`desired_activity`) +{ASSIGNER_ID_VALUE_OFFSET}, which NO Activity Host writes; \
                 D1_AH_ACTIVITY_INDEX writes \
                 global_activity_state+0x{PUBLISHED_ACTIVITY_INDEX_OFFSET:03X}, a different \
                 object. ⚠ And it runs ONLY inside the activity-launch countdown: FUN_0078A070 \
                 needs world-controller state 0x1B setup:orbit AND \
                 network_session_parameters(0)->countdown_timer set. A goal-forging launch poke \
                 skips it, and the client then says \
                 `'is_activity_launch_countdown_active' failed, because we're not in state \
                 'setup:orbit'`. Both gates are session parameters and are the operator's, not \
                 ours."
            ),
        ),
        TeamAssignment::CopyFromFireteam => (
            true,
            format!(
                "⛔⛔ team assignment: activity ID {index} is gametype team-mode 3 \
                 (COPY-FROM-FIRETEAM) - FUN_007ABB50 calls FUN_00B97EA0, which sets every posse \
                 player's +0x{POSSE_PLAYER_TEAM_OFFSET:02X} to 0xFF and then copies \
                 +0x{FIRETEAM_PLAYER_TEAM_SOURCE_OFFSET:02X} of the FIRETEAM player with the same \
                 id at +0x04. ⚠ SOURCE AND DESTINATION ARE DIFFERENT FIELDS: the only visible \
                 write to the source is FUN_00B805F0 (find_or_add_player) setting it to 0xFF, so \
                 an untouched fireteam player copies 0xFF and the dump prints \
                 `Multiplayer Team=-1`. [U] the full writer set for +0x7A is UNKNOWN, not empty - \
                 an indexed heap element is invisible to both xref and field-xref, so no \
                 controlled scan has been run. ⛔ EITHER WAY THIS ACTIVITY HOST CANNOT MOVE IT: \
                 the assigner keys on network_session_parameters(0)+0x{:03X} \
                 (`desired_activity`), and D1_AH_ACTIVITY_INDEX writes \
                 global_activity_state+0x{:03X} - a different object. ⭐ THE LEVER IS THE \
                 OPERATOR'S LAUNCH POKE, NOT THIS SERVER: `session-params.py --launch-poke \
                 --activity-id N` writes params+0x9B8 (`activity`), and FUN_00789D90 copies \
                 value+2 from there into +0x5A8 (0x789EB9 -> 0x789F12/0x789F23); its 9 confirmed \
                 callers include 0x7AAF60, the SOLE caller of the assigner, which calls the setter \
                 at 0x7AB345 and the assigner at 0x7AB38A. The IDs whose gametype assigns without \
                 a source are {:?}, so the A/B is `--launch-poke --activity-id 413` with \
                 D1_AH_ACTIVITY_INDEX LEFT AT 407 (that field feeds the rung-33 unlock evaluator; \
                 changing it perturbs a solved gate and measures nothing here). ⚠ [U] every such \
                 caller is guarded on a world-controller state, so first do a READ, not a poke: \
                 `session-params.py --session 0` in orbit, before and after the launch poke - if \
                 `desired_activity` has_value stays 0, this whole block is unreachable (0x7ABC3C).",
                ASSIGNER_ID_SESSION_PARAM_OFFSET,
                PUBLISHED_ACTIVITY_INDEX_OFFSET,
                sequential_team_indices()
            ),
        ),
        TeamAssignment::NoAssigner => (
            true,
            format!(
                "⛔ team assignment: activity {index} is measured but matches NEITHER branch of \
                 FUN_007ABB50 (gametype team-mode {}), so no assigner runs at all and \
                 `Multiplayer Team` stays -1.",
                lookup(index).map(|a| a.gametype_team_mode).unwrap_or_default()
            ),
        ),
        TeamAssignment::Unmeasured => (
            true,
            format!(
                "⚠ team assignment: activity {index} is NOT in the measured launchable band \
                 393-413, so its gametype team mode is UNKNOWN and this crate will not guess one. \
                 An index outside that band also fails FUN_005441C0 (`activity_def[+0xA9] == \
                 0xFF`) and stalls at rung 29 matchmaking, three rungs before anything solved \
                 here. Measure a new row by replaying FUN_00544AC0 on a banked snapshot."
            ),
        ),
    };
    TeamReceipt { index, assignment, is_warning, message }
}
