# PS3 update export validation

Regenerated on 2026-09-28 from the decrypted BLES01857 EBOOTs with
`p3n1cillin/d1xenia` commit `fe894860d5fe76c4fdfe195ee58ab62f421f5943`
(`scripts/d1_types.py` and `scripts/d1_builddiff.py`). The earlier exports used
the 01.00 type-definition layout for later executables, found only 352/351
types in 01.13/01.24, and falsely reported thousands of removed types.

| Build | Decrypted ELF bytes | SHA-256 | Types recognized |
| --- | ---: | --- | ---: |
| 01.00 | 24,853,536 | `e14e1cedaf85a10ef08a3d4b86d86ff942f222cd27fde9a68d612947169846c6` | 4,387 |
| 01.13 | 25,901,408 | `c0db242499726d46fc9213436a7ff8df03fba9d89e7a85e170799081c663b122` | 4,942 |
| 01.24 | 26,181,096 | `e7f100b38bbaf6c23f71b390fdb103d31b427187c38dcfbd47e09c94c4a44206` | 5,041 |

The corrected scanner detects the later definition layout: the field-list
pointer is at definition +0x34, and each field entry starts with a repeat
count. Fixed arrays are expanded back into individual fields for comparison.
Re-exporting 01.00 produced a byte-identical `types_01.00.json` (SHA-256
`1b677ba5073fa320e2973dc89f93bcddc6f17035e6f516e676eb609fa86c39a2`).

The corrected reports now find:

| Comparison | Matched | Structurally identical | Changed | Removed | Added |
| --- | ---: | ---: | ---: | ---: | ---: |
| 01.00 → 01.13 | 4,317 | 3,766 | 551 | 70 | 625 |
| 01.00 → 01.24 | 4,308 | 3,744 | 564 | 79 | 733 |
| 01.13 → 01.24 | 4,917 | 4,858 | 59 | 25 | 124 |

The matcher now reserves unchanged handles first and uses each new handle at
most once. Counts balance against the old and new export sizes. Matches with
changed handles remain heuristic: equal or similar structure alone cannot
prove two types have the same runtime purpose.

These are **static export comparisons**. They have not been checked against
type exports from a running 01.13 or 01.24 client. They also contain no
update-build package service table: the default export did not fetch the full
update package set. The separately documented 01.24 service-table analysis is
in `d1xenia/notes/final-build.md`.

Validation on the current code branch: 53 server tests passed (one skipped),
and 21 script tests passed. The tests use synthetic fixtures and do not prove
complete runtime type coverage.

Mission-state preflight on 2026-09-28 with `d1xenia` commit
`3b9c18ca4f347444b3958ad82461216a9efda808`:
`scripts/d1_mission_preflight.py`
encoded and decoded all 80 scripted state changes in `a_guardian_rises.json`
and `tower_npcs.json` against both the saved 01.00 activity schema and this
directory's corrected `types_01.24.json`; both passed. The 01.24 squad state
`0x8080071e` grew from 0x60 to 0x7c bytes. Its 2-bit, bias-1
existing-member field moved from +0x5c to +0x74. The former scripts named
+0x5c in 56 squad changes, which the 01.24 encoder silently omitted. The
code branch now resolves `squad_existing_members` to the offset for the
selected schema and rejects dropped fields before a run. Client behavior on
01.24 remains untested.
