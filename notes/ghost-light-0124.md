# 01.24 Ghost light and nullable fields — 3 October 2026

This is static evidence from the decrypted final BLES01857 01.24 ELF and the
latest local package 025c chain. It identifies the required wire layouts; it does
not establish that the Ghost torch has appeared or the power beat has completed.
The ELF SHA-256 is
`e7f100b38bbaf6c23f71b390fdb103d31b427187c38dcfbd47e09c94c4a44206`.
Raw definitions and game assets remain private.

## Objects and state types

Each entity definition carries its actual replicated ID. These are independently
matched against the corrected per-record names in group `0xfc0e45a3`:

| Name | Kind / index | Definition tag | State A |
| --- | --- | --- | --- |
| `o_device_invis_light` | `4 / 8` | `0x80cb82a9` | `0x808007d9` |
| `sq_nux_ghost` | `1 / 94` | `0x80cb82d3` | `0x8080071e` |
| `sq_nux_ghost__ghost` | `2 / 95` | Member component in `0x80cb8243` | `0x80800868` |
| `ho_ghostlight` | `0x18 / 101` | `0x80cb8244` | `0x8080065c` |
| `pt_nux_ghost_light_up_start` | `0x1d / 120` | `0x80cb8257` | `0x80800463` |
| `pt_nux_ghost_light_up_continue` | `0x1d / 121` | `0x80cb8258` | `0x80800463` |
| `pm_nux_ghost_light_up_fly_to_gun` | `0x1c / 123` | `0x80cb825a` | `0x808006eb` |
| `of_ghostlight` | `0x21 / 124` | `0x80cb8212` | `0x80800750` |
| `nux_ghost` | `0x28 / 125` | `0x80cb825b` | `0x80800556` |

The squad-definition tag and the member entity tag are separate; a reference to
member 95 also appears in the performance definition. Their presence is not a
reason to send kind-2 member state from the stand-in: the locally simulating client
spawns and reports those members when the squad generation and bubble gate work.

`ho_ghostlight` is **not** a player trigger. Its 0x60-byte state contains a bool at
`+0`, a biased i32 at `+4`, an object reference at `+8`, and a required field of
primitive kind `0x22` at `+0x10`. The definition also declares state type
`0x8080048a`. JOIN92's live record and its actual root component now confirm
that this is state B and its default baseline requires a B-presence bit.
The former exporter omission malformed the subsequent object stream; see
[the baseline investigation](private-host-investigation.md).
The bool, generation, reference target, and dynamic payload that produce the retail
torch still need to be confirmed.

`of_ghostlight` contains a bounded list with up to eight fields of primitive kind
`0x22`. Sending a nonempty filter therefore also requires correct typed-value
encoding. Its apply handler at `0x215f28` copies the 0x190-byte state to component
`+0x110`; it is not a direct light-on bool.

The performance state has optional `0x80800538` at `+0` (two u32s and an i32), and
an optional bounded list at `+0xc`. Those integers must not be guessed from the
name. The fly-to-gun marker state holds an object reference and an i32.

## The confirmed serializer gap

The final primitive reader tables at `0x1898778` and `0x18988c8` both select:

| Primitive kind | Final reader | 01.00 reader | Wire behavior |
| --- | --- | --- | --- |
| `0x17` (decimal 23) | `0xe45198` | `0xd1ccb8` | One presence bit; present adds a raw u32. Absent produces native `0xffffffff`. |
| `0x22` (decimal 34) | `0xe45604` | `0xd1d124` | Decode a nullable type handle, then decode the selected type if the handle is not `0xffffffff`. |

At `0xe45628` the kind-0x22 reader loads the definition for `0x80800046`, a
four-byte type whose only field is kind `0x17`. It calls `0x1db1a8`, compares the
resulting handle against `-1` at `0xe45640`, and, when present, calls `0x1db1a8`
again with that handle and destination `+0x10`. The generic resolver
`0x278d6c` resolves the supplied handle through the same type/tag table used by
ordinary nested types. Present values are encoded only for handles found in the
selected schema; unsupported handles are rejected.

The old Python encoder treated both kinds as width-zero integers and emitted no
bits. A required null field actually consumes **one zero bit**. At the end of
`ho_ghostlight`, that omission shortened the 89-bit state to 88 bits. In the middle
of another structure it moved every subsequent field one bit earlier. Padding a
shortened packet is not equivalent to encoding the missing field.

