# PS3 update export validation

Generated on 2026-09-28 with the default (no `-Packages`) `Invoke-FinalBuildExport.ps1`
workflow for BLES01857 versions 01.13 and 01.24. The export helpers came from
`p3n1cillin/d1xenia` branch `claude/brave-gauss-nqcibd` at commit
`4c3cd88eae3048c6b1b7ab6647d592c2f78cef19`.

The input 01.00 `EBOOT.elf` SHA-256 is
`e14e1cedaf85a10ef08a3d4b86d86ff942f222cd27fde9a68d612947169846c6`.
RPCS3 decrypted the 01.13 and 01.24 EBOOTs. Both outputs begin with ELF magic:

| Build | Decrypted ELF bytes | SHA-256 | Types recognized by static exporter |
| --- | ---: | --- | ---: |
| 01.00 | 24,853,536 | `e14e1cedaf85a10ef08a3d4b86d86ff942f222cd27fde9a68d612947169846c6` | 4,387 |
| 01.13 | 25,901,408 | `c0db242499726d46fc9213436a7ff8df03fba9d89e7a85e170799081c663b122` | 352 |
| 01.24 | 26,181,096 | `e7f100b38bbaf6c23f71b390fdb103d31b427187c38dcfbd47e09c94c4a44206` | 351 |

The `d1_types.py` scanner finds definitions compiled into each EBOOT using the
01.00 field-list layout. The much smaller counts in 01.13 and 01.24 have not
been explained or independently validated against a running updated game.
Consequently, the `diff_01.00_*.md` reports' large "removed types" counts are
**provisional**; they should not be read as proof that those runtime types were
removed from the game. The 01.13-to-01.24 comparison is between similarly sized
static exports, but it has the same coverage limitation.

The default run did not fetch the full update package set. These exports have
no update-build package service tables and no `final_*.txt` package listings.
Four branch unit-test files (`test_d1_builddiff.py`, `test_d1_eboot.py`,
`test_d1_types.py`, `test_ps3_update.py`) passed before publication. The tests
use synthetic fixtures and do not establish complete update-build type coverage.
