// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! **The positive controls, as tests.**
//!
//! Every scan in this crate can return nothing, and *"a scan with no positive control is not
//! evidence of absence"* (`historicaldocs/design/ATLAS.md` §4.2, cost: two sessions). These tests are where each
//! control lives, so a regression in a reader is a red build rather than a quiet wrong finding.
//!
//! They also satisfy ATLAS §4.4d - **a new tool is not evidence until it reproduces a KNOWN
//! result** - by pinning outputs that the *Python* tools produced before the port
//! (`tests/fixtures/`).
//!
//! Every test skips (rather than fails) when the pinned eboot is absent, so the workspace still
//! builds on a machine without the dumps.

use std::path::Path;

use d1_eboot::descriptors::{self, LEAF, SENTINEL};
use d1_eboot::disasm::{self, Anchor};
use d1_eboot::fieldxref;
use d1_eboot::functions;
use d1_eboot::image::{Image, ImageError, PINNED_EBOOT, PINNED_EBOOT_LEN, PINNED_EBOOT_SHA256};
use d1_eboot::reloc;
use d1_eboot::rtti;
use d1_eboot::xref::{self, Control, RefKind};

fn image() -> Option<Image> {
    if !Path::new(PINNED_EBOOT).exists() {
        eprintln!("SKIP: pinned eboot not present at {PINNED_EBOOT}");
        return None;
    }
    Some(Image::open(PINNED_EBOOT).expect("open pinned eboot"))
}

macro_rules! img {
    () => {
        match image() {
            Some(i) => i,
            None => return,
        }
    };
}

// ───────────────────────── image / address map ─────────────────────────

#[test]
fn build_identity_is_the_pinned_0129_eboot() {
    let img = img!();
    assert_eq!(img.data().len(), PINNED_EBOOT_LEN);
    assert_eq!(img.sha256(), PINNED_EBOOT_SHA256, "this is not the pinned build");
}

/// ⚠ ATLAS §4.1 - **the address bias is per-segment.** `0x4000` is the TEXT case. This pins the
/// six measured deltas so that "unifying the bias into one constant" fails the build instead of
/// silently corrupting every relocation read.
#[test]
fn address_bias_is_per_segment() {
    let img = img!();
    let deltas: Vec<(u32, i64)> = img.segments().iter().map(|s| (s.p_type, s.bias())).collect();
    // text and data LOAD both happen to be 0x4000 …
    let loads: Vec<i64> = img.load_segments().map(|s| s.bias()).collect();
    assert_eq!(loads, vec![0x4000, 0x4000]);
    // … and the metadata segments are NOT.
    let dynamic = img.segment_of_type(d1_eboot::image::PT_DYNAMIC).unwrap();
    let dynlib = img.segment_of_type(d1_eboot::image::PT_SCE_DYNLIBDATA).unwrap();
    assert_eq!(dynamic.vaddr, 0, "PT_DYNAMIC is addressed by raw file offset");
    assert_eq!(dynamic.bias(), 0x1bca750);
    assert_eq!(dynlib.vaddr, 0, "PT_SCE_DYNLIBDATA is addressed by raw file offset");
    assert_eq!(dynlib.bias(), 0x1a0d100);
    assert!(
        deltas.iter().filter(|(_, d)| *d != 0x4000).count() >= 4,
        "at least four segments have a bias that is not 0x4000"
    );
}

/// BSS is a legitimate runtime address that static reading cannot serve. `0x263D0F8` is the
/// `profile_setup` config object ATLAS names; a reader that returns a plausible byte for it is
/// lying. Positive control in the same test: a text address still resolves.
#[test]
fn bss_is_refused_not_guessed() {
    let img = img!();
    assert!(matches!(img.file_offset(0x263D0F8), Err(ImageError::Bss { .. })));
    assert_eq!(img.file_offset(0x56FFB0).unwrap(), 0x56FFB0 + 0x4000); // positive control
    assert!(matches!(img.file_offset(0xFFFF_FFFF_0000), Err(ImageError::Unmapped { .. })));
}

// ───────────────────────── relocations ─────────────────────────

/// `oodle/reloc-xref.py` hardcodes `RELA_OFF = 0x1A0D100 + 0xBDD0`, `RELA_SZ = 0x1B0918`. Here
/// those are **derived from `PT_DYNAMIC`**, and the historical constants survive only as this
/// control - ATLAS §4.3: *re-derive a constant rather than trusting a document's copy of it.*
#[test]
fn rela_tables_are_derived_and_match_the_historical_constants() {
    let img = img!();
    let t = reloc::tables(&img).unwrap();
    assert_eq!(t.rela_file_offset, 0x1A0D100 + 0xBDD0);
    assert_eq!(t.rela_size, 0x1B0918);
    assert_eq!(t.rela_entsize, 24);
    assert_eq!(t.jmprel_file_offset, 0x1A0D100 + 0x8200);
    assert_eq!(t.jmprel_size, 0x3BD0);
    assert_eq!(reloc::relative(&img).unwrap().len(), 70_682);
}