`server/investment.py` now handles both kinds in encoding and decoding. A nullable
handle accepts `None`, `-1`, or `0xffffffff` for null; numeric zero remains a
present handle. A typed value accepts null or
`{"type": HANDLE, "values": {OFFSET: VALUE}}`. The type can be an integer or a
base-prefixed string. Unknown types and malformed mappings fail explicitly.
Optional absence and present-null remain distinct.

On 4 October, the mission/control offset normalizer was extended to preserve
exact `type`/`values` wrappers and recursively normalize their payload offsets.
The preflight checker also accepts those structural path labels. Ordinary named
field keys and malformed wrappers still fail, and unknown selected handles still
fail schema validation. Six new regressions cover object specifications, timed
and reactive steps, live control, nested wrappers and rejection cases. All 155
server tests pass. A private check with the actual final schema encodes the HO
state with selector `0x80800a07` and its squad reference in exactly 176 bits;
encoding success does not establish visible torch behavior.

Eight regression tests cover exact bits, neighboring sentinels, nulls, known
nested values, optional presence, malformed/unknown values, and truncation.
All eight fail against the previous implementation (13 failed assertions and
four errors across subtests). The relevant real `0x8080065c` fields match in the
01.00 and 01.24 schema exports; the nested object-reference kind field grows
from six bits in 01.00 to seven in 01.24. The cross-build fixture covers both
widths, and a separate private check used both full exported schemas.

## Safe trigger arming

The final trigger apply at `0xc13974` copies eight bytes to component `+0x10c`.
Update `0xc13b40` checks the armed byte there and compares the last report time at
`+0x108` with the arming time at `+0x110`. Each arming needs a later host tick;
use `--ah-sim-clock` and `"$now"`.

The current mission file already defines the corrected start and continue
aliases. A live control line can arm the start volume using:

```json
{"set":{"trig_ghostlight":{"0":true,"4":"$now"}}}
```

Wait for a type-18 incident carrying `0xfc0e45a3 / 0x1d / 120`, then disarm it
with `{ "0": false }`. The analogous continue trigger is index 121. They refer
to volumes 173 and 174 respectively. Arming a trigger only enables its spatial
report; it does **not** itself turn the light on. Do not arm both solely to force
the narrative order.

The activity trigger `0x992bb471 / 0x1d / 37` and cinematic `pf_104a_pwr`
(`0x992bb471 / 6 / 10`) are a separate power beat. The mission currently starts
that cinematic when its trigger reports. It does not arm the two local Ghost-light
triggers or publish the verified torch/performance action yet. The serializer fix
removes a prerequisite defect while those action values remain under investigation.

## Helper-object selector candidate — independent check on 4 October

The following is a bounded live-test candidate, not an automatic mission action
or a confirmed retail torch sequence:

```json
{
  "0": false,
  "4": 0,
  "8": {"0": 0, "4": -1, "6": -1},
  "0x10": {
    "type": "0x80800a07",
    "values": {"0": {"0": 4228793763, "4": 1, "6": 94}}
  }
}
```

The numeric group value `4228793763` is `0xfc0e45a3`. Numeric payload fields
require integers; the selected `type` handle separately accepts a hex string.
This is state A for helper `0xfc0e45a3 / 0x18 / 101`, with its confirmed B
declaration retained in the mission's `ho_ghostlight` alias. For a newly launched
proxy, wrap these fields in `{"set":{"ho_ghostlight":FIELDS}}` in its live
control file. The alias adds no automatic state change. An already running proxy
does not reload the mission's object aliases, and explicit object-head strings
are not accepted as arbitrary control names.
The reference at `+8` is
explicitly absent: `0x155f60` tests both kind and index against `-1`. A numeric
zero or a default object mapping is not this absent reference. With it absent,
`0xc11ef0` takes the typed-selector branch at `0xc11fe0` rather than the
object-filter branch. There is therefore no need to invent a filter default.

The full private 5,041-type final schema defines `0x80800a07` as eight bytes,
containing one object ID. Static dispatch at `0xc738b0` compares the typed
header with values read through `0x18df230`, `0x18df234` and `0x18df238`.
Those globals lie outside the ELF's saved data. An owned read from JOIN96
independently confirmed their values:

| Guest global | Selected handle | First-selector handler |
| --- | --- | --- |
| `0x1ab1280` | `0x80800a07` | `0xc73c80` |
| `0x1ab1290` | `0x80800dfe` | `0xc73e68` |
| `0x1ab12a0` | `0x80800c8f` | `0xc73b1c` |

