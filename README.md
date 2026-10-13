# D1 Reimagined

Experimental, independent Destiny 1 preservation research and original local service implementation. **The complete game is not playable.** This source-only release includes local service stand-ins, protocol experiments, mission rules, diagnostic tools, scoped emulator patches and regression tests.

Project website: https://d1reimagined.com
Community Discord: https://d1reimagined.com/discord (questions, ideas and contributor discussion)

## Scope

The active target is **PlayStation 4 CUSA00219 version 01.29** with shadPS4.
The [PS4 source tree](ps4/README.md) now includes the Rust services, protocol
libraries, diagnostic tools, canonical harness and generators from the supplied
preservation project, with the latest preparation and content-reader fixes.
The retained PlayStation 3 implementation targets version 01.24 with RPCS3.
See [current results and contribution priorities](docs/STATUS.md). The exact
PS4 executable remains missing, and there is no accepted native PS4 boot here.
Native stability, rendering, combat, progression, complete activities and
multiplayer remain incomplete.

This is a source publication, not a game distribution or a ready-to-play installer. Game assets, firmware, executables, cryptographic materials, account data, private runtime inputs and captures are not included. Supply only inputs you are authorised to use. No public service, console, account or third-party system is a testing target.

## Layout

| Directory | Contents |
| --- | --- |
| `server/` | Loopback-only local service stand-ins, persistence and protocol tests |
| `scripts/` | Preparation, owned-run control, diagnostics and scoped audits |
| `missions/` | Original experimental mission and traversal rules |
| `patches/` | Scoped source increments for a separately obtained RPCS3 tree |
| `tools/content_capture/` | Optional private evidence capture and durable handoff tools |
| `tools/ps4_setup/` | Original preparation helpers and narrow patches for the PS4 runtime |
| `tests/` | Additional regression cases |
| `ps4/` | Active Rust preservation services, tools, research docs and canonical harness |
| `docs/` | Current contributor checkpoint and publication provenance |

## Development

Use Python 3.14, PowerShell 7 where applicable, and the declared packages:

```powershell
python -m venv .venv
.venv/Scripts/python.exe -m pip install -r requirements.txt
.venv/Scripts/python.exe -m unittest discover -s server -p 'test_*.py'
.venv/Scripts/python.exe -m unittest discover -s scripts -p 'test_*.py'
.venv/Scripts/python.exe -m unittest discover -s tools/content_capture -p 'test_*.py'
```

Some checks require additional privately supplied source or runtime inputs. Missing inputs, skips and blocked native acceptance must remain explicit. The publication audit checks source integrity and syntax; it does not certify that the game is playable or that every runtime integration can be reproduced from this checkout alone.

The retired global-process punch-sweep launcher is excluded. Use the current owned-run wrapper; never stop unrelated emulator or service processes. Private historical journals and raw evidence are not source dependencies and are not distributed.

Set `D1_LOCAL`, `D1_RPCS3`, `D1_USRDIR` and `D1_DATA_REPO` to your own private locations. The default ignored `local/` directory is for private inputs and generated state. Individual PS4 helpers expose path parameters; use their help and explicitly select your own installation rather than assuming example locations exist. Do not run native launch or input tools without inspecting their scope and effective containment. A Linux namespace does not contain a Windows process.

## Contributing

See CONTRIBUTING.md for small, reproducible changes and SECURITY.md for private security reports. Never attach game files, raw private traces, certificates, credentials or personal information to issues or pull requests. Published code does not grant website member access; that remains individually owner-approved.

## Licence and third-party rights

Original project source is offered under GPL-2.0-only unless a file carries a different existing licence notice, including GPL-2.0-or-later annotations. See LICENSE and THIRD_PARTY_NOTICES.md. Preserve upstream copyright and licence notices when applying patches. Source licensing does not cover Destiny assets, executables, firmware, trademarks or external software not included here.

D1 Reimagined is not affiliated with or endorsed by Bungie, Sony Interactive Entertainment, or RPCS3. Destiny and associated names and assets belong to their respective rights holders. There is no warranty, compatibility promise, legal immunity or announced playable release.
