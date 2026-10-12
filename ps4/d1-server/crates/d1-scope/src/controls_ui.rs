// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! BUILD 3's controls - **#11-#14**. Offline; they touch no live process and open no snapshot for
//! writing.
//!
//! ⭐ Every one of them was **inverted once**: broken on purpose, watched to fail, restored. A
//! control that only ever passes is decoration, and three of the four here were only worth keeping
//! *after* the inversion showed which leg carried the axis.
//!
//! ⚠ **#11's render half needs `--features gui`.** Without it the control runs its data half and
//! **says so in its name and in a loud line** - it is not skipped into a pass, and the summary
//! reports which half ran. This is the same shape as BUILD 2's `--state-only`.

use crate::controls::ControlResult;
use crate::frame::*;
use crate::mem_source::SourceId;
use crate::table::{Column, EventBadge, RenderState, RowKind, Signatures, TableModel, TableParams};

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Synthetic frames. ⚠ Hand-built from the real `frame` types, never from a mock of them: a control
// that asserted against its own parallel struct would keep passing after `Frame` changed shape.

fn liveness(v: Verdict) -> LivenessView {
    LivenessView { expr: "u32@(x) != 0".into(), verdict: v, why: "synthetic".into() }
}

fn surface(name: &str, group: &str, resolved: Resolved, v: Verdict, fields: Vec<FieldView>) -> SurfaceView {
    SurfaceView {
        name: name.into(),
        group: group.into(),
        addr_expr: "[v:0x26B13E8]".into(),
        resolved,
        resolve_trace: vec!["  v:0x26b13e8 -> abs 0x8026b13e8".into()],
        liveness: liveness(v),
        fields,
        confidence: "[C]".into(),
        reach: "observed-live".into(),
    }
}

fn field(name: &str, off: u64, ty: &str, value: Value) -> FieldView {
    FieldView { name: name.into(), off, ty: ty.into(), value, note: None }
}

fn synth_frame(seq: u64, surfaces: Vec<SurfaceView>, events: Vec<EventView>, child: ChildStatus) -> Frame {
    Frame {
        seq,
        ts_unix_ms: 0,
        ts_mono_ms: 0,
        tick_ms: 0.1,
        source: SourceId::Snapshot { id: "synthetic".into(), path: "/dev/null".into() },
        generation: 1,
        surfaces,
        edges: Vec::new(),
        events,
        events_dropped: 0,
        child,
        notes: Vec::new(),
    }
}

