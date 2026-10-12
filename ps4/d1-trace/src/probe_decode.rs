// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! Native decoder for the fixed-size records emitted by shadPS4's D1 account bootstrap tracer.
//!
//! This is the Rust port of `capture-rig/decode-account-trace.py`, absorbed so the shipped
//! `d1-trace` binary no longer needs Python at runtime. The port is BYTE-FAITHFUL: `d1-trace
//! decode` (human mode) and `d1-trace decode --json` reproduce the Python tool's output exactly,
//! including its refusals (bad magic, wrong version, wrong encoded size, non-multiple file length
//! all exit 2 with the same message shapes).
//!
//! ## The F4 anchor labels live HERE now
//!
//! `ingest::build_bap_anchors` scrapes `received_type=` / `inner_type=` out of the `explain` prose.
//! When that prose came from the Python script, a cosmetic reword over there silently took 1061
//! anchors to 0 (measured; see `controls.rs`). The producer is now this module, and [`explain`]
//! emits those labels FROM the same [`ANCHOR_TYPE_LABELS`] / [`ANCHOR_SEQ_LABELS`] constants the
//! scraper consumes, so producer and consumer can no longer drift apart. The `C-ANCHOR-*` controls
//! stay as belt and braces.
//!
//! ⚠ The 232-byte layout is still ALSO described by `capture-rig/ready-byte-report.py` and
//! `capture-rig/gate-check.py` (see the correction in `model.rs`); this port does not add a new
//! description so much as replace the one `d1-trace` already depended on at runtime.

use std::fmt::Write as _;

use serde::Serialize;

pub const MAGIC: u32 = 0x52543144;
pub const VERSION: u16 = 2;
/// `<IHHII15Q48s48s` = 4+2+2+4+4 + 15*8 + 48 + 48.
pub const RECORD_SIZE: usize = 232;

/// The labels `ingest::build_bap_anchors` scrapes. [`explain`] emits them from these constants,
/// so a reword changes both sides of the join at once (F4, structurally closed).
pub const ANCHOR_TYPE_LABELS: [&str; 2] = ["received_type=", "inner_type="];
pub const ANCHOR_SEQ_LABELS: [&str; 2] = ["received_sequence=", "inner_sequence="];
/// A probe whose presence PROVES the labels above must produce at least one anchor.
pub const ANCHOR_WITNESS_PROBE: &str = "secured_rx_dispatch_entry";

pub const REGISTER_NAMES: [&str; 15] = [
    "guest_vaddr",
    "rip",
    "rax",
    "rbx",
    "rcx",
    "rdx",
    "rsi",
    "rdi",
    "rbp",
    "rsp",
    "r12",
    "r13",
    "r14",
    "r15",
    "rflags",
];

// Register indices into `Record::regs`, in REGISTER_NAMES order.
const GUEST_VADDR: usize = 0;
const RIP: usize = 1;
const RAX: usize = 2;
const RBX: usize = 3;
const RCX: usize = 4;
const RDX: usize = 5;
const RSI: usize = 6;
const RDI: usize = 7;
const R12: usize = 10;
const R13: usize = 11;
const R14: usize = 12;
const R15: usize = 13;

/// Probe id → name, the same catalogue `decode-account-trace.py` carries.
pub fn probe_name(id: u16) -> String {
    let name = match id {
        0 => "armed",
        1 => "request_correlator",
        2 => "response_thunk",
        3 => "consumer_entry",
        4 => "state_check",
        5 => "identity_result",
        6 => "pre_write",
        7 => "post_write",
        8 => "queuez_serializer_entry",
        9 => "queuez_object_result",
        10 => "policy_dispatch",
        11 => "action_collection",
        12 => "rearm_point",
        13 => "envelope_decoder",
        14 => "substate_b",
        15 => "acct_ready",
        16 => "profile_bound",
        17 => "acct_object",
        18 => "profile_match",
        19 => "update_entry",
        20 => "substate_check",
        21 => "acctobj_entry",
        22 => "acctobj_manager",
        23 => "acctobj_record",
        24 => "acctobj_result",
        // The ready-byte chain (§26.41).
        25 => "producer_entry",
        26 => "producer_result",
        27 => "ready_setter",
        28 => "registry_find",
        29 => "group4_request",
        30 => "class4_added",
        // Client BAP receive dispatch - the research lane's verified catalog.
        31 => "bap_rx_dispatch_entry",
        32 => "bap_expected_type_result",
        33 => "bap_sequence_candidate",
        34 => "bap_push_type_result",
        35 => "bap_push_matched",
        36 => "bap_push_consumer_result",
        37 => "bap_no_recipient",
        38 => "bap_request_matched",
        39 => "bap_request_consumer_result",
        40 => "bap_receive_success",
        41 => "bap_receive_rearm_pump",
        42 => "secured_decrypt_entry",
        43 => "secured_decrypt_result",
        44 => "secured_decrypt_success",
        45 => "secured_decrypt_rearm_pump",
        46 => "secured_rx_dispatch_entry",
        47 => "secured_expected_type_result",
        48 => "secured_sequence_candidate",
        49 => "secured_push_type_result",
        50 => "secured_push_matched",
        51 => "secured_push_consumer_result",
        52 => "secured_no_recipient",
        53 => "secured_request_matched",
        54 => "secured_consumer_result",
        55 => "secured_receive_success",
        56 => "secured_receive_rearm_pump",
        57 => "queuez_push_entry",
        58 => "queuez_family_update_result",
        59 => "queuez_family_objects_dispatch",
        60 => "queuez_push_rearm_pump",
        61 => "queuez_object_apply_entry",
        62 => "queuez_object_slot_result",
        63 => "queuez_rsat_decode_returned",
        64 => "queuez_object_key_check",
        65 => "queuez_object_key_mismatch",
        66 => "queuez_object_updated",
        67 => "queuez_object_added",
        68 => "queuez_object_apply_rearm_pump",
        // destination availability builder 0x260200
        69 => "avail_entry",
        70 => "avail_fail_a",
        71 => "avail_fail_b",
        72 => "avail_fail_c",
        73 => "avail_fail_d",
        74 => "avail_fail_e",
        75 => "avail_verdict_read",
        76 => "avail_rearm_pump",
        // item / content gate: slot loop 0x1087710 and gate 0x555730
        77 => "item_loop_head",
        78 => "item_slot_resolve",
        79 => "item_gate_call",
        80 => "item_gate_result",
        81 => "item_loop_pump",
        82 => "item_tag_resolved",
        83 => "item_load_state_primary",
        84 => "item_load_state_secondary",
        85 => "item_gate_verdict",
        // world launch: the change-world seam (0x753540)
        86 => "world_launch_command",
        87 => "world_launch_return",
        88 => "world_change_entry",       // THE PRIMARY: 0x753540 callee entry, any caller
        89 => "world_launch_direct_call", // 0x754484, inside 0x754460
        90 => "world_launch_direct_return",
        other => return format!("unknown_{other}"),
    };
    name.to_string()
}

