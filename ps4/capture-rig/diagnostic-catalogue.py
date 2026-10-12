#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""diagnostic-catalogue.py - the client's own diagnostic strings, as a lookup.

Regenerates ``design/diagnostic-catalogue.md`` plus the full-row sidecar
``capture-rig/diagnostic-catalogue.tsv`` from two inputs and nothing else:

  * ``dumps/patch-01.29-CUSA00219/eboot.bin``  (pinned build, sha256 checked)
  * ``runs/*/emulator.log``                    (the SEEN column)

WHY THIS EXISTS
---------------
Three sessions theorised for hours about a "missing fireteam identity" behind a crash in a
4-slot table lookup.  The function that fills the table references a string literal reading
``voice_globals`` - it is the *voice chat* subsystem, which shadPS4 stubs entirely.  One string
reference, never looked up, because there was nowhere to look it up.  Twice in the same night a
theory was built on a diagnostic string that had **never actually fired**, because there was no
way to ask "have we ever seen this?" short of hand-grepping every run.

DESIGN NOTE - the scan is REFERENCE-FIRST, and that is deliberate (ATLAS §4.2b)
------------------------------------------------------------------------------
A scan for the format string at ``0x162E2C0`` once returned 0 references with a *passing*
control, and the conclusion drawn was wrong: the literal starts at ``0x162E2B5``.  Nothing
references a string interior.  A string-first catalogue re-creates that failure mode at scale,
because a C string pool has no framing - ``voice_globals`` at ``0x167D3E0`` is preceded by the
float bytes ``9a 99 89 40``, so a greedy "printable run" scan calls it ``@voice_globals`` and a
NUL-preceded scan misses it entirely.  Both readings are already in this project's documents
(``design/eboot-map.md`` §7's ``@waiting_for_login`` is the same artefact).

So we enumerate *references* and read the string at whatever address the code actually points
at.  Every start address in this catalogue is therefore correct by construction.  Strings with
no direct reference are still listed, flagged ``ref=none`` - they are reached by a table or a
form this scan cannot see, and that flag is the honest statement of it.

⚠ TWO-GATE TRAP (``status/LESSONS.md``, ``historicaldocs/design/guest-diagnostics.md``)
-----------------------------------------------------------------------
A string absent from the logs does **not** prove its code never ran.  There are two independent
gates and only the second is patched unconditionally in our runs.  Every row therefore carries
the site's own ``level``, and every run's applied threshold ``T`` is measured from its log - see
``--report`` output and the doc's SEEN section.

⭐⭐ THE SEEN COLUMN DECAYS.  ASK THE GUARD BEFORE CITING THE FILE.
------------------------------------------------------------------
``runs/`` grows; the checked-in TSV does not.  A stale ``seen=0`` is indistinguishable from a real
NOT-SEEN, so the file quietly manufactures exactly the false negative this tool exists to prevent.
Measured on 2026-08-10: the TSV on disk had been built from **146** run logs and the tree held
**248** - **148 rows** flipped ``seen`` 0 → 1 on regeneration, **every one of them** evidence class
``strong``, i.e. every one a *citable* false negative.

    python3 -B capture-rig/diagnostic-catalogue.py --check-stale

  exit 0  current · exit 1  STALE (names the newer runs) · exit 2  CANNOT TELL (no provenance line)

⚠ "cannot tell" is deliberately the WORST exit code.  Line 1 of the TSV is a provenance stamp
(run count, a sha256 over every log's name/size/mtime, the newest run seen, the eboot sha256,
the generation time); a file without one predates this guard and cannot be dated at all.
⭐ The ``evidence`` column is a STATIC property of the code and stays valid however stale the file
is.  It is ``seen`` / ``n_runs`` / ``last_run`` that must come from a fresh file or the live CLI.

Positive controls (ATLAS §4.2, §4.4d) are MANDATORY and printed on every run.  Exit status 2
means a control failed and the output must not be cited.
"""

from __future__ import annotations

import argparse
import bisect
import collections
import hashlib
import json
import os
import re
import struct
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

try:
    from capstone import CS_ARCH_X86, CS_MODE_64, Cs
except ImportError:  # pragma: no cover
    sys.exit("capstone is required:  pip install capstone")

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1build  # noqa: E402  (capture-rig/builds.json; D1_BUILD selects)

REPO = Path(__file__).resolve().parent.parent
BUILD = d1build.selected()
EBOOT = d1build.path(BUILD, "eboot")
# The census controls below were measured on the 01.29 reference, whose ELF hash this is. On any
# other build they are expected to fail until re-derived, and outputs go to a per-build directory
# so the reference catalogue in design/ is never overwritten by another build's scan.
EBOOT_SHA256 = "7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954"
RUNS = REPO / "runs"
if BUILD["is_reference"]:
    OUT_MD = REPO / "design" / "diagnostic-catalogue.md"
    OUT_TSV = REPO / "capture-rig" / "diagnostic-catalogue.tsv"
else:
    OUT_MD = REPO / "capture-rig" / "derived" / BUILD["id"] / "diagnostic-catalogue.md"
    OUT_TSV = REPO / "capture-rig" / "derived" / BUILD["id"] / "diagnostic-catalogue.tsv"

PROLOGUE = b"\x55\x48\x89\xe5"

# --- gate 1, the filter ------------------------------------------------------------------
# retail_log_check(system, level) -> threshold[system] <= level.  A HIGHER threshold is
# QUIETER; a site emits only when its own level >= the threshold in force.
RETAIL_LOG_CHECK = 0xE4DA0
RETAIL_LOG_REGISTER = 0xE4A40  # lazily registers the format string, returns a site id
RETAIL_LOG_EMIT = 0xE4AD0  # gate 2, the sink: memcpy into a ring nobody drains

# The four ranges we are actively stuck in (task brief).  Any string referenced from these is
# catalogued whether or not it carries a subsystem prefix - `voice_globals` is exactly that case.
HOT_RANGES = [
    (0xB41000, 0xB48000, "voice/session"),
    (0xB2E000, 0xB33000, "bap"),
    (0x570000, 0x590000, "investment/registry"),
    (0x790000, 0x7A1000, "world_controller states"),
]

# `networking:online:matchmaking:bungie: Resetting ...` -> subsystem is everything before the
# final `: `.  Components are lowercase identifiers; anything else is not a subsystem prefix.
#
# ⛔ FIXED 2026-08-10 - the component class was `[a-z0-9_]`, which **excludes the hyphen**, and
# Bungie hyphenates.  `networking:session:process-pending-joins:` therefore matched NOTHING and
# every string in twelve subsystems was absent from the catalogue entirely - not flagged, not
# `ref=none`, absent.  Found by the coverage control ("every observed guest line is explained by
# a catalogued string"), which failed with **495 unexplained guest lines collapsing to just 3
# formats** once the run corpus grew to include a session that processes pending joins.
#   ⭐ That is the control earning its keep: the hole had been there since the tool was written
#   and only a *widened input set* exposed it.  The three strings were referenced by two
#   `lea`s each from `0xb6c740` with all four `d1-eboot xref` positive controls passing - i.e.
#   the reference scan saw them and this regex threw them away.
# Measured delta, exhaustively over the whole string pool: **+12 subsystems, +28 strings, 0 lost.**
# Every gained name is plainly a real subsystem (`networking:session:join-abort`,
# `networking:simulation:players:apply-added`, …), so this widening does not manufacture the
# fake-subsystem artefact tier A2 warns about.
SUBSYS_RE = re.compile(r"^([a-z_][a-z0-9_-]*(?::[a-z0-9_-]+)*):(?:\s|$)")
# printf conversions, including the length modifiers this build actually uses (%lld, %016llX).
SPEC_RE = re.compile(r"%[-+ #0']*[0-9*]*(?:\.[0-9*]+)?(?:hh|h|ll|l|L|z|j|t|I64)?[diouxXeEfFgGaAcspn%]")
# Itanium type-name shapes: `26c_voice_controller_monitor`, `N3cui30c_..._componentE`.
RTTI_RE = re.compile(r"^(?:N(?:\d+[A-Za-z_][A-Za-z0-9_]*)+E|\d+[A-Za-z_][A-Za-z0-9_]*)$")
# A snake_case identifier of at least two words - the shape of an allocation / registry tag
# (`voice_globals`, `retail_log`, `investment_roster`). Requiring the underscore is what keeps the
# display words this heuristic otherwise swallows (`true`, `remote`, `silent`, `secure`) out.
TAG_RE = re.compile(r"^[a-z][a-z0-9]*(?:_[a-z0-9]+)+$")


# ==========================================================================================
# image
# ==========================================================================================
@dataclass
class Segment:
    p_type: int
    flags: int
    offset: int
    vaddr: int
    filesz: int
    memsz: int

    @property
    def bias(self) -> int:
        return self.offset - self.vaddr

    @property
    def is_exec(self) -> bool:
        return self.p_type == 1 and bool(self.flags & 1)

    @property
    def is_write(self) -> bool:
        return self.p_type == 1 and bool(self.flags & 2)


class Image:
    """⚠ ATLAS §4.1: the bias is PER SEGMENT.  0x4000 is the TEXT case, not a constant."""

    def __init__(self, path: Path):
        self.path = path
        self.data = path.read_bytes()
        self.sha256 = hashlib.sha256(self.data).hexdigest()
        if self.data[:4] != b"\x7fELF":
            raise SystemExit(f"{path}: not an ELF")
        e_phoff, = struct.unpack_from("<Q", self.data, 0x20)
        e_phentsize, e_phnum = struct.unpack_from("<HH", self.data, 0x36)
        self.segments = []
        for i in range(e_phnum):
            o = e_phoff + i * e_phentsize
            p_type, p_flags = struct.unpack_from("<II", self.data, o)
            p_offset, p_vaddr, _pa, p_filesz, p_memsz = struct.unpack_from("<QQQQQ", self.data, o + 8)
            self.segments.append(Segment(p_type, p_flags, p_offset, p_vaddr, p_filesz, p_memsz))
        self.text = next(s for s in self.segments if s.is_exec)
        self.rw = next(s for s in self.segments if s.is_write)
        self.blob = self.data[self.text.offset:self.text.offset + self.text.filesz]
        self.text_lo = self.text.vaddr
        self.text_hi = self.text.vaddr + self.text.filesz
        self.bss_lo = self.rw.vaddr + self.rw.filesz
        self.bss_hi = self.rw.vaddr + self.rw.memsz

    def in_text(self, va: int) -> bool:
        return self.text_lo <= va < self.text_hi

    def in_data(self, va: int) -> bool:
        return self.rw.vaddr <= va < self.bss_hi

    def cstr(self, va: int, limit: int = 512) -> bytes | None:
        """The NUL-terminated printable string at exactly `va`, or None.

        ⚠ `\\n` / `\\t` / `\\r` are ADMITTED. `networking:sessionid: '%s'\\n` is a real format
        string that a printable-only filter drops silently - and it is one of only two lines
        this project saw before the sink patch existed, so dropping it would have left the two
        best-attested strings in the corpus out of the catalogue.
        """
        if not self.in_text(va):
            return None
        i = va - self.text_lo
        end = self.blob.find(b"\x00", i, i + limit)
        if end < 0:
            return None
        s = self.blob[i:end]
        if not s or not all(0x20 <= c < 0x7F or c in (9, 10, 13) for c in s):
            return None
        return s


# ==========================================================================================
# decode
# ==========================================================================================
RIP_RE = re.compile(r"\[rip ([+-]) (0x[0-9a-f]+)\]")
MOVI_RE = re.compile(r"^e(di|si), (0x[0-9a-f]+|\d+)$")
XOR_RE = re.compile(r"^e(di|si), e(di|si)$")


@dataclass
class Ref:
    site: int
    fn: int
    mnemonic: str
    sys: int | None = None     # retail_log system id gating this site
    level: int | None = None   # this site's own message level
    emits: bool = False        # a `call retail_log_emit` follows this lea before the next one
    gated: bool = False        # a `call retail_log_check` precedes it in the same emit group


@dataclass
class Row:
    vaddr: int
    text: str
    subsystem: str | None
    refs: list[Ref] = field(default_factory=list)
    specs: list[str] = field(default_factory=list)
    seen: bool = False
    runs: list[str] = field(default_factory=list)
    tier: str = ""

    @property
    def arity(self) -> int:
        return sum(1 for s in self.specs if s != "%%")

    @property
    def fn(self) -> int | None:
        return self.refs[0].fn if self.refs else None

    @property
    def level(self) -> int | None:
        for r in self.refs:
            if r.level is not None:
                return r.level
        return None

    @property
    def sys(self) -> int | None:
        for r in self.refs:
            if r.sys is not None:
                return r.sys
        return None

    @property
    def emits(self) -> bool:
        return any(r.emits for r in self.refs)

    @property
    def unfiltered(self) -> bool:
        """Reaches the sink with NO `retail_log_check` in front of it - so with our sink patch
        applied it prints unconditionally, whatever threshold the run used."""
        return any(r.emits and not r.gated for r in self.refs)

    def seen_evidence(self, min_T: int) -> str:
        """How much a NOT-SEEN verdict is worth for this row. See the doc's SEEN section."""
        if self.seen:
            return "seen"
        if self.unfiltered:
            return "strong"          # unfiltered emit: absence == the code did not run
        lv = self.level
        if self.emits and lv is not None:
            return "weak-blind" if lv < min_T else "strong"
        return "none"                # not a retail_log site at all - absence proves nothing


class Scan:
    def __init__(self, img: Image, verbose: bool = True):
        self.img = img
        self.verbose = verbose
        self.md = Cs(CS_ARCH_X86, CS_MODE_64)
        self.md.detail = False
        blob = img.blob
        self.prologues = [m.start() + img.text_lo for m in re.finditer(re.escape(PROLOGUE), blob)]
        # Byte-pattern candidate set: REX.W(48/4c/49/4d) 8d modrm(mod=00 rm=101).  Used ONLY as
        # a cross-check on the decoder - a decoded `lea` that is not in here means the decode
        # desynced on a data island (ATLAS §4.5), and the count is printed as a health metric.
        self.lea_candidates: set[int] = set()
        for m in re.finditer(rb"[\x48\x4c\x49\x4d]\x8d[\x05\x0d\x15\x1d\x25\x2d\x35\x3d]", blob, re.S):
            self.lea_candidates.add(m.start() + img.text_lo)
        # filled by run()
        self.refs_by_target: dict[int, list[Ref]] = collections.defaultdict(list)
        # fn -> [(instruction site, data/BSS vaddr)], in program order
        self.globals_by_fn: dict[int, list[tuple[int, int]]] = collections.defaultdict(list)
        self.gate_sites: list[tuple[int, int | None, int | None]] = []
        self.emit_sites: list[int] = []
        self.insn_count = 0
        self.unconfirmed_lea = 0

    def function_bounds(self, i: int) -> tuple[int, int]:
        f = self.prologues[i]
        nxt = self.prologues[i + 1] if i + 1 < len(self.prologues) else self.img.text_hi
        return f, min(nxt, f + 0x8000)

    def enclosing(self, va: int) -> int | None:
        k = bisect.bisect_right(self.prologues, va) - 1
        return self.prologues[k] if k >= 0 else None

    def run(self) -> None:
        img = self.img
        blob = img.blob
        t0 = time.time()
        for i in range(len(self.prologues)):
            f, end = self.function_bounds(i)
            edi = esi = None
            cur_sys = cur_level = None
            pending: list[Ref] = []   # string leas since the last `call retail_log_emit`
            code = blob[f - img.text_lo:end - img.text_lo]
            for (a, sz, mn, op) in self.md.disasm_lite(code, f):
                self.insn_count += 1
                if mn == "mov":
                    m = MOVI_RE.match(op)
                    if m:
                        v = int(m.group(2), 0)
                        if m.group(1) == "di":
                            edi = v
                        else:
                            esi = v
                elif mn == "xor":
                    m = XOR_RE.match(op)
                    if m and m.group(1) == m.group(2):
                        if m.group(1) == "di":
                            edi = 0
                        else:
                            esi = 0
                elif mn == "call" and op.startswith("0x"):
                    t = int(op, 16)
                    if t == RETAIL_LOG_CHECK:
                        self.gate_sites.append((a, edi, esi))
                        # A valid gate call is (system 0..21, level 0..5); anything else is a
                        # stale register, i.e. one of the 18 sites guest-diagnostics.md marks [U].
                        if edi is not None and esi is not None and 0 <= edi <= 21 and 0 <= esi <= 5:
                            cur_sys, cur_level = edi, esi
                        else:
                            cur_sys = cur_level = None
                        edi = esi = None
                    elif t == RETAIL_LOG_EMIT:
                        # Close the emit group: every format string `lea`d since the previous
                        # emit is what this call prints, and `cur_*` is the filter it passed.
                        self.emit_sites.append(a)
                        for r in pending:
                            r.emits = True
                            r.gated = cur_sys is not None
                            r.sys, r.level = cur_sys, cur_level
                        pending.clear()
                        cur_sys = cur_level = None
                    continue
                m = RIP_RE.search(op)
                if not m:
                    continue
                d = int(m.group(2), 16)
                tgt = a + sz + (d if m.group(1) == "+" else -d)
                if mn == "lea" and a not in self.lea_candidates:
                    self.unconfirmed_lea += 1
                    continue
                if img.in_data(tgt):
                    self.globals_by_fn[f].append((a, tgt))
                elif img.in_text(tgt) and mn == "lea":
                    ref = Ref(a, f, mn)
                    self.refs_by_target[tgt].append(ref)
                    pending.append(ref)
        if self.verbose:
            print(
                f"  decoded {self.insn_count:,} instructions in {len(self.prologues):,} "
                f"prologue-anchored functions ({time.time() - t0:.1f}s)",
                file=sys.stderr,
            )


# ==========================================================================================
# SEEN - every runs/*/emulator.log
# ==========================================================================================
STDOUT_RE = re.compile(rb"\[stdout\] (.*?)(?:\x1b\[m)?$", re.M)
THRESH_RE = re.compile(rb"threshold T=(\d)")
SINK_RE = re.compile(rb"route retail_log sink to stdout")


