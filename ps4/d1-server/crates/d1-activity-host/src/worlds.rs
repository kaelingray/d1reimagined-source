// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐⭐ **The known-world scenario table - the values rung 33 compares OUR blob against.**
//!
//! ## Why this table exists at all
//!
//! Rung 33 (`setup:activity_world_transition`) is a **sequence of blob ↔ scenario-record field
//! checks**, and every one of them is a property of the **WORLD**, not of the activity:
//!
//! | gate | rule | fn |
//! |---|---|---|
//! | checksum    | `blob+0x5C` **==** `scenario_record+0x0C` (PRIVATE; `+0x10` is PUBLIC) | `FUN_00299100` |
//! | bubble count| `blob+0x10` **==** `scenario_record+0x48`                             | `FUN_002A0320` |
//! | bubble state| `blob+0x14[i]` = `0x00` chosen / `0xFF` unchosen, per bubble          | `FUN_002A0320` |
//!
//! ⛔⛔ **The failure this table is here to make impossible.** Those three values and the activity
//! NAME are four knobs that must all describe *the same world*, and until now they were four
//! independent environment variables. Publishing one world's checksum while naming another
//! re-fails gate 1 - *the client compares against the record of the world it **actually loaded***,
//! not the one we claim - and the symptom is `world_controller:transition_error: activity checksum
//! mismatch`, which reads exactly like "the checksum value is wrong" rather than "the checksum
//! belongs to a different world". Deriving all three from the **name** collapses four knobs into
//! one and makes the disagreement a loud startup warning instead of a run-time gate.
//!
//! ⚠ **A launch is still TWO values, and this table only carries one of them.** The MODE - the
//! `u16` index into the client's 540-entry activity table at `global_activity_state+0x680` - is
//! **not** a property of the world and is **nowhere derived from it** (`0x80800CC5` field `[7]
//! @+0x620` u16 and field `[10] @+0x628` `char[0x28]` are two adjacent, independent fields; two
//! derivations agree). So [`crate::state::ActivityInstance::DEFAULT_ACTIVITY_INDEX`] is a separate
//! constant on purpose, and this table must never grow one.
//!
//! ## Provenance - `[C]` `observed-live`, and how each row was taken
//!
//! A world's scenario record is resident **only while that world is loading**, in the single
//! scenario slot `0x5A9`. Both rows were captured with the two-poke procedure in `CLAUDE.md`:
//! publish the target world with any checksum (gate 1 fails on purpose) while
//! `capture-rig/scenario-checksum.py watch --hz 500` samples the slot, then re-fire with the
//! captured values.
//!
//! ⚠⚠ `--hz 50` **misses the ~100 ms window** and reads like "the world never loaded" - a false
//! negative that costs an hour. `[C]`, measured both ways.
//!
//! ⛔ **These are not derivable offline.** `dumps/` carries the packages, but the scenario record is
//! built at load time; nothing in this tree computes `+0x0C` from a world name. A new row costs one
//! live capture. That is why [`lookup`] returns `None` rather than guessing - an invented checksum
//! is indistinguishable on the wire from a stale one.

/// One world's scenario-record facts, as rung 33 reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownWorld {
    /// The NUL-terminated ASCII name published at `global_activity_state+0x688`.
    pub name: &'static str,
    /// The scenario record's content tag, for the run log. Not sent; it is how the live capture
    /// identified which record it had, and it makes a stale row falsifiable against a new capture.
    pub scenario_tag: u32,
    /// `scenario_record+0x0C` - the **PRIVATE** checksum. `FUN_00299100` indexes
    /// `record + 0xC + ac[3]*4` where `ac[3]` is the index into `{"PRIVATE","PUBLIC"}` that the
    /// activity client prints in its own `[AC PRIVATE CURRENT …]` line ⇒ **0**. `+0x10` is PUBLIC
    /// and is the wrong one.
    pub checksum: u32,
    /// `scenario_record+0x48` - the world's bubble count. ⭐ A PvP map is ONE bubble; orbit is 16.
    pub bubble_count: u32,
    /// Which bubbles may legally be marked **CHOSEN** (`0x00`).
    ///
    /// ⚠ `0x00` is not universally legal: `FUN_002A0320` rejects a chosen bubble whose scenario
    /// sub-bubble flag is clear with *"activity client bubble state index was valid, but state
    /// itself was invalid"*. Measured live on `orbit` (16/16): bubbles 5-8 and 15 have sub-bubble
    /// count 0, which is why a blanket `0x00` failed there.
    ///
    /// ⛔ And leaving a bubble `0xFF` is **not** the safe default it looks like: the unchosen
    /// branch calls `FUN_00343020(ctx, bubble*8, -1, 0)`, which **UNREGISTERS** the slice-set
    /// region, so an all-`0xFF` blob passes rung 33 and guarantees rung 34 fails with
    /// `error.sstm_slice_not_found` ("LOBSTER").
    pub chosen: &'static [usize],
}

