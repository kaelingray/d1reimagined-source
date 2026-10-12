// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The reducer: guest log lines in, dashboard facts out.
//!
//! **This is a reducer with a tape attached, not a tape.** 89% of `emulator.log` is `Render.Vulkan`
//! spam; the guest's own narration is 772 `[Tty]` lines out of 206,783 in the reference run - 0.37%.
//! A tail widget buries the state machine, so everything here is fed line-by-line and folded into
//! counters, ladders and last-seen blocks, with a **bounded** ring buffer for the tape.
//!
//! Incremental by construction: [`Reducer::feed_emulator_line`] is called once per raw line, whether
//! that line came from the initial whole-file load or from a 250 ms tail poll. There is exactly one
//! parse path, so a live board and a post-hoc board cannot disagree.
//!
//! ## What it refuses to do
//!
//! - **It does not parse KTOBUCKET's numeric interior.** The client's own printf emits visible
//!   garbage there (`[qlen r/t 8/-273202064][type -48][control policy -85]`); rendering that as a
//!   typed field manufactures a finding. The block is kept **verbatim**.
//! - **It does not collapse `AH->0` and `AH->--------:--------`.** Never-set and cleared are
//!   different facts.
//! - **It does not assert a cross-lane order.** Ordinals are within-lane line numbers only.

use std::collections::{BTreeMap, VecDeque};

use serde::Serialize;

/// The `[stdout]` marker that separates the guest's own narration from emulator chatter. Splitting
/// on it is exactly what `d1_run` does; keeping the constant here means the two cannot silently
/// disagree about where a guest line starts.
pub const GUEST_MARKER: &str = "log_flush: [stdout] ";

/// Hard scrollback cap for the TTY tape. Uniform line height + a bounded deque is what keeps the
/// viewer at 60 fps over a 234 MB log.
pub const TAPE_CAP: usize = 20_000;

/// The marker on shadPS4's own device line:
/// `vk_instance.cpp:638 CollectDeviceParameters: GPU_Model: llvmpipe (LLVM 21.1.8, 256 bits)`.
/// ⚠ This is a NON-guest line - no `log_flush: [stdout]` - so it is scraped from the raw line
/// BEFORE the guest-marker check, which would otherwise discard it.
pub const GPU_MODEL_MARKER: &str = "GPU_Model: ";

/// The substring that marks the device as CPU software rasterisation. Rendered as a red lamp.
pub const GPU_SOFTWARE: &str = "llvmpipe";

/// Strip ANSI SGR escapes at **ingest**, iterating `chars()`.
///
/// ⚠ F5: a byte-wise `push(b as char)` is a Latin-1 decode of UTF-8 - it turns `⭐` into mojibake
/// and makes the responder's highest-signal annotations permanently unmatchable. This delegates to
/// the one stripper `d1-trace` already ships and pins with `C-UTF8-STRIP`.
pub fn strip_ansi(line: &str) -> String {
    crate::ingest::strip_ansi_for_controls(line)
}

/// One `Entering`/`Leaving` state transition.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Transition {
    /// Line number in `emulator.log`. Within-lane ordinal only - **never** a cross-lane order key.
    pub line_no: u64,
    pub entering: bool,
    pub state: String,
    pub reason: String,
    /// Rung id, or `None` when the ladder is unavailable or the client named an unknown state.
    pub rung: Option<u32>,
}

impl Transition {
    /// A transition is a failure only because of its **reason**, never because of its state name.
    /// `cleanup` for `normal_flow` is the ordinary path from `character:prologue` into
    /// `setup:orbit_loading` and every clean boot takes it.
    pub fn is_error(&self) -> bool {
        self.reason.starts_with("error.")
    }
}

/// An `error.*` leaving-reason, with the state it left and (once known) the state it entered.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ErrorEvent {
    pub line_no: u64,
    pub reason: String,
    pub left_state: String,
    pub entered_state: Option<String>,
    pub rung: Option<u32>,
    /// The on-screen codename, where the project has measured one.
    pub codename: Option<&'static str>,
    /// The unmodified guest line.
    pub raw: String,
}

/// On-screen codenames for the errors this project has matched to a screen. Deliberately short:
/// guessing a codename would be worse than leaving it blank.
pub fn codename(reason: &str) -> Option<&'static str> {
    match reason.trim_start_matches("error.") {
        "lost_bap_connection" => Some("weasel"),
        "content_manifest_not_provided" => Some("termite"),
        _ => None,
    }
}

