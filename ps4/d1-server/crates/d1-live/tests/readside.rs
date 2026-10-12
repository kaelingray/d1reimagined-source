// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The read-side controls, as `#[test]`s - the `d1guest.py read|obj|tag|descriptors` parity.
//!
//! ⛔ **These never touch the emulator.** Each spawns a `__mem-target` child of this crate's own
//! binary: a real process holding the real eboot at a base, plus a registry laid out from
//! `guest-symbols.json`. The child refuses to start if shadPS4 is up, so these tests **fail loudly
//! during a live run rather than measuring the wrong process** - and the failure names the reason.
//!
//! They are integration tests because `CARGO_BIN_EXE_d1-live` is the only supported way to get the
//! crate's own binary path, and every control here needs a separate address space.
//!
//! ⭐ The same functions back `d1-live controls`, so the `#[test]` suite and the `controls`
//! subcommand cannot drift into testing different things.

use std::path::Path;

use d1_live::fixture::Perturb;
use d1_live::readcontrols as rc;

fn helper() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_d1-live"))
}

/// If a client is up the fixture cannot run. Fail with the reason rather than passing vacuously -
/// a green test that measured nothing is worse than a red one.
fn require_offline() {
    if let Some(why) = rc::blocked_by_a_live_client() {
        panic!("{why}");
    }
}

#[test]
fn base_is_recovered_and_reads_land_where_the_arithmetic_says() {
    require_offline();
    let d = rc::base_and_read(helper()).expect("read-side base and read");
    assert!(d.contains("reproduce the image"), "{d}");
    assert!(d.contains("+1 differs"), "the negative control did not run: {d}");
    assert!(d.contains("errored"), "an unmapped read must ERROR, not return zeros: {d}");
}

/// ⛔ INVERSION: one flipped byte inside the verify window. The anchors still agree, so only a
/// direct compare against the file catches it.
#[test]
fn a_drifted_image_fails_base_control_two() {
    require_offline();
    let d = rc::wrong_image_fails_the_verify(helper()).expect("verify-window inversion");
    assert!(d.contains("INVERTED"), "{d}");
    assert!(d.contains("exit 2"), "the refusal must be control-grade: {d}");
}

/// ⛔ INVERSION: two candidate bases is a refusal, not a tie to be broken.
#[test]
fn two_candidate_bases_are_refused() {
    require_offline();
    let d = rc::exactly_one_base(helper()).expect("duplicate-anchor inversion");
    assert!(d.contains("need EXACTLY one"), "{d}");
    assert!(d.contains("2 hit(s)"), "the decoy was never found, so nothing was inverted: {d}");
}

/// ⛔ INVERSION: one bit of stride must withdraw the element address AND drop the ladder.
#[test]
fn a_registry_that_disagrees_with_itself_yields_no_address() {
    require_offline();
    let d = rc::registry_cross_check_refuses(helper()).expect("family-stride inversion");
    assert!(d.contains("INVERTED"), "{d}");
    assert!(d.contains("L2"), "the liveness ladder must notice too: {d}");
}

#[test]
fn a_tag_resolves_through_the_signed_back_offset() {
    require_offline();
    let d = rc::tag_back_offset(helper()).expect("package tag control");
    assert!(d.contains("the bytes there are the object"), "{d}");
    assert!(d.contains("tag+1 lands elsewhere"), "the index is not discriminating: {d}");
}

#[test]
fn descriptors_decode_and_free_rows_are_filtered() {
    require_offline();
    let d = rc::descriptors_decode(helper()).expect("descriptor control");
    assert!(d.contains("update accepted"), "{d}");
    assert!(d.contains("update REFUSED"), "{d}");
    assert!(d.contains("free rows were filtered"), "{d}");
}

/// ⚠ The refusal two wrong readings were published for.
#[test]
fn a_read_after_death_is_an_error_not_a_zero() {
    require_offline();
    let d = rc::read_after_death(helper()).expect("dead-client control");
    assert!(d.contains("exit 3"), "{d}");
}

/// ⛔ THREE INVERSIONS, each paired with the positive read it perturbs. Every one of them was a
/// live defect in the first cut of this port, and none is reachable through the golden diff:
///
/// * `obj 576460752303423492:0` (`4 + 2^59`, and `0x60 * 2^59 == 0` mod 2^64) wrapped onto family
///   4/0's entry and printed the ACCOUNT object's bytes at **exit 0**, headed "cross-check SKIPPED".
/// * A symbol file with `entry_stride_family` deleted was answered from a compiled-in constant.
///   `d1guest.py` raises `KeyError` on the same file.
/// * `package_registry_layout.slot_data_base = 0x3C` read an 8-byte field out of a `0x40`-byte
///   record and **panicked** (exit 101). A crash is loud, but it is not a refusal.
#[test]
fn garbage_input_is_refused_and_never_answered() {
    require_offline();
    let d = rc::garbage_in_refuses(helper()).expect("garbage-in inversions");
    assert_eq!(
        d.matches("INVERTED").count(),
        3,
        "all three inversions must fire in the same window: {d}"
    );
    assert!(d.contains("POSITIVE"), "a refusal with no paired success proves nothing: {d}");
    assert!(d.contains("wraps onto"), "the aliasing-family inversion did not run: {d}");
}

/// The fixture itself must be honest about what it is: a **synthetic** address space whose registry
/// contents are ours. This asserts the shape a caller depends on, so a future change that stopped
/// writing the package object (say) fails here rather than silently weakening every control above.
#[test]
fn the_fixture_publishes_the_addresses_its_controls_need() {
    require_offline();
    let f = rc::Fixture::spawn(helper(), Perturb::None, 15.0).expect("fixture");
    let b = &f.banner;
    assert_eq!(b.base, d1_live::fixture::FIXTURE_BASE);
    assert_eq!(b.tag, d1_live::fixture::FIXTURE_TAG);
    assert_eq!(b.account_soid, d1_live::fixture::FIXTURE_SOID);
    for (name, a) in [
        ("registry", b.registry),
        ("package_header", b.package_header),
        ("package_object", b.package_object),
    ] {
        assert!(a != 0, "{name} was not published");
    }
    let proc = f.proc().expect("open the fixture");
    assert!(proc.mapping_of(b.base).is_some(), "the image window is not mapped");
    assert!(
        proc.mapping_of(b.unmapped).is_none(),
        "the 'unmapped' address is mapped, so the loud-failure inversion proves nothing"
    );
}
