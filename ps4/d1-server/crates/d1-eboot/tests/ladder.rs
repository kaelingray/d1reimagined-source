#![cfg(feature = "private-fixtures")]
// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! **The ladder controls, as tests** - `ladder.rs` is the port of `capture-rig/state-machine.py`,
//! and ATLAS §4.4d says a new tool is not evidence until it reproduces a KNOWN result. The known
//! results here are the Python prototype's own outputs (`tests/fixtures/state-machine-*`,
//! captured 2026-08-11 before the port), reproduced **byte for byte**.
//!
//! The inverted controls then prove the instrument can fail: perturbing one relocation addend in
//! the input image flips the rung-28 pinned control from PASS to FAIL and visibly changes the
//! table, and a doctored diagnostic catalogue flips the ENTER/EXIT narration controls. A control
//! that can only pass is decoration.
//!
//! Every test skips (rather than fails) when the pinned eboot - or, where needed, the generated
//! annotation TSVs - is absent, so the workspace still builds on a machine without the dumps.

use std::path::Path;

use d1_eboot::image::{Image, PINNED_EBOOT};
use d1_eboot::ladder;
use d1_eboot::reloc;

const TABLE_FIXTURE: &str = include_str!("fixtures/state-machine-table.txt");
const JSON_FIXTURE: &str = include_str!("fixtures/state-machine-handlers.json");

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

fn annotations() -> Option<ladder::Annotations> {
    let (a, d) = (Path::new(ladder::DEFAULT_ATLAS_TSV), Path::new(ladder::DEFAULT_DIAG_TSV));
    if !a.exists() || !d.exists() {
        eprintln!("SKIP: annotation TSVs not present ({a:?}, {d:?})");
        return None;
    }
    Some(ladder::load_annotations(a, d).expect("read annotation TSVs"))
}

// ───────────────────────── golden reproduction (ATLAS §4.4d) ─────────────────────────

/// The full-ladder table, byte for byte the prototype's default output. Pure image derivation -
/// no TSVs involved - so this fixture cannot drift with catalogue regeneration.
#[test]
fn table_matches_the_python_prototype_byte_for_byte() {
    let img = img!();
    let handlers = ladder::load_handlers(&img).expect("load handlers");
    assert_eq!(ladder::render_table(&handlers), TABLE_FIXTURE);
}

/// `--json`, byte for byte the prototype's `json.dumps(handlers, indent=1)` plus `print`'s
/// newline. This is the exact shape `d1-trace dash/ladder.rs` deserializes.
#[test]
fn json_matches_the_python_prototype_byte_for_byte() {
    let img = img!();
    let handlers = ladder::load_handlers(&img).expect("load handlers");
    let json = ladder::to_json_py(&handlers) + "\n";
    assert_eq!(json, JSON_FIXTURE);
    // And the d1-trace consumer contract: every element carries the four declared fields.
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = v.as_array().unwrap();
    assert_eq!(arr.len(), 35);
    for h in arr {
        for k in ["class", "short", "rung", "state", "vtable", "slots", "overridden"] {
            assert!(h.get(k).is_some(), "missing key {k}");
        }
        assert!(h["rung"].is_u64(), "rung must be numeric for the d1-trace parse");
    }
}