For `0x80800a07`, `0xc73c80` resolves the payload's object ID through
`0x155c50`, then initializes iteration with `0x245590`; the next-item path
uses `0x245504` and `0x24553c`. The helper calls `0xc11dc0` for each selected
existing member yielded by the iterator. Referencing squad 94 requests
enumeration of its live members; JOIN99 below records the one-member limit.
publishing a kind-2 member state is not a prerequisite. The squad must actually
spawn before the iterator can yield a member. Merely sending a generation
does not prove that the bubble gate or squad handler accepted it.

The proposed false bool leaves periodic creation attempts enabled:
`0xc12034` checks the bool at component `+0x110`, and false reaches
`0xc11ef0`. Applying a changed 0x60-byte state at `0xc12144` marks the
component dirty; `0xc1238c` then invokes creation before comparing the
generation at `+0x114` with its previous value. Thus changing the selector can
reach creation while that generation remains zero. This does not establish
that zero matches the retail sequence's later generation-driven action.

There is a real configured asset behind this action. The actual helper
definition `0x80cb8244` has its root component at `+0x9e4`; component field
`+0x44` is tag `0x80a3d8ad`. Its package entry class is `0x80800734`, while
its saved header at `+8` and the runtime descriptor contain `0x80800906`.
Those identifiers must not be conflated with an identified light component.
Constructor `0xc11924` reads that field and initializes component `+0x180`
through `0x1f81a0`. For a selected member, `0xc11dc0` checks the descriptor
and existing attachments before calling `0x1f7c94` with that member as its
target. A successful returned object is linked to the helper's existing-object
list. This proves a configured attachment/spawn path; it does not independently
identify a named light component, its illumination parameters, or prove visible
torch light. The asset contains several component definitions, so its resource
class alone must not be presented as a verified light class.

Useful bounded watches after the helper state is applied are:

```text
c11ef0 8 r3+0x110:96
c12010 12
c11dc0 12 r3+0x110:96
c11e54 12 r4+0:32
c11e5c 12
```

At `0xc11ef0`, `r3` is the HO component and `+0x110` holds its state A.
At `0xc12010` and `0xc11dc0`, `r3` is that component and `r4` is the selected
existing-member **datum handle**, not a guest pointer. At `0xc11e54`, `r3`
points to stack output storage, `r4` is the validated descriptor at component
`+0x180`, `r5` is the selected member and `r6` is `0xffffffff`. The call
enters `0x1f7c94` with those same arguments. The HO caller watch avoids counting
unrelated callers of that shared creator. At `0xc11e5c`, `r4` is the returned
created-object datum, or `0xffffffff` on failure; do not treat it as a pointer
for a memory dump. These observations distinguish an empty selector, a rejected
spawn and a created attachment. A visible illuminated Breach still requires a
live frame.

## JOIN98: selector accepted; no attachment created

JOIN98 reproduced Breach readiness with 249 occupied records and zero pending
baselines, including the Ghost squad, member component and helper. A transient
empty pool during handover was rejected as a readiness result: zero pending
records alone is insufficient without the required IDs.

The inactive `3:1` reset at 18:56:47 BST cleared the waiting mask, and the
explicit Ghost generation was accepted at 18:56:56. At 19:04:00 the helper
candidate above was accepted through live control. Twelve fresh `0xc11ef0`
observations show its actual component state containing the absent reference,
selector `0x80800a07` and squad `0xfc0e45a3 / 1 / 94`, exactly as encoded.
There are no hits at `0xc12010`, `0xc11dc0`, `0xc11e54`, `0xc11e5c` or the
shared creator `0x1f7c94` in that bounded capture. Frames at 19:04:04,
19:04:18 and 19:04:38 show the same windowed Breach interior, with no new
visible light. The candidate is therefore not an automatic torch action.

Before the helper action, the Ghost squad's live-member count at `+0x1d8`
was zero. The iterator checks that count at `0x245434..43c`; a generation
handler hit does not establish a physical member. The final squad state's
desired member counts at `+0x2c` require a separate check. See
[the squad-count investigation](squad-member-counts-0124.md).

The type-6 body after the reset contains the Ghost member's exact encoded ID,
but the current object matcher searches at any bit offset. Such a match can
also be a reference inside another object. Without decoded message framing or
a live member datum, it must not be treated as a top-level member update or a
verified spawn. JOIN98 timed out for inactivity before the attempted Guardian
emote check; the notice was captured and the owned stack stopped with evidence
preserved.

## JOIN99: the one-member selector skips its only member

