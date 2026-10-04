# 01.24 continuation — 5 October 2026

Continue from the [4 October checkpoint](checkpoint-2026-10-04-mission.md).
The active target remains final BLES01857 01.24, character creation through
A Guardian Rises. The combined SPU surface tracking and color-buffer fix
is retained; disabling color reads previously distorted Guardian models.

JOIN103 independently verifies all twelve Ghost helper components and
the network binding's association with the actual Ghost. The binding's
unit-mask gate is set. The helper container itself belongs to a different
pool and is not the bound unit. This closes that specific diagnosis, while
torch activation and an unobstructed Ghost view remain open. See
[the light investigation](ghost-light-0124.md#join103-the-helper-binding-targets-the-physical-ghost).

The reusable read-only inspector is
[`rpcs3_final124_attachment.py`](../scripts/rpcs3_final124_attachment.py):

```powershell
python scripts/rpcs3_final124_attachment.py `
  --process-manifest <private-run-processes.json> `
  --owner <fresh-registration-r4> --target <same-run-Ghost-unit> `
  --asset 0x80a3d8ad --out <private-attachment.json>
```

Supply the owner from creator registration `0x1e5060` with HO return
address `0xc11ee0`. Do not depend on the small `0x1f8c1c` accessor being
hooked: LLVM may inline it. The inspector rejects mismatched asset,
owner, target, chain count, pool bounds, undersized binding fields and
structural/link changes. Its sample is explicitly non-atomic. All 127
script tests pass, including eight focused attachment tests.

The actual final device definition and handlers also explain why
`o_device_invis_light` is absent in JOIN103: its baseline requests no
creation. The new mission alias permits a manual generation/Boolean test
without adding an automatic action. The actual schema preflight passes
the unchanged 39 scheduled/reactive states and the separate candidate.

JOIN103's last sampled corridor frame is clean; this does not extend the
earlier inventory-model evidence to an in-world third-person Guardian.
The run is stopped by exact process ownership after saving its evidence.

Stable Breach return completion, visible torch, durable rifle pickup,
Fallen encounters, mission rewards/results, Tower arrival and relog
remain unfinished. Keep those acceptance claims separate from protocol,
attachment and rendering observations.
