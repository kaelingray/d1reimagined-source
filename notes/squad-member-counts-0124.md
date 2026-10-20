# 01.24 squad member counts

Target: BLES01857 01.24. These observations distinguish restarting a squad's
generation from creating its physical members. Raw definitions and memory
snapshots remain private.

**October 5 update:** the count-only Ghost path below creates a physical
unit without the authored-member and performance bindings. JOIN109/111
verify a replacement kind-2 member State A, producing exactly one bound
Ghost while all desired counts remain zero. The shipped mission now uses
that member state; do not enable the historical Ghost count action beside
it. Counts for other squads retain their separate meaning. See
[the authored-member investigation](ghost-light-0124.md#join111-the-automatic-rule-creates-and-binds-one-ghost).

## Why a generation alone is insufficient

JOIN98 reached the Breach with all 249 occupied replication records ready.
After an inactive bubble reset and a Ghost squad generation, the actual
`sq_nux_ghost` component has generation 1 at `+0x108` and **zero live members**
at `+0x1d8`. Its kind-2 component exists in the replication registry, but that
does not establish a physical scene object.

The final-build generation handler `0x20cc38` changes the stored generation,
resets bookkeeping and calls `0x245204`. The latter only clears a byte at
component `+0x1bc`; it is not the physical spawn call. Earlier notes that use
those two hook hits plus an arbitrary-offset member-ID match as proof of a
spawn overstate the evidence.

Per-frame update `0x20c4a4` resolves state A `0x8080071e` and, when applicable,
calls `0x20c218`. That function subtracts existing counts and its internal
counters from desired counts at component `+0x384`. A positive total reaches
the placement/creation path through `0x248704` and `0x247300`; a zero total
returns without creating members. The state's mode byte at `+0x75` also gates
this path. Its retail mode choices are not inferred from their numeric values.

State `+0x2c` is type `0x80805b79`, a count at `+0` followed by a fixed-capacity
array at `+4`. The array's slot schema, `0x80805ba9`, contains eight signed
16-bit values. These are scalar array values, not nested `{ "0": value }`
objects. The apply path copies the selected entries into `+0x384`.

The actual Ghost squad definition `0x80cb82d3`, root `+0x3e4`, references
configuration `0x80cc3613`. The same resolution used by `0x20b620` yields one
member variant. This supports a bounded one-member experiment without choosing
counts for unrelated Fallen encounters.

## Bounded candidate

After the owned client reports current slice 24, confirm that the Ghost squad,
member and helper records exist and that no occupied record awaits baseline.
Rearm bubble 3 with a fresh inactive generation and observe its waiting bit
clear. Then publish, using a fresh squad generation for that session:

```json
{
  "set": {
    "sq_nux_ghost": {
      "0x2c": {"0": 1, "4": [1]},
      "0x44": 2,
      "squad_existing_members": -1
    }
  }
}
```

The actual final schema encodes this state in 72 bits and decodes the one
desired count, generation 2 and existing-member mode -1 exactly.

Require a nonzero live-member count and valid member datum before attributing
any helper attachment result to a spawned Ghost. Keep visible illumination, normal Guardian
appearance and stable slice completion as separate checks.

## JOIN99: one physical member created

At 19:30:58 BST on 4 October, after the current-slice and baseline checks and
the inactive `3:1` reset, the candidate above was sent through live control.
Two seconds later the Ghost squad has generation 2, desired counts
`[1, 0, 0, 0, 0, 0, 0, 0]`, and a live-member count of **one**. The native
trace reaches both `0x248704` and `0x247300` on the creation path.

The member pair is `0x17033651 / 0x28f58000`. Its second word is the actor
datum: resolving that datum through the actor pool yields an active actor
with unit datum `0x48fb82cd` at actor `+0x30`, the field returned by the
retail accessor `0x191960`. The first word is an auxiliary
lookup ID and must not be used as the actor datum. Subsequent reads retain
the live actor and unit. These addresses and handles belong to JOIN99 only;
fresh runs must resolve their own registry and pools.

This is physical creation evidence, beyond the earlier arbitrary-offset
network ID matches. It does not establish the Ghost's visible placement,
illumination or dialogue. The typed helper selector `0x80800a07` still
creates no attachment: its first-result path advances iterator index 0 to
1 before testing the one-member bound. See [the light investigation](ghost-light-0124.md).

## JOIN101: the shipped rule creates one member

The fresh run clears all 249 occupied baselines. A manual inactive `3:1`
reset at 20:27:16 BST satisfies the bubble gate. The mission's current-24
rule matches at 20:27:15.587 and emits its desired count at 20:27:36.660;
no manual count or squad-generation line is sent in this test.

At 20:27:38, generation 1 has one live member, the expected eight desired
counts and zero internal counters. The pair is `0x62ace605 / 0x02f58000`;
the actor's `+0x30` gives unit `0x74fb82ce`, with physical position
`(390.458, -786.975, 19.122)`. Fresh entries at `0x248704` and `0x247300`
confirm the physical creation path. These handles belong only to JOIN101.

This validates the mission's delayed one-Ghost state after a fresh bubble
reset and readiness. The reset is still a manual protocol action. Counts
for other squads, Ghost movement/dialogue and visible torch illumination
are not established by this result.