The desired-count test creates one live Ghost actor with a valid physical
unit; see [the count evidence](squad-member-counts-0124.md). The same typed
helper selector still yields no target. Native observations show
`0xc73c80` initializing index 0 through `0x245590`, followed by
`0xc73be0` calling `0x245504` to advance to index 1. The bounds check
then fails against live-member count 1 and returns `0xffffffff`.
No `0xc11dc0` call follows. This narrows the earlier description of
`0x80800a07`: its observed first-result path is unsuitable for this
one-member test. Creating an extra Ghost to evade the bound is not a
verified retail sequence.

The actual `of_ghostlight` object is `0xfc0e45a3 / 0x21 / 124`, state A
`0x80800750`. Its typed entry `0x808006d7` dispatches through
`0x286bac` and `0x211104`, with an explicit object reference. Mode 0
adds the resolved member to the filter via `0x2867f4`. A kind-2 reference
uses the member lookup and its unit datum, rather than the squad iterator.
These handlers support a separate bounded candidate; live resolution and
attachment creation still require observation.

JOIN100 tests one filter entry targeting member `0xfc0e45a3 / 2 / 95`,
then an HO state whose reference is filter `0xfc0e45a3 / 0x21 / 124`
and whose typed selector is absent. The filter and helper states round-trip
through the actual schema in 94 and 89 bits respectively. The mission's
filter alias enables that manual test; it adds no automatic light action.

## JOIN100: bound squad reference reaches attachment creation

JOIN100 repeats 249 occupied records with zero pending baselines at 19:55:42
BST. The explicit desired count creates one actor by 19:55:46. Its actual
unit accessor is `0x191960`, which returns actor `+0x30`: unit datum
`0x12fb82cd` resolves to a physical entity at `(390.458, -786.975, 18.948)`.
Actor `+0x38` is a different component datum and `+0x40` is absent; neither
should be substituted for the unit accessor.

The direct kind-2 filter decodes correctly and repeatedly reaches
`0x286bac -> 0x211104`, but returns no unit. The network binding table has
no entry at member index 95. It **does** have a live entry at squad index 94:
that entry's `+0x108` equals the actual Ghost unit, and its next link is absent.
The kind-1 branch of `0x210a20` enumerates that binding list, independently
of the typed squad iterator that skipped its first member.

At 20:03:41 the filter entry was changed to reference **squad 94**, using
the same single-result resolver:

```json
{
  "set": {
    "of_ghostlight": {
      "0": {
        "0": 1,
        "0x10": [{
          "type": "0x808006d7",
          "values": {"0": 0, "4": {"0": 4228793763, "4": 1, "6": 94}}
        }]
      }
    }
  }
}
```

The HO state for this filter path is:

```json
{
  "set": {
    "ho_ghostlight": {
      "0": false,
      "4": 0,
      "8": {"0": 4228793763, "4": 33, "6": 124},
      "0x10": null
    }
  }
}
```

Fresh function-entry observations show mode 0 adding `0x12fb82cd` through
`0x2867f4`, followed by `0xc11dc0` with that same target. The actual creator
`0x1f7c94` is called with `lr=0xc11e58`, descriptor `HO+0x180`, the Ghost
unit as `r5`, and `r6=0xffffffff`. A subsequent read shows a new binding
under HO index 101 with that target. Later attempts pass descriptor validation
and recognize the existing attachment. This establishes successful selection
and registration on the configured attachment path, without creating a second
Ghost or editing guest memory.

The earlier zero counts at the **mid-function** watches `0xc11e54` and
`0xc11e5c` do not establish that the call was skipped: those PCs need not be
LLVM block entries. Prefer the creator's real function entry and its HO return
address, then confirm the resulting binding. Do not use the four unrelated
creator calls during player setup as Ghost-light evidence.

The sampled frame at 19:55:53 is a transition fade; frames at 19:56:06,
19:56:20 and 20:04:08 render the Breach normally. The player was about 65
units from the Ghost, and these frames do not establish visible torch light.
A guarded ordinary walk reached `(346.785, -797.305, 14.095)` before the
client entered cleanup at 20:10:29. The captured notice explicitly reports an
inactivity return to Orbit, despite brief recent movement. The walk released
input immediately; the native/protocol evidence was preserved and the owned
stack stopped. JOIN101 repeats the corrected sequence promptly and tests
the mission's automatic desired count. No automatic light action is enabled
until placement and visible illumination are checked.

## JOIN101: repeated attachment path and close-view limit

The shipped mission count creates one physical Ghost after a manual bubble
reset and the 249-record readiness check. At 20:27:38 BST its unit is
`0x74fb82ce`, at `(390.458, -786.975, 19.122)`. The same corrected filter
state is sent manually at 20:27:38, followed by the HO state at 20:27:40.
The real creator entry again has `lr=0xc11e58` and the actual Ghost as
`r5`. Registration at `0x1e5060`, `lr=0xc11ee0`, carries the helper's
`0xfc0e45a3 / 0x18 / 101` ID. Its `r4=0x65fac971` is a distinct datum;
do not substitute it for the selected target or assume the two refer to
the same physical entity.