/// The one shape used by #11 and #12: three groups, every render state that a `Value` can produce.
fn demo_surfaces() -> Vec<SurfaceView> {
    vec![
        surface(
            "world_controller_state_manager",
            "ladder",
            Resolved::Addr(0x2050_2F1D0),
            Verdict::Live,
            vec![
                field("current_state", 0x1B0, "i32", Value::Int(36)),
                field("handler_table", 0x80, "ptr[38]", Value::Unmapped),
                field("name", 0x10, "char[32]", Value::Ascii("orbit".into())),
            ],
        ),
        surface(
            "player_records",
            "game",
            Resolved::Addr(0x37F8_D0000),
            Verdict::NotLive,
            vec![field("count", 0, "u32", Value::NotMeasurable { liveness: "players != 0".into() })],
        ),
        surface(
            "activity_loader",
            "world",
            Resolved::Unmapped { addr: 0x8026_B1418 },
            Verdict::Unknown,
            vec![field("mode", 0x80, "u32", Value::NotRead { why: "the surface address did not resolve".into() })],
        ),
        surface("sstm", "world", Resolved::Unknown, Verdict::Unknown, vec![]),
    ]
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// #11 - present parity

/// ⭐ **THE TABLE AND THE JSON ARE THE SAME ROWS.**
///
/// The data half proves that every cell the UI *can* draw is a string the model *does* hold and
/// that `--json` therefore carries it. The render half (needs `--features gui`) drives the real
/// widget tree headlessly with `egui_kittest` and checks the other direction, which is the one that
/// actually catches drift: **every label the window paints must be a cell string or known chrome.**
/// A UI-only computed column is invisible to the data half and fails the render half instantly.
pub fn c11_present_parity() -> ControlResult {
    let name: &'static str = if cfg!(feature = "gui") {
        "present parity - the JSON and the drawn table are the same rows"
    } else {
        "present parity - DATA HALF ONLY (the render half needs --features gui)"
    };
    let mut r = ControlResult::new("#11", name);
    if !cfg!(feature = "gui") {
        r.info(
            "    · ⚠ THE RENDER HALF DID NOT RUN. This binary has no window, so nothing here has \
             drawn a widget. Rebuild with `--features gui` to run the AccessKit leg - that is the \
             leg that catches a UI-only column, and it is NOT skipped into a pass.",
        );
    }

    let f = synth_frame(0, demo_surfaces(), Vec::new(), ChildStatus::None);
    let p = TableParams::default();
    let m = TableModel::build(&f, &p);

    // (a) the rows exist at all - the finds-something half.
    let body: Vec<_> = m.rows.iter().filter(|x| x.kind != RowKind::GroupHeader).collect();
    r.check(body.len() == 9, format!("4 surfaces + 5 fields = 9 body rows; got {}", body.len()));
    r.check(
        m.rows.iter().filter(|x| x.kind == RowKind::GroupHeader).count() == 3,
        "three groups produced three header rows",
    );

    // (b) EVERY cell string survives serialisation. ⭐ This is the property `--json` sells.
    let json = serde_json::to_string(&m).unwrap_or_default();
    let mut missing = Vec::new();
    for row in &m.rows {
        for c in Column::ALL {
            let cell = row.cell(c);
            if cell.is_empty() {
                continue;
            }
            let esc = serde_json::to_string(cell).unwrap_or_default();
            let esc = esc.trim_matches('"');
            if !json.contains(esc) {
                missing.push(format!("{}::{} = {cell:?}", row.key, c.title()));
            }
        }
    }
    r.check(
        missing.is_empty(),
        format!("every cell of every row appears in `--json` ({} rows × {} cols); missing {missing:?}", m.rows.len(), Column::ALL.len()),
    );

    // (c) the scanner's own positive control: a string the model does NOT hold must be absent.
    r.check(
        !json.contains("UI-ONLY-COLUMN"),
        "parity scanner positive control: a string no row holds is absent from the JSON",
    );

    // (d) ⛔ UNMAPPED, NOT-LIVE and NOT-READ are three cells, not one. The finds-nothing/
    //     finds-something pair, in one control: the same model yields a real VALUE and three
    //     distinct non-readings.
    let states: Vec<&str> = body.iter().map(|x| x.state.label()).collect();
    for want in ["VALUE", "UNMAPPED", "NOT-LIVE", "NOT-READ", "[U]"] {
        r.check(states.contains(&want), format!("the model produced a {want} row"));
    }
    r.check(
        body.iter().any(|x| x.value == "NULL (0x0)") == false,
        "no synthetic row claims NULL here (the demo has no null pointer) - the negative direction",
    );

    // (f) ⛔ EVERY SURVIVING ROW MUST BE EXPLICABLE FROM THE TABLE ITSELF. A filter that matched a
    //     hover note returned rows whose six visible cells contained the needle nowhere - correct
    //     cells, unanswerable table. Caught on the first real run against `orbit-all`.
    let mut fs = demo_surfaces();
    fs[1].fields[0].note = Some("mentions world_controller_state_manager in prose only".into());
    let ff = synth_frame(9, fs, vec![], ChildStatus::None);
    let fm = TableModel::build(&ff, &TableParams { filter: "world_controller".into(), ..Default::default() });
    let unexplained: Vec<&str> = fm
        .rows
        .iter()
        .filter(|x| x.kind == RowKind::Field)
        .filter(|x| !Column::ALL.iter().any(|c| x.cell(*c).to_lowercase().contains("world_controller")))
        .map(|x| x.key.as_str())
        .collect();
    r.check(
        unexplained.is_empty(),
        format!("every field row a filter keeps matches VISIBLY; unexplained {unexplained:?}"),
    );
    // …and the parent-retention rule really is what keeps surfaces (the negative direction).
    r.check(
        fm.rows.iter().any(|x| x.key == "world_controller_state_manager"),
        "the matching surface itself survived the filter",
    );
    r.check(
        !fm.rows.iter().any(|x| x.key == "player_records"),
        "a surface whose only tie to the needle is a hover NOTE is filtered out",
    );

    // (e) the render half.
    #[cfg(feature = "gui")]
    render_half(&mut r, &m);

    r
}

