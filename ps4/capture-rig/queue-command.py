#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Submit a command to the client's own lock-free command queue - and inspect it first.

WHAT THIS IS
------------
`0x753540` (world_controller change-world) is **never called directly at runtime**. It is
registered as the handler for **slot 3** of a 19-slot command queue and invoked indirectly by the
pump at `0x34C0BC`. The pump's protocol, read straight off the disassembly of `0x34C010`:

    registry            = [0x267B110]                       (via accessor 0x34C140)
    handlers[slot]      = registry + 0x80  + slot*8         must be non-NULL
    state[slot]         = registry + 0x1B4 + slot*4         0 = free, 2 = READY, 3 = running
    buffer[slot]        = registry + 0x200 + slot*0x1C04    what the handler receives in RDI

    0x34c03f  cmp ecx, 2                  ; scan: any slot with state == 2 ?
    0x34c0a4  lock cmpxchg [rbx], r15d    ; CAS 2 -> 3, claim it
    0x34c0b4  lea rdi, [r14+rax+0x200]    ; RDI = buffer[slot]
    0x34c0bc  call qword [r14+r13*8+0x80] ; the handler
    0x34c0d4  lock cmpxchg [rbx], r12d    ; CAS -> 0, release

⇒ **Two writes cause a dispatch:** put a command in `buffer[slot]`, then set `state[slot] = 2`.
The client's own pump does the rest. That is the whole mechanism.

⚠⚠ THIS IS A HOP MECHANIC OF THE LOWEST EVIDENTIAL QUALITY.
A dispatch forced this way proves the client CAN perform the operation. It proves nothing about
what a real service would have sent, and nothing about our server - the bytes never crossed a
wire. Everything downstream is `synthetic_ancestor` twice over. A poke can IDENTIFY a gate; it
cannot SATISFY one. Report the crash, if any, in the same breath as the advance.

USAGE
-----
    # read-only. Always do this first.
    python3 -B capture-rig/queue-command.py --inspect

    # submit a command blob to slot 3
    python3 -B capture-rig/queue-command.py --slot 3 --command cmd.bin \
        --experiment replay-ambient-bootstrap --confirm
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import pathlib
import struct
import sys
import time

HERE = pathlib.Path(__file__).resolve().parent
REPO = HERE.parent

# --- the queue layout, all read off 0x34C010 / 0x34BDC0 -------------------------------------------
REGISTRY_PTR_VADDR = 0x267B110
HANDLERS_OFFSET = 0x80
STATE_OFFSET = 0x1B4
BUFFER_OFFSET = 0x200
BUFFER_STRIDE = 0x1C04
SLOT_COUNT = 0x13  # `cmp eax, 0x13` in the scan loop
STATE_FREE, STATE_READY, STATE_RUNNING = 0, 2, 3
# Slots identified so far, from the 17 registrations at 0x752ED0.
KNOWN_SLOTS = {3: "world_controller change-world (0x753540)",
               0xA: "0x754320", 0xB: "0x754460"}
JOURNAL = REPO / "runs" / "_writes" / "queue-command.jsonl"


