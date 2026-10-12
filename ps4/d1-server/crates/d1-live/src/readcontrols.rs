// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! **Controls for the read side**, every one of them inverted.
//!
//! The instrument they protect is the one this project runs most: *"what is this value right now?"*
//! Until now its only self-check was `d1guest.py selftest`, which reproduces known results **against
//! a live client** - so it could not run without a run up, and it could not be inverted, because
//! nobody can ask a real client to be wrong on purpose.
//!
//! Each control below drives a [`crate::fixture`] child: a real process holding the **real eboot**
//! mapped at a base, plus a registry laid out from `guest-symbols.json`. The positive leg reads a
//! known answer out of it; the inverted leg perturbs one axis and requires the tool to **refuse**.
//!
//! | control | axis | inversion |
//! |---|---|---|
//! | `read-side-base-and-read` | the base is recovered and reads land where the arithmetic says | the same read one byte over must differ, and an **unmapped** address must ERROR rather than return zeros |
//! | `wrong-image-fails-the-verify` | control 2 compares a third address against the file **by direct read** | one flipped byte inside the verify window must make it FAIL, and `Session::open` must then refuse with exit 2 |
//! | `two-anchors-need-exactly-one-base` | control 1 requires the anchors to agree on exactly ONE base | a second copy of both anchors at another base must produce "need EXACTLY one" |
//! | `registry-cross-check-refuses` | an element address is derived twice and must agree | one bit of stride in the runtime entry must withdraw the address, not annotate it |
//! | `tag-resolves-through-the-back-offset` | the signed `entry - (mask & loc)` subtraction | a neighbouring tag must NOT land on the same object, and an empty slot must refuse |
//! | `descriptors-decode` | class/key/state/err/version at their offsets | free rows (state 0, key 0) must not appear, and a changed state must change the row |
//! | `read-after-death-is-an-error` | a dead client is an ERROR, never a zero | the *same* read against the *same* address succeeds while it is alive |
//!
//! ⛔ **These cannot run while shadPS4 is up, and they FAIL rather than skip.** The fixture is
//! discoverable by the same `pgrep` an instrument uses to find the client, and two discoverable
//! targets is how a measurement gets attributed to the wrong process. A skipped control that prints
//! nothing is the failure mode this project has paid for repeatedly, so the report says, out loud,
//! that the read side is **not certified in this session** - close the emulator and re-run.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

use anyhow::{anyhow, bail, Context, Result};

use crate::fixture::{Banner, Perturb};
use crate::proc::{self as live, Proc};
use crate::registry::Guest;
use crate::symbols::Symbols;

/// A fixture child, killed on drop.
pub struct Fixture {
    pub banner: Banner,
    child: Child,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Fixture {
    /// Spawn a fixture and read its banner. `helper` is this crate's own binary.
    pub fn spawn(helper: &Path, perturb: Perturb, hold: f64) -> Result<Self> {
        let name = match perturb {
            Perturb::None => "none",
            Perturb::FamilyStride => "family-stride",
            Perturb::VerifyWindow => "verify-window",
            Perturb::DuplicateAnchor => "duplicate-anchor",
        };
        let mut child = Command::new(helper)
            .args(["__mem-target", "--hold"])
            .arg(hold.to_string())
            .args(["--perturb", name])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("spawning the fixture {}", helper.display()))?;
        let stdout = child.stdout.take().expect("piped");
        let mut line = String::new();
        BufReader::new(stdout).read_line(&mut line).context("reading the fixture banner")?;
        if line.trim().is_empty() {
            let mut err = String::new();
            if let Some(e) = child.stderr.take() {
                let _ = BufReader::new(e).read_line(&mut err);
            }
            let _ = child.kill();
            bail!("the fixture printed no banner: {}", err.trim());
        }
        let banner: Banner = serde_json::from_str(line.trim())
            .with_context(|| format!("parsing the fixture banner {line:?}"))?;
        Ok(Fixture { banner, child })
    }

    pub fn proc(&self) -> Result<Proc> {
        Proc::open(self.banner.pid)
    }

    /// Kill the fixture and wait for it, so a caller can measure what a *dead* client reads like.
    pub fn kill_and_wait(&mut self) -> Result<()> {
        self.child.kill().context("killing the fixture")?;
        self.child.wait().context("reaping the fixture")?;
        Ok(())
    }
}

fn image_and_syms() -> Result<(d1_eboot::Image, Symbols)> {
    Ok((live::open_image(None)?, Symbols::open_default()?))
}

