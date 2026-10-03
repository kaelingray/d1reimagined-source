# 01.24 reason 0x4d and transition timeouts — 3 October 2026

This is static analysis of the decrypted BLES01857 01.24 ELF, with the JOIN88 AH
trace as supporting evidence. It does **not** establish which of the three selecting
session states JOIN88 used. The bounded native watches below distinguish them on the
next run. Raw logs and the ELF stay private.

ELF SHA-256: `e7f100b38bbaf6c23f71b390fdb103d31b427187c38dcfbd47e09c94c4a44206`.
Addresses below were disassembled in this file. The old 01.00 symbolic error names
are not evidence of the exact 01.24 condition.

## The selecting state matters

JOIN88's error-poster watch captured category `0x19`, reason `0x4d` at `0x355d54`,
with LR `0x35fa80`. Its direct call site is `0x35fa7c`. Leave condition 32 enters
`0x35f50c` through `0x35ead4`'s table `0x12f99e0`. At `0x35f5a0`, it calls
`0x35dd14(0, 0, manager, 0, &reason)`.

`0x35dd14` gets the selected session state through `0x33502c`. States 9 and higher
pass `0x335a54`; this branch indexes `0x12f9d9c` by `(state - 9) * 4`:

| Final-build session state | Mapped reason | Verified producer |
| --- | --- | --- |
| 9 | `0x4d` | Not yet located |
| 10 through 36 | `0x4e` through `0x68` | Not the repeated `0x4d` entries |
| 37 (`0x25`) | `0x4d` | `0x33ab7c`: pending context `+0x18` timeout |
| 38 (`0x26`) | `0x4d` | `0x33ab98`: pending context `+0x20` timeout |

In particular, final-build state 35 maps to `0x67`; the earlier state-36 name
mapping cannot be reused unchanged. The numeric reason alone does not prove an
IPAHP setup failure, a transport failure, or a missing generic acknowledgement.

`0x33502c` gets the activity manager from `0x3401f0` (`*0x1abc8a4`). The two
selectors index the slot map at `0x1abc79c`, using `selector0*4 + selector1*8`.
The mapped slot's state is the u32 at `manager + 0x4e0 + slot*0x170`.

## Two 120-second local callback timeouts

In the active-session branch of `0x33a808`, the context handle comes from the slot
mapping's `+0x14`, and `0x1d56d0(handle)` resolves the simulation context. The code
tests `0x1da250(context)` first, then `0x1da2d8(context)`.

Both require context byte `+4` to be active, a nonzero pending-state byte, and
elapsed ticks **strictly greater than** `0x12fccc(120.0)`. The float at `0x1da248`
is `0x42f00000`, or 120.0. Both also require the common gate
`0x1da970(0x10a76c0, 1)` to succeed; that gate can suppress timeouts through
`0x12ef80` or the engine-interface vtable slot `+4`.

| Predicate | Pending-state byte | Start timestamp | State written on success |
| --- | --- | --- | --- |
| `0x1da250` | context `+0x18` | context `+0x1c` | 37 at `0x33ab7c` |
| `0x1da2d8` | context `+0x20` | context `+0x30` | 38 at `0x33ab98` |

These are local transition states. They are separate from the AHP assignment
object's request state. See [the private-host investigation](private-host-investigation.md)
for the verified AHP request-ID echo defect and its separate recovery-path fix.

## Producers and clearers of the pending states

Every active context is updated by `0x1d6df8 -> 0x1da1d4`. The latter obtains the
engine interface with `0x15616c` (`*0x1897c80`), then passes the two embedded
pending structures by reference. The interface is bound at `0x3ad43c`, through
`0x156150(object+0x0c)`. Its secondary vtable is `0x13eca48`.

| Interface slot | Final target | Incoming world record | Pending structure |
| --- | --- | --- | --- |
| `+0x7c` | `0x57c458 -> 0x391100` | context `+0x5078` | context `+0x18` |
| `+0x80` | `0x57c484 -> 0x391488` | context `+0x5078` | context `+0x18` |
| `+0x84` | `0x57c4b0 -> 0x3adf8c` | context `+0x5080` | context `+0x20` |

Slot `+0x80` is the deferred update in `0x1da150`, conditional on context byte
`+5`, which it clears afterwards. It does not itself clear the pending byte.

The first machine (`0x391100`) starts pending state 1 and timestamps it at
`0x391190/0x39119c` when incoming record byte `+1` is neither 0 nor 4 and the
pending state is zero. It advances through states 1, 2, 3, and 4. Each advancement
clears the byte only when the new state is 5 (`0x391280`, `0x391320`,
`0x3913c8`, `0x39146c`). Its state-4 completion requires incoming record byte
`+1 == 4` and `0xc07e2c()` to return neither 3 nor 4 (`0x3913dc..0x391410`),
then invokes the final local engine callbacks. Calling this a fade/transition
machine is an interpretation of its callbacks, not a verified retail name.