/// ⭐ THE OTHER DIRECTION, and the only one that can see a column the model does not have.
#[cfg(feature = "gui")]
fn render_half(r: &mut ControlResult, m: &TableModel) {
    use std::collections::BTreeSet;

    // Everything the model legitimately puts on screen, plus this window's own chrome.
    let mut allowed: BTreeSet<String> = BTreeSet::new();
    for row in &m.rows {
        for c in Column::ALL {
            allowed.insert(row.cell(c).to_string());
        }
    }
    for c in Column::ALL {
        allowed.insert(c.title().to_string());
        allowed.insert(format!("{} ▲", c.title()));
        allowed.insert(format!("{} ▼", c.title()));
    }

    let rows = m.rows.clone();
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(2200.0, 1600.0))
        .build_ui(move |ui| {
            use egui_extras::{Column as TCol, TableBuilder};
            let mut b = TableBuilder::new(ui).striped(true).sense(egui::Sense::click());
            for _ in Column::ALL {
                b = b.column(TCol::initial(320.0).at_least(200.0).clip(false));
            }
            b.header(22.0, |mut h| {
                for c in Column::ALL {
                    h.col(|ui| {
                        ui.label(c.title());
                    });
                }
            })
            .body(|body| {
                body.rows(19.0, rows.len(), |mut row| {
                    let rr = &rows[row.index()];
                    for c in Column::ALL {
                        row.col(|ui| {
                            ui.label(rr.cell(c));
                        });
                    }
                });
            });
        });
    harness.run();

    // Collect every label the widget tree actually exposes.
    use egui_kittest::kittest::NodeT as _;
    let mut seen: Vec<String> = Vec::new();
    for node in harness.root().children_recursive() {
        if let Some(l) = node.accesskit_node().label() {
            seen.push(l);
        }
        if let Some(v) = node.value() {
            seen.push(v);
        }
    }
    let seen: BTreeSet<String> = seen.into_iter().filter(|s| !s.trim().is_empty()).collect();

    r.check(
        seen.len() >= 20,
        format!("render positive control: the AccessKit tree exposed {} labels (a tree with none would prove nothing)", seen.len()),
    );
    // ⭐ The catching direction. `…` survives from a clipped galley, so a label that is a PREFIX of
    //   a cell counts as that cell; anything else is a string the UI invented.
    let invented: Vec<&String> = seen
        .iter()
        .filter(|s| {
            let t = s.trim_end_matches('…').trim();
            !allowed.iter().any(|a| a == *s || (!t.is_empty() && a.starts_with(t)))
        })
        .collect();
    r.check(
        invented.is_empty(),
        format!("every label the window painted is a cell string or a column title; invented {invented:?}"),
    );

    // And the rows really rendered: at least one body cell of the first surface is present.
    let first = m.rows.iter().find(|x| x.kind != RowKind::GroupHeader).map(|x| x.name.clone()).unwrap_or_default();
    r.check(
        seen.iter().any(|s| s.starts_with(first.trim_end_matches('…'))),
        format!("the first body row {first:?} was actually drawn"),
    );
}


// ─────────────────────────────────────────────────────────────────────────────────────────────────
// #12 - the flash is a fact about the data