// ─────────────────────────────────────────────────────────────────────────────────────────────────

/// The base is recovered against a real image, reads land where the arithmetic says, and an
/// **unmapped** address is an error rather than a buffer of zeros.
pub fn base_and_read(helper: &Path) -> Result<String> {
    let f = Fixture::spawn(helper, Perturb::None, 30.0)?;
    let proc = f.proc()?;
    let (image, syms) = image_and_syms()?;
    let ctl = live::recover_base(&proc, &image, &syms, false);
    if !ctl.ok() {
        bail!("the base control failed on a CLEAN fixture:\n{}", ctl.report());
    }
    let base = ctl.base.ok_or_else(|| anyhow!("no base"))?;
    if base != f.banner.base {
        bail!("recovered base {base:#x} but the fixture mapped itself at {:#x}", f.banner.base);
    }

    // Positive: a file-backed window read through /proc/<pid>/mem equals the shipped bytes.
    let v = &syms.verify;
    let want = image.bytes(v.vaddr, v.len).map_err(|e| anyhow!("{e}"))?;
    let got = proc.read(base + v.vaddr, v.len)?;
    if got != want {
        bail!("a file-backed read at {:#x} did not reproduce the image", v.vaddr);
    }

    // ⛔ INVERSION 1: the same window ONE BYTE over must differ. Without this, a reader that
    // returned the file's own bytes regardless of the process would pass the line above.
    if proc.read(base + v.vaddr + 1, v.len)? == want {
        bail!("the window shifted by one byte still matched - it carries no information");
    }

    // ⛔ INVERSION 2: an address in no mapping at all must ERROR. A short read of zeros here is the
    // exact shape of "the field was zero", and that mistake has been published twice.
    let unmapped = proc.read(f.banner.unmapped, 8);
    let msg = match unmapped {
        Ok(b) => bail!(
            "a read at the unmapped {:#x} SUCCEEDED and returned {b:02x?} - a zero there is \
             indistinguishable from a field that really is zero",
            f.banner.unmapped
        ),
        Err(e) => format!("{e:#}"),
    };

    Ok(format!(
        "base {base:#x} recovered from two anchors and confirmed against the fixture's own \
         mapping; {} bytes at v:{:#x} reproduce the image; the same window +1 differs; a read at \
         the unmapped {:#x} errored ({})",
        v.len,
        v.vaddr,
        f.banner.unmapped,
        msg.lines().next().unwrap_or("").trim()
    ))
}

/// Control 2 is a **direct compare against the file**, so a memory image that has drifted from the
/// shipped bytes must fail it - even though the anchors still agree.
pub fn wrong_image_fails_the_verify(helper: &Path) -> Result<String> {
    let f = Fixture::spawn(helper, Perturb::VerifyWindow, 30.0)?;
    let proc = f.proc()?;
    let (image, syms) = image_and_syms()?;
    let ctl = live::recover_base(&proc, &image, &syms, false);
    if ctl.ok() {
        bail!(
            "one flipped byte inside the verify window did NOT fail the base control - control 2 \
             is not comparing what it claims to:\n{}",
            ctl.report()
        );
    }
    let anchors_passed = ctl.lines.iter().any(|l| l.name == "anchors" && l.pass);
    // ⚠ Not just "verify failed" - it must have failed by COMPARING and DIFFERING. Measured while
    // demonstrating that this suite can fail: with a doctored symbol file whose verify window is not
    // file-backed, control 2 fails without ever comparing, and a looser assertion here banked that
    // as a pass. A control that accepts the right verdict for the wrong reason is not a control.
    let verify_differed = ctl
        .lines
        .iter()
        .any(|l| l.name == "verify" && !l.pass && l.detail.contains("DIFFER from the file image"));
    if !anchors_passed || !verify_differed {
        bail!(
            "the control failed for the WRONG reason - the axis is 'the anchors agree but the \
             third address does not':\n{}",
            ctl.report()
        );
    }
    // …and the session gate must refuse with the control-grade code, not read anyway.
    let opts = crate::session::Opts { pid: Some(f.banner.pid), quiet: true, ..Default::default() };
    match crate::session::Session::open(&opts) {
        Ok(_) => bail!("Session::open ACCEPTED a client whose base control failed"),
        Err(e) => {
            let code = crate::session::exit_code(&e);
            if code != 2 {
                bail!("the refusal asked for exit {code}, not the control-grade 2: {e:#}");
            }
        }
    }
    Ok(format!(
        "⛔ INVERTED: one byte flipped inside the verify window; the anchors still agreed on \
         {:#x} and control 2 FAILED anyway, and Session::open refused with exit 2 rather than \
         reading. That is the whole point of a third, independent address compared by direct read.",
        f.banner.base
    ))
}