/// One decoded 232-byte record.
#[derive(Debug, Clone)]
pub struct Record {
    pub sequence: u32,
    pub probe_id: u16,
    pub probe: String,
    /// In [`REGISTER_NAMES`] order.
    pub regs: [u64; 15],
    pub sample_a: [u8; 48],
    pub sample_b: [u8; 48],
}

// ---- little helpers mirroring the Python readers -----------------------------------------------

fn u64le(d: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(d[off..off + 8].try_into().unwrap())
}

fn u32le(d: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(d[off..off + 4].try_into().unwrap())
}

fn u16le(d: &[u8], off: usize) -> u16 {
    u16::from_le_bytes(d[off..off + 2].try_into().unwrap())
}

fn i32le(d: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(d[off..off + 4].try_into().unwrap())
}

fn u16be(d: &[u8], off: usize) -> u16 {
    u16::from_be_bytes(d[off..off + 2].try_into().unwrap())
}

fn u32be(d: &[u8], off: usize) -> u32 {
    u32::from_be_bytes(d[off..off + 4].try_into().unwrap())
}

/// `bytes.hex(" ")`: lowercase pairs, single-space separated.
pub fn compact_hex(d: &[u8]) -> String {
    let mut s = String::with_capacity(d.len() * 3);
    for (i, b) in d.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn ascii_hint(d: &[u8]) -> String {
    d.iter()
        .map(|&b| if (32..127).contains(&b) { b as char } else { '.' })
        .collect()
}

/// `bytes.decode("ascii", "replace")` after splitting at the first NUL.
fn ascii_replace_to_nul(d: &[u8]) -> String {
    let end = d.iter().position(|&b| b == 0).unwrap_or(d.len());
    d[..end]
        .iter()
        .map(|&b| if b < 128 { b as char } else { '\u{FFFD}' })
        .collect()
}

/// Python's `f"{v:#010x}"` for a value that may be negative (envelope decoder guest-vaddr math).
fn py_hex_w10(v: i128) -> String {
    let neg = v < 0;
    let digits = format!("{:x}", v.unsigned_abs());
    let prefix = if neg { 3 } else { 2 }; // "-0x" / "0x"
    let pad = 10usize.saturating_sub(prefix + digits.len());
    format!("{}0x{}{}", if neg { "-" } else { "" }, "0".repeat(pad), digits)
}

fn py_bool(b: bool) -> &'static str {
    if b {
        "True"
    } else {
        "False"
    }
}

// ---- record reader -----------------------------------------------------------------------------