/// The activity-client flag strip, split into labelled lamps.
///
/// `[AC PRIVATE CURRENT CON-Y EST-N AH->0 MEM-0]`
///
/// ⚠ `CON-Y EST-N` is precisely the 2026-08-08 failure: **connected but never established**. A
/// panel that shows only "connected" reads that as success.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AcFlags {
    /// `PRIVATE` / `PUBLIC` - kept as the token the client printed.
    pub privacy: String,
    /// `CURRENT` / anything else the client prints in that slot.
    pub currency: String,
    /// `CON-Y` → `Some(true)`.
    pub connected: Option<bool>,
    /// `EST-N` → `Some(false)`.
    pub established: Option<bool>,
    /// Verbatim. `0` (never set) and `--------:--------` (cleared) are **different** and are not
    /// collapsed here or anywhere downstream.
    pub ah: String,
    /// Verbatim, for the same reason.
    pub mem: String,
    /// The whole bracketed strip exactly as printed.
    pub raw_strip: String,
    /// The whole guest line it came from.
    pub raw_line: String,
    pub line_no: u64,
}

/// Parse `[AC PRIVATE CURRENT CON-Y EST-N AH->0 MEM-0]` out of a guest line.
pub fn parse_ac(line: &str, line_no: u64) -> Option<AcFlags> {
    let start = line.find("[AC ")?;
    let rest = &line[start..];
    let end = rest.find(']')? + 1;
    let strip = &rest[..end];
    let inner = &strip[4..strip.len() - 1];
    let toks: Vec<&str> = inner.split_whitespace().collect();
    let mut f = AcFlags {
        privacy: String::new(),
        currency: String::new(),
        connected: None,
        established: None,
        ah: String::new(),
        mem: String::new(),
        raw_strip: strip.to_string(),
        raw_line: line.to_string(),
        line_no,
    };
    for (i, t) in toks.iter().enumerate() {
        if let Some(v) = t.strip_prefix("CON-") {
            f.connected = yn(v);
        } else if let Some(v) = t.strip_prefix("EST-") {
            f.established = yn(v);
        } else if let Some(v) = t.strip_prefix("AH->") {
            f.ah = v.to_string();
        } else if let Some(v) = t.strip_prefix("MEM-") {
            f.mem = v.to_string();
        } else if i == 0 {
            f.privacy = t.to_string();
        } else if i == 1 {
            f.currency = t.to_string();
        }
    }
    Some(f)
}

fn yn(v: &str) -> Option<bool> {
    match v {
        "Y" => Some(true),
        "N" => Some(false),
        _ => None,
    }
}

/// One KTOBUCKET line, kept **verbatim and unparsed**.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct KtoLine {
    pub line_no: u64,
    /// The text after `KTOBUCKET: `, byte-for-byte as the client printed it.
    pub text: String,
}

/// What the guest's own `push:queuez` / `server_object_manager` lines said about a family.
///
/// ⚠ This lane **corroborates**; it never establishes replication. State 2 is written *locally* at
/// send time, so a send log proves nothing about replication - only `d1guest.py descriptors`
/// (the client's own table) can say `state=3`. The two are rendered as separate columns.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct QueuezObservation {
    pub family_type: u32,
    pub root_soid: Option<String>,
    /// The last `object_count N` seen for this family in a push line.
    pub object_count: Option<u64>,
    /// True if an `object-count of zero in [...]` line named this family.
    pub saw_zero: bool,
    /// True if a `failed to process update` line named this family.
    pub saw_failure: bool,
    /// True if an `ignoring family-update due to unhandled replication state` line named it.
    pub saw_unhandled: bool,
    /// Every raw line that mentioned this family, verbatim.
    pub raw: Vec<String>,
}

/// A guest narration line, kept for the tape.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TtyLine {
    pub line_no: u64,
    pub subsystem: String,
    pub text: String,
    pub is_error: bool,
}

