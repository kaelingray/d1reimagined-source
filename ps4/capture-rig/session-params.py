#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Read (and optionally poke) the client's **network session parameters** - the activity-launch seam.

WHY THIS EXISTS
---------------
`c_world_controller_state_setup_orbit_handler::vfunc[7]` (`0x7AAF60`, orbit's per-frame update)
leaves rung 27 for rung 28 `setup:activity_session_creation` when a **session parameter** is
populated. Not on account state, not on an unlock, not through the director:

    lVar6 = FUN_00773fc0(0) + 0x9b8;                 // c_network_session_parameter_activity
    if (!FUN_00b8d600(lVar6)) goto stay_in_orbit;    // has_value == 0
    if (FUN_00b8c130(lVar6) == -1) goto stay_in_orbit;
    ...
    uVar8 = 0x1c;                                    // ⭐ request rung 28

So the whole activity launch hangs off a replicated-property block, and this reads it.

THE RUNTIME PATH  (`0x773FC0`, disassembled - it is NOT a `55 48 89 e5` prologue, use --force)
----------------------------------------------------------------------------------------------
    manager  = [0x26B1850]                 ⚠ an EBOOT BSS vaddr -> d1guest adds the base
    if manager == 0 or byte[manager] == 0:  no session
    slot     = manager + idx*0x131C8       ⚠ everything from here is a HEAP address -> --abs
    inner    = (s32)[slot + 8] * 0x98E0
    state    = u32[inner + slot + 0x8C58]  must be 4..10  (`bt 0x7F0, state`)
    PARAMS   = inner + slot + 0x6B38       <- what FUN_00773FC0 returns

⚠⚠ **All offsets below are relative to the pointer the accessor RETURNS**, because that is what the
client's own code indexes (`FUN_00773fc0(0) + 0x9b8`). A sibling derivation puts the parameter set
at `session + 0x6B28`; the 0x10 difference is the inner-array base. Do not mix the two bases.

THE UNIVERSAL PARAMETER SHAPE (`c_network_session_parameter_base`, `0xB8D440`, 0xE1 bytes)
------------------------------------------------------------------------------------------
    +0x00  vptr
    +0x38..+0xE0   0xA8 bytes of 0xFF - the per-peer version/ack vector
    +0xE0  u8 has_value      <- FUN_00B8D600 returns this & 1
    then, aligned to alignof(T):  T m_value, then T m_pending

⇒ every parameter's size is `align(0xE1) + 2*sizeof(T)`. Authority writes `m_value`; a non-authority
writes `m_pending` and asks the host. **This is a replicated-property system and the server is the
authority** - which is exactly why populating it locally is a poke and never a fix.

THE KTOBUCKET READER (`--ktobucket`) - the INPUT side of the same seam
----------------------------------------------------------------------
On every activity-transition failure `c_world_controller_state_manager` dumps a **KTOBUCKET**
block: eleven `printf` lines that state the exact launch parameters the client believes it has.
That block is not log noise, it is the levers. `--ktobucket <run>` parses it per attempt.

**What each field is** - the `ACTIVITY:` line's six arguments are read straight off the activity
client's embedded launch-request struct (`c_activity_client + 0x138`, from `FUN_00782720`'s own
call site; `FUN_002990f0` is literally `lea rax,[rdi+0x138]; ret`), so each printed field has a
pokeable address: **activity** `s16 [req+0x0E]` is the activity-definition index (393-413 are the
ids that bypass matchmaking); **insp** `s8 [req+0x10]` is the *insertion point* index - which
spawn/phase inside the activity to enter at, and `-1` means "none chosen"; **override activity**
`char[] [req+0x98]` names an activity definition that replaces the one the id selects (empty =
none); **bubbles** `char[] [req+0x198]` names the bubbles (map sub-regions) to load (empty =
none); **reservations** `u32 [req+0x698]` is the player-slot reservation count; **nonce**
`u32 [req+0x04]` is the per-attempt launch nonce and is the only field that changes between
retries. Alongside them the `ACTIVITY ACTNAME:[%s] SLICESET HASH:[0x%08x]` line reports the
*resolved* content: the activity's name and the **slice-set hash**, i.e. which set of content
slices the world transition will load. ⭐ **The levers we currently leave at sentinel are
`insp = -1`, `override activity = ""`, `bubbles = ""`, `ACTNAME = NONE` and
`SLICESET = 0x811C9DC5`** - that last one is the **FNV-1 32-bit offset basis**, i.e. the hash of
the empty string, this codebase's "no slice set" value. Every run in the tree that actually
loaded a world reported a real slice-set hash (`0x3559729A`, `0x8265820B`) with `ACTNAME=orbit`;
every failed activity attempt reported the basis with `ACTNAME=NONE`. `[C]` observed across all
runs, `[I]` as to causation - a sentinel slice set is *consistent with* "Transition completed,
but we don't have an active activity", not yet proof of it.

USAGE
-----
    python3 -B capture-rig/session-params.py                  # both sessions, all parameters
    python3 -B capture-rig/session-params.py --session 0 --param activity --hex
    python3 -B capture-rig/session-params.py --controls       # offline; static + KTOBUCKET controls
    python3 -B capture-rig/session-params.py --ktobucket runs/<run>      # offline; log reader
    python3 -B capture-rig/session-params.py --ktobucket latest --json

⚠⚠ A poke here is `synthetic_ancestor` twice over: it is not evidence about a real service and not
even about our server, because nothing crossed a wire. A poke can IDENTIFY a gate; it cannot SATISFY
one. Report the crash, if any, in the same breath as the advance.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import pathlib
import re
import struct
import sys

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent

MANAGER_PTR_VADDR = 0x26B1850     # eboot BSS - the ONLY address here that takes the eboot base
SESSION_STRIDE = 0x131C8
INNER_STRIDE = 0x98E0
STATE_OFFSET = 0x8C58
PARAMS_OFFSET = 0x6B38
VALID_STATE_MASK = 0x7F0          # `bt esi, ecx` with esi = 0x7F0 -> states 4..10

HAS_VALUE = 0xE0                  # u8, in every parameter

SESSION_NAMES = {0: "fireteam", 1: "activity/posse"}

