// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! **The pinned vectors, as tests.**
//!
//! This file is why the port exists. Deriving a payload used to be a shell command someone had to
//! remember to run with the right oracle and the right flags; here it is a `cargo test` assertion,
//! and a regression in the harness, the descriptor reader or the registry builder is a red build
//! rather than a quiet wrong payload on the wire.
//!
//! Every vector below was produced by the client's own codec and independently re-derived through
//! this crate. They are **not** analogies from a neighbouring schema - that distinction is the
//! whole point of the oracle (`.claude/skills/native-oracle/SKILL.md`).
//!
//! Tests **skip** rather than fail when the pinned eboot is absent, so the workspace still builds
//! on a machine without the dumps - the same convention as `d1-eboot/tests/controls.rs`.

use std::path::Path;

use d1_oracle::control::{self, hex, hexs};
use d1_oracle::error::OracleError;
use d1_oracle::object::pack_bits;
use d1_oracle::{MessageOracle, ObjectOracle, ACCOUNT_SOID, CHARACTER_SOID, PROFILE_SOID};

fn have_eboot() -> bool {
    let p = std::env::var("D1_EBOOT")
        .unwrap_or_else(|_| d1_eboot::image::PINNED_EBOOT.to_string());
    if Path::new(&p).exists() {
        return true;
    }
    eprintln!("SKIP: pinned eboot not present at {p}");
    false
}

macro_rules! need_eboot {
    () => {
        if !have_eboot() {
            return;
        }
    };
}

// ───────────────────────────── the control ─────────────────────────────

/// ⚠ **A decode reported without its control is not evidence.** Everything else in this file runs
/// through [`ObjectOracle::new`], which refuses to construct until this has passed - so this test
/// is the statement of the rule, not its only enforcement.
#[test]
fn object_control_passes() {
    need_eboot!();
    let c = control::object_control().as_ref().expect("object control");
    assert_eq!(c.tag, 0x8080_1954);
    assert_eq!(c.vector, control::CONTROL_VECTOR);
    assert_eq!(c.soid, control::CONTROL_SOID);
    assert_eq!(c.identity, control::CONTROL_IDENTITY);
    assert_eq!(c.bits, control::CONTROL_BITS, "the control's BIT COUNT is not decoration");
}

/// The message codec's own control: both vectors from `rsat-schema-encode-oracle.py --selftest`,
/// reproduced byte for byte by the Rust driver (ATLAS §4.4d - a new tool is not evidence until it
/// reproduces a known result).
#[test]
fn message_control_reproduces_the_python_selftest() {
    need_eboot!();
    let c = control::message_control().as_ref().expect("message control");
    assert_eq!(c.wire_503, control::PIN_503);
    assert_eq!(c.wire_206, control::PIN_206);
}

// ───────────────────────── the three pinned decodes ─────────────────────────

/// **The control vector, decoded through the public API.** SOID, identity, and *exactly* 97 bits.
/// Produced independently on 2026-08-06 by driving `0x105B410`
/// (`historicaldocs/design/rsat-character-object.md` §7) - not by this tool, which is what makes it a control.
#[test]
fn character_0x80801954_is_97_bits() {
    need_eboot!();
    let mut o = ObjectOracle::new(0x8080_1954).unwrap();
    let d = o.decode(&hex(control::CONTROL_VECTOR).unwrap()).unwrap();
    assert_eq!(d.bits, 97);
    assert_eq!(d.qword(0), CHARACTER_SOID);
    assert_eq!(&d.object[8..11], &[0u8, 1, 0]);
    d.require_nonzero_qwords(&[0x00, 0x08]).unwrap();
}

/// The account payload that opened the `character:signin` gate on the FIRST live attempt
/// (2026-08-07, `historicaldocs/design/character-and-orbit.md` §26.76). Derived from the descriptor table, not by
/// analogy: `[3] @+0x068 LEAF 64 bits` is the profile SOID.
///
/// ⚠ Field 3 is [`PROFILE_SOID`] = `0xD100000000000201`. An earlier write-up said `…0002`; that was
/// a hand-transcription error (§26.76.1). **Never transcribe one of these by eye** - every SOID in
/// this project is `0xD1` + six zero bytes + two significant ones.
#[test]
fn account_0x80801803_is_221_bits_with_four_nonzero_qwords() {
    need_eboot!();
    let mut o = ObjectOracle::new(0x8080_1803).unwrap();
    let d = o
        .decode(&hex("e880000000000000c74400000000000405d100000000000201000000").unwrap())
        .unwrap();
    assert_eq!(d.bits, 221);
    assert_eq!(d.object.len(), 29_496);
    // The check people skip: the EXACT set, so a stray non-zero qword fails the build.
    d.require_nonzero_qwords(&[0x00, 0x08, 0x10, 0x68]).unwrap();
    assert_eq!(d.qword(0x00), ACCOUNT_SOID);
    assert_eq!(d.qword(0x08), 1, "the SOID-list count");
    assert_eq!(d.qword(0x10), CHARACTER_SOID);
    assert_eq!(d.qword(0x68), PROFILE_SOID);
}