/// Decode a whole trace file, refusing exactly where the Python refused.
///
/// `origin` is the path string used in the length-mismatch message (the Python interpolated the
/// path it was given).
pub fn read_records(blob: &[u8], origin: &str) -> Result<Vec<Record>, String> {
    if blob.len() % RECORD_SIZE != 0 {
        return Err(format!(
            "{origin}: {} bytes is not a multiple of record size {RECORD_SIZE}",
            blob.len()
        ));
    }
    let mut records = Vec::with_capacity(blob.len() / RECORD_SIZE);
    for offset in (0..blob.len()).step_by(RECORD_SIZE) {
        let r = &blob[offset..offset + RECORD_SIZE];
        let magic = u32le(r, 0);
        let version = u16le(r, 4);
        let probe_id = u16le(r, 6);
        let record_size = u32le(r, 8);
        let sequence = u32le(r, 12);
        if magic != MAGIC {
            return Err(format!("record at {offset:#x}: bad magic {magic:#x}"));
        }
        if version != VERSION {
            return Err(format!("record at {offset:#x}: unsupported version {version}"));
        }
        if record_size as usize != RECORD_SIZE {
            return Err(format!(
                "record at {offset:#x}: encoded size {record_size}, expected {RECORD_SIZE}"
            ));
        }
        let mut regs = [0u64; 15];
        for (i, reg) in regs.iter_mut().enumerate() {
            *reg = u64le(r, 16 + i * 8);
        }
        let mut sample_a = [0u8; 48];
        sample_a.copy_from_slice(&r[136..184]);
        let mut sample_b = [0u8; 48];
        sample_b.copy_from_slice(&r[184..232]);
        records.push(Record {
            sequence,
            probe_id,
            probe: probe_name(probe_id),
            regs,
            sample_a,
            sample_b,
        });
    }
    records.sort_by_key(|r| r.sequence);
    Ok(records)
}

// ---- explain -----------------------------------------------------------------------------------