# run name -> log mtime. ⚠ "most recent run" must NOT be `max(name)`: run directories are mostly
# `YYYYMMDD-HHMMSS-…` but not all of them are (`sweep-sub0-5`), so lexicographic order silently
# reports the wrong run as the latest sighting.
RUN_ORDER: dict[str, float] = {}


def scan_runs(verbose: bool = True) -> tuple[dict[str, list[str]], dict[str, int | None], int]:
    """-> (guest line -> [run names, oldest first], run -> applied T or None, n_runs)."""
    lines: dict[str, list[str]] = collections.defaultdict(list)
    thresholds: dict[str, int | None] = {}
    runs = sorted(RUNS.glob("*/emulator.log"), key=lambda p: p.stat().st_mtime)
    t0 = time.time()
    for p in runs:
        run = p.parent.name
        RUN_ORDER[run] = p.stat().st_mtime
        raw = p.read_bytes()
        m = THRESH_RE.search(raw)
        thresholds[run] = int(m.group(1)) if (m and SINK_RE.search(raw)) else None
        for hit in STDOUT_RE.finditer(raw):
            try:
                s = hit.group(1).decode("utf-8", "replace").rstrip()
            except Exception:
                continue
            if s:
                lines[s].append(run)
    if verbose:
        print(
            f"  {len(runs)} runs, {len(lines):,} distinct guest [stdout] lines "
            f"({time.time() - t0:.1f}s)",
            file=sys.stderr,
        )
    return lines, thresholds, len(runs)


