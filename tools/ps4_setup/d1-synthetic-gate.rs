// SPDX-License-Identifier: GPL-2.0-or-later
//! Generate original synthetic gate-control bytes through the existing tracer encoder.
//! Install in d1-trace/src/bin/. No game, captured registers or native trace is read.
use d1_trace::probe_decode::{make_record_bytes, read_records};

fn fixture() -> Vec<u8> {
    let mut bytes = Vec::new();
    for (i, sequence) in [53, 59, 93, 102, 107, 123].into_iter().enumerate() {
        let mut record = make_record_bytes(15, sequence, &[0; 48]);
        // The encoder's test records start with zero registers. Set only synthetic RAX.
        let rax = if i < 2 { 0xdeadbeef54321a00u64 } else { 0xdeadbeef54321a01u64 };
        record[32..40].copy_from_slice(&rax.to_le_bytes());
        bytes.extend_from_slice(&record);
    }
    bytes
}

fn controls() -> Result<(), String> {
    let bytes = fixture();
    let rows = read_records(&bytes, "synthetic-gate")?;
    if rows.len() != 6 || rows.iter().any(|r| r.probe_id != 15)
        || rows.iter().filter(|r| r.regs[2] & 0xff == 1).count() != 4
        || rows.iter().any(|r| r.regs.iter().enumerate().any(|(i, v)| i != 2 && *v != 0)) {
        return Err("synthetic fixture decoded incorrectly".into());
    }
    if read_records(&bytes[..bytes.len()-1], "truncated").is_ok() {
        return Err("truncated fixture accepted".into());
    }
    Ok(())
}

fn main() {
    if let Err(e) = controls() { eprintln!("{e}"); std::process::exit(2); }
    match std::env::args().nth(1).as_deref() {
        Some("controls") => println!("2 synthetic gate controls pass"),
        Some("source") => {
            println!("// SPDX-License-Identifier: GPL-2.0-or-later");
            println!("// Synthetic data: d1-synthetic-gate source; no native input.\n&[");
            for chunk in fixture().chunks(24) {
                println!("    {},", chunk.iter().map(|b| format!("0x{b:02x}")).collect::<Vec<_>>().join(", "));
            }
            println!("]");
        }
        _ => { eprintln!("usage: d1-synthetic-gate controls|source"); std::process::exit(2); }
    }
}

#[cfg(test)] mod tests {
    #[test] fn synthetic_positive_and_truncation_controls() { super::controls().unwrap(); }
}
