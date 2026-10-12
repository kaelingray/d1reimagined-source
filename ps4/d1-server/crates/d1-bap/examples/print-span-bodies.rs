// SPDX-License-Identifier: GPL-2.0-or-later
//! Print the QueueZ span payloads d1-bap already builds, as hex, for a given account SOID.
//!
//! These are the bytes that go INSIDE the message-206 envelope span. They are printed by the
//! owning crate on purpose: re-deriving them in Python would be a second implementation of a
//! format that already lives here, which the no-format-twice rule forbids.
//!
//!   cargo run -p d1-bap --example print-span-bodies -- [SOID_HEX]
fn main() {
    let acct = std::env::args()
        .nth(1)
        .map(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).expect("bad SOID"))
        .unwrap_or(0xD100_0000_0000_0001);
    let chr = 0xD100_0000_0000_0101u64;
    let empty = d1_bap::queuez_empty_family_body_for(d1_bap::QUEUEZ_FAMILY_ACCOUNT, acct);
    // Minimal account RSAT: presence bit + u64be SOID, the same bit grammar character_rsat uses.
    // 0x57CC80 requires the first qword of the copied payload to be non-zero.
    let acct_rsat: Vec<u8> = vec![0xe8, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80];
    let obj = d1_bap::queuez_object_body(
        d1_bap::QUEUEZ_FAMILY_ACCOUNT, acct, d1_bap::ACCOUNT_INSTANCE_TYPE, acct, &acct_rsat);
    let chr_body = d1_bap::queuez_character_body(
        acct, chr, d1_bap::Identity::TITAN_HUMAN_FEMALE);
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    println!("acct_soid  {acct:#018x}");
    println!("empty_fam4 {:3}B {}", empty.len(), hex(&empty));
    println!("object_f4  {:3}B {}", obj.len(), hex(&obj));
    println!("character  {:3}B {}", chr_body.len(), hex(&chr_body));

    // ── P1: the character-creation shapes. Printed here so the OFFLINE evidence is reproducible:
    //    each of these is meant to be fed straight to `d1-oracle object|message --decode`, which is
    //    the only check that can tell "field absent" from "field present with value 0".
    let prof = 0xD100_0000_0000_0201u64;
    let acct_with = d1_bap::account_rsat(acct, &[], prof);
    let acct_zero = d1_bap::account_rsat(acct, &[], 0);
    let acct_none = d1_bap::account_rsat_no_profile(acct, &[]);
    let f4_baseline =
        d1_bap::queuez_family4_baseline_body_with_unlocks(acct, &[], prof, None, None, &[]);
    let f4_only = d1_bap::queuez_family4_account_only_body(acct);
    let f3_empty = d1_bap::queuez_family3_empty_roster_body(acct);
    println!();
    println!("acct_rsat(prof)      {:3}B {}   <- field 3 PRESENT  (+0x68 set)", acct_with.len(), hex(&acct_with));
    println!("acct_rsat(0)         {:3}B {}   <- field 3 present, value 0", acct_zero.len(), hex(&acct_zero));
    println!("acct_rsat_no_profile {:3}B {}   <- field 3 ABSENT   (the P1a shape)", acct_none.len(), hex(&acct_none));
    println!("family4 baseline     {:3}B {}", f4_baseline.len(), hex(&f4_baseline));
    println!("family4 ACCOUNT-ONLY {:3}B {}", f4_only.len(), hex(&f4_only));
    println!("family3 empty roster {:3}B {}", f3_empty.len(), hex(&f3_empty));
    println!();
    println!("206 body, no span      {}", hex(&d1_bap::fetch_family_response_body(None)));
    println!("206 body, family-3 span  {}", hex(&d1_bap::fetch_family_response_body(Some(&f3_empty))));
    println!("206 body, family-4 span  {}", hex(&d1_bap::fetch_family_response_body(Some(&f4_only))));

    // ── P2: the SECOND family-4 delivery - the availability-cache invalidation. Printed as a
    //    matched pair with the baseline it follows, because the whole claim is a comparison:
    //    same objects, different header. Feed the account payloads to
    //    `d1-oracle object --tag 0x808018c7 --decode` to see what the serial actually moves.
    let base = d1_bap::queuez_family4_baseline_body(acct, &[chr], prof);
    let ping = d1_bap::queuez_family4_delta_ping(acct, 1);
    let restamp = d1_bap::queuez_span_as_delta(&base, 1).expect("baseline must walk");
    let bumped = d1_bap::queuez_span_replace_object_payload(
        &base, 0, d1_bap::ACCOUNT_INSTANCE_TYPE,
        &d1_bap::account_rsat_with_serial(acct, &[chr], prof, 7),
    )
    .and_then(|s| d1_bap::queuez_span_as_delta(&s, 1))
    .expect("splice must land on the account object");
    println!();
    println!("P2 family4 BASELINE  {:3}B {}", base.len(), hex(&base));
    println!("P2 delta PING        {:3}B {}   <- 0 objects, purge-free", ping.len(), hex(&ping));
    println!("P2 delta RE-STAMP    {:3}B {}   <- same objects, version 1, flags 0x00",
             restamp.len(), hex(&restamp));
    println!("P2 delta SERIAL=7    {:3}B {}   <- account +0x60 = 7 -> 0x56B5F0 memcmp != 0",
             bumped.len(), hex(&bumped));
    println!("   restamp objects == baseline objects: {}",
             d1_bap::queuez_span_objects(&restamp) == d1_bap::queuez_span_objects(&base));
    println!("   account payload  base {} / serial7 {}",
             hex(&d1_bap::account_rsat(acct, &[chr], prof)),
             hex(&d1_bap::account_rsat_with_serial(acct, &[chr], prof, 7)));
}