/// Reproduces `reloc-xref.py 0x25a330 0x261fb0` and `--slot 0x1924ec8 --span 0x40` exactly.
/// The `0x261fb0` half is the point: the RSAT **decoder** is pointed at by nothing in `.rela`,
/// and saying so is only meaningful next to a lookup that does find something.
#[test]
fn reloc_addend_lookup_reproduces_python() {
    let img = img!();
    let m = reloc::by_addend(&img).unwrap();
    assert_eq!(m.get(&0x25A330).cloned().unwrap_or_default(), vec![0x18F5260]);
    assert!(m.get(&0x261FB0).is_none(), "decoder 0x261fb0 has no RELATIVE slot");

    let slots = reloc::by_slot(&img).unwrap();
    let walk: Vec<u64> = (0x1924EC8u64..0x1924F08)
        .step_by(8)
        .map(|v| slots.get(&v).copied().unwrap_or(0))
        .collect();
    assert_eq!(
        walk,
        vec![0x560CF0, 0x0, 0x15B2A20, 0x1924E40, 0x0, 0x0, 0x1924F60, 0x560AE0],
        "vtable region at 0x1924ec8 - the policy virtual +0x58 chain (§26.10)"
    );
}

// ───────────────────────── cross-references ─────────────────────────

/// ⭐ **THE TWO-SESSION BUG, kept reproducible.** ATLAS §4.2: §26.9 concluded "the producer cannot
/// be found by reading the binary" because the scan checked only `call rel32`. `0x56FFB0` is
/// entered by a **tail call**. Both halves are asserted, because only the pair is the lesson.
#[test]
fn tail_call_control_0x560d27_to_0x56ffb0() {
    let img = img!();

    // The old, call-only shape returns NOTHING - this is the false absence.
    let calls = xref::scan_kind(&img, 0x56FFB0, RefKind::Call).unwrap();
    assert_eq!(calls.iter().filter(|h| h.confirmed).count(), 0);

    // The four-kind scan finds it.
    let jmps = xref::scan_kind(&img, 0x56FFB0, RefKind::Jmp).unwrap();
    assert!(
        jmps.iter().any(|h| h.confirmed && h.site == 0x560D27),
        "mandatory positive control 0x560d27 -> 0x56ffb0 (ATLAS §4.2) not recovered"
    );
    assert_eq!(Control::pinned(RefKind::Jmp).unwrap().site, 0x560D27);
}

/// ⚠ `capture-rig/find-direct-call-xrefs.py`'s own docstring advertises the control
/// `--control 0x571940:0x5719f2`. **It fails**: the call at `0x5719f2` targets `0x5858c0`.
/// Pinned here so the correction cannot be lost, and so nobody "fixes" it by editing a constant
/// (ATLAS §6.1: a wrong pinned control is worse than none).
#[test]
fn call_control_the_python_docstring_advertised_is_wrong() {
    let img = img!();
    let bad = xref::scan_kind(&img, 0x571940, RefKind::Call).unwrap();
    assert!(!bad.iter().any(|h| h.confirmed && h.site == 0x5719F2), "the advertised control is wrong");

    let good = xref::scan_kind(&img, 0x5858C0, RefKind::Call).unwrap();
    assert!(good.iter().any(|h| h.confirmed && h.site == 0x5719F2), "0x5719f2 calls 0x5858c0");
    assert_eq!(Control::pinned(RefKind::Call).unwrap(), Control {
        kind: RefKind::Call,
        target: 0x5858C0,
        site: 0x5719F2
    });
}

/// Reproduces `oodle/global-xref.py 0x26a3888 --control-pair 0x164ce3e:0x7982da`: 52 confirmed
/// RIP-relative sites, and the independent control recovered through the identical code path.
#[test]
fn riprel_scan_reproduces_global_xref_py() {
    let img = img!();
    let hits = xref::scan_kind(&img, 0x26A3888, RefKind::RipRel).unwrap();
    assert_eq!(hits.iter().filter(|h| h.confirmed).count(), 52);
    // the character-signin gate `0x78C5F0` and `0x78C9F0` are both in the set (§26.75)
    for site in [0x78C619u64, 0x78CA27] {
        assert!(hits.iter().any(|h| h.confirmed && h.site == site), "missing {site:#x}");
    }
    let ctl = xref::scan_kind(&img, 0x164CE3E, RefKind::RipRel).unwrap();
    assert!(ctl.iter().any(|h| h.confirmed && h.site == 0x7982DA), "independent control");
}

