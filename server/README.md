# D1 Reimagined — local Destiny service stand-ins

These tools support the retained **PS3 BLES01857 01.24 / RPCS3** implementation.
Active engineering switched to the separate PS4 01.29 tree on **2026-10-11**;
see the [PS4 checkpoint](../notes/ps4-transition-2026-10-11.md).
Last status review: **2026-10-11**. The source repository is
[d1reimagined](https://github.com/p3n1cillin/d1reimagined); schemas and restricted
research inputs are in [d1reimagined-data](https://github.com/p3n1cillin/d1reimagined-data).

The current local flow signs in, persists a stored character/setup answers, enters the
local activity-host flow and has reached the opening mission in bounded native tests.
The services are experimental stand-ins. Schema-valid default replies, host reports
and a world-entry label do not establish playable missions or a complete backend.
Ghost illumination, enemy-kill acceptance, rewards, progression, complete
inventory semantics and multiplayer remain open. Native rifle pickup/firing,
mission-spawned Dreg attacks, player death and respawn now have bounded PS3 evidence.
See the
[current project status](../README.md), [renderer checkpoint](../notes/checkpoint-2026-10-07-renderer.md)
and [mission acceptance](../notes/guardian-rises-acceptance.md).

The opening mission uses native cinematic-end incidents for handover and automatic
Breach squad/dialogue rules. Equipment summaries now separate unit `appearance`
from model `arrangement`; explicit custom values survive parsing and encoding.
The [Ghost comparison](../notes/ghost-appearance-0124.md) records valid base/cache
lookups alongside incorrect images and a reproducible Nav guest access violation.
The 01.24 starter Generalist Shell now sends its art record's world appearance (row 0)
and Ghost dye 15/2493, which shows the authored shell in the revival (JOIN201). The October 10
checkpoint passes 225 server tests and 176 script tests; serialization and native lookup success are narrower
than visual or end-to-end mission acceptance.

Service 601 (world-loot claim) is answered: a claim matching a pending self record by
source kind and nonce clears that record, equips the item from a small art table (the
Khvostov 7G-02 so far), records one durable instance per nonce in the player database, and
publishes the next self and peer versions. In the opening mission this puts the rifle in
first person after the native pickup (JOIN226), using a private synthetic pending record.

Listeners bind only to loopback addresses. The DNS sinkhole answers locally and never
forwards upstream. Private CA material, session keys, stores, schemas and captured
requests/logs belong in `local/SIGNON_LOCAL` or the runtime selected by `D1_LOCAL`.
RPCS3's DNS configuration must select the local sinkhole. Local RPCN is an additional
sign-in dependency; preserve an existing shared instance when stopping owned tests.

| Component | Current role |
| --- | --- |
| `dns_sinkhole.py`, `hosts.py` | Local DNS and per-service loopback mappings. |
| `stun_stub.py` | Demonware NAT/IP-discovery replies used by the local startup path. |
| `signon_probe.py`, `signon_proto.py`, `auth3_proto.py` | SignOn/auth3 test handshake and encoded local session responses. |
| `sslv3.py`, `local_ca.py`, `game_ca_patch.py` | Legacy client TLS compatibility and private test trust setup; runtime patch identities retain their existing names for compatibility. |
| `lobby_probe.py` | Local Demonware lobby/session protocol experiments used by the join harness; complete online services are not implemented. |
| `bap_server.py` | BAP handshake/encrypted requests, schema-selected responses, local character store, equipment records and activity-host experiments. Many replies are defaults or mocks. |
| `bap_server.py` activity-host proxy mode | Local replicated-state/baseline feed, owned heartbeat and mission-control experiments; client acceptance must be observed independently. |
| `investment.py` | Schema-driven bitstream serialization used by the selected build. |
| `prepare_rpcs3_copy.py` | Preparation of a separate private emulator copy and test configuration. |
| `run_log.py`, `session_listener.py` | Private event analysis and connection capture. |

## Run and verify

Use the PowerShell 7 launchers rather than treating a listener banner as a successful
run. Pass explicit paths for imported runtimes and a fresh name for every experiment:

```powershell
pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 `
    -Name FINAL124_UNIQUE_RUN -Local <private-runtime> -DataRepo <d1reimagined-data-checkout>
pwsh -NoProfile -File scripts/Invoke-Final124Join.ps1 `
    -Name FINAL124_UNIQUE_RUN -Local <private-runtime> -DataRepo <d1reimagined-data-checkout> -Stop
python -m unittest discover -s server -p 'test_*.py' -v
```

Commands above run from the source repository root. `D1_DATA_REPO` is an alternative
to `-DataRepo`; sibling checkout lookup prefers the new name and accepts the legacy
folder name. The [runbook](../notes/next-session-playbook.md) covers build/schema
preflight, current host arguments, read-only observations and controlled experiments.
The 01.24 intro defaults are activity 229 / checksum `0x8fe07299`; older 01.00 examples
and address tables remain historical references, not interchangeable configurations.

The [state-preserving heartbeat investigation](../notes/rpcs3-world-heartbeat.md)
records a loaded-but-inactive target slice and the missing settled handoff. Avoid
inventing transport acknowledgements or promoting a mock state transition to native
acceptance. Renderer/device-loss issues can precede that handoff and are tracked
separately. Capture runs asynchronously when configured; its failures do not fail a
game launch, and all automatic media remains private review input.