def load_d1guest():
    """Import the one live-state instrument rather than re-deriving base recovery."""
    spec = importlib.util.spec_from_file_location("d1guest", HERE / "d1guest.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules["d1guest"] = module
    spec.loader.exec_module(module)
    return module


def open_live(d1guest, require: int):
    # `--symbols` defaults to d1guest.SYMBOLS in its own parser; passing None here reaches
    # Symbols(None) and fails inside os.path. Use the module's own default so the build gate
    # (symbol file vs eboot sha256) still runs.
    args = argparse.Namespace(
        symbols=d1guest.SYMBOLS, eboot=None, ignore_build_gate=False, full_scan=False,
        no_cache=False, allow_dead=False, account_soid=None, require=require,
    )
    syms, image, proc, guest = d1guest.open_guest(args)
    level = d1guest.print_liveness(guest, args, require=require)
    return syms, image, proc, guest, level


def queue_addresses(registry: int, slot: int) -> dict:
    return {
        "registry": registry,
        "handler": registry + HANDLERS_OFFSET + slot * 8,
        "state": registry + STATE_OFFSET + slot * 4,
        "buffer": registry + BUFFER_OFFSET + slot * BUFFER_STRIDE,
    }


def inspect(guest, proc, base: int) -> int:
    # ⚠ THE TRAP THAT COST A FINDING (2026-08-08): the registry pointer is a HEAP address and is
    # already absolute. `guest.read(vaddr)` / `d1guest read` WITHOUT --abs add the eboot base
    # (0x800000000), landing in unmapped memory that reads as ZEROS -- indistinguishable from
    # "the structure was torn down". A whole causal theory was published on that mistake.
    # Only eboot vaddrs (BSS/.text, e.g. 0x267B110) get the base added.
    registry = guest.u64(REGISTRY_PTR_VADDR)
    print(f"\nregistry [{REGISTRY_PTR_VADDR:#x}] = {registry:#018x}")
    if registry == 0:
        print("  NULL - the queue has not been built yet. Nothing can be submitted.")
        return 2
    print(f"\n{'slot':>4}  {'handler':>18}  {'state':>5}  {'buffer':>18}  note")
    live = 0
    for slot in range(SLOT_COUNT):
        a = queue_addresses(registry, slot)
        handler = struct.unpack("<Q", proc.read(a["handler"], 8))[0]
        state = struct.unpack("<I", proc.read(a["state"], 4))[0]
        if handler:
            live += 1
        name = {STATE_FREE: "free", STATE_READY: "READY", STATE_RUNNING: "running"}.get(state, "?")
        note = KNOWN_SLOTS.get(slot, "")
        if handler:
            # Handlers are eboot code; show the guest vaddr, which is what the docs use.
            note = f"handler_vaddr={handler - base:#x}  {note}"
        print(f"{slot:>4}  {handler:#018x}  {state:>2}({name:<7})  {a['buffer']:#018x}  {note}")
    print(f"\n{live} of {SLOT_COUNT} slots have a registered handler")
    print("A slot with handler == 0 can never dispatch, whatever you write to its buffer.")
    return 0


def submit(guest, proc, base: int, slot: int, blob: bytes, experiment: str) -> int:
    registry = guest.u64(REGISTRY_PTR_VADDR)
    if registry == 0:
        print("REFUSING: registry is NULL - the queue has not been built.", file=sys.stderr)
        return 2
    a = queue_addresses(registry, slot)

    handler = struct.unpack("<Q", proc.read(a["handler"], 8))[0]
    state = struct.unpack("<I", proc.read(a["state"], 4))[0]

    # --- refusals, each for a reason that would otherwise waste the run ---------------------------
    if handler == 0:
        print(f"REFUSING: handlers[{slot}] is NULL. The pump skips slots with no handler "
              f"(0x34C070 `cmp qword [...], 0; je`), so nothing would dispatch.", file=sys.stderr)
        return 2
    if state != STATE_FREE:
        name = {STATE_READY: "READY", STATE_RUNNING: "RUNNING"}.get(state, str(state))
        print(f"REFUSING: state[{slot}] is {name}, not free. A command is already in flight; "
              f"overwriting its buffer would corrupt it.", file=sys.stderr)
        return 2
    if len(blob) > BUFFER_STRIDE:
        print(f"REFUSING: command is {len(blob)} bytes, buffer stride is {BUFFER_STRIDE} "
              f"({BUFFER_STRIDE:#x}).", file=sys.stderr)
        return 2

    print("=" * 96)
    print("⚠ CLIENT MODIFICATION. A forced dispatch shows the client CAN do this. It is NOT")
    print("  evidence about a real service, and not even about our server - nothing crossed a")
    print("  wire. Everything downstream is `synthetic_ancestor`. Report any crash alongside")
    print("  any advance.")
    print("=" * 96)
    print(f"  slot      {slot}  {KNOWN_SLOTS.get(slot,'')}")
    print(f"  handler   {handler:#018x}  (vaddr {handler - base:#x})")
    print(f"  buffer    {a['buffer']:#018x}   {len(blob)} bytes")
    print(f"  state     {a['state']:#018x}   {state} -> {STATE_READY}")

    # 1. the command
    proc.write(a["buffer"], blob)
    readback = proc.read(a["buffer"], len(blob))
    if readback != blob:
        print("  *** BUFFER READBACK MISMATCH - NOT setting state. Nothing was dispatched. ***",
              file=sys.stderr)
        return 2
    print(f"  buffer written and verified ({len(blob)} bytes)")

    # 2. the trigger. Ordering matters: buffer first, then state, or the pump can dispatch a
    #    half-written command.
    proc.write(a["state"], struct.pack("<I", STATE_READY))
    now = struct.unpack("<I", proc.read(a["state"], 4))[0]
    print(f"  state now {now}  {'(claimed/dispatched already)' if now in (STATE_RUNNING, STATE_FREE) else ''}")

    record = {
        "t": time.strftime("%Y-%m-%dT%H:%M:%S"), "pid": proc.pid, "experiment": experiment,
        "slot": slot, "registry": registry, "buffer": a["buffer"], "state_addr": a["state"],
        "handler": handler, "bytes": len(blob), "readback_ok": True,
    }
    os.makedirs(JOURNAL.parent, exist_ok=True)
    with JOURNAL.open("a") as handle:
        handle.write(json.dumps(record) + "\n")
    print(f"  journalled to {JOURNAL}")
    print("\nWatch the guest log for the handler's own diagnostics. For slot 3 those are:")
    print("  world_controller: no activity specified, cannot initialize world")
    print("  world_controller: failed to activate activity: %s")
    print("  world_controller: load successful, but failed to instantiate world and start game")
    print("  world_controller: successfully changed world to: %s")
    print("  world_controller: failed to change to '%s', returning to pregame")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--inspect", action="store_true", help="read-only dump of every slot")
    parser.add_argument("--slot", type=int, default=3)
    parser.add_argument("--command", type=pathlib.Path, help="file holding the command blob")
    parser.add_argument("--experiment", help="name for the write journal")
    parser.add_argument("--confirm", action="store_true")
    parser.add_argument("--require", type=int, default=2, help="minimum liveness level")
    args = parser.parse_args()

    d1guest = load_d1guest()
    try:
        syms, image, proc, guest, level = open_live(d1guest, args.require)
    except SystemExit:
        raise
    except Exception as error:  # noqa: BLE001 - surface the real cause, do not mask it
        print(f"queue-command: cannot attach to a live client: {error}", file=sys.stderr)
        return 2

    if args.inspect or not args.command:
        return inspect(guest, proc, guest.base)

    if not (args.confirm and args.experiment):
        print("submitting requires --experiment <name> and --confirm. Read the header first.",
              file=sys.stderr)
        return 2
    if not 0 <= args.slot < SLOT_COUNT:
        print(f"slot must be 0..{SLOT_COUNT - 1}", file=sys.stderr)
        return 2
    blob = args.command.read_bytes()
    return submit(guest, proc, guest.base, args.slot, blob, args.experiment)


if __name__ == "__main__":
    sys.exit(main())