A normal guarded walk reaches within about twelve units of the Ghost,
without leaving slice 24 or writing guest memory. The nearby corridor
frames are dark. The player can open a clean Titan inventory preview,
return along the corridor and turn toward the brightly lit windows.
Those final views rule out a persistent whole-view fade in this run, but
do not verify illumination near the Ghost. View direction must accompany
position in the next comparison; proximity alone is insufficient.

The configured asset contains twelve component references. Their package
class is `0x80800861`; the six repeated roots carry `0x80801a4e`, and the
remaining roots include `0x8080222e`, `0x80aad032`, `0x80801b73`,
`0x8080079a`, `0x80800441` and `0x80800594`. This is structural evidence,
not a semantic identification of lights or illumination parameters.

## Generation action: default-payload path, not a verified light switch

The final HO update at `0xc1238c` first attempts creation, then compares
state generation at component `+0x114` with the previous `+0x170`.
An increase calls `0xc121bc`. That function enumerates the HO bindings,
reads each binding's unit at `+0x108`, and calls `0x12e79c(unit, 0)`
when the unit-mask bit is set.

The interface target is statically recoverable: `0x2fb2d0` installs object
`0x1ab9d7c`; initializer `0x2fd958` gives it vtable `0x13e7cf0`.
Slot `+0xa0` holds descriptor `0x1853ee0`, whose function is `0x2fbc14`.
It resolves the unit's entity through `0x2ff4b8`, then reaches
`0x9521e4 -> 0x950eb0 -> 0x431da8 -> 0x431c14`.
The final handler resolves a component and calls `0x432674` with the
default payload pointer from global `0x189a0a0` when its payload argument
is null. The callee copies a structured payload, rather than a numeric
light state. The handler marks component `+0x2a0` bit 3 and reports the
change. This establishes a default-payload update; its semantics and
effect on the configured attachment still need confirmation. Increasing
the HO generation must not be presented as a proven torch-on action.

## JOIN102: registered container and all twelve components resolved

JOIN102 repeats the full 249-record readiness check at 23:17:19 BST.
After manual bubble reset `3:1`, the shipped delayed count creates one
physical Ghost by 23:17:42, with unit `0x12fb82ce` at
`(390.458, -786.975, 19.073)`. The corrected squad-94 filter and HO state
again reach the actual creator and registration. Registration's distinct
datum is `0x17fac971`.

A read-only snapshot at 23:19:46 resolves that registered datum to
`0x604081a0`. Its container header contains configured asset `0x80a3d8ad`,
configuration offset 12 and header class `0x80800906`. The configured
asset's component array independently contains twelve entries; the header
offset was initially mislabeled as a count and is corrected by JOIN106.
The component pool at
global `0x1a21cb0` has a 24-byte row stride. Rows 2418–2429 all name the
registered container at row `+0xc`; each resolved component names the
actual Ghost unit at runtime `+0x1c`. All configured roots therefore have
runtime components linked to the selected target:

| Configured type | Runtime header class | Count |
| --- | --- | ---: |
| `0x8080222e` | `0x808029c3` | 1 |
| `0x80aad032` | `0x80aad033` | 1 |
| `0x80801b73` | `0x80801b6f` | 1 |
| `0x80801a4e` | `0x80801b21` | 6 |
| `0x8080079a` | `0x80800610` | 1 |
| `0x80800441` | `0x808008fb` | 1 |
| `0x80800594` | `0x808008f6` | 1 |

Resolve a component runtime pointer as `row - u32(row+8)`, modulo 32 bits.
The owner accessor reads **the pool row's** `+0xc` directly; subtracting
the relative offset first would read a different field. Row `+0x10` is
the next component datum and `+0x14` its configured type. The
`0x80aad032` component also holds a network binding and the exact
`0xfc0e45a3 / 0x18 / 101` helper ID. This confirms the configured
components were instantiated, but does not identify their light semantics.

The small accessor `0x1f8c1c` produced no observed entry with the expected
HO return address, although creator and registration entries did. An
inspection requiring that accessor hook consequently failed; independent
registered-container and pool reads succeeded. Small leaf functions can
be inlined, so that missing hook is not evidence of failed creation.

### Controlled views and camera-tool repair

