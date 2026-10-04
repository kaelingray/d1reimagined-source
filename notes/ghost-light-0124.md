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

## Generation action: default-state path, not a verified light switch

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
The final handler resolves a component and calls `0x432674` with its
default-state global when the supplied state value is zero. It marks
component `+0x2a0` bit 3 and reports the change. This establishes a
default-state update; its effect on the configured attachment still needs
a live observation. Increasing the HO generation must not be presented
as a proven torch-on action.

Next: resolve the newly registered entity independently of the Ghost,
inspect its configured component/default state and placement, and compare
controlled corridor views before and after any justified state change.
The automatic mission still sends only the verified Ghost count; the
filter and helper remain manual candidates.
