// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The `NOT-EXPRESSIBLE` resolvers: **Rust in, `a:0x…` out**.
//!
//! Some things a surface needs are not addresses you can write down. The scenario record is
//! reached by running the client's own hash-bucket walk on a 32-bit *tag*; the sobject census is a
//! loop over a pool. Neither is expressible in `d1_live::addr`, and inventing syntax for them would
//! be the fork this crate exists not to commit. So they are Rust, and they hand back an **absolute
//! address** or a plain count, which the shared grammar then consumes as `a:0x…`.
//!
//! ⭐⭐ EVERY CONSTANT HERE COMES OUT OF THE REGISTRY, NOT OUT OF THIS FILE. The pool address, the
//! bucket stride, and every field offset are read from `capture-rig/surfaces/registry.toml`
//! at call time. That is what makes control #2 meaningful: perturb one registry offset and these
//! resolvers must produce the wrong answer. A resolver with the offsets baked in would sail
//! through a perturbed registry and prove nothing.

use crate::eval::{Evaluator, Reading};
use crate::registry::Surface;

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Registry lookups - the only source of geometry.

fn surface<'a>(ev: &'a Evaluator, name: &str) -> Result<&'a Surface, Reading> {
    ev.registry
        .and_then(|r| r.surface(name))
        .ok_or_else(|| Reading::Unresolvable {
            why: format!("surface {name:?} is not in the registry (no registry bound?)"),
        })
}

fn field_off(s: &Surface, field: &str) -> Result<u64, Reading> {
    s.fields
        .iter()
        .find(|f| f.name == field)
        .map(|f| f.off)
        .ok_or_else(|| Reading::Unresolvable {
            why: format!("surface {:?} has no field {field:?}", s.name),
        })
}

/// Pull `stride 0xNN` out of a field's note. ⚠ A stride that lives only in prose is a stride
/// nobody can perturb; reading it here at least keeps it in the registry rather than in Rust.
fn stride_from_note(s: &Surface, field: &str) -> Result<u64, Reading> {
    let f = s.fields.iter().find(|f| f.name == field).ok_or_else(|| Reading::Unresolvable {
        why: format!("surface {:?} has no field {field:?}", s.name),
    })?;
    let note = f.note.clone().unwrap_or_default();
    let idx = note.find("stride 0x").ok_or_else(|| Reading::Unresolvable {
        why: format!("{}.{field} note does not state a `stride 0xNN`", s.name),
    })?;
    let rest = &note[idx + "stride 0x".len()..];
    let hex: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    u64::from_str_radix(&hex, 16).map_err(|e| Reading::Unresolvable {
        why: format!("{}.{field} stride is not hex: {e}", s.name),
    })
}

fn addr_of(ev: &Evaluator, s: &Surface) -> Result<u64, Reading> {
    ev.resolve_addr(&s.addr)
}

fn ru64(ev: &Evaluator, a: u64) -> Result<u64, Reading> {
    ev.ms.read_u64(a).map_err(|e| match e {
        crate::mem_source::ReadErr::Unmapped { addr, len } => Reading::Unmapped { addr, len },
        other => Reading::Unresolvable { why: other.to_string() },
    })
}

fn ru32(ev: &Evaluator, a: u64) -> Result<u32, Reading> {
    ev.ms.read_u32(a).map_err(|e| match e {
        crate::mem_source::ReadErr::Unmapped { addr, len } => Reading::Unmapped { addr, len },
        other => Reading::Unresolvable { why: other.to_string() },
    })
}