A bounded ordinary walk toward `(385, -791)` stops at its 120-second
budget near `(386.173, -795.120, 14.589)`. Corridor obstacles prevent
arrival; input is released and current slice remains 24. The subsequent
pitch sweep exposes a tooling error: `facing_from_pointers` rejects the
valid direction `(-0.004909, 0.997558, -0.069650)` because it assumes a
horizontal unit vector. All three pointer paths still resolve the same
camera vector, whose **three-dimensional** length is approximately one.

`d1_walker.py` now accepts finite 3D unit vectors with a usable horizontal
projection and computes yaw from that projection. Invalid and nearly
vertical vectors remain rejected. `refind` also retains a valid pointer
when its address is unchanged; a short camera tap no longer initiates
unnecessary movement probes. Regressions include the actual pitched
direction, steep pitch, invalid vectors and same-address recovery. All
119 script tests pass.

With both fixes, normal camera input captures the following views at
roughly `(386.7, -795.5, 14.45)` without running a movement search:

| BST | Target | Actual yaw | Actual pitch | Observed view |
| --- | --- | ---: | ---: | --- |
| 23:32:10 | Ghost, bearing about 66 degrees | 64.74 | 2.90 | Dark world, HUD visible |
| 23:32:14 | Same Ghost | 64.74 | 12.65 | Dark world, no visible beam |
| 23:32:17 | Same Ghost | 64.74 | 29.42 | Dark world, no visible beam |
| 23:32:22 | Back along corridor | 173.99 | 1.26 | Lit orange corridor geometry |

Earlier 23:17:48/58 frames also show clean, bright window geometry.
The controlled comparison strengthens the evidence against a persistent
whole-view fade. It does not establish an unobstructed line of sight to
the Ghost or a working torch.

### Generation 1 accepted; no visible illumination verified

At 23:33:52, the same manual HO state is sent with generation 1. By
23:34:02, its current `+0x114` and previous `+0x170` generations both
equal one. The squad retains one member, the same actor/unit and the
same desired counts. The registered container remains present. Sampled
runtime differences are limited to ordinary counters/position fields;
they do not identify a light activation parameter.

No requested generation-path entry is observed in the bounded native
trace. The consumed state is independently proven by memory, but these
missing watches cannot establish whether the statically traced payload
path ran. Subsequent views at yaw 64.56 and pitches 12.77/28.34 remain
dark with the HUD visible. Generation 1 therefore remains a diagnostic
candidate, not a verified light switch or an automatic mission action.

The owned stack is stopped after preserving evidence. Next: identify
the instantiated component semantics and their activation/transform
requirements, establish an unobstructed corridor viewpoint, then compare
illumination under a justified state change. The automatic mission still
sends only the verified Ghost count; the filter and helper remain manual.

## JOIN103: the helper binding targets the physical Ghost

JOIN103 reaches the guarded route endpoint at 23:58:15 BST on 4 October,
then reports current slice 24 at 23:59:22 with all 249 occupied records
baseline-ready. The engine remains in state 3 with return target 112;
this is not an idle-transition result. The manual bubble reset is followed
by the shipped count rule, which creates one Ghost by 23:59:50. The manual
filter/helper sequence completes at 00:00:24 on 5 October.

The newly added [attachment inspector](../scripts/rpcs3_final124_attachment.py)
checks the registered owner, configured asset, bounded component chain,
each component's owner and target, and the network component's binding
independently. It uses an owned-process manifest and read-only memory;
pool/header/link changes during a sample are rejected. Reads remain
non-atomic, and run-specific datums must come from that same run.

The live sample at 00:00:29 resolves owner `0x4bfac971` to the configured
`0x80a3d8ad` container and all twelve components. The selected Ghost is
`0x6bfb82ce`. The network component's binding `0x62fd203c` independently
resolves to that **same physical Ghost**, with the native unit-mask bit
set. The owner has a different pool discriminator and an index outside
the unit pool. This rules out the specific hypothesis that the helper
binding targets its non-unit container and therefore fails the native
unit-mask gate. It does not identify a light activation parameter.

A bounded scan of the current Breach group's network bindings finds only
squad 94 and helper 101, both associated with the Ghost. The placed
`o_device_invis_light` has no binding or tracked unit pair; its cached
State A generation is zero and its creation Boolean is false. Missing
placed-device creation is a separate question from helper attachment.
Its configured position is approximately `(428.85, -813.83, 21.84)`,
away from the Ghost. Its name alone does not prove it supplies the torch.

The 00:09:03 capture still shows clean window and corridor geometry.
The owned stack is stopped with its trace and snapshots preserved.
Raw observations and assets remain in the private runtime.