/// Two anchors must agree on **exactly one** base. A second copy at another base is not a tie to be
/// broken; it is a refusal.
pub fn exactly_one_base(helper: &Path) -> Result<String> {
    let f = Fixture::spawn(helper, Perturb::DuplicateAnchor, 30.0)?;
    let proc = f.proc()?;
    let (image, syms) = image_and_syms()?;
    let ctl = live::recover_base(&proc, &image, &syms, false);
    if ctl.ok() {
        bail!("two candidate bases were accepted:\n{}", ctl.report());
    }
    let line = ctl
        .lines
        .iter()
        .find(|l| l.name == "anchors")
        .ok_or_else(|| anyhow!("no anchors line at all:\n{}", ctl.report()))?;
    if line.pass || !line.detail.contains("need EXACTLY one") {
        bail!("the anchors control did not refuse for the right reason: {}", line.detail);
    }
    if !line.detail.contains("2 hit(s)") {
        bail!("the decoy was not actually found - the inversion did not fire: {}", line.detail);
    }
    Ok(format!(
        "⛔ INVERTED: a second copy of BOTH anchors at {:#x} made each of them match twice; the \
         control refused with \"need EXACTLY one\" rather than picking. Detail: {}",
        crate::fixture::DECOY_BASE,
        line.detail
    ))
}

/// An element address is derived twice - runtime entry table and static mapper - and one bit of
/// disagreement must **withdraw the address**, not annotate it.
pub fn registry_cross_check_refuses(helper: &Path) -> Result<String> {
    let (image, syms) = image_and_syms()?;
    let (fam, typ) = (syms.account_family, syms.account_type);

    // Positive leg: the clean fixture agrees, and the address is returned.
    let good = Fixture::spawn(helper, Perturb::None, 30.0)?;
    let gproc = good.proc()?;
    let g = Guest::new(&gproc, &image, &syms, good.banner.base);
    let ok = g.family_slot(fam, typ)?;
    let Some(addr) = ok.addr else {
        bail!("the CLEAN fixture failed its own cross-check: {:?}", ok.lines);
    };
    if !ok.lines.iter().any(|l| l.contains("PASS")) {
        bail!("the clean cross-check produced no PASS line: {:?}", ok.lines);
    }

    // Inverted leg: one bit of stride, and the address must be withheld.
    let bad = Fixture::spawn(helper, Perturb::FamilyStride, 30.0)?;
    let bproc = bad.proc()?;
    let b = Guest::new(&bproc, &image, &syms, bad.banner.base);
    let fr = b.family_slot(fam, typ)?;
    if fr.addr.is_some() {
        bail!(
            "the runtime stride {:#x} disagreed with the static table and an address was returned \
             ANYWAY: {:?}",
            fr.stride,
            fr.lines
        );
    }
    if !fr.lines.iter().any(|l| l.contains("FAIL") && l.contains("cross-check")) {
        bail!("the refusal did not say which check failed: {:?}", fr.lines);
    }
    // …and the ladder must notice too, or `obj` would be gated by a rung that cannot see this.
    let l = live::liveness(&bproc, &syms, bad.banner.base, syms.account_soid);
    if l.level >= 3 {
        bail!("the liveness ladder still reported L{} on a disagreeing registry", l.level);
    }
    Ok(format!(
        "⭐ family {fam}/{typ} resolves to {addr:#x} with both derivations agreeing. ⛔ INVERTED: \
         one bit of stride ({:#x} vs {:#x}) withdrew the address entirely and dropped the ladder to \
         L{} - it did not annotate it and carry on.",
        fr.stride,
        syms.family(fam, typ).map(|f| f.size).unwrap_or(0),
        l.level
    ))
}