# offset -> (name, struct, value_offset, value_size). Value offsets are align(0xE1, alignof(T)).
PARAMS: dict[int, tuple[str, str, int, int]] = {
    0x0000: ("world_controller_goal", "s_network_session_world_controller_goal", 0xE4, 0xC),
    0x0100: ("active_join_controls", "s_network_session_active_join_controls", 0xE2, 0x6),
    0x01F0: ("atomic_cascade_join_data", "s_network_session_atomic_cascade_join_data", 0xE8, 0x5C),
    0x0390: ("activity_host", "s_activity_host_connection_information", 0xE8, 0x20),
    0x04B8: ("orbit", "s_network_session_parameter_orbit", 0xE4, 0x4),
    0x05A8: ("desired_activity", "s_desired_activity_information", 0xE4, 0x9C),
    0x07C8: ("desired_roster_join_activity", "s_desired_roster_join_activity", 0xE4, 0x4),
    0x08B8: ("activity_start_status", "s_activity_start_status", 0xE4, 0xC),
    0x09B8: ("activity", "s_activity_information", 0xE8, 0xA8),
    0x0BF0: ("user_join_controls", "s_network_session_user_join_controls", 0xE2, 0x2),
    # sizeof derived from the gap, not from the setter: 0xF0 - 0xE4 = 0xC, /2 = 6. An upstream
    # reading guessed 0x8, which the "fits before the next parameter" control rejected. `[I]`
    0x0CD8: ("desired_join_controls", "s_network_session_desired_join_controls", 0xE4, 0x6),
    0x0DC8: ("language", "e_language", 0xE4, 0x4),
    0x0EB8: ("countdown_timer", "s_network_session_parameter_countdown_timer", 0xE8, 0x1C),
    0x0FD8: ("remote_session_join_data", "s_network_session_remote_session_join_data", 0xE8, 0x78),
    0x11B0: ("remote_session_join_data_2", "s_network_session_remote_session_join_data", 0xE8, 0x78),
    0x1388: ("remote_session_join_result", "s_network_session_remote_session_join_result", 0xE8, 0x10),
    0x1490: ("bool_1", "bool", 0xE1, 0x1),
    0x1578: ("session_messaging", "s_network_session_parameter_session_messaging", 0xE4, 0xC),
    0x1678: ("bool_2", "bool", 0xE1, 0x1),
    0x1760: ("life_cycle_disbanding_reason", "e_life_cycle_disbanding_reason", 0xE4, 0x4),
    0x1850: ("matchmaking_progress", "s_replicated_activity_matchmaking_progress", 0xE8, 0x10),
    0x1958: ("initial_slice_set_status", "s_network_session_parameter_initial_slice_set_status", 0xE4, 0xC),
    0x1A58: ("live_quality", "s_network_quality_live_for_all_peers", 0xE8, 0x20),
}

STATES = [
    "pregame", "bootflow:media_check", "bootflow:bootload", "bootflow:intro_screen_display",
    "bootflow:start", "bootflow:platform_signin", "bootflow:platform_account_warnings",
    "bootflow:enumerate_content_rights", "bootflow:account_signin", "bootflow:eula",
    "bootflow:upsell", "bootflow:email", "bootflow:confirm_account_transfer",
    "bootflow:content_check", "bootflow:content_install", "bootflow:insufficient_content_space",
    "bootflow:illegal_dlc_media", "bootflow:queuing", "bootflow:package_registration",
    "bootflow:bap_signin", "bootflow:investment_signin", "bootflow:profile_setup",
    "bootflow:prepare_for_orbit", "character:signin", "character:prologue", "cleanup",
    "setup:orbit_loading", "setup:orbit", "setup:activity_session_creation", "setup:matchmaking",
    "setup:activity_host_setup", "setup:prologue_intro_loading", "setup:orbit_outro",
    "setup:activity_world_transition", "activity:initial_slice_set_loading",
    "activity:physics_join", "activity:in_world", "activity:watch_video",
]


