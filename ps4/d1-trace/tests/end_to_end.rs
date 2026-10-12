// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! End-to-end regression tests over `load_run`, built from fixtures in a temp directory.
//!
//! The unit tests in `ingest.rs`/`query.rs` pin the individual predicates; these pin the *whole
//! pipeline*, because every defect in the 2026-08-08 adversarial review survived a green unit-test
//! suite. Each test names the defect it would have caught.
//!
//! Fixtures are written under a per-test directory inside `std::env::temp_dir()` and removed at the
//! end; nothing under `runs/` is touched (it is read-only evidence).

use std::path::{Path, PathBuf};

use d1_trace::model::{BapDir, Confidence, Stream};
use d1_trace::{load_run, Query, Trace};

/// A responder.log with: listener banners, an HTTP exchange whose body carries the BAP endpoint,
/// then three BAP frames - outer header, raw hex, inner decode, reply identity, server reply - plus
/// the annotation lines that used to be dropped on the floor.
const RESPONDER: &str = "\
 INFO listening http://0.0.0.0:80
 INFO --> POST www4.signon.deadorbit.net/SignOn  797B
 INFO     authorization: Bearer d1-local-token
 INFO     success response: type=0 silo=12345 env=live bap=10.77.0.2:3075 manifest_url=http://cdn/x.bin
 INFO <-- 200 SignOn, 201B placeholder (synthetic-generic) - watching for the parser's complaint
 WARN === BAP TCP CONNECT from 10.77.0.2:51260 ===
 WARN     BAP TCP msg#1 140B [type 0102 len 134 (+6 = 140)]
 INFO       01 02 00 00 00 86 00 1e 00 00 00 00 44 45 53 54
 INFO       envelope v1 mode 2 payload 134 | inner type 0x001e seq 0
 INFO       -> start response (0x001f), seq 0
 WARN     --> BAP REPLY 142B: 01 02 00 00 00 88 00 1f 00 00
 WARN     BAP TCP msg#2 28B [type 0101 len 22 (+6 = 28)]
 INFO       01 01 00 00 00 16 b8 d4 0f 98 99 bc b6 62 57 08
 WARN       SECURED (6B): 00 79 00 00 00 02 -> Ok(Inner { msg_type: 121, sequence: 2 })
 WARN       INVESTMENT message id 503 (47 body bytes)
 WARN       503 RAW BODY (45B): 180000000d100000000000001000000000
 INFO       -> register subscriber rsp (0x007a), seq 2
 WARN     --> BAP SECURED REPLY 30B: 01 01 00 00 00 18 fe fa 14 40
 WARN       ⭐ FAMILY 4 ANSWER seq 0 root 0xd100000000000001 - answering the SUBSCRIPTION, not pushing
 WARN     BAP TCP msg#3 28B [type 0101 len 22 (+6 = 28)]
 INFO       01 01 00 00 00 16 b8 d4 0f 98 99 bc b6 62 57 09
 WARN       SECURED (6B): 00 fa 00 00 00 63 -> Ok(Inner { msg_type: 250, sequence: 99 })
 INFO       -> echo rsp (0x00fb), seq 99
 WARN     --> BAP SECURED REPLY 30B: 01 01 00 00 00 18 fe fa 14 41
 INFO --> PUT srv.shadps4.net:31315/v1/sessions/000  128B
 INFO <-- 200 session updated (128B)
";

