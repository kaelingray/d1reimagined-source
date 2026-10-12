#![cfg(feature = "private-fixtures")]
// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Controls for the message-catalogue port (`catalogue.rs` <- `capture-rig/message-catalogue.py`).
//!
//! The centrepiece is an EXECUTED golden diff: `tests/fixtures/message-catalogue-full.txt` is the
//! Python prototype's own full output, captured before the port, and the Rust render must equal
//! it byte-for-byte. The inverted controls then prove the instrument can fail: perturbing one
//! byte of the input image flips a pinned policy row from PASS to FAIL, and a truncated
//! definitions file is refused loudly. A control that can only pass is decoration.
//!
//! Every test skips (rather than fails) when the pinned eboot or the shipped-content definitions
//! file is absent, so the workspace still builds on a machine without the dumps.

use std::path::Path;

use d1_eboot::catalogue::{self, Data, InvestmentDef, Reader};
use d1_eboot::image::{Image, PINNED_EBOOT};

const DEFS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../capture-rig/schemas/investment-message-definitions.bin"
);
const D1BAP: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../d1-bap/src/lib.rs");
const GOLDEN: &str = include_str!("fixtures/message-catalogue-full.txt");

fn setup() -> Option<(Image, Vec<InvestmentDef>, String)> {
    if !Path::new(PINNED_EBOOT).exists() {
        eprintln!("SKIP: pinned eboot not present at {PINNED_EBOOT}");
        return None;
    }
    if !Path::new(DEFS).exists() {
        eprintln!("SKIP: investment definitions not present at {DEFS}");
        return None;
    }
    let image = Image::open(PINNED_EBOOT).expect("open pinned eboot");
    let defs = catalogue::investment_defs(&std::fs::read(DEFS).expect("read defs"))
        .expect("parse defs");
    let d1bap = std::fs::read_to_string(D1BAP).expect("read d1-bap lib.rs");
    Some((image, defs, d1bap))
}

macro_rules! setup {
    () => {
        match setup() {
            Some(t) => t,
            None => return,
        }
    };
}

/// ⭐ THE GOLDEN DIFF, executed and then pinned. The fixture is the Python prototype's complete
/// output over the pinned eboot + the pinned definitions file; the port must reproduce every
/// byte: controls, envelope roles ("NONE - do not answer" included), policies, schema records,
/// the investment table, and the d1-bap cross-check.
#[test]
fn full_render_is_byte_identical_to_the_python_prototype() {
    let (image, defs, d1bap) = setup!();
    let reader = Reader::new(&image).expect("reader");
    let data = Data::build(&image, Some(defs), Some(&d1bap)).expect("data");
    let (text, ok) = catalogue::render_full(&image, &reader, &data);
    assert!(ok, "controls failed on the pinned build");
    assert_eq!(text, GOLDEN, "render diverged from the Python prototype's pinned output");
}

/// Every structured control row passes on the pinned build, and all fourteen rows are present
/// (4 pinned schemas, the 503 cross-instrument row, coverage, header sizes, 2 pinned policies,
/// 2 pinned definition rows, the paired absence control, tag resolution, the record-length rule).
#[test]
fn all_fourteen_control_rows_pass_on_the_pinned_build() {
    let (image, defs, _) = setup!();
    let reader = Reader::new(&image).expect("reader");
    let data = Data::build(&image, Some(defs), None).expect("data");
    let (_, ok, rows) = catalogue::render_controls(&image, &reader, &data);
    assert!(ok);
    assert_eq!(rows.len(), 14, "control row count changed - a control was added or lost");
    for r in &rows {
        assert!(r.passed, "control failed on pinned build: {}", r.name);
    }
}

/// ⭐ INVERTED: perturb one byte of the input and prove the output changes and the control FAILS.
///
/// Policy family 13's expected-response constant (`0x000D`, the subscription response) lives in a
/// `mov eax/al, imm; ret` thunk behind vtable slot +0x30. Flipping the immediate's low byte must
/// (a) change the derived response type and (b) flip the pinned-policy control row to FAIL. This
/// is what makes the passing rows above a measurement rather than decoration.
#[test]
fn perturbing_the_policy_response_constant_fails_the_pinned_control() {
    let (image, defs, _) = setup!();
    let reader = Reader::new(&image).expect("reader");
    let pristine = Data::build(&image, Some(defs.clone()), None).expect("data");
    assert_eq!(pristine.pols[13].response_type, Some(0x0D));

    let vt = pristine.pols[13].vtable.expect("family 13 vtable");
    let rsp_fn = reader.ptr(vt + 0x30).expect("response-type thunk");
    let off = image.file_offset(rsp_fn).expect("thunk file offset");
    let mut bytes = image.data().to_vec();
    // `mov eax, imm32` (B8) or `mov al, imm8` (B0): the immediate starts one byte in.
    assert!(
        bytes[off] == 0xB8 || bytes[off] == 0xB0,
        "unexpected thunk encoding {:#04x} - the perturbation would be aimed wrong",
        bytes[off]
    );
    bytes[off + 1] ^= 0x11;

    let mutated = Image::from_bytes(image.path().to_path_buf(), bytes).expect("mutated image");
    let mreader = Reader::new(&mutated).expect("reader");
    let mdata = Data::build(&mutated, Some(defs), None).expect("data");
    assert_ne!(mdata.pols[13].response_type, Some(0x0D), "perturbation did not reach the output");
    let (_, ok, rows) = catalogue::render_controls(&mutated, &mreader, &mdata);
    assert!(!ok, "controls still pass on a perturbed image - they discriminate nothing");
    let row = rows
        .iter()
        .find(|r| r.name == "catalogue pinned policy family 13")
        .expect("pinned policy row present");
    assert!(!row.passed, "the pinned policy control did not catch the perturbation");
}

/// The definitions-file stride check refuses both truncation and a corrupted count, with the
/// prototype's exact wording. A definitions file that parses "mostly" would silently shift every
/// row, so refusal is the behaviour under test.
#[test]
fn defs_stride_check_refuses_truncation_and_count_corruption() {
    let (_, _, _) = setup!();
    let good = std::fs::read(DEFS).expect("read defs");
    assert!(catalogue::investment_defs(&good).is_ok());

    let truncated = &good[..good.len() - 0x40];
    let e = catalogue::investment_defs(truncated).expect_err("truncation must be refused");
    assert!(e.contains("stride check failed"), "unexpected error text: {e}");

    let mut bad_count = good.clone();
    bad_count[0x18] ^= 0x01;
    let e = catalogue::investment_defs(&bad_count).expect_err("count corruption must be refused");
    assert!(e.contains("stride check failed"), "unexpected error text: {e}");
}

/// The d1-bap constant harvest matches the prototype's regex semantics: hex-literal `u16`
/// constants only, aliases defined in terms of other constants skipped.
#[test]
fn d1bap_constant_harvest_matches_the_regex_semantics() {
    let src = "pub const TYPE_A: u16 = 0x0019;\n\
               pub const TYPE_ALIAS: u16 = TYPE_A;\n\
               pub const NOT_TYPE: u16 = 0x0001;\n\
               pub const TYPE_B: u16 = 0x012f;\n";
    let got = catalogue::d1bap_constants(src);
    assert_eq!(got, vec![("TYPE_A".to_string(), 0x19), ("TYPE_B".to_string(), 0x12F)]);
}