/// Per-probe interpretation, byte-identical to the Python `explain()`.
pub fn explain(record: &Record) -> Vec<String> {
    let probe = record.probe.as_str();
    let regs = &record.regs;
    let a: &[u8] = &record.sample_a;
    let b: &[u8] = &record.sample_b;
    let mut lines = vec![format!(
        "#{:02} {:<20} guest={:#010x} rip={:#018x}",
        record.sequence, probe, regs[GUEST_VADDR], regs[RIP]
    )];

    match probe {
        "armed" => {
            let serial = ascii_replace_to_nul(a);
            let version = ascii_replace_to_nul(b);
            lines.push(format!(
                "     eboot_base={:#x} image_size={:#x}",
                regs[RAX], regs[RBX]
            ));
            lines.push(format!("     title={serial} app_version={version}"));
        }
        "request_correlator" | "response_thunk" => {
            lines.push(format!(
                "     rsi_handle={:#x} rcx_handle={:#x}",
                regs[RSI], regs[RCX]
            ));
        }
        "consumer_entry" => {
            lines.push(format!(
                "     manager={:#x} response={:#x} type={} soid@+8={:#018x}",
                regs[RDI],
                regs[RSI],
                a[0],
                u64le(a, 8)
            ));
            lines.push(format!("     correlated_identity={}", compact_hex(b)));
            lines.push(format!("     identity_ascii={}", ascii_hint(b)));
        }
        "state_check" => {
            lines.push(format!(
                "     record={:#x} state={} platform_user_unaligned={:#018x} soid@+10={:#018x}",
                regs[RBX],
                a[0],
                u64le(a, 1),
                u64le(a, 0x10)
            ));
            lines.push(format!("     record_bytes={}", compact_hex(a)));
        }
        "identity_result" => {
            let local = &a[..16];
            let correlated = &b[..16];
            lines.push(format!(
                "     AL={} equal_bytes={} record={:#x}",
                regs[RAX] & 0xff,
                py_bool(local == correlated),
                regs[RBX]
            ));
            lines.push(format!(
                "     local      ={}  {}",
                compact_hex(local),
                ascii_hint(local)
            ));
            lines.push(format!(
                "     correlated ={}  {}",
                compact_hex(correlated),
                ascii_hint(correlated)
            ));
        }
        "pre_write" => {
            lines.push(format!(
                "     record={:#x} old_state={} old_soid={:#018x} response_soid={:#018x}",
                regs[RBX],
                a[0],
                u64le(a, 0x10),
                u64le(b, 8)
            ));
        }
        "post_write" => {
            lines.push(format!(
                "     record={:#x} state={} soid@+10={:#018x}",
                regs[RBX],
                a[0],
                u64le(a, 0x10)
            ));
            lines.push(format!("     record_bytes={}", compact_hex(a)));
        }
        "queuez_serializer_entry" => {
            lines.push(format!(
                "     arg1={:#x} arg2={:#x} arg3={:#x} caller={:#018x}",
                regs[RDI],
                regs[RSI],
                regs[RDX],
                u64le(b, 0)
            ));
            lines.push(format!(
                "     handle_type={:#010x} handle_object={:#018x} handle_bytes={}",
                u32le(a, 0),
                u64le(a, 8),
                compact_hex(&a[..16])
            ));
        }
        "policy_dispatch" => {
            lines.push(format!(
                "     policy={:#x} collection={:#x} record={:#x} message_id={:#06x}",
                regs[RDI],
                regs[R14],
                regs[RBX],
                u16le(b, 0)
            ));
            lines.push(format!(
                "     producer_return={:#018x} saved_rbp={:#018x}",
                u64le(a, 8),
                u64le(a, 0)
            ));
            lines.push(format!("     record_bytes={}", compact_hex(&b[..24])));
        }
        "action_collection" => {
            lines.push(format!(
                "     collection={:#x} message_id={:#06x} pointer={:#018x} count={}",
                regs[RBX],
                regs[R12] & 0xFFFF,
                u64le(a, 0x10),
                i32le(a, 0x18)
            ));
            lines.push(format!("     producer_return={:#018x}", u64le(b, 8)));
            lines.push(format!("     collection_bytes={}", compact_hex(&a[..32])));
        }
        "envelope_decoder" => {
            let base: i128 = 0x800000000;
            let vt = regs[RAX];
            let dec = u64le(a, 0x18);
            lines.push(format!(
                "     codec vtable={:#x} (guest {})  DECODER=[vt+0x18]={:#x}  GUEST VADDR {}",
                vt,
                py_hex_w10(vt as i128 - base),
                dec,
                py_hex_w10(dec as i128 - base)
            ));
            lines.push(format!("     vtable_slots={}", compact_hex(&a[..48])));
        }
        "substate_check" | "update_entry" => {
            // Python: `u32le(a, 0x30) if len(a) > 0x33 else None` - len(a) is 48 < 0x33+1, so
            // the value is ALWAYS None. Preserved exactly (this is a port, not a redesign).
            lines.push(format!(
                "     h={:#x}  h[+0x30]=None  (need 7)   h[+0x2c]={}",
                regs[RDI],
                u32le(a, 0x2c)
            ));
            // Python slice a[0x20:0x38] clamps to the 48-byte sample.
            lines.push(format!("     h[0x20:0x38]={}", compact_hex(&a[0x20..48])));
        }
        "acct_ready" | "profile_bound" | "profile_match" => {
            lines.push(format!(
                "     AL={}  ->  {}",
                regs[RAX] & 0xFF,
                if regs[RAX] & 0xFF != 0 {
                    "TRUE"
                } else {
                    "*** FALSE - THIS IS THE BLOCKER ***"
                }
            ));
        }
        "acct_object" => {
            lines.push(format!(
                "     account_object={:#x}  ->  {}",
                regs[RAX],
                if regs[RAX] != 0 {
                    "OK"
                } else {
                    "*** NULL - THIS IS THE BLOCKER ***"
                }
            ));
        }
        "substate_b" => {
            lines.push(format!(
                "     handler={:#x} rbx={:#x} (poll)",
                regs[RDI], regs[RBX]
            ));
        }
        "queuez_object_result" => {
            lines.push(format!(
                "     accessor_result={:#018x} caller={:#018x}",
                regs[RAX],
                u64le(b, 0)
            ));
            lines.push(format!(
                "     local_handle_type={:#010x} local_handle_object={:#018x} handle_bytes={}",
                u32le(a, 0),
                u64le(a, 8),
                compact_hex(&a[..16])
            ));
        }
        "secured_decrypt_entry" => {
            lines.push(format!(
                "     connection={:#018x} MODE1_GUEST_CONTROL=FIRED",
                regs[RDI]
            ));
        }
        "secured_decrypt_result" | "secured_decrypt_success" => {
            let verdict = regs[RAX] & 0xFF;
            if probe == "secured_decrypt_result" && verdict == 0 {
                lines.push(format!(
                    "     AL=0 GCM_VERIFY_FAILED candidate_buffer={}",
                    compact_hex(&a[..16])
                ));
            } else {
                // F4: the anchor labels come from the shared constants the scraper reads.
                lines.push(format!(
                    "     AL={verdict} {}{:#06x} {}{} plaintext={}",
                    ANCHOR_TYPE_LABELS[1],
                    u16be(a, 0),
                    ANCHOR_SEQ_LABELS[1],
                    u32be(a, 2),
                    compact_hex(&a[..16])
                ));
            }
        }
        "secured_rx_dispatch_entry" => {
            lines.push(format!(
                "     {}{:#06x} {}{}",
                ANCHOR_TYPE_LABELS[0],
                u16le(a, 0),
                ANCHOR_SEQ_LABELS[0],
                u32le(a, 2)
            ));
        }
        "secured_expected_type_result"
        | "secured_sequence_candidate"
        | "secured_push_type_result"
        | "secured_push_matched"
        | "secured_push_consumer_result"
        | "secured_no_recipient"
        | "secured_request_matched"
        | "secured_consumer_result"
        | "secured_receive_success" => {
            lines.push(format!(
                "     AL={} {}{:#06x} {}{} r12={:#018x}",
                regs[RAX] & 0xff,
                ANCHOR_TYPE_LABELS[0],
                u16le(a, 0),
                ANCHOR_SEQ_LABELS[0],
                u32le(a, 2),
                regs[R12]
            ));
        }
        "queuez_family_update_result" | "queuez_family_objects_dispatch" => {
            lines.push(format!(
                "     AL={} family={} root={:#018x} flags={:#04x} object_count={}",
                regs[RAX] & 0xff,
                regs[R15] & 0xffffffff,
                u64le(a, 0),
                b[0],
                u32le(b, 1)
            ));
        }
        "queuez_object_apply_entry" => {
            lines.push(format!(
                "     family={} root={:#018x} instance={:#010x} soid={:#018x} rsat_length={}",
                regs[RSI] & 0xffffffff,
                u64le(a, 0),
                u32le(b, 0),
                u64le(b, 4),
                u32le(b, 12)
            ));
        }
        "queuez_object_slot_result" => {
            lines.push(format!(
                "     slot={:#018x} expected_soid={:#018x} slot_key={:#018x}",
                regs[RAX],
                u64le(b, 0),
                u64le(a, 0)
            ));
        }
        "queuez_rsat_decode_returned"
        | "queuez_object_key_check"
        | "queuez_object_key_mismatch"
        | "queuez_object_updated"
        | "queuez_object_added" => {
            lines.push(format!(
                "     slot={:#018x} decoded_key={:#018x} expected_soid={:#018x} key_match={}",
                regs[R13],
                u64le(a, 0),
                u64le(b, 0),
                py_bool(u64le(a, 0) == u64le(b, 0))
            ));
        }
        // ---- destination availability builder 0x260200 ----------------------------------------
        "avail_entry" => {
            lines.push(format!(
                "     ctx={:#018x} ctx+0x28={:#018x} ctx+0x18={:#018x}",
                regs[RDI],
                u64le(a, 0x28),
                u64le(a, 0x18)
            ));
        }
        "avail_fail_a" | "avail_fail_b" | "avail_fail_c" | "avail_fail_d" | "avail_fail_e" => {
            // These five are the SAME instruction at five addresses and they fall through rather
            // than return, so print the address plainly -- it is the only thing that
            // distinguishes them.
            lines.push(format!(
                "     FAILURE SITE {:#010x} (verdict byte set to 0)",
                regs[GUEST_VADDR]
            ));
        }
        "avail_verdict_read" => {
            lines.push(format!(
                "     reached the full-path verdict read (NOT the early 0x26042E exit); AL will be {}",
                a[0x1f]
            ));
        }
        "avail_rearm_pump" => {
            // RAX's low byte is about to become header[1]. sample_a is the header BEFORE the store.
            lines.push(format!(
                "     VERDICT AL={} header_before={} header_ptr={:#018x}",
                regs[RAX] & 0xff,
                compact_hex(&a[..3]),
                regs[R13]
            ));
        }
        // ---- item / content gate --------------------------------------------------------------
        "item_loop_head" => {
            lines.push(format!(
                "     slot_index={} slot_base={:#018x}",
                u32le(a, 0x10),
                u64le(a, 8)
            ));
        }
        "item_slot_resolve" | "item_gate_call" => {
            // The 32-byte slot: u16 content index at +8, object key at +0x10.
            let slot_u16 = u16le(a, 8);
            lines.push(format!(
                "     slot_u16={:#06x} slot_key={:#018x} registry_element={:#018x} {}",
                slot_u16,
                u64le(a, 0x10),
                regs[RAX],
                if u64le(a, 0x10) == 0 { "EMPTY SLOT" } else { "" }
            ));
        }
        "item_tag_resolved" => {
            let tag = regs[RAX] & 0xFFFFFFFF;
            lines.push(format!(
                "     world_tag={:#010x} {}",
                tag,
                if tag == 0xFFFFFFFF {
                    "UNRESOLVED (0xFFFFFFFF)"
                } else {
                    "resolved"
                }
            ));
        }
        "item_load_state_primary" | "item_load_state_secondary" => {
            let state = regs[RAX] & 0xFFFFFFFF;
            lines.push(format!(
                "     load_state={} {}",
                state,
                if state == 2 {
                    "== 2 REQUIRED: PASS"
                } else {
                    "REQUIRED 2: FAIL"
                }
            ));
        }
        "item_gate_verdict" => {
            lines.push(format!("     gate_verdict_r15b={}", regs[R15] & 1));
        }
        "world_launch_command" | "world_change_entry" | "world_launch_direct_call" => {
            lines.push(format!(
                "     command={:#018x} (0x18f8 bytes -> .blob sidecar) head={}",
                regs[RDI],
                compact_hex(&a[..16])
            ));
        }
        "world_launch_return" => {
            lines.push(format!(
                "     returned from 0x753540; AL={}",
                regs[RAX] & 0xff
            ));
        }
        "item_gate_result" => {
            // sample_b starts at RBP-0x50 and the out flag lives at RBP-0x49, i.e. offset 7.
            lines.push(format!(
                "     caller_out_flag={} (nonzero = item not fully loaded)",
                b[7]
            ));
        }
        _ => {}
    }
    lines
}

