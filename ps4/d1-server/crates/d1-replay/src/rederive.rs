// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! `d1-replay rederive` - re-derive the activity-lifetime record facts from a snapshot.
//!
//! This is the control that lets a later session RE-CHECK the lifetime-phase evidence
//! (`design/activity-lifetime-state.md`) instead of trusting a transcript - and it is the check
//! that caught a tainted snapshot once already. From nothing but a snapshot and the datum id it
//! walks the client's own datum registry and reports:
//!
//! * the three fixed schema slots (name-ref `v:0x1902030`, sensor `v:0x1902280`,
//!   tag-ref `v:0x197deb0`) and the schema tag each points at;
//! * the record for datum `0x27FA2003` (class `0x8080039D`, globals record 3), resolved through
//!   the registry at `[v:0x1A0B420]` - never taken from a doc's cached address;
//! * the `+0xA8` bound-setter quad, the `+0x108` unbound udt-7 slot, the `+0x1AC` phase cluster
//!   and the replication datum at `+0x1A8`, followed to its own slot;
//! * the activity-client world pool (`[v:0x263C9D8]`, stride `[v:0x263C9E0]`);
//! * the full `0x504`-byte value image at `record+0x1AC` (hex-dumpable via `--dump-image`).
//!
//! The stdout rendering is byte-identical to the prototype this ports (the lifetime-phase lane's
//! `/tmp/rederive.py`), so the two can be golden-diffed - and was, on
//! `runs/snapshots/rung36-biped-chain.snap`.
//!
//! ⛔ **A missing range is NOT a zero.** Where the prototype died in a raw `struct.error` (or,
//! worse, could have carried a truncated buffer forward), every fact here is either derived or
//! reported as `NOT CAPTURED` with the address and length that were wanted - and the process exits
//! non-zero. `runs/snapshots/rung36-in-world-moonbase.snap` does not carry the datum-registry
//! heap; on it this subcommand must say so, never print a zero.
//!
//! ⛔ **Provenance still applies.** This subcommand reads memory; it does not validate the
//! snapshot. Run `d1-replay controls <snap>` first (exit 2 = the snapshot may not be cited -
//! `guardian-in-world-moonbase.snap` is the standing example), and `d1-replay rederive <snap>
//! --controls` for this subcommand's own controls.

use anyhow::Result;

/// The activity-lifetime content record's datum id (class `0x8080039D`, globals record 3).
pub const DEFAULT_DATUM: u32 = 0x27FA_2003;
/// The datum-registry pointer - `DAT_01A0B420`, the package registry global.
pub const REGISTRY_PTR_VADDR: u64 = 0x01A0_B420;
/// The activity-client world pool pointer and its stride.
pub const WORLD_POOL_VADDR: u64 = 0x0263_C9D8;
pub const WORLD_STRIDE_VADDR: u64 = 0x0263_C9E0;
/// The value image copied to `record+0x1AC` by the setter `FUN_0011DE30` (`memcpy` len `0x504`).
pub const IMAGE_LEN: usize = 0x504;
/// The three fixed schema slots. The display names carry the prototype's exact spacing so the
/// rendering stays golden-diffable against it.
pub const SCHEMA_SLOTS: [(&str, u64); 3] = [
    ("name-ref  [v:0x1902030]", 0x0190_2030),
    ("sensor    [v:0x1902280]", 0x0190_2280),
    ("tag-ref   [v:0x197deb0]", 0x0197_DEB0),
];

/// A fact is either derived, or `NOT CAPTURED` with the address that was wanted. There is no
/// third state and there is never a fabricated zero.
pub type Fact<T> = std::result::Result<T, String>;

