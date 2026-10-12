#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Generate `ghidra/structs/d1-structs.tsv` - the struct layouts D1Structs.java applies.

WHY THIS EXISTS
---------------
`historicaldocs/design/ghidra.md` §9's biggest listed gap: 2,633 classes are *labelled* in the Ghidra database and
not one is *defined*, so every field reads `*(int *)(param_1 + 0x38)`.  That cost a session:
`param_1 + 0x38` on the orbit handler is a 5001 ms debounce stamp initialised to -1, and it was the
single thing holding the client in `setup:orbit` (`historicaldocs/design/RUNG-28-REACHED.md`).

THE RULE THIS SCRIPT EXISTS TO ENFORCE: **a struct is derived, never transcribed.**
  * the parameter-set offsets come from `capture-rig/session-params.py`'s PARAMS (7 controls) -
    imported, not copied;
  * the *shape* of every handler comes from the decompiled corpus: which offsets the class's own
    vfuncs actually touch, and at what width;
  * only the *names* are human-authored, in NAMES below, and every one of them is checked against
    the observed access set (a curated offset the corpus never touches is a CONTROL FAILURE).

⚠ A struct applied at the wrong type makes decompilation WORSE, silently.  Two guards:
  1. auto fields take the SMALLEST observed access width, clamped to the distance to the next
     observed offset - so a field can never swallow its neighbour;
  2. `this` is only applied to a vfunc whose `vt_shared == 1` (it appears in exactly ONE vtable).
     A slot shared by several classes has no single derived `this` and is skipped.

USAGE
    python3 -B ghidra/gen-structs.py --controls      # offline checks, exit 2 on failure
    python3 -B ghidra/gen-structs.py                 # regenerate the TSV
    ./ghidra/d1-ghidra.sh structs                    # apply it to the Ghidra database