# ==========================================================================================
# ⭐ THE FRESHNESS GUARD  (added 2026-08-10)
# ==========================================================================================
# WHY. The SEEN column is a claim about *every run ever made*, so it decays the moment a run
# finishes - and it decays SILENTLY, because a stale `seen=0` is indistinguishable from a real
# NOT-SEEN.  On 2026-08-10 the checked-in TSV was two days and 102 run logs out of date, and
# THREE independent lanes hit it in one session (`historicaldocs/design/arrival-handoff.md` §3.2 wrote the rule
# down; it had no guard, so the next lane paid for it again).
#
# ⚠ A mistake this project has already made twice: writing the lesson in prose and no code.
# The TSV therefore carries its own provenance on line 1 and the tool can be asked, offline and
# in well under a second, whether the file on disk still describes the tree.
#
# Line 1 is a `#` comment, so a reader that skips comment lines is unaffected.  The two in-tree
# consumers (`capture-rig/state-machine.py`, `capture-rig/eboot-atlas.py`) were updated in the
# same commit; `read_catalogue_tsv()` below is the shared, correct reader.
PROVENANCE_SCHEMA = 1
PROVENANCE_PREFIX = "# diagnostic-catalogue.tsv provenance="


def run_logs() -> list[Path]:
    """Every `runs/*/emulator.log` on disk, name-sorted (stable, unlike mtime order)."""
    return sorted(RUNS.glob("*/emulator.log"), key=lambda p: p.parent.name)