/// The package registry's signed back-offset: `object = entry - (i32 mask & u64 loc)`.
pub fn tag_back_offset(helper: &Path) -> Result<String> {
    let f = Fixture::spawn(helper, Perturb::None, 30.0)?;
    let proc = f.proc()?;
    let (image, syms) = image_and_syms()?;
    let g = Guest::new(&proc, &image, &syms, f.banner.base);

    let (obj, why) = g.resolve_tag(f.banner.tag)?;
    let obj = obj.ok_or_else(|| anyhow!("the fixture's own tag did not resolve: {why}"))?;
    if obj != f.banner.package_object {
        bail!(
            "tag {:#010x} resolved to {obj:#x}, but the fixture put the object at {:#x} - the \
             signed subtraction is wrong",
            f.banner.tag,
            f.banner.package_object
        );
    }
    let bytes = proc.read(obj, 16)?;
    if !bytes.starts_with(b"D1-LIVE FIXTURE") {
        bail!("the resolved address does not hold the object: {bytes:02x?}");
    }

    // ⛔ INVERSION 1: the neighbouring tag shares a slot but not an entry, so it must NOT land here.
    // A resolver that ignored the index would pass everything above.
    let (near, _) = g.resolve_tag(f.banner.tag + 1)?;
    if near == Some(obj) {
        bail!("tag {:#010x} resolved to the same object - the index is not being used", f.banner.tag + 1);
    }
    // ⛔ INVERSION 2: an empty slot is a refusal with a reason, not an address.
    let (empty, ewhy) = g.resolve_tag(1)?;
    if empty.is_some() {
        bail!("tag 0x1 resolved to an address out of an empty slot");
    }
    if !ewhy.contains("empty") {
        bail!("an empty slot did not say so: {ewhy}");
    }
    Ok(format!(
        "tag {:#010x} -> {obj:#x} ({why}), and the bytes there are the object. ⛔ INVERTED: tag+1 \
         lands elsewhere ({:?}), and tag 0x1 refuses - \"{ewhy}\".",
        f.banner.tag,
        near.map(|a| format!("{a:#x}"))
    ))
}

/// The descriptor table decodes class/key/state/err/version, and **free rows never appear**.
pub fn descriptors_decode(helper: &Path) -> Result<String> {
    let f = Fixture::spawn(helper, Perturb::None, 30.0)?;
    let proc = f.proc()?;
    let (image, syms) = image_and_syms()?;
    let g = Guest::new(&proc, &image, &syms, f.banner.base);
    let rows = g.descriptors()?;

    if rows.len() != 2 {
        bail!(
            "expected exactly the 2 claimed rows the fixture wrote, got {} - a decoder that \
             mis-sized the stride would return hundreds",
            rows.len()
        );
    }
    let accepts = rows.iter().find(|d| d.index == 5).ok_or_else(|| anyhow!("row 5 missing"))?;
    let refuses = rows.iter().find(|d| d.index == 9).ok_or_else(|| anyhow!("row 9 missing"))?;
    if accepts.class != 3 || accepts.state != 3 || accepts.err != 0 || accepts.version != 7 {
        bail!("row 5 decoded wrong: {accepts:?}");
    }
    if refuses.class != 1 || refuses.state != 1 || refuses.err != 7 {
        bail!("row 9 decoded wrong: {refuses:?}");
    }
    // The distinction the table exists to show: one state ACCEPTS updates and one REFUSES them.
    if !accepts.line().contains("update accepted") || !refuses.line().contains("update REFUSED") {
        bail!("the state names lost the accepts/refuses distinction:\n{}\n{}", accepts.line(), refuses.line());
    }
    if !refuses.line().contains("TIMEOUT") {
        bail!("err 7 did not render as TIMEOUT: {}", refuses.line());
    }
    // ⛔ INVERSION: 255 of the 257 slots are free, and a decoder that emitted them all would still
    // have satisfied every field assertion above for rows 5 and 9.
    let count = syms.layout.get("descriptor_count").copied().unwrap_or(257);
    if rows.iter().any(|d| d.state == 0 && d.key == 0) {
        bail!("a free row was emitted - the state/key filter is not applied");
    }
    Ok(format!(
        "2 of {count} descriptor slots are claimed and both decode exactly:\n{}\n{}\n⛔ INVERTED: \
         the other {} free rows were filtered, so a row in this output is a claim the client made.",
        accepts.line(),
        refuses.line(),
        count - 2
    ))
}