/// The full-path subsystem chip for a guest line.
///
/// ⚠ Deliberately **not** `d1_run`'s `guest_subsystem`, which collapses to one or two levels
/// (`networking:investment`). The dashboard's chips are the client's own full colon path
/// (`networking:investment:push:queuez`, `world_controller:state_manager`) because the 2026-08-08
/// session grepped at that granularity. Two different questions, two functions; neither is a
/// re-description of a format.
pub fn subsystem(msg: &str) -> String {
    // The namespace is the run of `[a-z0-9_:]` before the first ": " that ends it.
    let bytes = msg.as_bytes();
    let mut last_colon = None;
    for (i, c) in msg.char_indices() {
        if c == ':' {
            // A namespace colon is followed by another namespace char or by a space that ends it.
            let next = bytes.get(i + 1).copied();
            if next == Some(b' ') || next.is_none() {
                last_colon = Some(i);
                break;
            }
            continue;
        }
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' {
            continue;
        }
        break;
    }
    match last_colon {
        Some(i) if i > 0 => msg[..i].to_string(),
        _ => "(unnamespaced)".to_string(),
    }
}

/// `Entering state 'X' for reason 'Y'.` → `(entering, X, Y)`
fn parse_state_line(body: &str) -> Option<(bool, String, String)> {
    let entering = body.contains("Entering state ");
    let leaving = body.contains("Leaving state ");
    if !entering && !leaving {
        return None;
    }
    let key = if entering {
        "Entering state "
    } else {
        "Leaving state "
    };
    let rest = body.split(key).nth(1)?;
    let state = rest.split('\'').nth(1)?.to_string();
    let reason = rest.split("for reason ").nth(1)?.split('\'').nth(1)?.to_string();
    Some((entering, state, reason))
}

/// The accumulated dashboard state for one run.
#[derive(Debug, Default)]
pub struct Reducer {
    /// Raw lines fed (every line of `emulator.log`, spam included).
    pub raw_lines: u64,
    /// Guest `[stdout]` lines fed - the only ones that carry a guest fact.
    pub guest_lines: u64,
    /// Bytes of `emulator.log` seen. Used to tell "no log" from "log with no guest narration".
    pub emulator_bytes: u64,

    pub transitions: Vec<Transition>,
    pub errors: Vec<ErrorEvent>,
    /// Distinct states, in first-entry order.
    pub states_in_order: Vec<String>,
    /// Per-state entry counts - a repeat is evidence, not noise.
    pub state_counts: BTreeMap<String, u64>,

    pub ac: Option<AcFlags>,
    /// The most recent KTOBUCKET block, verbatim.
    pub ktobucket: Vec<KtoLine>,
    /// Total KTOBUCKET lines ever seen (the current block is only the last one).
    pub ktobucket_total: u64,
    /// The state-error that the most recent KTOBUCKET block is the autopsy of.
    pub ktobucket_stamp: Option<String>,

    pub queuez: BTreeMap<u32, QueuezObservation>,

    pub tape: VecDeque<TtyLine>,
    pub tape_dropped: u64,
    pub subsystems: BTreeMap<String, u64>,

    /// True if any lane decoded lossily (an invalid byte was replaced). Loud, never silent.
    pub lossy: bool,

    /// The Vulkan device shadPS4 actually created, verbatim from `GPU_Model: `. `None` until the
    /// line is seen. ⛔ 268 of the project's runs read `llvmpipe` (CPU software rasterisation) and
    /// nothing surfaced it; the 6-7 fps in-game was that, not the game. This is NOT emulator chatter
    /// to be filtered - it is the single most load-bearing config fact on the page.
    pub gpu_model: Option<String>,

    /// Rung lookup, injected so the reducer never transcribes the ladder.
    rungs: BTreeMap<String, u32>,
    ladder_available: bool,
    /// Index into `ktobucket` where the block currently being accumulated began.
    kto_block_open: bool,
}

impl Reducer {
    pub fn new(ladder: &super::ladder::Ladder) -> Reducer {
        Reducer {
            rungs: ladder
                .rungs
                .iter()
                .map(|r| (r.state.clone(), r.rung))
                .collect(),
            ladder_available: ladder.source.is_available(),
            ..Default::default()
        }
    }

    fn rung_of(&self, state: &str) -> Option<u32> {
        if !self.ladder_available {
            return None;
        }
        self.rungs.get(state).copied()
    }

    /// **Furthest** rung entered - `max()`, not `last()` - over states the run actually EARNED.
    ///
    /// A run that reached `setup:activity_host_setup` (30) and fell back through `cleanup` to
    /// `setup:orbit` (27) reports **30**. A failed run that re-enters `bootflow:start` still reports
    /// its deepest rung, not 4.
    ///
    /// ⛔ R1 (review 2026-08-09, fixed 2026-08-11): `cleanup` (rung 25) is excluded OUTRIGHT, and so
    /// is any state entered for an `error.*` reason. `cleanup` is both ladder position 25 and the
    /// universal failure sink, so counting it put a floor of 25 under exactly the runs that failed:
    /// `20260806-140313-signon-f2` died twice at `bootflow:content_check` (rung 13) and the headline
    /// read "rung 25 - cleanup", in green. Depth is what the run reached on purpose.
    pub fn furthest(&self) -> Option<(u32, String)> {
        self.transitions
            .iter()
            .filter(|t| t.entering && t.state != "cleanup" && !t.is_error())
            .filter_map(|t| t.rung.map(|r| (r, t.state.clone())))
            .max_by_key(|(r, _)| *r)
    }