// ---- diagnoses ---------------------------------------------------------------------------------

pub fn diagnosis(records: &[Record]) -> String {
    let seen = |name: &str| records.iter().any(|r| r.probe == name);
    if !seen("armed") {
        return "INVALID: tracer never emitted its armed record".into();
    }
    if !seen("request_correlator") {
        return "REQUEST_BINDING: 0x571890 never ran".into();
    }
    if !seen("response_thunk") {
        return "RESPONSE_BINDING: 0x571890 ran but 0x571940 never ran".into();
    }
    if !seen("consumer_entry") {
        return "MANAGER_DISPATCH: 0x571940 ran but 0x5858C0 was not entered".into();
    }
    if !seen("state_check") {
        return "CONSUMER_EARLY_EXIT: 0x5858C0 entered but selected no non-empty record".into();
    }
    if !seen("identity_result") {
        return "PRE_COMPARE_EXIT: record was inspected but local identity validation returned early"
            .into();
    }
    let comparison = records.iter().find(|r| r.probe == "identity_result").unwrap();
    if comparison.regs[RAX] & 0xFF == 0 {
        return "IDENTITY_MISMATCH: 0x2B7840 returned false".into();
    }
    if !seen("pre_write") {
        return "POST_COMPARE_CONTROL: identity matched but the SOID write path was not reached"
            .into();
    }
    if !seen("post_write") {
        return "WRITE_PATH: pre-write executed but the record copy did not complete".into();
    }
    let last = records.iter().find(|r| r.probe == "post_write").unwrap();
    let state = last.sample_a[0];
    let soid = u64le(&last.sample_a, 0x10);
    if state == 3 && soid != 0 {
        return format!("TRANSITION_CONFIRMED: record is state 3 with SOID {soid:#018x}");
    }
    format!("WRITE_INEFFECTIVE: post-write record is state {state}, SOID {soid:#018x}")
}