/// The family-4 / type-4 payload, pre-verified by the oracle before any run.
#[test]
fn family_4_type_4_0x8080102e_is_67_bits_with_one_nonzero_qword() {
    need_eboot!();
    let mut o = ObjectOracle::new(0x8080_102E).unwrap();
    let d = o.decode(&hex("e88000000000000080").unwrap()).unwrap();
    assert_eq!(d.bits, 67);
    assert_eq!(d.object.len(), 3_984);
    assert_eq!(d.qword(0x00), ACCOUNT_SOID);
    d.require_nonzero_qwords(&[0x00]).unwrap();
}

// ───────────────────────── the traps, as tests ─────────────────────────

/// ⚠ **§26.67, exactly.** A 17-byte account body decodes "fine" while the client reads **157 bits
/// out of 136** - 21 bits of overrun. The bitstream returns zeros past the end, which is
/// indistinguishable from "those fields were absent", so the payload looks correct and is not.
///
/// This must be a **hard error**, never a silent pass. If this test ever starts returning `Ok`,
/// the oracle has stopped being able to tell a short payload from an absent field.
#[test]
fn an_under_length_payload_is_a_hard_error_not_a_quiet_success() {
    need_eboot!();
    let mut o = ObjectOracle::new(0x8080_1803).unwrap();
    let err = o
        .decode(&hex("e880000000000000c74400000000000404").unwrap())
        .expect_err("a 17-byte account body must NOT decode quietly");
    match err {
        OracleError::ShortPayload { bits_read, bits_available } => {
            assert_eq!(bits_read, 157);
            assert_eq!(bits_available, 136);
        }
        other => panic!("expected ShortPayload, got {other}"),
    }
}

/// A stray non-zero qword is a bit-grammar bug that would otherwise reach the wire, so the
/// exact-set check must actually reject. (Negative control for
/// [`Decoded::require_nonzero_qwords`](d1_oracle::object::Decoded::require_nonzero_qwords) - an
/// assertion helper that never fails is not an assertion.)
#[test]
fn require_nonzero_qwords_rejects_a_wrong_set() {
    need_eboot!();
    let mut o = ObjectOracle::new(0x8080_1803).unwrap();
    let d = o
        .decode(&hex("e880000000000000c74400000000000405d100000000000201000000").unwrap())
        .unwrap();
    assert!(d.require_nonzero_qwords(&[0x00, 0x08, 0x10]).is_err(), "missing +0x68 must fail");
    assert!(
        d.require_nonzero_qwords(&[0x00, 0x08, 0x10, 0x68, 0x70]).is_err(),
        "an extra offset must fail"
    );
}

/// **Same variable, both places.** The account's field 3 *names* the profile SOID; if the object
/// actually delivered under that key carries a different one, the client's registry lookup finds
/// nothing and the field is silently inert. So assert the two agree - in one test, not two docs.
#[test]
fn the_account_field_3_soid_is_the_soid_the_profile_object_is_delivered_under() {
    need_eboot!();
    let mut acct = ObjectOracle::new(0x8080_1803).unwrap();
    let a = acct
        .decode(&hex("e880000000000000c74400000000000405d100000000000201000000").unwrap())
        .unwrap();

    let mut pro = ObjectOracle::new(0x8080_1743).unwrap();
    let payload = pack_bits(&format!("1{PROFILE_SOID:064b}{}", "0".repeat(17)));
    assert_eq!(hexs(&payload), "e880000000000100800000");
    let p = pro.decode(&payload).unwrap();
    assert_eq!(p.bits, 82);
    p.require_nonzero_qwords(&[0x00]).unwrap();

    assert_eq!(
        a.qword(0x68),
        p.qword(0x00),
        "the account names a profile SOID the profile object is not delivered under"
    );
}

// ───────────────────────── encode direction ─────────────────────────

/// The roster vector, rebuilt by the client's own **encoder** from a C object and decoded back to
/// the identical struct. This is the direction that proves we can *emit*, not only read.
#[test]
fn roster_0x80801951_encode_round_trips_to_the_pinned_wire() {
    need_eboot!();
    let mut o = ObjectOracle::new(0x8080_1951).unwrap();
    let mut body = vec![0u8; o.struct_size()];
    body[0x00..0x08].copy_from_slice(&ACCOUNT_SOID.to_le_bytes());
    body[0x08..0x0C].copy_from_slice(&1u32.to_le_bytes());
    body[0x10..0x18].copy_from_slice(&CHARACTER_SOID.to_le_bytes());
    let (enc, dec) = o.round_trip(&body, 0x9000).unwrap();
    let want =
        format!("e880000000000000c74400000000000407{}{}", "0001".repeat(27), "8000".repeat(5));
    assert_eq!(hexs(&enc.payload), want);
    assert_eq!(dec.qword(0x00), ACCOUNT_SOID);
    assert_eq!(dec.qword(0x10), CHARACTER_SOID);
}

