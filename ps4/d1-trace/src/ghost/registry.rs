// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The poke registry: a const table of every write ghost knows how to make, each a load-bearing
//! fact with a citation, exactly what a `#[test]` control pins (`design/d1-ghost.md` §4).
//!
//! ⭐ The registry is expected to SHRINK. A poke earned back onto the wire (served, not forced) is
//! DELETED here. So the table doubles as the honest, machine-readable list of what is still
//! synthetic: `registry().len()` is the count of writes the project still cannot serve.

/// What a poke is *for*. Rendered on the button; the house rule "poke to identify a gate, never to
/// satisfy one" (`CLAUDE.md`) means a `Satisfy` entry carries a standing warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Class {
    /// Reads a gate's answer without changing what the client would do (a probe).
    Probe,
    /// Fabricates a gate's output so the flow proceeds. ⚠ The deliverable is server code; this is
    /// the disposable half.
    Satisfy,
    /// Leaves orbit into an activity - the one non-memory action.
    Launch,
}

impl Class {
    pub fn as_str(self) -> &'static str {
        match self {
            Class::Probe => "Probe",
            Class::Satisfy => "Satisfy",
            Class::Launch => "Launch",
        }
    }
    /// The standing warning a `Satisfy` button must render. Empty for the honest classes.
    pub fn warning(self) -> &'static str {
        match self {
            Class::Satisfy => {
                "SATISFY: this fabricates a gate output the server does not yet serve. Everything \
                 downstream is synthetic_ancestor. The deliverable is server code, not this poke."
            }
            _ => "",
        }
    }
}

/// A gate over observed client state. Every field is an AND; a `None` field is "don't care".
/// The gate reads [`super::Observed`], which comes from the same dash panels the operator sees -
/// not from recollection (`design/d1-ghost.md` §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Gate {
    /// Minimum rung the client must be SITTING IN. The two code hooks crash the client below
    /// rung 36 (`design/GUARDIAN-IN-WORLD.md`, hook sequencing), so they gate here.
    pub min_rung: Option<u32>,
    /// ⛔ MAXIMUM rung - the poke's window has an UPPER edge too. The activity launch is only valid
    /// AT orbit; firing it in-world (rung 36) re-enters the launch path and strands the client (it
    /// crashed ghost and dash). A poke with a `max_rung` refuses once the client is past it.
    pub max_rung: Option<u32>,
    /// Minimum `d1guest` liveness. L3 = registry objects are live; every memory write needs it.
    pub min_liveness: u8,
}

impl Gate {
    /// Is this gate satisfied by what we have observed? Returns the first unmet reason, or `None`
    /// when the write may proceed.
    ///
    /// ⛔ An UNKNOWN rung (ladder unresolved) does NOT satisfy a rung bound: a write windowed to a
    /// rung must refuse when we cannot prove the client is inside the window. Cannot-tell is a
    /// refusal, never a pass - the same rule the containment assertion learned (`CLAUDE.md`).
    pub fn unmet(&self, obs: &super::Observed) -> Option<String> {
        if obs.liveness < self.min_liveness {
            return Some(format!(
                "liveness L{} < required L{}",
                obs.liveness, self.min_liveness
            ));
        }
        if let Some(need) = self.min_rung {
            match obs.rung {
                Some(have) if have >= need => {}
                Some(have) => return Some(format!("rung {have} < required {need}")),
                None => {
                    return Some(format!(
                        "rung is UNKNOWN and this poke needs rung >= {need}; cannot-tell refuses"
                    ))
                }
            }
        }
        if let Some(max) = self.max_rung {
            match obs.rung {
                Some(have) if have <= max => {}
                Some(have) => {
                    return Some(format!("rung {have} > {max}: past this poke's window (too late)"))
                }
                None => {
                    return Some(format!(
                        "rung is UNKNOWN and this poke's window ends at rung {max}; cannot-tell refuses"
                    ))
                }
            }
        }
        None
    }
}