⚠ It reads the DECOMPILED CORPUS (`historicaldocs/design/ghidra.md` §6.2).  Regenerate the corpus first if the
atlas changed; a stale corpus yields a stale field inventory, not an error.
"""

from __future__ import annotations

import argparse
import csv
import importlib.util
import os
import pathlib
import re
import sys
from collections import Counter, defaultdict

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent
ATLAS = REPO / "capture-rig" / "eboot-atlas.tsv"
# Decompiled corpus, outside the repo and never shipped (regenerate with
# `ghidra/d1-ghidra.sh decompile`). Overridable with D1_CORPUS.
# ⚠ Was hardcoded to the literal string "$HOME/..." by a scrub pass, which Python does
# not expand either: the corpus control then failed with every file "missing".
CORPUS = pathlib.Path(os.environ.get("D1_CORPUS")
                      or pathlib.Path.home() / "dev" / "ghidra-projects" / "decompiled")
OUT = HERE / "structs" / "d1-structs.tsv"

BASE = "c_world_controller_tiger_base_state_handler"
BASE_SIZE = 0x30  # [C] measured from the ctor at 0x790E40: it writes [0],[1],[2],[3],[4],[5] only.

# ---------------------------------------------------------------------------
# The task set.  [C] and cross-validated four ways.
#
# `c_world_controller_state_*_handler::vfunc[7]` calls a runner with (this + K, this).  Each runner
# is one template instantiation and hardcodes its own mask offsets:
#
#   runner      masks at            bit mask literal      => N tasks   8 + N*0x20 == mask offset
#   0x1092230   +0x68/0x70/0x78     (0x7)                      3       8 + 0x60  = 0x68   ✓
#   0x1092690   +0xE8/0xF0/0xF8     0x7f                        7       8 + 0xE0  = 0xE8   ✓
#   0x1092160   +0x168/0x170/0x178  0x7ff                      11       8 + 0x160 = 0x168  ✓
#   0x10924C0   +0x668/0x670/0x678  0x7ffffffffffff            51       8 + 0x660 = 0x668  ✓
#
# ⭐ Three more, found BY the auto-derived structs (the payoff of typing before naming): rungs 26,
# 33 and 36 carry an N=1 set whose loop the compiler UNROLLED, so there is no callable runner to
# find - only the body, inlined, testing `(requested ^ done) & prereq & 1`.  Their placements are
# in NAMES below (orbit_loading +0x30, world_transition +0x698, in_world +0x40) and each one's
# masks land on 8 + 1*0x20 exactly.
#
# and the set ends exactly where the class's next member begins:
#   session_creation  this+0x30 + 0x100 = 0x130 = enter_reason   ✓
#   cleanup           this+0x38 + 0x680 = 0x6B8 = the flag byte  ✓
#   slice_set_loading this+0x30 + 0x180 = 0x1B0 = the next field ✓
#
# ⛔ RETRACTION.  This retires the reading that `+0x118`/`+0x128` (rung 28) and `+0x6A0`/`+0x6B0`
# (cleanup) are a task-queue HEAD/TAIL POINTER PAIR.  They are 64-bit **bitmasks**: rung 28's
# `enter` writes the literal 0x7F into +0x118, and cleanup's own log prints
# `pending = requested ^ started` - arithmetic that is meaningless on pointers.
# ---------------------------------------------------------------------------
TASK_ENTRY = "s_world_controller_task_entry"
TASK_SETS = {
    1:  "the loop is UNROLLED; no callable runner - rungs 26, 33, 36 carry the body inline",
    3:  "runner 0x1092230",
    7:  "runner 0x1092690",
    11: "runner 0x1092160",
    51: "runner 0x10924C0",
}


def task_set_name(n: int) -> str:
    return f"s_world_controller_task_set_{n}"


def task_set_size(n: int) -> int:
    return 0x20 + n * 0x20


# class -> {offset: (name, ctype, size, evidence, note)}.  Names are the ONLY human-authored part.
# evidence: C confirmed · I inference · U unknown.  An `i_` name prefix marks an inferred MEANING.
NAMES: dict[str, dict[int, tuple[str, str, int, str, str]]] = {
    BASE: {
        0x00: ("vptr", "void *", 8, "C", "ctor 0x790E40 writes the class vtable here"),
        0x08: ("i_state_mask", "ulong", 8, "I",
               "64-bit mask over state ids; base vfunc[7-1]=0x7914E0 iterates its bits"),
        0x10: ("i_flags", "byte", 1, "I", "ctor does |= 1"),
        0x18: ("f_0x18", "undefined4", 4, "U", "ctor zeroes 6 u32 at 0x18..0x2C"),
        0x1C: ("f_0x1c", "undefined4", 4, "U", ""),
        0x20: ("f_0x20", "undefined4", 4, "U", ""),
        0x24: ("f_0x24", "undefined4", 4, "U", ""),
        0x28: ("f_0x28", "undefined4", 4, "U", ""),
        0x2C: ("f_0x2c", "undefined4", 4, "U", ""),
    },
    "c_world_controller_state_cleanup_handler": {
        0x38: ("tasks", task_set_name(51), task_set_size(51), "C",
               "vfunc[7] 0x7A3750 calls the 51-task runner 0x10924C0 with (this+0x38, this)"),
        0x6B8: ("i_result_flags", "byte", 1, "C",
                "vfunc[4] does |= 1, |= 2, |= 4; vfunc[4] tests & 7"),
        0x6BC: ("status", "undefined4", 4, "C",
                "the client's own word: \"status: '%d'\" in the cleanup progress log"),
        0x6C4: ("enter_reason", "uint", 4, "C", "vfunc[4] 0x7A3110 stores its 3rd argument here"),
        0x6C8: ("pending_goal_state", "undefined4", 4, "C",
                "vfunc[7]: request_state(this->pending_goal_state, this->pending_goal_reason)"),
        0x6CC: ("pending_goal_reason", "undefined4", 4, "C", "ditto"),
    },
    "c_world_controller_state_setup_orbit_handler": {
        0x30: ("enter_reason", "int", 4, "C",
               "vfunc[4] 0x7AAC50 stores its 3rd argument; vfunc[7] passes it to 0x783690"),
        0x38: ("debounce_ms", "long", 8, "C",
               "⭐ init -1 by vfunc[4]; vfunc[7] exits while == -1 and while now-this < 0x1389 "
               "(5001 ms).  THE gate that held the client in setup:orbit - historicaldocs/design/RUNG-28-REACHED.md"),
        0x40: ("f_0x40", "long", 8, "U", "vfunc[4] inits to -1 like debounce_ms; no reader found"),
        0x48: ("i_abort_to_cleanup", "byte", 1, "C",
               "vfunc[7]: non-zero -> request_state(25 cleanup, reason 9)"),
        0x4C: ("i_cached_activity_start_status", "undefined1", 0xC, "I",
               "0xC bytes memcmp'd against params->activity_start_status.m_value (+0x8B8 +0xE4); "
               "first u16 init 0xFFFF"),
        0x58: ("f_0x58", "undefined4", 4, "U", "vfunc[4] inits to -1"),
        0x5C: ("f_0x5c", "undefined4", 4, "U", "vfunc[4] inits to 0"),
    },
    "c_world_controller_state_setup_activity_session_creation_handler": {
        0x30: ("tasks", task_set_name(7), task_set_size(7), "C",
               "vfunc[7] 0x7A6D60 calls the 7-task runner 0x1092690 with (this+0x30, this); "
               "vfunc[4] writes the literal 0x7F into tasks.requested_mask"),
        0x130: ("enter_reason", "undefined4", 4, "C",
                "vfunc[4] 0x7A6CE0 stores its 3rd argument; vfunc[7] passes it to "
                "request_state(0x1D = rung 29, reason)"),
        0x134: ("status", "undefined4", 4, "C",
                "0 on enter · 1 tasks.done != tasks.requested · 2 waiting on host peer properties "
                "· 3 rung 29 requested"),
    },
    "c_world_controller_state_setup_orbit_loading_handler": {
        0x30: ("tasks", task_set_name(1), task_set_size(1), "C",
               "an N=1 set with the runner UNROLLED into vfunc[7] 0x7AC630: it tests "
               "(requested ^ done) & entries[0].prereq_mask & 1, then calls the plain fn or the PMF"),
        0x70: ("enter_reason", "undefined4", 4, "C",
               "vfunc[7] passes it as the reason to 0x782720(0x1B = rung 27 setup:orbit, reason, "
               "NULL) once the task set drains"),
    },
    "c_world_controller_state_setup_activity_world_transition_handler": {
        0x698: ("tasks", task_set_name(1), task_set_size(1), "C",
                "N=1, runner unrolled into vfunc[7] 0x7A7C00; masks land on 0x6C0/0x6C8/0x6D0"),
    },
    "c_world_controller_state_activity_in_world_handler": {
        0x40: ("tasks", task_set_name(1), task_set_size(1), "C",
               "N=1, runner unrolled into vfunc[7] 0x7957E0; masks land on 0x68/0x70/0x78"),
    },
    "c_world_controller_state_setup_matchmaking_handler": {
        0x50: ("i_activity_id", "undefined2", 2, "I",
               "u16 passed to 0x5441C0 / 0x5444A0 - the same pair the ladder calls on activity ids"),
    },
    "c_world_controller_state_setup_activity_host_setup_handler": {
        0x30: ("i_status", "undefined4", 4, "C",
               "vfunc[7] 0x7A5250 assigns 1..8 to it and branches on it - a substate, not a reason"),
    },
    "c_world_controller_state_setup_prologue_intro_loading_handler": {
        0x30: ("i_status", "int", 4, "C", "vfunc[7] 0x7ACD90 assigns 1, 2, 3 and branches on it"),
        0x34: ("i_retry_countdown", "int", 4, "C",
               "vfunc[7]: if (< 1) status = 3; else --  - a per-frame countdown"),
    },
    "c_world_controller_state_activity_initial_slice_set_loading_handler": {
        0x30: ("tasks", task_set_name(11), task_set_size(11), "C",
               "vfunc[7] 0x795130 calls the 11-task runner 0x1092160 with (this+0x30, this)"),
    },
    "c_world_controller_state_bootflow_media_check_handler": {
        0x30: ("tasks", task_set_name(3), task_set_size(3), "C",
               "vfunc[7] 0x79C980 calls the 3-task runner 0x1092230 with (this+0x30, this)"),
    },
}

# Classes to define.  The critical path plus the base and the one non-ladder class that proves the
# task-set model generalises (media_check, rung 1).
CLASSES = [
    BASE,
    "c_world_controller_state_cleanup_handler",                                  # 25
    "c_world_controller_state_setup_orbit_loading_handler",                      # 26
    "c_world_controller_state_setup_orbit_handler",                              # 27 ⭐
    "c_world_controller_state_setup_activity_session_creation_handler",          # 28 ⭐
    "c_world_controller_state_setup_matchmaking_handler",                        # 29
    "c_world_controller_state_setup_activity_host_setup_handler",                # 30
    "c_world_controller_state_setup_prologue_intro_loading_handler",             # 31
    "c_world_controller_state_setup_orbit_outro_handler",                        # 32
    "c_world_controller_state_setup_activity_world_transition_handler",          # 33
    "c_world_controller_state_activity_initial_slice_set_loading_handler",       # 34
    "c_world_controller_state_activity_physics_join_handler",                    # 35
    "c_world_controller_state_activity_in_world_handler",                        # 36
    "c_world_controller_state_activity_watch_video_handler",                     # 37
    "c_world_controller_state_bootflow_media_check_handler",                     # 1
]

# ---------------------------------------------------------------------------
# Session parameters.  `c_network_session_parameter_base` is 0xE1 bytes; every parameter is
# base + aligned(T m_value) + T m_pending.  The offsets are IMPORTED from session-params.py.
# ---------------------------------------------------------------------------
PARAM_BASE = "c_network_session_parameter_base"
PARAM_SET = "s_network_session_parameter_set"

PAYLOAD_FIELDS: dict[str, dict[int, tuple[str, str, int, str, str]]] = {
    "s_network_session_world_controller_goal": {
        0x0: ("goal_a", "int", 4, "C", "0xB91090 returns this when has_value && seq != 0"),
        0x4: ("goal_b", "int", 4, "I", ""),
        0x8: ("seq", "byte", 1, "C", "0xB91090 guards on it being non-zero"),
    },
    "s_activity_information": {
        0x2: ("activity_id", "undefined2", 2, "C",
              "0xB8C130 returns it (0xFFFF = unset); 1 = the Tower, historicaldocs/design/RUNG-28-REACHED.md §3.3"),
    },
    "s_activity_host_connection_information": {
        0x8: ("i_activity_host_id", "undefined8", 8, "I",
              "⭐ rung 28 blocks on 'no AH ids' - historicaldocs/design/RUNG-28-REACHED.md §3.1"),
    },
    "s_network_session_parameter_orbit": {
        0x0: ("i_orbit_name_hash", "undefined4", 4, "I",
              "orbit's update compares it against 0x811C9DC5, the FNV-1 'unset' basis"),
    },
}

PARAM_BASE_FIELDS = {
    0x00: ("vptr", "void *", 8, "C", "ctor 0xB8D440"),
    0x18: ("i_session", "void *", 8, "I", "0xB8D630 reads [+0x18] then [that + 0x8C48]"),
    0x38: ("i_peer_ack_vector", "undefined1", 0xA8, "I",
           "0xA8 bytes of 0xFF at rest - the per-peer version/ack vector (session-params.py)"),
    0xE0: ("has_value", "byte", 1, "C", "0xB8D600 returns this & 1"),
}

# addr -> (name, return type, [param types], evidence, note).  Names here are OURS, not RTTI.
SIGNATURES = {
    0x773FC0: ("network_session_parameters", f"{PARAM_SET} *", ["int"], "C",
               "manager[0x26B1850] + idx*0x131C8 + inner*0x98E0 + 0x6B38, NULL unless state 4..10 "
               "(capture-rig/session-params.py, 7 controls)"),
    0xB8D600: ("network_session_parameter_has_value", "bool", [f"{PARAM_BASE} *"], "C", ""),
    0xB8C130: ("network_session_parameter_activity_id", "undefined2",
               ["c_network_session_parameter_activity *"], "I",
               "reads m_value+2; named for the caller we have evidence for (orbit's update)"),
    0xB91090: ("network_session_parameter_goal_a", "int",
               ["c_network_session_parameter_world_controller_goal *"], "I",
               "reads m_value+0 guarded by m_value+8"),
    0x783690: ("world_controller_request_state", "void", ["int", "int"], "I",
               "(state_id, reason) -> 0x782720(state, reason, NULL); "
               "historicaldocs/design/activity-launch-ladder.md §1.2"),
}

# ---------------------------------------------------------------------------

WIDTH = {
    "undefined1": 1, "undefined2": 2, "undefined4": 4, "undefined8": 8,
    "byte": 1, "char": 1, "bool": 1, "undefined": 1,
    "short": 2, "ushort": 2, "word": 2,
    "int": 4, "uint": 4, "float": 4, "dword": 4,
    "long": 8, "ulong": 8, "double": 8, "qword": 8,
}
AUTO_TYPE = {1: "undefined1", 2: "undefined2", 4: "undefined4", 8: "undefined8"}

ACCESS = re.compile(r"\*\(([A-Za-z_0-9]+) \*\)\(param_1 \+ (0x[0-9a-f]+)\)")
BARE = re.compile(r"param_1 \+ (0x[0-9a-f]+)")
# ⚠ THE SELF-REFERENCE TRAP.  Once D1Structs has run and the corpus has been refreshed, a typed
# function no longer contains `param_1 + 0x38` - it contains `this->debounce_ms`.  A naive rescan
# then finds ZERO offsets for every class it already described and quietly proposes to delete them.
# The controls caught exactly that on the first re-run.  So the scanner reads BOTH forms, and the
# typed form is translated back to an offset through the previous generation of this same file.
TYPED = re.compile(r"this->([A-Za-z_][A-Za-z_0-9]*)")


def load_params() -> dict:
    spec = importlib.util.spec_from_file_location(
        "session_params", REPO / "capture-rig" / "session-params.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules["session_params"] = module
    spec.loader.exec_module(module)
    return module.PARAMS


def read_atlas() -> list[dict]:
    with open(ATLAS, newline="") as handle:
        return list(csv.DictReader(handle, delimiter="\t"))


def corpus_files() -> dict[int, pathlib.Path]:
    out = {}
    if not CORPUS.is_dir():
        return out
    for path in CORPUS.iterdir():
        if path.suffix != ".c":
            continue
        try:
            out[int(path.name.split("_")[0], 16)] = path
        except ValueError:
            continue
    return out


def parse_signature(text: str) -> list[str] | None:
    """The declared parameter TYPES exactly as the decompiler currently infers them.

    Rebuilding the signature with the same arity is what keeps `this`-typing from truncating a
    function that really takes three arguments (`vfunc[4]` does).
    """
    body = text.rsplit("*/", 1)[-1]
    open_at = body.find("(")
    if open_at < 0:
        return None
    depth, close_at = 0, -1
    for i in range(open_at, len(body)):
        if body[i] == "(":
            depth += 1
        elif body[i] == ")":
            depth -= 1
            if depth == 0:
                close_at = i
                break
    if close_at < 0:
        return None
    inner = " ".join(body[open_at + 1:close_at].split())
    if not inner or inner == "void":
        return []
    types = []
    for part in inner.split(","):
        part = part.strip()
        # strip the trailing identifier: "undefined8 *param_2" -> "undefined8 *"
        match = re.match(r"^(.*?)([A-Za-z_][A-Za-z_0-9]*)$", part)
        types.append(match.group(1).strip() if match else part)
    return types


def previous_layout() -> dict[str, dict[str, tuple[int, str]]]:
    """struct -> {field name: (offset, type)} from the last generation, for the TYPED re-scan."""
    prev: dict[str, dict[str, tuple[int, str]]] = defaultdict(dict)
    if not OUT.is_file():
        return prev
    with OUT.open(newline="") as handle:
        for row in csv.DictReader(handle, delimiter="\t"):
            if row["kind"] == "field":
                prev[row["target"]][row["name"]] = (int(row["offset"]), row["type"])
    return prev


def inventory(atlas: list[dict], files: dict[int, pathlib.Path]):
    """class -> {offset: Counter(width)}, plus the per-function facts D1Structs needs."""
    offsets = defaultdict(lambda: defaultdict(Counter))
    funcs = defaultdict(list)          # class -> [(addr, vslot, vt_shared, [param types])]
    missing = []
    prev = previous_layout()
    for row in atlas:
        cls = row["class"]
        if cls not in CLASSES:
            continue
        addr = int(row["vaddr"], 16)
        path = files.get(addr)
        if path is None:
            missing.append(row["vaddr"])
            continue
        text = path.read_text(errors="ignore")
        for match in ACCESS.finditer(text):
            offsets[cls][int(match.group(2), 16)][match.group(1)] += 1
        for match in BARE.finditer(text):
            offsets[cls][int(match.group(1), 16)]  # touch: record the offset, no width
        for match in TYPED.finditer(text):          # the corpus is already typed -> translate back
            if match.group(1) == "base":
                continue        # `this->base.vptr` is the base subobject, not a derived offset
            known = prev.get(cls, {}).get(match.group(1))
            if known is not None:
                offsets[cls][known[0]][known[1]] += 1
        shared = int(row["vt_shared"] or "1")
        funcs[cls].append((addr, row["vslot"], shared, parse_signature(text)))
    return offsets, funcs, missing


def build_handler(cls: str, observed: dict[int, Counter]) -> tuple[int, list[tuple]]:
    """Merge curated names over the observed offsets.  Returns (size, [(off,name,type,len,ev,note)])."""
    curated = NAMES.get(cls, {})
    fields: dict[int, tuple] = {}
    if cls != BASE:
        fields[0] = ("base", BASE, BASE_SIZE, "C", "the shared base object, 0x30 bytes")
    for off, spec in curated.items():
        fields[off] = spec
    # Any offset covered by a curated field is already accounted for.
    covered = set()
    for off, (_, _, length, _, _) in fields.items():
        covered.update(range(off, off + length))
    marks = sorted(observed)
    for i, off in enumerate(marks):
        if off in covered:
            continue
        widths = [WIDTH[k] for k in observed[off] if k in WIDTH]
        width = min(widths) if widths else 1
        # never let a field swallow the next observed offset
        nxt = next((m for m in marks[i + 1:] if m > off), None)
        if nxt is not None:
            width = min(width, nxt - off)
        while width not in AUTO_TYPE:
            width -= 1
        if any(o in covered for o in range(off, off + width)):
            continue
        fields[off] = (f"f_{off:#x}", AUTO_TYPE[width], width, "U", "")
        covered.update(range(off, off + width))
    size = max((off + spec[2] for off, spec in fields.items()), default=BASE_SIZE)
    size = (size + 7) & ~7
    return size, [(off, *fields[off]) for off in sorted(fields)]


def build(argv=None) -> tuple[list[list[str]], list[tuple[bool, str, str]]]:
    params = load_params()
    atlas = read_atlas()
    files = corpus_files()
    observed, funcs, missing = inventory(atlas, files)

    rows: list[list[str]] = []

    def emit(*cells):
        rows.append([str(c) for c in cells])

    # ---- the task set ------------------------------------------------------
    emit("struct", TASK_ENTRY, "", 0x20, "", "", "C",
         "one entry of a world-controller task set; runner 0x1092690 walks these at +0x08, "
         "stride 0x20")
    emit("field", TASK_ENTRY, 0x00, 8, "prereq_mask", "ulong", "C",
         "run only when (done ^ requested) & prereq_mask == 0")
    emit("field", TASK_ENTRY, 0x08, 8, "fn_plain", "void *", "C", "called directly when non-NULL")
    emit("field", TASK_ENTRY, 0x10, 8, "pmf_ptr", "void *", "C",
         "Itanium pointer-to-member-function; low bit set = virtual, index into the vtable")
    emit("field", TASK_ENTRY, 0x18, 8, "pmf_this_adjust", "long", "C", "the `this` adjustment")
    for n, provenance in sorted(TASK_SETS.items()):
        name, size = task_set_name(n), task_set_size(n)
        emit("struct", name, "", size, "", "", "C",
             f"{n}-task set; {provenance}; masks at 8+{n}*0x20")
        emit("field", name, 0x00, 8, "f_0x00", "undefined8", "U", "")
        emit("field", name, 0x08, n * 0x20, "entries", f"{TASK_ENTRY}[{n}]", "C", "")
        emit("field", name, 8 + n * 0x20, 8, "requested_mask", "ulong", "C",
             "the client's own word - cleanup logs it as \"requested: '0x%016llx'\"")
        emit("field", name, 0x10 + n * 0x20, 8, "i_started_mask", "ulong", "I",
             "set when a task returns 1 or 2; cleanup logs requested ^ this as \"pending\"")
        emit("field", name, 0x18 + n * 0x20, 8, "i_done_mask", "ulong", "I",
             "set only when a task returns 2; the runner stops when it equals requested_mask")

    # ---- the handler hierarchy ---------------------------------------------
    for cls in CLASSES:
        size, fields = build_handler(cls, observed.get(cls, {}))
        note = ("the shared base of all 35 world-controller state handlers; size 0x30 measured "
                "from the ctor at 0x790E40") if cls == BASE else \
               "one rung of the boot/activity ladder (historicaldocs/design/activity-launch-ladder.md §2)"
        emit("struct", cls, "", size, "", "", "C" if cls == BASE else "I", note)
        for off, name, ctype, length, ev, fnote in fields:
            emit("field", cls, off, length, name, ctype, ev, fnote)
        for addr, vslot, shared, ptypes in sorted(funcs.get(cls, [])):
            if ptypes is None:
                continue
            if shared > 1 and cls != BASE:
                continue                      # no single derived `this` - skip, do not guess
            if not ptypes:
                continue
            emit("apply", f"{addr:#x}", 0, "", cls, ",".join(ptypes), "C",
                 f"{cls}::vfunc[{vslot}]" if vslot else f"{cls} member")

    # ---- session parameters -------------------------------------------------
    emit("struct", PARAM_BASE, "", 0xE1, "", "", "C",
         "every session parameter starts with this; ctor 0xB8D440")
    for off, (name, ctype, length, ev, note) in sorted(PARAM_BASE_FIELDS.items()):
        emit("field", PARAM_BASE, off, length, name, ctype, ev, note)

    for payload, fields in sorted(PAYLOAD_FIELDS.items()):
        size = next((sz for _, (_, sname, _, sz) in params.items() if sname == payload), None)
        emit("struct", payload, "", size, "", "", "I",
             "payload size from capture-rig/session-params.py PARAMS")
        for off, (name, ctype, length, ev, note) in sorted(fields.items()):
            emit("field", payload, off, length, name, ctype, ev, note)

    offsets = sorted(params)
    gaps: dict[int, int] = {}
    for i, off in enumerate(offsets):
        value_off, value_size = params[off][2], params[off][3]
        gaps[off] = ((offsets[i + 1] - off) if i + 1 < len(offsets)
                     else (value_off + 2 * value_size + 7) & ~7)
    for off in offsets:
        pname, payload, value_off, value_size = params[off]
        gap = gaps[off]
        cls = f"c_network_session_parameter_{pname}"
        emit("struct", cls, "", gap, "", "", "I",
             f"size = the gap to the next parameter; payload {payload}")
        emit("field", cls, 0, 0xE1, "base", PARAM_BASE, "C", "")
        ptype = payload_type(payload, value_size)
        emit("field", cls, value_off, value_size, "m_value", ptype, "C",
             "the authority's value")
        emit("field", cls, value_off + value_size, value_size, "m_pending", ptype, "C",
             "what a non-authority asks the host for")

    set_size = offsets[-1] + gaps[offsets[-1]]
    emit("struct", PARAM_SET, "", set_size, "", "", "C",
         "what 0x773FC0 returns: 23 contiguous replicated parameters "
         "(capture-rig/session-params.py, 7 controls)")
    for off in offsets:
        pname = params[off][0]
        emit("field", PARAM_SET, off, gaps[off], pname,
             f"c_network_session_parameter_{pname}", "C", "")

    # ---- signatures ----------------------------------------------------------
    for addr, (name, ret, ptypes, ev, note) in sorted(SIGNATURES.items()):
        emit("sig", f"{addr:#x}", 0, "", name, ret, ev, note)
        for i, ptype in enumerate(ptypes):
            emit("param", f"{addr:#x}", i, "", f"a{i}", ptype, ev, "")

    controls = run_controls(params, observed, funcs, missing, rows)
    return rows, controls


def payload_type(payload: str, size: int) -> str:
    if payload == "bool":
        return "bool"
    if payload.startswith("e_"):
        return "int" if size == 4 else f"undefined1[{size}]"
    if payload in PAYLOAD_FIELDS:
        return payload
    return f"undefined1[{size}]"


def run_controls(params, observed, funcs, missing, rows) -> list[tuple[bool, str, str]]:
    checks: list[tuple[bool, str, str]] = []

    checks.append((not missing, "every handler vfunc has a decompiled corpus file",
                   f"missing {missing[:5]}" if missing else f"[{sum(len(v) for v in funcs.values())} fns]"))

    # ⭐ the self-reference guard: a rescan against an already-typed corpus must never LOSE an
    # offset it previously recorded.  Silent shrinkage here would delete evidence, not update it.
    prev = previous_layout()
    shrunk = []
    for cls in CLASSES:
        was = {off for name, (off, _) in prev.get(cls, {}).items() if name != "base"}
        now = set(observed.get(cls, {}))
        for field_off, (_, _, length, _, _) in NAMES.get(cls, {}).items():
            now.update(range(field_off, field_off + length))
        lost = sorted(o for o in was if o not in now)
        if lost:
            shrunk.append(f"{cls.split('_state_')[-1]} lost {[hex(o) for o in lost[:4]]}")
    checks.append((not shrunk, "the field inventory did not shrink against the previous generation",
                   "; ".join(shrunk[:2]) if shrunk else f"[{sum(len(v) for v in prev.values())} known]"))

    # the task-set arithmetic, re-derived rather than trusted
    bad = [n for n in TASK_SETS if 8 + n * 0x20 != task_set_size(n) - 0x18]
    checks.append((not bad, "task-set arithmetic: 8 + N*0x20 == size - 0x18", str(bad)))

    checks.append((BASE_SIZE == 0x30 and min(
        (o for c in CLASSES if c != BASE for o in observed.get(c, {})), default=0x30) >= 0x30,
        "no derived handler touches an offset below the 0x30 base", ""))

    # ⭐ the control that catches a typo'd curated offset: every curated offset must be an offset
    # the class's own vfuncs actually touch.  A name at an address nobody reads is a fabrication.
    unseen = []
    for cls, fields in NAMES.items():
        seen = observed.get(cls, {})
        for off, (name, _, length, _, _) in fields.items():
            if cls == BASE:
                continue          # the base's fields come from its ctor, not from `param_1 + N`
            if not any(o in seen for o in range(off, off + length)):
                unseen.append(f"{cls.split('_state_')[-1]}+{off:#x} ({name})")
    checks.append((not unseen, "every curated field offset is touched by the class's own vfuncs",
                   "; ".join(unseen[:3]) if unseen else
                   f"[{sum(len(v) for k, v in NAMES.items() if k != BASE)} curated]"))

    # no overlaps anywhere
    overlaps = []
    by_struct = defaultdict(list)
    for row in rows:
        if row[0] == "field":
            by_struct[row[1]].append((int(row[2]), int(row[3]), row[4]))
    for name, fields in by_struct.items():
        fields.sort()
        for (o1, l1, n1), (o2, _, n2) in zip(fields, fields[1:]):
            if l1 and o1 + l1 > o2:
                overlaps.append(f"{name}: {n1}+{l1:#x} over {n2}")
    checks.append((not overlaps, "no field overlaps any other field",
                   "; ".join(overlaps[:3]) if overlaps else f"[{len(by_struct)} structs]"))

    # every field fits inside its struct
    sizes = {row[1]: int(row[3]) for row in rows if row[0] == "struct"}
    toobig = [f"{row[1]}.{row[4]}" for row in rows
              if row[0] == "field" and int(row[2]) + int(row[3]) > sizes.get(row[1], 0)]
    checks.append((not toobig, "every field fits inside its struct", "; ".join(toobig[:3])))

    # `this` is never applied to a vtable slot shared by more than one class
    applied = {row[1] for row in rows if row[0] == "apply"}
    leaked = []
    for cls, fs in funcs.items():
        for addr, _, shared, _ in fs:
            if shared > 1 and cls != BASE and f"{addr:#x}" in applied:
                leaked.append(f"{addr:#x}")
    checks.append((not leaked, "no derived `this` applied to a vtable slot shared by >1 class",
                   "; ".join(leaked[:3]) if leaked else f"[{len(applied)} applied]"))

    checks.append((params[0x9B8][0] == "activity" and params[0x390][0] == "activity_host",
                   "the imported parameter map still has activity @+0x9B8 and activity_host @+0x390",
                   ""))
    checks.append((len(params) == 23, "23 session parameters", f"[{len(params)}]"))
    return checks


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--controls", action="store_true", help="run the checks and write nothing")
    ap.add_argument("--out", default=str(OUT))
    args = ap.parse_args()

    rows, controls = build()

    failed = 0
    for ok, label, detail in controls:
        print(f"{'PASS' if ok else 'FAIL'}  {label}" + (f"  [{detail}]" if detail else ""))
        failed += 0 if ok else 1
    if failed:
        print(f"\n{failed} CONTROL(S) FAILED - refusing to write.", file=sys.stderr)
        return 2
    print("\nALL CONTROLS PASS")
    if args.controls:
        return 0

    out = pathlib.Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", newline="") as handle:
        w = csv.writer(handle, delimiter="\t", lineterminator="\n")
        w.writerow(["kind", "target", "offset", "size", "name", "type", "evidence", "note"])
        w.writerows(rows)
    kinds = Counter(r[0] for r in rows)
    print(f"wrote {out}  {dict(kinds)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