def load_d1guest():
    spec = importlib.util.spec_from_file_location("d1guest", HERE / "d1guest.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules["d1guest"] = module
    spec.loader.exec_module(module)
    return module


def open_live(d1guest, require: int = 2):
    args = argparse.Namespace(
        symbols=d1guest.SYMBOLS, eboot=None, ignore_build_gate=False, full_scan=False,
        no_cache=False, allow_dead=False, account_soid=None, require=require,
    )
    syms, image, proc, guest = d1guest.open_guest(args)
    d1guest.print_liveness(guest, args, require=require)
    return proc, guest


def resolve(proc, guest, index: int) -> dict:
    """Reproduce 0x773FC0 exactly. Returns {} when the accessor itself would return NULL."""
    manager = guest.u64(MANAGER_PTR_VADDR)          # eboot vaddr -> the guest reader adds the base
    if manager == 0:
        return {"error": f"manager [{MANAGER_PTR_VADDR:#x}] is NULL - no session manager yet"}
    # ⚠ EVERYTHING BELOW IS ABSOLUTE. `manager` came OUT of memory, so it is already a real address.
    if proc.read(manager, 1)[0] == 0:
        return {"error": "byte[manager] == 0 - the manager is present but not valid"}
    slot = manager + index * SESSION_STRIDE
    inner = struct.unpack("<i", proc.read(slot + 8, 4))[0] * INNER_STRIDE
    state = struct.unpack("<I", proc.read(inner + slot + STATE_OFFSET, 4))[0]
    out = {"manager": manager, "slot": slot, "inner": inner, "state": state,
           "params": inner + slot + PARAMS_OFFSET}
    if state > 0xA or not (VALID_STATE_MASK >> state) & 1:
        out["error"] = (f"session state {state} is outside 4..10, so 0x773FC0 would return NULL "
                        f"(mask {VALID_STATE_MASK:#x})")
    return out


def dump(proc, params_base: int, only: str | None, show_hex: bool) -> None:
    print(f"\n  {'off':>7} {'parameter':32} {'has':>4}  value")
    for offset in sorted(PARAMS):
        name, struct_name, value_off, value_size = PARAMS[offset]
        if only and only not in name:
            continue
        base = params_base + offset
        try:
            has = proc.read(base + HAS_VALUE, 1)[0] & 1
            value = proc.read(base + value_off, value_size)
        except Exception as error:                       # noqa: BLE001 - surface it, do not mask
            print(f"  {offset:#07x} {name:32}  ??   <unreadable: {error}>")
            continue
        mark = "⭐" if has else "  "
        shown = value[:16].hex()
        if len(value) > 16:
            shown += "…"
        print(f"  {offset:#07x} {name:32} {mark}{has}  {shown}")
        if show_hex and has:
            print(f"          {struct_name}  @{base + value_off:#x}  {value.hex()}")


ORBIT_HANDLER_VTABLE = 0x8019_2F410      # c_world_controller_state_setup_orbit_handler, +base
ORBIT_DEBOUNCE_OFFSET = 0x38             # the -1 timestamp that actually holds orbit shut
GOAL_VALUE = 0xE4                        # goal_a; 0xB91090 reads [param+0xE4]
ACTIVITY_ID = 0xEA                       # 0xB8C130 reads word[param+0xEA]
RUNG_ORBIT, RUNG_IN_WORLD = 0x1B, 0x24


def launch_poke(proc, guest, params_base: int, activity_id: int, dry_run: bool) -> int:
    """The three writes that move a live client from rung 27 to rung 28, re-derived each run.

    ⚠⚠ `synthetic_ancestor` ×2. Nothing crosses a wire. This IDENTIFIES the gate; it cannot satisfy
    it. Full write-up and the negative control that justifies each write:
    `historicaldocs/design/RUNG-28-REACHED.md`. Heap addresses move every run, which is the whole reason this is a
    mode and not a copied hex constant.
    """
    import subprocess

    activity_param = params_base + 0x9B8
    goal_param = params_base + 0x0

    # The orbit handler object is found by its vtable, not assumed - it lives in eboot BSS but its
    # exact slot is not pinned anywhere we trust.
    finder = subprocess.run(
        [sys.executable, "-B", str(HERE / "find-in-guest.py"), hex(ORBIT_HANDLER_VTABLE),
         "--context", "0", "--max", "4"], capture_output=True, text=True)
    hits = [int(line.split()[0], 16) for line in finder.stdout.splitlines()
            if line.strip().startswith("0x") and "[LE]" in line]
    if len(hits) != 1:
        print(f"REFUSING: expected exactly 1 hit for the orbit handler vtable "
              f"{ORBIT_HANDLER_VTABLE:#x}, got {len(hits)}: {[hex(h) for h in hits]}", file=sys.stderr)
        return 2
    handler = hits[0]

    writes = [
        (activity_param + ACTIVITY_ID, activity_id, "u16", "activity id (0xFFFF = unset)"),
        (goal_param + GOAL_VALUE, RUNG_IN_WORLD, "u32", "goal_a (0x1B = stay in orbit)"),
        (handler + ORBIT_DEBOUNCE_OFFSET, 0, "u64", "⭐ orbit debounce stamp (-1 = exit every frame)"),
    ]
    print(f"\n  orbit handler {handler:#x}   params {params_base:#x}")
    for addr, value, kind, why in writes:
        print(f"  {addr:#x}  <- {value:#x} ({kind})   {why}")
    if dry_run:
        print("\n  --dry-run: nothing written.")
        return 0

    for addr, value, kind, _ in writes:
        result = subprocess.run(
            [sys.executable, "-B", str(HERE / "d1guest.py"), "poke", hex(addr), str(value),
             "--type", kind, "--abs", "--experiment", "orbit-to-rung28", "--confirm"],
            capture_output=True, text=True)
        ok = "OK" in result.stdout
        before = next((l.strip() for l in result.stdout.splitlines() if l.strip().startswith("before")), "")
        print(f"  {addr:#x}  {'OK ' if ok else 'FAILED'}  {before}")
        if not ok:
            print(result.stdout[-600:], file=sys.stderr)
            return 2
    print("\n  three writes done. Watch for: Entering state 'setup:activity_session_creation'")
    return 0


# ─────────────────────────────────────────────────────────────────────────────────────────────────
# KTOBUCKET - the client's own launch-parameter dump, parsed
#
# ⭐ THE REGEXES ARE NOT WRITTEN BY HAND. Each one is MECHANICALLY DERIVED from the client's own
# `printf` format string (pinned below, verbatim, with the eboot offset it was read from). That is
# the axis the controls protect: a parser built from a shape I invented would still parse the log
# lines I happened to look at, and would silently mis-file the ones I did not.
# ─────────────────────────────────────────────────────────────────────────────────────────────────

EBOOT = REPO / "dumps" / "patch-01.29-CUSA00219" / "eboot.bin"

KTO_MARK = "world_controller:state_manager: KTOBUCKET: "
ANSI = re.compile(r"\x1b\[[0-9;]*m")
SHADPS4_DUP = re.compile(r"Skipped \d+ duplicate messages")

# FNV-1 32-bit offset basis == the hash of the empty string == this codebase's "no slice set".
SLICESET_NONE = 0x811C9DC5

# kind -> (eboot offset of the string, format string EXACTLY as it appears in the eboot, field names)
KTO_FORMATS: dict[str, tuple[int, str, tuple[str, ...]]] = {
    "state_change_reason": (
        0x164BF69,
        "world_controller:state_manager: KTOBUCKET: STATE_CHANGE_REASON [%s], Current State [%s], "
        "Next State [%s], Context [%s]",
        ("reason", "current_state", "next_state", "context"),
    ),
    "state_status": (
        0x164BFE0,
        "world_controller:state_manager: KTOBUCKET: State status [%lld], tasks remaining: "
        "[0x%016llx], tasks requested: [0x%016llx]",
        ("status", "tasks_remaining", "tasks_requested"),
    ),
    "datamine_unavailable": (
        0x164C05B,
        "world_controller:state_manager: KTOBUCKET: Unable to log KTO reason to retail datamine",
        (),
    ),
    "bap_desc": (
        0x164C24F,
        "world_controller:state_manager: KTOBUCKET: BAP DESC: %s IP: %s",
        ("desc", "ip"),
    ),
    "socket_info": (
        0x164C28E,
        "world_controller:state_manager: KTOBUCKET: SOCKET INFO: %s",
        ("info",),
    ),
    "send_recv_info": (
        0x164C2C9,
        "world_controller:state_manager: KTOBUCKET: SEND/RECV INFO: %s",
        ("info",),
    ),
    "actname": (
        0x164C307,
        "world_controller:state_manager: KTOBUCKET: ACTIVITY ACTNAME:[%s] SLICESET HASH:[0x%08x]",
        ("actname", "sliceset"),
    ),
    "last_bap_log": (
        0x164C35F,
        "world_controller:state_manager: KTOBUCKET: LAST BAP LOG NOTIFICATION: %s",
        ("notification",),
    ),
    "activity_status": (
        0x164C3A8,
        "world_controller:state_manager: KTOBUCKET: ACTIVITY %s STATUS: [%s] SID: %s",
        ("flags", "status", "sid"),
    ),
    "activity": (
        0x164C3F4,
        "world_controller:state_manager: KTOBUCKET: ACTIVITY: activity=%d, insp=%d, "
        "override activity=%s, bubbles=%s, reservations=%d, nonce=%d",
        ("activity", "insp", "override_activity", "bubbles", "reservations", "nonce"),
    ),
    "session": (
        0x164C47B,
        "world_controller:state_manager: KTOBUCKET: SESSION ID:=%s, HOST=%s, LEADER=%s, MEM=%s, "
        "PEERS=%d, ESTPEERS=%d",
        ("session_id", "host", "leader", "mem", "peers", "estpeers"),
    ),
}

# ⚠ DEFECT INJECTORS - used ONLY by --controls-invert, which proves the controls FAIL when the
# parser is broken on purpose. A control that cannot fail is decoration (LESSONS #7, #12).
# ⭐ Each one models the SPECIFIC realistic bug its control exists to catch. The first draft of this
# block used "drop the \A..\Z anchors" for the no-coercion axis; the inversion refused it, because
# the required trailing literals - not the anchors - are what reject a truncated line. The anchors
# turn out to protect a DIFFERENT axis (round-trip fidelity), and are now inverted against that one.
KTO_DEFECT_UNANCHORED = False       # drop \A..\Z -> non-greedy %s can match empty; round-trip breaks
KTO_DEFECT_HARDCODE: int | None = None   # pretend `activity` is a constant, ignoring the text
KTO_DEFECT_SCRAPE = False           # the naive parser: `re.search("activity=(\\w+)")` per field,
                                    # with a default when a field is missing or non-numeric
KTO_DEFECT_IGNORE_DEDUP = False     # treat shadPS4's `Skipped N duplicate messages` as noise

CONVERSION = re.compile(r"%(0\d+)?(?:ll)?([dsx])")


def _kto_compile(fmt: str) -> tuple[re.Pattern, list[str], str]:
    """Turn a C format string into (regex, per-group coercion kinds, literal prefix).

    Only the conversions become capture groups; every other byte is escaped literally. This is
    what makes the shape the CLIENT'S and not mine.
    """
    parts, kinds, pos, prefix = [], [], 0, None
    for match in CONVERSION.finditer(fmt):
        literal = fmt[pos:match.start()]
        if prefix is None:
            prefix = literal
        parts.append(re.escape(literal))
        width, conv = match.group(1), match.group(2)
        if conv == "s":
            parts.append("(.*?)")
            kinds.append("s")
        elif conv == "d":
            parts.append(r"(-?\d+)")
            kinds.append("d")
        else:                                   # x
            n = int(width) if width else 0
            parts.append(f"([0-9a-fA-F]{{{n}}})" if n else "([0-9a-fA-F]+)")
            kinds.append("x")
        pos = match.end()
    parts.append(re.escape(fmt[pos:]))
    if prefix is None:
        prefix = fmt
    body = "".join(parts)
    pattern = body if KTO_DEFECT_UNANCHORED else r"\A" + body + r"[ \t]*\Z"
    return re.compile(pattern), kinds, prefix


_KTO_CACHE: dict[bool, dict] = {}


def _kto_patterns() -> dict[str, tuple[re.Pattern, list[str], str, tuple[str, ...]]]:
    key = KTO_DEFECT_UNANCHORED
    if key not in _KTO_CACHE:
        table = {}
        for kind, (_off, fmt, fields) in KTO_FORMATS.items():
            regex, kinds, prefix = _kto_compile(fmt)
            table[kind] = (regex, kinds, prefix, fields)
        _KTO_CACHE[key] = table
    return _KTO_CACHE[key]


def kto_body(raw: str) -> str | None:
    """Strip shadPS4's ANSI wrapper and its `[stdout] ` prefix; return the client's own line."""
    line = ANSI.sub("", raw.rstrip("\r\n"))
    index = line.find(KTO_MARK)
    return line[index:] if index >= 0 else None


def kto_parse_line(body: str) -> tuple[str | None, dict | None, str | None]:
    """(kind, fields, error). A line that does not FULLY match its format is FLAGGED, never coerced.

    Returning partial values here is the failure mode this whole tool exists to avoid: a truncated
    `ACTIVITY:` line that yielded `activity=393` and silence about the rest would read exactly like
    a healthy dump.
    """
    if KTO_DEFECT_SCRAPE:
        prefix = _kto_patterns()["activity"][2]
        if body.startswith(prefix):
            scraped: dict = {}
            for name, key, numeric in (("activity", "activity", True), ("insp", "insp", True),
                                       ("override_activity", "override activity", False),
                                       ("bubbles", "bubbles", False),
                                       ("reservations", "reservations", True),
                                       ("nonce", "nonce", True)):
                found = re.search(re.escape(key) + r"=([^,]*)", body)
                text = found.group(1) if found else ""
                if numeric:
                    try:
                        scraped[name] = int(text)
                    except ValueError:
                        scraped[name] = 0            # ← the coercion this control forbids
                else:
                    scraped[name] = text
            return "activity", scraped, None

    hits = []
    for kind, (regex, kinds, _prefix, fields) in _kto_patterns().items():
        match = regex.match(body) if KTO_DEFECT_UNANCHORED else regex.fullmatch(body)
        if not match:
            continue
        values: dict = {}
        for name, kind_char, text in zip(fields, kinds, match.groups()):
            if kind_char == "d":
                values[name] = int(text, 10)
            elif kind_char == "x":
                values[name] = int(text, 16)
            else:
                values[name] = text
        if KTO_DEFECT_HARDCODE is not None and "activity" in values:
            values["activity"] = KTO_DEFECT_HARDCODE
        hits.append((kind, values))
    if len(hits) == 1:
        return hits[0][0], hits[0][1], None
    if len(hits) > 1:
        return None, None, f"AMBIGUOUS: matches {sorted(k for k, _ in hits)}"
    # No full match. Attribute it to the kind whose literal prefix it shares, so the operator sees
    # *which* line went wrong rather than a generic "unparsed".
    best, best_len = None, 0
    for kind, (_regex, _kinds, prefix, _fields) in _kto_patterns().items():
        if body.startswith(prefix) and len(prefix) > best_len:
            best, best_len = kind, len(prefix)
    return None, None, f"MALFORMED {best}" if best else "UNKNOWN KTOBUCKET line"


def _new_attempt(index: int, line_no: int) -> dict:
    return {"attempt": index, "line": line_no, "reason": None, "current_state": None,
            "next_state": None, "context": None, "state_status": None, "activity": None,
            "actname": None, "statuses": [], "sessions": [], "bap": [], "last_bap_log": None,
            "malformed": [], "eaten": [], "states_walked": []}


def kto_scan(path: pathlib.Path) -> list[dict]:
    """Group a run's KTOBUCKET lines into one record per activity-transition attempt.

    An attempt begins at each STATE_CHANGE_REASON line - that is the first line the client emits in
    the block, confirmed across every run in `runs/` (208 dumps, one STATE_CHANGE_REASON and one
    `State status` apiece).
    """
    attempts: list[dict] = []
    current = None
    pending_states: list[str] = []
    previous_kto: str | None = None            # the last line, IF it was a KTOBUCKET line
    entering = re.compile(r"Entering state '([^']+)' for reason '([^']+)'")
    with path.open(errors="replace") as handle:
        for line_no, raw in enumerate(handle, 1):
            if "KTOBUCKET" not in raw:
                if "Entering state" in raw:
                    match = entering.search(raw)
                    if match:
                        pending_states.append(f"{match.group(1)}({match.group(2)})")
                elif (previous_kto is not None and current is not None
                      and not KTO_DEFECT_IGNORE_DEDUP and SHADPS4_DUP.search(raw)):
                    # ⭐ shadPS4 collapses a line IDENTICAL TO THE ONE IMMEDIATELY BEFORE IT. So a
                    # `Skipped N duplicate messages` here means N copies of `previous_kto` were
                    # DROPPED FROM THE LOG. This is not hypothetical: run 20260809-084932 loses a
                    # second `ACTIVITY NONE STATUS` line this way, in every attempt.
                    count = re.search(r"Skipped (\d+)", raw)
                    current["eaten"].append({"line": line_no,
                                             "count": int(count.group(1)) if count else 1,
                                             "was": previous_kto[len(KTO_MARK):]})
                previous_kto = None
                continue
            body = kto_body(raw)
            if body is None:
                previous_kto = None
                continue
            previous_kto = body
            kind, values, error = kto_parse_line(body)
            if kind == "state_change_reason" or current is None:
                current = _new_attempt(len(attempts) + 1, line_no)
                current["states_walked"] = pending_states
                pending_states = []
                attempts.append(current)
            if error:
                current["malformed"].append({"line": line_no, "why": error, "body": body})
                continue
            if kind == "state_change_reason":
                current.update(values)
            elif kind == "state_status":
                current["state_status"] = values
            elif kind == "activity":
                current["activity"] = values
            elif kind == "actname":
                current["actname"] = values
            elif kind == "activity_status":
                current["statuses"].append(values)
            elif kind == "session":
                current["sessions"].append(values)
            elif kind in ("bap_desc", "socket_info", "send_recv_info"):
                current["bap"].append({"kind": kind, **values})
            elif kind == "last_bap_log":
                current["last_bap_log"] = values["notification"]
    return attempts


def kto_levers(attempt: dict) -> list[tuple[str, str, bool]]:
    """(label, rendered value, at_sentinel). The sentinel set is DEFINED here, not eyeballed."""
    act, name = attempt.get("activity"), attempt.get("actname")
    out: list[tuple[str, str, bool]] = []
    if act:
        out.append(("activity", str(act["activity"]), act["activity"] in (-1, 0xFFFF)))
        out.append(("insp", str(act["insp"]), act["insp"] == -1))
        out.append(("override", act["override_activity"] or "⟨empty⟩", act["override_activity"] == ""))
        out.append(("bubbles", act["bubbles"] or "⟨empty⟩", act["bubbles"] == ""))
        out.append(("resv", str(act["reservations"]), act["reservations"] == 0))
        out.append(("nonce", str(act["nonce"]), False))
    if name:
        out.append(("actname", name["actname"], name["actname"] == "NONE"))
        out.append(("sliceset", f"0x{name['sliceset']:08x}", name["sliceset"] == SLICESET_NONE))
    return out


def kto_report(attempts: list[dict], brief: bool) -> None:
    if not attempts:
        print("  no KTOBUCKET dumps in this log - the client never failed a state transition, or "
              "the log predates the dump.")
        return
    columns = ["activity", "insp", "override", "bubbles", "resv", "nonce", "actname", "sliceset"]
    widths = {c: max(len(c), 8) for c in columns}
    rows = []
    for attempt in attempts:
        levers = dict((label, (value, sentinel)) for label, value, sentinel in kto_levers(attempt))
        cells = {}
        for column in columns:
            value, sentinel = levers.get(column, ("-", False))
            cells[column] = value + ("⚠" if sentinel else "")
            widths[column] = max(widths[column], len(cells[column]))
        rows.append((attempt, cells))

    header = "  #   line      " + " ".join(f"{c:<{widths[c]}}" for c in columns) + "  reason"
    print(f"\n{header}\n  " + "─" * (len(header) - 2))
    for attempt, cells in rows:
        line = "  ".join([f"{attempt['attempt']:<3d} {attempt['line']:<9d}"])
        line += " ".join(f"{cells[c]:<{widths[c]}}" for c in columns)
        flag = " ⚠MALFORMED" if attempt["malformed"] else ""
        print(f"  {line}  {attempt['reason'] or '-'}{flag}")
    print("\n  ⚠ = at sentinel (insp -1 = no insertion point · empty override/bubbles = none · "
          f"ACTNAME NONE · sliceset 0x{SLICESET_NONE:08x} = FNV-1 basis = hash of \"\")")
    print("  - = the client NEVER PRINTED that line, which is not the same as a sentinel value. The"
          "\n      ACTIVITY: line is emitted only when an activity client exists AND `ac+0x130 & 2`"
          "\n      is set AND diagnostic filter (system 5, level 2) passes; the whole block is"
          "\n      emitted only on a state-change failure.")

    if brief:
        return
    for attempt in attempts:
        print(f"\n{'=' * 96}\n attempt {attempt['attempt']}  (log line {attempt['line']})")
        print(f"  reason    {attempt['reason']}   [{attempt['current_state']}] -> "
              f"[{attempt['next_state']}]")
        print(f"  context   {attempt['context']}")
        if attempt["state_status"]:
            status = attempt["state_status"]
            print(f"  tasks     status {status['status']}  remaining "
                  f"0x{status['tasks_remaining']:016x}  requested 0x{status['tasks_requested']:016x}")
        for session in attempt["sessions"]:
            print(f"  session   {session['session_id']:<34} HOST={session['host']} "
                  f"LEADER={session['leader']} MEM={session['mem']} "
                  f"PEERS={session['peers']} EST={session['estpeers']}")
        for status in attempt["statuses"]:
            print(f"  ac        {status['flags']}  STATUS [{status['status']}]  SID {status['sid']}")
        if attempt["last_bap_log"]:
            print(f"  last bap  {attempt['last_bap_log']}")
        if attempt["states_walked"]:
            walked = " -> ".join(attempt["states_walked"][-8:])
            print(f"  walked    {walked}")
        for eaten in attempt["eaten"]:
            print(f"  ⚠ line {eaten['line']}: shadPS4 ATE {eaten['count']} repeat(s) of the "
                  f"KTOBUCKET line above it - this record is missing "
                  f"{eaten['count']}× \"{eaten['was'][:70]}\"")
        for bad in attempt["malformed"]:
            print(f"  ⚠ line {bad['line']}  {bad['why']}: {bad['body'][len(KTO_MARK):][:110]}")


def kto_resolve(target: str) -> pathlib.Path | None:
    """Accept a log file, a run directory, or the literal `latest`."""
    if target == "latest":
        logs = sorted((REPO / "runs").glob("*/emulator.log"), key=lambda p: p.stat().st_mtime)
        return logs[-1] if logs else None
    path = pathlib.Path(target)
    if not path.is_absolute():
        path = (REPO / target) if (REPO / target).exists() else path
    if path.is_dir():
        path = path / "emulator.log"
    return path if path.is_file() else None


# ── KTOBUCKET controls ───────────────────────────────────────────────────────────────────────────
# Every fixture line below is a VERBATIM copy of a real line from
# runs/20260809-084932-RUNG33-PERMS6/emulator.log, ANSI wrapper and all.

_P = "\x1b[0;37m[Tty] <Info> (job #0/4) logger.cpp:62 log_flush: [stdout] " + KTO_MARK
_S = "\x1b[m"

KTO_FIXTURE = [
    _P + "STATE_CHANGE_REASON [error.loading_activity_resources], Current State "
         "[setup:activity_world_transition], Next State [cleanup], Context [Transition completed, "
         "but we don't have an active activity.]" + _S,
    _P + "State status [1], tasks remaining: [0x0000000000000001], tasks requested: "
         "[0x0000000000000001]" + _S,
    _P + "SESSION ID:=fireteam:671B7F5C:9C5003FF, HOST=Y, LEADER=Y, MEM=Y, PEERS=1, ESTPEERS=1" + _S,
    _P + "SESSION ID:=posse:D47636A1:212F9728, HOST=Y, LEADER=Y, MEM=Y, PEERS=1, ESTPEERS=1" + _S,
    _P + "ACTIVITY ACTNAME:[NONE] SLICESET HASH:[0x811c9dc5]" + _S,
    _P + "LAST BAP LOG NOTIFICATION: " + _S,
    _P + "ACTIVITY [AC PRIVATE CURRENT CON-Y EST-Y AH->D1A00000:00000001 MEM-1] STATUS: [ok] SID: "
         "d1ah-d1a0000000000001" + _S,
    _P + "ACTIVITY: activity=393, insp=-1, override activity=, bubbles=, reservations=1, "
         "nonce=61711" + _S,
    _P + "ACTIVITY NONE STATUS: [NONE] SID: UNKNOWN" + _S,
    _P + "BAP DESC: BAP  OUT PRIMARY _connected[10.77.0.2:3075]-> 1.5s avg/s [rx 0.0p  0.0kb]"
         "[tx 0.0p  0.0kb][tx block  0.0/q-rem  0.0/s-rem  0.0] IP: 10.77.0.2:3075" + _S,
    _P + "SOCKET INFO: BAP  OUT PRIMARY [state unknown socket state][qlen r/t 8/-273196864]"
         "[type -48][control policy -85][srx: 23243611/42025856/0 bps][stx: 7/8/8 bps]"
         "[sbuf(r/t): 0/0]" + _S,
    _P + "SEND/RECV INFO: BAP  OUT PRIMARY [sent 50 messages/23570 bytes/last 1100 ms ago]"
         "[received 52 messages/25040 bytes/last 958 ms ago]" + _S,
    # ── second attempt: identical except the nonce and the posse session id ──
    _P + "STATE_CHANGE_REASON [error.loading_activity_resources], Current State "
         "[setup:activity_world_transition], Next State [cleanup], Context [Transition completed, "
         "but we don't have an active activity.]" + _S,
    _P + "State status [1], tasks remaining: [0x0000000000000001], tasks requested: "
         "[0x0000000000000001]" + _S,
    _P + "SESSION ID:=fireteam:671B7F5C:9C5003FF, HOST=Y, LEADER=Y, MEM=Y, PEERS=1, ESTPEERS=1" + _S,
    _P + "SESSION ID:=posse:12846CB0:62327C6D, HOST=Y, LEADER=Y, MEM=Y, PEERS=1, ESTPEERS=1" + _S,
    _P + "ACTIVITY ACTNAME:[NONE] SLICESET HASH:[0x811c9dc5]" + _S,
    _P + "ACTIVITY: activity=393, insp=-1, override activity=, bubbles=, reservations=1, "
         "nonce=35758" + _S,
]

# NEGATIVE fixtures. Each is a real line damaged in one specific way.
KTO_FIXTURE_TRUNCATED = [
    KTO_FIXTURE[0],
    _P + "ACTIVITY: activity=393, insp=-1" + _S,                       # the printf died mid-line
]
KTO_FIXTURE_NONNUMERIC = [
    KTO_FIXTURE[0],
    _P + "ACTIVITY: activity=abc, insp=-1, override activity=, bubbles=, reservations=1, "
         "nonce=61711" + _S,
]

# Sample arguments for the round-trip control, one tuple per format, in field order.
KTO_SAMPLES: dict[str, tuple] = {
    "state_change_reason": ("error.loading_activity_resources", "setup:activity_world_transition",
                            "cleanup", "Transition completed, but we don't have an active activity."),
    "state_status": (1, 1, 1),
    "datamine_unavailable": (),
    "bap_desc": ("BAP  OUT PRIMARY _connected[10.77.0.2:3075]-> 1.5s avg/s", "10.77.0.2:3075"),
    "socket_info": ("BAP  OUT PRIMARY [state unknown socket state]",),
    "send_recv_info": ("BAP      OUT FAH [sent 19 messages/6850 bytes]",),
    "actname": ("orbit", 0x3559729A),
    "last_bap_log": ("",),                       # empty %s + trailing space: the real shape
    "activity_status": ("[AC PRIVATE CURRENT CON-Y EST-Y AH->D1A00000:00000001 MEM-1]", "ok",
                        "d1ah-d1a0000000000001"),
    "activity": (393, -1, "", "", 1, 61711),
    "session": ("fireteam:671B7F5C:9C5003FF", "Y", "Y", "Y", 1, 1),
}


def _kto_pyfmt(fmt: str) -> str:
    """C format -> Python format. Only the length modifier differs (`ll` has no Python meaning)."""
    return CONVERSION.sub(lambda m: "%" + (m.group(1) or "") + m.group(2), fmt)


def _kto_first(attempts: list[dict], key: str) -> dict:
    return attempts[0].get(key) or {}


def ktobucket_controls() -> list[tuple[str, bool, str, str]]:
    """(id, ok, what it asserts, detail). Each id's AXIS is named in the label."""
    results: list[tuple[str, bool, str, str]] = []

    def check(cid, ok, label, detail=""):
        results.append((cid, bool(ok), label, str(detail)))

    # ── K1/K2: the pinned fixture, which is verbatim real client output ──────────────────────────
    # AXIS: this proves the parser reads the CLIENT'S REAL DUMP SHAPE, not a shape I invented.
    attempts = kto_scan_lines(KTO_FIXTURE)
    check("K1", len(attempts) == 2,
          "AXIS shape: the pinned real-log fixture splits into 2 attempts at STATE_CHANGE_REASON",
          f"[{len(attempts)}]")
    act = _kto_first(attempts, "activity")
    name = _kto_first(attempts, "actname")
    expected = {"activity": 393, "insp": -1, "override_activity": "", "bubbles": "",
                "reservations": 1, "nonce": 61711}
    got = {k: act.get(k) for k in expected}
    check("K2a", got == expected,
          "AXIS shape: the ACTIVITY line's six fields parse to the pinned real values",
          f"{got}")
    check("K2b", name.get("actname") == "NONE" and name.get("sliceset") == 0x811C9DC5,
          "AXIS shape: ACTNAME==NONE and SLICESET==0x811c9dc5 from the real line",
          f"{name}")
    check("K2c", attempts and attempts[0]["reason"] == "error.loading_activity_resources"
          and attempts[0]["current_state"] == "setup:activity_world_transition"
          and attempts[0]["next_state"] == "cleanup"
          and attempts[0]["context"] == "Transition completed, but we don't have an active activity.",
          "AXIS shape: STATE_CHANGE_REASON / states / Context parse from the real line",
          attempts[0]["reason"] if attempts else "")
    check("K2d", len(attempts[0]["sessions"]) == 2
          and attempts[0]["sessions"][0]["session_id"] == "fireteam:671B7F5C:9C5003FF"
          and attempts[0]["sessions"][1]["peers"] == 1
          and attempts[0]["sessions"][1]["estpeers"] == 1,
          "AXIS shape: both SESSION ID lines parse, with PEERS/ESTPEERS as ints",
          f"[{len(attempts[0]['sessions'])} sessions]")
    check("K2e", not any(a["malformed"] for a in attempts),
          "AXIS shape: not one line of the real fixture is flagged malformed",
          f"{[m['why'] for a in attempts for m in a['malformed']][:2]}")

    # ── K3: the mutation control ────────────────────────────────────────────────────────────────
    # AXIS: values come FROM THE TEXT. A tool with the answers hardcoded passes K2 and fails K3.
    mutated = [line.replace("activity=393", "activity=394")
                   .replace("nonce=61711", "nonce=7")
                   .replace("0x811c9dc5", "0xdeadbeef") for line in KTO_FIXTURE]
    really_changed = mutated != KTO_FIXTURE
    mact = _kto_first(kto_scan_lines(mutated), "activity")
    mname = _kto_first(kto_scan_lines(mutated), "actname")
    check("K3", really_changed and mact.get("activity") == 394 and mact.get("nonce") == 7
          and mname.get("sliceset") == 0xDEADBEEF,
          "AXIS not-hardcoded: mutating the fixture moves every parsed value with it",
          f"activity={mact.get('activity')} nonce={mact.get('nonce')} "
          f"sliceset={mname.get('sliceset')}")

    # ── K4/K5: the negative fixtures ────────────────────────────────────────────────────────────
    # AXIS: a partial or damaged line is FLAGGED, never coerced into a plausible value.
    trunc = kto_scan_lines(KTO_FIXTURE_TRUNCATED)
    trunc_flagged = bool(trunc and trunc[0]["malformed"])
    trunc_clean = bool(trunc and trunc[0]["activity"] is None)
    check("K4", trunc_flagged and trunc_clean,
          "AXIS no-coercion: a TRUNCATED ACTIVITY line is flagged and yields NO activity/insp/nonce",
          f"flagged={trunc_flagged} activity={trunc[0]['activity'] if trunc else '?'}")
    nonnum = kto_scan_lines(KTO_FIXTURE_NONNUMERIC)
    nonnum_flagged = bool(nonnum and nonnum[0]["malformed"])
    nonnum_clean = bool(nonnum and nonnum[0]["activity"] is None)
    check("K5", nonnum_flagged and nonnum_clean,
          "AXIS no-coercion: `activity=abc` is flagged, not coerced to 0",
          f"flagged={nonnum_flagged} activity={nonnum[0]['activity'] if nonnum else '?'}")

    # ── K6: round-trip every pinned format, and prove no two kinds are ambiguous ─────────────────
    # AXIS: the regexes ARE the client's format strings mechanically converted; an ambiguity would
    # silently mis-file a line into the wrong record.
    bad_roundtrip, ambiguous = [], []
    for kind, (_off, fmt, fields) in KTO_FORMATS.items():
        args = KTO_SAMPLES[kind]
        synthetic = _kto_pyfmt(fmt) % args if args else fmt
        got_kind, values, error = kto_parse_line(synthetic)
        if error and error.startswith("AMBIGUOUS"):
            ambiguous.append(f"{kind}: {error}")
            continue
        if got_kind != kind:
            bad_roundtrip.append(f"{kind} -> {got_kind or error}")
            continue
        want = dict(zip(fields, args))
        if values != want:
            bad_roundtrip.append(f"{kind}: {values} != {want}")
    check("K6a", not bad_roundtrip,
          f"AXIS client-shape: all {len(KTO_FORMATS)} pinned formats round-trip to their own args",
          "; ".join(bad_roundtrip[:2]))
    check("K6b", not ambiguous,
          "AXIS client-shape: no synthesized line matches two different KTOBUCKET kinds",
          "; ".join(ambiguous[:2]))

    # ── K7: the formats really are the client's bytes ────────────────────────────────────────────
    # AXIS: provenance. If someone paraphrases a format string here, this fails immediately.
    if EBOOT.is_file():
        data = EBOOT.read_bytes()
        missing = []
        for kind, (offset, fmt, _fields) in KTO_FORMATS.items():
            raw = fmt.encode()
            if data[offset:offset + len(raw)] != raw:
                missing.append(f"{kind}@{offset:#x}")
        check("K7", not missing,
              f"AXIS provenance: all {len(KTO_FORMATS)} format strings are byte-exact in the eboot "
              f"at their pinned offsets", "; ".join(missing[:3]))
    else:
        check("K7", False, "AXIS provenance: the pinned eboot is required to verify the format "
                           f"strings", f"missing {EBOOT}")

    # ── K9: shadPS4's log dedup, which really does eat KTOBUCKET lines ──────────────────────────
    # AXIS: a line the EMULATOR discarded is reported as discarded. This is the project's
    # "a gate that succeeds emptily is invisible" failure mode wearing a log-plumbing hat: without
    # this, a missing ACTIVITY line reads as "the client never printed one".
    eaten_fixture = kto_scan_lines([
        KTO_FIXTURE[0], KTO_FIXTURE[8],
        "\x1b[1;33mSkipped 2 duplicate messages..\x1b[m",
        KTO_FIXTURE[7],
    ])
    ignored_fixture = kto_scan_lines([
        KTO_FIXTURE[0],
        "\x1b[1;33m[Render.Vulkan] <Warning> image.cpp:467 SanitizeCopyLayers: whatever\x1b[m",
        "\x1b[1;33mSkipped 2 duplicate messages..\x1b[m",
        KTO_FIXTURE[7],
    ])
    eaten = eaten_fixture[0]["eaten"] if eaten_fixture else []
    check("K9a", len(eaten) == 1 and eaten[0]["count"] == 2
          and eaten[0]["was"].startswith("ACTIVITY NONE STATUS"),
          "AXIS eaten-lines: a dedup notice right after a KTOBUCKET line is reported, with the "
          "count and the line that was lost", f"{eaten}")
    check("K9b", ignored_fixture and not ignored_fixture[0]["eaten"],
          "AXIS eaten-lines: a dedup notice after a NON-KTOBUCKET line is NOT charged to the "
          "attempt", f"{ignored_fixture[0]['eaten'] if ignored_fixture else '?'}")

    # ── K8: the sentinel table is defined, not eyeballed ────────────────────────────────────────
    fnv1_basis = 0x811C9DC5
    levers = dict((n, s) for n, _v, s in kto_levers(attempts[0]))
    check("K8", SLICESET_NONE == fnv1_basis and levers.get("insp") and levers.get("override")
          and levers.get("bubbles") and levers.get("sliceset") and levers.get("actname")
          and levers.get("nonce") is False,
          "AXIS sentinel-definition: insp/override/bubbles/actname/sliceset flag as sentinel and "
          "nonce does not", f"{levers}")

    return results


def kto_scan_lines(lines: list[str]) -> list[dict]:
    """kto_scan()'s body, over an in-memory list - this is what the fixtures exercise."""
    import tempfile
    with tempfile.NamedTemporaryFile("w", suffix=".log", delete=False) as handle:
        handle.write("\n".join(lines) + "\n")
        temp = pathlib.Path(handle.name)
    try:
        return kto_scan(temp)
    finally:
        temp.unlink(missing_ok=True)


def controls_invert() -> int:
    """⭐ Prove the controls CAN FAIL. A control that only ever passes is decoration (LESSONS #7).

    Two defects are injected, one at a time, each the realistic bug the matching control exists to
    catch. Injecting a defect and still passing is itself a failure, and exits 2.
    """
    inversions = [
        ("KTO_DEFECT_SCRAPE", True,
         "the naive parser - scrape each field with its own regex, default when missing/non-numeric",
         {"K4", "K5"}),
        ("KTO_DEFECT_HARDCODE", 393,
         "return a constant 393 for `activity` instead of reading the text", {"K3"}),
        ("KTO_DEFECT_UNANCHORED", True,
         "drop the \\A..\\Z anchors - non-greedy %s then matches empty and the shape stops being "
         "the client's", {"K6a"}),
        ("KTO_DEFECT_IGNORE_DEDUP", True,
         "treat shadPS4's `Skipped N duplicate messages` as noise - a line the emulator ate then "
         "reads as a line the client never printed", {"K9a"}),
    ]
    failed = 0
    for defect, value, description, must_fail in inversions:
        restore = globals()[defect]
        globals()[defect] = value
        _KTO_CACHE.clear()
        broken = {cid for cid, ok, _l, _d in ktobucket_controls() if not ok}
        globals()[defect] = restore
        _KTO_CACHE.clear()
        caught = must_fail & broken
        ok = caught == must_fail
        failed += 0 if ok else 1
        print(f"{'PASS' if ok else 'FAIL'}  inverted [{defect}] {description}\n"
              f"        expected {sorted(must_fail)} to FAIL, actually failed {sorted(broken)}")
    print("\nINVERSION PROVEN - every control above can fail" if not failed
          else f"\n{failed} INVERSION(S) DID NOT TRIP - those controls are decoration")
    return 0 if not failed else 2


def controls() -> int:
    """Static controls only - these must hold with no client running."""
    checks = [
        (len(PARAMS) == 23, "the parameter map has 23 entries", f"[{len(PARAMS)}]"),
        (PARAMS[0x9B8][0] == "activity",
         "+0x9B8 is `activity` - the parameter orbit's update tests", PARAMS[0x9B8][0]),
        (PARAMS[0x000][0] == "world_controller_goal",
         "+0x000 is `world_controller_goal`", PARAMS[0x000][0]),
        (PARAMS[0x390][0] == "activity_host",
         "+0x390 is `activity_host` - where the AH address arrives", PARAMS[0x390][0]),
        (VALID_STATE_MASK == 0x7F0, "the session-state mask is 0x7F0 (states 4..10)", ""),
        (STATES[27] == "setup:orbit" and STATES[28] == "setup:activity_session_creation",
         "the state table agrees with the ladder at 27/28", ""),
    ]
    # Sizes must satisfy align(0xE1) + 2*sizeof(T) < the gap to the next parameter.
    offsets = sorted(PARAMS)
    overlaps = []
    for a, b in zip(offsets, offsets[1:]):
        name, _, value_off, value_size = PARAMS[a]
        if value_off + 2 * value_size > (b - a):
            overlaps.append(f"{name} needs {value_off + 2*value_size:#x}, gap is {b-a:#x}")
    checks.append((not overlaps,
                   "every parameter's base+2*sizeof(T) fits before the next parameter",
                   "; ".join(overlaps[:2]) if overlaps else f"[{len(offsets)-1} gaps]"))

    failed = 0
    print("── session parameter map (static) " + "─" * 60)
    for ok, label, detail in checks:
        print(f"{'PASS' if ok else 'FAIL'}  {label}" + (f"  [{detail}]" if detail else ""))
        failed += 0 if ok else 1

    print("\n── KTOBUCKET reader " + "─" * 73)
    for cid, ok, label, detail in ktobucket_controls():
        print(f"{'PASS' if ok else 'FAIL'}  {cid:5} {label}" + (f"  [{detail}]" if detail else ""))
        failed += 0 if ok else 1

    print("\nALL CONTROLS PASS" if not failed else f"\n{failed} CONTROL(S) FAILED")
    return 0 if not failed else 2


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--controls", "--self-test", action="store_true", dest="controls")
    parser.add_argument("--controls-invert", action="store_true",
                        help="break the KTOBUCKET parser on purpose and require the controls to FAIL")
    parser.add_argument("--ktobucket", metavar="RUN",
                        help="parse a run dir / emulator.log / `latest` for the client's own "
                             "KTOBUCKET launch-parameter dumps (offline; no client needed)")
    parser.add_argument("--brief", action="store_true", help="with --ktobucket: table only")
    parser.add_argument("--json", action="store_true", help="with --ktobucket: raw records")
    parser.add_argument("--session", type=int, help="0 = fireteam, 1 = activity/posse")
    parser.add_argument("--param", help="only parameters whose name contains this")
    parser.add_argument("--hex", action="store_true", help="full hex for populated parameters")
    parser.add_argument("--require", type=int, default=2)
    parser.add_argument("--launch-poke", action="store_true",
                        help="the three writes that move rung 27 -> 28 (synthetic_ancestor x2)")
    parser.add_argument("--activity-id", type=int, default=1,
                        help="u16 written to s_activity_information+0x02 (1 = the Tower)")
    parser.add_argument("--dry-run", action="store_true", help="with --launch-poke: show, do not write")
    args = parser.parse_args()

    if args.controls_invert:
        return controls_invert()

    if args.controls:
        return controls()

    if args.ktobucket:
        path = kto_resolve(args.ktobucket)
        if path is None:
            print(f"session-params: no emulator.log for {args.ktobucket!r}", file=sys.stderr)
            return 2
        attempts = kto_scan(path)
        if args.json:
            print(json.dumps(attempts, indent=2))
            return 0
        print(f"\n{path}")
        kto_report(attempts, args.brief)
        return 0

    d1guest = load_d1guest()
    try:
        proc, guest = open_live(d1guest, args.require)
    except SystemExit:
        raise
    except Exception as error:                            # noqa: BLE001
        print(f"session-params: cannot attach to a live client: {error}", file=sys.stderr)
        return 2

    if args.launch_poke:
        info = resolve(proc, guest, 0)
        if "manager" not in info or "error" in info:
            print(f"REFUSING: session 0 is not usable - {info.get('error', 'no manager')}",
                  file=sys.stderr)
            return 2
        return launch_poke(proc, guest, info["params"], args.activity_id, args.dry_run)

    indices = [args.session] if args.session is not None else [0, 1]
    for index in indices:
        info = resolve(proc, guest, index)
        label = SESSION_NAMES.get(index, str(index))
        print(f"\n{'=' * 92}\n session {index} ({label})")
        if "manager" not in info:
            print(f"  {info['error']}")
            continue
        state = info["state"]
        print(f"  manager {info['manager']:#x}  slot {info['slot']:#x}  "
              f"inner {info['inner']:#x}\n  session state {state}  params {info['params']:#x}")
        if "error" in info:
            print(f"  ⚠ {info['error']}")
        dump(proc, info["params"], args.param, args.hex)

    print("\n⭐ = has_value. Orbit advances to rung 28 when `activity` (+0x9B8) has a value "
          "and its short != -1.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
