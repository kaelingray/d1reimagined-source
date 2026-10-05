# 01.24 continuation — 5 October 2026

Continue from the [4 October checkpoint](checkpoint-2026-10-04-mission.md).
The latest continuation is JOIN115/116 below: repeated Ghost bindings/manual
movement and real gameplay recordings replacing the still-only devlog edits.
JOIN112/113 retain the guarded character-login retry and the identification of
the generation callback's damage component.
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
kind `0x28` (40 decimal)/index 125 and actual light controls next. See the
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

## JOIN107: daylight Guardian and the authored Ghost member

The second JOIN106 dump is now inspected against its 01:34:25 BST frame.
Its early albedo/normal buffers contain the environment without the emote
Guardian; a later target contains the affected Guardian. The sampled
floating-point buffers are finite. These guest-memory readbacks do not
prove that every GPU target contains current data or identify a faulty shader.

JOIN107's unobstructed daylight emote frames at 09:11:29 and 09:11:33 show
scenery through a transparent, refractive Guardian with blue/silver edges.
That supports investigating invisibility and incomplete character loading;
it does not establish their cause or exclude a renderer fault. The earlier
09:10:34/37 attempt is obstructed by the message of the day and is excluded.
No renderer settings or code are changed. Normal inventory proportions
still do not verify the in-world Guardian, and combined flicker/character
acceptance remains open. The private matching outdoor dump is `2052905136`.

The route reaches the Wall at 09:15:10, then all 249 Breach records become
ready. A manual inactive `3:1` reset at 09:15:48 is followed by the mission's
one-Ghost count at 09:15:52. Actor `0x26f58000` has physical unit
`0x67fb82ce`. The ramp and near-Ghost walks reach an unobstructed shell/core
view at 09:18:37, without a verified torch beam. Current 24 still has engine
state 3 and return target 112.