/// ⚠ ATLAS §4.2b - **a passing control does not mean you asked about the right address.**
/// `0x162E2C0` returned 0 references with a passing control and the conclusion drawn was wrong:
/// the literal starts at `0x162E2B5` and `0x162E2C0` pointed into its interior.
#[test]
fn aim_check_catches_a_string_interior_target() {
    let img = img!();
    let report = xref::scan(&img, 0x162E2C0, &[RefKind::RipRel], &[Control::pinned(RefKind::RipRel).unwrap()])
        .unwrap();
    assert_eq!(report.confirmed().count(), 0);
    assert!(report.is_evidence, "the instrument works …");
    assert_eq!(report.string_start_warning, Some(0x162E2B5), "… but the AIM was wrong");

    // At the true start there are references, and they are in `queuez_family_update_begin`.
    let real = xref::scan_kind(&img, 0x162E2B5, RefKind::RipRel).unwrap();
    let confirmed: Vec<_> = real.iter().filter(|h| h.confirmed).collect();
    assert_eq!(confirmed.len(), 2);
    assert!(confirmed.iter().all(|h| h.function == Some(0x56AF60)));
}

/// A report with no control must never be citable as absence.
#[test]
fn a_scan_without_a_control_is_not_evidence() {
    let img = img!();
    let none = xref::scan(&img, 0xDEAD_BEEF, &[RefKind::Call], &[]).unwrap();
    assert!(!none.is_evidence);
    let with = xref::scan(&img, 0xDEAD_BEEF, &[RefKind::Call], &[Control::pinned(RefKind::Call).unwrap()])
        .unwrap();
    assert!(with.is_evidence);
    assert_eq!(with.confirmed().count(), 0, "a nonsense target really has no references");
}

// ───────────────────────── function attribution (the `fn` column) ─────────────────────────
//
// ⛔⛔ **THE MIS-ATTRIBUTION THAT PUBLISHED A WRONG VERDICT** (fixed 2026-08-11).
// `xref`'s `fn` column was `image.nearest_prologue(site)`. 11,446 of this image's functions are
// frame-pointer-omitted, so every site inside one was credited to the PRECEDING prologue-anchored
// function. `xref 0x25c0c58` printed `fn 0x1d1e80` for a site at `0x1d20a1`; `FUN_001D1E80` ends
// at `0x1D2020` and the site is inside `0x1D2030`.
//
// `0x1D1E80` **writes** that pool and `0x1D2030` **reads** it. With the reads mis-filed under the
// writer, the pool looked write-only and a session concluded the function it dispatches to was
// unreachable dead code, and published that. The chain was live.
// ⇒ `status/LESSONS.md`, `design/activity-lifetime-state.md`.
//
// Each test below carries its own INVERTED half - the OLD mechanism, still callable, returning the
// wrong answer in the same window - so none of these can pass by being vacuous.

/// ⭐ **THE REGRESSION, PINNED.** Both referring instructions must attribute to `0x1D2030`.
#[test]
fn xref_fn_column_attributes_an_fpo_body_to_its_own_function() {
    let img = img!();

    for (target, site) in [(0x25C_0C58u64, 0x1D2_0A1u64), (0x25C_0C70, 0x1D2_087)] {
        let hits = xref::scan_kind(&img, target, RefKind::RipRel).unwrap();
        let h = hits
            .iter()
            .find(|h| h.confirmed && h.site == site)
            .unwrap_or_else(|| panic!("{target:#x}: no confirmed reference at {site:#x}"));
        assert_eq!(
            h.function,
            Some(0x1D2_030),
            "{site:#x} is inside the FPO lookup 0x1d2030, not its neighbour"
        );
        assert_ne!(h.function, Some(0x1D1_E80), "0x1d1e80 is the WRITER - the whole defect");
        assert_eq!(h.containing.how, functions::Containment::Fpo);
        assert_eq!(h.containing.label(), "fn 0x01d2030");
    }

    // ── INVERTED: the mechanism that was wrong, still wrong, in this same window ──────────────
    // If this ever stops holding, the assertions above have stopped discriminating and are
    // decoration. (CLAUDE.md: *invert every control.*)
    for site in [0x1D2_0A1u64, 0x1D2_087] {
        assert_eq!(
            img.nearest_prologue(site, 0x4000),
            Some(0x1D1_E80),
            "the OLD `fn` column really did name 0x1d1e80 for {site:#x}"
        );
    }

    // And the extents that make it wrong, read from the index rather than recalled.
    let ix = img.functions();
    let writer = ix.funcs.iter().find(|f| f.start == 0x1D1_E80).expect("0x1d1e80");
    let reader = ix.funcs.iter().find(|f| f.start == 0x1D2_030).expect("0x1d2030");
    assert_eq!((writer.anchored, writer.end), (functions::Anchored::Prologue, 0x1D2_020));
    assert_eq!((reader.anchored, reader.end), (functions::Anchored::CallTarget, 0x1D2_150));
    assert!(0x1D2_087 >= writer.end && 0x1D2_087 < reader.end);
}

