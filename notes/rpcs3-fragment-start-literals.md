# Fragment-program start and NOP literals

Investigation on 5 October 2026. The parser correction is verified; the native
comparison still produces malformed live programs. **Device loss remains open.**

## Source defect

RPCS3's `fragment_program_utils::analyse_fragment_program()` first skips
leading NOP instructions, then scans the remaining program for metadata.
The second scan and `get_fragment_program_ucode_size()` skip a 16-byte literal
when any source field declares a constant. The first scan previously advanced
only one slot, even when a NOP declared that following literal.

The existing `Assembler/FPToCFG.cpp` NOP path explicitly skips such a literal
and describes that behavior as verified on real hardware. The correction makes
the preliminary metadata scan follow that same instruction layout.

This lets a literal become the apparent first instruction. A fixture with the
float-looking words from the [earlier malformed-program investigation](rpcs3-fragment-cache-validation.md)
reproduces a false start at byte 16 and a false 32-byte program, instead of
the real instruction at byte 32. The literal has an END-looking bit, so a
simple END/size check cannot identify this metadata error.

An END NOP also previously reported only 16 bytes, with zero constant-buffer
length, when its source fields declared a following literal.

[`rpcs3-fragment-leading-nop-literals.patch`](../patches/rpcs3-fragment-leading-nop-literals.patch)
changes only `ProgramStateCache.cpp`: the leading-NOP scan advances over its
literal, and END-NOP metadata includes that literal's size. It does not discard
draws, rotate caches, alter SPU colour reads or disable Read Color Buffers.
Other pre-existing private emulator modifications are retained.

## Regression and build evidence

[`rpcs3_fragment_start_audit.py`](../scripts/rpcs3_fragment_start_audit.py)
extracts the actual parser functions, opcode enum, destination union and metadata
declaration from the supplied checkout into a standalone C++ audit. Its `v128`
shim copies the same 16 bytes as the production load; it does not replace the
parser logic. The audit uses constructed instruction/literal fixtures, not a
capture of the bound shader that caused the GPU fault.

The original source fails **8 of 12** cases. The correction passes all 12:
ordinary MOV, ordinary leading NOP, leading and END NOP literals in each of
the three source fields, two successive NOP/literal pairs, ordinary instruction
literals, plain END NOP, and an END-looking bit inside skipped literal data.
The compiled audit retains the production declarations' MSVC nameless-union
and signed-sentinel warnings; no warning-free build is claimed.

From an x64 Visual Studio developer prompt:

```powershell
python scripts/rpcs3_fragment_start_audit.py --checkout <RPCS3-checkout> --out <new-private-audit-directory>
cd <new-private-audit-directory>
cl /nologo /std:c++20 /EHsc /W4 fragment-start-audit.cpp /Fe:fragment-start-audit.exe /Fo:fragment-start-audit.obj
.\fragment-start-audit.exe
```

The full incremental Release build produces the new emulator executable.
The previous deployed executable is preserved privately; the replacement's
SHA-256 is checked against the build output. `git apply --reverse --check`
accepts the patch against that checkout. JOIN125 is the fresh native comparison
using the same starter fixture, cache generation and renderer settings.

## Native comparison and remaining fault

JOIN123 logs malformed live fragment instructions and then device loss after
its corrected inventory view. JOIN124 records solid exterior armour, resolved
equipped icons and an empty Materials tab before this parser patch. It also
returns to the world and shows solid armour after the menu, with no recorded
fatal entry through that sample. That brief successful baseline round trip
does not establish sustained stability or prove the parser caused the crash.

JOIN125 uses the rebuilt parser with the same fixture, cache generation, SPU
colour handling and Read Color Buffers. It reaches the mission, shows solid
outdoor armour, resolves the equipped thumbnails and keeps Materials empty.
A bounded native recording at 17:54:15 captures the complete world → character
→ inventory → character → world sequence and fully decodes. A later character
hold of approximately 49 seconds, two additional inventory/character cycles,
return to the world and a movement input produce fresh rendered frames. The
17:57:26 native sample retains engine zero/current 112, all five correct
arrangements and their applied model roots.

The final pre-shutdown log reaches native time **17:04.145** without a recorded
fatal/device-loss entry. However, the same malformed TXD bytes first appear at
native **5:41.759**, before the recorded menu sequence. It records **19,991**
`Invalid Src type` messages and **51,037** refused malformed cache stores, with
the program creation counter reaching **+52,945/-0**. The cinematic-only sample
had remained near +2,008. JOIN124's old parser also completed a brief round trip
without device loss while recording malformed programs and rapid growth.
Different observation lengths and workloads prevent a rate comparison. These
results do **not** establish a live binding repair or sustained stability.

A read-only JOIN125 scan finds the big-endian float prefix
`3f99999a3ee666673f7f18b9` at `0xc01f1c20` and `0xc01f3f20`, inside blocks of
floating-point data. Their fourth word differs, consistent with changing data
entering the program key. This is a clue, not proof that either address was the
bound shader: the scan is non-atomic and 24 one-MiB chunks of local memory were
unreadable. Earlier JOIN124's broader-chunk scan found no match; that negative
sample does not contradict these later locations. Capture the original bound
program address, location, metadata start offset and nearby bytes next, then
trace the writer or binding that selects the block.

JOIN125's owned stack, proxy and outer launcher are verified exited; XInput is
restored. Keep bounded recordings finalized with `finally` cleanup before a
risky continuation. A screenshot failure must not prevent recorder finalization.
The inventory repair remains independently validated and pushed.