The second machine (`0x3adf8c`) starts pending state 1 and timestamps context
`+0x30` for a nonzero incoming request other than finish state 3. Its state 1
posts the slice switch through `0x35097c` after the engine readiness checks
`0x350a50/0x3509fc`. State 2 advances to 3 when the engine target matches
(`0x350a8c/0x158e1c`). State 3 needs the incoming request's **state 3**:
`0x3ae0d8` checks the local phase and `0x3ae0dc` checks the incoming state.
Its success path clears the pending byte at `0x3ae10c`. `0x391068` additionally
gates the final `0x72932c` callback, requiring `0xc07b30(request)` to be nonzero
and the other pending byte `+0x18` to be zero. The second pending byte still
clears after accepted host state 3 regardless of that callback gate's result.

The type-11 world-state receiver (`0x1d97a4 -> 0x1da894`) copies the decoded
world state to context `+0x70`. Thus the two inputs are group-block fields
`+0x21c0` and `+0x21c8` within world-state group `+0x2e48`. The second is
`0x80800bf9`, the host's group slice-set request. The existing
`world_state_delta(teleport=(state, index, hash))` encoder writes this field.
The clearing action is a type-11 world state carrying teleport finish state 3,
not a newly invented ACK for another wire message.

## JOIN88's omitted target delta

The private `FINAL124_JOIN88-ahl.log` establishes this ordering for the same
AH connection `20261003-152950-687008`:

| Timestamp | Event |
| --- | --- |
| 15:36:40.892 | Client target 24 / `0x68935472`, state 1, peer byte 3 |
| 15:38:20.477 | Control queues teleport to 24 |
| 15:38:21.948 | Host publishes teleport state 1 to 24 |
| 15:38:55.915 | Client reports state 3 |
| 15:39:31.987 | Client reports state 2, peer byte 4 |
| 15:39:32.187 | Client reports current -1 |
| 15:39:49.802 | Client reports state 3 |
| 15:39:54.222 | Client reports state 4 |
| 15:41:15 | Captured pumpkin cleanup, reason `0x4d` |

The last state-4 type-21 body is only two bytes, `a1a0`. It omits the target
already sent before host teleport start. The AH log has no finish-state-3
publication for 24. The stand-in formerly reset `teleport_targeted` on the
queued start, and could only set it from a target field in a later packet;
this sequence therefore fails to arm its finishing action.

The concrete protocol correction is to retain type-21 slice fields per AH
connection: recognize the matching target already observed when the request
starts, then allow the later state-only 4 to schedule finish state 3. Respect
explicit target clears/changes and reset this state between connections and
requests. An encrypted regression should reproduce the exact ordering above.
This explains a missing finish publication; live status 38 remains to be
captured before attributing JOIN88's reason to this machine specifically.

## Bounded native watches

The watch logger prints r3 through r10, plus configured memory dumps. Registers
such as r27 are not printed automatically. These hooks are failure-only:

```text
35de40 10
33ab7c 4 r29+0:52
33ab98 4 r29+0:52
```

At `0x35de40`, r3 is the selected session state minus 9: **0 => 9**,
**0x1c => 37**, **0x1d => 38**. At `0x33ab7c/0x33ab98`, r3 is respectively
`0x25/0x26`; r29 points to the context. The 52-byte dump includes active byte
`+4`, both pending bytes, and both timestamps. If the failure-only hooks show
state 38 but the host claims a finish publication, add these bounded watches:

```text
3ae0e0 6 r4+0:16 r30+0:20
3ae10c 6 r30+0:20
```

`0x3ae0e0` runs only when local phase 3 and incoming finish state 3 both match;
the dumps show the incoming request and pending copy. `0x3ae10c` is the final
clear. A watch at `0x3ae0d8` would run every frame while waiting and could use
its budget before the finish arrives.

Reproduce the static checks with `scripts/d1_eboot.py --elf <private-01.24-ELF>`:
`disasm 0x35dd14 --func`, `disasm 0x33a808 --func`,
`disasm 0x1da250 --count 78`, `disasm 0x1da1d4 --func`,
`disasm 0x57c458 --count 32`, `disasm 0x391100 --func`, and
`disasm 0x3adf8c --func`. The compiler may emit calls through descriptors;
resolve the vtable descriptor before interpreting indirect call targets.