def run_fingerprint() -> dict:
    """A cheap, exact description of the SEEN column's whole input set.

    Uses (name, size, mtime_ns) rather than content hashes: hashing 248 logs costs seconds,
    stat()ing them costs milliseconds, and an *appended* log - the case that matters, since a
    run that prints a new string is exactly an append - changes both size and mtime.
    """
    logs = run_logs()
    parts, newest_ns, newest_name = [], 0, ""
    for p in logs:
        st = p.stat()
        parts.append(f"{p.parent.name}\t{st.st_size}\t{st.st_mtime_ns}")
        if st.st_mtime_ns > newest_ns:
            newest_ns, newest_name = st.st_mtime_ns, p.parent.name
    return {
        "schema": PROVENANCE_SCHEMA,
        "runs": len(logs),
        "runs_fingerprint": hashlib.sha256("\n".join(parts).encode()).hexdigest(),
        "newest_run": newest_name,
        "newest_mtime_ns": newest_ns,
    }


def read_provenance(path: Path) -> dict | None:
    """Parse line 1 of a catalogue TSV.  None = the file predates this guard (or is not one)."""
    try:
        with path.open(encoding="utf-8") as fh:
            first = fh.readline()
    except OSError:
        return None
    if not first.startswith(PROVENANCE_PREFIX):
        return None
    try:
        return json.loads(first[len(PROVENANCE_PREFIX):].strip())
    except ValueError:
        return None


def read_catalogue_tsv(path: Path):
    """⭐ The reader every consumer should use: skips the provenance line, yields dict rows.

    Kept here rather than in each consumer so there is one place that knows the file's shape.
    """
    import csv as _csv

    with path.open(encoding="utf-8") as fh:
        yield from _csv.DictReader((ln for ln in fh if not ln.startswith("#")), delimiter="\t")


def staleness(path: Path = OUT_TSV) -> tuple[int, str, list[str]]:
    """-> (exit code, one-line verdict, names of run logs newer than the catalogue).

    2 = cannot tell (missing file / no provenance) · 1 = stale · 0 = current.
    ⚠ "cannot tell" is deliberately the WORST code: an unanswerable freshness question is not a
    pass, and this project's signature failure is reading silence as a negative result.
    """
    if not path.exists():
        return 2, f"{path} does not exist - nothing to check", []
    prov = read_provenance(path)
    if prov is None:
        return 2, (f"{path.name} carries NO provenance line - it predates the freshness guard, "
                   f"so its SEEN column cannot be dated. Regenerate with --write."), []
    now = run_fingerprint()
    if prov.get("runs_fingerprint") == now["runs_fingerprint"]:
        return 0, f"current - {now['runs']} run logs, unchanged since the catalogue was written", []

    since = int(prov.get("newest_mtime_ns", 0))
    newer = sorted(p.parent.name for p in run_logs() if p.stat().st_mtime_ns > since)
    delta = now["runs"] - int(prov.get("runs", 0))
    bits = [f"{now['runs']} run logs on disk vs {prov.get('runs', '?')} in the catalogue"]
    if delta > 0:
        bits.append(f"+{delta} new")
    elif delta < 0:
        bits.append(f"{delta} removed")
    if newer:
        bits.append(f"{len(newer)} newer than the newest run it saw "
                    f"({prov.get('newest_run', '?')})")
    return 1, "STALE - " + "; ".join(bits), newer


def fmt_to_regex(fmt: str) -> re.Pattern:
    """A format string as an anchored regex: literals exact, conversions wildcards.

    ⚠ Trailing whitespace is optional on BOTH sides. The log reader strips the line, and an
    empty `%s` at the end leaves the preceding literal space dangling - which is exactly why
    `world_controller: successfully changed world to: %s` and
    `…KTOBUCKET: LAST BAP LOG NOTIFICATION: %s` first came back NOT SEEN when they had in fact
    fired in 100+ runs. A SEEN column that reports a false negative is worse than no column.
    """
    fmt = fmt.rstrip()
    chunks: list[tuple[str, str]] = []
    last = 0
    for m in SPEC_RE.finditer(fmt):
        if m.start() > last:
            chunks.append(("lit", fmt[last:m.start()]))
        chunks.append(("lit", "%") if m.group() == "%%" else ("conv", ""))
        last = m.end()
    if last < len(fmt):
        chunks.append(("lit", fmt[last:].rstrip()))
    # A format ending in a conversion that renders EMPTY leaves the preceding literal's
    # trailing space dangling, and the log reader strips it. Make that whitespace optional.
    if chunks and chunks[-1][0] == "conv":
        for i in range(len(chunks) - 1, -1, -1):
            if chunks[i][0] == "lit":
                chunks[i] = ("lit-ws", chunks[i][1].rstrip())
                break
    out = []
    for kind, text in chunks:
        if kind == "conv":
            out.append(".*?")
        elif kind == "lit-ws":
            out.append(re.escape(text) + r"\s*")
        else:
            out.append(re.escape(text))
    return re.compile("^" + "".join(out) + r"\s*$", re.S)


def match_seen(rows: list[Row], lines: dict[str, list[str]]) -> dict[str, list[Row]]:
    """Two stage: literal-head prefix filter, then a full anchored regex confirm."""
    heads: dict[str, list[Row]] = collections.defaultdict(list)
    for r in rows:
        m = SPEC_RE.search(r.text)
        head = r.text[:m.start()] if m else r.text
        # Both forms: the log line is rstripped, so a head ending in a space (the usual shape
        # when the trailing conversion rendered empty) would never prefix-match otherwise.
        for h in {head, head.rstrip()}:
            if len(h) >= 6:  # a shorter head matches too much to be a useful filter
                heads[h].append(r)
    lens = sorted({len(h) for h in heads})
    compiled = {id(r): fmt_to_regex(r.text) for hs in heads.values() for r in hs}
    unmatched: dict[str, list[Row]] = {}
    for line, runs in lines.items():
        hit = False
        for L in lens:
            if L > len(line):
                break
            for r in heads.get(line[:L], ()):
                if compiled[id(r)].match(line):
                    r.seen = True
                    for run in runs:
                        if run not in r.runs:
                            r.runs.append(run)
                    hit = True
        if not hit:
            unmatched[line] = []
    return unmatched


