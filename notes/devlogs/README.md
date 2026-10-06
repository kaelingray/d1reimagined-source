# D1 Reimagined — development evidence and historical drafts

Status reviewed **2026-10-06**. The development machine supplies original native
game/audio footage, screenshots and factual engineering context through the
[automatic capture/outbox system](../../tools/content_capture/README.md).
Normal operation requires no manual recording, naming, metadata, copying or retry
steps after private configuration. The separate production machine decides how to
group evidence and performs editing, narration, captions, rendering and publication.

Engineering results drive the evidence. Prefer comparable before/after tests and
continuous input-to-result demonstrations when they establish a real outcome.
Unchanged observations are deduplicated; a CPU test, bound actor or successful mock
reply does not prove visible gameplay. Keep timestamps, maturity, missing coverage,
remaining issues and privacy/audio/rights limitations explicit. Dev Blog identifiers
remain null until assigned downstream.

The current [October 6 checkpoint](../checkpoint-2026-10-06-renderer.md) records GPU
fault/conformance work and live capture verification. Guardian flicker/distortion,
combined body/world/mission rendering, Ghost illumination, combat and complete
mission progression remain open. A clean bounded validator run alone is not a fix.
The [mission acceptance checks](../guardian-rises-acceptance.md) define what footage
would need to demonstrate.

## Retained historical material

The October 5 Episode 01 drafts cover authored Ghost member/performance bindings,
a separate manual movement experiment and unfinished gameplay/rendering. Existing
review cuts use actual recorded gameplay in every section; they include reused
b-roll and synthetic draft narration and are not continuous complete playthroughs.
Their source/timecode manifests and media remain private.

The [Episode 02 inventory draft](02-2026-10-05-inventory.md) retains the tested
equipment-record/bucket repair and real before/after source footage. It is not a
renderer-stability claim or a finished release. `scripts/render_devlog.py` and its
tests remain historical tooling; they are not the current development workflow.
Previously drafted upload copy is not authorization to publish.

## Current handoff contract

Closed recordings are fully decoded and indexed with actual dimensions, distinct
reported average/base rates, audio availability, hashes and coverage gaps. Selected
sources and optional screenshot candidates enter schema-1 packages in a durable
outbox. The worker verifies every transferred payload before the atomic `.ready`
transition; completed packages are immutable and corrections require a new identity.
Earlier delivered footage can be referenced without resending it.

Native capture targets the game output. No automated visual secret or rights review
is available, so automatic footage is flagged sensitive and requiring downstream
review. Do not add private logs, machine/account identities, credentials or extracted
game assets to release material. Decoding and NAS delivery do not approve publication.
The capture README documents integration points, supported signals, retries and
the intermittent source-identification failure still under investigation.

Source: [d1reimagined](https://github.com/p3n1cillin/d1reimagined).