/// ⚠ **A read after death is an ERROR, not a zero** - the refusal two wrong readings were published
/// for.
pub fn read_after_death(helper: &Path) -> Result<String> {
    let mut f = Fixture::spawn(helper, Perturb::None, 30.0)?;
    let proc = f.proc()?;
    // ⭐ Deliberately an address with NON-ZERO contents. The first draft used the registry base,
    // which happens to hold zeros, so the "while it was alive" half of the pairing read the same
    // bytes a broken dead-read would have invented - a positive control that could not tell the two
    // apart. The package object carries recognisable text.
    let addr = f.banner.package_object;
    let alive = proc.read(addr, 8)?;
    if alive.iter().all(|&b| b == 0) {
        bail!("the live half read zeros, so it cannot distinguish a dead read from a zero field");
    }

    f.kill_and_wait()?;
    // The pid is reaped, so `alive()` fails on the start-time comparison rather than on an errno.
    let after = proc.read(addr, 8);
    let e = match after {
        Ok(b) => bail!(
            "a read after the client died SUCCEEDED and returned {b:02x?}. That value is \
             indistinguishable from 'our delivery never worked' - the mistake this refusal exists \
             to stop."
        ),
        Err(e) => e,
    };
    if !live::is_dead(&e) {
        bail!("the failure was not reported as a DEATH, so the caller cannot exit 3: {e:#}");
    }
    if crate::session::exit_code(&e) != 3 {
        bail!("a dead-client error did not ask for exit 3: {e:#}");
    }
    Ok(format!(
        "the same address {addr:#x} read {} while the fixture lived, and after it was killed the \
         read ERRORED and asked for exit 3 - \"{}\". ⭐ The pairing is the control: a failure with \
         no matching success is a failure that could be a bad address.",
        alive.iter().map(|b| format!("{b:02x}")).collect::<String>(),
        e.to_string().lines().next().unwrap_or("")
    ))
}

/// Write `doc` to a scratch symbol file and open it, so a control can hand the reader a symbol file
/// that is wrong in exactly one way.
/// ⚠ The file is removed as soon as it is parsed - `Symbols::open` reads it whole, so nothing later
/// depends on the path, and a control must not leave scratch state behind for the next one to find.
fn syms_with(name: &str, doc: serde_json::Value) -> Result<Symbols> {
    let p = std::env::temp_dir()
        .join(format!("d1-live-control-{}-{name}.json", std::process::id()));
    std::fs::write(&p, serde_json::to_vec(&doc)?)
        .with_context(|| format!("writing the doctored symbol file {}", p.display()))?;
    let s = Symbols::open(&p);
    let _ = std::fs::remove_file(&p);
    s
}