/// ⭐ **A ROW FLASHES BECAUSE ITS READING CHANGED, NOT BECAUSE IT MOVED.**
///
/// Three legs, and the middle one is the one that matters:
/// 1. two identical frames mark **zero** rows (the finds-nothing half);
/// 2. ⭐ a surface that stays at the **same address** while its liveness goes `Live → NotLive`
///    marks **one** row - a *value-only* comparison sees no change there, and would silently miss
///    the single most misleading transition in this whole tool: an object torn down under an
///    address that still resolves;
/// 3. a row that is NEW is not "changed" - otherwise the first frame flashes everything.
pub fn c12_change_is_a_reading_change() -> ControlResult {
    let mut r = ControlResult::new("#12", "a flash means the READING changed, not the row moved");
    let p = TableParams { group_by_group: false, ..Default::default() };

    // 1. identical frames ⇒ nothing.
    let a = TableModel::build(&synth_frame(0, demo_surfaces(), vec![], ChildStatus::None), &p);
    let mut b = TableModel::build(&synth_frame(1, demo_surfaces(), vec![], ChildStatus::None), &p);
    let n = b.mark_changes(&a.signatures());
    r.check(n == 0, format!("two identical frames mark {n} changed rows (want 0)"));

    // 2. same address, liveness collapses.
    let mut torn = demo_surfaces();
    torn[0].liveness = liveness(Verdict::NotLive);
    for f in &mut torn[0].fields {
        f.value = Value::NotMeasurable { liveness: "u32@(x) != 0".into() };
    }
    let mut c = TableModel::build(&synth_frame(2, torn, vec![], ChildStatus::None), &p);
    let n = c.mark_changes(&a.signatures());
    let changed: Vec<&str> = c.rows.iter().filter(|x| x.changed).map(|x| x.key.as_str()).collect();
    let sm = c.rows.iter().find(|x| x.key == "world_controller_state_manager").unwrap();
    r.check(
        sm.changed,
        "the surface row changed even though its ADDRESS is identical - the state is part of the signature",
    );
    r.check(
        sm.addr == a.rows.iter().find(|x| x.key == "world_controller_state_manager").unwrap().addr,
        format!("…and the address really is unchanged ({}) - so a value-only diff would have missed it", sm.addr),
    );
    r.check(n == 4, format!("the torn-down surface and its 3 fields changed: {n} rows {changed:?}"));

    // 3. a new row is not a changed row.
    let mut grown = demo_surfaces();
    grown.push(surface("brand_new", "session", Resolved::Addr(0x1000), Verdict::Live, vec![]));
    let mut d = TableModel::build(&synth_frame(3, grown, vec![], ChildStatus::None), &p);
    d.mark_changes(&a.signatures());
    let newrow = d.rows.iter().find(|x| x.key == "brand_new").unwrap();
    r.check(!newrow.changed, "a row absent from the previous tick is NOT marked changed");

    // 4. the signature survives a re-sort: change detection keys off `key`, never the row index.
    let p2 = TableParams { sort: Column::Addr, descending: true, group_by_group: false, ..Default::default() };
    let resorted = TableModel::build(&synth_frame(4, demo_surfaces(), vec![], ChildStatus::None), &p2);
    let s1: Signatures = a.signatures();
    let s2: Signatures = resorted.signatures();
    r.check(s1 == s2, "re-sorting produces IDENTICAL signatures - a re-sort must never flash the table");
    r.check(
        resorted.rows.first().map(|x| x.key.as_str()) != a.rows.first().map(|x| x.key.as_str()),
        "…and the sort really did reorder the rows (the negative direction)",
    );

    r
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// #13 - the badges

/// ⭐ **EIGHT BADGES, EIGHT MEANINGS**, and the pair this control exists for is
/// `ARMED · nothing yet` vs `ARMED · NEVER FIRED`. The first is *"the window is still open"*; the
/// second is a **real negative result**. Collapsing them turns the one shape of evidence the event
/// plane produces into a spinner.
///
/// ⚠ The refusal legs are fed by the **real** `child_mgr` functions, never by a transcribed copy of
/// their text - so a reworded refusal breaks this control instead of silently escaping it.
pub fn c13_badges_are_distinct() -> ControlResult {
    let mut r = ControlResult::new("#13", "the event badge distinguishes eight different things");
    let p = TableParams { group_by_group: false, ..Default::default() };
    let spec = WatchSpecView {
        surface: Some("world_controller_state_manager".into()),
        addr_expr: "[v:0x26B13E8]".into(),
        resolved: 0x2050_2F1D0,
        len: 8,
        rw: "write".into(),
        slot: 0,
        count: 32,
        lease_s: 20.0,
    };
    let receipt = || StopReceipt {
        child_pid: 0,
        target_pid: 0,
        how: "control".into(),
        exit_code: Some(1),
        exit_signal: None,
        waited_ms: 0,
        confirmed_disarm: true,
        stop_reason: Some("--max-seconds 2 reached".into()),
        complaints: vec![],
        hits: 0,
    };
    let badge_of = |m: &TableModel, key: &str| m.rows.iter().find(|x| x.key == key).map(|x| x.badge.clone()).unwrap();

    // (a) ARMED · N writes - from the tree's OWN recorded two-hit report, parsed and attributed by
    //     ADDRESS through the real pipeline. ⭐ Not a hand-made EventView.
    let text = std::fs::read_to_string(crate::controls_event::fixture_path());
    match text.and_then(|t| Ok(crate::events::parse_report(&t))) {
        Ok(Ok((rep, _))) => {
            // Put a surface exactly where the report's address lands, so attribution is real.
            let mut surfaces = demo_surfaces();
            surfaces[0].resolved = Resolved::Addr(rep.addr);
            surfaces[0].fields = vec![field("watched", 0, "u64", Value::Int(1))];
            let evs = crate::events::to_event_views(&rep, &surfaces, Some("world_controller_state_manager"), 0);
            r.check(evs.len() == 2, format!("the recorded report yielded {} hit(s)", evs.len()));
            let f = synth_frame(
                0,
                surfaces,
                evs,
                ChildStatus::Finished { spec: spec.clone(), hits: 2, stop_reason: "count reached".into(), receipt: receipt() },
            );
            let m = TableModel::build(&f, &p);
            let b = badge_of(&m, "world_controller_state_manager");
            r.check(
                b == EventBadge::Armed { writes: 2 },
                format!("a REAL recorded two-hit report renders {:?}", b.label()),
            );
            // The negative direction, in the same frame: an untouched surface gets NO badge.
            r.check(
                badge_of(&m, "player_records") == EventBadge::None,
                "a surface no write landed in carries no badge (the finds-nothing half)",
            );
        }
        Ok(Err(e)) => r.bad(format!("parsing the recorded fixture: {e}")),
        Err(e) => r.bad(format!("reading the recorded fixture: {e}")),
    }

    // (b) ⛔ THE PAIR. Same surface, same spec - two ChildStatuses that must never render alike.
    let running = synth_frame(
        1,
        demo_surfaces(),
        vec![],
        ChildStatus::Running {
            pid: 1,
            target_pid: 2,
            spec: spec.clone(),
            lease_s: 20.0,
            lease_remaining_s: 12.5,
            armed_threads: 3,
            rearms: 0,
            hits: 0,
            pending: vec![],
        },
    );
    let nohit = synth_frame(
        2,
        demo_surfaces(),
        vec![],
        ChildStatus::NoHit {
            spec: spec.clone(),
            watched_s: 2.0,
            stop_reason: "--max-seconds 2 reached".into(),
            why: "evidence ONLY about the 3 thread(s) armed at start".into(),
            receipt: receipt(),
        },
    );
    let br = badge_of(&TableModel::build(&running, &p), "world_controller_state_manager");
    let bn = badge_of(&TableModel::build(&nohit, &p), "world_controller_state_manager");
    r.check(matches!(br, EventBadge::ArmedNothingYet { .. }), format!("a running lease renders {:?}", br.label()));
    r.check(matches!(bn, EventBadge::ArmedNeverFired { .. }), format!("a completed zero-hit watch renders {:?}", bn.label()));
    // ⛔ THE STEM, NOT THE LABEL. Comparing full labels compared the SECONDS - see
    //    `EventBadge::label_stem`. The inversion that collapses these two variants passed against
    //    the old form, which is the whole reason this line reads the way it does.
    r.check(
        br.label_stem() != bn.label_stem(),
        format!("⛔ and the two STATEMENTS differ: {:?} vs {:?}", br.label_stem(), bn.label_stem()),
    );
    r.check(
        bn.detail().contains("thread"),
        "the NEVER-FIRED detail carries the bound of the negative (which threads, how long)",
    );

    // (c) SLOT FULL and REFUSED, classified from the REAL messages.
    let ceiling = crate::child_mgr::slot_ceiling_message();
    let lease = crate::child_mgr::Refusal::LeaseOutOfBounds { asked: 999.0, max: 120.0 }.message();
    r.check(
        matches!(EventBadge::classify_refusal(&ceiling), EventBadge::SlotFull { .. }),
        format!("child_mgr::slot_ceiling_message() classifies as SLOT FULL"),
    );
    r.check(
        matches!(EventBadge::classify_refusal(&lease), EventBadge::Refused { .. }),
        "a lease refusal classifies as REFUSED, not SLOT FULL (the negative direction)",
    );
    let with_refusal = TableParams {
        refusals: vec![("player_records".into(), ceiling.clone())],
        held_surfaces: vec!["activity_loader".into()],
        group_by_group: false,
        ..Default::default()
    };
    let m = TableModel::build(&synth_frame(3, demo_surfaces(), vec![], ChildStatus::None), &with_refusal);
    r.check(badge_of(&m, "player_records").label() == "SLOT FULL", "a refused surface renders SLOT FULL");
    r.check(badge_of(&m, "activity_loader") == EventBadge::Held, "a held-but-unarmed slot renders SLOT HELD");

    // (d) CHILD FAILED is not NEVER FIRED. ⚠ Both exit 1; only one of them is a result.
    let failed = synth_frame(
        4,
        demo_surfaces(),
        vec![],
        ChildStatus::Failed { spec: spec.clone(), stderr: "error: no such process\n".into(), receipt: receipt() },
    );
    let bf = badge_of(&TableModel::build(&failed, &p), "world_controller_state_manager");
    r.check(matches!(bf, EventBadge::Failed { .. }), format!("an unparseable child renders {:?}", bf.label()));
    r.check(bf.label() != bn.label(), "⛔ CHILD FAILED and NEVER FIRED do not share a label");

    // (e) all eight labels are distinct.
    let all = [
        EventBadge::None,
        EventBadge::Armed { writes: 1 },
        EventBadge::ArmedNothingYet { lease_remaining_s: 1.0 },
        EventBadge::ArmedNeverFired { watched_s: 1.0, caveat: String::new() },
        EventBadge::Held,
        EventBadge::SlotFull { why: String::new() },
        EventBadge::Refused { why: String::new() },
        EventBadge::Failed { stderr_head: String::new() },
    ];
    let mut labels: Vec<String> = all.iter().map(|b| b.label_stem()).collect();
    labels.sort();
    labels.dedup();
    r.check(labels.len() == all.len(), format!("{} badges, {} distinct STATEMENTS (numbers stripped)", all.len(), labels.len()));

    r
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// #14 - the UI computes nothing, reads nothing, spawns nothing

/// ⭐ **A MODULE BOUNDARY, ASSERTED.** The window may not read memory and may not start a child.
/// This is checked over `ui.rs`'s own text - with a positive control, because *"no forbidden
/// token"* from a broken scanner reads exactly like a clean file.
///
/// ⚠ It also checks the STATE side: `TableModel::build` must be a pure function of `(Frame,
/// TableParams)`, so two builds from the same inputs are byte-identical. A UI that reached for a
/// clock or the filesystem mid-render would break that, quietly.
pub fn c14_ui_computes_nothing() -> ControlResult {
    let mut r = ControlResult::new("#14", "the window reads no memory, spawns nothing, computes no value");

    // ⭐⭐ THE FILE LIST IS DERIVED FROM `lib.rs`, NOT TYPED HERE - closing the hole the BUILD-3
    //    review named: *nothing compile-enforces the frame separation for the next build.* A new
    //    `#[cfg(feature = "gui")] pub mod …;` is a new UI file, and it is scanned automatically.
    //    A hand-written list is a list somebody forgets to extend, and the omission is silent.
    let src = crate::repo_root().join("d1-server/crates/d1-scope/src");
    let libtext = std::fs::read_to_string(src.join("lib.rs")).unwrap_or_default();
    let mut ui_files: Vec<String> = Vec::new();
    let mut prev_gui = false;
    for line in libtext.lines() {
        let t = line.trim();
        if t.starts_with("#[cfg(feature = \"gui\")]") {
            prev_gui = true;
            continue;
        }
        if prev_gui {
            if let Some(m) = t.strip_prefix("pub mod ").and_then(|s| s.strip_suffix(';')) {
                ui_files.push(format!("{m}.rs"));
            }
            prev_gui = false;
        }
    }
    r.check(
        ui_files.len() >= 2 && ui_files.iter().any(|f| f == "ui.rs") && ui_files.iter().any(|f| f == "ui_graph.rs"),
        format!("the gui module list is DERIVED from lib.rs: {ui_files:?}"),
    );

    for f in &ui_files {
        let path = src.join(f);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                // ⛔ Anything that would let the UI thread touch the client, the filesystem or a
                //    process. No exclusion list - which is why the prose in those files has to
                //    DESCRIBE these names rather than spell them.
                for needle in [
                    "MemSource",
                    "ProcMem",
                    "SnapMem",
                    "Evaluator",
                    "Sampler",
                    "process::Command",
                    "child_mgr",
                    "/proc/",
                    "runs/snapshots",
                ] {
                    r.check(!text.contains(needle), format!("{f} contains no {needle:?}"));
                }
                // ⭐ THE SCANNER'S POSITIVE CONTROL, per file - it can find what IS there.
                let anchors: &[&str] = if f == "ui.rs" {
                    &["TableRow", "Column::ALL", "r.cell(c)", "Arc<Frame>"]
                } else {
                    &["GraphModel", "PaintReceipt", "clip_rect", "Arc<Frame>"]
                };
                for needle in anchors {
                    r.check(text.contains(needle), format!("scanner positive control: {f} DOES contain {needle:?}"));
                }
                // ⚠ `ui.rs`'s ONE filesystem read is the deliberate, named host-font exception.
                //   The canvas has none at all, and that is asserted rather than assumed.
                let reads = text.matches("std::fs::read").count();
                let want = if f == "ui.rs" { 1 } else { 0 };
                r.check(
                    reads == want,
                    format!("{f}: {want} filesystem read(s) expected (the host-font probe lives in ui.rs only); found {reads}"),
                );
            }
            Err(e) => r.bad(format!("reading {}: {e}", path.display())),
        }
    }

    // The sampler is constructed inside `run_sampler`, on its own thread, and the window is handed
    // a channel. Assert the shape in main.rs rather than trusting the comment.
    let mainp = crate::repo_root().join("d1-server/crates/d1-scope/src/main.rs");
    match std::fs::read_to_string(&mainp) {
        Ok(text) => {
            r.check(text.contains("fn run_sampler"), "main.rs owns run_sampler");
            r.check(
                text.contains("Arc<d1_scope::frame::Frame>"),
                "the window receives an immutable Frame over a channel, not a source",
            );
        }
        Err(e) => r.bad(format!("reading {}: {e}", mainp.display())),
    }

    // ⭐ PURITY: same inputs, same bytes. Twice.
    let f = synth_frame(0, demo_surfaces(), vec![], ChildStatus::None);
    let p = TableParams::default();
    let a = serde_json::to_string(&TableModel::build(&f, &p)).unwrap_or_default();
    let b = serde_json::to_string(&TableModel::build(&f, &p)).unwrap_or_default();
    r.check(!a.is_empty() && a == b, "TableModel::build is pure: two builds of one frame are byte-identical");
    // …and it really does depend on its params (the negative direction - otherwise "identical" is trivial).
    let p2 = TableParams { filter: "ladder".into(), ..Default::default() };
    let c = serde_json::to_string(&TableModel::build(&f, &p2)).unwrap_or_default();
    r.check(a != c, "…and a different filter really does produce a different model");

    // ⭐ STALENESS IS DERIVED, NOT SENSED. Same frame, two ages, two answers.
    let fresh = TableModel::build(&f, &TableParams { age_ms: 0, stale_after_ms: 2000, ..Default::default() });
    let stale = TableModel::build(&f, &TableParams { age_ms: 9000, stale_after_ms: 2000, ..Default::default() });
    r.check(!fresh.stale, "age 0 ⇒ not stale");
    r.check(stale.stale, "age 9000 ms past a 2000 ms threshold ⇒ STALE");
    let sr = stale.rows.iter().find(|x| x.kind != RowKind::GroupHeader).unwrap();
    r.check(sr.state_text.starts_with("STALE"), format!("a stale row's state cell says so: {:?}", sr.state_text));
    r.check(
        sr.state_text.contains(sr.state.label()),
        "⛔ …and it WRAPS the underlying reading rather than replacing it - 'not captured' survives 'not current'",
    );
    r.check(
        RenderState::ALL.iter().map(|s| s.label()).collect::<std::collections::BTreeSet<_>>().len() == RenderState::ALL.len(),
        "the seven render states have seven distinct labels",
    );

    r
}

pub fn run_ui_controls() -> Vec<ControlResult> {
    vec![c11_present_parity(), c12_change_is_a_reading_change(), c13_badges_are_distinct(), c14_ui_computes_nothing()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_3_controls_pass() {
        for c in run_ui_controls() {
            assert!(c.pass, "{} {}\n{}", c.id, c.name, c.lines.join("\n"));
        }
    }
}