/// How a registry entry actually writes. Ghost owns no address arithmetic beyond what the entry
/// states (`design/d1-ghost.md` §6): the executor turns each of these into a `d1guest.py poke` or
/// a `session-params.py` invocation and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum Action {
    /// A single typed DIRECT write: `d1guest.py poke <target> <value> --type <ty> [--abs] [--offset]`.
    /// No pointer indirection - `target` is a raw vaddr (or an `--abs` host address) that is written
    /// as-is. For an indirected write (through a pointer), use [`Action::DerefWrite`], which resolves
    /// the pointer itself; ⛔ `d1guest.py poke --deref --offset X` applies the offset on BOTH sides
    /// of the deref (a bug for X != 0), so ghost never uses it.
    Write {
        target: &'static str,
        abs: bool,
        offset: u64,
        ty: ValTy,
        value: u64,
    },
    /// A write through a pointer chain, resolved by the executor and applied with `--abs` at the
    /// final heap address (the operator's "raw vaddr for the pointer, `--abs` for the heap target"
    /// rule). One or more fields of the pointed-to object are written as a single ledgered poke.
    ///
    /// Resolution: `addr = *(target)`; then for each `o` in `derefs`, `addr = *(addr + o)`; then the
    /// base is `addr + base_offset`; each field is written at `base + field.offset` via `--abs`. This
    /// makes every hop an explicit `d1guest.py read`, so a wrong address is caught by a readback
    /// rather than smeared by `poke --deref`'s double-offset.
    DerefWrite {
        /// The raw vaddr (or `--abs` host address) where the FIRST pointer lives.
        target: &'static str,
        abs: bool,
        /// Offsets applied at each successive deref between the first pointer and the base object.
        /// Empty for a single-level pointer.
        derefs: &'static [u64],
        /// Added to the fully-resolved pointer to reach the base object (pointer arithmetic, not a
        /// deref) - e.g. the `+0x80` from pool header to `player[0]`.
        base_offset: u64,
        /// The fields to write, each at `base + offset`, all in one ledgered poke.
        fields: &'static [WriteField],
    },
    /// A multi-byte code patch, applied inside a freeze window. ⛔ `d1guest.py poke` writes ONE
    /// typed value; a torn multi-byte patch on a live thread crashes the client
    /// (`design/GUARDIAN-IN-WORLD.md`). The executor STOPs the process, writes the bytes as a
    /// sequence of u8 pokes, then CONTs - see [`super::exec`].
    Patch {
        /// The `d1guest` target token for byte 0.
        target: &'static str,
        abs: bool,
        /// The replacement bytes, in order.
        bytes: &'static [u8],
        /// The bytes we expect to find BEFORE patching - a readback guard so we never patch the
        /// wrong address or a moved function.
        expect_before: &'static [u8],
    },
    /// The activity launch: `session-params.py --launch-poke --activity-id <id>`. The one command
    /// that is not a memory write and the only non-`d1guest` tool ghost may spawn.
    Launch { activity_id: u32 },
}

/// One field of an [`Action::DerefWrite`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct WriteField {
    pub offset: u64,
    pub ty: ValTy,
    pub value: u64,
}

/// The `--type` values `d1guest.py poke` accepts. Kept as a closed enum so a control can assert the
/// executor never emits a type the tool would reject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ValTy {
    U8,
    U16,
    U32,
    U64,
}

impl ValTy {
    pub fn as_str(self) -> &'static str {
        match self {
            ValTy::U8 => "u8",
            ValTy::U16 => "u16",
            ValTy::U32 => "u32",
            ValTy::U64 => "u64",
        }
    }
    /// Width in bytes - the size of the readback the executor verifies.
    pub fn width(self) -> usize {
        match self {
            ValTy::U8 => 1,
            ValTy::U16 => 2,
            ValTy::U32 => 4,
            ValTy::U64 => 8,
        }
    }
}

