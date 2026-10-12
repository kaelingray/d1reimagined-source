// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Rendering, kept **byte-identical** to `capture-rig/d1guest.py`.
//!
//! ⭐ This is not fussiness. Three years of this project's `design/` docs quote hexdumps pasted out
//! of that script, and `status/` rows diff one run against another with `diff`. A port that
//! "improved" the layout would make every archived dump un-diffable against a fresh one, and the
//! quiet cost lands on whoever next tries to reproduce a finding.
//!
//! So the reference is the Python's own format strings:
//!
//! ```text
//! f"  {addr+i:#014x}  {chunk.hex(' '):<47}  {txt}"          # a hexdump row
//! f"    +{off:#06x} {name:<12} {ty:<4} = {val:#x} ({val})"  # a struct field
//! ```

use crate::symbols::StructDef;

/// One `FMT` row of `d1guest.py`: a struct format and its width.
pub fn fmt_width(ty: &str) -> Option<usize> {
    Some(match ty {
        "u8" => 1,
        "u16" => 2,
        "u32" | "i32" => 4,
        "u64" | "i64" | "ptr" => 8,
        _ => return None,
    })
}

/// Decode `ty` at `off`, printed the way the Python prints it.
///
/// ⚠ `i32`/`i64` are **signed** in the decimal column and unsigned in the hex column, because that
/// is what `struct.unpack('<i')` then `f"{val:#x} ({val})"` produces: Python's `%x` of a negative
/// int prints `-0x…`. Reproduced exactly, including the minus sign.
fn decode(ty: &str, data: &[u8], off: usize) -> Option<(String, String)> {
    let n = fmt_width(ty)?;
    if off + n > data.len() {
        return None;
    }
    let b = &data[off..off + n];
    Some(match ty {
        "u8" => (format!("{:#x}", b[0]), format!("{}", b[0])),
        "u16" => {
            let v = u16::from_le_bytes(b.try_into().unwrap());
            (format!("{v:#x}"), format!("{v}"))
        }
        "u32" => {
            let v = u32::from_le_bytes(b.try_into().unwrap());
            (format!("{v:#x}"), format!("{v}"))
        }
        "i32" => {
            let v = i32::from_le_bytes(b.try_into().unwrap());
            (py_hex_i64(v as i64), format!("{v}"))
        }
        "i64" => {
            let v = i64::from_le_bytes(b.try_into().unwrap());
            (py_hex_i64(v), format!("{v}"))
        }
        _ => {
            let v = u64::from_le_bytes(b.try_into().unwrap());
            (format!("{v:#x}"), format!("{v}"))
        }
    })
}

/// Python's `f"{v:#x}"` for a signed value: `-0x1` rather than `0xffffffffffffffff`.
fn py_hex_i64(v: i64) -> String {
    if v < 0 {
        format!("-{:#x}", v.unsigned_abs())
    } else {
        format!("{v:#x}")
    }
}

/// `d1guest.py`'s `hexdump`. `label` replaces the absolute address column with `label+0x…`, which is
/// what the `--deref` path prints (`*+0x0000`), so a dereferenced dump does not pretend the offsets
/// are addresses in a space the reader can look up.
pub fn hexdump(b: &[u8], addr: u64, label: Option<&str>) -> String {
    let mut out = Vec::new();
    for (i, chunk) in b.chunks(16).enumerate() {
        let i = i * 16;
        let txt: String = chunk
            .iter()
            .map(|&c| if (32..127).contains(&c) { c as char } else { '.' })
            .collect();
        let hex = chunk.iter().map(|c| format!("{c:02x}")).collect::<Vec<_>>().join(" ");
        let pre = match label {
            None => format!("{:#014x}", addr.wrapping_add(i as u64)),
            Some(l) => format!("{l}+{i:#06x}"),
        };
        out.push(format!("  {pre}  {hex:<47}  {txt}"));
    }
    out.join("\n")
}

/// `d1guest.py`'s `show_struct`.
///
/// ⚠ A field past the end of the read prints `<past the end of the read>` rather than being skipped.
/// Silence there reads as "the field is not in this struct", which is a different claim.
pub fn show_struct(sd: &StructDef, data: &[u8]) -> String {
    let mut out = vec![format!(
        "  struct {} ({} bytes)  {}  {}",
        sd.name, sd.size, sd.confidence, sd.source
    )];
    for f in &sd.fields {
        match decode(&f.ty, data, f.off) {
            None if fmt_width(&f.ty).is_none() => {
                out.push(format!(
                    "    +{:#06x} {:<12} <unknown field type {:?} - guest-symbols.json needs one of \
                     u8/u16/u32/u64/i32/i64/ptr>",
                    f.off, f.name, f.ty
                ));
            }
            None => {
                out.push(format!("    +{:#06x} {:<12} <past the end of the read>", f.off, f.name));
            }
            Some((hex, dec)) => {
                let note = if f.note.is_empty() { String::new() } else { format!("   {}", f.note) };
                out.push(format!(
                    "    +{:#06x} {:<12} {:<4} = {} ({}){}",
                    f.off, f.name, f.ty, hex, dec, note
                ));
            }
        }
    }
    out.join("\n")
}

