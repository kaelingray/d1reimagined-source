// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! **The function atlas** - ATLAS Layer 4, generated rather than hand-written.
//!
//! `design/eboot-map.md` names 164 addresses; the six map documents together name about 500. The
//! image has **35,834** prologue-anchored functions. Hand-curation was never going to close that,
//! so this module merges every mechanically-harvestable name source into one row per function:
//!
//! | source | what it gives | reach |
//! |---|---|---|
//! | [`crate::rtti`] | `class::vfunc[N]` - the class the compiler recorded, and the slot | `static-client-inference`, and the compiler's own output |
//! | [`crate::strings`] `SourcePath` | the `.cpp` the function's assert/log macro embedded | ditto, exact |
//! | [`crate::strings`] `Identifier` | the named things a function reaches for | ditto, suggestive |
//! | [`crate::functions`] | callers/callees, leaf/tail, decode quality | structural |
//!
//! ## What a name here does and does not claim
//!
//! `c_investment_bap_manager::vfunc[7]` is a **true statement about what the client calls this
//! function for**. It is not the identifier Bungie typed - that is unrecoverable and stated as such
//! in ATLAS §2. Treat the class as ownership and the slot as role.
//!
//! ## The probe warnings are part of the map
//!
//! ATLAS §4.4c cost a run: arming a UD2 over `0x56CA70` - the registry `find`, a hot leaf called
//! from several threads - is cross-modifying code without serialisation, and the client lost its
//! BAP connection. A function's *caller count* is exactly the number that predicts that, so it
//! travels with the row rather than in a doc nobody re-reads.

use std::collections::{HashMap, HashSet};

use crate::functions::{self, Anchored, Index};
use crate::image::{Image, Result};
use crate::rtti::{self, Rtti};
use crate::strings::{Corpus, StrKind};

/// Caller count at or above which a function is a shared hot leaf - do not probe it, probe the
/// specific caller (ATLAS §4.4c).
pub const HOT_CALLERS: usize = 40;

#[derive(Debug, Clone)]
pub struct Row {
    pub anchored: Anchored,
    pub start: u64,
    pub end: u64,
    pub insns: u32,
    pub clean: bool,
    pub padded: bool,
    pub leaf: bool,
    pub tail: bool,
    pub n_callers: usize,
    pub n_callees: usize,
    /// Demangled owning class, from RTTI.
    pub class: String,
    pub vslot: Option<usize>,
    /// How many distinct vtables carry this function - >1 means an inherited implementation.
    pub vt_shared: usize,
    /// `__FILE__` path the function references.
    pub source_file: String,
    /// Up to three strings the function reaches.
    pub strings: Vec<String>,
    /// Classes whose **vtable pointer** this function loads - i.e. it constructs or destroys one.
    pub vptr_classes: Vec<String>,
    /// Corpus spellings whose FNV-1 appears as a 32-bit immediate in this function's code.
    pub hash_names: Vec<String>,
    /// `0x80xxxxxx` schema/class tags that appear as immediates - the objects this code encodes.
    pub tags: Vec<u32>,
    /// Named library imports this function calls (via their PLT stubs).
    pub imports: Vec<String>,
    pub name: String,
    pub name_source: &'static str,
}

impl Default for Row {
    fn default() -> Self {
        Self {
            anchored: Anchored::Prologue,
            start: 0,
            end: 0,
            insns: 0,
            clean: false,
            padded: false,
            leaf: false,
            tail: false,
            n_callers: 0,
            n_callees: 0,
            class: String::new(),
            vslot: None,
            vt_shared: 0,
            source_file: String::new(),
            strings: Vec::new(),
            vptr_classes: Vec::new(),
            hash_names: Vec::new(),
            tags: Vec::new(),
            imports: Vec::new(),
            name: String::new(),
            name_source: "none",
        }
    }
}

impl Row {
    pub fn named(&self) -> bool {
        self.name_source != "none"
    }
    pub fn warn(&self) -> String {
        let mut w = Vec::new();
        if self.n_callers >= HOT_CALLERS {
            w.push("hot");
        }
        if self.leaf {
            w.push("leaf");
        }
        if self.tail {
            w.push("tail");
        }
        if !self.clean {
            w.push("decode-dirty");
        }
        if !self.padded {
            w.push("unpadded");
        }
        w.join(",")
    }
}