const GUEST: &str = "\
[Tty] <Info> log_flush: [stdout] networking:sku_manager: file version=2
[Tty] <Info> log_flush: [stdout] networking:bootflow: entering 'bootflow:prepare_for_orbit'
[Tty] <Info> log_flush: [stdout] investment: error.content_manifest_not_provided
";

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    /// `probe_explain` supplies the `explain` lines for two synthetic probe records; passing the
    /// reworded labels reproduces F4.
    fn new(name: &str) -> Fixture {
        let dir = std::env::temp_dir().join(format!(
            "d1-trace-test-{}-{name}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("responder.log"), RESPONDER).unwrap();
        std::fs::write(dir.join("emulator.log"), GUEST).unwrap();
        Fixture { dir }
    }

    fn guest_bytes(self, bytes: &[u8]) -> Self {
        std::fs::write(self.dir.join("emulator.log"), bytes).unwrap();
        self
    }

    fn trace_bytes(self, bytes: &[u8]) -> Self {
        std::fs::write(self.dir.join("account-trace.bin"), bytes).unwrap();
        self
    }

    fn path(&self) -> &Path {
        &self.dir
    }

    fn load(&self) -> Trace {
        load_run(&self.dir).expect("load_run must not refuse a run directory")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Only ever removes the directory this fixture created.
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn rows(t: &Trace, stream: Stream) -> Vec<&d1_trace::Event> {
    t.events.iter().filter(|e| e.stream == stream).collect()
}

// --------------------------------------------------------------------------------------------
// F2 - no probe spine ⇒ no invented cross-stream order
// --------------------------------------------------------------------------------------------

#[test]
fn f2_no_probe_trace_refuses_to_synthesise_cross_stream_order() {
    let f = Fixture::new("f2");
    let t = f.load();

    assert!(!t.summary.spine, "a run with no probes has no spine");
    assert!(
        t.events.iter().all(|e| e.confidence == Confidence::Approx),
        "every row must be ≈ when there is no spine"
    );
    assert!(
        t.summary
            .notes
            .iter()
            .any(|n| n.contains("NO CROSS-STREAM SPINE")),
        "the refusal must be LOUD, not implied: {:?}",
        t.summary.notes
    );

    // The old bug: one distinct order value (0.0) for every row, then a stream-priority tiebreak
    // that manufactured "all guest before all http before all bap".
    let distinct: std::collections::BTreeSet<u64> =
        t.events.iter().map(|e| e.order.to_bits()).collect();
    assert!(
        distinct.len() > 1,
        "all {} rows collapsed onto one order value again",
        t.events.len()
    );

    // Lanes are grouped, and within a lane the native ordinal is monotonic.
    for stream in [Stream::Guest, Stream::Http, Stream::Bap] {
        let ords: Vec<u64> = rows(&t, stream).iter().map(|e| e.ordinal).collect();
        assert!(
            ords.windows(2).all(|w| w[0] <= w[1]),
            "{stream:?} lane is not in its own native order"
        );
    }
}

// --------------------------------------------------------------------------------------------
// F3 - one invalid UTF-8 byte must not silently zero a lane
// --------------------------------------------------------------------------------------------

#[test]
fn f3_invalid_utf8_does_not_zero_the_guest_lane() {
    let mut bytes = GUEST.as_bytes().to_vec();
    // Truncate a multibyte char the way a byte-range snapshot of a live run does.
    bytes.extend_from_slice(b"[Tty] <Info> log_flush: [stdout] tainted \xff line\n");
    let f = Fixture::new("f3").guest_bytes(&bytes);
    let t = f.load();

    assert_eq!(
        t.summary.guest, 4,
        "all four guest lines must survive one bad byte (got {})",
        t.summary.guest
    );
    assert!(
        t.summary.notes.iter().any(|n| n.contains("not valid UTF-8")),
        "the lossy decode must be reported: {:?}",
        t.summary.notes
    );
    // And the lane is genuinely usable, not just counted.
    assert!(t.summary.bootflow_reached.is_some());
}

#[test]
fn f3_unreadable_lane_is_a_note_not_an_empty_success() {
    let f = Fixture::new("f3b");
    // A directory where emulator.log should be: read() fails with EISDIR, not NotFound.
    std::fs::remove_file(f.path().join("emulator.log")).unwrap();
    std::fs::create_dir(f.path().join("emulator.log")).unwrap();
    let t = f.load();
    assert_eq!(t.summary.guest, 0);
    assert!(
        t.summary
            .notes
            .iter()
            .any(|n| n.contains("EMPTY BECAUSE OF AN ERROR")),
        "a read error must say so: {:?}",
        t.summary.notes
    );
}

// --------------------------------------------------------------------------------------------
// F6 / F10 - one row per wire frame, and both directions present
// --------------------------------------------------------------------------------------------

#[test]
fn f6_outer_headers_are_folded_and_f10_sends_are_on_the_timeline() {
    let f = Fixture::new("f6");
    let t = f.load();

    let recv = t
        .events
        .iter()
        .filter(|e| e.bap.as_ref().is_some_and(|b| b.direction == BapDir::Recv))
        .count();
    let send = t
        .events
        .iter()
        .filter(|e| e.bap.as_ref().is_some_and(|b| b.direction == BapDir::Send))
        .count();
    assert_eq!(recv, 3, "three wire frames ⇒ three recv rows, not six");
    assert_eq!(send, 3, "F10: BapDir::Send must actually be constructed");

    // No standalone `BAP TCP msg#N` rows, and no `BAP BAP …` double prefix.
    assert_eq!(
        t.events
            .iter()
            .filter(|e| e.summary.contains("BAP TCP msg#"))
            .count(),
        0
    );
    assert_eq!(
        t.events.iter().filter(|e| e.summary.contains("BAP BAP")).count(),
        0
    );

    // The header is not lost - it is the frame's detail, and so is the raw hex.
    let first = t
        .events
        .iter()
        .find(|e| e.summary.contains("BAP recv type=0x001e"))
        .expect("the envelope frame");
    assert!(first.detail.iter().any(|d| d.contains("BAP TCP msg#1")));
    assert!(first.detail.iter().any(|d| d.contains("44 45 53 54")));

    // The send row inherits the frame's REPLY identity, which is what the guest's rx probes see.
    let send_ids: Vec<(Option<u16>, Option<u32>)> = t
        .events
        .iter()
        .filter_map(|e| e.bap.as_ref())
        .filter(|b| b.direction == BapDir::Send)
        .map(|b| (b.msg_type, b.sequence))
        .collect();
    assert_eq!(
        send_ids,
        vec![
            (Some(0x001f), Some(0)),
            (Some(0x007a), Some(2)),
            (Some(0x00fb), Some(99)),
        ],
        "each send row carries the reply (type, seq) of its own frame"
    );
}

// --------------------------------------------------------------------------------------------
// F7 - nothing is dropped, and search reaches detail
// --------------------------------------------------------------------------------------------

#[test]
fn f7_previously_dropped_lines_are_present_and_searchable() {
    let f = Fixture::new("f7");
    let t = f.load();

    // Every one of these was measured as 0 matches before the fix.
    for needle in [
        "world_tag_absent_here", // negative control, see below
        "bearer",
        "manifest_url",
        "investment message id 503",
        "503 raw body",
        "family 4 answer",
        "success response",
        "listening http",
    ] {
        let hits = t.events.iter().filter(|e| Query::parse(needle).matches(e)).count();
        if needle == "world_tag_absent_here" {
            assert_eq!(hits, 0, "negative control: search must not match everything");
        } else {
            assert!(hits > 0, "F7: `{needle}` is still unreachable");
        }
    }

    // The BAP endpoint the whole boot depends on is findable.
    assert!(t
        .events
        .iter()
        .any(|e| Query::parse("10.77.0.2:3075").matches(e)));
}

#[test]
fn f7_detail_search_finds_a_probe_register_annotation() {
    // Probe `explain` lines are detail; `world_tag` never appears in a summary.
    let mut ev = d1_trace::Event {
        id: 0,
        stream: Stream::Probe,
        order: 53.0,
        confidence: Confidence::Exact,
        ordinal: 53,
        summary: "#53 item_tag_resolved    guest=0x00555775".into(),
        detail: vec!["     world_tag=0x80a6035a resolved".into()],
        probe: None,
        bap: None,
    };
    assert!(Query::parse("world_tag").matches(&ev));
    ev.detail.clear();
    assert!(!Query::parse("world_tag").matches(&ev));
}

// --------------------------------------------------------------------------------------------
// F8 - a broken probe lane must not refuse the whole directory
// --------------------------------------------------------------------------------------------

#[test]
fn f8_undecodable_trace_still_opens_the_run() {
    // 100 bytes is not a multiple of the 232-byte record - the 216-byte-era failure exactly.
    let f = Fixture::new("f8").trace_bytes(&[0u8; 100]);
    let t = f.load(); // load(), not a Result check: refusing here is the defect.

    assert_eq!(t.summary.probe, 0);
    assert!(t.summary.http > 0 && t.summary.bap > 0 && t.summary.guest > 0,
        "the readable lanes must still be there: h{} b{} g{}",
        t.summary.http, t.summary.bap, t.summary.guest);
    assert!(
        t.summary
            .notes
            .iter()
            .any(|n| n.contains("EMPTY BECAUSE OF AN ERROR")),
        "the decoder's stderr must be carried as a note: {:?}",
        t.summary.notes
    );
}

// --------------------------------------------------------------------------------------------
// F1 - bracketing is only asserted between two anchors (end to end, via a real anchor)
// --------------------------------------------------------------------------------------------

/// Build an `account-trace.bin` is not possible without the 232-byte writer, so F1 is pinned
/// end-to-end by `d1-trace controls` against the pinned run (C-NO-FALSE-BRACKET, C-SIGNON-ORDER)
/// and here by the interpolation unit tests in `ingest.rs`. What this test pins is the *fallback*:
/// with no anchors at all, nothing may claim `~`.
#[test]
fn f1_without_anchors_nothing_claims_bracketed() {
    let f = Fixture::new("f1");
    let t = f.load();
    assert_eq!(
        t.events
            .iter()
            .filter(|e| e.confidence == Confidence::Bracketed)
            .count(),
        0,
        "no anchors exist, so no row may be labelled ~"
    );
}

// --------------------------------------------------------------------------------------------
// F5 - UTF-8 survives the whole pipeline
// --------------------------------------------------------------------------------------------

#[test]
fn f5_utf8_survives_ingest() {
    let f = Fixture::new("f5");
    let t = f.load();
    let star = t
        .events
        .iter()
        .find(|e| e.summary.contains("FAMILY 4 ANSWER"))
        .expect("the ⭐ annotation must reach the timeline");
    assert!(star.summary.starts_with('⭐'), "got {:?}", star.summary);
    assert!(
        t.events
            .iter()
            .any(|e| e.summary.contains("watching for the parser's complaint")
                && e.summary.contains('-')),
        "the em dash must not be mojibake"
    );
}