/// ⭐ `pvp_moonbase` - **First Light**, the map the rung-36 walk was taken on.
///
/// One bubble, bubble 0 chosen. Every value below is from the live capture of run
/// `20260809`'s scenario slot during the moonbase load.
pub const PVP_MOONBASE: KnownWorld = KnownWorld {
    name: "pvp_moonbase",
    scenario_tag: 0x80F0_8003,
    checksum: 0x5966_E491,
    bubble_count: 1,
    chosen: &[0],
};

/// `orbit` - the world the client is already in when an activity is launched.
///
/// ⭐ Kept because it is the **only** row with an independent second derivation: `orbit`'s record is
/// resident at rest, so `+0x0C = 0x33F044D3` was read directly rather than through the 100 ms
/// window, and `D1_AH_ACTIVITY=orbit D1_AH_ACTIVITY_INDEX=407 D1_AH_ACTIVITY_CHECKSUM=0x33f044d3`
/// is the combination that made `Activity checksum mismatch!` disappear on the live client. It is
/// this table's positive control against the moonbase row being a transcription.
pub const ORBIT: KnownWorld = KnownWorld {
    name: "orbit",
    scenario_tag: 0x80B5_2003,
    checksum: 0x33F0_44D3,
    bubble_count: 16,
    // Bubbles 5-8 and 15 have sub-bubble count 0 and CANNOT be chosen. Measured live, 16/16.
    chosen: &[0, 1, 2, 3, 4, 9, 10, 11, 12, 13, 14],
};

/// Every world whose scenario record has been captured. Two, and the honest number is two.
pub const KNOWN_WORLDS: &[KnownWorld] = &[PVP_MOONBASE, ORBIT];

/// The world an Activity Host publishes when nothing overrides it.
///
/// ⭐ `pvp_moonbase` / activity **407 = ELIMINATION** is the *only* configuration this project has
/// ever walked to `activity:in_world` on, and it is what `runs/snapshots/rung36-*.snap` were taken
/// under. The previous default - `act_cosmo_m0_highway`, checksum 0, bubble count 0, bubbles
/// unset - published **every documented rung-33/34 wall at once**, and had already silently shipped
/// in runs that meant to use the good set but inherited it from an unexported shell variable.
pub const DEFAULT_WORLD: KnownWorld = PVP_MOONBASE;

/// The captured facts for `name`, or `None` if this world has never been measured.
///
/// ⛔ Deliberately **not** a guess. An invented checksum is byte-indistinguishable on the wire from
/// a stale one, and both surface as the same `activity checksum mismatch`; returning `None` lets
/// the caller keep the old loud "checksum 0" warning, which at least names itself.
pub fn lookup(name: &str) -> Option<&'static KnownWorld> {
    KNOWN_WORLDS.iter().find(|w| w.name == name)
}

