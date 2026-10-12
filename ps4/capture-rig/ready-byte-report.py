#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Read a trace and answer ONE question: why is the account record's ready byte still zero?

`decode-account-trace.py` prints every record, which is the right tool for exploring and the wrong
one for deciding -- a 60,000-record dump buries the four facts that matter. This prints only those.

THE CHAIN (historicaldocs/design/character-and-orbit.md §26.41), each link with the probe that observes it:

    0x584F74  group4_request   client REQUESTS a class-4 object keyed by the account SOID
    0x56D306  class4_added     ...the object ARRIVES in the registry            <-- usually missing
    0x56CA70  registry_find    ...a lookup finds it
    0x583484  producer_result  0x583450 returns true/false                      <-- the decision
    0x58585E  ready_setter     record+0x18 = that boolean                       <-- the blocker
    0x79C48D  acct_object      current_account_object() -> non-NULL             <-- predicate 3

A link that never fires is as informative as one that fires with a bad value, so EVERY link is
printed with its firing count, including the zeroes. `never fired` on an unarmed probe and
`never fired` on an armed one mean different things, so the armed count is printed too -- it is the
control (§4.4: a silently no-opped edit once armed 12 probes while the doc claimed 18).

    ./ready-byte-report.py runs/<run>/account-trace.bin
"""
import collections
import pathlib
import struct
import sys

RECORD = struct.Struct("<IHHII15Q48s48s")
MAGIC = 0x52543144  # "D1TR" little-endian; must match d1_account_trace.cpp TraceMagic

NAMES = {
    25: "producer_entry",
    26: "producer_result",
    27: "ready_setter",
    28: "registry_find",
    29: "group4_request",
    30: "class4_added",
    15: "acct_ready",
    16: "profile_bound",
    17: "acct_object",
    18: "profile_match",
    8: "queuez_serializer_entry",
    0: "armed",
}
CHAIN = [29, 30, 28, 26, 27, 17]
PREDICATES = [(15, "1 account_ready"), (16, "2 profile_bound"),
              (17, "3 current_account_object"), (18, "4 profile_record_matches")]
REGS = ("guest_vaddr", "rip", "rax", "rbx", "rcx", "rdx", "rsi", "rdi",
        "rbp", "rsp", "r12", "r13", "r14", "r15", "rflags")


def records(path):
    blob = pathlib.Path(path).read_bytes()
    for off in range(0, len(blob) - RECORD.size + 1, RECORD.size):
        f = RECORD.unpack_from(blob, off)
        if f[0] != MAGIC:
            continue
        # ⚠ Field order is magic, version, probe_id, record_size, sequence -- probe_id is index 2,
        # NOT 3. Reading index 3 yields record_size (a constant 232) for every record, so every
        # probe reads "NEVER FIRED" and the tool reports a broken chain that is not broken.
        # Caught only because 232 is a recognisable number. The `armed` control caught it too:
        # a trace whose first record is `armed` cannot legitimately report 0 armed records.
        yield {"probe": f[2], "seq": f[4],
               **{n: v for n, v in zip(REGS, f[5:20])},
               "a": f[20], "b": f[21]}


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)
    rs = list(records(sys.argv[1]))
    by = collections.defaultdict(list)
    for r in rs:
        by[r["probe"]].append(r)

    print(f"{len(rs)} records, {len(by)} distinct probes fired\n")

    print("THE READY-BYTE CHAIN")
    print("-" * 72)
    for pid in CHAIN:
        hits = by.get(pid, [])
        name = NAMES.get(pid, f"probe{pid}")
        if not hits:
            print(f"  {name:<18} NEVER FIRED   <-- the chain stops at or before here")
            continue
        print(f"  {name:<18} {len(hits):>6} hits   seq {hits[0]['seq']}..{hits[-1]['seq']}")
        if pid == 26:  # producer_result: RAX is the found element, 0 = miss
            hit = sum(1 for h in hits if h["rax"])
            print(f"  {'':18} registry HIT {hit} / MISS {len(hits)-hit}"
                  f"   key={hits[-1]['b'][:8][::-1].hex()}")
        if pid == 27:  # ready_setter: AL is the value stored into record+0x18
            ones = sum(1 for h in hits if h["rax"] & 0xFF)
            print(f"  {'':18} wrote 1: {ones}   wrote 0: {len(hits)-ones}")
            print(f"  {'':18} last record bytes: {hits[-1]['a'][:32].hex()}")
        if pid == 29:  # group4_request: ESI is the class, RDX -> the key
            cls = collections.Counter(h["rsi"] & 0xFFFFFFFF for h in hits)
            print(f"  {'':18} classes requested: {dict(cls)}  key={hits[-1]['a'][:8][::-1].hex()}")
        if pid == 17:  # acct_object: RAX is the returned object
            nn = sum(1 for h in hits if h["rax"])
            print(f"  {'':18} non-NULL {nn} / NULL {len(hits)-nn}")

    print("\nREGISTRY LOOKUPS BY (class, type)  -- which buckets are consulted, and by whom")
    print("-" * 72)
    seen = collections.Counter(
        ((h["rsi"] & 0xFFFFFFFF), (h["rdx"] & 0xFFFFFFFF)) for h in by.get(28, []))
    for (cls, typ), n in sorted(seen.items()):
        mark = "   <-- the account lookup" if (cls, typ) == (4, 0) else ""
        print(f"  class {cls:<3} type {typ:<3}  {n:>6} lookups{mark}")
    if not seen:
        print("  (registry_find never fired)")

    print("\nSUBSTATE B PREDICATES  (all four must hold in ONE frame)")
    print("-" * 72)
    for pid, label in PREDICATES:
        hits = by.get(pid, [])
        if not hits:
            print(f"  {label:<28} never reached")
            continue
        true = sum(1 for h in hits if (h["rax"] & 0xFF if pid != 17 else h["rax"]))
        verdict = "PASSES" if true else "*** BLOCKING ***"
        print(f"  {label:<28} {true}/{len(hits)} true   {verdict}")

    armed = by.get(0, [])
    print(f"\nCONTROL: {len(armed)} 'armed' record(s). If this is 0, the tracer never started and "
          f"every 'never fired' above is meaningless.")


if __name__ == "__main__":
    main()