    /// The rung the client is **sitting in** - the last state entered. Rendered differently from a
    /// rung it merely passed through.
    pub fn current(&self) -> Option<(Option<u32>, String, String)> {
        self.transitions
            .iter()
            .rev()
            .find(|t| t.entering)
            .map(|t| (t.rung, t.state.clone(), t.reason.clone()))
    }

    /// The guest log exists and has content, but carries **no guest narration at all**.
    ///
    /// ⛔ This is an INSTRUMENT FAULT, not rung 0 - the guest-diagnostic sink patch was not applied.
    /// The two-gate trap makes it silent by design: 626 of 1,614 sites already pass the default
    /// filter and still emit nothing, so widening the filter alone produces zero new output and
    /// looks exactly like "the game never got there".
    pub fn instrument_fault(&self) -> bool {
        self.emulator_bytes > 0 && self.guest_lines == 0
    }

    /// Feed one raw `emulator.log` line (ANSI included).
    pub fn feed_emulator_line(&mut self, raw: &str, line_no: u64) {
        self.raw_lines += 1;
        let Some(pos) = raw.find(GUEST_MARKER) else {
            // GPU_Model is emulator chatter, not guest narration - catch it here or never.
            // Last-wins: if a run directory is reused, the newest emulator start is the truth.
            if let Some(i) = raw.find(GPU_MODEL_MARKER) {
                let v = strip_ansi(&raw[i + GPU_MODEL_MARKER.len()..]);
                let v = v.trim();
                if !v.is_empty() {
                    self.gpu_model = Some(v.to_string());
                }
            }
            return;
        };
        let clean = strip_ansi(&raw[pos + GUEST_MARKER.len()..]);
        let body = clean.trim();
        if body.is_empty() {
            return;
        }
        self.feed_guest_body(body, line_no);
    }

