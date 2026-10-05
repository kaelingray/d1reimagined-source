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
structural/link changes. Its sample is explicitly non-atomic. JOIN106
corrects the container header's configuration offset, initially mislabeled
as a count: the inspector now resolves the configured asset and reads its
actual component-array count. All 130 script tests pass, including eleven
focused attachment tests.

The actual final device definition and handlers also explain why
`o_device_invis_light` is absent in JOIN103: its baseline requests no
creation. The new mission alias permits a manual generation/Boolean test
without adding an automatic action. The actual schema preflight passes
the unchanged 39 scheduled/reactive states and the separate candidate.

JOIN103's last sampled corridor frame is clean; this does not extend the
earlier inventory-model evidence to an in-world third-person Guardian.
The run is stopped by exact process ownership after saving its evidence.

## JOIN104: placed-device lifecycle and reaching the Ghost

The fresh run repeats all 249 ready records, the automatic one-Ghost
count after manual reset, and the corrected manual filter/helper.
The public attachment inspector succeeds before and after HO generation
one: all twelve components remain, and the network binding still targets
the same Ghost with its unit-mask bit set.

The manual device candidate creates a physical unit within two seconds
at the actual configured position. Generation two with creation false
removes it and clears the tracked pair; generation three with creation
true creates a new datum. Both native creator entries corroborate this
lifecycle. A controlled comparison retains the nearby orange light
source in all three states, so this is not accepted as a torch fix.

Following recorded Breach points 72–74 onto the ramp resolves the
previous blocked approach. A subsequent ordinary-input walk reaches
height 18.47 near `(390, -792)`. The Ghost's shell and blue core are
clearly visible about five units away. The exact successful targets,
camera directions, lifecycle states and native entries are in
[the JOIN104 light note](ghost-light-0124.md#join104-placed-device-lifecycle-and-a-clear-ghost-view).

From this clear viewpoint, increasing the helper generation reaches
native handlers `0xc121bc`, `0x12e79c` and `0x431c14`, with the actual
Ghost and null payload. The state is consumed, but the controlled view
still does not verify a torch beam. Continue with the handler's first
type lookup (`0x8080206f`) and subsequent compatibility check (`0x8080236c`), and the
configured components' actual activation requirements. Do not infer
failed execution solely from the unobserved `0x432674` hook.

The 00:30:28 Titan inventory frame again has normal proportions and
clean armor lighting. The level-zero/loading slots and preview-only
rifle remain. The combined rendering fix is retained, with no new
in-world third-person acceptance claim.

Validation: 127 script tests, 156 server tests and the actual final
schema preflight pass. The owned run and separate proxy are stopped;
all recorded identities are checked exited. XInput is restored.

Stable Breach return completion, visible torch, durable rifle pickup,
Fallen encounters, mission rewards/results, Tower arrival and relog
remain unfinished. Keep those acceptance claims separate from protocol,
attachment and rendering observations.

## JOIN105/106: input profile and configured component count

JOIN105 stays at the title despite repeated scripted Cross presses: the
private emulator still uses the XInput profile restored after JOIN104.
`Invoke-Final124Join.ps1` now selects its existing keyboard automation
profile before launch. This is required by every scripted key in the
unattended join. JOIN105 is stopped by exact manifest ownership before
changing the profile. JOIN106 signs in and joins successfully with the
corrected launcher. Restore XInput after stopping the owned run as usual.

The attachment inspector's count correction reads the header's asset
and configuration offset, checks the configured root class and bounded
asset extent, then reads the root's actual component-array count. It
also rechecks that configuration during the non-atomic capture. Three
new regression tests distinguish count from offset, reject invalid
offsets/classes and detect a changed configured count. The 130 script
tests and 156 server tests pass; the launcher passes PowerShell parsing.

## JOIN106 stopping checkpoint

The live attachment check passes before and after generation one. The
Ghost's two associated roots have configured counts 45 and 12, both with
runtime configuration offset 12. Neither interface table supplies the
handler's first requested type `0x8080206f`. The native trace reaches its
query with the actual Ghost; a complete offline bucket scan agrees with
the live missing result. Successful query accessors are observed for
other callers during the same bounded watch. Thus consumed HO generation
does not demonstrate torch activation. Follow `nux_ghost` performance
kind 28/index 125 and actual light controls next. See the
[JOIN106 evidence](ghost-light-0124.md#join106-the-ho-generation-stops-at-the-first-component-lookup).

The 01:26:33 and 01:28:21 BST third-person emote captures show dark,
patchily reflective armor. The 01:27:43 inventory Titan remains normally
proportioned and lit. The combined SPU tracking/color-buffer changes
remain enabled, but the in-world Guardian requires further diagnosis.
The active game-specific configuration retains color/depth reads and
writes and RSX memory tiling. The first dump's matching screenshot shows
the emote had already ended, so it cannot diagnose the armor. Resume by
inspecting the second private dump `d1/dump/45918215523` and matching
`BLES01857_screenshot_2026_10_05_01_34_25.png`. Do not infer a GPU cause
before comparing an actual emote frame and the unfinished spawn/profile
effects. No new graphics acceptance is recorded.

At the user's request to stop for the night, JOIN106 and its separate
proxy are stopped by exact manifest ownership. All eight recorded
identities are verified exited and XInput is restored. Validation for
this checkpoint remains 130 script tests and 156 server tests, plus
PowerShell parsing and actual final-build preflight. Raw native captures
and dumps are preserved privately, with no new automatic mission action.