/// **Garbage in must be refused, never answered.** Three inversions, each paired with the positive
/// read it perturbs, because a refusal with no matching success is a refusal that could be a broken
/// fixture.
///
/// Every one of the three was a real defect found by running this port against inputs the golden
/// diff does not generate:
///
/// 1. **A wrapped family index.** `0x60 * 2^59 == 0` mod 2^64, so `family = 4 + 2^59` aliases
///    exactly onto family 4/0's entry. Before the fix, `obj 576460752303423492:0` printed the
///    ACCOUNT object's bytes at **exit 0**, headed "no static entry … cross-check SKIPPED".
///    `d1guest.py` refuses the same input - Python integers do not wrap.
/// 2. **A missing `registry_layout` key.** The reader defaulted it to a compiled-in constant and
///    answered; the Python raises `KeyError` and refuses. A hardcoded fallback is precisely the
///    "place where a superseded constant survives a retraction" that `d1guest.py` exists to abolish.
/// 3. **A `package_registry_layout` offset past the slot record.** That was a slice panic, exit 101
///    with a backtrace - loud, but a crash is not a refusal.
pub fn garbage_in_refuses(helper: &Path) -> Result<String> {
    let f = Fixture::spawn(helper, Perturb::None, 30.0)?;
    let proc = f.proc()?;
    let (image, syms) = image_and_syms()?;
    let base = f.banner.base;
    let mut notes = Vec::new();

    // ── 1. the aliasing family index ────────────────────────────────────────────────────────────
    let g = Guest::new(&proc, &image, &syms, base);
    let sf = crate::registry::layout_req(&syms, "entry_stride_family")?;
    let good = g.family_slot(syms.account_family, syms.account_type)?;
    let good_addr = good
        .addr
        .ok_or_else(|| anyhow!("the CLEAN fixture failed its own cross-check: {:?}", good.lines))?;
    // The smallest positive `k` with `k * sf == 0` (mod 2^64) is `2^(64 - v)` where `v` is the
    // number of trailing zero bits in the stride: `sf = 0x60 = 2^5 * 3`, so `v = 5` and `k = 2^59`.
    let v = sf.trailing_zeros();
    if sf == 0 || v == 0 {
        bail!("stride {sf:#x} has no power-of-two factor, so no family index aliases onto another");
    }
    let shift = 64 - v;
    let alias = syms.account_family.wrapping_add(1u64 << shift);
    if alias.wrapping_mul(sf) != syms.account_family.wrapping_mul(sf) {
        bail!(
            "the control could not construct an aliasing family for stride {sf:#x} - it did not \
             wrap onto family {}, so the inversion would prove nothing",
            syms.account_family
        );
    }
    match g.family_slot(alias, syms.account_type) {
        Ok(fr) => bail!(
            "family {alias} wraps onto family {} and was ANSWERED rather than refused: addr={:?}. \
             That prints one object's bytes under another object's name, at exit 0.",
            syms.account_family,
            fr.addr.map(|a| format!("{a:#x}"))
        ),
        Err(e) => {
            if !e.to_string().contains("overflow") {
                bail!("family {alias} was refused, but not as an overflow: {e:#}");
            }
            notes.push(format!(
                "⛔ INVERTED: family {alias} (= {} + 2^{shift}) wraps onto family {}'s entry at \
                 {good_addr:#x} and is REFUSED (\"{}\"), while family {} itself still resolves to \
                 {good_addr:#x}",
                syms.account_family,
                syms.account_family,
                e.to_string().lines().next().unwrap_or(""),
                syms.account_family,
            ));
        }
    }

    // ── 2. a deleted registry_layout key ────────────────────────────────────────────────────────
    let mut doc = syms.doc.clone();
    doc["registry_layout"]
        .as_object_mut()
        .ok_or_else(|| anyhow!("registry_layout is not an object"))?
        .remove("entry_stride_family");
    let bad = syms_with("no-stride-family", doc)?;
    let gb = Guest::new(&proc, &image, &bad, base);
    match gb.family_slot(bad.account_family, bad.account_type) {
        Ok(fr) => bail!(
            "a symbol file with entry_stride_family DELETED still produced an address ({:?}). The \
             reader is answering from a compiled-in constant, which is the sixth copy of the very \
             thing d1guest.py collapsed five scripts to remove.",
            fr.addr.map(|a| format!("{a:#x}"))
        ),
        Err(e) => notes.push(format!(
            "⛔ INVERTED: deleting registry_layout.entry_stride_family REFUSES (\"{}\") instead of \
             defaulting to {sf:#x} and answering",
            e.to_string().lines().next().unwrap_or("")
        )),
    }

    // ── 3. a package offset past the end of the slot record ─────────────────────────────────────
    let (obj, why) = g.resolve_tag(f.banner.tag)?;
    let obj = obj.ok_or_else(|| anyhow!("the fixture's own tag did not resolve: {why}"))?;
    let mut doc = syms.doc.clone();
    doc["package_registry_layout"]
        .as_object_mut()
        .ok_or_else(|| anyhow!("package_registry_layout is not an object"))?
        .insert("slot_data_base".into(), serde_json::json!(0x3C));
    let bad = syms_with("slot-data-base-oob", doc)?;
    let gb = Guest::new(&proc, &image, &bad, base);
    match gb.resolve_tag(f.banner.tag) {
        Ok((a, w)) => bail!(
            "slot_data_base = 0x3C reads an 8-byte field out of a 0x40-byte record and was \
             ANSWERED: {a:?} / {w}"
        ),
        Err(e) => notes.push(format!(
            "⛔ INVERTED: package_registry_layout.slot_data_base = 0x3C REFUSES (\"{}\") rather \
             than panicking on the slice, while the true offset still resolves tag {:#010x} to \
             {obj:#x}",
            e.to_string().lines().next().unwrap_or(""),
            f.banner.tag
        )),
    }

    Ok(format!(
        "⭐ POSITIVE, in the same window: family {}/{} resolves to {good_addr:#x} and tag {:#010x} \
         resolves to {obj:#x}. {}",
        syms.account_family,
        syms.account_type,
        f.banner.tag,
        notes.join(" · ")
    ))
}

/// Is there a shadPS4 running that would make the fixture unstartable? Returns its reason.
pub fn blocked_by_a_live_client() -> Option<String> {
    let pids = live::emulator_pids().unwrap_or_default();
    if pids.is_empty() {
        return None;
    }
    Some(format!(
        "shadPS4 is running (pid(s) {pids:?}) and the read-side fixture refuses to start beside it \
         - it is discoverable by the same `pgrep` an instrument uses to find the client. ⛔ The \
         READ SIDE IS THEREFORE NOT CERTIFIED IN THIS SESSION. Close the emulator and re-run \
         `d1-live controls`. (This is a FAIL and not a skip on purpose: a control that quietly \
         reports nothing is the failure mode this project keeps paying for.)"
    ))
}