/// The MINIMAL roster body, built straight from the bit grammar (presence bit, then the value
/// inline MSB-first) rather than from the encoder - so the grammar itself is under test.
#[test]
fn roster_minimal_body_matches_the_bit_grammar() {
    need_eboot!();
    let mut o = ObjectOracle::new(0x8080_1951).unwrap();
    let payload =
        pack_bits(&format!("1{ACCOUNT_SOID:064b}1{:04b}{CHARACTER_SOID:064b}00", 1));
    assert_eq!(hexs(&payload), "e880000000000000c74400000000000404");
    let d = o.decode(&payload).unwrap();
    assert_eq!(d.bits, 136);
    assert_eq!(d.qword(0x00), ACCOUNT_SOID);
    assert_eq!(u32::from_le_bytes(d.object[8..12].try_into().unwrap()), 1);
    assert_eq!(d.qword(0x10), CHARACTER_SOID);
}

/// The heaviest case the Python oracle pins: encoding a whole zeroed PROFILE emits **20,318 bytes /
/// 162,539 bits**, and decoding it back yields the identical C object.
///
/// This is the harness stress test as much as a vector. It is the run that exercises the RSAT
/// codec's dynamic-field **bitset** - the frame local that is `memset` twice per encode, and the
/// only reason a stack-overlapping `memset` is permitted at all. If the narrowed guard in
/// `object.rs` ever stops matching the real call sites, this test fails rather than the guard
/// silently widening.
#[test]
fn profile_full_encode_matches_the_pinned_20318_bytes() {
    need_eboot!();
    let mut o = ObjectOracle::new(0x8080_1743).unwrap();
    let body = vec![0u8; o.struct_size()];
    let (enc, _) = o.round_trip(&body, 0x40000).unwrap();
    assert_eq!(enc.payload.len(), 20_318);
    assert_eq!(enc.bits, 162_539);
}

// ───────────────────────── plumbing controls ─────────────────────────

/// A tag that is not an anchored descriptor must be refused up front, not decoded into noise.
#[test]
fn an_unknown_tag_is_refused() {
    need_eboot!();
    assert!(matches!(ObjectOracle::new(0xDEAD_BEEF), Err(OracleError::UnknownTag(0xDEAD_BEEF))));
}

/// `tag_slot` reproduces the client's own arithmetic. ⚠ Keying on `tag & 0x1fff` alone collapses
/// the 4,555 descriptors onto 3,873 indices with 656 collisions - the registry would then resolve
/// the *wrong* descriptor for one tag in seven, silently.
#[test]
fn tag_slot_is_not_just_the_low_13_bits() {
    use d1_eboot::descriptors;
    use d1_oracle::registry::tag_slot;
    need_eboot!();
    let img = d1_eboot::image::Image::open_reference().unwrap();
    let idx = descriptors::index(&img).unwrap();
    let tags: Vec<u32> = idx.tags().collect();
    let full: std::collections::HashSet<(u32, u32)> = tags.iter().map(|&t| tag_slot(t)).collect();
    let low: std::collections::HashSet<u32> = tags.iter().map(|&t| t & 0x1FFF).collect();
    assert_eq!(full.len(), tags.len(), "the client's (slot, index) pair must be injective");
    assert!(low.len() < tags.len(), "the low 13 bits alone collide - that is the point");
}

/// The message oracle drives a *different* codec, and the difference is a `u16` id plus a trailing
/// span-presence bit. Pinning the 503 body's leading `01f7` here makes "I used the wrong oracle"
/// visible: `0x01f7` is 503 big-endian, and no QueueZ object body carries it.
#[test]
fn the_message_codec_wraps_a_u16_id_the_object_codec_does_not() {
    need_eboot!();
    let mut o = MessageOracle::new(0x8080_1825, 503, 0x8080_1726).unwrap();
    let (h, obj) = o.make_handle(&[], &[]).unwrap();
    o.write_object_u64(obj, 0x08, ACCOUNT_SOID).unwrap();
    o.write_object_u64(obj, 0x18, 0x7FFF_FFFF_FFFF_FFFF).unwrap();
    let w = o.encode(h, 0x20000).unwrap();
    assert_eq!(&w[..2], &[0x01, 0xF7], "u16 message id 503, big-endian, ahead of the body");
    assert_eq!(hexs(&w), control::PIN_503);
}