pub fn queuez_diagnosis(records: &[Record]) -> String {
    let entry = records.iter().find(|r| r.probe == "queuez_serializer_entry");
    let result = records.iter().find(|r| r.probe == "queuez_object_result");
    let Some(entry) = entry else {
        return "NOT_REACHED: 0x570A40 never ran".into();
    };
    let handle_type = u32le(&entry.sample_a, 0);
    let handle_object = u64le(&entry.sample_a, 8);
    let caller = u64le(&entry.sample_b, 0);
    let Some(result) = result else {
        return format!(
            "ACCESSOR_NOT_RETURNED: caller {caller:#018x} supplied handle \
             {{type={handle_type:#x}, object={handle_object:#018x}}}"
        );
    };
    let returned = result.regs[RAX];
    let local_type = u32le(&result.sample_a, 0);
    let local_object = u64le(&result.sample_a, 8);
    if (local_type, local_object) != (handle_type, handle_object) {
        return format!(
            "HANDLE_COPY_MISMATCH: entry {{type={handle_type:#x}, \
             object={handle_object:#018x}}}, local {{type={local_type:#x}, \
             object={local_object:#018x}}}, accessor={returned:#018x}"
        );
    }
    if returned != handle_object {
        return format!(
            "ACCESSOR_MISMATCH: caller {caller:#018x} supplied object {handle_object:#018x}, \
             accessor returned {returned:#018x}"
        );
    }
    if returned == 0 {
        return format!(
            "NULL_OBJECT_CONFIRMED: caller {caller:#018x} supplied handle type {handle_type:#x} \
             with a NULL object"
        );
    }
    format!(
        "OBJECT_RESOLVED: caller {caller:#018x} supplied handle type {handle_type:#x}, object \
         {returned:#018x}"
    )
}

/// Name the producer of the empty argument collection, and measure the count directly.
pub fn producer_diagnosis(records: &[Record]) -> String {
    let mut lines: Vec<String> = Vec::new();
    let dispatch = records.iter().find(|r| r.probe == "policy_dispatch");
    let actions: Vec<&Record> = records.iter().filter(|r| r.probe == "action_collection").collect();
    let action = actions.first();

    if let Some(dispatch) = dispatch {
        // sample_a is the frame: +0x00 saved RBP, +0x08 the caller's return address. The tail
        // call at 0x560D27 discards 0x560CF0's frame, so this names 0x560CF0's OWN caller.
        let caller = u64le(&dispatch.sample_a, 8);
        let message_id = u16le(&dispatch.sample_b, 0);
        lines.push(format!(
            "POLICY_DISPATCH: message id {message_id:#06x} ({message_id}), collection @ \
             {:#018x}, producer returns to {caller:#018x}",
            dispatch.regs[R14]
        ));
    } else {
        lines.push("POLICY_DISPATCH: 0x560D0F never ran (no policy virtual dispatched)".into());
    }

    let Some(action) = action else {
        lines.push(
            "ACTION_COLLECTION: 0x5700C8 never ran - 0x56FFB0's 0x541850 or 0x1F70 gate rejected \
             the dispatch, so the crash did not come from this path"
                .into(),
        );
        return lines.join("\n");
    };

    let pointer = u64le(&action.sample_a, 0x10);
    let count = i32le(&action.sample_a, 0x18);
    let caller = u64le(&action.sample_b, 8);
    let verdict = if count <= 0 {
        "EMPTY_COLLECTION_CONFIRMED"
    } else {
        "COLLECTION_POPULATED"
    };
    lines.push(format!(
        "{verdict}: message id {:#06x}, collection @ {:#018x} = {{pointer={pointer:#018x}, \
         count={count}}}, producer returns to {caller:#018x}",
        action.regs[R12] & 0xFFFF,
        action.regs[RBX]
    ));
    if actions.len() > 1 {
        lines.push(format!(
            "  ({} dispatches captured; per-dispatch spans:)",
            actions.len()
        ));
        for r in &actions {
            lines.push(format!(
                "    msg {:#06x}  count={}  ptr={:#x}",
                r.regs[R12] & 0xFFFF,
                i32le(&r.sample_a, 0x18),
                u64le(&r.sample_a, 0x10)
            ));
        }
    }
    if count > 0 {
        lines.push(
            "  NOTE: count > 0 means this dispatch built a valid handle; the crashing call is a \
             later one and these one-shot probes captured the wrong iteration."
                .into(),
        );
    }
    lines.join("\n")
}