fn u32le(b: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn u64le(b: &[u8], off: usize) -> u64 {
    u64::from_le_bytes(b[off..off + 8].try_into().unwrap())
}
fn i16le(b: &[u8], off: usize) -> i16 {
    i16::from_le_bytes(b[off..off + 2].try_into().unwrap())
}
fn i32le(b: &[u8], off: usize) -> i32 {
    i32::from_le_bytes(b[off..off + 4].try_into().unwrap())
}
fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Read `len` bytes at `addr`, or say NOT CAPTURED - with what was wanted, where, and why.
fn rd<R>(read: &R, what: &str, addr: u64, len: usize) -> Fact<Vec<u8>>
where
    R: Fn(u64, usize) -> Result<Vec<u8>>,
{
    read(addr, len)
        .map_err(|e| format!("NOT CAPTURED: {what} wanted {len:#x} byte(s) at {addr:#x} - {e:#}"))
}

/// Resolve a datum id through the registry slot table to the record base.
///
/// The walk is the client's own (`FUN_001072C0` family): `slot = (((d>>13)>>18) | 0x3ff) &
/// (d>>13)`, `idx = d & 0x1fff`; the slot entry (0x40 bytes) carries the element array at `+0x10`,
/// a signed stride at `+0x30` and a signed head mask at `+0x34`; the record base is the element
/// minus the masked qword at `element+8`. ⚠ Stride and mask are SIGNED 32-bit and the mask is
/// sign-extended before the AND - a port that zero-extends returns a plausible wrong base.
pub fn resolve<R>(read: &R, table: u64, datum: u32) -> Fact<(u64, u64, u64)>
where
    R: Fn(u64, usize) -> Result<Vec<u8>>,
{
    let d = datum as u64;
    let slot = (((d >> 13) >> 18) | 0x3ff) & (d >> 13);
    let idx = d & 0x1fff;
    let e = rd(
        read,
        &format!("the registry slot entry for datum 0x{datum:08X} (slot {slot:#x})"),
        table + slot * 0x40,
        0x40,
    )?;
    let arr = u64le(&e, 0x10);
    let stride = i32le(&e, 0x30) as i64;
    let mask = i32le(&e, 0x34) as i64 as u64; // sign-extend, then mask - the prototype's `& 2**64-1`
    let elem = (arr as i64).wrapping_add((idx as i64).wrapping_mul(stride)) as u64;
    let hdr = rd(
        read,
        &format!("the datum element header for 0x{datum:08X}"),
        elem,
        0x10,
    )?;
    Ok((elem.wrapping_sub(mask & u64le(&hdr, 8)), slot, idx))
}

pub struct RecordFacts {
    pub rec: u64,
    pub slot: u64,
    pub idx: u64,
    /// The first 0x1C0 bytes of the record - covers `+0xA8`, `+0x108` and the `+0x1AC` cluster.
    pub head: Vec<u8>,
}

pub struct ReplFacts {
    /// The replication datum read out of `record+0x1A8`.
    pub datum: u32,
    pub addr: u64,
    pub slot: u64,
    pub idx: u64,
    /// The first 0x70 bytes of the replication slot.
    pub bytes: Vec<u8>,
}

pub struct WorldFacts {
    pub pool: u64,
    pub stride: u32,
    /// Per world: `(addr, byte +0x1C, byte +0x1D, byte +0x654)`.
    pub worlds: Vec<Fact<(u64, u8, u8, u8)>>,
}

pub struct Derived {
    pub datum: u32,
    pub schema: Vec<(&'static str, u64, Fact<(u64, u32)>)>,
    pub record: Fact<RecordFacts>,
    pub replication: Fact<ReplFacts>,
    pub worlds: Fact<WorldFacts>,
    /// The full `0x504`-byte value image at `record+0x1AC`.
    pub image: Fact<Vec<u8>>,
}

impl Derived {
    /// TRUE only when every fact was derived. Anything less exits non-zero at the CLI.
    pub fn complete(&self) -> bool {
        self.schema.iter().all(|s| s.2.is_ok())
            && self.record.is_ok()
            && self.replication.is_ok()
            && self.image.is_ok()
            && match &self.worlds {
                Ok(w) => w.worlds.iter().all(|x| x.is_ok()),
                Err(_) => false,
            }
    }

    /// How many facts are NOT CAPTURED (sections count once; worlds count per world).
    pub fn missing(&self) -> usize {
        let mut n = self.schema.iter().filter(|s| s.2.is_err()).count();
        n += usize::from(self.record.is_err());
        n += usize::from(self.replication.is_err());
        n += usize::from(self.image.is_err());
        n += match &self.worlds {
            Ok(w) => w.worlds.iter().filter(|x| x.is_err()).count(),
            Err(_) => 1,
        };
        n
    }
}

/// Derive every fact. `read` is absolute-address, length-exact and MUST fail on uncaptured ranges
/// ([`crate::snapshot::Snapshot::read`] does); `base` is where eboot vaddr 0 is mapped.
pub fn derive<R>(read: &R, base: u64, datum: u32) -> Derived
where
    R: Fn(u64, usize) -> Result<Vec<u8>>,
{
    let v = |off: u64| base + off;

    let schema = SCHEMA_SLOTS
        .iter()
        .map(|&(nm, va)| {
            let f = (|| -> Fact<(u64, u32)> {
                let p = u64le(&rd(read, &format!("schema slot {nm}"), v(va), 8)?, 0);
                let tag = u32le(&rd(read, &format!("schema object behind {nm}"), p, 4)?, 0);
                Ok((p, tag))
            })();
            (nm, va, f)
        })
        .collect();

    // The registry: [v:0x1A0B420] -> registry object, whose first qword is the slot table.
    let table: Fact<u64> = (|| {
        let reg = u64le(
            &rd(read, "the datum-registry pointer [v:0x1a0b420]", v(REGISTRY_PTR_VADDR), 8)?,
            0,
        );
        Ok(u64le(&rd(read, "the registry slot table pointer", reg, 8)?, 0))
    })();

    let record: Fact<RecordFacts> = table.clone().and_then(|t| {
        let (rec, slot, idx) = resolve(read, t, datum)?;
        let head = rd(read, "the record head", rec, 0x1C0)?;
        Ok(RecordFacts { rec, slot, idx, head })
    });

    let replication: Fact<ReplFacts> = match (&table, &record) {
        (Ok(t), Ok(r)) => (|| {
            let d = u32le(&r.head, 0x1A8);
            let (addr, slot, idx) = resolve(read, *t, d)?;
            let bytes = rd(read, "the replication slot", addr, 0x70)?;
            Ok(ReplFacts { datum: d, addr, slot, idx, bytes })
        })(),
        (Err(e), _) | (_, Err(e)) => Err(e.clone()),
    };

    let worlds: Fact<WorldFacts> = (|| {
        let pool = u64le(
            &rd(read, "the world pool pointer [v:0x263c9d8]", v(WORLD_POOL_VADDR), 8)?,
            0,
        );
        let stride = u32le(
            &rd(read, "the world stride [v:0x263c9e0]", v(WORLD_STRIDE_VADDR), 4)?,
            0,
        );
        let worlds = (0..3u64)
            .map(|i| {
                let w = pool + i * stride as u64;
                let b = rd(read, &format!("world[{i}] head"), w, 0x20)?;
                let g = rd(read, &format!("world[{i}] +0x650"), w + 0x650, 0x10)?;
                Ok((w, b[0x1C], b[0x1D], g[4]))
            })
            .collect();
        Ok(WorldFacts { pool, stride, worlds })
    })();

    let image: Fact<Vec<u8>> = match &record {
        Ok(r) => rd(read, "the record+0x1AC value image", r.rec + 0x1AC, IMAGE_LEN),
        Err(e) => Err(e.clone()),
    };

    Derived { datum, schema, record, replication, worlds, image }
}

/// Render the facts. ⚠ For fully-derived facts this is BYTE-IDENTICAL to the Python prototype's
/// stdout - that identity is the executed golden diff, so do not "improve" the spacing. NOT
/// CAPTURED facts render as their message in place of the numbers, never as zeros.
pub fn render(d: &Derived) -> String {
    let mut s = String::new();

    s.push_str("== schema slots (fixed vaddrs) ==\n");
    for (nm, _va, f) in &d.schema {
        match f {
            Ok((p, tag)) => s.push_str(&format!("  {nm} -> {p:#x} +0x00 = {tag:#010x}\n")),
            Err(e) => s.push_str(&format!("  {nm} -> {e}\n")),
        }
    }

    let datum_hdr = format!("0x{:08X}", d.datum);
    match &d.record {
        Ok(r) => {
            let h = &r.head;
            s.push_str(&format!(
                "\n== record = resolve(datum {datum_hdr}) = {:#x}  (slot {:#x} idx {}) ==\n",
                r.rec, r.slot, r.idx
            ));
            s.push_str(&format!(
                "  +0x00 = {:#010x}   +0xA8 cell = {{{:#010x}, ord {}, obj {:#010x}, {}, {}}}\n",
                u32le(h, 0),
                u32le(h, 0xA8),
                u32le(h, 0xAC),
                u32le(h, 0xB0),
                u32le(h, 0xB4),
                u64le(h, 0xB8)
            ));
            s.push_str(&format!(
                "  +0x108 cell = {{{:#010x}, {:#010x}, {:#010x}}}   (udt-7 slot)\n",
                u32le(h, 0x108),
                u32le(h, 0x10C),
                u32le(h, 0x110)
            ));
            s.push_str(&format!(
                "  +0x1A8 = {:#010x}   +0x1AC(phase) = {:#04x}  +0x1AE = {:#04x}  +0x1B4 = {:#010x}\n",
                u32le(h, 0x1A8),
                h[0x1AC],
                h[0x1AE],
                u32le(h, 0x1B4)
            ));
        }
        Err(e) => s.push_str(&format!("\n== record = resolve(datum {datum_hdr}) ==\n  {e}\n")),
    }

    match &d.replication {
        Ok(r) => {
            let b = &r.bytes;
            s.push_str(&format!(
                "\n== replication slot = resolve({:#x}) = {:#x}  (slot {:#x} idx {}) ==\n",
                r.datum, r.addr, r.slot, r.idx
            ));
            s.push_str(&format!(
                "  name-ref +0x00 = {{hash {:#010x}, i8 {:#04x}, i16 {}}}\n",
                u32le(b, 0),
                b[4],
                i16le(b, 6)
            ));
            s.push_str(&format!(
                "  +0x08 = {:#010x}  +0x0C = {:#010x}  +0x14/15/16 = {},{},{}  +0x60 = {:#010x}  +0x66 = {}\n",
                u32le(b, 8),
                u32le(b, 0xC),
                b[0x14],
                b[0x15],
                b[0x16],
                u32le(b, 0x60),
                b[0x66]
            ));
        }
        Err(e) => s.push_str(&format!("\n== replication slot ==\n  {e}\n")),
    }

    match &d.worlds {
        Ok(w) => {
            s.push_str(&format!(
                "\n== activity-client worlds: pool {:#x} stride {:#x} ==\n",
                w.pool, w.stride
            ));
            for (i, f) in w.worlds.iter().enumerate() {
                match f {
                    Ok((addr, b1c, b1d, g654)) => s.push_str(&format!(
                        "  world[{i}] {addr:#x}  +0x1C = {b1c}  +0x1D = {b1d}  +0x654 = {g654}\n"
                    )),
                    Err(e) => s.push_str(&format!("  world[{i}] {e}\n")),
                }
            }
        }
        Err(e) => s.push_str(&format!("\n== activity-client worlds ==\n  {e}\n")),
    }

    match &d.image {
        Ok(img) => s.push_str(&format!(
            "\n== record+0x1AC image ({} bytes) sha-prefix ==\n  {} ... {}\n",
            img.len(),
            hex(&img[..24]),
            hex(&img[0x4C4..0x4D0])
        )),
        Err(e) => s.push_str(&format!("\n== record+0x1AC image ==\n  {e}\n")),
    }

    s
}

/// The bare-hex image dump, exactly as the prototype wrote it (Python `bytes.hex()`, no
/// newline) - so the two files golden-diff byte for byte.
pub fn render_image_hex(img: &[u8]) -> String {
    hex(img)
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Controls (ATLAS §4.4d) - run as `d1-replay rederive <snap> --controls`, exit 2 on any FAIL.
// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// The rederive controls, against a real snapshot. Always run on [`DEFAULT_DATUM`] - the known
/// answers below are facts about THAT record, published in `design/activity-lifetime-state.md`.
///
/// | control | the lie it catches |
/// |---|---|
/// | schema-slot-tags | the walk landed on the wrong objects (tags are the published schema ids) |
/// | record-setter-quad | the registry resolve returned a plausible wrong base |
/// | replication-value-schema | the `+0x1A8` follow resolved to something other than the value slot |
/// | phase-inversion (INVERTED) | the output does not actually track the bytes - it cannot fail |
/// | missing-range-is-loud (INVERTED) | an uncaptured range reads back as zeros instead of refusing |
pub fn run_controls(snap: &crate::snapshot::Snapshot) -> crate::controls::Report {
    use crate::controls::{Control, Report, Status};
    let mut c: Vec<Control> = Vec::new();
    let mut push = |name: &str, ok: bool, detail: String| {
        c.push(Control {
            name: name.into(),
            status: if ok { Status::Pass } else { Status::Fail },
            detail,
        });
    };

    let base = snap.base();
    let read = |a: u64, l: usize| snap.read(a, l);
    let d = derive(&read, base, DEFAULT_DATUM);

    // 1. Known answer: the fixed schema slots point at the published schema tags.
    match (&d.schema[0].2, &d.schema[1].2) {
        (Ok((_, nr)), Ok((_, se))) => push(
            "schema-slot-tags",
            *nr == 0x8080_04B1 && *se == 0x8080_0CF7,
            format!(
                "name-ref +0x00 = {nr:#010x} (want 0x808004b1), sensor +0x00 = {se:#010x} \
                 (want 0x80800cf7) - the schema ids named in design/activity-lifetime-state.md"
            ),
        ),
        (a, b) => push(
            "schema-slot-tags",
            false,
            a.as_ref().err().or(b.as_ref().err()).cloned().unwrap_or_default(),
        ),
    }

    // 2. Known answer: the record's +0xA8 bound-setter quad names its own class and ordinal and
    //    echoes the datum - {0x8080039D, ord 2, obj 0x27FA2003}. A wrong base cannot echo all three.
    match &d.record {
        Ok(r) => {
            let (cls, ord, obj) =
                (u32le(&r.head, 0xA8), u32le(&r.head, 0xAC), u32le(&r.head, 0xB0));
            push(
                "record-setter-quad",
                cls == 0x8080_039D && ord == 2 && obj == DEFAULT_DATUM,
                format!(
                    "record {:#x}: +0xA8 = {{{cls:#010x}, ord {ord}, obj {obj:#010x}}} - want \
                     {{0x8080039d, ord 2, obj 0x27fa2003}}, the PMF dispatch cell of the phase setter",
                    r.rec
                ),
            );
        }
        Err(e) => push("record-setter-quad", false, e.clone()),
    }

    // 3. Known answer: the slot resolved from record+0x1A8 carries the value schema 0x808008A3
    //    at +0x0C - the tag the sensor frame's 339-bit value block decodes under.
    match &d.replication {
        Ok(r) => {
            let tag = u32le(&r.bytes, 0xC);
            push(
                "replication-value-schema",
                tag == 0x8080_08A3,
                format!(
                    "replication slot {:#x}: +0x0C = {tag:#010x} - want 0x808008a3, the value \
                     schema of the sensor_auth_update frame",
                    r.addr
                ),
            );
        }
        Err(e) => push("replication-value-schema", false, e.clone()),
    }

    // 4. INVERTED: flip the phase byte through a read overlay and prove the output CHANGES. A
    //    control that can only pass is decoration; this one fails the moment the renderer stops
    //    tracking the bytes.
    match &d.record {
        Ok(r) => {
            let t = r.rec + 0x1AC;
            let flip = |a: u64, l: usize| -> Result<Vec<u8>> {
                let mut b = snap.read(a, l)?;
                if a <= t && t < a + l as u64 {
                    b[(t - a) as usize] ^= 0xFF;
                }
                Ok(b)
            };
            let d2 = derive(&flip, base, DEFAULT_DATUM);
            match &d2.record {
                Ok(r2) => {
                    let (p1, p2) = (r.head[0x1AC], r2.head[0x1AC]);
                    // The flipped value must show up in the PHASE FIELD of the rendering, not
                    // merely change the rendering somewhere: the overlay also flips byte 0 of
                    // the value image, so a whole-string inequality would pass even if the
                    // phase line printed a constant (adversarial verify, 2026-08-11).
                    let phase_line = format!("+0x1AC(phase) = {p2:#04x}");
                    let tracked = p1 != p2 && render(&d2).contains(&phase_line);
                    push(
                        "phase-inversion (INVERTED)",
                        tracked,
                        format!(
                            "phase byte perturbed through a read overlay: {p1:#04x} -> {p2:#04x}; \
                             the phase field {} - the output tracks the byte",
                            if tracked { "rendered the flipped value" } else { "DID NOT track it" }
                        ),
                    );
                }
                Err(e) => push("phase-inversion (INVERTED)", false, e.clone()),
            }
        }
        Err(e) => push("phase-inversion (INVERTED)", false, e.clone()),
    }

    // 5. INVERTED: withhold the image range and prove the tool says NOT CAPTURED rather than
    //    reading zeros. The hole starts at rec+0x1C0 so the record head still derives.
    match &d.record {
        Ok(r) => {
            let (lo, hi) = (r.rec + 0x1C0, r.rec + 0x1AC + IMAGE_LEN as u64);
            let refuse = |a: u64, l: usize| -> Result<Vec<u8>> {
                if a < hi && a + l as u64 > lo {
                    anyhow::bail!(
                        "synthetic hole: this control withholds {lo:#x}..{hi:#x} to prove a \
                         missing range refuses rather than zero-fills"
                    );
                }
                snap.read(a, l)
            };
            let d3 = derive(&refuse, base, DEFAULT_DATUM);
            let loud = d3.image.as_ref().err().map(|e| e.contains("NOT CAPTURED")).unwrap_or(false);
            push(
                "missing-range-is-loud (INVERTED)",
                d3.record.is_ok() && loud && render(&d3).contains("NOT CAPTURED"),
                if loud {
                    "the withheld image range came back NOT CAPTURED, in the fact and in the \
                     rendering - a missing range is not a zero"
                        .into()
                } else {
                    format!(
                        "the withheld range did NOT refuse loudly: image = {:?}",
                        d3.image.as_ref().map(|i| i.len())
                    )
                },
            );
        }
        Err(e) => push("missing-range-is-loud (INVERTED)", false, e.clone()),
    }

    Report { controls: c }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Tests - the same axes, self-contained on synthetic memory (no snapshot file needed).
// ─────────────────────────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Synthetic memory with the snapshot's refusal semantics: a read outside a region ERRORS.
    struct Mem(BTreeMap<u64, Vec<u8>>);
    impl Mem {
        fn new() -> Self {
            Mem(BTreeMap::new())
        }
        fn put(&mut self, addr: u64, bytes: Vec<u8>) {
            self.0.insert(addr, bytes);
        }
        fn poke(&mut self, addr: u64, bytes: &[u8]) {
            let (&start, blob) = self
                .0
                .range_mut(..=addr)
                .next_back()
                .expect("poke outside synthetic memory");
            let off = (addr - start) as usize;
            blob[off..off + bytes.len()].copy_from_slice(bytes);
        }
        fn read(&self, addr: u64, len: usize) -> Result<Vec<u8>> {
            let Some((&start, blob)) = self.0.range(..=addr).next_back() else {
                anyhow::bail!("{addr:#x} is not in this snapshot - a missing range is NOT a zero");
            };
            let end = start + blob.len() as u64;
            if addr + len as u64 > end {
                anyhow::bail!(
                    "{addr:#x}+{len:#x} is not in this snapshot (region {start:#x}..{end:#x}) - \
                     a missing range is NOT a zero"
                );
            }
            let off = (addr - start) as usize;
            Ok(blob[off..off + len].to_vec())
        }
    }

    const BASE: u64 = 0x8_0000_0000;
    const REC: u64 = 0x102ED0;

    /// A coherent synthetic world with every value known in advance.
    fn world() -> Mem {
        let mut m = Mem::new();
        // Fixed vaddr slots, one 8-byte region each.
        m.put(BASE + REGISTRY_PTR_VADDR, 0x2000u64.to_le_bytes().to_vec());
        m.put(BASE + WORLD_POOL_VADDR, 0x400000u64.to_le_bytes().to_vec());
        m.put(BASE + WORLD_STRIDE_VADDR, 0xDB0u64.to_le_bytes().to_vec());
        m.put(BASE + SCHEMA_SLOTS[0].1, 0x500000u64.to_le_bytes().to_vec());
        m.put(BASE + SCHEMA_SLOTS[1].1, 0x500010u64.to_le_bytes().to_vec());
        m.put(BASE + SCHEMA_SLOTS[2].1, 0x500020u64.to_le_bytes().to_vec());
        // Schema objects.
        m.put(0x500000, 0x8080_04B1u32.to_le_bytes().to_vec());
        m.put(0x500010, 0x8080_0CF7u32.to_le_bytes().to_vec());
        m.put(0x500020, 0x8080_0046u32.to_le_bytes().to_vec());
        // Registry object: first qword is the slot table at 0x3000.
        m.put(0x2000, 0x3000u64.to_le_bytes().to_vec());
        // Slot table region covering slots 0x3A1 and 0x3D1.
        let mut table = vec![0u8; 0x40 * 0x400];
        // Slot 0x3D1 (datum 0x27FA2003, idx 3): arr 0x100000, stride 0x1000, mask -1.
        let e = 0x3D1 * 0x40;
        table[e + 0x10..e + 0x18].copy_from_slice(&0x100000u64.to_le_bytes());
        table[e + 0x30..e + 0x34].copy_from_slice(&0x1000i32.to_le_bytes());
        table[e + 0x34..e + 0x38].copy_from_slice(&(-1i32).to_le_bytes());
        // Slot 0x3A1 (datum 0x33F42003, idx 3): arr 0x200000, stride 0x800, mask 0 - the slot IS
        // the element, so the +8 header qword doubles as slot data and must not be subtracted.
        let e = 0x3A1 * 0x40;
        table[e + 0x10..e + 0x18].copy_from_slice(&0x200000u64.to_le_bytes());
        table[e + 0x30..e + 0x34].copy_from_slice(&0x800i32.to_le_bytes());
        table[e + 0x34..e + 0x38].copy_from_slice(&0i32.to_le_bytes());
        m.put(0x3000, table);
        // Record element: base 0x100000 + 3*0x1000 = 0x103000; header qword at +8 = 0x130
        // => record = 0x103000 - 0x130 = 0x102ED0 = REC. One blob covers header AND record.
        let mut rec = vec![0u8; 0x103010 - REC as usize + 0x6C0];
        let o = (0x103000 - REC) as usize;
        rec[o + 8..o + 16].copy_from_slice(&0x130u64.to_le_bytes());
        rec[0..4].copy_from_slice(&0x80F0_80C3u32.to_le_bytes());
        rec[0xA8..0xAC].copy_from_slice(&0x8080_039Du32.to_le_bytes());
        rec[0xAC..0xB0].copy_from_slice(&2u32.to_le_bytes());
        rec[0xB0..0xB4].copy_from_slice(&DEFAULT_DATUM.to_le_bytes());
        rec[0x108..0x114].copy_from_slice(&[0xFF; 12]);
        rec[0x1A8..0x1AC].copy_from_slice(&0x33F4_2003u32.to_le_bytes());
        rec[0x1AC] = 0xFF;
        rec[0x1AE] = 0x01;
        rec[0x1B4..0x1B8].copy_from_slice(&0x811C_9DC5u32.to_le_bytes());
        // Give the image tail a non-zero landmark inside the printed 0x4C4..0x4D0 window.
        rec[0x1AC + 0x4C8..0x1AC + 0x4CC].copy_from_slice(&0x811C_9DC5u32.to_le_bytes());
        m.put(REC, rec);
        // Replication element: base 0x200000 + 3*0x800 = 0x201800; header qword at +8 = 0
        // => slot address = element itself.
        let mut repl = vec![0u8; 0x80];
        repl[0..4].copy_from_slice(&0xC2DA_CAF7u32.to_le_bytes());
        repl[4] = 0x10;
        repl[6..8].copy_from_slice(&(-2i16).to_le_bytes()); // exercises the SIGNED i16 decode
        repl[8..12].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        repl[0xC..0x10].copy_from_slice(&0x8080_08A3u32.to_le_bytes());
        repl[0x14] = 1;
        repl[0x15] = 1;
        repl[0x16] = 1;
        repl[0x60..0x64].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        m.put(0x201800, repl);
        // Three worlds at pool 0x400000, stride 0xDB0.
        let mut pool = vec![0u8; 3 * 0xDB0];
        pool[0x1C] = 1;
        m.put(0x400000, pool);
        m
    }

    /// The registry walk, including the SIGNED mask sign-extension - with its own inversion:
    /// zeroing the mask must move the answer, or the mask was decoration.
    #[test]
    fn resolve_applies_the_signed_mask_to_the_element_header() {
        let m = world();
        let r = |a: u64, l: usize| m.read(a, l);
        let (rec, slot, idx) = resolve(&r, 0x3000, DEFAULT_DATUM).expect("resolves");
        assert_eq!((rec, slot, idx), (REC, 0x3D1, 3));

        // INVERTED: mask 0 => the header offset is ignored and the answer must CHANGE.
        let mut m2 = world();
        m2.poke(0x3000 + 0x3D1 * 0x40 + 0x34, &0i32.to_le_bytes());
        let r2 = |a: u64, l: usize| m2.read(a, l);
        let (rec2, _, _) = resolve(&r2, 0x3000, DEFAULT_DATUM).expect("resolves");
        assert_eq!(rec2, 0x103000, "mask 0 must yield the raw element base");
        assert_ne!(rec, rec2, "perturbing the mask must move the record base");
    }

    /// A missing range is NOT CAPTURED - in the fact, in the rendering, and never a zero.
    #[test]
    fn a_missing_range_is_not_captured_not_zero() {
        let mut m = world();
        // Truncate the record blob to the head only: the 0x504 image tail is gone.
        let head = m.read(REC, 0x1C0).unwrap();
        m.0.remove(&REC);
        // Keep the element header readable (it lives above the head in the original blob).
        m.put(REC, head);
        m.put(0x103000, {
            let mut h = vec![0u8; 0x10];
            h[8..16].copy_from_slice(&0x130u64.to_le_bytes());
            h
        });
        let r = |a: u64, l: usize| m.read(a, l);
        let d = derive(&r, BASE, DEFAULT_DATUM);
        assert!(d.record.is_ok(), "the head is still derivable");
        let e = d.image.as_ref().err().expect("the image must NOT derive");
        assert!(e.contains("NOT CAPTURED"), "the fact says so: {e}");
        let out = render(&d);
        assert!(out.contains("NOT CAPTURED"), "the rendering says so");
        assert!(!out.contains("(1284 bytes)"), "no fabricated image line");
        assert!(!d.complete(), "an incomplete derivation must not report complete");
        assert_eq!(d.missing(), 1, "exactly the image is missing");
    }

    /// INVERTED: perturb the phase byte, prove the output changes. This is the control that makes
    /// "the phase is 0xFF" a measurement instead of a constant.
    #[test]
    fn perturbing_the_phase_byte_changes_the_output() {
        let m = world();
        let r = |a: u64, l: usize| m.read(a, l);
        let d1 = derive(&r, BASE, DEFAULT_DATUM);
        assert_eq!(d1.record.as_ref().unwrap().head[0x1AC], 0xFF);

        let mut m2 = world();
        m2.poke(REC + 0x1AC, &[0x03]);
        let r2 = |a: u64, l: usize| m2.read(a, l);
        let d2 = derive(&r2, BASE, DEFAULT_DATUM);
        assert_eq!(d2.record.as_ref().unwrap().head[0x1AC], 0x03);
        assert_ne!(render(&d1), render(&d2), "the rendering must track the byte");
        assert!(render(&d2).contains("+0x1AC(phase) = 0x03"));
    }

    /// The rendering is the prototype's, line for line - spacing included. The real golden diff
    /// ran against the Python on `rung36-biped-chain.snap`; this pins the format so a later edit
    /// cannot silently drift it.
    #[test]
    fn render_matches_the_prototype_format() {
        let m = world();
        let r = |a: u64, l: usize| m.read(a, l);
        let d = derive(&r, BASE, DEFAULT_DATUM);
        assert!(d.complete(), "the synthetic world must derive fully: {}", render(&d));
        let out = render(&d);
        for line in [
            "== schema slots (fixed vaddrs) ==",
            "  name-ref  [v:0x1902030] -> 0x500000 +0x00 = 0x808004b1",
            "  sensor    [v:0x1902280] -> 0x500010 +0x00 = 0x80800cf7",
            "  tag-ref   [v:0x197deb0] -> 0x500020 +0x00 = 0x80800046",
            "== record = resolve(datum 0x27FA2003) = 0x102ed0  (slot 0x3d1 idx 3) ==",
            "  +0x00 = 0x80f080c3   +0xA8 cell = {0x8080039d, ord 2, obj 0x27fa2003, 0, 0}",
            "  +0x108 cell = {0xffffffff, 0xffffffff, 0xffffffff}   (udt-7 slot)",
            "  +0x1A8 = 0x33f42003   +0x1AC(phase) = 0xff  +0x1AE = 0x01  +0x1B4 = 0x811c9dc5",
            "== replication slot = resolve(0x33f42003) = 0x201800  (slot 0x3a1 idx 3) ==",
            "  name-ref +0x00 = {hash 0xc2dacaf7, i8 0x10, i16 -2}",
            "  +0x08 = 0xffffffff  +0x0C = 0x808008a3  +0x14/15/16 = 1,1,1  +0x60 = 0xffffffff  +0x66 = 0",
            "== activity-client worlds: pool 0x400000 stride 0xdb0 ==",
            "  world[0] 0x400000  +0x1C = 1  +0x1D = 0  +0x654 = 0",
            "  world[1] 0x400db0  +0x1C = 0  +0x1D = 0  +0x654 = 0",
            "  world[2] 0x401b60  +0x1C = 0  +0x1D = 0  +0x654 = 0",
            "== record+0x1AC image (1284 bytes) sha-prefix ==",
        ] {
            assert!(out.contains(line), "missing line {line:?} in:\n{out}");
        }
        // The image landmark planted at +0x4C8 must appear in the printed 0x4C4..0x4D0 window.
        assert!(out.contains("00000000c59d1c8100000000"), "image tail window:\n{out}");
    }
}
