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
declaration retained in the object specification. The reference at `+8` is
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
existing member. Referencing squad 94 therefore selects its live members;
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
`+0x44` is tag `0x80a3d8ad`, whose saved class is `0x80800906`.
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