/// All 17 prototype controls pass on the pinned image, with the prototype's exact rendered lines
/// for the image-derived rows. The last three rows' *details* carry counts from the generated
/// TSVs, which legitimately change when the catalogue is regenerated from new runs - for those,
/// label and verdict are pinned and the counts are not (stated normalization, not an oversight).
#[test]
fn controls_pass_on_the_pinned_image_with_the_prototype_rows() {
    let img = img!();
    let Some(ann) = annotations() else { return };
    let (text, ok, rows) = ladder::controls(&img, &ann).expect("controls run");
    assert!(ok, "controls failed on the pinned image:\n{text}");
    assert_eq!(rows.len(), 17, "a control was added or lost");
    let lines: Vec<&str> = text.lines().collect();
    let pinned = [
        "PASS  vtable slot reader reproduces the pinned s_investment_utility_context slots  \
         [[+0x80]=0x260a10 [+0xc8]=0x260bd0]",
        "PASS  RTTI yields 35 world-controller state handlers  [[35]]",
        "PASS  every handler class maps to a rung in the state table  [[35/35]]",
        "PASS  rung 28 UPDATE == 0x7A6D60, the census's requesting site for rung 29  [0x7a6d60]",
        "PASS  rung 25 UPDATE == 0x7A3750, which the atlas names cleanup_handler::vfunc[7]  \
         [0x7a3750]",
        "PASS  rung 30 UPDATE == 0x7A5250, the census's requesting site for rung 31  [0x7a5250]",
        "PASS  rung 27 (orbit) UPDATE == 0x7AAF60  [0x7aaf60]",
        "PASS  base +0x30 is the shared 0x7914E0 thunk (inherited, so a real negative exists)  \
         [0x7914e0]",
        "PASS  base UPDATE (+0x38) is pure-virtual - every rung must supply its own  [0x0]",
        "PASS  all 35 handlers override UPDATE  [[35/35]]",
        "PASS  rung 28's update contains `mov edi, 0x1d` -> call 0x783690 (id == rung number)",
        "PASS  EXIT is slot 5 (+0x28) - the dispatch that follows the pump's own \"Leaving \
         state\" emit  [0x28]",
        "PASS  ENTER is slot 4 (+0x20) - the dispatch that follows the pump's own \"Entering \
         state\" emit  [0x20]",
        "PASS  base ENTER is pure-virtual and base EXIT is the shared no-op (ABI corroboration)  \
         [enter=0x0 exit=0x752d30]",
    ];
    for (i, want) in pinned.iter().enumerate() {
        assert_eq!(lines[i], *want, "control row {i} drifted");
    }
    // The TSV-derived rows: verdict pinned, counts normalized away.
    assert!(lines[14].starts_with("PASS  load_atlas() returns names for the whole atlas"));
    assert!(lines[15].starts_with("PASS  ...and the pump 0x782300 resolves to a name"));
    assert!(lines[16].starts_with("PASS  load_strings_by_fn() reads the diagnostic catalogue"));
    assert_eq!(*lines.last().unwrap(), "ALL CONTROLS PASS");
}

// ───────────────────────── inverted controls: the instrument can fail ─────────────────────────

/// ⭐ INVERTED (image perturbation): flip one byte of the `R_X86_64_RELATIVE` addend that fills
/// rung 28's UPDATE slot. The derived table must change and the pinned rung-28 control must FAIL.
/// This proves the slots are read through the relocation table (the file bytes at the vtable are
/// zero and are NOT touched here), and that the passing rows above discriminate.
#[test]
fn perturbing_the_rung28_update_relocation_flips_the_control() {
    let img = img!();
    let Some(ann) = annotations() else { return };
    let handlers = ladder::load_handlers(&img).expect("load handlers");
    let h28 = handlers.iter().find(|h| h.rung == Some(28)).expect("rung 28 present");
    assert_eq!(h28.slots.update, 0x7A6D60);

    let slot = h28.vtable + 0x38;
    let t = reloc::tables(&img).expect("rela tables");
    let idx = reloc::all(&img)
        .expect("rela entries")
        .iter()
        .position(|r| r.slot == slot && r.r_type == reloc::R_X86_64_RELATIVE)
        .expect("rung 28 UPDATE slot has a RELATIVE relocation");
    let addend_off = (t.rela_file_offset + idx as u64 * t.rela_entsize + 16) as usize;

    let mut bytes = img.data().to_vec();
    assert_eq!(
        u64::from_le_bytes(bytes[addend_off..addend_off + 8].try_into().unwrap()),
        0x7A6D60,
        "perturbation is aimed at the wrong file offset"
    );
    bytes[addend_off] ^= 0x01;

    let mutated = Image::from_bytes(img.path().to_path_buf(), bytes).expect("mutated image");
    let mhandlers = ladder::load_handlers(&mutated).expect("load handlers on mutated image");
    let m28 = mhandlers.iter().find(|h| h.rung == Some(28)).expect("rung 28 still present");
    assert_eq!(m28.slots.update, 0x7A6D61, "perturbation did not reach the output");
    assert_ne!(
        ladder::render_table(&mhandlers),
        TABLE_FIXTURE,
        "the table did not change - the reader is not reading the relocation table"
    );

    let (_, ok, rows) = ladder::controls(&mutated, &ann).expect("controls on mutated image");
    assert!(!ok, "controls still pass on a perturbed image - they discriminate nothing");
    let row = rows.iter().find(|r| r.label.starts_with("rung 28 UPDATE")).unwrap();
    assert!(!row.passed, "the rung-28 pinned control did not catch the perturbation");
    assert_eq!(row.detail, "0x7a6d61");
}