# ==========================================================================================
# build
# ==========================================================================================
def hot_range_of(va: int | None) -> str | None:
    if va is None:
        return None
    for lo, hi, name in HOT_RANGES:
        if lo <= va < hi:
            return name
    return None


def demangle(s: str) -> str:
    """`26c_x` -> `c_x`; `N3cui30c_yE` -> `cui::c_y`.  Simple Itanium type names only."""
    if s.startswith("N") and s.endswith("E"):
        body, parts, i = s[1:-1], [], 0
        while i < len(body):
            j = i
            while j < len(body) and body[j].isdigit():
                j += 1
            if j == i:
                return s
            n = int(body[i:j])
            parts.append(body[j:j + n])
            i = j + n
        return "::".join(parts)
    j = 0
    while j < len(s) and s[j].isdigit():
        j += 1
    return s[j:] if j and int(s[:j]) == len(s) - j else s


def build(img: Image, scan: Scan) -> tuple[list[Row], list[Row]]:
    """-> (catalogue rows, rtti rows)."""
    rows: dict[int, Row] = {}

    def add(va: int, text: str, tier: str, subsystem: str | None) -> Row:
        r = rows.get(va)
        if r is None:
            r = rows[va] = Row(va, text, subsystem, tier=tier)
            r.specs = [m.group() for m in SPEC_RE.finditer(text)]
            r.refs = list(scan.refs_by_target.get(va, ()))
        return r

    # --- tier A: every referenced string carrying a subsystem prefix ----------------------
    for va in scan.refs_by_target:
        b = img.cstr(va)
        if b is None:
            continue
        s = b.decode("ascii", "replace")
        m = SUBSYS_RE.match(s)
        if m:
            add(va, s, "A:subsystem", m.group(1))

    # --- tier A2: subsystem strings with NO reference this scan can see -------------------
    # Reference-first cannot find a string reached through a table.  Enumerating them is the
    # only honest way to say how much of the space that is; they are flagged `ref=none`.
    #
    # ⚠ NUL-PRECEDED starts ONLY.  An earlier revision also tried `start+1` (to catch a string
    # preceded by alien bytes, as `voice_globals` is) and that manufactured a whole fake
    # subsystem: 13 Demonware `Error: …` strings were catalogued as `rror: …` at addresses one
    # byte past their true start - i.e. exactly the ATLAS §4.2b string-interior defect, created
    # by the tool rather than walked into. Strings behind alien bytes are found by REFERENCE
    # (tier A/B), which is the only method that can know where they start.
    for m in re.finditer(rb"(?<=\x00)[\x20-\x7e\t]{6,}(?=\x00)", img.blob):
        va = m.start() + img.text_lo
        if va in rows:
            continue
        s = m.group().decode("ascii", "replace")
        sm = SUBSYS_RE.match(s)
        if sm:
            add(va, s, "A:subsystem", sm.group(1))

    # --- tier B: anything referenced from a range we are stuck in ------------------------
    for va, refs in scan.refs_by_target.items():
        hot = {hot_range_of(r.fn) for r in refs} - {None}
        if not hot:
            continue
        b = img.cstr(va)
        if b is None or len(b) < 3:
            continue
        s = b.decode("ascii", "replace")
        if va in rows:
            continue
        if RTTI_RE.match(s):
            continue  # counted in the RTTI section instead
        add(va, s, "B:hot-range", None)

    # --- tier C: RTTI class names --------------------------------------------------------
    rtti: dict[int, Row] = {}
    for m in re.finditer(rb"\x00([A-Za-z0-9_]{4,255})\x00", img.blob):
        s = m.group(1).decode("ascii")
        if not RTTI_RE.match(s):
            continue
        va = m.start() + 1 + img.text_lo
        r = Row(va, s, demangle(s), tier="C:rtti")
        r.refs = list(scan.refs_by_target.get(va, ()))
        rtti[va] = r

    return sorted(rows.values(), key=lambda r: r.vaddr), sorted(rtti.values(), key=lambda r: r.vaddr)