Performance `nux_ghost` is kind **0x28 (40 decimal)**, index 125, asset
`0x80cb825b`, State A `0x80800556`, with no State B. Its configured target
is the authored kind-2 member 95. The physical actor has no performance
binding at `+0x48`; the authored member has no actor at `+0x174`. The
performance's primary cue is also unset. See the
[native performance investigation](ghost-light-0124.md#join107-authored-member-and-performance-binding).
At this point the mission exposes manual member and performance aliases,
with no automatic action for either. JOIN109 below tests the member path.
Raw assets and live snapshots stay private.

JOIN107 and its separate proxy are stopped by exact manifest ownership.
All eight identities are verified exited, and XInput is restored before
starting the next owned run.

## JOIN108: skip only the opening Mars prologue

At the user's request, `Invoke-Final124Join.ps1` gains the opt-in
`-SkipOpeningCinematic` switch for replay runs. It checks exact process
ownership and an unchanged frontend stage `24 / character:prologue`,
then posts one 1.5-second Circle hold. Activity loading, revival and other
mission stages are ineligible. Omit the switch for a first viewing.
Future runs also save the eligible pre-input sample privately.

JOIN108 posts the guarded input at 09:35:10 BST; the client requests and
receives matchmaking at 09:35:10.696/697, rather than playing the remaining
opening. The first saved post-skip bootflow at 09:35:14 is activity loading.
The activity joins at 09:35:12; the clear 09:36:38 capture shows the
letterboxed Cosmodrome revival scene. Current slice 112 and engine state
zero are observed at 09:39:37, followed by the ordinary Wall route.
The 09:35:07 image is black during the frontend transition; it does not
provide an identifiable Mars movie frame. Acceptance is the guarded input,
immediate matchmaking, and retained mission revival.

Validation: all 130 script tests and 156 server tests pass. The final
schema preflight accepts all 39 mission state changes, including the
new manual aliases; PowerShell parsing and `git diff --check` pass.

## JOIN109: authored Ghost creation and binding

A private mission copy removes the automatic count-only Ghost action.
After ordinary Wall-route arrival at 09:54:49 BST, all 249 records are
ready. The inactive bubble reset `3:1` clears its blocking mask. At
09:55:42 the typed member State A is sent once:

```json
{"set":{"sq_nux_ghost__ghost":{"0":1,"4":-1,"5":0,"6":true}}}
```

At 09:55:44 the squad has exactly one live member while all eight desired
counts remain zero. Authored member 95 points to actor `0x71f58000`, with
physical unit `0x2afb82ce`. The actor has both member owner `0x6bfa20da`
and performance owner `0x1cfa20f2`, with zero offsets. The native creation
path passes the actual member component to `0x246df0` and binds that same
actor through `0x255a18`. The older count-only path passes null and leaves
these associations absent. See [the detailed trace](ghost-light-0124.md#join109-one-ghost-with-the-authored-member-and-performance-bindings).

The shipped current-24 reaction now uses this authored member State A
instead of the count-only Ghost state. Its trigger and 20-second delay
are retained. The regression checks the 42-bit member payload and rejects
an automatic squad-count path being enabled alongside it. No performance
cue or torch action is added; those still require visible acceptance.
JOIN109 is stopped, all eight owned identities are verified exited, and
XInput is restored.

JOIN109 also repeats the opening skip: its saved 09:48:39.701 pre-input
sample has current and target 24, `character:prologue`, and an unchanged
stage. The 09:48:41 input is followed by matchmaking at 09:48:41.908/909;
player control is subsequently reached. The switch now requires both
current and target to match, as well as the name and stability check.

JOIN110 fails during sign-in with `centipede`, before any activity host
or mission rule starts. A Circle dismiss does not recover within the
eight-minute deadline. All seven owned stack identities and the private
observers are stopped; XInput is restored before fresh JOIN111. This is
a sign-in failure, not evidence against the new mission state.

## JOIN111: the in-world character has its starter equipment

The final appearance wrapper `0x720198` calls builder `0x71f86c` with the
player record's normalized character data at `+0x840`. Final armor assembly
`0x71f46c` selects five gear records at character `+0x5f0`, stride `0x30`,
and reads item indexes at each record's `+8`. These are final-build
addresses and runtime fields, not the older peer-summary offsets.

The read-only 10:17 BST sample resolves the player-pool descriptor at
`0x1b29450`, whose header reports stride `0x2930` and one record. That live
record contains subclass 710, armor 204/198/200/206/202 and Ghost shell
862, with the expected instance IDs. Its weapon/ship/vehicle slots are
empty. The descriptor and data pointer are unchanged across the sample;
the sample remains non-atomic. This rules out an entirely empty in-world
equipment summary in this run. It does not prove armor art finished
loading or establish a rendering cause. Continue from the appearance
assembly and model-loading/effect path rather than adding the starter
equipment again. The private snapshot is `FINAL124_JOIN111-world-gear.json`.

## JOIN111: automatic authored Ghost state verified

The public mission runs with no manual member-state line. The ordinary
route reaches the Wall at 10:19:45 BST and the client's current-24 report
matches the rule at 10:20:03.950. All 249 records are ready before the
manual `3:1` reset. The rule sends the authored state at 10:20:24.150.
At 10:20:26 one physical Ghost exists with both member and performance
bindings, while all eight desired counts stay zero. The native trace
passes the actual member into creation and binds the same resulting actor.
This verifies the automatic rule after the existing manual reset gate.

The saved Mars pre-input sample again has current/target 24 and
`character:prologue`. Skip input at 10:13:38 proceeds to activity loading,
and actual current 112 is reported at 10:16:30. The mission revival is
retained. At this point the performance cue is unset; visible torch
behavior remains unverified.

The ordinary ramp walk reaches height 16.56 at 10:21:20. The close walk
then reaches `(390.099, -792.445, 18.467)` at 10:23:19; the camera
calibration stops when its pitch input does not move. The 10:23:37 frame
clearly shows the Ghost shell/core and nearby orange source, without a
verified beam.

The subsequent manual performance test resolves the actual category/key
layout before sending a state. Category global `0x1a2772c` matches the
asset's `0xafb11a12`; primary State A field `+0` must contain its action
key `0x9b1d6a71`, with empty override at `+4` and generation one at `+8`.
Native lookup returns true with indexes zero/zero, then reaches the unit
animation dispatcher. The same Ghost moves about 21 units in twelve
seconds, retaining both bindings and one live member. The later sample
still has processed generation zero, so completion is unverified. The
bright core is visible after dispatch, without a confirmed beam. This
action remains manual because its mission timing and light behavior are
not accepted. See [the performance trace](ghost-light-0124.md#join111-configured-performance-action-dispatches-and-moves-the-ghost).

The final 10:31:51 sample remains in-world. The owned JOIN111 stack and
separate proxy are then stopped; all eight recorded identities are checked
exited at 10:31:52 and XInput is restored. The join observer reports its
owned emulator exited after this explicit stop, not a spontaneous client
disconnect. Private observers have completed; captures and candidate files
stay outside Git.

Final validation: **130 script tests + 156 server tests = 286 passing**,
PowerShell parsing, `git diff --check`, and the actual final-build preflight
(39 intro states, 53 Tower states, 94 services and final starter gear).
The new member regression checks one authored rule, exact field values,
42 encoded bits, round-trip equality and absence of an automatic Ghost
count action. Graphics, visible torch, stable Breach return, rifle pickup,
combat, mission completion and Tower arrival remain open.

## JOIN112: recover a missed login and repeat the Ghost bindings

The 10:39:59 BST emulator remains responsive after the join wrapper's
seven-minute matchmaking wait expires. At 10:45:53 it is still in
`character:signin` (current/target 23). The verified stage vtable is
`0x013e8d58`; fields `+0x3c/+0x40` both equal 5. A bounded native watch
shows `0x3694d4 -> 0x35cbe0` returning 3, while the family manager's
account ID is present and active-character ID is zero. No matchmaking
request has arrived. This is a character-selection wait, not proof of
a crashed emulator or failed activity host.

A fresh Cross at 10:53:25 produces service 504 and a self delta naming
the active stored character. Matchmaking arrives at 10:54:15. The ordinary
route reaches the Wall at 11:02:11. After the existing readiness/reset
gate, the automatic rule creates one Ghost at 11:02:46, with zero desired
counts, member owner `0x25fa20da` and performance owner `0x21fa20f2`.
The actor is `0x52f58000`, unit `0x5bfb82ce`. This repeats JOIN111's binding
result without a manual member state. Current 24 still has engine state 3
and return target 112; settled progression is not accepted.

The private player-interface probe initially rejects an assumed `0x40`
unit stride. The actual descriptor has stride **`0xc0`**. After verifying
that layout, a fresh `0x138e24` hook identifies player entity `0x602cd380`;
exact datum resolution matches one occupied unit, `0x3dfb8296`. Its base
root `0x80a6a022` exposes interface `0x8080206f`, unlike the Ghost's base
and helper roots. Package metadata identifies this interface family as
**damage**, so it is not evidence that the helper should target the
Guardian. See [the correction](ghost-light-0124.md#join112-the-generation-interface-is-a-damage-component).
No helper state targeting the Guardian is sent.

All eight owned JOIN112 identities are verified exited; XInput is restored.

## JOIN113: guarded character-login recovery

`Invoke-Final124Join.ps1` now allows at most three Cross retries, at least
six seconds apart, only while the exact native character sign-in stage
is unchanged at 5/5 and no selection/create response is logged. Each
retry retains the full ownership check and saves its pre-input sample.
The reader verifies the stage vtable and rechecks its pointer and fields.
Setup, creation and cinematic stages fail the retry policy; the Mars
skip retains its separate current/target-24 gate and opt-in flag.

JOIN113 uses a private copy of the wrapper that withholds only the first
login Cross. At 11:16:02, its saved sample has `character:signin` 23/23,
the expected vtable, and unchanged 5/5 fields. **One** retry is posted;
504 answers at 11:16:02.989. A new prologue-24 sample precedes the Circle
skip; matchmaking answers at 11:16:08.472 and the host proxy is contacted.
There is no second retry. This exercises the recovery path without
manual input. This deliberately minimal join recipe subsequently enters
cleanup and displays **lobster**, so this run proves frontend recovery
and skip separation, not a successful mission join. The full mission's
binding evidence comes from JOIN111/112.

The script suite passes **131 tests** and the PowerShell retry policy
passes **18 cases**, including changing layouts/stages and answered
selections. The new PowerShell sources parse; `git diff --check` passes.
JOIN113's eight owned identities are verified exited and XInput restored.

## Devlog review material

The [episode manifest and publishing drafts](devlogs/README.md) document
the tested Ghost binding milestone and its limitations. Private outputs
contain a **52-second captioned vertical cut** and a **5:24 narrated,
captioned landscape cut**, with SRT captions and measured segment timings.
They use actual test stills, clearly labeled as a capture montage, and
Windows desktop synthetic draft narration. Both final MP4s fully decode
without errors; inspected frames have readable captions. The long cut's
48-kHz mono AAC audio measures -15.9 dB mean and -1.3 dB peak. Nothing is
uploaded, and no capture/audio/video is committed.

The next engineering task is the helper's actual lighting component
activation/transform path, alongside settled Breach progression. The
damage callback is no longer a proposed torch gate. Guardian distortion,
torch, rifle, combat, completion and Tower arrival remain unresolved.

## JOIN115/116: record real gameplay for the revised devlogs

The user requested footage and identified empty visual sections in the first
review cuts. Those still-image edits are superseded by the recorded edits in
[the devlog material](devlogs/README.md). RPCS3's native F11 recorder captures
game frames and game audio, including while the window is in the background.
The encoder's active output can initially be zero bytes; stop once and wait
for a probeable finalized MP4 before stopping the owned emulator.

JOIN114 stopped before launch: its outer launcher log used the public run
prefix and correctly tripped the existing duplicate-name guard. No emulator
or gameplay evidence comes from that attempt. Outer launcher output for the
fresh JOIN115/116 runs uses a separate prefix.

JOIN115 repeats the full revival-to-Wall recipe. Ordinary controller movement
returns `route_reached`, then the host teleport is queued at 11:48:08 BST.
At 11:48:58, all 249 Breach records are ready. After the external inactive
bubble-3 reset, the unchanged public automatic rule creates exactly one Ghost
at 11:49:26. Actor `0x61f58000`, unit `0x16fb82ce`, has member owner
`0x1cfa20da` and performance owner `0x26fa20f2`; all eight desired counts
remain zero. The camera route reaches a close corridor view using controller
input, preserving the same player identity.

At 11:55:39, a separate manual action uses the already verified configured
key `0x9b1d6a71`, empty override `0x811c9dc5`, generation 1. A fresh read
also verifies global category `0xafb11a12`. The same bound actor moves from
approximately `(390.458, -786.975, 18.975)` to
`(393.852, -766.086, 18.908)`—about 21.2 units. The uninterrupted recorded
view shows the orange Ghost changing to a blue core and moving away. This
repeats the manual dispatch result, without enabling an automatic cue. The
processed generation remains zero; no torch beam or completed action is
established. Native slice 24/engine 3 with target 112 remains unsettled.

JOIN116 records the retained revival after the guarded Mars skip and reaches
current slice 112 at 12:02:35, with a stable engine-zero appearance sample.
Daylight emote footage at 12:04:56 and a pitched-down view at 12:05:29 clearly
retain the transparent/refractive Guardian. JOIN115's inventory recording
shows normal model proportions. The comparison is across two owned runs,
and it demonstrates the remaining defect rather than a new rendering fix.

The eight owned identities from each run are verified exited, the private
JOIN115 observers/outer launchers are stopped, and XInput is restored. Native
recordings, selected clips and their recording/trim/crop index remain private.
Ghost close-ups are cropped detail views; traversal and appearance footage
retain the landscape game view. Some clips repeat as illustrative b-roll.

The renderer now requires video for every section, rejects missing/escaped
sources and invalid trims, and requires explicit permission in the manifest
to loop a short source window. Six media-validation regressions pass. PCM
intermediates and sample/frame clocks replace seeked AAC loops, which caused
timestamp warnings in the first video review attempt. Native game audio is
retained, lowered beneath synthetic narration in the longer edit. No video
is published or committed to the repository.

The final vertical edit is **52 seconds** (1080×1920); the narrated landscape
edit is **5:26.375** (1920×1080). Both are H.264 at 24 fps with 48-kHz stereo
AAC, fully decode without errors, and have reviewed frames for every section.
Image comparisons confined to the video area detect movement in all eight
short segments and all 38 long caption segments. The long mix measures
-18.8 dB mean and -1.7 dB peak. Private SRT/timing manifests align captions
and narration to output frames. All 137 script regressions, PowerShell input
script parsing and `git diff --check` pass. No new renderer/gameplay fix is
claimed from producing these videos.

## JOIN117–119: repair missing world armour selections

The [appearance investigation](guardian-appearance-0124.md) identifies a concrete
profile defect. JOIN117 has correct normalized Titan item indexes, but gear-slot
+0xc defaults to -1. Final conversion `0x46ed4c` resolves that field through
`0x2a6474`; no arrangement means no normalized model art. All five applied
armour-model roots are `0xffffffff`. The inventory's normal model preview did
not prove the world summary was complete.

The final client asset library supplies wildcard-class model arrangement indexes
for all fifteen starter armour pieces. JOIN118 changes only those five selections
in a private peer/select overlay. Five valid applied roots replace the invalid
ones, and the 12:59:28 BST daylight emote shows solid Titan armour. Its 13:01:04
character preview has normal proportions, with equipment icons still loading and
level/light zero. The existing SPU colour-read patch and Read Color Buffers setting
are retained; no shader or emulator change is made for this repair.

The server now includes these model selections in `starter:01.24:CLASS`, preserving
the 01.00 shorthand, inventory identities and equipped references. A replacement
item cannot inherit the original item's model. Custom JSON supports an explicitly
verified signed `arrangement` value, including zero. The four new regressions
cover all three classes, overrides, validation and the actual separate optional
wire field. All **160 server tests** and **137 script tests** pass. Peer/select
objects also roundtrip through the private final schema; the actual final-build
preflight and `git diff --check` pass.

JOIN119 repeats the normal full mission launcher with the public Titan fixture and
**no private overlay**. At 13:15:15, the live engine-zero/current-112 sample has
all five correct item/model pairs and the same five valid model roots. The
13:15:37 daylight emote visibly repeats the solid Guardian.

The user suggested the armour was stored on an inventory tab. The 13:16:19
capture confirms all seven starter entries, including armour and Ghost thumbnails,
are listed under **Materials** on that tab. This is an open inventory-category/UI
issue; profile model selection alone does not fix it.

JOIN118's RSX thread reports `VK_ERROR_DEVICE_LOST` at approximately 13:01:44.
JOIN119 reproduces device loss when returning from inventory to character, at
about 13:16:40, this time surfaced on a PPU job thread. Both reports arise in
Vulkan event waiting; that site does not identify the originating GPU fault.
The first successful world-armour views precede the crashes. Both native logs
are preserved; no crash suppression or speculative renderer change is shipped.
Combined menu/world stability remains unaccepted.

Native JOIN117/118/119 recordings fully decode without errors. Candidate and
standard-fixture trims are private under `APPEARANCE/2026-10-05`, retaining actual
moving game footage for review and future devlogs. No binary media is committed.
JOIN117/118/119 owned stacks and outer launchers are stopped and XInput restored
between runs. JOIN120 starts the same public fixture for an indoor appearance
check, avoiding further menu interaction until that check completes.

JOIN120's ordinary controller route returns `route_reached` at 13:28:48 BST,
with final position approximately `(352.469, -796.772, 14.064)`. A host teleport
is then queued for 24. Native samples at 13:29:00 and 13:30:14 retain all five
correct item/model pairs and applied roots, with current slice 24, engine 3,
target 112 and fresh unit datum `0x10fac63f`. The sampled Breach entrance is lit.
Two Down/emote attempts leave the sampled view in first person, so this does
**not** accept visible indoor body rendering. The return transition remains
unsettled, and why the emote view is unavailable is not established.

The uninterrupted JOIN120 native log has no recorded device-loss/fatal entry
through traversal and these indoor samples; the menu crash case is not repeated
in this run. Its native recording fully decodes without errors. At 13:32:32,
all eight owned stack identities, the entry observer and outer launcher are
verified exited, and XInput is restored. No live test stack remains.

## JOIN121–123: equipment records and inventory buckets

The user's inventory-tab observation leads to two concrete profile corrections.
JOIN121's self-family kind-3 pool has the verified item type `0x80801732`, stride
0xa8 and capacity 646, but no active instance IDs. The server had supplied
inventory summaries and equipped IDs without the individual records those IDs
resolve to. The family builder now emits a minimal record for each populated
instance in the merged self inventory, on snapshots and deltas.

JOIN122 accepts seven records with the correct owner, item index and quantity.
Its daylight emote retains solid Titan armour, but character thumbnails still
load and all entries remain under Materials. An initialized, valid native cache
rules out receipt/cache validity as sufficient proof of the UI repair. RSX device
loss follows at approximately 17:00 BST; later native values are frozen evidence,
not a fresh menu pass. The separate recording fully decodes.

Tracing the final investment globals supplies the second correction: inventory
is a fixed 320-entry array partitioned into bucket ranges. The fixture's packed
positions 0–6 are in Materials. Subclass belongs at 40, helmet 43, gauntlets 53,
chest 63, legs 73, class item 83 and Ghost 153. Inventory +0 is a mutation serial,
not item count; native insertion writes that serial to entry +0x14 and increments
it. The server now uses the verified final bucket starts, and its individual-item
builder scans sparse populated entries independently of the serial. Older layouts
keep their previous behavior. See [inventory evidence](inventory-items-0124.md)
for the type, getter, resource and native-update addresses.

JOIN123 uses the public starter fixture with no raw overlay. At 17:17, native
memory confirms the seven correctly placed entries, seven accepted records and
matching equipped IDs. At 17:19:23, current 112/engine zero retains the five
correct model arrangements, valid applied roots and a physical Guardian unit.
At 17:20:41, the character view shows the subclass, Ghost and all five armour
icons with a normally proportioned Titan preview. At 17:21:21, Materials is
**0/20**, with no equipment listed there. These are accepted UI improvements.
Displayed level/light remain zero, unset weapon slots retain indicators, and
subclass upgrades remain unverified.

RSX `VK_ERROR_DEVICE_LOST` repeats at approximately 17:21:25. Live fragment
decompilation logs the same float-looking malformed bytes described by the
[persisted-cache investigation](rpcs3-fragment-cache-validation.md) before the
crash. The cache guard protects preload/storage; it does not repair this runtime
binding. A later screenshot fails. JOIN123's recorder does not finalize before
that failure interrupts the input sequence; its preserved MP4 lacks `moov` and
fails decoding. Do not use it as video evidence. The screenshots and native log
establish the inventory pass, while combined menu/world stability stays open.

Four new family-wire tests cover starter instances, sparse bucket positions,
matching equipped IDs, snapshots/deltas, merged overrides with serial zero,
zero-instance entries, duplicate identities and unknown-build handling.
All **164 server tests** and **137 script tests** pass, along with the actual
final-build preflight (94 services, flags and 39 mission state changes) and
`git diff --check`. An earlier full server run under emulator startup load hit
two socket-timing failures; the focused teleport suite and subsequent full suite
pass. Both owned stacks and their outer launchers are verified exited, and
XInput is restored. No renderer configuration or shader patch changes in this
inventory repair. The next stability task is the malformed live program's
binding/lifetime, followed by the still-open mission progression.

## JOIN124–125: repeat inventory footage and parser boundary

JOIN124 repeats the public repaired fixture with the existing emulator. Native
video at 17:35:35 shows solid exterior armour, all equipped thumbnails and
Materials 0/20, and is finalized before the tab return. A 17:38:35 world emote
retains the solid Guardian after the menu. There is no fatal entry through
native time 13:16.760, but 7,374 invalid-source messages and program count
+19,606/-0 remain. A brief successful return does not settle menu stability.
The 18-second JOIN122 before clip and 17.77-second JOIN124 after clip are
reviewed H.264/AAC exports with native audio and full decoding passes. The
[Episode 02 draft](devlogs/02-2026-10-05-inventory.md) documents this inventory
milestone without claiming a completed mission or a stable renderer.

Source inspection finds a separate proven parser defect: the fragment metadata
scan skips leading NOPs without advancing over their declared literal data.
An END NOP also omits a declared literal from its reported lengths. The CFG
assembler already follows the correct layout. The
[parser patch and audit](rpcs3-fragment-start-literals.md) correct both cases.
The audit extracts the actual production functions/declarations; the original
fails 8 of 12 constructed fixtures and the corrected source passes all 12.
The full incremental Release build links, its output hash matches the deployed
copy, and the previous executable is preserved. Unrelated private emulator
changes and renderer settings are retained.

JOIN125 uses this parser with the same fixture/cache generation. Its 24.50-second
native recording at 17:54:15 fully decodes and shows world → Character → Inventory
→ Character → world. A roughly 49-second Character hold and two additional
tab cycles also return to fresh world frames with movement; the 17:57:26 sample
retains current 112/engine zero and all five correct arrangements/model roots.
No fatal/device-loss entry is recorded through native time 17:04.145.

However, the malformed TXD pattern starts at native 5:41.759, before the recorded
menu sequence. The final pre-shutdown log has 19,991 invalid-source messages,
51,037 refused malformed cache stores and program count +52,945/-0. The parser
repair is accepted independently; **the live binding fault and sustained menu
stability remain open**. Longer observation and different work prevent comparing
program-growth rates directly with JOIN124.

A non-atomic read-only RSX-memory scan finds the float prefix at `0xc01f1c20`
and `0xc01f3f20`, surrounded by float data with a changing fourth word. These
are candidates to correlate with the bound-program address, not established
shader bindings. Twenty-four one-MiB chunks were unreadable. The next renderer
probe should log original program location/address, metadata start offset and
nearby bytes at the malformed draw, then trace the writer/binding. The existing
persisted-cache guard continues to reject malformed storage; it cannot repair
the live source.

JOIN124 and JOIN125 owned stacks, proxies and outer launchers are verified
exited, with XInput restored. Bounded native recording finalization now uses
`finally`, so a failed screenshot cannot bypass its stop input. No media or
emulator executable enters Git.

## Shutdown and the reported Anteater

The user confirms Anteater appeared around 17:59 during JOIN125 shutdown,
rather than earlier in the menu test. The pre-shutdown observer
still records `activity:in_world` at 17:58:50; the activity host receives client
traffic through 17:58:57. The manual cleanup incorrectly uses shortened
`-Name JOIN125` instead of `FINAL124_JOIN125`, and the wrapper previously
reported that nonexistent run as stopped. Subsequent exact-identity cleanup
stops the proxy while the client is still briefly open. BAP remains active
through 17:59:16; the client then exits. This timing supports a shutdown-induced
disconnect. The confirmation concerns this cleanup event, not a general
explanation for Anteater. No renderer fatal appears in the saved pre-stop log.

The stop wrapper now rejects a name without either ownership record, writes the
cooperative stop marker for a recorded stack, detaches its debugger and closes
its client before its service processes or proxy. Shared ordering also applies
to normal run cleanup and failed stack startup. Every stop still checks executable
and exact creation time, preserving unrelated or reused-PID processes.

`Test-Final124JoinStop.ps1` uses sleeping owned children rather than a game or
network service. It verifies that an unknown name attempts no stops, the backend
is alive at client shutdown, the debugger is already gone, both backend and
proxy stop after client exit, stale identities preserve an unrelated child, the
cooperative marker is written, and repeat stops preserve the original evidence.
All these checks and the existing isolated `Test-Final124Run.ps1` suite pass.
This prevents the cleanup window that can visibly remove the local host from a
running client; it is not a general networking or Anteater repair.

## Zero-local renderer candidate and automatic diagnostic capture

The [bounded depth candidate](rpcs3-fragment-depth-only.md) adds an opt-in END NOP
for strictly guarded zero-local draws and passes 20 predicate/canonical fixtures
alongside the 12 actual-parser fixtures. The full Release build is deployed with
the previous executable backed up. JOIN129 samples 15 real canonical draws with
depth tests/writes enabled, including three stencil draws. Its log has no malformed
source but still reports Vulkan device loss at 7:55. Keep the marker absent outside
owned experiments; combined visible world/menu/body acceptance remains outstanding.

The [automatic source pipeline](content-capture-acceptance.md) is now the normal
devlog workflow, using the 500 GB NAS share. The next increment repairs reading the
open native Windows log, preserves earlier and incomplete diagnostic lines, clears
recovered sample errors and reduces repository polling. Capture and transfer remain
independent of the primary development process. Experimental renderer observations
and persistent crashes must retain distinct maturity labels.