// ---- JSON output (matches `json.dumps(..., indent=2)` field-for-field) --------------------------

#[derive(Serialize)]
struct RegistersJson {
    guest_vaddr: u64,
    rip: u64,
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    rsp: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    rflags: u64,
}

#[derive(Serialize)]
struct RecordJson {
    sequence: u32,
    probe_id: u16,
    probe: String,
    registers: RegistersJson,
    sample_a: String,
    sample_b: String,
    explain: Vec<String>,
}

#[derive(Serialize)]
struct OutputJson {
    records: Vec<RecordJson>,
    diagnosis: String,
    queuez_diagnosis: String,
    producer_diagnosis: String,
}

fn hex_plain(d: &[u8]) -> String {
    let mut s = String::with_capacity(d.len() * 2);
    for b in d {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Python's `json.dumps` default is `ensure_ascii=True`: every non-ASCII character in a string is
/// `\uXXXX`-escaped (surrogate pairs above the BMP). serde_json emits raw UTF-8 instead, which
/// diverged byte-wise on an `armed` record whose title bytes were non-ASCII (the `"replace"`
/// decode puts U+FFFD in the explain line; Python printed `�`). Non-ASCII can only appear
/// inside JSON string literals here, so a whole-document character pass is exact.
fn ensure_ascii(s: &str) -> String {
    if s.is_ascii() {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let mut buf = [0u16; 2];
            for unit in c.encode_utf16(&mut buf) {
                let _ = write!(out, "\\u{unit:04x}");
            }
        }
    }
    out
}

/// The full `--json` document, matching the Python tool's output byte-for-byte (verified by an
/// executed golden diff over the pinned run; see the report in the porting session).
pub fn to_json(records: &[Record]) -> String {
    let out = OutputJson {
        records: records
            .iter()
            .map(|r| RecordJson {
                sequence: r.sequence,
                probe_id: r.probe_id,
                probe: r.probe.clone(),
                registers: RegistersJson {
                    guest_vaddr: r.regs[0],
                    rip: r.regs[1],
                    rax: r.regs[2],
                    rbx: r.regs[3],
                    rcx: r.regs[4],
                    rdx: r.regs[5],
                    rsi: r.regs[6],
                    rdi: r.regs[7],
                    rbp: r.regs[8],
                    rsp: r.regs[9],
                    r12: r.regs[10],
                    r13: r.regs[11],
                    r14: r.regs[12],
                    r15: r.regs[13],
                    rflags: r.regs[14],
                },
                sample_a: hex_plain(&r.sample_a),
                sample_b: hex_plain(&r.sample_b),
                explain: explain(r),
            })
            .collect(),
        diagnosis: diagnosis(records),
        queuez_diagnosis: queuez_diagnosis(records),
        producer_diagnosis: producer_diagnosis(records),
    };
    ensure_ascii(&serde_json::to_string_pretty(&out).expect("plain structs cannot fail to serialize"))
}

/// Human mode: exactly what the Python printed without `--json`.
pub fn to_human(records: &[Record]) -> String {
    let mut out = String::new();
    for r in records {
        out.push_str(&explain(r).join("\n"));
        out.push('\n');
    }
    let _ = write!(
        out,
        "\nDIAGNOSIS: {}\nQUEUEZ DIAGNOSIS: {}\nPRODUCER DIAGNOSIS:\n{}\n",
        diagnosis(records),
        queuez_diagnosis(records),
        producer_diagnosis(records)
    );
    out
}

// ---- test fixtures shared with `controls` ------------------------------------------------------

/// Build a syntactically valid record for tests and self-controls.
pub fn make_record_bytes(probe_id: u16, sequence: u32, sample_a: &[u8; 48]) -> [u8; RECORD_SIZE] {
    let mut r = [0u8; RECORD_SIZE];
    r[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    r[4..6].copy_from_slice(&VERSION.to_le_bytes());
    r[6..8].copy_from_slice(&probe_id.to_le_bytes());
    r[8..12].copy_from_slice(&(RECORD_SIZE as u32).to_le_bytes());
    r[12..16].copy_from_slice(&sequence.to_le_bytes());
    r[136..184].copy_from_slice(sample_a);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rx_entry_bytes() -> [u8; RECORD_SIZE] {
        // secured_rx_dispatch_entry with received_type=0x001f, received_sequence=7.
        let mut sample_a = [0u8; 48];
        sample_a[0..2].copy_from_slice(&0x001fu16.to_le_bytes());
        sample_a[2..6].copy_from_slice(&7u32.to_le_bytes());
        make_record_bytes(46, 9, &sample_a)
    }

    #[test]
    fn decodes_a_valid_record() {
        let bytes = rx_entry_bytes();
        let recs = read_records(&bytes, "test").unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].probe, "secured_rx_dispatch_entry");
        assert_eq!(recs[0].sequence, 9);
        let lines = explain(&recs[0]);
        assert_eq!(lines[1], "     received_type=0x001f received_sequence=7");
    }

    /// The anchor labels the ingest scraper consumes MUST be the ones explain() emits (F4,
    /// producer side).
    #[test]
    fn explain_emits_the_anchor_labels_the_scraper_reads() {
        let bytes = rx_entry_bytes();
        let recs = read_records(&bytes, "test").unwrap();
        let text = explain(&recs[0]).join("\n");
        assert!(text.contains(ANCHOR_TYPE_LABELS[0]), "{text}");
        assert!(text.contains(ANCHOR_SEQ_LABELS[0]), "{text}");
        assert_eq!(recs[0].probe, ANCHOR_WITNESS_PROBE);
    }

    /// Refusals: each of the Python's four rejects must reject here too. These controls can FAIL
    /// (delete a check in read_records and they do).
    #[test]
    fn refuses_bad_magic() {
        let mut bytes = rx_entry_bytes();
        bytes[0] ^= 0xff;
        let err = read_records(&bytes, "test").unwrap_err();
        assert!(err.contains("bad magic"), "{err}");
    }

    #[test]
    fn refuses_wrong_version() {
        let mut bytes = rx_entry_bytes();
        bytes[4] = 9;
        let err = read_records(&bytes, "test").unwrap_err();
        assert!(err.contains("unsupported version 9"), "{err}");
    }

    #[test]
    fn refuses_wrong_encoded_size() {
        let mut bytes = rx_entry_bytes();
        bytes[8] = 216; // the pre-232-era size
        let err = read_records(&bytes, "test").unwrap_err();
        assert!(err.contains("encoded size 216, expected 232"), "{err}");
    }

    #[test]
    fn refuses_non_multiple_length() {
        let err = read_records(&[0u8; 100], "p").unwrap_err();
        assert_eq!(err, "p: 100 bytes is not a multiple of record size 232");
    }

    /// INVERTED control: perturb the input, prove the output changes.
    #[test]
    fn perturbing_the_sample_changes_the_explanation() {
        let clean = rx_entry_bytes();
        let mut perturbed = clean;
        perturbed[136] ^= 0x01; // sample_a[0]: low byte of received_type
        let a = explain(&read_records(&clean, "t").unwrap()[0]).join("\n");
        let b = explain(&read_records(&perturbed, "t").unwrap()[0]).join("\n");
        assert_ne!(a, b, "a one-bit input change must change the output");
    }

    #[test]
    fn records_sort_by_sequence() {
        let mut blob = Vec::new();
        blob.extend_from_slice(&make_record_bytes(0, 5, &[0u8; 48]));
        blob.extend_from_slice(&make_record_bytes(1, 2, &[0u8; 48]));
        let recs = read_records(&blob, "t").unwrap();
        assert_eq!(recs[0].sequence, 2);
        assert_eq!(recs[1].sequence, 5);
    }

    #[test]
    fn diagnosis_ladder_names_the_first_missing_probe() {
        assert!(diagnosis(&[]).starts_with("INVALID"));
        let armed = read_records(&make_record_bytes(0, 0, &[0u8; 48]), "t").unwrap();
        assert!(diagnosis(&armed).starts_with("REQUEST_BINDING"));
    }

    #[test]
    fn python_hex_width_formatting() {
        assert_eq!(py_hex_w10(5), "0x00000005");
        assert_eq!(py_hex_w10(-5), "-0x0000005");
        assert_eq!(py_hex_w10(0x123456789a), "0x123456789a");
    }

    /// Python's `json.dumps(ensure_ascii=True)` escapes non-ASCII; the port must too. An `armed`
    /// record with high bytes in the title puts U+FFFD in the explain line - the JSON document
    /// must carry `�`, not raw UTF-8 (found by the adversarial golden diff).
    #[test]
    fn json_escapes_non_ascii_like_python() {
        let mut sample_a = [0u8; 48];
        sample_a[..6].copy_from_slice(b"CU\xff\xfe12");
        let recs = read_records(&make_record_bytes(0, 0, &sample_a), "t").unwrap();
        let json = to_json(&recs);
        assert!(json.contains("title=CU\\ufffd\\ufffd12"), "{json}");
        assert!(json.is_ascii(), "the JSON document must be pure ASCII");
        // Human mode is NOT escaped (Python printed the raw character there).
        assert!(to_human(&recs).contains('\u{FFFD}'));
    }

    #[test]
    fn substate_probe_prints_the_python_none() {
        // Python's dead `len(a) > 0x33` guard makes h[+0x30] always None; the port preserves it.
        let recs = read_records(&make_record_bytes(20, 0, &[0u8; 48]), "t").unwrap();
        let text = explain(&recs[0]).join("\n");
        assert!(text.contains("h[+0x30]=None"), "{text}");
    }
}