# ==========================================================================================
# controls  (ATLAS §4.2 / §4.4d - mandatory, printed, and fatal)
# ==========================================================================================
def controls(img: Image, scan: Scan, rows: list[Row], rtti: list[Row],
             thresholds: dict[str, int | None],
             allocs: list[dict], unmatched: dict) -> list[tuple[str, bool, str]]:
    by_va = {r.vaddr: r for r in rows}
    by_text = {r.text: r for r in rows}
    out: list[tuple[str, bool, str]] = []

    def c(name: str, ok: bool, detail: str = "") -> None:
        out.append((name, bool(ok), detail))

    c("build identity (sha256) == 01.29 reference the census was measured on",
      img.sha256 == EBOOT_SHA256, f"{img.sha256} ({BUILD['id']})")

    # -- the decoder, against guest-diagnostics.md §3's independently-derived census -------
    c("`call 0xe4da0` sites == 1632 (guest-diagnostics.md §3)",
      len(scan.gate_sites) == 1632, f"{len(scan.gate_sites)}")
    valid = [(a, s, l) for (a, s, l) in scan.gate_sites
             if s is not None and l is not None and 0 <= s <= 21 and 0 <= l <= 5]
    c("...of which 1614 have both immediates statically recoverable",
      len(valid) == 1614, f"{len(valid)}")
    hist = collections.Counter(l for _, _, l in valid)
    want = {0: 80, 1: 265, 2: 277, 3: 366, 4: 626}
    c("level histogram == {0:80, 1:265, 2:277, 3:366, 4:626}",
      dict(hist) == want, str(dict(sorted(hist.items()))))
    c("22 distinct retail_log system ids (0..21)",
      len({s for _, s, _ in valid}) == 22, f"{len({s for _, s, _ in valid})}")

    # -- ⭐ THE VOICE CONTROL: the string reference that cost three hours ------------------
    vg = by_text.get("voice_globals")
    c("⭐ `voice_globals` catalogued at 0x167d3e0", vg is not None and vg.vaddr == 0x167D3E0,
      f"{vg.vaddr:#x}" if vg else "MISSING")
    c("⭐ `voice_globals` attributed to fn 0xb41000",
      vg is not None and any(r.fn == 0xB41000 for r in vg.refs),
      ",".join(f"{r.fn:#x}" for r in vg.refs) if vg else "")

    # -- the allocation heuristic, both known cases ---------------------------------------
    c("⭐ allocation tag `voice_globals` -> global 0x2815770 from fn 0xb41000",
      any(a["tag"] == "voice_globals" and a["fn"] == 0xB41000 and a["global"] == 0x2815770
          for a in allocs),
      next((f"{a['global']:#x}" for a in allocs if a["tag"] == "voice_globals"), "MISSING"))
    c("allocation tag `retail_log` -> global 0x256eef0 from fn 0xe4890",
      any(a["tag"] == "retail_log" and a["fn"] == 0xE4890 and a["global"] == 0x256EEF0
          for a in allocs),
      next((f"{a['global']:#x}" for a in allocs if a["tag"] == "retail_log"), "MISSING"))

    # -- the ui-substage name table (design/eboot-map.md §7) ------------------------------
    tbl = img.blob[0x164993F - img.text_lo:0x164993F - img.text_lo + 24]
    c("ui-substage table anchor 0x164993f reads `@waiting_for_login`",
      tbl.startswith(b"@waiting_for_login\x00"), repr(tbl[:20]))
    names = img.blob[0x1649940 - img.text_lo:0x1649940 - img.text_lo + 4096].split(b"\x00")
    c("...+10 entries in, ENUM(19) == `bootflow bap signin`",
      len(names) > 29 and names[29] == b"bootflow bap signin",
      names[29].decode() if len(names) > 29 else "")

    # -- the SEEN column, both directions -------------------------------------------------
    seen_ctl = by_va.get(0x1648203)
    c("⭐ SEEN: `world_controller:state_manager: Entering state` marked SEEN",
      seen_ctl is not None and seen_ctl.seen,
      f"{len(seen_ctl.runs)} runs" if seen_ctl else "MISSING")
    c("...and it is UNFILTERED, so SEEN is a clean execution trace for it",
      seen_ctl is not None and seen_ctl.unfiltered, "")
    # ⛔ RETRACTED 2026-08-10 - this pair used to assert that `networking:investment:peer:
    # Account soids in error state` (0x162F4A1) is **NOT SEEN**, and it FAILED on regeneration:
    # the string fired in `20260809-223241-TASK7-SUPPRESS` and `20260809-225119-UNION8-MOONBASE`.
    # The control was not wrong when written - it EXPIRED, which is the same decay the SEEN
    # column itself suffers, reproduced inside the guard meant to police it.
    #
    # ⭐ Replaced with two controls that cannot expire, because they assert the MATCHER rather
    # than any one string's current state:
    #   (1) `seen` must agree with an INDEPENDENT literal grep of the run logs - a different code
    #       path from `fmt_to_regex`, so a broken matcher fails in either direction; and
    #   (2) the column must still DISCRIMINATE (plenty of strong-evidence rows remain unseen), so
    #       "everything is SEEN" can never pass silently.
    ts = [v for v in thresholds.values() if v is not None]
    for va in (0x1648203, 0x162F4A1):
        row = by_va.get(va)
        if row is None:
            c(f"⭐ SEEN agrees with an independent literal grep ({va:#x})", False, "ROW MISSING")
            continue
        # Derive the literal, never transcribe it: everything before the first conversion.
        literal = row.text.split("%")[0].rstrip()
        hits = sorted(p.parent.name for p in run_logs()
                      if literal.encode() in p.read_bytes()) if len(literal) >= 20 else []
        c(f"⭐ SEEN({va:#x}) agrees with an independent literal grep of runs/ "
          f"[{'both say SEEN' if row.seen else 'both say NOT SEEN'}]",
          row.seen == bool(hits),
          f"seen={int(row.seen)} grep={len(hits)} run(s)"
          + (f" (e.g. {hits[-1]})" if hits else ""))
    strong_unseen = sum(1 for r in rows
                        if not r.seen and r.seen_evidence(min(ts) if ts else 5) == "strong")
    c("⭐ the SEEN column still DISCRIMINATES - strong-evidence rows that are NOT seen",
      strong_unseen >= 100, f"{strong_unseen} strong + unseen")
    # ⭐ The coverage control that makes the SEEN column two-sided: if a line the client
    # actually printed has no catalogued string, the catalogue has a hole and every NOT SEEN
    # verdict is suspect. This is the check that found the empty-trailing-`%s` false negatives.
    c("⭐ every observed guest line is explained by a catalogued string",
      len(unmatched) == 0, f"{len(unmatched)} unexplained")
    c("SEEN scan found >=100 runs with a measured threshold",
      sum(1 for v in thresholds.values() if v is not None) >= 100,
      f"{sum(1 for v in thresholds.values() if v is not None)}/{len(thresholds)}")

    # -- coverage floors: a scan that silently collapses must fail, not look tidy ----------
    c("catalogue >= 1700 strings", len(rows) >= 1700, f"{len(rows)}")
    c("RTTI names >= 2000", len(rtti) >= 2000, f"{len(rtti)}")
    c("decoder desync low: unconfirmed `lea` sites < 1% of confirmed refs",
      scan.unconfirmed_lea < 0.01 * sum(len(v) for v in scan.refs_by_target.values()),
      f"{scan.unconfirmed_lea} unconfirmed")
    return out


# ==========================================================================================
# report
# ==========================================================================================
def subsystem_index(rows: list[Row], depth: int = 2) -> list[dict]:
    groups: dict[str, list[Row]] = collections.defaultdict(list)
    for r in rows:
        if not r.subsystem:
            continue
        groups[":".join(r.subsystem.split(":")[:depth])].append(r)
    out = []
    for k, rs in groups.items():
        fns = sorted({r.fn for r in rs if r.fn is not None})
        out.append({
            "subsystem": k,
            "strings": len(rs),
            "seen": sum(1 for r in rs if r.seen),
            "str_lo": min(r.vaddr for r in rs),
            "str_hi": max(r.vaddr for r in rs),
            "fn_lo": fns[0] if fns else None,
            "fn_hi": fns[-1] if fns else None,
            "fns": len(fns),
            "leaf_prefixes": sorted({r.subsystem for r in rs}),
            "fn_list": fns,
        })
    return sorted(out, key=lambda d: -d["strings"])


def allocations(img: Image, scan: Scan, rows: list[Row], rtti: list[Row]) -> list[dict]:
    """`[I]` Allocation / global tags.

    A bare-identifier string `lea`d by a function that also takes the address of a data-or-BSS
    global; the global reported is the one whose `lea` sits nearest the tag's `lea`, which is
    the shape both known allocators emit:

        0xb41000  lea r14,[0x2815770] ; lea rsi,[voice_globals] ; call 0x23de00   (size 0x6ba0)
        0x0e4890  lea rdi,[retail_log] ; mov esi,0x75ba8 ; call 0xc4210 ; mov [0x256eef0],rax

    ⚠ This is a HEURISTIC, tagged `[I]`, and it is the only heuristic in this tool. Its two
    controls are asserted in `controls()`; a row here is a lead, not a fact.
    """
    out = []
    for va, refs in scan.refs_by_target.items():
        b = img.cstr(va, 80)
        if b is None:
            continue
        s = b.decode("ascii", "replace")
        if not TAG_RE.match(s):
            continue
        for r in refs:
            gs = scan.globals_by_fn.get(r.fn, ())
            if not gs:
                continue
            site, g = min(gs, key=lambda sg: abs(sg[0] - r.site))
            out.append({"tag": s, "tag_va": va, "fn": r.fn, "global": g,
                        "hot": hot_range_of(r.fn),
                        "globals": sorted({x[1] for x in gs})})
    return sorted(out, key=lambda d: (d["tag"], d["fn"]))


