# Persisted fragment-program validation — 01.24

Research and patch validation on 3 October 2026. This concerns cache startup
failure, rather than proof of an in-game rendering or mission fix.

## Captured failure

The private `FINAL124_JOIN91-native-boot.log` reports invalid source type 3
for opcode `0x19` (TXD), with decoded words beginning
`999a3f99 / 66673ee6 / 18b93f7f`. A preserved 32-byte raw fragment-program
file reproduces this pattern. Its END bit is already set and its source-0
constant has a following 16-byte literal, so size and END checks alone would
admit it. TXD's first gradient source has reserved register type 3.

Rotating the private cache before JOIN92 allowed boot and screenshots again.
The old cache was retained for diagnosis. The current runtime also saves
this malformed pattern to its fresh cache, so validating reads alone would
allow the cache to become polluted again after a successful boot.

## Patch behavior and limits

[`rpcs3-fragment-cache-validation.patch`](../patches/rpcs3-fragment-cache-validation.patch)
changes only `rsx_cache.h` and adds a standard-library-only validation header
and standalone test/scan executable. It preserves the pre-existing RPCS3
modifications. The cache loader skips a rejected raw program before shader
preloading, leaving its file intact. Cache storage rejects that program before
writing its raw program or pipeline entry. No cache files are removed and no
new launch gate is introduced.

Each preload keeps a mutex-protected set of raw fragment hashes rejected by
these checks. Later pipeline variants referencing the same rejected raw file
are skipped before loading either raw shader again. The set contains only
deterministic validation failures; open failures, empty files and short reads
remain retryable. It exists only for that preload invocation, so the next
preload checks repaired files again. Concurrent first references may still
validate the same file before its rejection enters the set. Valid files always
follow the existing load and compilation path.

The validator walks at most `file_size / 16` instruction slots:

- Files must contain complete 16-byte slots and an END instruction within
  those bounds. The first END ends the walk; trailing bytes are allowed.
- A constant source advances over one literal slot, including unused source
  fields on NOP. A required literal must fit inside the file.
- Flow opcodes `0x40..0x45` and fences `0x3d/0x3e` do not treat their fields
  as literal registers, matching `FPToCFG.cpp`.
- Used ordinary source fields cannot have reserved type 3. Unknown opcodes
  are not rejected solely for their opcode. TXD consumes coordinates and
  two gradients, even though the assembler operand-count helper reports one.
- IFE, LOOP and REP targets must fit the file's slot range, allowing a merge
  immediately after the final slot. Empty IFE blocks ignore the unused else
  offset, matching the CFG implementation.

This is a narrow cache sanity check, not full shader verification. It does
not bound the CFG's nesting or repair the runtime binding that supplied
malformed shader bytes. Existing live decompilation behavior is unchanged.

## Reproduce the checks

Apply the patch in the private RPCS3 source checkout. From a Visual Studio
x64 developer command prompt, run:

```bat
mkdir build\fragment-cache-validation
cl /nologo /std:c++20 /EHsc /W4 /I rpcs3 rpcs3\tests\fragment_cache_validation_standalone.cpp /Fe:build\fragment-cache-validation\fragment-cache-validation.exe /Fo:build\fragment-cache-validation\fragment-cache-validation.obj
build\fragment-cache-validation\fragment-cache-validation.exe
build\fragment-cache-validation\fragment-cache-validation.exe "<preserved-shaders-cache>\raw" "<fresh-shaders-cache>\raw"
```

The scan reads `.fp` files and reports each rejection category. It prints the
paths of branch-bound failures for manual inspection. Rejected scan entries
are expected evidence, so the exit code reflects fixture/read failures,
rather than whether an archived cache contains malformed files.

MSVC 14.51.36231 compiled the standalone executable with `/W4`, without
warnings. All **22 fixtures passed**, including alignment, missing END,
truncated literals, END-looking literal contents, NOP literals, used versus
unused source types, both TXD gradients, valid flow fields with low bits 2,
empty IFE and fences, invalid branch targets, and the captured float pattern.
The standalone translation unit also includes the helper with Qt's `slots`
and Windows' `min/max` macros defined, checking that those caller macros
remain intact. The helper uses `instruction_slots` to avoid the Qt keyword
collision when included from GUI translation units.
An independent reviewer reran the fixtures and archived scan.

Read-only scan snapshots at approximately 21:50 BST:

| Raw cache | Accepted | Used source type 3 | Branch out of bounds | Other rejects |
| --- | ---: | ---: | ---: | ---: |
| Preserved before JOIN92 | 674 | 11,136 | 4 | 0 |
| Fresh JOIN92 snapshot | 385 | 978 | 0 | 0 |

The archived total is 11,814 raw programs. All four branch failures contain
float-looking bytes: the bounded walk reaches REP at slot 5 with end offset
`e1483e7a`, or `0x38520f9e` instruction slots, while the files contain only
7–8 slots. They are not a plausible ordinary branch target. The fresh cache
is live and its counts may increase. “Accepted” means the checks above passed;
it does not establish that every accepted program renders correctly.

`git apply --reverse --check` succeeds against the changed private source.
The full Release emulator compiled successfully on 3 October. The resulting
binary was deployed on 4 October, with the previous executable retained privately.
A direct game-launch probe using the preserved bad cache records rejected raw
programs being skipped before preloading. JOIN94 also exercised that cache in the
full local stack before it was stopped for its lengthy preload. These checks do not establish a live binding repair or clean
mission rendering; see the [October 4 checkpoint](checkpoint-2026-10-04-mission.md).

## Preserved-cache preload cost on 4 October

JOIN94 used the retained cache with **198,264 pipeline entries**, referencing
11,814 distinct raw fragment files. Of those pipeline entries, 194,314 refer
to raw files rejected for source types and 32 refer to rejected branch targets;
3,918 refer to raw files accepted by the narrow validator. Thus 98.02% of this
cache's pipeline entries cannot be preloaded.

Before rejection memoization, a native log snapshot at 196.9 seconds contained
33,828 rejection messages for only 6,525 distinct fragment hashes. A later
snapshot at 331.9 seconds contained 52,973 messages for 9,609 hashes. These
snapshots show continuing preload work, with substantial repeated loading and
logging; they do not show a stalled loader. A separate read-only validation of
the raw files and pipeline filename census took approximately 9.9 seconds.
That timing excludes shader compilation and is not a game-boot prediction.

The per-preload rejection set addresses this duplicated work without deleting
the archived files or admitting rejected shaders. The original 22 standalone
validation fixtures still pass. The full Release build including memoization
passed on 4 October and was deployed with its predecessor retained privately.
A live boot with that additional change remains to be checked.