/// ⭐ INVERTED (table-row perturbation): doctor the diagnostic catalogue so every "Leaving state"
/// text reads "Entering state". The EXIT narration control must then find NO dispatch and the
/// ENTER control must find two, and both must FAIL - proving the ENTER/EXIT pairing is *derived
/// from the narration on every run*, not transcribed.
#[test]
fn doctoring_the_diag_catalogue_fails_the_narration_controls() {
    let img = img!();
    if annotations().is_none() {
        return;
    }
    let text = std::fs::read_to_string(ladder::DEFAULT_DIAG_TSV).expect("read diag tsv");
    assert!(text.contains("Leaving state"), "the catalogue no longer carries the pump strings");
    let doctored_path =
        std::env::temp_dir().join(format!("d1-ladder-doctored-{}.tsv", std::process::id()));
    std::fs::write(&doctored_path, text.replace("Leaving state", "Entering state"))
        .expect("write doctored tsv");
    let ann = ladder::load_annotations(Path::new(ladder::DEFAULT_ATLAS_TSV), &doctored_path)
        .expect("load doctored annotations");
    let (_, ok, rows) = ladder::controls(&img, &ann).expect("controls run");
    let _ = std::fs::remove_file(&doctored_path);

    assert!(!ok, "controls still pass with a doctored catalogue");
    let exit_row = rows.iter().find(|r| r.label.starts_with("EXIT is slot")).unwrap();
    assert!(!exit_row.passed);
    assert_eq!(exit_row.detail, "NO DISPATCH FOUND");
    let enter_row = rows.iter().find(|r| r.label.starts_with("ENTER is slot")).unwrap();
    assert!(!enter_row.passed, "ENTER control must see both dispatches narrated as Entering");
    assert_eq!(enter_row.detail, "0x20 0x28");
}

// ───────────────────────── refusals: fail loudly, never plausibly ─────────────────────────

#[test]
fn a_non_elf_is_refused() {
    let e = Image::from_bytes("<mem>".into(), b"garbage, not an elf".to_vec());
    assert!(e.is_err(), "a non-ELF must not open");
}

/// A truncated eboot must error, not produce a shorter ladder.
#[test]
fn a_truncated_eboot_is_refused() {
    let img = img!();
    let bytes = img.data()[..1 << 20].to_vec();
    let outcome = Image::from_bytes(img.path().to_path_buf(), bytes)
        .and_then(|i| ladder::load_handlers(&i));
    assert!(outcome.is_err(), "a truncated eboot must fail loudly");
}