/// The positive control **in the same instrument**: an ordinary prologue-anchored attribution is
/// unchanged. `0x162E2B5`'s two references live in `queuez_family_update_begin` (`0x56AF60`) - the
/// same pair `aim_check_catches_a_string_interior_target` has always asserted, so a fix that broke
/// normal attribution to make the FPO case pass would fail here.
#[test]
fn function_attribution_still_names_a_prologue_anchored_function() {
    let img = img!();
    let hits = xref::scan_kind(&img, 0x162_E2B5, RefKind::RipRel).unwrap();
    let confirmed: Vec<_> = hits.iter().filter(|h| h.confirmed).collect();
    assert_eq!(confirmed.len(), 2);
    assert!(confirmed.iter().all(|h| h.function == Some(0x56A_F60)));
    assert!(confirmed.iter().all(|h| h.containing.how == functions::Containment::Prologue));

    // The call control too, through the same path: 0x5719f2 is inside 0x571940.
    let call = xref::scan_kind(&img, 0x585_8C0, RefKind::Call).unwrap();
    let c = call.iter().find(|h| h.confirmed && h.site == 0x571_9F2).unwrap();
    assert_eq!(c.function, Some(0x571_940));
    assert_eq!(c.containing.how, functions::Containment::Prologue);
}

/// ⭐ **A wrong name is worse than an honest unknown.** An address no function contains must come
/// back as `Containment::None` / `fn -`, never as the nearest preceding function.
#[test]
fn function_attribution_says_nothing_rather_than_name_a_neighbour() {
    let img = img!();

    // (a) a DATA address, 19 MB past the end of the text segment.
    let data = img.containing_function(0x25C_0C58);
    assert_eq!(data.how, functions::Containment::None);
    assert_eq!(data.start, None);
    assert_eq!(data.label(), "fn -");

    // (b) before the first function start (the image's first function is at 0x20).
    let low = img.containing_function(0x10);
    assert_eq!(low.how, functions::Containment::None);
    assert_eq!(low.label(), "fn -");

    // ── INVERTED: the shortcut that does not check an end still names a function for both ─────
    // `Index::function_of` is "greatest start at or below", and `atlas --addr 0x25c0c58` really did
    // answer `function 0x129f850 ..0x129f868` - a function that does not contain the address.
    let starts = img.functions().starts();
    assert_eq!(
        functions::Index::function_of(&starts, 0x25C_0C58),
        Some(0x129_F850),
        "the end-blind shortcut still names a neighbour - so (a) is a real discrimination"
    );
    assert_eq!(starts.first().copied(), Some(0x20), "the first function start is pinned at 0x20");
}

/// The two readers must not drift: whatever `atlas --addr` reports as the containing function is
/// what `xref` files a site under. One index, one answer (this is why it lives on `Image`).
#[test]
fn atlas_and_xref_agree_on_the_containing_function() {
    let img = img!();
    let ix = img.functions();
    for (site, want) in [
        (0x1D2_087u64, 0x1D2_030u64), // FPO lookup
        (0x1D2_0A1, 0x1D2_030),
        (0x1D1_F00, 0x1D1_E80), // prologue-anchored writer
        (0x1D2_027, 0x1D2_020), // the 16-byte FPO tail-call thunk between them
        (0x571_9F2, 0x571_940),
    ] {
        let c = ix.containing(site);
        assert_eq!(c.start, Some(want), "{site:#x}");
        assert!(c.how.proven(), "{site:#x} attribution must be proven, got {:?}", c.how);
        assert_eq!(c.proven_start(), Some(want));
    }
}

// ───────────────────────── disassembly ─────────────────────────

/// ATLAS §4.5: *"twice this project has disassembled from a guessed address and produced
/// authoritative-looking garbage."* The default must be refusal.
#[test]
fn disassembly_refuses_a_guessed_start() {
    let img = img!();
    assert!(matches!(
        disasm::disasm_range(&img, 0x5719E8, 0x571A00, Anchor::Prologue),
        Err(ImageError::NotAPrologue { .. })
    ));
    // Snap finds the real function start …
    let d = disasm::disasm_range(&img, 0x5719E8, 0x571A00, Anchor::Snap).unwrap();
    assert_eq!(d.anchor, Some(0x571940));
    assert_eq!(d.insns[0].mnemonic, "push");
    // … and a real prologue is accepted.
    let p = disasm::disasm_range(&img, 0x571940, 0x571960, Anchor::Prologue).unwrap();
    assert_eq!(p.anchor, Some(0x571940));
}

/// The instruction covering a site is found by anchoring, never by decoding from `site - N`.
#[test]
fn prologue_anchored_confirmation_resolves_the_call_and_its_target() {
    let img = img!();
    let (containing, insn) = disasm::instruction_at(&img, 0x5719F2).unwrap().unwrap();
    assert_eq!(containing.start, Some(0x571940));
    assert_eq!(containing.how, functions::Containment::Prologue);
    assert_eq!(insn.address, 0x5719F2);
    assert_eq!(insn.mnemonic, "call");
    assert_eq!(insn.branch_target, Some(0x5858C0));

    // RIP-relative operands resolve to an absolute vaddr (`oodle/dis-annot.py`'s job).
    let (_, mov) = disasm::instruction_at(&img, 0x5719F7).unwrap().unwrap();
    assert_eq!(mov.rip_target, Some(0x1A06C78));
}