pub struct Atlas {
    pub rows: Vec<Row>,
    pub rtti: Rtti,
    pub index: Index,
    pub corpus: Corpus,
}

/// The bases of the source trees the build embedded, longest first - trimmed off `__FILE__` paths
/// so the column is the file, not the build machine's directory layout.
const PATH_PREFIXES: [&str; 5] = [
    "bungie/external_libraries/",
    "bungie/evil/shared/engine/source/",
    "bungie/evil/",
    "devel/projects/",
    "perforce/Clients/",
];

/// Normalise a `__FILE__` to `component/path/file.cpp`.
///
/// ⚠ The literals carry a **drive letter** (`d:\bungie\evil\…`, `C:/perforce/…`). An earlier
/// pass here stripped `\bungie\…` and matched nothing, which silently produced full build-machine
/// paths in the artifact rather than an error - the quiet kind of wrong.
/// A string used as a name: its `subsystem:` prefix if it has one, else its first 48 characters.
/// `world_controller:state:orbit_loading: Failed to find the orbit slice-set name for …` is a
/// perfectly good identity and a terrible label.
fn short(s: &str) -> String {
    let head: String = s.chars().take(64).collect();
    if let Some(i) = head.rfind(": ") {
        let pre = &head[..i];
        if !pre.is_empty()
            && pre.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b':')
        {
            return pre.to_string();
        }
    }
    head.chars().take(48).collect()
}

fn short_path(p: &str) -> String {
    let s = p.replace('\\', "/");
    let s = match s.as_bytes() {
        [c, b':', b'/', ..] if c.is_ascii_alphabetic() => s[3..].to_string(),
        _ => s,
    };
    for pre in PATH_PREFIXES {
        if let Some(rest) = s.strip_prefix(pre) {
            return rest.to_string();
        }
    }
    s
}

/// Generic library calls. A function that calls `memcpy` has told you nothing; one that calls
/// `sceVoiceCreatePort` has told you what it is. Ranked last so a specific import always wins.
const DULL_IMPORTS: [&str; 24] = [
    "memcpy", "memset", "memmove", "memcmp", "strlen", "strcmp", "strncmp", "strcpy", "strncpy",
    "malloc", "free", "calloc", "realloc", "snprintf", "vsnprintf", "sprintf", "abort", "qsort",
    "__cxa_atexit", "__cxa_guard_acquire", "__cxa_guard_release", "_Znwm", "_ZdlPv", "__stack_chk_fail",
];

