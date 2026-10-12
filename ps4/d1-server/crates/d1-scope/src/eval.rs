// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The **type layer over the shared address grammar**, and nothing else.
//!
//! ⛔⛔ THIS MODULE CONTAINS NO ADDRESS PARSER. Every address string ends up in
//! [`d1_live::addr::addr`] - byte for byte the function `d1-live watch --addr` calls, with the
//! project's rules intact: *bare = eboot vaddr (`base + N`)*, *`a:`/`abs` = absolute*, *`[…]`
//! dereferences*, *a bare number after `+`/`-` is a plain offset*.
//!
//! What d1-scope owns above that, and why each piece has to exist:
//!
//! | thing | why |
//! |---|---|
//! | [`Ty`] - a **superset** of [`d1_live::addr::Width`] adding `i8/i16/i64/f32/f64` and `T[N]` | the registry's receipts read `i16@…`, `i64@…`, `i32[3]@…`, `f32[3]@…`; `Width` knows five widths |
//! | [`unwrap_parens`] - strips **one matched pair** enclosing the whole address text | the registry writes `u32@(EXPR)`; the shared grammar has no parentheses at all, so `u32@([v:0x…]+0x10)` is a **parse error** in `d1_live::addr` today. ⭐ Counted and reported by control #5; the surface `addr` strings need it **zero** times |
//! | [`expand_resolvers`] - rewrites `resolve(0xTAG)` to `a:0x…` before parsing | SPEC.md §1: *"own the `NOT-EXPRESSIBLE` resolvers (Rust in, `a:0x…` out)"*. The macro expands, then the **shared** grammar parses the result |
//!
//! ⚠ Any one of those three, done sloppily, is a second grammar. They are here, in one file, each
//! with a control, precisely so that nobody adds a fourth somewhere else.

use std::cell::RefCell;

use d1_live::addr::Ctx;
use d1_live::symbols::Symbols;

use crate::mem_source::{MemSource, ReadErr};

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Types

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Ptr,
}

impl Ty {
    pub fn parse(s: &str) -> Option<Ty> {
        Some(match s.trim() {
            "u8" => Ty::U8,
            "u16" => Ty::U16,
            "u32" => Ty::U32,
            "u64" => Ty::U64,
            "i8" => Ty::I8,
            "i16" => Ty::I16,
            "i32" => Ty::I32,
            "i64" => Ty::I64,
            "f32" => Ty::F32,
            "f64" => Ty::F64,
            "ptr" => Ty::Ptr,
            _ => return None,
        })
    }
    pub fn size(self) -> usize {
        match self {
            Ty::U8 | Ty::I8 => 1,
            Ty::U16 | Ty::I16 => 2,
            Ty::U32 | Ty::I32 | Ty::F32 => 4,
            Ty::U64 | Ty::I64 | Ty::F64 | Ty::Ptr => 8,
        }
    }
    pub fn is_float(self) -> bool {
        matches!(self, Ty::F32 | Ty::F64)
    }
    fn decode_int(self, b: &[u8]) -> i128 {
        match self {
            Ty::U8 => b[0] as i128,
            Ty::I8 => b[0] as i8 as i128,
            Ty::U16 => u16::from_le_bytes(b[..2].try_into().unwrap()) as i128,
            Ty::I16 => i16::from_le_bytes(b[..2].try_into().unwrap()) as i128,
            Ty::U32 => u32::from_le_bytes(b[..4].try_into().unwrap()) as i128,
            Ty::I32 => i32::from_le_bytes(b[..4].try_into().unwrap()) as i128,
            Ty::U64 | Ty::Ptr => u64::from_le_bytes(b[..8].try_into().unwrap()) as i128,
            Ty::I64 => i64::from_le_bytes(b[..8].try_into().unwrap()) as i128,
            Ty::F32 | Ty::F64 => unreachable!("float decoded as int"),
        }
    }
    fn decode_float(self, b: &[u8]) -> f64 {
        match self {
            Ty::F32 => f32::from_le_bytes(b[..4].try_into().unwrap()) as f64,
            Ty::F64 => f64::from_le_bytes(b[..8].try_into().unwrap()),
            _ => unreachable!("int decoded as float"),
        }
    }
}