/// The `--as u32|u64|ptr` view. `base` enables the `-> guest 0x…` annotation, which is the one
/// column that turns a wall of pointers into "this one is inside the eboot image".
///
/// ⚠ The window is `base ..= base + 2^32`, exactly as the Python has it. A pointer outside that is
/// **not** annotated rather than being annotated wrongly.
pub fn as_words(data: &[u8], view: &str, base: u64) -> String {
    let size = match view {
        "u32" => 4,
        _ => 8,
    };
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + size <= data.len() {
        let v = if size == 4 {
            u32::from_le_bytes(data[i..i + 4].try_into().unwrap()) as u64
        } else {
            u64::from_le_bytes(data[i..i + 8].try_into().unwrap())
        };
        let mut extra = String::new();
        if view == "ptr" && v != 0 && v >= base && v < base + (1u64 << 32) {
            extra = format!("   -> guest {:#x}", v - base);
        }
        // Python: f"{v:#0{size*2+2}x}" - zero-padded to the full width of the word, `0x` included.
        let width = size * 2 + 2;
        out.push(format!("  +{i:#06x}  {:#0width$x}{extra}", v, width = width));
        i += size;
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbols::{Field, StructDef};

    /// Goldens produced by running `d1guest.py`'s own `hexdump` under CPython over the same bytes.
    /// ⭐ Column positions are part of the contract; `{:<47}` is what keeps the ASCII column aligned
    /// on a short final row.
    #[test]
    fn hexdump_is_byte_identical_to_the_python() {
        let data: Vec<u8> = (0u8..20).collect();
        let got = hexdump(&data, 0x8_0263_D0F8, None);
        let want = "  0x00080263d0f8  00 01 02 03 04 05 06 07 08 09 0a 0b 0c 0d 0e 0f  ................\n  \
                    0x00080263d108  10 11 12 13                                      ....";
        assert_eq!(got, want);
    }

    #[test]
    fn hexdump_labelled_form_replaces_the_address_column() {
        let data: Vec<u8> = vec![0x41, 0x7f, 0x20, 0x1f];
        assert_eq!(hexdump(&data, 0, Some("*")), "  *+0x0000  41 7f 20 1f                                      A. .");
    }

    /// The INVERSION: change one byte and the rendering must change. A formatter that dropped the
    /// payload would pass every alignment assertion above.
    #[test]
    fn hexdump_changes_when_the_bytes_change() {
        let a = hexdump(&[0u8; 16], 0x1000, None);
        let mut b8 = [0u8; 16];
        b8[7] = 1;
        let b = hexdump(&b8, 0x1000, None);
        assert_ne!(a, b);
        // …and when the ADDRESS changes, so a dump cannot be quoted against the wrong place.
        assert_ne!(hexdump(&[0u8; 4], 0x1000, None), hexdump(&[0u8; 4], 0x1004, None));
    }

    #[test]
    fn struct_fields_render_like_the_python_including_the_overrun_note() {
        let sd = StructDef {
            name: "t".into(),
            size: 16,
            confidence: "[C]".into(),
            source: "test".into(),
            fields: vec![
                Field { off: 0, name: "tag".into(), ty: "u8".into(), note: "must be 3".into() },
                Field { off: 1, name: "key".into(), ty: "u64".into(), note: String::new() },
                Field { off: 64, name: "far".into(), ty: "u32".into(), note: String::new() },
            ],
        };
        let mut data = vec![0u8; 16];
        data[0] = 3;
        data[1..9].copy_from_slice(&0xD100_0000_0000_0001u64.to_le_bytes());
        let got = show_struct(&sd, &data);
        assert!(got.contains("    +0x0000 tag          u8   = 0x3 (3)   must be 3"));
        assert!(got.contains("    +0x0001 key          u64  = 0xd100000000000001 (15060037153926938625)"));
        assert!(got.contains("    +0x0040 far          <past the end of the read>"));
    }

    /// Signed widths print Python-style (`-0x1`), not as a 64-bit two's-complement blob.
    #[test]
    fn signed_fields_print_the_way_python_prints_them() {
        let sd = StructDef {
            name: "t".into(),
            size: 4,
            confidence: String::new(),
            source: String::new(),
            fields: vec![Field { off: 0, name: "v".into(), ty: "i32".into(), note: String::new() }],
        };
        let got = show_struct(&sd, &(-1i32).to_le_bytes());
        assert!(got.contains("= -0x1 (-1)"), "{got}");
    }

    #[test]
    fn ptr_view_annotates_only_inside_the_image_window() {
        let base = 0x8_0000_0000u64;
        let mut d = Vec::new();
        d.extend_from_slice(&(base + 0x26A2040).to_le_bytes()); // inside  -> annotated
        d.extend_from_slice(&0x2_04ED_A280u64.to_le_bytes()); // a heap pointer -> not
        d.extend_from_slice(&0u64.to_le_bytes()); // NULL -> never annotated
        let out = as_words(&d, "ptr", base);
        assert!(out.contains("-> guest 0x26a2040"), "{out}");
        assert_eq!(out.matches("-> guest").count(), 1, "{out}");
        assert!(out.contains("  +0x0000  0x00000008026a2040"), "{out}");
    }
}