pub fn build(image: &Image, dict: &[String]) -> Result<Atlas> {
    let rtti = rtti::harvest(image)?;
    let index = functions::build(image)?;
    let mut corpus = Corpus::build(image, 3)?;
    corpus.add_names(rtti.types.iter().map(|t| t.name.clone()));

    let starts = index.starts();
    let callers = index.callers();
    let callees = index.callees();
    let by_vaddr = corpus.by_vaddr();
    let owner = rtti.owner_of();
    let ti_name: HashMap<u64, &str> = rtti.types.iter().map(|t| (t.ti, t.name.as_str())).collect();

    // how many distinct vtables carry each function - inherited implementations show up in many
    let mut vt_count: HashMap<u64, HashSet<u64>> = HashMap::new();
    for vt in &rtti.vtables {
        for &f in &vt.slots {
            if f != 0 {
                vt_count.entry(f).or_default().insert(vt.vptr);
            }
        }
    }

    // ⭐ vtable references: a constructor installs the vptr, so a function that reaches a vtable
    // RIP-relatively is that class's ctor, dtor or factory - which names functions RTTI's slot
    // walk cannot reach, because a constructor is never itself a virtual slot.
    //
    // ⚠ **It references the vtable GROUP START (`vptr - 0x10`), not the vptr.** That is the `_ZTV`
    // symbol - offset-to-top first, typeinfo second, slot 0 at +0x10 - and the `+0x10` is folded
    // into the store, so a scan aimed at the vptr finds **3** references image-wide and reads as
    // "constructors don't do this". They do; the aim was wrong (ATLAS §4.2b: the control proves the
    // instrument, never the target). Measured: `0x5745E0 lea rax,[rip+0x13814f9]` -> `0x18F5AE0`,
    // which is `s_investment_utility_context`'s vtable group, inside `0x574460`.
    let mut vptr_owner: HashMap<u64, u64> = HashMap::new();
    for v in &rtti.vtables {
        vptr_owner.insert(v.vptr, v.ti);
        vptr_owner.insert(v.vptr - 0x10, v.ti);
    }

    // per-function string references, resolved through the corpus
    let mut fn_paths: HashMap<u64, String> = HashMap::new();
    let mut fn_strings: HashMap<u64, Vec<String>> = HashMap::new();
    let mut fn_vptr: HashMap<u64, Vec<String>> = HashMap::new();
    for &(f, target) in &index.riprefs {
        if let Some(ti) = vptr_owner.get(&target) {
            if let Some(n) = ti_name.get(ti) {
                let e = fn_vptr.entry(f).or_default();
                if !e.iter().any(|x| x == n) {
                    e.push((*n).to_string());
                }
            }
            continue;
        }
        let Some(s) = by_vaddr.get(&target) else { continue };
        match s.kind {
            StrKind::SourcePath => {
                fn_paths.entry(f).or_insert_with(|| short_path(&s.text));
            }
            // Any string long enough to be a topic. The diagnostic *format* strings are the
            // richest of these - `capture-rig/diagnostic-catalogue.py` turns their `subsystem:`
            // prefix into a subsystem, and the merge in `capture-rig/eboot-atlas.py` joins that
            // back in rather than re-deriving it here (one derivation, one place).
            StrKind::Identifier | StrKind::Format | StrKind::Other if s.len >= 4 => {
                let e = fn_strings.entry(f).or_default();
                if e.len() < 3 && !e.contains(&s.text) {
                    e.push(s.text.chars().take(80).collect());
                }
            }
            _ => {}
        }
    }

    // ⭐ FNV-1 name-hash immediates. THE-CHAIN §4.2: `patrol`, `arena`, `trials_of_osiris`,
    // `salvage`, `rift`, `orbit`, `tower` are all compiled in **as hashes only** - the spelling
    // appears nowhere in the image - so this reaches names no string scan can. The measured base
    // rate for a random u32 occurring in the image is 0.130%, which is what makes one hit evidence.
    //
    // ⚠ Single-token lookup only. Composite (`A_B`) meet-in-the-middle cracking has a **42%
    // false-positive rate per target** (THE-CHAIN §4.4) and has no place in a generated artifact.
    let mut fn_hashes: HashMap<u64, Vec<String>> = HashMap::new();
    let mut fn_tags: HashMap<u64, Vec<u32>> = HashMap::new();
    for &(f, imm) in &index.imms {
        if imm >> 24 == 0x80 {
            let e = fn_tags.entry(f).or_default();
            if e.len() < 6 && !e.contains(&imm) {
                e.push(imm);
            }
            continue;
        }
        for n in corpus.resolve(imm) {
            let e = fn_hashes.entry(f).or_default();
            if e.len() < 4 && !e.contains(n) {
                e.push(n.clone());
            }
        }
    }

    // ⭐ Library imports. 638 symbols, 638 PLT stubs, and 19,510 functions call one - the single
    // largest naming signal left in the image after RTTI (`eboot-atlas-roadmap.md` §2.1).
    let mut imps = crate::dynsym::imports(image)?;
    crate::dynsym::resolve(&mut imps, dict.iter().cloned());
    let stub_name: HashMap<u64, String> = imps
        .iter()
        .filter_map(|i| Some((i.plt_stub?, i.name.clone()?)))
        .collect();
    let mut fn_imports: HashMap<u64, Vec<String>> = HashMap::new();
    for &(f, t) in &index.calls {
        if let Some(n) = stub_name.get(&t) {
            let e = fn_imports.entry(f).or_default();
            if !e.contains(n) {
                e.push(n.clone());
            }
        }
    }
    for v in fn_imports.values_mut() {
        v.sort_by_key(|n| (DULL_IMPORTS.contains(&n.as_str()), n.clone()));
    }

    let mut rows = Vec::with_capacity(index.funcs.len());
    for f in &index.funcs {
        let mut r = Row {
            anchored: f.anchored,
            start: f.start,
            end: f.end,
            insns: f.insns,
            clean: f.clean,
            padded: f.padded,
            leaf: f.leaf,
            tail: f.tail,
            n_callers: callers.get(&f.start).map(|v| v.len()).unwrap_or(0),
            n_callees: callees.get(&f.start).map(|v| v.len()).unwrap_or(0),
            source_file: fn_paths.get(&f.start).cloned().unwrap_or_default(),
            strings: fn_strings.get(&f.start).cloned().unwrap_or_default(),
            vptr_classes: fn_vptr.get(&f.start).cloned().unwrap_or_default(),
            hash_names: fn_hashes.get(&f.start).cloned().unwrap_or_default(),
            tags: fn_tags.get(&f.start).cloned().unwrap_or_default(),
            imports: fn_imports.get(&f.start).cloned().unwrap_or_default(),
            name_source: "none",
            ..Default::default()
        };
        if let Some(&(ti, slot)) = owner.get(&f.start) {
            r.class = ti_name.get(&ti).copied().unwrap_or("").to_string();
            r.vslot = Some(slot);
            r.vt_shared = vt_count.get(&f.start).map(|s| s.len()).unwrap_or(0);
            if !r.class.is_empty() {
                r.name = format!("{}::vfunc[{}]", r.class, slot);
                r.name_source = "rtti";
            }
        }
        // ⚠ Order matters, and the first draft had it wrong. A vptr install is a strong ctor/dtor
        // signal ONLY when the function has nothing better to say about itself: `0x753540` - the
        // change-world entry of THE-CHAIN §6.1 - installs a `c_fiber_game_system` vptr *and*
        // references `change_world_options`, and calling it `c_fiber_game_system::ctor_or_dtor` is
        // a true statement that buries the useful one. Strings win; the vptr classes stay in their
        // own column so nothing is lost.
        // A `__FILE__` is the most precise textual evidence there is - the compiler put it there.
        if r.name.is_empty() && !r.source_file.is_empty() {
            r.name = format!("{}@{:#x}", r.source_file, f.start);
            r.name_source = "source-file";
        }
        if r.name.is_empty() && !r.strings.is_empty() {
            r.name = format!("~{}", short(&r.strings[0]));
            r.name_source = "string-ref";
        }
        // A function that installs exactly ONE class's vptr is that class's ctor/dtor. Installing
        // several is a factory or an inlined chain, so the row keeps the list and the name says so
        // rather than picking one arbitrarily.
        if r.name.is_empty() && !r.vptr_classes.is_empty() {
            if r.class.is_empty() {
                r.class = r.vptr_classes[0].clone();
            }
            r.name = if r.vptr_classes.len() == 1 {
                format!("{}::ctor_or_dtor", r.vptr_classes[0])
            } else {
                format!("{}::ctor_or_dtor+{}", r.vptr_classes[0], r.vptr_classes.len() - 1)
            };
            r.name_source = "rtti-vptr";
        }
        // A named import is a behavioural label: `calls sceVoiceCreatePort` says what the function
        // DOES. Ranked below the textual sources (which say what it IS) and above nothing.
        if r.name.is_empty() && r.imports.first().is_some_and(|n| !DULL_IMPORTS.contains(&n.as_str())) {
            r.name = format!("~calls:{}", r.imports[0]);
            r.name_source = "import";
        }
        if r.name.is_empty() && !r.hash_names.is_empty() {
            r.name = format!("~fnv1:{}", r.hash_names.join("/"));
            r.name_source = "name-hash";
        }
        if r.name.is_empty() && !r.tags.is_empty() {
            r.name = format!("~schema:{:#010x}", r.tags[0]);
            r.name_source = "schema-tag";
        }
        rows.push(r);
    }

    // ── thunk propagation ───────────────────────────────────────────────────────────────────
    // A one-to-four-instruction body ending in `jmp elsewhere` is a thunk: an adjustor for a
    // secondary base, an alias, or a tail-call wrapper. It has the identity of what it jumps to,
    // and naming it that is a statement about the code, not a guess.
    //
    // ⚠ ATLAS §4.2's whole lesson lives here: a `call`-only scan cannot see these edges at all -
    // `0x56FFB0` is entered by `jmp` from `0x560D27` and has ZERO call sites.
    let name_by_start: HashMap<u64, (String, &'static str)> = rows
        .iter()
        .filter(|r| r.named())
        .map(|r| (r.start, (r.name.clone(), r.name_source)))
        .collect();
    let tail_target: HashMap<u64, u64> = index.tailcalls.iter().copied().collect();
    let mut thunks = 0usize;
    for r in rows.iter_mut() {
        if r.named() || r.insns > 4 || !r.tail {
            continue;
        }
        let Some(t) = tail_target.get(&r.start) else { continue };
        let Some((n, _)) = name_by_start.get(t) else { continue };
        r.name = format!("thunk -> {n}");
        r.name_source = "thunk";
        thunks += 1;
    }
    let _ = (thunks, &starts);
    Ok(Atlas { rows, rtti, index, corpus })
}

impl Atlas {
    pub fn coverage(&self) -> Coverage {
        let mut c = Coverage { total: self.rows.len(), ..Default::default() };
        for r in &self.rows {
            if r.anchored == Anchored::Prologue {
                c.prologue += 1;
                if r.named() {
                    c.prologue_named += 1;
                }
            } else {
                c.call_target += 1;
                if r.named() {
                    c.call_target_named += 1;
                }
            }
            match r.name_source {
                "rtti" => c.rtti += 1,
                "rtti-vptr" => c.rtti_vptr += 1,
                "source-file" => c.source_file += 1,
                "string-ref" => c.string_ref += 1,
                "name-hash" => c.name_hash += 1,
                "schema-tag" => c.schema_tag += 1,
                "thunk" => c.thunk += 1,
                "import" => c.import += 1,
                _ => {}
            }
            if r.named() {
                c.named += 1;
            }
            if !r.clean {
                c.dirty += 1;
            }
            if r.leaf {
                c.leaf += 1;
            }
            if r.n_callers == 0 {
                c.no_callers += 1;
            }
            if r.insns <= 4 {
                c.tiny += 1;
            }
        }
        c
    }
}

#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
pub struct Coverage {
    pub total: usize,
    pub named: usize,
    /// `55 48 89 e5` functions - the 35,834 denominator every published percentage uses.
    pub prologue: usize,
    pub prologue_named: usize,
    /// Frame-pointer-omitted functions, found as direct `call` targets. Not in the denominator.
    pub call_target: usize,
    pub call_target_named: usize,
    pub rtti: usize,
    pub rtti_vptr: usize,
    pub source_file: usize,
    pub string_ref: usize,
    pub name_hash: usize,
    pub schema_tag: usize,
    pub thunk: usize,
    pub import: usize,
    pub dirty: usize,
    pub leaf: usize,
    pub no_callers: usize,
    pub tiny: usize,
}

pub const TSV_HEADER: &str = "vaddr\tanchored\tend\tsize\tinsns\tname\tname_source\tclass\tvslot\tvt_shared\tvptr_classes\tsource_file\thash_names\ttags\timports\tn_callers\tn_callees\twarn\tstrings";

fn clean_field(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

pub fn tsv_row(r: &Row) -> String {
    format!(
        "{:#x}\t{}\t{:#x}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        r.start,
        if r.anchored == Anchored::Prologue { "prologue" } else { "call-target" },
        r.end,
        r.end.saturating_sub(r.start),
        r.insns,
        clean_field(&r.name),
        r.name_source,
        clean_field(&r.class),
        r.vslot.map(|v| v.to_string()).unwrap_or_default(),
        if r.vt_shared > 0 { r.vt_shared.to_string() } else { String::new() },
        clean_field(&r.vptr_classes.join(" | ")),
        clean_field(&r.source_file),
        clean_field(&r.hash_names.join(" | ")),
        r.tags.iter().map(|t| format!("{t:#010x}")).collect::<Vec<_>>().join(" "),
        clean_field(&r.imports.join(" | ")),
        r.n_callers,
        r.n_callees,
        r.warn(),
        clean_field(&r.strings.join(" | ")),
    )
}
