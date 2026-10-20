# Public source verification — 12 October 2026

These checks used the public working tree with no bundled game or firmware
inputs. They establish source/tool behavior, not native gameplay acceptance.

| Check | Result |
| --- | --- |
| PS3 Python service suite | 225 tests, 4 explicit skips for absent external inputs. |
| PS3 Python script suite | 176 tests, 2 explicit skips for the excluded derived label index. |
| Original capture tool suite | 82 tests pass. |
| PS4 package/scanner/platform bin tests | 7 tests pass on Windows and Linux. |
| Scanner controls | 9 pass on Windows. |
| Platform controls | 10 pass on Linux; Windows bin tests execute the same control set. |
| Tracer without GUI | 114 library tests, 9 integration tests and 1 synthetic-generator test pass on Windows. |
| Linux d1-bap, d1-run, d1-responder and shadNet | 263 tests pass, including 9 live loopback participant tests. |

Two native eboot golden suites require `private-fixtures` plus locally regenerated
files and are excluded from default public builds. A passing public check does
not verify any absent native inputs. The byte scanner, converter and logical PFS
results are summarized separately in [STATUS.md](STATUS.md).

The upstream shadNet schema is byte-identical to its attributed source hash
`61f4fef46c15b81c273ea0c29f5f7fb303542cf98d6fcbcbe7f0329594edac87`.
Source-only scripts carry executable bits for Linux checkouts. Imported source
whitespace is preserved rather than reformatted across pinned upstream files.

The public update adds a fresh source snapshot with ordinary commits. It does
not import private Git history or source data from `d1reimagined-data`. Private
history, working files and production evidence remain outside the public tree.

## Subsequent consolidation — 20 October 2026

The source-only scope above records the 12 October publication. The owner subsequently requested a single public repository including both private histories and their data. See [REPOSITORY-CONSOLIDATION.md](REPOSITORY-CONSOLIDATION.md) for the current layout and verification.