/// An image whose relocation table is emptied (DT_SCE_RELASZ zeroed in PT_DYNAMIC) yields no
/// RTTI and therefore no handlers - and [`ladder::require_sane`] refuses to render that as an
/// empty-but-plausible table. The prototype printed "0 handlers." and exited 0 here.
#[test]
fn an_image_with_no_relocations_is_refused_not_rendered() {
    let img = img!();
    let dyn_seg = img
        .segment_of_type(d1_eboot::image::PT_DYNAMIC)
        .expect("pinned eboot has PT_DYNAMIC")
        .clone();
    let mut bytes = img.data().to_vec();
    let base = dyn_seg.offset as usize;
    let mut p = base;
    let mut zeroed = false;
    while p + 16 <= base + dyn_seg.filesz as usize {
        let tag = u64::from_le_bytes(bytes[p..p + 8].try_into().unwrap());
        if tag == 0 {
            break;
        }
        if tag == reloc::DT_SCE_RELASZ {
            bytes[p + 8..p + 16].fill(0);
            zeroed = true;
        }
        p += 16;
    }
    assert!(zeroed, "DT_SCE_RELASZ not found - the perturbation is aimed wrong");
    let mutated = Image::from_bytes(img.path().to_path_buf(), bytes).expect("mutated image");
    let handlers = ladder::load_handlers(&mutated).expect("harvest itself survives");
    assert!(handlers.is_empty(), "no relocations must mean no RTTI classes");
    let refusal = ladder::require_sane(&handlers);
    assert!(refusal.is_err(), "an empty ladder must be refused, not rendered");
    assert!(refusal.unwrap_err().contains("Refusing to print an empty ladder"));
}

/// The second `require_sane` arm: a handler set in which nothing overrides UPDATE (the shape an
/// emptied vtable region produces) is a plausible all-`-` table and must be refused.
#[test]
fn a_ladder_with_no_update_overrides_is_refused() {
    let img = img!();
    let mut handlers = ladder::load_handlers(&img).expect("load handlers");
    assert!(ladder::require_sane(&handlers).is_ok(), "the pinned image must pass");
    for h in &mut handlers {
        h.overridden.update = false;
    }
    let refusal = ladder::require_sane(&handlers);
    assert!(refusal.is_err());
    assert!(refusal.unwrap_err().contains("no handler overrides UPDATE"));
}

/// Missing annotation TSVs: the loaders return empty maps (the prototype's render behaviour) and
/// the controls FAIL rather than pass vacuously - silence is never a clean result.
#[test]
fn missing_tsvs_load_empty_and_fail_the_controls() {
    let img = img!();
    let ann = ladder::load_annotations(
        Path::new("/nonexistent/atlas.tsv"),
        Path::new("/nonexistent/diag.tsv"),
    )
    .expect("missing files load as empty, not as an error");
    assert!(ann.atlas.is_empty() && ann.strings_by_fn.is_empty());
    let (_, ok, rows) = ladder::controls(&img, &ann).expect("controls run");
    assert!(!ok, "controls must fail without their annotation sources");
    for needle in ["load_atlas()", "load_strings_by_fn()", "EXIT is slot", "ENTER is slot"] {
        let row = rows.iter().find(|r| r.label.starts_with(needle)).unwrap();
        assert!(!row.passed, "{needle} row must fail with empty annotations");
    }
}

/// The one deliberate deviation from the prototype, asserted: a frame-pointer-omitted handler
/// function (rung 28's slot40, 0x7a7050) decodes to a real body when reached through a vtable
/// slot, where the prototype silently printed an empty one - and an arbitrary mid-instruction
/// `--fn` address is refused loudly rather than decoded from a guess.
#[test]
fn fpo_handler_bodies_decode_and_guessed_starts_are_refused() {
    let img = img!();
    let ann = ladder::Annotations::default();
    let text = ladder::read_function(&img, 0x7A7050, &ann, 4000, true)
        .expect("a vtable-slot FPO function must decode");
    assert!(
        text.lines().count() > 10,
        "the FPO body must be a real decode, not the prototype's silent empty"
    );
    assert!(ladder::read_function(&img, 0x7A6D61, &ann, 4000, false).is_err());
}
