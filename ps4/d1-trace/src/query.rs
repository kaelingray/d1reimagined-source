// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! The search predicate - the feature that earns the tool.
//!
//! Grammar (space-separated terms, ANDed):
//!   - `stream:<http|bap|probe|guest>`      restrict to one stream
//!   - `probe:<substr>`                     restrict to probe events whose name contains substr
//!   - `<field>[&<mask>]<op><value>`        register/field predicate (probe events only)
//!         fields: rax..r15, rip, rflags, guest_vaddr, seq, probe_id
//!         ops:    == != <= >= < >
//!         value/mask: 0x… or decimal
//!   - bare word                            case-insensitive substring over the summary AND detail
//!
//! The flagship query is `probe:item_load_state_primary rax&0xffffffff==2` - a field predicate
//! matches ONLY probe events, so the question means exactly what it says.
//!
//! ⚠ Three silent false negatives this module is now built to refuse:
//!   - **F7** a bare word searched the summary ONLY, so `world_tag` (681 probe records, the design
//!     doc's own flagship cross-stream example) matched nothing. It searches detail too.
//!   - **F9b** `rax=2` (single `=`) silently degraded to a bare-word substring search over text
//!     that never contains it. A field-shaped token with a single `=` is now [`Term::Invalid`].
//!   - **F9c** `rax&0xffffffff==0xdead00000002` is unsatisfiable by construction (the value has
//!     bits the mask clears). It parses, but [`Query::warnings`] says so out loud.

use crate::model::{Event, Stream};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Op {
    fn apply(self, lhs: u64, rhs: u64) -> bool {
        match self {
            Op::Eq => lhs == rhs,
            Op::Ne => lhs != rhs,
            Op::Lt => lhs < rhs,
            Op::Le => lhs <= rhs,
            Op::Gt => lhs > rhs,
            Op::Ge => lhs >= rhs,
        }
    }
}

#[derive(Debug, Clone)]
enum Term {
    Stream(Stream),
    Probe(String),
    Field {
        field: String,
        mask: Option<u64>,
        op: Op,
        value: u64,
    },
    Word(String),
    /// A token we could not parse - kept, with the reason, so the UI reports it instead of
    /// silently ignoring it (or, worse, silently reinterpreting it as a substring search).
    Invalid { token: String, reason: String },
}

/// A compiled query. Build with [`Query::parse`]; test with [`Query::matches`].
#[derive(Debug, Clone, Default)]
pub struct Query {
    terms: Vec<Term>,
}