/// One registered poke.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Poke {
    /// Recipe order to reach in-world (`design/GUARDIAN-IN-WORLD.md`). The console and CLI list
    /// pokes by this, so the sequence "launch -> suppress -> force -> hooks -> spawn -> fade" reads
    /// top to bottom. It is a display order; the GATE is what actually enforces the window.
    pub step: u8,
    /// Short slug, the ledger key. Unique across the registry (enforced by a control).
    pub name: &'static str,
    /// One line of what it does, for the button and the ledger.
    pub summary: &'static str,
    pub class: Class,
    pub gate: Gate,
    pub action: Action,
    /// The design doc + section that OWNS this poke. Nonempty, enforced by `C-GHOST-CITATION`.
    pub citation: &'static str,
}

/// The name of the ONLY memory read/write tool ghost spawns.
pub const TOOL_GUEST: &str = "capture-rig/d1guest.py";
/// The name of the ONLY launch tool ghost spawns.
pub const TOOL_PARAMS: &str = "capture-rig/session-params.py";

/// The complete whitelist of executables ghost may spawn. `C-GHOST-NO-LAUNCH` asserts the executor
/// touches nothing outside this set - ghost is never a launcher (`design/d1-ghost.md` §7).
pub const TOOL_WHITELIST: &[&str] = &[TOOL_GUEST, TOOL_PARAMS];

