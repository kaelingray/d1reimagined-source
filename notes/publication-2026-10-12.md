# Public contribution checkpoint — 12 October 2026

The user authorized public source publication for Destiny and Aster Chess.
The existing private Destiny histories were not made public or imported into
the source release. The companion game-data repository remains private.

- Public Destiny: https://github.com/p3n1cillin/d1reimagined-source
  at `971918ddf14daeab1f75ce60d78d256363bcfe50`.
- Public Aster Chess: https://github.com/p3n1cillin/aster-chess
  at `dea202465adb57f2cb4e9ddc9aa2d2e70dfb88d4`.

Both repository heads were verified through the unauthenticated GitHub API.
Issues are enabled. Aster's existing Windows/Linux CI passed at the published
head; the new Destiny source workflow is tracked in the public release.

## Destiny source

The public `ps4/` tree contains 346 source/doc files from the supplied preservation
snapshot plus the local fixes. Existing PS3 source remains at the repository root.
Current status, contribution priorities, provenance, attribution, source checks
and build instructions ship alongside it. Linux script executable bits are set.

Excluded: game/firmware binaries, generated native fixtures/catalogues, raw run
evidence, private machine/operator instructions, external tool installations,
build output and all private Git history. Full reachable-blob targeted scans
found only an existing public rejection test's literal private-key header;
manual review confirms it has no key body. The scanner's location-only report
and reviewed finding remain outside the public tree. This is a scoped review,
not proof that arbitrary future contributions cannot contain secrets.

Public-only adaptations keep native eboot golden suites behind an explicit
`private-fixtures` feature, and mark two PS3 derived-label tests as skipped when
their excluded index is absent. The gate fixture is newly generated synthetic
data through the existing tracer encoder, with zero registers except synthetic
RAX, never copied from the native trace. Its source generator is retained here
as `tools/ps4_setup/d1-synthetic-gate.rs` and installed in the separate tracer.
These adaptations do not change the private canonical native runtime.

Public checks: 225 PS3 server tests (4 skips), 176 script tests (2 skips),
82 capture tests; 7 package-bin tests on Windows/Linux; 9 scanner controls;
10 platform controls; 124 tracer tests on Windows; 263 Rust service tests
on Linux, including 9 loopback participant tests. The attributed shadNet schema
remains byte-identical. The exact PS4 executable/native gates remain open.

## Aster Chess

Its existing source/research history is preserved. A full targeted scan covered
all 2,011 reachable historical blobs (619,061,092 bytes) with no matches or
suspect credential paths. Original Aster source now has GPL-3.0-or-later terms;
existing Stockfish source notices and GPL text remain, and the development-only
python-chess oracle tables have their upstream license beside them. Contribution
guidance and a PR template were added. Immutable research artifacts were unchanged.

Release build: no warnings/errors. All 60 engine and 10 play tests pass, depth-7
benchmark completes, independent checks cover 2,614 positions/14 reference perft
positions, and the browser HTTP/SSE/native replay smoke test passes. The NAS chess
checkout fast-forwarded to the same published head without local changes.
