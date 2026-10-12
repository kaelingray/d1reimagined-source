# D1 Reimagined — PS4 preservation source

The active target is **Destiny 1, CUSA00219, patch 01.29**, using shadPS4 and
the local Rust services in this tree. **The complete game is not playable.**
This source publication includes the supplied preservation project's servers,
protocol libraries, diagnostic tools and generators, with our tested preparation
and content-reader fixes. Game inputs, firmware, generated catalogues, native
fixtures and private run evidence are excluded.

## Current local checkpoint — 12 October 2026

All 1,647 retained 01.29 content packages were recovered from the owned local
repack: all tables/layouts and 701,164 stored-block hashes pass. The regenerated
English manifest matches the pinned reference. The contained server-only run
passes 19 checks. Neither result establishes guest transport or a native boot.

The exact executable is still missing. The available later US and original EU
executables fail its identity/layout gates; a complete scan of logical repack
nonpayload space found no hidden executable or build marker. Do not substitute
those executables or relax the target pins. See [the current evidence summary](../docs/STATUS.md).

The supplied project's `status/` and `design/` documents preserve its August 2026
research checkpoint. Its Guardian-in-world observations involved explicitly
synthetic interventions and a different operator's inputs. They are historical
research, not successful runs on this installation. Private citations may name
files that are intentionally absent from this source publication.

## Build without game inputs

Use stable Rust and Cargo. Package/scanner development needs no game files:

```sh
cd ps4/d1-server
cargo test --locked -p d1-pkg --bin d1-byte-scan --bin d1-platform --bin d1-pkg-header-audit
cargo run --locked -p d1-pkg --bin d1-byte-scan -- controls
cargo run --locked -p d1-pkg --bin d1-platform -- controls
```

Broader servers need the dependencies listed in [BOOTSTRAP.md](BOOTSTRAP.md),
including `protoc`, a C/C++ toolchain and CMake for some analysis crates.
The `d1-trace` and `d1-observe` crates are standalone; build them separately.
For the decoder and synthetic controls without a GUI, run from the repository root:

```sh
cargo test --locked --manifest-path ps4/d1-trace/Cargo.toml --no-default-features
cargo run --locked --manifest-path ps4/d1-trace/Cargo.toml --no-default-features --bin d1-synthetic-gate -- controls
```

The gate fixture is original synthetic data generated through the existing
tracer encoder, with zero registers except synthetic RAX. It is never native
evidence. Its generator and rejection control ship as source. Two eboot golden
integration suites require the opt-in `private-fixtures` feature and locally
regenerated files under `d1-eboot/tests/fixtures/`. They are excluded from default
public checks; never call that a native verification pass.

## Owned-input and native work

Follow [BOOTSTRAP.md](BOOTSTRAP.md) for the input/generator order, then inspect
the canonical `capture-rig/run-boot.sh` plan and effective settings. That guide
records upstream checks from 11 August; it is not a promise of a boot here.
Use `tools/ps4_setup` from the repository root with explicit paths to this `ps4/`
tree and your private tool/dump locations. Defaults describe the development
installation and must be adapted. The already-applied source fixes mean their
hash-guarded patch installers are unnecessary in this public checkout.

A Linux network namespace contains Linux children only. Windows guest isolation
and transport remain unaccepted. Never send a game client to public services
as a debugging shortcut. Do not post game files, decrypted inputs, certificates,
account records or raw private traces in issues or pull requests.

`d1-server`, the original PS4 tools and their existing notices retain
**GPL-2.0-or-later**; see [d1-server/LICENSE](d1-server/LICENSE) and
[attribution](d1-server/ATTRIBUTION.md). The repository's GPL-2.0-only default for
the retained PS3 source does not replace these notices.