/// What the Activity Host will actually publish, after the world table and any overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The world name published at `global_activity_state+0x688`.
    pub name: String,
    /// The table row `name` matched, if any. `None` = an unmeasured world; the three values below
    /// then fall back to the pre-2026-08-10 zeros, which are loud rather than wrong-and-quiet.
    pub world: Option<&'static KnownWorld>,
    pub checksum: u32,
    pub bubble_count: u32,
    pub chosen: Vec<usize>,
    /// ⭐⭐ **Every override that CONTRADICTS the named world's measured value**, in words.
    ///
    /// This is the whole point of the type. Each entry is a configuration that will produce a
    /// rung-33 gate failure whose message names the *field* and never the *cause* - so it is
    /// reported once, loudly, at startup, where it is cheap.
    pub disagreements: Vec<String>,
}

/// ⭐⭐ Derive the three rung-33 scenario values from the **world name**, letting explicit
/// overrides win and reporting every one that contradicts a measured world.
///
/// `None` for an override means *the caller did not ask* (derive it). `Some(v)` means the caller
/// asked for exactly `v`, including `Some(0)` and `Some(vec![])` - both of which are deliberate,
/// documented negative controls (`D1_AH_ACTIVITY_CHECKSUM=0` reproduces every pre-2026-08-09 run;
/// `D1_AH_BUBBLE_CHOSEN=` publishes all-`0xFF` and reproduces LOBSTER). Collapsing "unset" and
/// "set to the falsy value" would silently delete both.
pub fn resolve(
    name: Option<&str>,
    checksum: Option<u32>,
    bubble_count: Option<u32>,
    chosen: Option<Vec<usize>>,
) -> Resolved {
    let name = name.unwrap_or(DEFAULT_WORLD.name).to_string();
    let world = lookup(&name);
    let mut disagreements = Vec::new();
    if let Some(w) = world {
        if let Some(c) = checksum {
            if c != w.checksum {
                disagreements.push(format!(
                    "D1_AH_ACTIVITY_CHECKSUM={c:#010x} but {}'s measured scenario_record+0x0C is \
                     {:#010x} (tag {:#010x}) - FUN_00299100 compares against the record of the \
                     world the client ACTUALLY LOADED, so this publishes one world's checksum \
                     while naming another and re-fails gate 1 as `activity checksum mismatch`",
                    w.name, w.checksum, w.scenario_tag
                ));
            }
        }
        if let Some(n) = bubble_count {
            if n != w.bubble_count {
                disagreements.push(format!(
                    "D1_AH_BUBBLE_COUNT={n} but {}'s measured scenario_record+0x48 is {} - \
                     FUN_002A0320 requires equality and reports the mismatch as \
                     `activity client bubble count mismatch`",
                    w.name, w.bubble_count
                ));
            }
        }
        if let Some(sel) = chosen.as_ref() {
            let illegal: Vec<usize> =
                sel.iter().copied().filter(|i| !w.chosen.contains(i)).collect();
            if !illegal.is_empty() {
                disagreements.push(format!(
                    "D1_AH_BUBBLE_CHOSEN names bubble(s) {illegal:?}, which {} cannot legally \
                     have CHOSEN (its choosable set is {:?}) - FUN_002A0320 rejects a chosen \
                     bubble whose scenario sub-bubble flag is clear with `activity client bubble \
                     state index was valid, but state itself was invalid`",
                    w.name, w.chosen
                ));
            }
            if sel.is_empty() && w.bubble_count > 0 {
                disagreements.push(format!(
                    "D1_AH_BUBBLE_CHOSEN is EXPLICITLY EMPTY - all {} of {}'s bubbles publish as \
                     0xFF `unchosen`. That PASSES rung 33 and then UNREGISTERS every slice-set \
                     region, so rung 34 fails with error.sstm_slice_not_found (\"LOBSTER\"). This \
                     is a negative control, not a configuration",
                    w.bubble_count, w.name
                ));
            }
        }
    }
    Resolved {
        checksum: checksum.or_else(|| world.map(|w| w.checksum)).unwrap_or(0),
        bubble_count: bubble_count.or_else(|| world.map(|w| w.bubble_count)).unwrap_or(0),
        chosen: chosen.or_else(|| world.map(|w| w.chosen.to_vec())).unwrap_or_default(),
        name,
        world,
        disagreements,
    }
}