/// `ty[count]@addr_text`, split apart. `ty == None` ⇒ the expression yields an **address**.
#[derive(Clone, Debug)]
pub struct TypedExpr {
    pub ty: Option<Ty>,
    pub count: usize,
    pub addr_text: String,
    /// ⭐ Did [`unwrap_parens`] have to fire? Control #5 counts these.
    pub paren_unwrapped: bool,
}

/// Strip exactly one **matched** paren pair that encloses the entire text.
///
/// ⚠ `(a)+(b)` starts with `(` and ends with `)` and must NOT be unwrapped - the depth has to
/// return to zero only at the final character. Getting this wrong silently changes what an
/// expression means, which is the whole class of bug this project keeps paying for.
pub fn unwrap_parens(s: &str) -> (&str, bool) {
    let t = s.trim();
    if !(t.starts_with('(') && t.ends_with(')')) {
        return (t, false);
    }
    let b = t.as_bytes();
    let mut depth = 0usize;
    for (i, &c) in b.iter().enumerate() {
        match c {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return if i == b.len() - 1 { (t[1..t.len() - 1].trim(), true) } else { (t, false) };
                }
            }
            _ => {}
        }
    }
    (t, false)
}

impl TypedExpr {
    pub fn parse(expr: &str) -> TypedExpr {
        let e = expr.trim();
        // Split at the FIRST `@` - the same rule `d1_live::addr::value` uses.
        if let Some((lhs, rhs)) = e.split_once('@') {
            let (base_ty, count) = match lhs.trim().split_once('[') {
                Some((t, n)) => {
                    let n = n.trim_end_matches(']');
                    (t.to_string(), n.trim().parse::<usize>().unwrap_or(1))
                }
                None => (lhs.trim().to_string(), 1),
            };
            if let Some(ty) = Ty::parse(&base_ty) {
                let (inner, unwrapped) = unwrap_parens(rhs);
                return TypedExpr {
                    ty: Some(ty),
                    count,
                    addr_text: inner.to_string(),
                    paren_unwrapped: unwrapped,
                };
            }
            // Not a width prefix - e.g. the `@` inside "orb@D1Guardian@". Fall through: the whole
            // thing is an address expression (and will fail loudly if it is not one).
        }
        let (inner, unwrapped) = unwrap_parens(e);
        TypedExpr { ty: None, count: 1, addr_text: inner.to_string(), paren_unwrapped: unwrapped }
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Readings

#[derive(Clone, Debug)]
pub enum Reading {
    /// The expression yielded an **address**. Nothing was read yet.
    Addr(u64),
    Ints(Vec<i128>),
    Floats(Vec<f64>),
    Str(String),
    Set(Vec<u64>),
    Ratio(u64, u64),
    /// ⛔ NOT zero. The range is not in this source.
    Unmapped { addr: u64, len: usize },
    /// A hop failed for a reason that is not "unmapped" - the `why` names the hop.
    Unresolvable { why: String },
    /// No machine meaning: the receipt is a tool invocation or prose.
    NotEvaluable { why: String },
}

impl Reading {
    pub fn render(&self) -> String {
        match self {
            Reading::Addr(a) => format!("a:{a:#x}"),
            Reading::Ints(v) => v
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            Reading::Floats(v) => v
                .iter()
                .map(|n| format!("{n:.3}"))
                .collect::<Vec<_>>()
                .join(", "),
            Reading::Str(s) => format!("{s:?}"),
            Reading::Set(v) => format!(
                "{{{}}}",
                v.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",")
            ),
            Reading::Ratio(a, b) => format!("{a} / {b}"),
            Reading::Unmapped { addr, len } => format!("UNMAPPED ({addr:#x}, {len} bytes)"),
            Reading::Unresolvable { why } => format!("UNRESOLVABLE: {why}"),
            Reading::NotEvaluable { why } => format!("NOT-EVALUABLE: {why}"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// The evaluator

pub struct Evaluator<'a> {
    pub ms: &'a dyn MemSource,
    pub syms: &'a Symbols,
    /// ⭐ Bound so that [`crate::resolvers`] takes every address, stride and field offset it uses
    /// FROM THE REGISTRY. Perturb a registry offset and the resolvers must go wrong - that is
    /// control #2's whole claim, and it is only true because of this field.
    pub registry: Option<&'a crate::registry::Registry>,
    pub base: u64,
    /// Every hop, verbatim from `d1_live::addr`. ⭐ A resolution nobody saw is a resolution nobody
    /// checked.
    pub last_trace: RefCell<Vec<String>>,
    /// How many times [`unwrap_parens`] fired. Control #5 reports it.
    pub paren_unwraps: std::cell::Cell<usize>,
}

impl<'a> Evaluator<'a> {
    pub fn new(ms: &'a dyn MemSource, syms: &'a Symbols) -> anyhow::Result<Self> {
        let base = ms
            .base()
            .ok_or_else(|| anyhow::anyhow!("this source has no measured base - a bare vaddr cannot be resolved"))?;
        Ok(Self {
            ms,
            syms,
            registry: None,
            base,
            last_trace: RefCell::new(Vec::new()),
            paren_unwraps: std::cell::Cell::new(0),
        })
    }

    pub fn with_registry(mut self, r: &'a crate::registry::Registry) -> Self {
        self.registry = Some(r);
        self
    }

    /// Resolve an address expression **through the shared grammar**, recording every hop.
    pub fn resolve_addr(&self, text: &str) -> Result<u64, Reading> {
        let expanded = match crate::resolvers::expand_resolvers(self, text) {
            Ok(t) => t,
            Err(r) => return Err(r),
        };
        let deref_err: RefCell<Option<ReadErr>> = RefCell::new(None);
        let read = |a: u64| -> anyhow::Result<u64> {
            match self.ms.read_u64(a) {
                Ok(v) => Ok(v),
                Err(e) => {
                    let msg = e.to_string();
                    *deref_err.borrow_mut() = Some(e);
                    Err(anyhow::anyhow!(msg))
                }
            }
        };
        let mut ctx = Ctx::new(self.base, self.syms);
        let f: &dyn Fn(u64) -> anyhow::Result<u64> = &read;
        ctx.mem = Some(f);
        let out = d1_live::addr::addr(&expanded, &ctx);
        *self.last_trace.borrow_mut() = ctx.take_trace();
        match out {
            Ok(a) => Ok(a),
            Err(e) => {
                if let Some(ReadErr::Unmapped { addr, len }) = deref_err.borrow().clone() {
                    // ⭐ A deref that walked off the captured set is UNMAPPED, not "unresolvable"
                    // and above all not zero.
                    Err(Reading::Unmapped { addr, len })
                } else {
                    Err(Reading::Unresolvable { why: format!("{e:#}") })
                }
            }
        }
    }

    /// Evaluate one registry expression to a [`Reading`].
    pub fn eval(&self, expr: &str) -> Reading {
        // 1. Whole-expression resolvers (census, tag-registry accessors) come first: they are not
        //    address expressions at all.
        if let Some(r) = crate::resolvers::eval_whole(self, expr) {
            return r;
        }
        // 2. Everything else is `ty[count]@addr` or a bare address.
        let te = TypedExpr::parse(expr);
        if te.paren_unwrapped {
            self.paren_unwraps.set(self.paren_unwraps.get() + 1);
        }
        let addr = match self.resolve_addr(&te.addr_text) {
            Ok(a) => a,
            Err(r) => return r,
        };
        match te.ty {
            None => Reading::Addr(addr),
            Some(ty) => self.read_typed(ty, te.count, addr),
        }
    }

    pub fn read_typed(&self, ty: Ty, count: usize, addr: u64) -> Reading {
        let len = ty.size() * count;
        let bytes = match self.ms.read_vec(addr, len) {
            Ok(b) => b,
            Err(ReadErr::Unmapped { addr, len }) => return Reading::Unmapped { addr, len },
            Err(e) => return Reading::Unresolvable { why: e.to_string() },
        };
        if ty.is_float() {
            Reading::Floats(
                bytes.chunks_exact(ty.size()).map(|c| ty.decode_float(c)).collect(),
            )
        } else {
            Reading::Ints(bytes.chunks_exact(ty.size()).map(|c| ty.decode_int(c)).collect())
        }
    }

    pub fn read_cstr(&self, addr: u64, max: usize) -> Reading {
        match self.ms.read_cstr(addr, max) {
            Ok(s) => Reading::Str(s),
            Err(ReadErr::Unmapped { addr, len }) => Reading::Unmapped { addr, len },
            Err(e) => Reading::Unresolvable { why: e.to_string() },
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Syntax-only checking - "does the SHARED grammar accept this?", with no memory involved.

/// A dereference source that always succeeds, so that the only thing that can fail is **syntax**.
///
/// ⭐ Control #5 asks *"does every registry address parse in `d1_live::addr`?"*, and asking it
/// against a real snapshot would conflate a **parse error** with an **unmapped pointee** - two
/// facts with opposite meanings ("the registry is malformed" vs "this snapshot did not capture
/// that heap"). Feeding the grammar a constant memory removes the second failure mode entirely.
pub const PARSE_CHECK_BASE: u64 = 0x8_0000_0000;

pub fn parse_check(text: &str, syms: &Symbols) -> Result<u64, String> {
    // `resolve(0x…)` is a d1-scope macro; substitute a syntactically valid stand-in so the SHARED
    // grammar sees only terms it knows.
    let mut t = text.to_string();
    while let Some(i) = t.find("resolve(") {
        let Some(close) = t[i..].find(')') else { break };
        t.replace_range(i..i + close + 1, "a:0x1000");
    }
    let te = TypedExpr::parse(&t);
    let ctx = {
        let mut c = Ctx::new(PARSE_CHECK_BASE, syms);
        let f: &dyn Fn(u64) -> anyhow::Result<u64> = &|_a: u64| Ok(0x1000);
        c.mem = Some(f);
        c
    };
    d1_live::addr::addr(&te.addr_text, &ctx).map_err(|e| format!("{e:#}"))
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────
// Expected values, as written in the registry's `value` receipts.

#[derive(Clone, Debug)]
pub enum Expected {
    /// `v:0x…` (an eboot vaddr) or `a:0x…` (absolute).
    Addr { raw: u64, is_vaddr: bool },
    Num(i128),
    Nums(Vec<f64>),
    Str(String),
    AllZero { len: usize },
    Unmapped,
    Bytes(Vec<u8>),
    Set(Vec<u64>),
    Ratio(u64, u64),
    /// Prose, an elided byte dump, or a tool invocation. ⇒ SKIP, with the text, never a silent pass.
    Opaque(String),
}

fn parse_num(t: &str) -> Option<i128> {
    let t = t.trim();
    let (neg, t) = match t.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, t),
    };
    let v = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        i128::from_str_radix(h, 16).ok()?
    } else {
        t.parse::<i128>().ok()?
    };
    Some(if neg { -v } else { v })
}

impl Expected {
    pub fn parse(value: &str) -> Expected {
        let v = value.trim();
        if v.eq_ignore_ascii_case("UNMAPPED") {
            return Expected::Unmapped;
        }
        if v.starts_with('"') {
            if let Some(end) = v[1..].rfind('"') {
                return Expected::Str(v[1..1 + end].to_string());
            }
        }
        if v.starts_with("all zero") {
            // "all zero (0x20 read)" / "all zero"
            let len = v
                .split_once('(')
                .and_then(|(_, r)| r.split_whitespace().next())
                .and_then(|n| parse_num(n))
                .unwrap_or(0x20) as usize;
            return Expected::AllZero { len };
        }
        if v.contains('…') || v.contains("...") {
            return Expected::Opaque(format!("elided byte dump: {v}"));
        }
        if v.starts_with('{') {
            let inner: String =
                v.trim_start_matches('{').split('}').next().unwrap_or("").to_string();
            let items: Vec<u64> = inner
                .split(',')
                .filter_map(|t| parse_num(t))
                .map(|n| n as u64)
                .collect();
            return Expected::Set(items);
        }
        {
            let parts: Vec<&str> = v.split('/').map(|s| s.trim()).collect();
            if parts.len() == 2 {
                if let (Some(a), Some(b)) = (parse_num(parts[0]), parse_num(parts[1])) {
                    return Expected::Ratio(a as u64, b as u64);
                }
            }
        }
        {
            let toks: Vec<&str> = v.split_whitespace().collect();
            if toks.len() >= 2
                && toks.iter().all(|t| t.len() == 2 && t.chars().all(|c| c.is_ascii_hexdigit()))
            {
                return Expected::Bytes(
                    toks.iter().map(|t| u8::from_str_radix(t, 16).unwrap()).collect(),
                );
            }
        }
        if let Some(rest) = v.strip_prefix("v:") {
            if let Some(n) = parse_num(rest) {
                return Expected::Addr { raw: n as u64, is_vaddr: true };
            }
        }
        if let Some(rest) = v.strip_prefix("a:") {
            if let Some(n) = parse_num(rest) {
                return Expected::Addr { raw: n as u64, is_vaddr: false };
            }
        }
        if v.contains(',') {
            let parts: Vec<&str> = v.split(',').map(|s| s.trim()).collect();
            let nums: Vec<Option<f64>> = parts
                .iter()
                .map(|p| parse_num(p).map(|n| n as f64).or_else(|| p.parse::<f64>().ok()))
                .collect();
            if nums.iter().all(|n| n.is_some()) {
                return Expected::Nums(nums.into_iter().map(|n| n.unwrap()).collect());
            }
        }
        if let Some(n) = parse_num(v) {
            return Expected::Num(n);
        }
        Expected::Opaque(v.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Skip,
}

pub struct Judgement {
    pub verdict: Verdict,
    pub got: String,
    pub why: String,
}

/// Compare a [`Reading`] against an [`Expected`], reading memory where the expectation demands it.
///
/// ⛔ The three no-value outcomes are kept apart to the end: `Unmapped` never satisfies a numeric
/// expectation, an `Opaque` receipt is a **SKIP with its text**, never a silent pass, and an
/// `Unresolvable` is a FAIL that names the hop.
pub fn judge(ev: &Evaluator, reading: &Reading, expected: &Expected) -> Judgement {
    let pass = |got: String| Judgement { verdict: Verdict::Pass, got, why: String::new() };
    let fail = |got: String, why: String| Judgement { verdict: Verdict::Fail, got, why };
    let skip = |got: String, why: String| Judgement { verdict: Verdict::Skip, got, why };

    if let Expected::Opaque(t) = expected {
        return skip(reading.render(), format!("receipt is not machine-comparable: {t}"));
    }
    if let Reading::NotEvaluable { why } = reading {
        return skip(reading.render(), why.clone());
    }

    // UNMAPPED is symmetric and absolute: it satisfies exactly one expectation and no other.
    if let Reading::Unmapped { .. } = reading {
        return if matches!(expected, Expected::Unmapped) {
            pass(reading.render())
        } else {
            fail(reading.render(), "the range is not in this source".into())
        };
    }
    if matches!(expected, Expected::Unmapped) {
        // The expression resolved; the expectation says the BYTES are not captured. Touch them.
        if let Reading::Addr(a) = reading {
            return match ev.ms.read_vec(*a, 8) {
                Err(ReadErr::Unmapped { .. }) => pass(format!("UNMAPPED at {a:#x}")),
                Ok(b) => fail(
                    format!("{a:#x} -> {b:02x?}"),
                    "expected UNMAPPED but the bytes are captured".into(),
                ),
                Err(e) => fail(format!("{a:#x}"), e.to_string()),
            };
        }
        return fail(reading.render(), "expected UNMAPPED".into());
    }

    match (reading, expected) {
        (Reading::Unresolvable { why }, _) => fail(reading.render(), why.clone()),

        (Reading::Addr(a), Expected::Addr { raw, is_vaddr }) => {
            let want = if *is_vaddr { ev.base + raw } else { *raw };
            if *a == want {
                pass(format!("a:{a:#x}"))
            } else {
                fail(format!("a:{a:#x}"), format!("expected a:{want:#x}"))
            }
        }
        (Reading::Addr(a), Expected::Num(n)) => {
            if *a as i128 == *n {
                pass(format!("{a:#x}"))
            } else {
                fail(format!("{a:#x}"), format!("expected {n:#x}"))
            }
        }
        (Reading::Addr(a), Expected::Str(s)) => match ev.read_cstr(*a, 128) {
            Reading::Str(got) => {
                if got == *s {
                    pass(format!("{got:?}"))
                } else {
                    fail(format!("{got:?}"), format!("expected {s:?}"))
                }
            }
            other => fail(other.render(), format!("expected {s:?}")),
        },
        (Reading::Addr(a), Expected::AllZero { len }) => match ev.ms.read_vec(*a, *len) {
            Ok(b) if b.iter().all(|&x| x == 0) => pass(format!("{len} bytes, all zero")),
            Ok(b) => fail(
                format!("{:02x?}…", &b[..8.min(b.len())]),
                format!("expected {len} zero bytes"),
            ),
            Err(e) => fail(e.to_string(), "expected zeros; the range is not readable".into()),
        },
        (Reading::Addr(a), Expected::Bytes(want)) => match ev.ms.read_vec(*a, want.len()) {
            Ok(b) if b == *want => pass(
                b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "),
            ),
            Ok(b) => fail(
                b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "),
                "byte mismatch".into(),
            ),
            Err(e) => fail(e.to_string(), "expected bytes".into()),
        },

        (Reading::Ints(v), Expected::Num(n)) => {
            if v.len() == 1 && v[0] == *n {
                pass(format!("{} ({:#x})", v[0], v[0] as u64))
            } else {
                fail(reading.render(), format!("expected {n}"))
            }
        }
        (Reading::Ints(v), Expected::Nums(want)) => {
            let got: Vec<f64> = v.iter().map(|n| *n as f64).collect();
            if got.len() == want.len() && got.iter().zip(want).all(|(a, b)| a == b) {
                pass(reading.render())
            } else {
                fail(reading.render(), format!("expected {want:?}"))
            }
        }
        (Reading::Ints(v), Expected::Addr { raw, is_vaddr }) => {
            let want = if *is_vaddr { ev.base + raw } else { *raw } as i128;
            if v.len() == 1 && v[0] == want {
                pass(format!("{:#x}", v[0]))
            } else {
                fail(reading.render(), format!("expected {want:#x}"))
            }
        }
        (Reading::Floats(v), Expected::Nums(want)) => {
            // The receipts are printed to three decimals; compare at that resolution and no finer.
            let ok = v.len() == want.len()
                && v.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-3);
            if ok {
                pass(reading.render())
            } else {
                fail(reading.render(), format!("expected {want:?} (±1e-3)"))
            }
        }
        (Reading::Str(got), Expected::Str(s)) => {
            if got == s {
                pass(format!("{got:?}"))
            } else {
                fail(format!("{got:?}"), format!("expected {s:?}"))
            }
        }
        (Reading::Set(got), Expected::Set(want)) => {
            let mut a = got.clone();
            let mut b = want.clone();
            a.sort_unstable();
            b.sort_unstable();
            if a == b {
                pass(reading.render())
            } else {
                fail(reading.render(), format!("expected {want:?}"))
            }
        }
        (Reading::Ratio(a, b), Expected::Ratio(wa, wb)) => {
            if a == wa && b == wb {
                pass(reading.render())
            } else {
                fail(reading.render(), format!("expected {wa} / {wb}"))
            }
        }
        (Reading::Ints(v), Expected::Ratio(wa, wb)) => {
            // A ratio receipt against a single count is a shape mismatch, not a pass.
            fail(reading.render(), format!("expected the ratio {wa} / {wb}, got {} value(s)", v.len()))
        }
        (r, e) => fail(r.render(), format!("shape mismatch against {e:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_parens_only_strips_a_matched_enclosing_pair() {
        assert_eq!(unwrap_parens("([v:0x10]+0x8)"), ("[v:0x10]+0x8", true));
        assert_eq!(unwrap_parens("[v:0x10]+0x8"), ("[v:0x10]+0x8", false));
        // ⭐ THE ONE THAT MATTERS: two sibling groups must not be unwrapped into `a)+(b`.
        assert_eq!(unwrap_parens("(a)+(b)"), ("(a)+(b)", false));
        assert_eq!(unwrap_parens("(resolve(0x1)+0x0C)"), ("resolve(0x1)+0x0C", true));
    }

    #[test]
    fn typed_prefix_is_a_superset_of_the_shared_width_set() {
        // The five the shared grammar knows…
        for w in ["u8", "u16", "u32", "u64", "i32"] {
            assert!(d1_live::addr::Width::parse(w).is_some());
            assert!(Ty::parse(w).is_some());
        }
        // …and the ones the registry needs that it does not.
        for w in ["i8", "i16", "i64", "f32"] {
            assert!(d1_live::addr::Width::parse(w).is_none(), "{w} unexpectedly known upstream");
            assert!(Ty::parse(w).is_some(), "{w} must be known here");
        }
    }

    #[test]
    fn typed_expr_splits_at_the_first_at_sign() {
        let t = TypedExpr::parse("i32[3]@([v:0x27E7F18]+0x90)");
        assert_eq!(t.ty, Some(Ty::I32));
        assert_eq!(t.count, 3);
        assert_eq!(t.addr_text, "[v:0x27E7F18]+0x90");
        assert!(t.paren_unwrapped);
        // An `@` that is not a width prefix leaves the expression alone.
        let t = TypedExpr::parse("[v:0x27E7F18]+0x1C");
        assert_eq!(t.ty, None);
    }

    #[test]
    fn expected_receipts_parse_into_the_right_shapes() {
        assert!(matches!(Expected::parse("UNMAPPED"), Expected::Unmapped));
        assert!(matches!(Expected::parse("\"sobjects\""), Expected::Str(_)));
        assert!(matches!(Expected::parse("all zero (0x20 read)"), Expected::AllZero { len: 0x20 }));
        assert!(matches!(Expected::parse("577 / 577"), Expected::Ratio(577, 577)));
        assert!(matches!(Expected::parse("{} (empty)"), Expected::Set(_)));
        assert!(matches!(Expected::parse("ff ff ff ff ff ff ff ff"), Expected::Bytes(_)));
        assert!(matches!(Expected::parse("a:0x22D5A6E00"), Expected::Addr { is_vaddr: false, .. }));
        assert!(matches!(Expected::parse("v:0x26B5588"), Expected::Addr { is_vaddr: true, .. }));
        assert!(matches!(Expected::parse("-1, -1, -1"), Expected::Nums(_)));
        assert!(matches!(Expected::parse("0x7FF"), Expected::Num(0x7FF)));
        // ⭐ An elided dump is a SKIP, not a pass-by-accident.
        assert!(matches!(Expected::parse("01 00 00 00 … ff ff ff ff"), Expected::Opaque(_)));
    }
}