impl Query {
    pub fn parse(input: &str) -> Query {
        let terms = input.split_whitespace().map(parse_term).collect();
        Query { terms }
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// Any tokens that failed to parse - surfaced in the UI so a typo does not silently match all.
    pub fn errors(&self) -> Vec<String> {
        self.terms
            .iter()
            .filter_map(|t| match t {
                Term::Invalid { token, reason } => Some(format!("{token} ({reason})")),
                _ => None,
            })
            .collect()
    }

    /// Terms that parse but can never match anything, with the reason. A query that is
    /// unsatisfiable *by construction* must not be reported as "0 match(es)" and nothing else
    /// (F9c) - that reads as a fact about the run when it is a fact about the query.
    pub fn warnings(&self) -> Vec<String> {
        self.terms
            .iter()
            .filter_map(|t| match t {
                Term::Field {
                    field,
                    mask: Some(m),
                    value,
                    ..
                } if value & !m != 0 => Some(format!(
                    "{field}&{m:#x}=={value:#x}: value has bits outside the mask (extra bits \
                     {:#x}) - UNSATISFIABLE by construction, it can never match",
                    value & !m
                )),
                _ => None,
            })
            .collect()
    }

    /// The `probe:<substr>` terms, so a caller holding the run's real probe names can tell the
    /// operator that a typo - not the run - is why there are no matches (F9a).
    pub fn probe_terms(&self) -> Vec<String> {
        self.terms
            .iter()
            .filter_map(|t| match t {
                Term::Probe(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn matches(&self, event: &Event) -> bool {
        self.terms.iter().all(|term| match term {
            Term::Stream(s) => event.stream == *s,
            Term::Probe(sub) => {
                event.stream == Stream::Probe
                    && event
                        .probe
                        .as_ref()
                        .map(|p| p.probe.contains(sub.as_str()))
                        .unwrap_or(false)
            }
            Term::Field {
                field,
                mask,
                op,
                value,
            } => match event.field(field) {
                // A field predicate is inherently a probe question: events without the field fail.
                Some(mut v) => {
                    if let Some(m) = mask {
                        v &= *m;
                    }
                    op.apply(v, *value)
                }
                None => false,
            },
            // F7: summary AND detail. `world_tag`, `Bearer`, `XETROV`, `manifest_url`, `key-only`
            // all live only in detail; searching the summary alone reported 0 for every one of them.
            Term::Word(w) => event.contains_text(w.as_str()),
            // An unparseable token matches nothing, so a bad query returns an empty (visibly wrong)
            // result rather than accidentally matching everything.
            Term::Invalid { .. } => false,
        })
    }
}

fn parse_int(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else {
        s.parse::<u64>().ok()
    }
}

fn invalid(token: &str, reason: &str) -> Term {
    Term::Invalid {
        token: token.to_string(),
        reason: reason.to_string(),
    }
}

fn parse_term(token: &str) -> Term {
    if let Some(rest) = token.strip_prefix("stream:") {
        return match Stream::parse(rest) {
            Some(s) => Term::Stream(s),
            None => invalid(token, "not a stream; use http|bap|probe|guest"),
        };
    }
    if let Some(rest) = token.strip_prefix("probe:") {
        return Term::Probe(rest.to_string());
    }

    // Field predicate: <field>[&<mask>]<op><value>. Find the operator.
    for (op_str, op) in [
        ("==", Op::Eq),
        ("!=", Op::Ne),
        ("<=", Op::Le),
        (">=", Op::Ge),
        ("<", Op::Lt),
        (">", Op::Gt),
    ] {
        if let Some(pos) = token.find(op_str) {
            let (lhs, rhs) = (&token[..pos], &token[pos + op_str.len()..]);
            let (field, mask) = match lhs.split_once('&') {
                Some((f, m)) => (f, parse_int(m)),
                None => (lhs, None),
            };
            // Reject a malformed mask (`rax&zzz==2`) rather than silently dropping it.
            if lhs.contains('&') && mask.is_none() {
                return invalid(token, "mask is not an integer");
            }
            let field = field.trim().to_string();
            if field.is_empty() {
                return invalid(token, "no field name before the operator");
            }
            if !is_field_name(&field) {
                return invalid(token, "not a known field (rax..r15, rip, rflags, guest_vaddr, seq, probe_id)");
            }
            return match parse_int(rhs) {
                Some(value) => Term::Field {
                    field,
                    mask,
                    op,
                    value,
                },
                None => invalid(token, "value is not an integer"),
            };
        }
    }

    // F9b: `rax=2` has no operator, so it used to fall through to a bare-word substring search and
    // report "0 match(es)" with no complaint. A field-shaped token with a single `=` is a typo, and
    // a typo must be loud.
    if let Some((lhs, _rhs)) = token.split_once('=') {
        let base = lhs.split('&').next().unwrap_or(lhs).trim();
        if is_field_name(base) {
            return invalid(token, "single '='; field predicates use '==' (did you mean '=='?)");
        }
    }

    Term::Word(token.to_ascii_lowercase())
}

fn is_field_name(name: &str) -> bool {
    matches!(
        name,
        "rax" | "rbx"
            | "rcx"
            | "rdx"
            | "rsi"
            | "rdi"
            | "rbp"
            | "rsp"
            | "r12"
            | "r13"
            | "r14"
            | "r15"
            | "rip"
            | "rflags"
            | "guest_vaddr"
            | "seq"
            | "sequence"
            | "probe_id"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Confidence, Event, ProbeRecord, Stream};
    use std::collections::BTreeMap;

    fn probe_event(name: &str, rax: u64, seq: u64) -> Event {
        let mut registers = BTreeMap::new();
        registers.insert("rax".to_string(), rax);
        Event {
            id: 0,
            stream: Stream::Probe,
            order: seq as f64,
            confidence: Confidence::Exact,
            ordinal: seq,
            summary: format!("{name} record"),
            detail: vec![],
            probe: Some(ProbeRecord {
                sequence: seq,
                probe_id: 83,
                probe: name.to_string(),
                registers,
                sample_a: String::new(),
                sample_b: String::new(),
                explain: vec![],
            }),
            bap: None,
        }
    }

    fn guest_event(text: &str) -> Event {
        Event {
            id: 0,
            stream: Stream::Guest,
            order: 0.0,
            confidence: Confidence::Approx,
            ordinal: 0,
            summary: text.to_string(),
            detail: vec![],
            probe: None,
            bap: None,
        }
    }

    #[test]
    fn flagship_masked_predicate() {
        // "did item_load_state_primary ever return 2" - rax carries load_state in its low 32 bits.
        let q = Query::parse("probe:item_load_state_primary rax&0xffffffff==2");
        assert!(q.errors().is_empty());
        // load_state 2, with junk in the high bits, still matches under the mask.
        assert!(q.matches(&probe_event("item_load_state_primary", 0xdead_0000_0000_0002, 2596)));
        // load_state 0 does not.
        assert!(!q.matches(&probe_event("item_load_state_primary", 0x0000_0000_0000_0000, 54)));
        // a different probe with rax==2 does not (probe: term excludes it).
        assert!(!q.matches(&probe_event("avail_entry", 2, 10)));
    }

    #[test]
    fn field_predicate_excludes_non_probe_streams() {
        let q = Query::parse("rax==2");
        assert!(!q.matches(&guest_event("rax==2 appears in text but this is a guest line")));
    }

    #[test]
    fn stream_and_word_terms() {
        assert!(Query::parse("stream:guest").matches(&guest_event("bootflow:orbit")));
        assert!(Query::parse("orbit").matches(&guest_event("entering bootflow:orbit")));
        assert!(!Query::parse("stream:probe").matches(&guest_event("x")));
    }

    #[test]
    fn bad_tokens_are_reported_not_ignored() {
        assert_eq!(Query::parse("stream:banana").errors().len(), 1);
        assert_eq!(Query::parse("rax==notanumber").errors().len(), 1);
        assert_eq!(Query::parse("notafield==2").errors().len(), 1);
        // A query that is only an invalid token must match nothing, not everything.
        assert!(!Query::parse("rax==zz").matches(&probe_event("p", 2, 1)));
    }

    #[test]
    fn comparison_ops() {
        assert!(Query::parse("seq>=2596").matches(&probe_event("p", 0, 2596)));
        assert!(!Query::parse("seq>2596").matches(&probe_event("p", 0, 2596)));
        assert!(Query::parse("probe_id==83").matches(&probe_event("p", 0, 1)));
    }

    /// F7 regression: a bare word must reach the DETAIL lines. This is exactly the measured miss -
    /// `world_tag` lives in the probe's `explain` (681 records) and never in the summary, so the
    /// design doc's own flagship cross-stream example returned 0 matches.
    #[test]
    fn word_term_searches_detail_not_just_summary() {
        let mut ev = probe_event("item_tag_resolved", 0, 53);
        ev.summary = "#53 item_tag_resolved    guest=0x00555775 rip=0x0000000800555775".into();
        ev.detail = vec![
            ev.summary.clone(),
            "     world_tag=0x80a6035a resolved".into(),
        ];
        assert!(Query::parse("world_tag").matches(&ev), "F7: detail must be searched");
        assert!(Query::parse("0x80a6035a").matches(&ev));

        // Negative control: the search must not have become "match everything".
        assert!(!Query::parse("zzz_not_in_this_event").matches(&ev));

        // And the same on a non-probe event, where detail is the raw responder line.
        let mut http = guest_event("HTTP GET srv.shadps4.net/v1/users/D1Guardian/friendList");
        http.detail = vec!["authorization: Bearer d1-local-token".into()];
        assert!(Query::parse("bearer").matches(&http));
        assert!(!Query::parse("notpresent").matches(&http));
    }

    /// F9b regression: a single `=` on a known field is a typo, not a substring search.
    #[test]
    fn single_equals_on_a_field_is_invalid_not_a_word_search() {
        let q = Query::parse("rax=2");
        assert_eq!(q.errors().len(), 1, "rax=2 must be reported, got {:?}", q.errors());
        assert!(q.errors()[0].contains("'=='"));
        assert!(!q.matches(&probe_event("p", 2, 1)));
        // A masked form is caught too.
        assert_eq!(Query::parse("rax&0xff=2").errors().len(), 1);
        // …but a genuine bare word containing '=' is still a word search.
        assert!(Query::parse("world_tag=0x80a6035a").errors().is_empty());
        let mut ev = probe_event("item_tag_resolved", 0, 53);
        ev.detail = vec!["     world_tag=0x80a6035a resolved".into()];
        assert!(Query::parse("world_tag=0x80a6035a").matches(&ev));
    }

    /// F9c regression: a mask/value mismatch is unsatisfiable by construction and must say so.
    #[test]
    fn unsatisfiable_mask_is_warned() {
        let q = Query::parse("rax&0xffffffff==0xdead00000002");
        assert!(q.errors().is_empty(), "it parses fine - it is just unsatisfiable");
        assert_eq!(q.warnings().len(), 1, "got {:?}", q.warnings());
        assert!(q.warnings()[0].to_lowercase().contains("unsatisfiable"));
        // The flagship query must NOT warn.
        assert!(Query::parse("probe:item_load_state_primary rax&0xffffffff==2")
            .warnings()
            .is_empty());
    }

    /// F9a support: the caller can see which probe names a query asked for, so a typo can be
    /// checked against the run's real names instead of silently returning 0.
    #[test]
    fn probe_terms_are_reportable() {
        let q = Query::parse("probe:item_load_state_primry rax==2");
        assert_eq!(q.probe_terms(), vec!["item_load_state_primry".to_string()]);
    }
}
