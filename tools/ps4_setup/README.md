# PlayStation 4 preparation helpers

The active Rust runtime source now ships under [ps4/](../../ps4/README.md).
Use explicit paths to that tree; its source fixes are already applied.

`d1-byte-scan.rs` provides nine bounded planning/scanning controls and three
unit tests. `Export-D1RepackSlack.ps1` reuses the separately installed licensed
PFS reader to export logical nonpayload ranges. All outputs stay private;
neither tool accepts a native executable. `d1-synthetic-gate.rs` generates
the public gate fixture through the existing tracer encoder, with two
positive/rejection controls and no native input. These Rust sources also
ship in their matching crates in `ps4/`.

Original Windows/WSL preparation helpers and scoped patches for the preservation
runtime. External tool installations, console inputs and game data are not
vendored here. Select your own installation using each helper's path parameters;
do not assume an example drive path exists.

The helpers support exact-input checks, content-only validation, planning and bounded server-only checks. Those results do not prove executable availability, native game boot, console isolation, Windows guest transport or gameplay. Never substitute a different executable to satisfy an identity gate, weaken containment, or label an incomplete readiness check as passed.

The package-owner and canonical-preparation patches must be applied only to their expected source revisions. Preserve unrelated changes and retain existing upstream licences. External reader modifications require their corresponding source and notices when distributed; this repository supplies no reader executable. See THIRD_PARTY_NOTICES.md.