    /// Feed one already-extracted guest narration line. Exposed for fixtures and controls.
    ///
    /// ⚠ The `guest_lines` counter lives HERE, not in [`Reducer::feed_emulator_line`], so a fixture
    /// that feeds bodies directly is not mistaken for an instrument fault. Counting it in the outer
    /// function made every synthetic board report "no guest narration" and suppress its own rung.
    pub fn feed_guest_body(&mut self, body: &str, line_no: u64) {
        self.guest_lines += 1;
        let sub = subsystem(body);
        *self.subsystems.entry(sub.clone()).or_insert(0) += 1;

        // ---- D1/D2: the ladder and the error tape -------------------------------------------
        if let Some((entering, state, reason)) = parse_state_line(body) {
            let rung = self.rung_of(&state);
            if entering {
                if !self.states_in_order.iter().any(|s| s == &state) {
                    self.states_in_order.push(state.clone());
                }
                *self.state_counts.entry(state.clone()).or_insert(0) += 1;
                // Close out an error that was waiting to learn where the flow went.
                if let Some(e) = self.errors.last_mut() {
                    if e.entered_state.is_none() {
                        e.entered_state = Some(state.clone());
                    }
                }
            }
            if !entering && reason.starts_with("error.") {
                self.errors.push(ErrorEvent {
                    line_no,
                    reason: reason.clone(),
                    left_state: state.clone(),
                    entered_state: None,
                    rung,
                    codename: codename(&reason),
                    raw: body.to_string(),
                });
            }
            // ⛔ R2 (review 2026-08-09, fixed 2026-08-11): early-boot gate failures print ONLY the
            // entering form - `Entering state 'cleanup' for reason 'error.X'.` - with no Leaving
            // line at all. Eight measured runs showed "0 errors" over a named gate failure while
            // the KTOBUCKET panel displayed the error on the same screen. Record from EITHER
            // direction; a Leaving/Entering pair with the same reason on adjacent lines is ONE
            // error, de-duplicated here (the leaving form already recorded it, and the entering
            // fill-in above named where the flow went).
            if entering && reason.starts_with("error.") {
                let already_paired = self.errors.last().is_some_and(|e| {
                    e.reason == reason && line_no.saturating_sub(e.line_no) <= 3
                });
                if !already_paired {
                    // The state this error LEFT is the last state entered before this line.
                    let left_state = self
                        .transitions
                        .iter()
                        .rev()
                        .find(|t| t.entering)
                        .map(|t| t.state.clone())
                        .unwrap_or_default();
                    let left_rung = self.rung_of(&left_state);
                    self.errors.push(ErrorEvent {
                        line_no,
                        reason: reason.clone(),
                        left_state,
                        entered_state: Some(state.clone()),
                        rung: left_rung,
                        codename: codename(&reason),
                        raw: body.to_string(),
                    });
                }
            }
            self.transitions.push(Transition {
                line_no,
                entering,
                state,
                reason,
                rung,
            });
        }

        // ---- D4: the activity-client flag strip ----------------------------------------------
        if body.contains("[AC ") {
            if let Some(ac) = parse_ac(body, line_no) {
                self.ac = Some(ac);
            }
        }

        // ---- D4: the KTOBUCKET autopsy -------------------------------------------------------
        if let Some(idx) = body.find("KTOBUCKET: ") {
            let text = body[idx + "KTOBUCKET: ".len()..].to_string();
            if text.starts_with("STATE_CHANGE_REASON") {
                // A new autopsy begins. Keep only the most recent block.
                self.ktobucket.clear();
                self.kto_block_open = true;
                self.ktobucket_stamp = Some(text.clone());
            } else if !self.kto_block_open {
                self.kto_block_open = true;
                self.ktobucket.clear();
            }
            self.ktobucket.push(KtoLine { line_no, text });
            self.ktobucket_total += 1;
        }

        // ---- D5's corroborating log lane -----------------------------------------------------
        if body.contains("family_type ENUM(") || body.contains("family_type=ENUM(") {
            self.feed_queuez(body);
        }

        // ---- D6: the tape --------------------------------------------------------------------
        let is_error = body.contains("error.");
        if self.tape.len() == TAPE_CAP {
            self.tape.pop_front();
            self.tape_dropped += 1;
        }
        self.tape.push_back(TtyLine {
            line_no,
            subsystem: sub,
            text: body.to_string(),
            is_error,
        });
    }

    fn feed_queuez(&mut self, body: &str) {
        let Some(ty) = between(body, "family_type ENUM(", ")")
            .or_else(|| between(body, "family_type=ENUM(", ")"))
            .and_then(|s| s.trim().parse::<u32>().ok())
        else {
            return;
        };
        let e = self.queuez.entry(ty).or_default();
        e.family_type = ty;
        if let Some(soid) = between(body, "family_root_soid ", ",")
            .or_else(|| between(body, "family_root_soid=", ","))
        {
            e.root_soid = Some(soid.trim().to_string());
        }
        if let Some(n) = between(body, "object_count ", "]").and_then(|s| s.trim().parse::<u64>().ok())
        {
            e.object_count = Some(n);
        }
        if body.contains("object-count of zero") {
            e.saw_zero = true;
        }
        if body.contains("failed to process update") {
            e.saw_failure = true;
        }
        if body.contains("unhandled replication state") {
            e.saw_unhandled = true;
        }
        e.raw.push(body.to_string());
    }
}