## Placed-device creation path in the final executable

The actual device definition declares State A `0x808007d9` and State B
`0x80800387`. Its mission alias includes B presence and adds no automatic
state change. The runtime State A descriptor is reached through global
`0x18def9c`; the State B alias is `0x18defa0`. These are final-build
addresses, separate from the old 01.00 labels.

Apply handler `0x2151d0` checks definition byte `+0x68`, handles generation
replacement through `0x215104`, then copies the 0xe0-byte State A to
component `+0x110`. For the device's observed nonzero definition byte,
update `0x215b2c` requires reported generation `+0x1f0` to be less than
requested generation `+0x110`, and creation Boolean `+0x114` to be true.
It selects a variant via `0x2148a0`, calls creator `0x214478`, stores the
returned unit pair at `+0x2e0`, and advances the reported generation after
success. A generation-only state with the Boolean false cannot pass this
creation path.

The bounded manual candidate requests generation one and creation true,
preserving the cached absent object reference at State A `+8` as
`{0:0x811c9dc5, 4:-1, 6:-1}`. Its kind and index are **signed** in the
schema; unsigned 255/65535 fail the value-preservation preflight. The
candidate round-trips through the actual 5,041-type final schema.
Native creation, state consumption and a physical unit must be checked
before attributing any illumination to it or scheduling it automatically.

## JOIN104: placed-device lifecycle and a clear Ghost view

All times below are BST on 5 October. The guarded Wall route reaches
`(352.345, -796.712, 14.059)` at 00:23:18; its host request is queued only
after arrival. At 00:23:22, the client reports current slice 24 with all
249 occupied records baseline-ready. The readiness sample has engine
state 6, followed later by state 3 with return target 112. It does not
prove a settled idle Breach transition.

The manual bubble reset and shipped count rule create one Ghost by
00:23:43. The corrected manual filter/helper sequence again instantiates
all twelve components. The public inspector, including its component
header and binding consistency checks, succeeds on the live run: owner
`0x07fac971`, target `0x7afb82ce`, network binding `0x63fd203c`. The
binding independently targets the same Ghost and its unit-mask bit is set.

### The manual placed-device candidate works

At 00:24:01, the exact preflighted State A is sent:

```json
{"set":{"o_device_invis_light":{"0":1,"4":true,"8":{"0":2166136261,"4":-1,"6":-1}}}}
```

By 00:24:03, requested and reported generations are both one, the
existence report is true, and variant zero has physical unit
`0x75fb82cf` at `(428.847, -813.827, 21.841)`. Its unit-mask bit is set.
Native creator `0x214478` is observed with `lr=0x215bfc`, the exact
device component and its copied State A; entity creator `0x13f068`
follows with `lr=0x2145d0`. This independently agrees with the unit pair.

A fixed-view comparison verifies the lifecycle, rather than treating
accepted input as successful creation:

| Captured BST | Requested generation / Boolean | Reported generation / exists | Tracked unit |
| --- | --- | --- | --- |
| 00:27:38 | 1 / true | 1 / true | `0x75fb82cf` |
| 00:27:45 | 2 / false | 2 / false | Absent; both pair words `0xffffffff` |
| 00:27:52 | 3 / true | 3 / true | New datum `0x76fb82cf` |

Each later control retains the same absent object reference. Increment
the generation when testing removal/recreation; simply clearing the
Boolean after an already consumed generation does not exercise the
generation-replacement path. The nearby orange source remains visible
in all three views, at identical yaw 93.79 and pitch 2.45 degrees with
negligible position drift. This proves creation/removal/recreation,
not a torch effect or the correct mission phase for this device.
No automatic device action is added.

### The ramp resolves the earlier blocked approach

The recorded [Breach route](../missions/routes/breach.json) includes the
turn onto the ramp at points 72–74. A bounded, ordinary-input walk uses
points `56, 60, 64, 67, 69, 70, 71, 72, 73, 74`, reaching
`(389.513, -799.540, 16.644)` at 00:25:36. It then follows targets
`(390, -795)` and `(390, -791)`, reaching
`(389.943, -792.458, 18.466)` at 00:28:20. The player remains in current
slice 24, with no cleanup, respawn or guest-memory position writes.
The previous direct approach toward `(385, -791)` stayed below this ramp.

The 00:28:24 and 00:29:37 frames clearly show the Ghost's shell and blue
core about five units away. This is an unobstructed Ghost view, extending
the earlier proximity-only evidence. A fixed orange fixture lights
nearby geometry; it must not be mistaken for the Ghost's torch.

