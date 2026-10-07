# Filtered native PPU entry watches

The LLVM watch callback runs at compiled **function entries**. A watch on an
arbitrary instruction after a call does not establish that call's return value.
Routine messages can consume a finite trace before the mission transition.
The optional filters in `patches/rpcs3-ppu-watch-filters.patch` select the relevant
arguments before deduplication or hit counting.

Apply this incremental patch to the private renderer with the existing Destiny
PPU watch implementation (`rpcs3-destiny-main-memory-blit.patch`). It changes
only watch parsing/filtering and adds its header; it does not modify guest state,
transport behavior, rendering or session timeouts. Keep game binaries, generated
fixtures, raw traces and native evidence outside this repository.

Each line still accepts a hexadecimal entry address, optional hit budget
(default 50), optional `u`, and `rN[+OFFSET]:LENGTH` memory dumps. New predicates:

```text
48e7f0 80 if=r5==0x15
47676c 80 if=m6+1:4==0x15 r6:13
479fa8 80 if=m6+1:4==0x15 r6:13
```

These are the verified **01.24** transport wrapper, queue and serializer entries.
The wrapper receives the subtype in r5; the downstream serialized header holds
it at byte1 as a four-byte big-endian integer. The fixed header is 13 bytes;
avoid dumping uninitialized tails from variable-length payloads. Use one watch
per function. This example deliberately omits `u` so repeated subtype 21 calls
remain visible.

`if=rN==VALUE` compares the full 64-bit register. `if=mN[+OFFSET]:WIDTH==VALUE`
reads a big-endian integer through the low 32-bit guest pointer; WIDTH must be
1, 2, 4 or 8. Values/offsets accept decimal or lowercase `0x` hexadecimal; register
numbers and widths are decimal. Multiple predicates are ANDed. r1 uses the live
stack pointer passed by translated code, including for memory filters. The old
callback does not expose live LLVM ABI values for r0 or r2; predicates using
those registers are rejected. The native transport watches use r5 and r6.
Malformed or out-of-range predicates reject the whole watch. Unreadable or
wrapping guest ranges do not match. Rejected calls leave the watch armed, do
not use its hit budget and do not populate unique keys. Existing unfiltered,
unique-dump/LR and one-hit coverage behavior remains available.

## Verification — 7 October 2026

The actual private watch declaration, parser and callback are extracted into a
CPU fixture with fake guest memory; the actual new header is copied into that
private fixture directory. Generate it with:

```powershell
python scripts/rpcs3_ppu_watch_filter_audit.py --root <private-rpcs3-root> --out <new-private-directory>
```

Compile the generated `ppu-watch-filter-audit.cpp` with a C++20 compiler and run
it. MSVC `/std:c++20 /EHsc /W4 /O2` passes **44 checks**, including 100,000 rejected
routine calls, a later accepted target, unique-key/budget retention, invalid
predicate rejection, big-endian unaligned reads, unreadable/wrapping ranges,
live r1 and unchanged ordinary/coverage watches. The incremental patch applies
to saved pre-change files and reproduces all three modified files with normalized
newlines.

These checks establish diagnostic behavior only. Native packet delivery,
destination activation, Guardian control and combined character/world graphics
require separate native evidence. In JOIN171 the target was copied into both
engine/host last-sent records without a corresponding host-received message;
neither a copied baseline nor the wrapper's unconditional success return proves
delivery. See [the session timeline](session-transition-probe-0124.md).