def stats(rows, rtti, idx, thresholds, allocs, scan) -> dict:
    ts = [v for v in thresholds.values() if v is not None]
    min_T = min(ts) if ts else 5
    ev = collections.Counter(r.seen_evidence(min_T) for r in rows)
    lv = collections.Counter(r.level for r in rows if r.level is not None)
    spec = collections.Counter(s for r in rows for s in r.specs)
    hot = collections.Counter(hot_range_of(r.fn) for r in rows if hot_range_of(r.fn))
    hot_seen = collections.Counter(hot_range_of(r.fn) for r in rows
                                   if hot_range_of(r.fn) and r.seen)
    return {
        "rows": len(rows),
        "seen": sum(1 for r in rows if r.seen),
        "with_ref": sum(1 for r in rows if r.refs),
        "no_ref": sum(1 for r in rows if not r.refs),
        "gated": sum(1 for r in rows if r.level is not None),
        "level_hist": dict(sorted(lv.items())),
        "level0_seen": sum(1 for r in rows if r.level == 0 and r.seen),
        "level0": sum(1 for r in rows if r.level == 0),
        "seen_ungated": sum(1 for r in rows if r.seen and r.level is None),
        "runs": len(thresholds),
        "runs_with_sink": len(ts),
        "T_hist": dict(sorted(collections.Counter(ts).items())),
        "min_T_ever": min(ts) if ts else None,
        "rtti": len(rtti),
        "subsystems": len(idx),
        "top_specs": spec.most_common(12),
        "hot": dict(hot),
        "hot_seen": dict(hot_seen),
        "evidence": dict(ev),
        "emit_sites": len(scan.emit_sites),
        "gate_sites": len(scan.gate_sites),
        "emits": sum(1 for r in rows if r.emits),
        "unfiltered": sum(1 for r in rows if r.unfiltered),
        "allocs": len(allocs),
        "arity0": sum(1 for r in rows if r.arity == 0),
        "max_arity": max((r.arity for r in rows), default=0),
        "index": idx[:40],
    }


def latest_run(runs: list[str]) -> str:
    return max(runs, key=lambda n: RUN_ORDER.get(n, 0.0)) if runs else ""