From that viewpoint, the HO generation is increased from zero to one.
The 00:29:44 frame has the same actual yaw 86.05 and pitch 0.64 degrees
as the preceding frame. Requested and consumed generations both reach
one. The same Ghost and all twelve helper components remain present,
but no new beam or corridor illumination is verified.

This time the native watch observes `0xc121bc -> 0x12e79c -> 0x431c14`,
with the actual Ghost datum and null payload. Thus the generation action
does reach the unit-action handler. `0x432674` is not observed; that
alone cannot prove whether an inlined call ran. The handler first queries
type `0x8080206f` through `0x147340`, then checks the returned runtime
view's compatibility with type `0x8080236c` through `0x109c58`.
Identify the lookup result and the attachment's actual
activation requirements before inventing another light-state value.
The metadata record's `+8` link is not proven to be an inheritance parent.

The 00:30:28 inventory frame again shows a normally proportioned Titan
and clean armor lighting. Level zero, loading equipment slots and the
starter-overlay rifle remain separate unfinished investment work.
The combined rendering fix remains enabled. This run adds no acceptance
claim for an in-world third-person Guardian or completed mission.

The owned stack is stopped after preserving the final native log,
attachment snapshots, device lifecycle samples and controlled views.
All manifest-owned processes and the separately owned proxy are checked
exited, and the user's XInput pad profile is restored.

## JOIN106: the HO generation stops at the first component lookup

JOIN106 repeats the 249 ready records, manual bubble reset and automatic
one-Ghost creation. Current slice 24 is observed with engine state 3 and
return target 112; this still does not establish a settled Breach return.
The fresh Ghost is `0x1afb82ce`. Its helper owner is `0x71fac971`, and
binding `0x36fd203c` independently associates with that same Ghost with
the unit-mask bit set. The corrected public inspector verifies all twelve
helper components before and after generation one.

The first root attached to the Ghost is `0x7ffac943`, configured from
base asset `0x80cb82ed` with 45 components. The next root is the helper,
asset `0x80a3d8ad`, with twelve components. Both runtime headers have
configuration offset twelve: this independent 45-versus-12 comparison
confirms that runtime header `+8` is an offset, not a component count.

The native query follows the unit row's `+0x3c` root list. `0x1fc408`
resolves the requested type and reads its interface name hash at `+4`;
`0x1fb910` resolves each root's configured asset and adds its configuration
offset before querying the interface table through `0x2049e0` and
`0x284e28`. Type `0x8080206f` uses key `0xe52389ae`. Neither the base
nor helper table contains it. A complete offline scan of their 512 and
64 buckets independently agrees with the live query. Key `0x1de71ad5`
for type `0x8080236c` is also absent from both configured tables.

The bounded five-second generation-one trace observes this exact path:

```text
c121bc (lr c123d4)
  -> 12e79c (lr c122cc, Ghost 1afb82ce, null payload)
  -> 431c14 (lr 431dc0, same Ghost)
  -> 147340 (lr 431c64, unit row 602cfd80, type 8080206f)
  -> 1fc408 (lr 147398, same unit row and type)
```

The handler's matching call chain has no successful-view accessor
`0x1fbaf4`, compatibility check `0x109c58` or light action `0x432674`.
During the same watch, 787 other calls reach `0x1fbaf4`, providing a
positive control. Together with the actual missing interface records
and verified query algorithm, this supports failure of the first lookup;
it is stronger evidence than an absent light hook alone. The HO update
still consumes its generation after the call, so consumption does not
prove a light action succeeded. No automatic HO torch action is added.
The next investigation should follow the actual `nux_ghost` performance
(kind 28, index 125) and its light controls rather than repeat this reset.

The ramp walk again reaches an unobstructed Ghost view at 01:25:51 BST.
The shell and blue core are visible, with no verified beam. Third-person
emote frames at 01:26:33 and 01:28:21 show dark armor with patchy, bright
silver shading, while the 01:27:43 inventory model retains normal
proportions and lighting. Treat in-world Guardian rendering as unresolved.
The active game-specific configuration still enables color/depth reads
and writes and RSX memory tiling. The first render-target dump was captured
after the emote ended, as its 01:29:34 screenshot confirms; its lack of
a Guardian cannot diagnose the armor. A second dump and matching frame
were saved at 01:34:24/25 just before the user requested a stop. They
remain private and await inspection. No rendering change or new combined
flicker/character acceptance is claimed.

The owned JOIN106 stack and separate proxy are stopped and every recorded
identity is checked exited. XInput is restored. Raw traces, screenshots,
assets and render-target dumps remain outside Git.