/// ⚠ Equivalence finding, recorded as a test: `capture-rig/disassemble-elf-range.py` prints
/// `warning: disassembly stopped after 0x16 of 0x18 bytes` and exits 1 on this range, because it
/// hands capstone exactly `blob[first:last]` and the last instruction straddles `end`. That looks
/// exactly like the data-island truncation the warning exists to report. Here `truncated_at` is
/// `None` for a clean range and `Some` only when decoding really stopped.
#[test]
fn truncation_is_reported_only_when_it_really_happened() {
    let img = img!();
    let clean = disasm::disasm_range(&img, 0x5719E8, 0x571A00, Anchor::Forced).unwrap();
    assert!(clean.truncated_at.is_none(), "straddling the end is not truncation");
    assert_eq!(clean.insns.last().unwrap().address, 0x5719FE);
}

// ───────────────────────── descriptors ─────────────────────────

/// 4,555 anchored descriptors - a pinned count that ships with its extractor (ATLAS §6.1).
/// Presence and absence controls in the same test (§4.2).
#[test]
fn descriptor_index_count_and_presence_absence_controls() {
    let img = img!();
    let idx = descriptors::index(&img).unwrap();
    assert_eq!(idx.len(), 4555);
    for tag in [0x80801825u32, 0x80801803, 0x80801954, 0x80801796, 0x80801845, 0x8080194D] {
        assert!(idx.get(tag).is_some(), "{tag:#x} must be present");
    }
    // absence control: message-catalogue.py's known-absent tag
    assert!(idx.get(0x808033F4).is_none(), "0x808033F4 is known to be absent from this image");
}

/// ⚠ Defect 1 - **the C-object offset is at `marker-8`, not `marker+0x20`.**
/// The `0x80801825` fit check is the proof: last field `@+0x18`, span 944, `0x18 + 944 == 968`,
/// exactly the declared struct size. Reading `marker+0x20` gives a different number entirely.
#[test]
fn defect_1_c_object_offset_is_at_marker_minus_8() {
    let img = img!();
    let idx = descriptors::index(&img).unwrap();
    let d = idx.get(0x80801825).unwrap();
    assert_eq!((d.struct_size, d.field_count), (968, 5));
    let fs = descriptors::fields(&img, d.anchor, d.field_count);
    let last = fs.last().unwrap().1.unwrap();
    assert_eq!(last.offset, 0x18, "marker-8");
    assert_eq!(last.child_tag, 0x80801941);
    let child = idx.get(last.child_tag).unwrap();
    assert_eq!(last.offset as u64 + child.struct_size as u64, d.struct_size as u64);

    // the defective reading, kept visible so it cannot quietly come back
    let wrong = img.u32(last.marker + 0x20).unwrap();
    assert_ne!(wrong, last.offset, "marker+0x20 is the NEXT field's column, not this one's offset");
}

/// ⚠ Defect 2 - **the header is NOT at fixed offsets.** `0x80801796` and `0x808045f0` place their
/// field counts at different offsets from the registry object start; a fixed-offset reader yields
/// a *tag* where the count should be and rejects valid descriptors as UNRESOLVED. Everything here
/// is anchor-relative, so both resolve.
#[test]
fn defect_2_header_is_anchor_relative_not_fixed_offset() {
    let img = img!();
    let idx = descriptors::index(&img).unwrap();
    let a = idx.get(0x80801796).unwrap();
    let b = idx.get(0x808045F0).unwrap();
    assert_eq!((a.field_count, a.struct_size), (2, 8));
    assert_eq!((b.field_count, b.struct_size), (2, 44));
    // every one of the 4,555 has a sane count when read anchor-relative
    assert!(idx.iter().all(|(_, d)| d.field_count <= 64));
}

/// ⚠ Defect 3 - **the field sentinel is NOT a constant u64.** `marker+0` is `serialized_start`
/// and varies; only `marker+4` is `0x3F000000`. An exact-u64 test passes only when
/// `serialized_start == 0`. This reading was already retracted once in §13.4 and re-made.
#[test]
fn defect_3_sentinel_is_the_high_word_only() {
    let img = img!();
    let idx = descriptors::index(&img).unwrap();
    let d = idx.get(0x80801941).unwrap();
    let fs = descriptors::fields(&img, d.anchor, d.field_count);
    assert_eq!(fs.len(), 8);
    assert!(fs.iter().all(|(_, f)| f.is_some()), "stride is an ordinary 0x28 - no variable stride");

    let varying: Vec<u32> = fs.iter().filter_map(|(_, f)| f.map(|f| f.serialized_start)).collect();
    assert!(varying.iter().any(|&s| s != 0), "serialized_start varies across fields");

    for (mk, f) in &fs {
        let f = f.unwrap();
        assert_eq!(img.u32(mk + 4).unwrap(), SENTINEL, "the constant is the HIGH word");
        if f.serialized_start != 0 {
            assert_ne!(
                img.u64(*mk).unwrap(),
                (SENTINEL as u64) << 32,
                "an exact-u64 sentinel test would falsely report a missing marker here"
            );
        }
    }
}