/// The v1 registry: the currently-known synthetic estate from `design/GUARDIAN-IN-WORLD.md` §3-5
/// and `design/activity-lifetime-state.md`. Every entry here is a write the project cannot yet
/// serve; the list shrinks as each is earned onto the wire.
pub fn registry() -> Vec<Poke> {
    let mut v = vec![
        // -- Launch: leave orbit. 407 = Elimination, the stated endgame (memory: activity-launch).
        Poke {
            step: 1,
            name: "launch-407",
            summary: "leave orbit into activity 407 (Elimination); ⛔ ONLY AT ORBIT - firing it in-world re-enters the launch path and strands the client",
            class: Class::Launch,
            // The window is orbit: orbit_loading (26) or orbit (27). Past that the client is already
            // in the activity ladder and a second launch strands it.
            gate: Gate { min_rung: Some(26), max_rung: Some(27), min_liveness: 3 },
            action: Action::Launch { activity_id: 407 },
            citation: "design/activity-launch.md §2; design/GUARDIAN-IN-WORLD.md §3",
        },
        // -- Task-7 first-line cine escape: byte0 of *[0x26A33E8] = 1 (single-level pointer).
        Poke {
            step: 2,
            name: "suppress-cine",
            summary: "task-7 first-line cinematic escape: *[0x26A33E8] byte0 = 1 (during activity load, repeat as needed)",
            class: Class::Satisfy,
            gate: Gate { min_rung: None, max_rung: None, min_liveness: 3 },
            action: Action::DerefWrite {
                target: "0x26A33E8",
                abs: false,
                derefs: &[],
                base_offset: 0,
                fields: &[WriteField { offset: 0, ty: ValTy::U8, value: 1 }],
            },
            citation: "design/GUARDIAN-IN-WORLD.md §3",
        },
        // -- Done mask: force past tasks 9/10 (world content we do not serve). Direct vaddr write.
        Poke {
            step: 3,
            name: "done-mask-7ff",
            summary: "force the rung-34 task done-mask to 0x7FF, marking tasks 9/10 (world content we do not serve) complete so the ladder advances",
            class: Class::Satisfy,
            gate: Gate { min_rung: Some(34), max_rung: None, min_liveness: 3 },
            action: Action::Write {
                target: "0x26B5730",
                abs: false,
                offset: 0,
                ty: ValTy::U32,
                value: 0x7FF,
            },
            citation: "design/GUARDIAN-IN-WORLD.md §3; design/activity-lifetime-state.md",
        },
        // -- Join counter: rung 35 -> 36. Direct vaddr write.
        Poke {
            step: 4,
            name: "join-ctr-0",
            summary: "clear the join counter 0x26B5770 = 0 - completes the activity-host join so the client crosses rung 35 -> 36 (into the in-world/host-setup rung)",
            class: Class::Satisfy,
            gate: Gate { min_rung: Some(35), max_rung: None, min_liveness: 3 },
            action: Action::Write {
                target: "0x26B5770",
                abs: false,
                offset: 0,
                ty: ValTy::U32,
                value: 0,
            },
            citation: "design/GUARDIAN-IN-WORLD.md §3",
        },
        // -- Hook A: fabricate lifetime state 3 in the getter. ⛔ Gated at rung 36: applying it
        //    before in-world crashes the client (hook sequencing).
        Poke {
            step: 5,
            name: "hook-0010c500",
            summary: "code hook: force the activity-lifetime getter (FUN_0010C500) to always report phase 3, which the spawn gate demands and the server does not yet serve",
            class: Class::Satisfy,
            gate: Gate { min_rung: Some(36), max_rung: None, min_liveness: 3 },
            action: Action::Patch {
                target: "0x10C500",
                abs: false,
                bytes: &[0xc7, 0x07, 0x03, 0x00, 0x00, 0x00, 0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3],
                expect_before: &[
                    0x55, 0x48, 0x89, 0xe5, 0x41, 0x56, 0x53, 0x48, 0x83, 0xec, 0x20, 0x49,
                ],
            },
            citation: "design/GUARDIAN-IN-WORLD.md §4",
        },
        // -- Hook B: NOP the per-player reject `je` so construction runs.
        Poke {
            step: 6,
            name: "je-411e16-nop",
            summary: "code hook: NOP the per-player reject branch at 0x411E16 (6 bytes) so execution falls into the Guardian construction body instead of bailing",
            class: Class::Satisfy,
            gate: Gate { min_rung: Some(36), max_rung: None, min_liveness: 3 },
            action: Action::Patch {
                target: "0x411E16",
                abs: false,
                bytes: &[0x90, 0x90, 0x90, 0x90, 0x90, 0x90],
                expect_before: &[0x0f, 0x84, 0x71, 0x03, 0x00, 0x00],
            },
            citation: "design/GUARDIAN-IN-WORLD.md §4",
        },
        // -- Clear the held screen-fade block: the black screen is a PARKED fade object waiting
        //    for a release event we never serve (the task-9/10 hand-off the done-mask force skips),
        //    not a render failure. Pointer at raw vaddr 0x2694B90 -> block; three u32s: count +0x00
        //    1->0, mode +0x14 1->0, alpha +0x18 0x3f800000 (1.0f, fully black) -> 0. One button,
        //    resolved once then written --abs (the offsets are NOT poke --deref offsets).
        Poke {
            step: 8,
            name: "clear-fade",
            summary: "zero the held screen-fade block (count/mode/alpha) through *0x2694B90 - unblocks the black screen (do this LAST, once in-world)",
            class: Class::Satisfy,
            gate: Gate { min_rung: Some(36), max_rung: None, min_liveness: 3 },
            action: Action::DerefWrite {
                target: "0x2694B90",
                abs: false,
                derefs: &[],
                base_offset: 0,
                fields: &[
                    WriteField { offset: 0x00, ty: ValTy::U32, value: 0 },
                    WriteField { offset: 0x14, ty: ValTy::U32, value: 0 },
                    WriteField { offset: 0x18, ty: ValTy::U32, value: 0 },
                ],
            },
            citation: "design/GUARDIAN-IN-WORLD.md (fade lever, 2026-08-11)",
        },
        // -- Fire the spawn: player[0]+0x0C = 3, where player[0] = *(*(0x2725700)+8) + 0x80.
        //    ⛔ Double deref + a pointer-arithmetic base offset; the pool HEADER is at +0x0, player 0
        //    at +0x80. Resolved as: p = *(0x2725700); q = *(p+8); base = q+0x80; write base+0xC --abs.
        Poke {
            step: 7,
            name: "spawn-word-3",
            summary: "request the Guardian spawn: player[0]+0x0C = 3 fires the construction path the NOP unguarded. ⚠ only after 'fully_enabled' in-world, or it spawns into an unready world",
            class: Class::Satisfy,
            gate: Gate { min_rung: Some(36), max_rung: None, min_liveness: 3 },
            action: Action::DerefWrite {
                target: "0x2725700",
                abs: false,
                derefs: &[0x8],
                base_offset: 0x80,
                fields: &[WriteField { offset: 0x0C, ty: ValTy::U32, value: 3 }],
            },
            citation: "design/GUARDIAN-IN-WORLD.md §5 (player[0] = [[0x2725700]+8]+0x80)",
        },
    ];
    // Present in recipe order, so every surface (CLI, console) lists the sequence to enter game
    // top to bottom regardless of declaration order.
    v.sort_by_key(|p| p.step);
    v
}