fn between<'a>(hay: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = hay.find(start)? + start.len();
    let rest = &hay[i..];
    let j = rest.find(end)?;
    Some(&rest[..j])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dash::ladder::{Ladder, Rung};

    /// A ladder fixture that carries the real rung ids for the states these tests use, without
    /// shelling out - the shell-out itself is exercised by `C-DASH-LADDER-DENOM`.
    fn fixture_ladder() -> Ladder {
        let json = r#"[
            {"rung":4,"state":"bootflow:start"},
            {"rung":22,"state":"bootflow:prepare_for_orbit"},
            {"rung":24,"state":"character:prologue"},
            {"rung":25,"state":"cleanup"},
            {"rung":26,"state":"setup:orbit_loading"},
            {"rung":27,"state":"setup:orbit"},
            {"rung":28,"state":"setup:activity_session_creation"},
            {"rung":29,"state":"setup:matchmaking"},
            {"rung":30,"state":"setup:activity_host_setup"}
        ]"#;
        let rungs: Vec<Rung> = serde_json::from_str(json).unwrap();
        crate::dash::ladder::Ladder::from_rungs_for_tests(rungs)
    }

    fn feed(r: &mut Reducer, lines: &[&str]) {
        for (i, l) in lines.iter().enumerate() {
            r.feed_guest_body(l, i as u64);
        }
    }

    #[test]
    fn subsystem_is_the_full_colon_path() {
        assert_eq!(
            subsystem("world_controller:state_manager: Entering state 'x'."),
            "world_controller:state_manager"
        );
        assert_eq!(
            subsystem("networking:investment:push:queuez: object-count of zero"),
            "networking:investment:push:queuez"
        );
        assert_eq!(subsystem("no namespace here"), "(unnamespaced)");
    }

    #[test]
    fn state_lines_parse_both_directions() {
        let (e, s, r) =
            parse_state_line("world_controller:state_manager: Entering state 'setup:orbit' for reason 'normal_flow'.")
                .unwrap();
        assert!(e && s == "setup:orbit" && r == "normal_flow");
        let (e, s, r) = parse_state_line(
            "world_controller:state_manager: Leaving state 'setup:activity_host_setup' for reason 'error.boom'.",
        )
        .unwrap();
        assert!(!e && s == "setup:activity_host_setup" && r == "error.boom");
    }

    /// ⭐ The axis: depth is `max()`, not `last()`. A run that reached 30 and fell back to 27 must
    /// still report 30 - and a run that re-entered `bootflow:start` must not report 4.
    #[test]
    fn furthest_is_max_not_last() {
        let l = fixture_ladder();
        let mut r = Reducer::new(&l);
        feed(
            &mut r,
            &[
                "world_controller:state_manager: Entering state 'setup:activity_host_setup' for reason 'normal_flow'.",
                "world_controller:state_manager: Entering state 'cleanup' for reason 'error.x'.",
                "world_controller:state_manager: Entering state 'bootflow:start' for reason 'normal_flow'.",
            ],
        );
        assert_eq!(r.furthest().map(|(n, _)| n), Some(30));
        assert_eq!(r.current().map(|(_, s, _)| s), Some("bootflow:start".into()));
    }

    /// ⭐ `cleanup` (rung 25) is a NORMAL TRANSIT STATE: entering it for `normal_flow` is not a
    /// failure, and it must not cap the furthest rung of a run that has been past it.
    #[test]
    fn cleanup_for_normal_flow_is_not_a_failure() {
        let l = fixture_ladder();
        let mut r = Reducer::new(&l);
        feed(
            &mut r,
            &[
                "world_controller:state_manager: Entering state 'character:prologue' for reason 'normal_flow'.",
                "world_controller:state_manager: Entering state 'cleanup' for reason 'normal_flow'.",
                "world_controller:state_manager: Entering state 'setup:orbit' for reason 'normal_flow'.",
            ],
        );
        assert!(r.errors.is_empty(), "normal_flow cleanup is not an error");
        assert!(r.transitions.iter().all(|t| !t.is_error()));
        assert_eq!(r.furthest().map(|(n, _)| n), Some(27), "cleanup did not cap depth");
    }

    /// ⛔ R1: `cleanup` must never RAISE the headline. A run that died at rung 13 and fell into the
    /// failure sink (rung 25) reads 13, not 25. This is the exact shape of
    /// `runs/20260806-140313-signon-f2`, which the 2026-08-09 review reproduced.
    #[test]
    fn cleanup_never_raises_the_headline_rung() {
        let l = fixture_ladder();
        let mut r = Reducer::new(&l);
        feed(
            &mut r,
            &[
                "world_controller:state_manager: Entering state 'bootflow:start' for reason 'normal_flow'.",
                "world_controller:state_manager: Entering state 'cleanup' for reason 'error.content_manifest_not_provided'.",
            ],
        );
        assert_eq!(
            r.furthest().map(|(n, _)| n),
            Some(4),
            "cleanup put a floor of 25 under a rung-4 failure"
        );
        // And a state entered FOR an error reason is not depth either.
        let mut r2 = Reducer::new(&l);
        feed(
            &mut r2,
            &[
                "world_controller:state_manager: Entering state 'bootflow:start' for reason 'normal_flow'.",
                "world_controller:state_manager: Entering state 'setup:orbit' for reason 'error.boom'.",
            ],
        );
        assert_eq!(
            r2.furthest().map(|(n, _)| n),
            Some(4),
            "a rung entered for an error.* reason was not EARNED"
        );
    }

    /// ⛔ R2: an error that prints ONLY on the entering form must land on the tape. Eight measured
    /// runs showed "0 errors" over a named gate failure because the tape only read `Leaving`.
    #[test]
    fn entering_form_only_errors_are_recorded() {
        let l = fixture_ladder();
        let mut r = Reducer::new(&l);
        feed(
            &mut r,
            &[
                "world_controller:state_manager: Entering state 'bootflow:start' for reason 'normal_flow'.",
                "world_controller:state_manager: Entering state 'cleanup' for reason 'error.content_manifest_not_provided'.",
            ],
        );
        assert_eq!(r.errors.len(), 1, "the entering-form error was invisible");
        assert_eq!(r.errors[0].reason, "error.content_manifest_not_provided");
        assert_eq!(r.errors[0].left_state, "bootflow:start");
        assert_eq!(r.errors[0].entered_state.as_deref(), Some("cleanup"));
        assert_eq!(r.errors[0].codename, Some("termite"));
    }

    /// R2's de-dup half: a Leaving/Entering pair with the same reason on adjacent lines is ONE
    /// error, not two.
    #[test]
    fn a_leaving_entering_pair_is_one_error() {
        let l = fixture_ladder();
        let mut r = Reducer::new(&l);
        feed(
            &mut r,
            &[
                "world_controller:state_manager: Leaving state 'setup:activity_host_setup' for reason 'error.boom'.",
                "world_controller:state_manager: Entering state 'cleanup' for reason 'error.boom'.",
            ],
        );
        assert_eq!(r.errors.len(), 1, "the pair was double-counted");
        assert_eq!(r.errors[0].left_state, "setup:activity_host_setup");
        assert_eq!(r.errors[0].entered_state.as_deref(), Some("cleanup"));
        // ...and two SEPARATE failures with the same reason, far apart, stay two.
        let mut r2 = Reducer::new(&l);
        r2.feed_guest_body(
            "world_controller:state_manager: Entering state 'cleanup' for reason 'error.x'.",
            10,
        );
        r2.feed_guest_body(
            "world_controller:state_manager: Entering state 'bootflow:start' for reason 'normal_flow'.",
            50,
        );
        r2.feed_guest_body(
            "world_controller:state_manager: Entering state 'cleanup' for reason 'error.x'.",
            100,
        );
        assert_eq!(r2.errors.len(), 2, "distinct failures were merged");
    }

    #[test]
    fn errors_carry_left_and_entered_states() {
        let l = fixture_ladder();
        let mut r = Reducer::new(&l);
        feed(
            &mut r,
            &[
                "world_controller:state_manager: Leaving state 'setup:activity_host_setup' for reason 'error.boom'.",
                "world_controller:state_manager: Entering state 'cleanup' for reason 'error.boom'.",
            ],
        );
        assert_eq!(r.errors.len(), 1);
        assert_eq!(r.errors[0].left_state, "setup:activity_host_setup");
        assert_eq!(r.errors[0].entered_state.as_deref(), Some("cleanup"));
        assert_eq!(r.errors[0].rung, Some(30));
    }

    #[test]
    fn ac_flags_keep_ah_verbatim() {
        let a = parse_ac("networking:activity_client: [AC PRIVATE CURRENT CON-Y EST-N AH->0 MEM-0] x", 7)
            .unwrap();
        assert_eq!(a.privacy, "PRIVATE");
        assert_eq!(a.connected, Some(true));
        assert_eq!(a.established, Some(false));
        assert_eq!(a.ah, "0");
        let b = parse_ac(
            "world_controller:state_manager: KTOBUCKET: ACTIVITY [AC PRIVATE CURRENT CON-Y EST-N AH->--------:-------- MEM-0] STATUS: [x]",
            8,
        )
        .unwrap();
        assert_eq!(b.ah, "--------:--------");
        // Never-set and cleared are DIFFERENT.
        assert_ne!(a.ah, b.ah);
    }

    #[test]
    fn ktobucket_keeps_only_the_last_block_and_stamps_it() {
        let l = fixture_ladder();
        let mut r = Reducer::new(&l);
        feed(
            &mut r,
            &[
                "world_controller:state_manager: KTOBUCKET: STATE_CHANGE_REASON [error.one], Current State [a]",
                "world_controller:state_manager: KTOBUCKET: SESSION ID:=fireteam:1, HOST=Y",
                "world_controller:state_manager: KTOBUCKET: STATE_CHANGE_REASON [error.two], Current State [b]",
                "world_controller:state_manager: KTOBUCKET: SESSION ID:=posse:2, HOST=Y",
            ],
        );
        assert_eq!(r.ktobucket.len(), 2, "only the most recent block is kept");
        assert_eq!(r.ktobucket_total, 4);
        assert!(r.ktobucket_stamp.as_deref().unwrap().contains("error.two"));
        // Verbatim: the numeric interior is never re-rendered.
        assert_eq!(r.ktobucket[1].text, "SESSION ID:=posse:2, HOST=Y");
    }

    #[test]
    fn queuez_lane_records_object_count_zero() {
        let l = fixture_ladder();
        let mut r = Reducer::new(&l);
        feed(&mut r, &[
            "networking:investment:push:queuez: object-count of zero in [family_type ENUM(0), family_root_soid 0xD100000000000001, version 0, flags 0x01, object_count 0]",
            "networking:investment:push:queuez: failed to process update for [family_type ENUM(4), family_root_soid 0xD100000000000001, version 0, flags 0x01, object_count 1]",
        ]);
        let f0 = &r.queuez[&0];
        assert!(f0.saw_zero && f0.object_count == Some(0));
        let f4 = &r.queuez[&4];
        assert!(f4.saw_failure && f4.object_count == Some(1) && !f4.saw_zero);
    }

    /// The GPU lamp: both measured forms (268 llvmpipe runs, 18 RADV runs) scrape from the raw
    /// NON-guest line, ANSI included, and llvmpipe is recognisably software.
    #[test]
    fn gpu_model_is_scraped_from_the_non_guest_line() {
        let l = fixture_ladder();
        let mut r = Reducer::new(&l);
        assert!(r.gpu_model.is_none(), "None until seen, never a default");
        r.feed_emulator_line(
            "[Render.Vulkan] <Info> (shadPS4:Main) vk_instance.cpp:638 CollectDeviceParameters: GPU_Model: llvmpipe (LLVM 21.1.8, 256 bits)\u{1b}[0m",
            0,
        );
        assert_eq!(r.gpu_model.as_deref(), Some("llvmpipe (LLVM 21.1.8, 256 bits)"));
        assert!(r.gpu_model.as_deref().unwrap().contains(GPU_SOFTWARE));
        // A reused run dir: the newest emulator start wins.
        r.feed_emulator_line(
            "[Render.Vulkan] <Info> (shadPS4:Main) vk_instance.cpp:638 CollectDeviceParameters: GPU_Model: AMD Radeon 8060S Graphics (RADV GFX1151)",
            1,
        );
        assert_eq!(
            r.gpu_model.as_deref(),
            Some("AMD Radeon 8060S Graphics (RADV GFX1151)")
        );
        assert!(!r.gpu_model.as_deref().unwrap().contains(GPU_SOFTWARE));
        // And it never fires on a guest line's text.
        assert_eq!(r.guest_lines, 0);
    }

    /// Zero guest lines over a non-empty log is an instrument fault, never rung 0.
    #[test]
    fn instrument_fault_when_the_sink_never_emitted() {
        let l = fixture_ladder();
        let mut r = Reducer::new(&l);
        r.emulator_bytes = 4096;
        for i in 0..50 {
            r.feed_emulator_line("[Render.Vulkan] <Info> Skipped 3 duplicated messages", i);
        }
        assert!(r.instrument_fault());
        assert!(r.furthest().is_none(), "no rung may be reported");
    }

    #[test]
    fn tape_is_bounded() {
        let l = fixture_ladder();
        let mut r = Reducer::new(&l);
        for i in 0..(TAPE_CAP as u64 + 10) {
            r.feed_guest_body("x:y: hello", i);
        }
        assert_eq!(r.tape.len(), TAPE_CAP);
        assert_eq!(r.tape_dropped, 10);
    }

    /// ⚠ With the ladder unavailable, no rung may be invented - not even for a state we could have
    /// looked up. `None` propagates.
    #[test]
    fn no_ladder_means_no_rung_numbers() {
        let l = Ladder::unavailable("python3 missing");
        let mut r = Reducer::new(&l);
        feed(
            &mut r,
            &["world_controller:state_manager: Entering state 'setup:orbit' for reason 'normal_flow'."],
        );
        assert!(r.furthest().is_none());
        assert_eq!(r.transitions[0].rung, None);
        assert_eq!(r.states_in_order, vec!["setup:orbit".to_string()]);
    }
}