/// ⚠ ATLAS §4.3 - `marker+0x14` is "width in BITS" **only for leaf fields**. `array-fields.py`
/// proves that on a composite the same column is the byte offset the element count is loaded from.
/// The typed API therefore refuses to call it `bits` unless the field is a leaf.
#[test]
fn bits_column_is_leaf_only() {
    let img = img!();
    let idx = descriptors::index(&img).unwrap();
    let d = idx.get(0x80801825).unwrap();
    let fs = descriptors::fields(&img, d.anchor, d.field_count);
    let leaf = fs.iter().filter_map(|(_, f)| *f).find(|f| f.is_leaf()).unwrap();
    let composite = fs.iter().filter_map(|(_, f)| *f).find(|f| !f.is_leaf()).unwrap();
    assert_eq!(leaf.bits(), Some(64));
    assert_eq!(composite.bits(), None, "a composite's +0x14 is NOT a bit width");
    assert_eq!(leaf.child_tag, LEAF);
}

/// THE ACCOUNT SCHEMA. `0x80801803` → `0x808018c7`, and field **`[3] @+0x068 LEAF 64 bits`** is
/// the profile SOID whose absence held the character gate shut (§26.76/§26.77).
#[test]
fn account_schema_field_3_is_the_profile_soid_at_0x68() {
    let img = img!();
    let idx = descriptors::index(&img).unwrap();
    let outer = idx.get(0x80801803).unwrap();
    assert_eq!((outer.field_count, outer.struct_size), (1, 29496));
    let f0 = descriptors::fields(&img, outer.anchor, 1)[0].1.unwrap();
    assert_eq!(f0.child_tag, 0x808018C7);

    let inner = idx.get(0x808018C7).unwrap();
    assert_eq!(inner.field_count, 25);
    let f3 = descriptors::fields(&img, inner.anchor, inner.field_count)[3].1.unwrap();
    assert_eq!((f3.offset, f3.kind, f3.child_tag, f3.bits()), (0x68, 0x106, LEAF, Some(64)));
}

/// THE CHARACTER REFERENCE VECTOR - five child tags in order. Independently reproduces
/// `message-catalogue.py`'s `PINNED_SCHEMAS["character"]` from a different implementation.
#[test]
fn character_schema_reference_vector() {
    let img = img!();
    let idx = descriptors::index(&img).unwrap();
    let d = idx.get(0x80801954).unwrap();
    assert_eq!(d.field_count, 6);
    let children: Vec<u32> = descriptors::fields(&img, d.anchor, d.field_count)
        .iter()
        .map(|(_, f)| f.unwrap().child_tag)
        .collect();
    assert_eq!(
        children,
        vec![LEAF, 0x808017FC, 0x8080181A, 0x80801755, 0x80801855, 0x80801923]
    );
}

/// `0x8080194D` is the same shape as `0x80801845` - the lookup that turned investment 702 from an
/// investigation into a one-line answer (`historicaldocs/design/message-catalogue.md` row 83, §26.77).
#[test]
fn msg_0x8080194d_is_the_same_shape_as_0x80801845() {
    let img = img!();
    let idx = descriptors::index(&img).unwrap();
    let a = idx.get(0x8080194D).unwrap();
    let b = idx.get(0x80801845).unwrap();
    assert_eq!((a.field_count, a.struct_size), (1, 8));
    assert_eq!((b.field_count, b.struct_size), (1, 8));
    let ca = descriptors::fields(&img, a.anchor, 1)[0].1.unwrap();
    let cb = descriptors::fields(&img, b.anchor, 1)[0].1.unwrap();
    assert_eq!(ca.child_tag, 0x80801796);
    assert_eq!(cb.child_tag, ca.child_tag);
    assert_eq!((ca.offset, ca.kind), (cb.offset, cb.kind));
}

/// ATLAS §4.4d - reproduce the **Python's own output**, byte for byte, on three schema trees.
/// The fixtures in `tests/fixtures/` were generated by `capture-rig/eboot-descriptors.py` before
/// the port; if the Rust walk drifts, these fail.
#[test]
fn walk_render_matches_the_python_golden_output() {
    let img = img!();
    let idx = descriptors::index(&img).unwrap();
    for (tag, fixture) in [
        (0x80801803u32, "walk-80801803-account.txt"),
        (0x80801825, "walk-80801825-inv503-response.txt"),
        (0x80801954, "walk-80801954-character.txt"),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(fixture);
        let want = std::fs::read_to_string(&path).expect("fixture");
        let got = descriptors::render(&descriptors::walk(&img, &idx, tag));
        assert_eq!(got, want, "{fixture} diverged for {tag:#x}");
    }
}