fn rbytes(ev: &Evaluator, a: u64, n: usize) -> Result<Vec<u8>, Reading> {
    ev.ms.read_vec(a, n).map_err(|e| match e {
        crate::mem_source::ReadErr::Unmapped { addr, len } => Reading::Unmapped { addr, len },
        other => Reading::Unresolvable { why: other.to_string() },
    })
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The package tag registry - the client's own bucket walk, verbatim.

/// `record = resolve(tag)`, exactly as `FUN_00299100` @ `0x29916b..0x299190` does it.
///
/// ```text
/// registry_base = *(u64*)[v:0x1A0B420]        ⭐ DOUBLE indirection - getting it wrong is silent
/// slot          = (tag >> 13) & 0xFFFF        bucket = registry_base + slot*stride
/// data_base     = u64 @ bucket+0x10
/// esz           = i32 @ bucket+0x30
/// mask          = i32 @ bucket+0x34           (SIGNED, sign-extended to 64)
/// p             = data_base + (tag & 0x1FFF) * esz
/// record        = p - (mask & u64@(p+8))
/// ```
pub fn tag_resolve(ev: &Evaluator, tag: u64) -> Result<u64, Reading> {
    let s = surface(ev, "package_tag_registry")?;
    let one = addr_of(ev, s)?; // `[v:0x1A0B420]`
    let registry_base = ru64(ev, one)?; // …and the second hop
    let stride = stride_from_note(s, "bucket")?;
    let slot = (tag >> 13) & 0xFFFF;
    let bucket = registry_base.wrapping_add(slot.wrapping_mul(stride));
    let data_base = ru64(ev, bucket + field_off(s, "bucket.data_base")?)?;
    let esz = ru32(ev, bucket + field_off(s, "bucket.entry_size")?)? as i32 as i64;
    let mask = ru32(ev, bucket + field_off(s, "bucket.mask")?)? as i32 as i64 as u64;
    let p = data_base.wrapping_add((tag & 0x1FFF).wrapping_mul(esz as u64));
    let back = ru64(ev, p + 8)?;
    Ok(p.wrapping_sub(mask & back))
}

fn bucket_field(ev: &Evaluator, slot: u64, which: &str) -> Result<i64, Reading> {
    let s = surface(ev, "package_tag_registry")?;
    let one = addr_of(ev, s)?;
    let registry_base = ru64(ev, one)?;
    let stride = stride_from_note(s, "bucket")?;
    let bucket = registry_base.wrapping_add(slot.wrapping_mul(stride));
    let off = field_off(s, which)?;
    Ok(ru32(ev, bucket + off)? as i32 as i64)
}

/// The scenario record's self-relative name: `ptr = (record+off) + i64@(record+off)`.
fn scenario_name(ev: &Evaluator, record: u64) -> Result<String, Reading> {
    let s = surface(ev, "scenario_record")?;
    let off = field_off(s, "name")?;
    let at = record + off;
    let rel = ru64(ev, at)? as i64;
    let p = (at as i64).wrapping_add(rel) as u64;
    match ev.read_cstr(p, 128) {
        Reading::Str(s) => Ok(s),
        other => Err(other),
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The sobject census.

struct Pool {
    element_base: u64,
    stride: u64,
    capacity: u64,
    high_water: u64,
    free: u64,
    words: u64,
    typeword_off: u64,
    self_handle_off: u64,
}

fn sobjects(ev: &Evaluator) -> Result<Pool, Reading> {
    let pool_s = surface(ev, "sobjects_pool")?;
    let state_s = surface(ev, "sobjects_state")?;
    let bit_s = surface(ev, "sobjects_bitvec")?;
    let elem_s = surface(ev, "sobject_element")?;

    let pool = addr_of(ev, pool_s)?; // inline BSS header
    let element_base = ru64(ev, pool + field_off(pool_s, "element_base")?)?;
    let stride = ru32(ev, pool + field_off(pool_s, "stride")?)? as u64;
    let capacity = ru32(ev, pool + field_off(pool_s, "capacity")?)? as u64;
    let high_water = ru32(ev, pool + field_off(pool_s, "high_water")?)? as u64;

    let state = addr_of(ev, state_s)?;
    let free_off = field_off(state_s, "free")?;
    let free = u16::from_le_bytes(rbytes(ev, state + free_off, 2)?.try_into().unwrap()) as u64;

    let bitvec_obj = addr_of(ev, bit_s)?;
    let words = ru64(ev, bitvec_obj + field_off(bit_s, "words_ptr")?)?;

    Ok(Pool {
        element_base,
        stride,
        capacity,
        high_water,
        free,
        words,
        typeword_off: field_off(elem_s, "typeword")?,
        self_handle_off: field_off(elem_s, "self_handle")?,
    })
}

/// ⭐ THE AUTHORITATIVE OCCUPANCY PREDICATE (`sobjects_bitvec`'s own note): bit `i` set ⇔ element
/// `i` is live. Measured equal to `capacity - free` on all three snapshots.
fn bitvec_set(ev: &Evaluator, p: &Pool, n: u64) -> Result<Vec<u64>, Reading> {
    let nwords = n.div_ceil(64) as usize;
    let bytes = rbytes(ev, p.words, nwords * 8)?;
    let mut out = Vec::new();
    for i in 0..n {
        let w = u64::from_le_bytes(bytes[(i / 64) as usize * 8..][..8].try_into().unwrap());
        if (w >> (i % 64)) & 1 == 1 {
            out.push(i);
        }
    }
    Ok(out)
}

/// ⚠ A SUPERSET OF LIVENESS, NOT LIVENESS. The type enum is the **LOW** byte of the u16 at
/// `+typeword_off`; the HIGH byte is `0xFF` on any slot whose type word was ever written - including
/// freed ones. Kept because the census the tree publishes is computed over exactly this set, and
/// because the direction that matters is safe: `set(bitvec) - set(highbyte)` is empty on all three
/// snapshots, so this rule never *misses* a live object.
fn highbyte_set(ev: &Evaluator, p: &Pool) -> Result<Vec<(u64, u8)>, Reading> {
    let mut out = Vec::new();
    for i in 0..p.high_water {
        let a = p.element_base + i * p.stride + p.typeword_off;
        let b = rbytes(ev, a, 2)?;
        if b[1] == 0xFF {
            out.push((i, b[0]));
        }
    }
    Ok(out)
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Public entry points

/// Rewrite every `resolve(0xTAG)` in an address expression to `a:0x…`, then hand the result to the
/// **shared** grammar. ⭐ Macro expansion, not a new syntax: what `d1_live::addr` finally parses
/// contains only terms it already knows.
pub fn expand_resolvers(ev: &Evaluator, text: &str) -> Result<String, Reading> {
    const PAT: &str = "resolve(";
    if !text.contains(PAT) {
        return Ok(text.to_string());
    }
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find(PAT) {
        out.push_str(&rest[..i]);
        let after = &rest[i + PAT.len()..];
        let close = after.find(')').ok_or_else(|| Reading::Unresolvable {
            why: format!("unbalanced `resolve(` in {text:?}"),
        })?;
        let arg = after[..close].trim();
        let tag = parse_u64(arg).ok_or_else(|| Reading::Unresolvable {
            why: format!("resolve() wants a numeric tag, got {arg:?}"),
        })?;
        let record = tag_resolve(ev, tag)?;
        out.push_str(&format!("a:{record:#x}"));
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn parse_u64(t: &str) -> Option<u64> {
    let t = t.trim();
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()
    } else {
        t.parse().ok()
    }
}

/// Is this expression handled *entirely* by a Rust resolver (as opposed to being an address
/// expression, possibly containing a `resolve(…)` macro)?
///
/// ⭐ Callers must ask this BEFORE handing an expression to the address grammar. `popcount(words
/// [0..1500])` is not an address and the grammar is right to reject it; classifying it as one
/// produces a confident, wrong "the registry does not parse".
pub fn is_whole_resolver(expr: &str) -> bool {
    let e = expr.trim();
    e.starts_with("name(resolve(")
        || e.starts_with("bucket(")
        || e.starts_with("popcount(")
        || e.starts_with("set(")
        || e.starts_with("count(")
        || e == "self_handle check"
}

/// Whole-expression resolvers. Returns `None` when the expression is an ordinary address/typed
/// expression and should go to [`crate::eval::TypedExpr`].
pub fn eval_whole(ev: &Evaluator, expr: &str) -> Option<Reading> {
    let e = expr.trim();

    // ── package tag registry ─────────────────────────────────────────────────────────────────
    if let Some(rest) = e.strip_prefix("name(resolve(") {
        let arg = rest.trim_end_matches(')').trim_end_matches(')');
        return Some(match parse_u64(arg).ok_or(()) {
            Err(()) => Reading::Unresolvable { why: format!("bad tag {arg:?}") },
            Ok(tag) => match tag_resolve(ev, tag).and_then(|r| scenario_name(ev, r)) {
                Ok(s) => Reading::Str(s),
                Err(r) => r,
            },
        });
    }
    if let Some(rest) = e.strip_prefix("bucket(") {
        let (arg, which) = match rest.split_once(").") {
            Some(x) => x,
            None => return Some(Reading::Unresolvable { why: format!("bad bucket() {e:?}") }),
        };
        let field = format!("bucket.{}", which.trim());
        return Some(match parse_u64(arg) {
            None => Reading::Unresolvable { why: format!("bad slot {arg:?}") },
            Some(slot) => match bucket_field(ev, slot, &field) {
                Ok(v) => Reading::Ints(vec![v as i128]),
                Err(r) => r,
            },
        });
    }

    // ── the sobject census ───────────────────────────────────────────────────────────────────
    if e.starts_with("popcount(words[") {
        return Some(match sobjects(ev) {
            Err(r) => r,
            Ok(p) => {
                // ⚠ The bound in the receipt is the CAPACITY. Read it from the pool header rather
                // than trusting the literal, and say so if they disagree.
                let n = e
                    .split("..")
                    .nth(1)
                    .and_then(|t| parse_u64(t.trim_end_matches(|c| c == ']' || c == ')')))
                    .unwrap_or(p.capacity);
                if n != p.capacity {
                    Reading::Unresolvable {
                        why: format!("receipt says {n} but the pool header says capacity {}", p.capacity),
                    }
                } else {
                    match bitvec_set(ev, &p, n) {
                        Ok(v) => Reading::Ints(vec![v.len() as i128]),
                        Err(r) => r,
                    }
                }
            }
        });
    }
    if e == "set(highbyte) - set(bitvec)" || e == "set(bitvec) - set(highbyte)" {
        return Some(match sobjects(ev) {
            Err(r) => r,
            Ok(p) => {
                let hb = match highbyte_set(ev, &p) {
                    Ok(v) => v,
                    Err(r) => return Some(r),
                };
                let bv = match bitvec_set(ev, &p, p.capacity) {
                    Ok(v) => v,
                    Err(r) => return Some(r),
                };
                let hb_i: Vec<u64> = hb.iter().map(|(i, _)| *i).collect();
                let diff: Vec<u64> = if e.starts_with("set(highbyte)") {
                    hb_i.iter().filter(|i| !bv.contains(i)).copied().collect()
                } else {
                    bv.iter().filter(|i| !hb_i.contains(i)).copied().collect()
                };
                Reading::Set(diff)
            }
        });
    }
    if let Some(rest) = e.strip_prefix("count(type ==") {
        let want = parse_u64(rest.trim_end_matches(')').trim());
        return Some(match (sobjects(ev), want) {
            (Err(r), _) => r,
            (_, None) => Reading::Unresolvable { why: format!("bad type in {e:?}") },
            (Ok(p), Some(t)) => match highbyte_set(ev, &p) {
                Ok(v) => Reading::Ints(vec![v.iter().filter(|(_, ty)| *ty as u64 == t).count() as i128]),
                Err(r) => r,
            },
        });
    }
    if e == "self_handle check" {
        return Some(match sobjects(ev) {
            Err(r) => r,
            Ok(p) => {
                let mut ok = 0u64;
                for i in 0..p.high_water {
                    let a = p.element_base + i * p.stride + p.self_handle_off;
                    match ru32(ev, a) {
                        Ok(h) => {
                            if (h as u64 & 0x1FFF) == i {
                                ok += 1;
                            }
                        }
                        Err(r) => return Some(r),
                    }
                }
                Reading::Ratio(ok, p.high_water)
            }
        });
    }

    // ── receipts that are not machine-comparable at all ──────────────────────────────────────
    if e.starts_with("d1-eboot ") || e.starts_with("d1-replay ") || e.starts_with("d1-live ") {
        return Some(Reading::NotEvaluable {
            why: format!("the receipt is a tool invocation, not a memory read: {e}"),
        });
    }
    if !e.contains('@')
        && !e.contains('[')
        && !e.contains("v:")
        && !e.contains("a:")
        && !e.chars().next().is_some_and(|c| c.is_ascii_digit())
    {
        return Some(Reading::NotEvaluable { why: format!("not an address expression: {e}") });
    }
    None
}

/// The whole census, for `d1-scope surfaces` and for control #1's named assertions.
pub struct Census {
    pub high_water: u64,
    pub capacity: u64,
    pub free: u64,
    pub bitvec_live: u64,
    pub highbyte_live: u64,
    pub by_type: Vec<(u8, u64)>,
}

pub fn census(ev: &Evaluator) -> Result<Census, Reading> {
    let p = sobjects(ev)?;
    let hb = highbyte_set(ev, &p)?;
    let bv = bitvec_set(ev, &p, p.capacity)?;
    let mut by: std::collections::BTreeMap<u8, u64> = Default::default();
    for (_, t) in &hb {
        *by.entry(*t).or_default() += 1;
    }
    Ok(Census {
        high_water: p.high_water,
        capacity: p.capacity,
        free: p.free,
        bitvec_live: bv.len() as u64,
        highbyte_live: hb.len() as u64,
        by_type: by.into_iter().collect(),
    })
}