def write_tsv(path: Path, rows: list[Row], thresholds: dict[str, int | None],
              eboot_sha256: str = "") -> dict:
    ts = [v for v in thresholds.values() if v is not None]
    min_T = min(ts) if ts else 5
    cols = ["vaddr", "tier", "subsystem", "root", "fn", "hot_range", "sys", "level",
            "emits", "unfiltered", "evidence", "arity", "specs", "n_refs", "ref_sites",
            "seen", "n_runs", "last_run", "text"]
    # ⭐ line 1 is the freshness guard - see staleness() above.  It is a `#` comment so that a
    # comment-skipping reader is unaffected; `read_catalogue_tsv()` is the reference reader.
    prov = run_fingerprint()
    prov["generated"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
    prov["eboot_sha256"] = eboot_sha256
    prov["rows"] = len(rows)
    prov["seen"] = sum(1 for r in rows if r.seen)
    with path.open("w", encoding="utf-8") as fh:
        fh.write(PROVENANCE_PREFIX + json.dumps(prov, sort_keys=True) + "\n")
        fh.write("\t".join(cols) + "\n")
        for r in rows:
            fh.write("\t".join([
                f"{r.vaddr:#x}",
                r.tier,
                r.subsystem or "",
                (r.subsystem or "").split(":")[0],
                f"{r.fn:#x}" if r.fn is not None else "",
                hot_range_of(r.fn) or "",
                str(r.sys) if r.sys is not None else "",
                str(r.level) if r.level is not None else "",
                "1" if r.emits else "0",
                "1" if r.unfiltered else "0",
                r.seen_evidence(min_T),
                str(r.arity),
                " ".join(r.specs),
                str(len(r.refs)),
                " ".join(f"{x.site:#x}" for x in r.refs[:8]),
                "1" if r.seen else "0",
                str(len(r.runs)),
                latest_run(r.runs),
                r.text.replace("\t", "\\t").replace("\n", "\\n"),
            ]) + "\n")
    return prov


# ==========================================================================================
# queries - "which subsystem is this address in?" as a command, not a grep
# ==========================================================================================
def query(rows: list[Row], rtti: list[Row], idx: list[dict], min_T: int, args) -> None:
    def line(r: Row) -> str:
        mark = "SEEN" if r.seen else {"strong": "----", "weak-blind": "BLND",
                                      "none": "  · "}[r.seen_evidence(min_T)]
        fn = f"{r.fn:#09x}" if r.fn is not None else " ref=none"
        lv = f"L{r.level}" if r.level is not None else "L-"
        return f"{mark} {r.vaddr:#09x} fn {fn} {lv:<3} {r.text}"

    if args.addr is not None:
        va = args.addr
        print(f"query address {va:#x}")
        hot = hot_range_of(va)
        print(f"  hot range : {hot if hot else '(none of the four)'}")
        fns = sorted({r.fn for r in rows if r.fn is not None})
        k = bisect.bisect_right(fns, va) - 1
        owner = fns[k] if k >= 0 else None
        if owner is not None:
            here = [r for r in rows if r.fn == owner]
            # A prefix carried by only ONE string in the whole image is a label, not a
            # subsystem (`in_os_party_chat: %s` is a status-overlay line). Rank by weight so
            # the answer to "which subsystem is this" is not a one-off label.
            weight = collections.Counter(r.subsystem for r in rows if r.subsystem)
            subs = sorted({r.subsystem for r in here if r.subsystem},
                          key=lambda s: -weight[s])
            shown = [f"{s} (label?, 1 string)" if weight[s] == 1 else f"{s} ({weight[s]})"
                     for s in subs]
            print(f"  ⭐ nearest catalogued function AT OR BELOW: {owner:#x} (+{va - owner:#x})")
            print(f"     subsystem(s): {', '.join(shown) if shown else '(no subsystem prefix - '
                  f'read the strings below)'}")
            for r in here[:12]:
                print(f"       {line(r)}")
            if len(here) > 12:
                print(f"       ... {len(here)-12} more")
        nxt = fns[k + 1] if k + 1 < len(fns) else None
        if nxt is not None:
            subs = sorted({r.subsystem for r in rows if r.fn == nxt and r.subsystem})
            print(f"  next catalogued function ABOVE: {nxt:#x} (-{nxt - va:#x})"
                  f"  {', '.join(subs) if subs else '(no subsystem prefix)'}")
        brackets = [d for d in idx if d["fn_lo"] is not None and d["fn_lo"] <= va <= d["fn_hi"]]
        if brackets:
            print("  subsystems whose function span brackets it (weak - spans overlap):")
            for d in sorted(brackets, key=lambda d: d["fn_hi"] - d["fn_lo"])[:6]:
                print(f"     {d['subsystem']}  fns {d['fn_lo']:#x}..{d['fn_hi']:#x}")
        return
    if args.subsystem:
        for r in rows:
            if r.subsystem and r.subsystem.startswith(args.subsystem):
                print(line(r))
        return
    if args.text:
        needle = args.text.lower()
        for r in rows:
            if needle in r.text.lower():
                print(line(r))
        for r in rtti:
            if needle in r.text.lower() or needle in (r.subsystem or "").lower():
                print(f"RTTI {r.vaddr:#09x} {r.subsystem or r.text}")
        return


# ==========================================================================================
def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--eboot", default=str(EBOOT))
    ap.add_argument("--write", action="store_true", help="regenerate the markdown + tsv")
    ap.add_argument("--json", metavar="PATH", help="dump every row as JSON")
    ap.add_argument("--addr", type=lambda s: int(s, 0), help="query: which subsystem owns this vaddr")
    ap.add_argument("--subsystem", help="query: list a subsystem's strings")
    ap.add_argument("--text", help="query: substring search over every catalogued string")
    ap.add_argument("--no-runs", action="store_true", help="skip the SEEN scan (controls will fail)")
    ap.add_argument("--stats", action="store_true", help="summary numbers as JSON")
    ap.add_argument("--dump-unmatched", action="store_true",
                    help="print every observed guest line no catalogued string explains")
    ap.add_argument("--check-stale", action="store_true",
                    help="⭐ freshness guard: is the checked-in TSV's SEEN column still current? "
                         "Offline, <1s, no eboot parse. Exit 0 current, 1 STALE, 2 cannot tell.")
    args = ap.parse_args()

    # ⭐ The guard runs before anything expensive: this is the call a session makes *before*
    # citing a NOT-SEEN off the file, so it must be cheap enough to be reflexive.
    if args.check_stale:
        code, verdict, newer = staleness()
        print(f"{OUT_TSV}\n  {verdict}")
        for name in newer[:20]:
            print(f"    newer: {name}")
        if len(newer) > 20:
            print(f"    … and {len(newer) - 20} more")
        if code:
            print("\n⚠ The `seen` / `n_runs` / `last_run` columns MUST NOT be cited from this file.\n"
                  "  Either regenerate it:   python3 -B capture-rig/diagnostic-catalogue.py --write\n"
                  "  or ask the live CLI:    python3 -B capture-rig/diagnostic-catalogue.py --text …\n"
                  "  (the `evidence` column is static and stays valid either way.)")
        return code

    print(f"reading {args.eboot}", file=sys.stderr)
    img = Image(Path(args.eboot))
    scan = Scan(img)
    scan.run()
    lines, thresholds, n_runs = ({}, {}, 0) if args.no_runs else scan_runs()
    rows, rtti = build(img, scan)
    unmatched = match_seen(rows, lines)
    idx = subsystem_index(rows)
    allocs = allocations(img, scan, rows, rtti)
    ctl = controls(img, scan, rows, rtti, thresholds, allocs, unmatched)

    if args.addr is not None or args.subsystem or args.text:
        _ts = [v for v in thresholds.values() if v is not None]
        query(rows, rtti, idx, min(_ts) if _ts else 5, args)
        return 0

    if args.stats:
        print(json.dumps(stats(rows, rtti, idx, thresholds, allocs, scan), indent=1))
        return 0

    if args.dump_unmatched:
        for line in sorted(unmatched):
            print(line)
        return 0

    print()
    for name, ok, detail in ctl:
        print(f"{'PASS' if ok else 'FAIL'}  {name}" + (f"  [{detail}]" if detail else ""))
    passed = all(ok for _, ok, _ in ctl)
    print("\n" + ("ALL CONTROLS PASS" if passed else
                  "*** CONTROL FAILURE - this output must not be cited ***"))

    seen = sum(1 for r in rows if r.seen)
    print(f"\ncatalogue: {len(rows)} strings, {seen} SEEN ({100*seen/max(len(rows),1):.1f}%), "
          f"{len(rtti)} RTTI names, {len(idx)} subsystems, "
          f"{len(unmatched)} observed lines matched to no catalogued string")

    # ⭐ WARN, not FAIL: the numbers printed above were scanned live and ARE current - it is the
    # checked-in file that has decayed.  Failing the control set here would teach readers to
    # ignore a control, which is worse than the staleness it reports.
    if not args.write and not args.no_runs:
        code, verdict, _ = staleness()
        if code:
            print(f"\n⚠⚠ {OUT_TSV.name} on disk: {verdict}\n"
                  f"   Its seen/n_runs/last_run columns are NOT what this run just measured.\n"
                  f"   `--check-stale` for detail; `--write` to refresh.")

    if args.json:
        Path(args.json).write_text(json.dumps({
            "eboot_sha256": img.sha256,
            "controls": [{"name": n, "passed": o, "detail": d} for n, o, d in ctl],
            "thresholds": thresholds,
            "rows": [{"vaddr": r.vaddr, "text": r.text, "subsystem": r.subsystem, "tier": r.tier,
                      "fn": r.fn, "sys": r.sys, "level": r.level, "arity": r.arity,
                      "specs": r.specs, "seen": r.seen, "runs": sorted(r.runs),
                      "emits": r.emits, "unfiltered": r.unfiltered,
                      "last_run": latest_run(r.runs),
                      "refs": [{"site": x.site, "fn": x.fn} for x in r.refs]} for r in rows],
            "rtti": [{"vaddr": r.vaddr, "mangled": r.text, "name": r.subsystem} for r in rtti],
            "subsystems": idx,
        }, indent=1))
        print(f"wrote {args.json}", file=sys.stderr)

    if args.write:
        OUT_TSV.parent.mkdir(parents=True, exist_ok=True)
        written = write_tsv(OUT_TSV, rows, thresholds, img.sha256)
        # ⭐ READ THE GUARD BACK, in the same breath as the write: a guard that is never parsed
        # is decoration.  ⚠ The axis tested is "does the provenance line ROUND-TRIP", NOT "is the
        # file current" - re-stat'ing would be flaky whenever the operator has a run live and its
        # emulator.log is still growing, and a control that fails for the wrong reason gets muted.
        back = read_provenance(OUT_TSV)
        if back != written:
            print(f"*** freshness guard did NOT round-trip: wrote {written!r}, read {back!r}",
                  file=sys.stderr)
            passed = False
        else:
            print(f"freshness guard: provenance line round-trips "
                  f"({written['runs']} run logs, fingerprint {written['runs_fingerprint'][:12]}…)",
                  file=sys.stderr)
        from diagnostic_catalogue_doc import render  # noqa: E402  (kept out of this file)
        OUT_MD.write_text(render(img, scan, rows, rtti, idx, thresholds, unmatched, ctl, allocs,
                                 len(lines)))
        print(f"wrote {OUT_TSV}\nwrote {OUT_MD}", file=sys.stderr)

    return 0 if passed else 2


if __name__ == "__main__":
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    sys.exit(main())