/// The index is built **per LOAD segment** (ATLAS §4.1), where `eboot-descriptors.py` searches the
/// whole 29 MB file and applies the text bias globally. This pins that they agree on this image,
/// so the correctness improvement is not a silent behaviour change: every anchor hit lands in
/// LOAD segment 0.
#[test]
fn every_anchor_hit_is_in_the_text_load_segment() {
    let img = img!();
    let idx = descriptors::index(&img).unwrap();
    let text = img.load_segments().next().unwrap();
    assert!(idx.iter().all(|(_, d)| text.file_backed_vaddrs().contains(&d.anchor)));
    // whole-file byte count of the anchor pattern, i.e. what the Python scans
    let whole_file = img
        .data()
        .windows(8)
        .filter(|w| *w == descriptors::ANCHOR)
        .count();
    let in_load = img.find_all(&descriptors::ANCHOR).len();
    assert_eq!(whole_file, in_load, "no anchor bytes outside a LOAD segment in this build");
}

// ═════════════════════ field-xref: register-relative field access ═════════════════════
//
// ⭐⭐ THE AXIS THESE TESTS PROTECT, IN WORDS:
//
//   *`field-xref` must find register-relative field access - `mov rax,[rdi+8]`, and `[rbx]` where
//   `rbx` was `add`ed to the field address - which an absolute-address `xref` is blind to BY
//   CONSTRUCTION; and it must NOT simply report every `[reg+N]` instruction it walks past.*
//
// Those are two different failures and a single test cannot see both, so the pinned set is a
// DISCRIMINATING PAIR: positives that only a working tracker can satisfy, and an inverted case that
// only a *discriminating* tracker can satisfy. Each was verified to FAIL by breaking the mechanism
// it protects, before shipping (`historicaldocs/design/field-xref.md` §5):
//
//   break                                              | positives      | inverted
//   ---------------------------------------------------|----------------|----------
//   don't seed RDI at entry (tracker trusts nothing)    | 3 FAIL         | pass
//   `tracked: true` unconditionally (trusts everything) | pass           | **FAIL**
//   `add`/`lea` kill the offset instead of moving it    | 2 FAIL         | pass
//   drop the argument-forwarding hop                    | 1 FAIL         | **FAIL** (liveness half)
//   attribute accessors with `Hit::function`            | 1 FAIL         | **FAIL** (liveness half)
//
// A control that only ever passes is decoration (CLAUDE.md trap 7). None of these do.

/// The whole point, stated as an assertion: the absolute-address scanner CANNOT see the field
/// access that `field-xref` finds. If this ever fails, `xref` grew the capability and `field-xref`
/// needs a new reason to exist - which is a finding, not a regression to paper over.
#[test]
fn absolute_xref_is_blind_to_the_access_field_xref_finds() {
    let img = img!();
    // 0x267AF10 IS `c_activity_loader` + 8 - the field's true absolute address.
    let abs = xref::scan_kind(&img, 0x267_AF10, RefKind::RipRel).unwrap();
    let sites: Vec<u64> = abs.iter().filter(|h| h.confirmed).map(|h| h.site).collect();
    assert!(
        !sites.contains(&0x340_240),
        "absolute xref unexpectedly reports 0x340240 - re-read field-xref's premise"
    );
    // Positive control IN THE SAME TEST, so "found nothing" cannot pass by the scanner being dead:
    // the same call DOES find the one RIP-relative access there is.
    assert!(!sites.is_empty(), "absolute xref found nothing at all - the scanner is broken");
    // And the bytes at 0x340240 contain no address: `mov rax,[rdi+8]`.
    assert_eq!(img.bytes(0x340_240, 4).unwrap(), &[0x48, 0x8B, 0x47, 0x08]);
}

/// Every pinned [`fieldxref::FieldControl`], through the same code path a user's query takes.
#[test]
fn field_xref_pinned_controls_pass() {
    let img = img!();
    let index = functions::build(&img).unwrap();
    let r = rtti::harvest(&img).unwrap();
    for c in fieldxref::controls(&img, &index, &r).unwrap() {
        assert!(c.passed, "field-xref control FAILED: {} [{}]", c.control.what, c.detail);
    }
}

