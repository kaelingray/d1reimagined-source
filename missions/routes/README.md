# D1 Reimagined — experimental traversal routes

`enter_the_wall.json` and `breach.json` contain recorded player positions for the
**PS3 01.24** opening-mission research target. Coordinates are `[x, y, z]`, with
`z` up. `scripts/d1_traverse.py` records/replays them using the exact owned emulator
manifest. Run from the source repository root and select the appropriate private
runtime before use:

```powershell
python scripts/d1_traverse.py --help
python scripts/d1_traverse.py --manifest <run>-processes.json record missions/routes/<new-name>.json
```

Status reviewed **2026-10-06**: earlier native tests reach initial player control,
enter the Wall and show Breach views; pitched camera handling has been improved.
These breadcrumbs are traversal experiments, not a completed mission script.
Reliable slice handoff, Ghost lighting, weapon pickup, combat, objectives and mission
completion remain unverified. Rendering corruption can invalidate a visual traversal
comparison. A replay or position change alone is not progression acceptance.

Preserve original routes and record a separate file when testing a changed scenario.
Check native camera, movement and slice state against the
[runbook](../../notes/next-session-playbook.md) and
[mission acceptance criteria](../../notes/guardian-rises-acceptance.md).