/// Look one poke up by its ledger name.
pub fn find(name: &str) -> Option<Poke> {
    registry().into_iter().find(|p| p.name == name)
}

/// The registered `Launch` entry for a given activity index, if one exists. Only the wired
/// endgame (407) has an entry today; other launchable indices show in the selector but are not yet
/// firable, which the selector says.
pub fn find_launch(activity_id: usize) -> Option<Poke> {
    registry().into_iter().find(|p| {
        matches!(p.action, Action::Launch { activity_id: a } if a as usize == activity_id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// C-GHOST-CITATION: every entry carries a nonempty citation and a class. No exemptions.
    #[test]
    fn every_entry_is_cited() {
        for p in registry() {
            assert!(!p.citation.trim().is_empty(), "{} has no citation", p.name);
            assert!(!p.summary.trim().is_empty(), "{} has no summary", p.name);
        }
    }

    /// Names are the ledger key: they must be unique.
    #[test]
    fn names_are_unique() {
        let mut seen = HashSet::new();
        for p in registry() {
            assert!(seen.insert(p.name), "duplicate registry name {}", p.name);
        }
    }

    /// The gate must refuse an unknown rung when the entry requires one - cannot-tell is a refusal.
    #[test]
    fn unknown_rung_refuses_a_rung_gate() {
        let hook = find("hook-0010c500").unwrap();
        let obs = super::super::Observed { rung: None, state: String::new(), liveness: 3 };
        assert!(hook.gate.unmet(&obs).is_some(), "unknown rung must not pass a rung-36 gate");
        let low = super::super::Observed { rung: Some(30), state: String::new(), liveness: 3 };
        assert!(hook.gate.unmet(&low).is_some(), "rung 30 must not pass a rung-36 gate");
        let ok = super::super::Observed { rung: Some(36), state: String::new(), liveness: 3 };
        assert!(hook.gate.unmet(&ok).is_none(), "rung 36 L3 must pass");
    }

    /// Liveness below L3 refuses every write.
    #[test]
    fn dead_client_refuses_every_write() {
        let dead = super::super::Observed { rung: Some(36), state: String::new(), liveness: 1 };
        for p in registry() {
            assert!(p.gate.unmet(&dead).is_some(), "{} wrote into an L1 client", p.name);
        }
    }

    /// A patch's expect_before and bytes must be the same length - we replace in place, never
    /// resize code.
    #[test]
    fn patches_are_length_preserving() {
        for p in registry() {
            if let Action::Patch { bytes, expect_before, .. } = &p.action {
                assert_eq!(
                    bytes.len(),
                    expect_before.len(),
                    "{} patch changes code length",
                    p.name
                );
            }
        }
    }
}