/// ⭐ The inverted half, spelled out rather than hidden inside the loop above: the SAME sweep over
/// the SAME 67 candidates at an offset the object does not have must return **zero tracked**
/// accesses while STILL producing raw displacement candidates. Both halves matter - the first
/// catches a tracker that trusts every base register, the second catches a sweep that has quietly
/// stopped running and would satisfy a bare "is it empty?" assertion.
#[test]
fn field_xref_discriminates_a_field_the_object_does_not_have() {
    let img = img!();
    let index = functions::build(&img).unwrap();
    let rt = rtti::harvest(&img).unwrap();

    let hit = fieldxref::scan(&img, &index, &rt, Some("c_activity_loader"), Some(0x267_AF08), 8, 1)
        .unwrap();
    let miss =
        fieldxref::scan(&img, &index, &rt, Some("c_activity_loader"), Some(0x267_AF08), 0x20, 1)
            .unwrap();

    // Same candidate set, same code path - only the offset differs.
    assert_eq!(hit.candidates.len(), miss.candidates.len());

    assert!(
        hit.tracked().any(|h| h.site == 0x340_240),
        "the +8 positive must still fire in the same test as the inverted one"
    );
    assert_eq!(
        miss.tracked().count(),
        0,
        "offset +0x20 is past the end of the object; first leak: {:?}",
        miss.tracked().next().map(|h| (h.site, h.text.clone()))
    );
    assert!(
        miss.hits.len() >= 20,
        "the +0x20 sweep produced only {} raw candidates - it did not actually run",
        miss.hits.len()
    );
}

/// ⭐ ATLAS §4.2c, as a test. `c_activity_loader::vfunc[1]` folds `+8` into the base with
/// `add rbx,8` and then spells the field `[rbx]` - **displacement zero**. A scanner matching the
/// literal displacement byte misses the class's own initialiser.
#[test]
fn field_xref_sees_an_offset_folded_into_the_base_register() {
    let img = img!();
    let index = functions::build(&img).unwrap();
    let rt = rtti::harvest(&img).unwrap();
    let r = fieldxref::scan(&img, &index, &rt, Some("c_activity_loader"), Some(0x267_AF08), 8, 1)
        .unwrap();
    let folded = r.tracked().find(|h| h.site == 0x33F_FA2).expect("0x33ffa2 [rbx] not found");
    assert_eq!(folded.disp, 0, "this is the disp==0 case or it is not the case at all");
    assert_eq!(img.bytes(0x33F_FA2, 3).unwrap(), &[0x48, 0x8B, 0x03]);
}

/// ⭐ The field is never STORED to. It is initialised by an allocator the class hands the field's
/// ADDRESS to. A tool reporting only `mov [reg+8], …` would answer "nobody writes this field",
/// confidently and wrongly - so the escape path is pinned.
#[test]
fn field_xref_reports_the_address_escaping_into_the_initialiser() {
    let img = img!();
    let index = functions::build(&img).unwrap();
    let rt = rtti::harvest(&img).unwrap();
    let r = fieldxref::scan(&img, &index, &rt, Some("c_activity_loader"), Some(0x267_AF08), 8, 1)
        .unwrap();
    assert!(
        r.escapes.iter().any(|e| e.site == 0x33F_F9D && e.callee == 0x23D_E00 && e.arg_index == 0),
        "the +8 address must be seen entering FUN_0023DE00 in RDI; got {:?}",
        r.escapes.iter().map(|e| (e.site, e.callee)).collect::<Vec<_>>()
    );
    // No plain store to the field exists anywhere in the candidate set - which is WHY the escape
    // path is load-bearing rather than a nicety.
    assert_eq!(r.tracked().filter(|h| h.access.writes()).count(), 0);
}

/// ⛔ The prologue trap, as a regression test. `FUN_0033FF30` (`lea rax,[rip+…]; ret`) has **no**
/// `55 48 89 e5`, so a `nearest_prologue` attribution names the previous function instead. On this
/// module's first run that silently produced the wrong 8 callers and the sweep missed its own
/// control. Accessor attribution must go through the function index.
///
/// (`xref`'s own `Hit::function` was the same defect and was fixed on 2026-08-11 - see
/// `xref_fn_column_attributes_an_fpo_body_to_its_own_function`. This test still pins `field-xref`'s
/// independent resolution, because the point is that it does not depend on someone else's scanner.)
#[test]
fn accessor_attribution_survives_a_function_with_no_prologue() {
    let img = img!();
    let index = functions::build(&img).unwrap();
    let rt = rtti::harvest(&img).unwrap();
    let r = fieldxref::scan(&img, &index, &rt, None, Some(0x267_AF08), 8, 1).unwrap();
    assert!(
        r.accessors.contains(&0x33F_F30),
        "0x33ff30 is THE accessor and must be attributed to itself, not to its neighbour; got {:?}",
        r.accessors
    );
    // The trap, asserted directly: the prologue walk really does name a different function.
    assert_ne!(img.nearest_prologue(0x33F_F30, 0x4000), Some(0x33F_F30));
    // … and the fixed attribution names the accessor itself.
    assert_eq!(img.containing_function(0x33F_F30).proven_start(), Some(0x33F_F30));
}
