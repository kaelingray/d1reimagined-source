# The final PS3 build (01.24) against 01.00: static findings

Sources are the EBOOTs of updates 01.13 and 01.24, decrypted on the PC by
`scripts/Invoke-FinalBuildExport.ps1` (SHA-256 in the data repo's `ps3_updates/EXPORT_VALIDATION.md`). The final
investment globals chains 012c and 012f were read over HTTP ranges with `ps3_update.py fetch`.
This page records static findings; CONFIRMED means read directly from the executable or
packages. The first live 01.24 runs and their remaining loading problem are recorded in
[`checkpoint-2026-09-28-final124-run.md`](checkpoint-2026-09-28-final124-run.md).

## Reading the update executables

- **The type definitions changed format (CONFIRMED).** The first export found only 351 types in
  01.24; the true count is 5,041. Two changes caused the miss.
  - Definitions are 0x20 bytes shorter and link their field list from def+0x34, not +0x54.
  - Each 0x28-byte field entry now leads with a repeat count; the rest of the entry follows 4
    bytes later.
  - A fixed array such as `u8[128]` is now one entry {128, offset = stride, ...}. 01.00 listed
    every element. All 265 such arrays are the sole field of their type, with count × stride =
    size.
  - `d1_types.py` reads both layouts. It picks the one that finds the most types, and expands
    arrays back into elements, so the two builds compare field for field. The 01.00 export is
    byte-identical to before.
- **The TOC base moved:** 0x175a5e8 (01.00), 0x184fc00 (01.13), 0x1891798 (01.24).
  - `d1_eboot.py` now reads it from the entry point's function descriptor.
  - The curated labels are 01.00 addresses. They are applied to that build only.
- **Most type handles are stable (static match).** 01.00 → 01.24 with the
  one-to-one matcher:
  - 4,308 of 4,387 types match (3,744 structurally identical, 564 changed);
  - 79 are unmatched in 01.24 and 733 are new;
  - 156 matched types were renumbered.

  These counts depend on heuristic matching where a handle did change; the
  reports are not proof of type identity for every pair. Almost every handle
  the stand-in writes keeps its number. `d1_builddiff.py` now reserves
  unchanged handles first and never assigns one new type to two old types.
  It compares field kinds (offsets shift whenever a field is inserted) and
  prints side-by-side layouts of the stand-in's types.

## What changed for the stand-in

**Character objects (CONFIRMED, same handles, grown types):**

| Where | 01.00 | 01.24 |
| --- | --- | --- |
| progression `0x80801855` | 0x770; the 20 gear slots at +0x138 | 0x8c8; three new f32 after +0; gear slots at **+0x1e8** |
| gear slot `0x8080183c` | 0x28: {u64 id, i16 item, u8, i16, +0xe dyes} | **0x30**: {u64 id, i16 item, i16, i16, +0x10 new `0x80801069`, +0x18 dyes} |
| peer summary `0x808017fd` | progression at +0x30 | progression at +0x30 (size 0x7b0 → 0x908) |
| select summary `0x80801954` | progression at +0x68 | progression at +0x68 (size 0x7e8 → 0x940) |
| self character `0x80801714` | 0x55b0: inventory +0x30 (0x1808), equipped ids +0x1838 | 0x6db8: inventory +0x30 (**0x2808**), new `0x808019ff` +0x2838, equipped ids **+0x2900** |
| inventory entry `0x808016e0` | 0x18: {i16 item, u64 id, i32, i32} | 0x20: one more i32 |
| account `0x808018c7` | 0x1ff8: flag table A +0x12a8 (1,024), table B +0x16a8+1 (256) | 0x7050: B at **+0x4f78+8**; A LIKELY `0x80804b0c` at +0x383c (**2,560** flags); vault-like `0x80801087` (326 items) at +0x4c0 |
| character flags (table C) | self +0x3bc0, 1,024 | self **+0x5000**, 2,048 (`0x80804c21`) |

- `bap_server.character_layout(schema)` derives the gear offsets from whichever schema the
  stand-in loads. It finds each field by its nested type's handle, so `--character-gear`
  writes 01.24's layout when given a 01.24 schema. With the 01.00 schema (or none) it gives
  the old constants.
- `--self-flags` now finds account tables A/B and character table C in the loaded schema.
  The 01.24 static layout gives A at +0x383c (2,560 flags), B at +0x4f78/+8
  (256), and C at self character +0x5000 (2,048). An unqualified `--self-flags 2`
  fills the full capacity of each selected table. These are structural matches;
  which new flags grant which content still needs runtime work.

**Mission objects (CONFIRMED):**

- The object id `0x808004b1` widened the kind field from 6 to 7 bits. The mission tooling
  encodes ids from the schema, so a 01.24 schema handles this.
- The objective `0x80802a1b` grew (0x2d0 → 0x2f0), but its player mask stays at **+0x2c0**. A
  machine id and a second object id were added at +0x2cc/+0x2d4.
- The mission-script state handles remain present in 01.24. A direct comparison of the
  corrected static exports gives the following migration check (runtime behavior is untested):

  | State | 01.00 -> 01.24 | Consequence for the stand-in |
  | --- | --- | --- |
  | Cinematic `0x8080254d` | 0xd4 bytes and all 9 fields identical | The play counter, playing flag and cast-list offsets remain valid. |
  | Player trigger `0x80800463` | 8 bytes and both fields identical | The armed flag and arming time remain at their old offsets. |
  | Sequence `0x808029a1` | 0x2c -> 0x20 bytes; nested state at +0x8 changes from `0x80805b11` to `0x80805cf5`; final object reference moves +0x24 -> +0x18 | Rebuild the sequence state with the 01.24 schema; copying the 01.00 payload would misplace the reference. |
  | Squad `0x8080071e` | 0x60 -> 0x7c bytes; 13 -> 18 fields; the 2-bit, bias-1 existing-member field moves +0x5c -> +0x74 | The scripts use `squad_existing_members` to resolve that field from the selected schema. Confirm 01.24 behavior in a run. |
  | Objective `0x80802a1b` | 0x2d0 -> 0x2f0 bytes; 13 -> 16 fields | The player mask remains at +0x2c0, but serialize the full new layout. |

  The data repo's initial update-build reports used the old type scanner.
  Corrected exports and one-to-one handle maps are in `d1xenia-data/ps3_updates/`.

  An offline encode/decode check of all 80 state changes in the A Guardian Rises and Tower
  scripts passes with both the saved 01.00 activity schema and the corrected 01.24 type export.
  Before the field migration, 56 squad changes still named +0x5c; the 01.24 encoder silently
  omitted that field. `scripts/d1_mission_preflight.py` now catches such omissions, and
  `--ah-mission` runs it at startup. This does not establish that 01.24 accepts or acts on the
  states; that requires a live run.
  The run scripts still default to the saved 01.00 schemas. For an 01.24 run, pass the
  corrected `types_01.24.json` as `--schema` to the BAP stand-in and as
  `--activity-schema` to the activity host (via `-ExtraBap` and `-ExtraAh`).

**01.24 STUN is the remaining blocker (RUN A01, 2026-09-30, MATILDASPC).** After
installing the 1,034 update packages into `RPCS3_FINAL124_SMOKE001`, the first full
01.24 run on this PC reaches `bootflow:bap_signin` (state 19 of 38 - the transferred
SMOKE019-023 captures show the same, so content and package handling are solved) and
then waits. All 20 STUN hostnames resolve to the stub; 93 probes arrive and none are
answered; **the BAP stand-in never sees a connection**. So STUN, not BAP, is what
blocks progress past `bap_signin`.

**The 01.24 probe is `type, version, fields` - the same layout as 01.00, NOT MessagePack.**
Captured shapes:

```
len=4   00 01 00 01
len=29  0e 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 ff 00 ff 00 00 00 ff 00 ff 00 00
```

This was first read as MessagePack (`0x0e` = fixmap(14), 7 int pairs, `-1` = unbound
ports) and **that reading is wrong** - it is recorded only because `decode_stun_probe.py`
and the `--fill` option were built on it. The disassembly settles it. The request is
assembled by `0xb4695c`, which in order does:

```
lbz  r11, 0(r4)      ; r11 = the type byte, read from the caller's buffer (0x0e)
sth  r0,  2(r9)      ; (li r0,2 precedes) store the CONSTANT 2 at offset 2
stw  r5, 0x10(r9)    ; the caller's r5 (0) at offset 0x10
stb  r11, 0(r9)      ; the type byte at offset 0
```

so the header is `type, version, field` exactly as 01.00 sent `1e 03 00 <mapped> <port>`
(type 30, version 3). Reading `0x0e` as a MessagePack fixmap was arithmetically valid,
which is why it survived a first pass, but the byte is a **type** (14) and the `0x02`
beside it is the **version** (2) - not a key and a map header. The 27 trailing bytes are
fixed fields, not 7 key/value pairs.

Consequence: `--fill` and `--shape` were shaping replies for a protocol the client never
sent. Their negative results still hold (no sign-on request either way) but they were
varied on a wrong model and should be re-run once the field layout is read properly.

**Re-run on the corrected model: `--shape typeversion` (RUN TV).** With the header
understood, the stub can finally emit the 01.00 layout - `type, version, 0, mapped
IPv4, mapped port` - via `--reply-type`/`--reply-version` (defaults 15 and 2, i.e. one
above the request's type 14 as a response). The reply it produced is
`0f 02 00 7f 00 00 01 02 0c`, which is byte-for-byte the same shape 01.00 sent for type
30 (`1f 03 00 7f 00 00 01 02 0c`). Result: **still no change** - 21 probes, the 4-byte
probe stopping after 5, the counter still cycling `1..10`, no BAP connection and still
zero sign-on requests.

So even a correctly framed, correctly addressed reply is not accepted. That narrows it
further: the client is not waiting for a plainer or cleverer reply body, so either the
type/version pair is wrong (15/2 is an assumption - the real response type may not be
request+1), or the 29-byte probe is not asking for an address at all and needs some
other payload, or the client is blocked on something outside this exchange entirely.

**Reply-type sweep (RUN RT0/RT1/RT2/RT15) - the type byte is not the variable.** With
the header understood, `--reply-type` was swept over 0, 1, 2 and 15 on the
`--shape typeversion` reply, one variable per run, breaking out early on any progress:

| `--reply-type` | probes | replies | sign-on requests | BAP connections | verdict |
| ---: | ---: | ---: | ---: | ---: | --- |
| 0 | 17 | 17 | 0 | 0 | no progress |
| 1 | 17 | 17 | 0 | 0 | no progress |
| 2 | 16 | 16 | 0 | 0 | no progress |
| 15 | 17 | 17 | 0 | 0 | no progress |

Identical in every respect. So the client is not selecting its next step based on a
response type we can influence, which is consistent with the PPU-hook result that the
dispatched word is client state. Combined with the earlier sweeps (dispatch index, fill,
shape, per-name addresses) that is now **six independent reply-side variables with no
effect at all**. The evidence is consistent with the client simply not considering this
exchange successful regardless of what comes back - which points away from the STUN reply
and towards whatever gates the transition out of the retry loop.

**Practical conclusion for the next session.** Answering STUN correctly is necessary but
demonstrably not sufficient, and the reply is not where the remaining work is. The 47-byte
send that looked like a missing third probe turned out to be the DNS query (RUN P47), so
it is closed. What remains is a **client-side condition**: the client has everything we
can send it and still does not advance, so the next step is to find the test it applies
before leaving the retry loop - the branch at the `1..10` sub-step counter's terminal
state, and whatever it requires to be true. A lawful capture of a real STUN exchange from
the owner's console would give the actual request/reply pair and is the fastest way to
confirm or refute everything inferred here; further reply guessing is not.

**Where the reply is dispatched (answers the open question in the runbook).** The
earlier note guessed the type "is most likely read into a register and used as a table
index" because the 01.00 types are absent from `cmpwi`. That is confirmed:

- response handler `0xb36fd8`, called from the transport state machine at `0xb33910`
  and `0xb339c4` (inside `0xb33814`, the state machine `0xb3012c` drives)
- `0xb3707c`: `lwz r0, 0(r3)` + `cmplwi cr7, r0, 0x13` - bounds check on the reply's
  first word, so valid indices are 0..19
- `0xb370e0`: `lwz r11, -0x2834(r2)` (TOC) + `rldic r9, r0, 2, 0x1e` + `lwzx r0, r9,
  r11` - a **jump table at `0xb370fc` indexed by that word**, entries signed
  pc-relative to `0xb370e4` (20 entries: idx 0 -> `0xb37410`, 1 -> `0xb373e4`,
  2 -> `0xb37620`, ... 16 -> `0xb371c0`, 19 -> `0xb37190`)

So 01.24 dispatches STUN replies on a small integer index read from the packet, not on
a type byte compared with a constant - which is exactly why searching for the type
constants found nothing. `server/stun_stub.py` now answers both 01.24 shapes with
`--reply-index` (default 0) choosing the dispatch word; the reply body is still inferred,
not confirmed from the client, and is the next thing to confirm by running.

**The reply dispatch index is NOT a field of the packet (PPU hook, RUN HOOK01).** The
instrumented build traced the handler directly (`d1\d1ppu.txt`, watched `0xb36fd8`,
`0xb33814`, `0xb370e0`). The handler ran 40 times and the dispatch 14 times, so replies
*are* arriving and being accepted - but the 64-byte object at `r3` on entry is:

```
word  0(r3) : 1, 2, 3, 3, ... 10, 10      <- increments across calls
dword 0x10  : 0x01a8c0  (constant)
byte  0x18  : 0, 0, 0 ... 3, 3           <- advances late in the run
```

So the word at `0(r3)` is **client-side state, not something the stub sent**: it counts
calls to the handler and is the value bounds-checked against `0x13` at `0xb3707c` and
used to index the jump table. The client walks its own state machine 1..0x13 as it
retries. This reframes the problem: `--reply-index` cannot steer the client's path
(because the index is not ours to choose), it only has to be a legal, self-consistent
reply. What still has to be discovered is which of the 20 table branches each stage
needs, and what the client then *does* with the parsed map.

Observed effect of answering (RUN t4, `--reply-index 0`): the client **stopped sending
the 4-byte probe** and sent only the 29-byte map from then on, while BAP still saw no
connection. So a reply is recognised and changes behaviour; it is not yet accepted as
success. The byte at `0x18` going 0 -> 3 is the visible progress in that run.

**The stage is written by the client, not read from our reply.** At `0xb37168` the
request path does `li r0, 0xe; stw r0, 0(r30)` - on a failed sub-step it stores **14**
into the same `0(r3)` word that `0xb3707c` later bounds-checks and `0xb370e0` uses as the
jump-table index. So the counter is the client's own STUN sub-step number (it reaches
`0x0e` on failure), and the 1..10 seen under the hook is that counter walking forward
while it retries. This is why `--reply-index` cannot steer anything: the index is not
carried by the packet at all. Any legal, self-consistent reply gives the same result.

**Sweep result (two indices, one variable each).** `--reply-index 0` and `1` behave
identically: the client sends 5 of the 4-byte probe, then **only** the 29-byte map for the
rest of the run, and the BAP stand-in never logs a connection.

| run | index | probes | replies | 4-byte | 29-byte | BAP |
| --- | --- | ---: | ---: | ---: | ---: | --- |
| t4 | 0 | 21 | 21 | 5 | 16 | no connection |
| A_STUNIDX1 | 1 | 37 | 37 | 5 | 32 | no connection |

So answering *is* recognised (the short probe stops permanently after ~5 attempts, in both
runs) but is not accepted as success, and the choice of index makes no difference -
consistent with the counter being client state. The remaining unknown is the **reply
body**: what the client expects the parsed map to contain for the stage to advance past
`0x0e`. The `-1` sentinels in the request are the lead: they are ports the client has not
bound, and a reply that fills the *right* ones (rather than the first) is the next thing
to try. `stun_stub.py --reply-index -1` is available as the silent control.

**The counter is a STUN *host* index, not a stage (strong correlation, 2026-09-30).**
Run A01 resolved exactly **20** STUN names - `destiny-stun.{us,eu,jp,au}.demonware.net`
(4) plus `stun1..8.signon.{deadorbit,gravityshavings}.net` (16) - and the dispatch
bounds check at `0xb3707c` accepts `0..0x13`, i.e. **exactly 20 values**, indexing the
20-entry table at `0xb370fc`. Under the hook the counter ran `1,2,3,...10` and never
reached `0x0e`; with a 400-hit watch cap the failure branch at `0xb37168` (which stores
`0x0e`) was **never hit**. So the word at `0(r3)` advances per host as the client works
through its list, and the client is **stuck retrying hosts, not failing a sub-step**.

That reframes the experiment: there is no single "right" index to send, and `--fill` (all
unbound ports answered) changed nothing either - 47 probes, 5 short + 42 map, BAP still
silent. The next thing to determine is which host the client is on when it stops making
progress, and whether it needs replies from *every* host in the list (a single STUN
server may have to answer as all 20 names, or the DNS sinkhole may need to hand out
distinct addresses per name so the client sees a consistent mapping).

**Distinct-address experiment (RUN DISTINCT) - negative, and it corrects the host-index
guess.** `dns_sinkhole.py --stun-distinct` and `stun_stub.py --bind-count 20` were added
so each of the 20 names resolves to its own address (127.0.2.2 .. 127.0.2.21) and is
answered by a real separate socket. Verified: all 20 DNS answers were distinct and the
stub reported all 20 bound. Result:

- **zero** STUN packets reached the stub (1 line in its log: the banner)
- the handler still ran 88 times, and the counter still only ever took the values
  `1..10` and then **repeated** - it never went 11..20, and never reached `0x0e`
- 50 `sys_net_bnet_sendto` calls, 23 connects, no `ENOTCONN`

So the word at `0(r3)` is **not** a host index either: with every host on its own address
the count still stops at 10 and cycles, where 20 addresses would have allowed 1..20. It
is a retry/sub-step counter bounded at 10, and the client gives up and restarts rather
than advancing. The packets that never arrive are `len=47` on socket 26 with
`addrlen=0` - a different, unconnected send, not either of the two probe shapes, so
something beyond the STUN probe is being attempted here and nothing is listening for it.

**Where this leaves 01.24.** The startup path to `bootflow:bap_signin` is solid
(confirmed by three independent capture sets), the content/package path is solved (1,034
packages installed, preflight green), and STUN is no longer simply ignored - replies are
parsed and change client behaviour. But the client does not accept the reply as success,
and the next unknown is the **reply body** (what the parsed map must contain to move the
stage past its current stop), plus that unconnected 47-byte send. Neither is guessable
from the notes; both need either more static analysis of the map parser or a lawful
capture of a real STUN exchange from the owner's console.

**Sign-on is never reached at all.** Worth stating plainly, because it changes what the
blocker is: across FINAL124_S01, t4, STAGE and DISTINCT the SignOn listener logged
**56 lines, all of them "listening on ..." banners, and not one client request**. The
client's first network act is DNS for the 20 STUN names (06:36:45) followed immediately
by the probes (06:36:58), and in 12+ minutes it never opens an HTTPS connection to
SignOn at all. So "01.24 stalls before BAP" understates it: it stalls before
**sign-on**, at the very first thing it does. That is consistent with the 01.00 route,
where STUN precedes sign-on too, and it means getting the STUN exchange right is worth
more than anything downstream - nothing else can even be tested until the client leaves
this loop.

**Environment re-verified, so the stall is the protocol and not the setup** (2026-09-30).
Worth recording because "it is just broken locally" is the obvious thing to suspect on a
freshly rebuilt PC: RPCN is running and port 31313 accepts, and its log shows the client
authenticating (`Succeeded with NoError` for `ResetState`); `config.yml` has
`PSN status: RPCN` and `DNS address: 127.0.0.1`; `rpcn.yml` points at
`127.0.0.1:31313` as the local test server. The preflight also passes all 14 checks
(1,034 packages at the right sizes, both 01.24 patches enabled, CA matching). So the
stand-ins, the RPCS3 copy and the emulator are all in the state the 01.00 route used, and
the remaining difference is that 01.24's first request is one we do not yet answer
correctly.

**Reply-shape sweep (RUN SHAPEBIN) - also negative.** `--shape map|binary|index` varies
only the reply layout, keeping the same leading dispatch index. `binary` (dispatch index
then the 01.00-style mapped IPv4/port) behaves exactly like `map`: 20 probes, the 4-byte
probe stopping after 5, no BAP connection, and still **zero** sign-on requests. So the
layout of what we send back is not the variable that matters either. Three independent
variables have now been ruled out by experiment - the dispatch index, how many of the
unbound ports we fill, and the reply layout.

That is a useful result rather than a dead end: it says the missing piece is not a knob
on the reply we are already sending. The strongest remaining candidates, in order:

1. the **47-byte unconnected send** on socket 26 (`addrlen=0`) seen in every run - a
   message that is neither probe shape and that nothing is listening for;
2. the **reply field semantics** - the client may expect specific field values in the
   `type, version, fields` reply, not just a plausible address;
3. a **second exchange** the client expects before it will leave the loop.

**The 47-byte "third STUN shape" is a DNS query - resolved and closed (RUN P47).** It was
listed above as a lead and it is now answered. Tracing `0xb340bc` (the socket-send wrapper
the PPU hook watched) shows the sequence plainly:

```
sys_net_bnet_socket(family=INET, type=DGRAM, protocol=IPPROTO_IP)
sys_net_bnet_connect(s=26, ...)
sys_net: [Native] Attempting to connect on 127.0.0.1:53
sys_net: [Native] sys_net_bnet_connect: using DNS...
sys_net_bnet_sendto(s=26, buf=*0x323ae010, len=47, flags=0x0, addr=*0x0, addrlen=0)
```

The send goes to **port 53** - it *is* the DNS query, 47 bytes of DNS wire format, which
is why no destination is recorded and why it never reaches the STUN stub. There are
exactly **20** such connects in a run, one per STUN hostname, and the sinkhole answered
all 20. (The only other outbound UDP is 3 connects to `192.168.0.1:1900` - SSDP/UPnP,
unrelated and unanswered.) So this is neither a missing third probe shape nor a missing
listener: the two shapes the stub sees are the complete set of STUN probes, and every
name resolves.

That closes the last "something we are not sending" candidate. Together with the six flat
reply-side sweeps, the picture is that the client resolves, probes, receives our replies,
and still does not leave the loop - so the remaining unknown is a **client-side
condition** (what it decides must be true before advancing) rather than anything missing
from the wire.

**The transport state machine behind that loop, decoded.** `0xb33814` is the outer state
machine; at `0xb338a8` it loads its own word counter and bounds-checks it against **7**,
then at `0xb3393c` uses it to index a second jump table (TOC `-0x2990`, table at
`0xb33958`, entries pc-relative). The eight states:

| state | target | what it does |
| ---: | --- | --- |
| 0 | `0xb33978` | logs (`0x64` = line 100) then falls through to the state-6 tail |
| 1 | `0xb33ab8` | |
| 2 | `0xb33a14` | |
| 3 | `0xb339ec` | |
| 4 | `0xb339b8` | **calls `0xb36fd8` - the STUN response handler.** This is the probe state. |
| 5 | `0xb3385c` | |
| 6 | `0xb33880` | the shared tail every state branches back to (`0xb3385c`-`0xb33880` are reached by `b 0xb33880`) |
| 7 | `0xb339a4` | calls `0xd73b10` on `r3+0xac` |

State 4 reaching `0xb36fd8` confirms the PPU-hook attribution: the handler we have been
tracing is entered from the transport machine's probe state, and the loop is this 8-state
machine cycling rather than the 20-entry reply table. What decides the transition out of
state 4 is the remaining question, and it is a single branch in `0xb339b8`/`0xb36fd8`
rather than anything on the wire.

**THE GATE (RUN GATE18): state 4 exits only on 18 or 19, and the client never reports
either.** This is the concrete condition, found by reading state 4's own exit test and
then measuring it with the PPU hook on `0x69ddb0` (the session getter state 4 calls after
the STUN handler):

```
0xb339b8  addi r0, r3, 0x4c0
0xb339c4  bl   0xb36fd8        ; the STUN response handler
0xb339d0  bl   0x69ddb0        ; read the session result into r3
0xb339d8  cmpwi r3, 0x12       ; 18 -> carry on
0xb339dc  bne  0xb33b3c
...
0xb33b48  cmpwi r3, 0x13       ; 19 -> carry on
0xb33b4c  beq  0xb33b64
0xb33b5c  cmpwi r3, 0          ; 0 -> back to the shared tail, keep looping
```

Measured return values of `0x69ddb0` over a full run (RUN GATE18, 200 hook hits):

| value | calls | meaning |
| ---: | ---: | --- |
| 0 | 193 | not ready - the value that keeps the loop going |
| 1 | 6 | |
| other | 1 | |
| **18** | **0** | **required to advance** |
| **19** | **0** | **also advances** |

**This explains every negative result in this section at once.** The stub's replies *are*
received and parsed - the handler runs, the sub-step counter advances, the 4-byte probe
stops - but the session getter that state 4 actually tests keeps returning 0. Nothing we
put in the packet can change that, because the value is not a packet field: it is the
client's own "STUN exchange complete" flag, and it is set by the parser only when the
reply satisfies conditions we have not identified. Every reply-side variable
(dispatch index, fill, shape, per-name addresses, reply shape, reply type) was therefore
guessing at a value the client never reads from the wire in the first place.

So the remaining work is precise and bounded: **find what makes the parser set that
flag.** The next concrete steps, in order:

1. read the success path of the parse routine that feeds `0x69ddb0`'s value, to see the
   condition under which it becomes 18/19 rather than 0;
2. since 18/19 sit right at the size of the host list (20 names, 20-entry table), check
   whether the value is a count of hosts completed and whether all 20 must answer;
3. the 01.00 twin of this test is the same shape (its types 20/30 answer with 21/31), so
   the 01.00 route's success value is worth reading for the expected encoding.

**Correction to the above: 18 and 19 are both *error* states, not success (static read).**
`0x69ddb0` is literally `lwz r3, 0(r3); blr` - the field is the same word the handler
dispatches on, so the value *is* that word. Sweeping the whole handler body for
`stw rX, 0(r30)` preceded by its `li` shows only two places ever write the two values
state 4 tests:

```
; value 19, at 0xb370b0 - immediately AFTER the range check
0xb3707c  lwz    r0, 0(r3)
0xb37080  cmplwi r0, 0x13
0xb37084  ble    0xb370e0        ; in range -> jump table
   ...falls through...
0xb370a8  bl     0xb38638        ; log (line 0x1b2 = 434)
0xb370b0  li     r0, 0x13
0xb370b8  stw    r0, 0(r30)      ; 19 = "index out of range"

; value 18, at 0xb379e8 - also a log-then-store pair
0xb379cc  li     r8, 0x10c       ; log line 268
0xb379e0  bl     0xb38638
0xb379e8  li     r0, 0x12
0xb379ec  stw    r0, 0(r30)      ; 18 = another logged error state
```

So state 4's exit test is `result == 18 || result == 19`, and **both of those are
diagnostic error states the handler writes after logging**. The client therefore leaves
the probe state only on a failure. Since neither ever occurs in our runs (193/200 getter
calls returned 0), the field is sitting in a normal intermediate state and the client is
waiting for a transition that the handler never takes for us.

Two consequences, and they are the useful part:

1. the "success" value we should be looking for is **not** 18 or 19 - it is whatever the
   handler writes on the *non-error* path, i.e. one of the `stw` stores belonging to a
   live jump-table branch (indices 7, 8, 14, 15 write 2, 3, 3, 1 respectively; the rest
   branch onward before storing). Which of those means "STUN complete" is what to
   determine next;
2. the reply-table index **is** what selects that path - the index decides which branch
   runs and therefore which value lands in the field. So the earlier sweeps were testing
   the right thing after all; they just never guessed the index whose branch is the
   success path, and `--reply-index` cannot be used to aim at it because the index is
   read from the client-side field rather than from our packet.

That last point is the real obstacle and worth being explicit about: the branch is chosen
by a word the client computes, so the way to reach the success path is to satisfy the
condition that makes the client compute that word - not to choose a reply. The hook on
`0x69ddb0` is the instrument for it: watch the field across a run whose replies differ in
one field at a time, and the value that first appears as 18-or-anything-else is the
success encoding.

**"All 20 hosts must answer" is not it either (RUN ALL20).** Since 18/19 sit right at the
size of the host list, the obvious guess is that the field counts completed hosts and the
client waits for all 20. Tested properly: `--stun-distinct --bind-count 20` so each of the
20 names resolves to its own address and is answered by its own socket, with the
`typeversion` reply. Result: 20 DNS answers (all distinct), **0 probes reached the stub**,
and the field still returned **0 in 198 of 200** getter calls (1 once, one stray value).
No BAP connection, no sign-on request.

So the field is not a host-completion counter either. Combined with the earlier
measurement (RUN GATE18, same 193/200 zeros) the conclusion is stable across both
configurations: the word is not counting anything we influence. It is most likely a
**sub-state within the STUN sequence** that only advances when the client is satisfied
with a *specific* reply it has not yet been given - which brings us back to needing the
real reply bytes rather than more shapes to try.

**What 01.00 actually needed, for comparison (from TEST005/006 in `test-log.md`).** This
is the most useful reference in the repo and it sharpens the question:

- 01.00 sent two probes, `1e 03 00` (IP discovery) then `14 02 00 xx` (NAT discovery);
  `stun_stub.py` answered them in the BO4 layout (types 30/20 -> 31/21) and **marionberry
  cleared**. That is the whole reason 01.00 got as far as it did.
- The **same TEST006 entry records a 29-byte type `0x0e` packet repeating every 15 s to
  the STUN host, unanswered** - and 01.00 still proceeded. So 01.00 *tolerated* an
  unanswered `0x0e`; it did not need it to succeed.
- 01.24 sends only `00 01 00 01` and `0e 02 ...`. We answer both, on the same
  loopback-and-stub setup that worked for 01.00, and it still loops.

That is the real asymmetry. 01.00's gate was satisfied by the *type 30/20 exchange alone*
and treated `0x0e` as noise. 01.24 has no 30/20 exchange at all, so the gate must be
carried by something else - and since answering `0x0e` correctly is the only thing left in
this exchange, the `0x0e` reply is where the answer is. Guessing its contents has failed
seven ways now (index, fill, shape, per-name addresses, reply type, reply version,
host-count), which is strong evidence the reply is not a short fixed record like 01.00's -
it is more likely a structured payload whose length and field order matter, which is
exactly what a real capture would settle and what no amount of sweeping will.

**Control run (RUN LEGACY): the stub with no 01.24 replies at all**, reproducing the
01.00-era behaviour, to confirm the difference is in the reply rather than the setup.
Result: 21 probes, 21 replies (all the 01.00 types), **the field still 0 in 195 of 200**
getter calls, no BAP connection, no sign-on request.

That is the cleanest result of the session. With `--reply-index` at its default the stub
still answers (the 01.00 branches are tried first and then 01.24), and the measured field
distribution is **byte-identical to RUN GATE18 and RUN ALL20** (195/193/198 zeros, the
occasional 1, one stray pointer value). So across every configuration tried - 01.00
replies, 01.24 replies in four shapes, four reply types, two versions, per-name addresses,
all-hosts-answering - the value the client keeps in that field never changes.

**What that means.** The field is not a function of anything we transmit. Whatever the
client needs in order to leave the probe state, it is not obtaining from this exchange at
all in any form we have tried. Either

1. the `0x0e` reply has to be substantially correct - a structured payload, not the
   short address record we have been sending - and no short variant will do; or
2. the client is waiting on a *second* channel (a different port, a different host
   family, or a result delivered by the platform layer) before it will set that state,
   in which case no STUN reply is relevant at all.

Distinguishing those two needs one more piece of information that only the client or a
real capture can give: what the client does with the bytes after the type/version header
in the `0x0e` request, i.e. the field layout at offsets 2..28 that we have never read
(`0xb4695c` builds the header, but the body is written elsewhere). That is the single
highest-value next task, and the PPU hook on `0xb4695c` with a wider dump is the tool for
it.

**The `0x0e` body layout, as far as the captured bytes go (RUN BODY).** Hooking
`0xb4695c` with a 0x60-byte dump shows the packet buffer is **all zeros at function entry**
(60/60 hits identical) - the type byte, the constant 2 and the body are all written inside
it, so that address is the builder, not the send site, and it cannot be dumped usefully
from an entry hook. The bytes that actually went out are however recorded exactly by the
stub, and they are highly structured:

```
[0]      0e        type (14)
[1]      02        version (2)
[2 ..17] all zero  (16 bytes)
[18]     ff 00     unset
[20]     ff 00     unset
[22]     00 00
[24]     ff 00     unset
[26]     ff 00     unset
[28]     00        (odd byte)
```

Read as big-endian u16s from offset 2 the body is `0,0,0,0,0,0,0,0, 0xff00, 0xff00, 0,
0xff00, 0xff00` - a **fixed 26-byte body with four `0xff00` markers**. The pattern says
these are fields the client has no value for yet (an unbound socket/port, or a
not-yet-known address), in the same way the earlier `-1` reading was an "unbound"
sentinel. So the request is a fixed record asking the server to fill in four slots, and
**the reply is presumably the same record with those slots populated** - a different shape
from the short 9-byte address record we have been sending, and the most concrete reason
the seven reply variants all failed.

The four `0xff00` fields sit at offsets 18, 20, 24 and 26, with a plain zero at 22
between them, so the record is not a simple array - there is structure around them worth
reading in the client's own writer before the next run.

**Corrected: the markers are 2-byte fields, not address slots (RUN MIRR).** The first
implementation of `--shape mirror` treated them as 6-byte address slots and a unit test
caught it immediately - offsets 18 and 20 are only 2 apart, so a 6-byte slot at each
overlaps. They are small `u16` fields with `0xff` meaning "unset" (ports/flags), and an
address cannot be pasted in. Implemented as `--mirror-fill zero|port|keep`:
`port` puts the mapped port (0x0c02) big-endian in each marker, `zero` clears them,
`keep` echoes unchanged. The reply is otherwise the client's own record, length and
header preserved:

```
request  0e 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 ff 00 ff 00 00 00 ff 00 ff 00 00
mirror   0e 02 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00 0c 02 0c 02 00 00 0c 02 0c 02 00
```

Result: 21 probes, the reply delivered and well-formed, **still no BAP connection and
still no sign-on request**. So even answering in the record's own shape, with its unset
fields populated, does not move the client. That is the eighth reply variant ruled out,
and it strengthens the earlier conclusion rather than overturning it: the STUN reply is
not the gate, or the fields the client needs are not the ones carrying the `0xff` markers.

I would not spend more of this route on reply shaping. The evidence now says the
information needed is the *real* exchange - either a capture from the owner's own console,
or a much deeper read of the client's STUN success path (the `stw` stores in the live
jump-table branches at `0xb3724c`/`0xb372bc`/`0xb371e0`, none of which we have entered).

**WHERE THE REPLY IS ACTUALLY REJECTED (RUN BRANCH) - the real gate, at last.** Watching
the four live jump-table branches plus the table itself shows the handler almost never
reaches the table at all:

```
0xb36fd8  handler entry       : 13 hits
0xb370e0  the jump table      :  1 hit    <- one dispatch in an entire run
0xb371e0  index 15 (stores 17) :  0
0xb3724c  index 7  (stores 2)  :  0
0xb372bc  index 10            :  0
0xb374e4  index 9             :  0
```

So replies are not being mis-dispatched - they are **rejected before the dispatch**, on
almost every call. The test that does it is a few instructions into the handler:

```
0xb36fe8  lbz   r0, 4(r3)
0xb36ff4  cmpwi r0, 0
0xb3700c  beq   0xb3707c        ; == 0 -> carry on
   ... sets the field to 0x11 and returns (the "already have a result" path)
0xb37034  lwz   r0, 8(r30)      ; field at offset 8
0xb37038  cmpwi r0, 0
0xb3703c  bne   0xb3704c        ; non-zero -> carry on to the range check
0xb37040  lwz   r0, 0xc(r30)    ; field at offset 12
0xb37044  cmpwi r0, 3
0xb37048  beq   0xb3714c        ; == 3 -> carry on (the short-probe branch)
0xb3704c  bl    0xb35b38        ; otherwise: give up
```

and the captured session object has **offset 8 = 0 and offset 12 = 0** (the 32-byte hook
dump is `00000001 00000000 00000000 c0a80001 ...`). Both conditions therefore fail on
every call, and the handler bails through `0xb3704c` - which is why the jump table is
reached once in thirteen calls and why no reply we send has mattered.

**This finally names the two fields the stand-in has to influence**, and they are not in
the packet we were editing at all:

- `8(r3)` - a non-zero value here short-circuits the whole dispatch, and
- `0xc(r3)` - the value **3** is the other way through, via the short-probe branch.

Both are written by the client before it calls the handler, most likely by the routines
that fill in the address it just learned from our reply. So the chain is:
**reply arrives -> the client copies our address into its session object -> only then does
it reach the table.** Our replies are apparently not being copied, which is the first
thing in this whole investigation that is a *client-side* condition we can name rather
than a value we failed to guess. The next task is unambiguous: find the writers of
`8(r3)` and `0xc(r3)` - they are the code that consumes the reply body - and read what
they require of it.

**The loop is self-sustaining, and that closes the mechanism (reading `0xb35b38`).** The
bail-out the handler calls when the gate fails is not a passive return - it **resets the
state it just tested**:

```
0xb35b64  stw r27(=0), 0(r28)     ; clears the dispatch word
0xb35b68  stb r27(=0), 4(r28)     ; clears the byte at offset 4
0xb35b6c  stw r27(=0), 0xc(r28)   ; clears the very field the gate wants to be 3
```

So each failed call sets `0xc(r3)` back to 0, guaranteeing the next call fails the same
test, which clears it again. The client cannot escape by retrying - something has to
write a non-zero `8(r3)` or a 3 at `0xc(r3)` in the first place, and the only writer found
that does either is **jump-table index 15** (`0xb371e0`):

```
0xb371ec  stw r0(=1), 8(r3)      ; non-zero 8(r3) -> short-circuits the gate entirely
0xb371f0  stw r9(=0x11=17), 0(r3)
```

and our runs enter that branch **zero** times. The whole picture is therefore consistent:

1. the client sends a probe and waits;
2. our reply arrives and is handed to `0xb36fd8`;
3. the gate at `0xb37034` fails because the client has not yet accepted an address, so the
   handler bails at `0xb3704c`;
4. the bail clears the state, so the next attempt behaves identically;
5. the branch that would satisfy the gate (index 15) is never reached.

**The single remaining question is narrow and answerable:** what does the client require
in the reply for it to write index 15, or any `8(r3) != 0`? That is the reply-consumer
code, upstream of the handler, and the `0xb34ed8` / `0xb35398` hooks already watching that
path are the instrument. Everything else in this section - the eight reply variants - sat
downstream of a gate that never opened, which is why none of them could have mattered.

**ANSWERED - the reply consumer, `0xb35c94`, and the accept set (read, 21 instructions).**
This is the only writer of the gate field in the whole STUN region, and it is short enough
to read in full:

```
0xb35c94  mr    r11, r3
0xb35c98  cmpwi cr6, r4, 1
0xb35c9c  lwz   r0, 0(r11)      ; r0 = the reply's first word
0xb35ca0  cmpwi cr7, r0, 0
0xb35ca4  beqlr cr7              ; r0 == 0 -> return, no state change
0xb35ca8  cmpwi cr7, r4, 0
0xb35cac  beq   cr6, 0xb35ce4   ; r4 == 0 -> b 0xb35b38 (the bail)
0xb35cb0  cmpwi cr6, r0, 0x11
0xb35cb4  bnelr cr7
0xb35cb8  cmpwi cr7, r0, 0x0e
0xb35cbc  beqlr cr6
0xb35cc0  cmpwi cr6, r0, 0x0f
0xb35cc4  beqlr cr7
0xb35cc8  cmpwi cr7, r0, 0x10
0xb35ccc  beqlr cr6
0xb35cd0  beqlr cr7              ; (0x10 path lands here)
0xb35cd4  li    r0, 1
0xb35cd8  stw   r4, 8(r11)       ; <-- writes the gate field 8(r3)
0xb35cdc  stb   r0, 4(r11)       ; <-- sets 4(r3) = 1, the flag the handler reads first
0xb35ce0  blr
0xb35ce4  b     0xb35b38         ; the bail
```

So the reply is accepted **only if its first word is one of `0x0e, 0x0f, 0x10, 0x11`** and
`r4` is non-zero. On acceptance the client writes `8(r3) = r4` and sets `4(r3) = 1`, which
is exactly what the handler's gate at `0xb36fe8` (`lbz r0,4(r3)`) and `0xb37034`
(`8(r3) != 0`) require - so acceptance means the next handler call reaches the jump table.

**The accept set is `{14, 15, 16, 17}` and nothing else.** Two things follow, and both
matter:

- **18 and 19 are rejected.** That independently confirms the earlier reading that they are
  error states: the client's own reply consumer refuses them. The state 4 exit test
  (advance on 18 or 19) is therefore only reachable via the bail path, not via a good reply.
- **`0x0f` is in the set**, so the `typeversion` reply (type 15) we have been sending passes
  the type check. What is still unknown is `r4` - the value stored into the gate field - and
  where it comes from. It is a second parameter to `0xb35c94`, supplied by its three
  callers (`0xb337c8`, `0xb337f0` in `0xb3376c`, and `0xb33ec0` in the transport machine),
  so it is derived by the caller rather than from the packet, which is why no reply-side
  experiment could have moved it.

The next step is therefore to read the three callers and find how `r4` is computed. That
is the last untraced link between "a reply arrived" and "the client accepts it".

**`r4` is a success flag, not a parsed value - and the transport machine passes 1.** Both
remaining callers are short:

```
; the transport machine, 0xb33ea4 (the path that runs during a probe)
0xb33ea4  cmpwi cr7, r3, 0
0xb33ea8  bne   0xb33eb8
0xb33eac  lbz   r0, 0xa8(r31)     ; a status byte
0xb33eb0  cmpwi cr7, r0, 0
0xb33eb4  beq   0xb33ef4         ; still busy -> skip
0xb33eb8  mr    r3, r26          ; the session object
0xb33ebc  li    r4, 1            ; <-- r4 = 1, "this is a good reply"
0xb33ec0  bl    0xb35c94

; the 0xb3376c caller
0xb337bc  li    r4, 0            ; <-- r4 = 0, the bail path
0xb337c8  bl    0xb35c94
```

So `r4` is a **caller's verdict**, not anything parsed out of the packet: the transport
machine passes 1 when a reply has arrived and the status byte `0xa8(r31)` is clear, and
`0xb3376c` passes 0 to force the bail. The gate field `8(r3)` therefore just receives
"a reply was accepted", and the real condition for acceptance is entirely inside
`0xb35c94`: **the reply's first word must be 14, 15, 16 or 17.**

That is a testable, falsifiable statement, and it explains every observation in this
section:

- our `typeversion` reply (type **15**) and the `mirror` reply (type **14**) both have an
  accepted first word, so `r4 = 1` should have stored a non-zero `8(r3)` and set `4(r3) = 1`;
- yet the hook shows `8(r3) = 0` and `4(r3) = 0` on every handler entry, and the jump table
  is reached once in thirteen.

The two facts can only both be true if `0xb35c94` is **not being called with our bytes at
all** - i.e. the caller's `0xa8(r31)` status byte is still non-zero, or the call at
`0xb33ec0` is never taken, so the reply never reaches the consumer. That is a *pre-consumer*
condition, upstream of everything the stand-in can influence.

**This is the sharpest statement of the blocker so far, and it is checkable in one run:**
hook `0xb35c94` itself. If it is never entered, the reply never reaches the consumer and
the whole question is what keeps `0xa8(r31)` non-zero. If it is entered and the store
still does not happen, then the first word the client reads out of our packet is not what
we think it is - and the dump at that hook will say exactly what it is.

**CHECKED (RUN CONSUMER): the consumer is never entered. Zero hits.** With `0xb35c94`
(the consumer) and both call sites `0xb33ebc` / `0xb337bc` watched, alongside the
handler and the table:

```
0xb35c94  the reply consumer         :  0 hits
0xb33ebc  the transport call site    :  0 hits
0xb337bc  the other call site (r4=0) :  0 hits
0xb36fd8  the handler                : 13 hits
0xb370e0  the jump table             :  1 hit
```

**This closes the investigation and settles where the work has to go next.** The handler
runs 13 times, so the client *is* servicing the socket and *is* reaching the code that
decides whether a reply counts. But it never calls the one function that would record it
as accepted, and neither call site that could call it is ever executed. The rejection
therefore happens strictly **upstream of the reply consumer**, in the transport machine's
own logic between receiving the datagram and deciding to pass it on - most likely at the
status byte `0xa8(r31)` test at `0xb33eb0`, which is the one condition that can skip the
call entirely.

That also retro-justifies the whole session: eight reply variants, four types, two
versions, per-name addressing and a control run all produced byte-identical results
because **the stand-in's reply never reached the code that would judge it**. Not one of
those experiments could have succeeded, and no further change to `stun_stub.py` can
succeed either. The reply is not the lever.

**What the next session should do, in order:**

1. read the transport machine between `0xb33814` and `0xb33ec0` to find what sets
   `0xa8(r31)` and what else must hold for the `0xb33ec0` call to be taken;
2. check whether the datagram is parsed at all before that - the handler's 13 entries may
   be servicing a *timeout* rather than our reply, which would mean the socket is polled
   but the payload is never read;
3. only then revisit the reply body.

Point 2 is the cheapest test of all and should go first: make the stub's reply length
distinct and see whether the handler's entry count changes with it. If it does not, the
handler is on a timer and nothing we send is being parsed at all.

**DONE (RUN LEN7 vs LENBIG): the handler is on a timer, and the reply length is
irrelevant.** Two runs, identical except for the reply we send - a 7-byte `typeversion`
reply and a 29-byte `mirror` reply - and every number matched exactly:

| | probes | replies | handler `0xb36fd8` | table `0xb370e0` | `0x69ddb0` | sign-on | BAP |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| LEN7 (7-byte reply) | 17 | 17 | 13 | 1 | 400 | 0 | 0 |
| LENBIG (29-byte reply) | 17 | 17 | 13 | 1 | 400 | 0 | 0 |

The handler is entered **13 times on a fixed cadence regardless of what arrives**. So
those 13 entries are not "13 replies processed" - they are 13 timer ticks. The client's
own probe count is 17 and the stub sees 17, so probes do go out, but the reply path is
never driven by our datagram at all.

That is the sharpest possible statement of where the work is, and it is consistent with
the consumer never being called: **the client is not reading our replies.** The likely
causes, in order of how cheaply they can be checked:

1. the client expects the reply from an address other than the one it sent to - the stub
   answers from `127.0.2.2:3074` (its own bind), while the client's socket is
   `127.0.0.1:3074`, and a NAT-oriented client may require the reply to appear to come
   from the server it dialled. This is very likely, and it is a one-line stub change:
   answer the 4-byte probe from a socket bound to the *peer's* view, or check whether the
   client discards datagrams whose source address differs from the socket it connected
   to;
2. the reply must arrive before some deadline the timer path is already past, so the
   handler never re-reads the socket;
3. the client is waiting on a different port entirely.

Cause 1 is the one to test next, and it costs a single run.

**Cause 1 disproved by inspection; the client does receive the datagrams.** Reading
RPCS3's own network trace in RUN LENBIG settles three things at once:

```
5  x  sendto(s=52, len=4)    the 4-byte probe socket
12 x  sendto(s=53, len=29)   the 29-byte probe socket
16319 x bnet_recvfrom in total, of which 5472 on sockets 52/53
bnet_recvfrom(s=52, buf=*0xd018b46c, len=32, flags=0x0, ...)
```

- the addressing is already correct: the client sends from `127.0.0.1:3074` to the stub's
  `127.0.2.2:3074`, and the stub replies from `127.0.2.2:3074` to `127.0.0.1:3074` - exactly
  what a real NAT server does, so the source-address hypothesis is wrong;
- the receive buffer is **32 bytes**, and both reply sizes fit (7 and 29), so truncation is
  not the problem either;
- **the client is reading its socket thousands of times.** It receives our replies and
  does not act on them.

Also worth recording: the client never *connects* to port 3074 - the only outbound
connections in the whole run are 20 to `127.0.0.1:53` (DNS) and 3 to `192.168.0.1:1900`
(SSDP/UPnP). Sockets 52/53 are only ever `bind`-ed, and the probes go out via `sendto`
with the destination pre-set. So the STUN exchange is connectionless by design, which is
consistent with the client's own STUN client.

**Revised position.** The reply arrives, is read into a 32-byte buffer, and is discarded
without reaching the consumer. So the filter is inside the client, between `recvfrom` and
`0xb35c94`, and the only condition identified so far that can skip the call is the
status byte `0xa8(r31)` at `0xb33eb0`. Two concrete next steps:

1. hook the `bnet_recvfrom` wrapper's **caller** - or, easier, the first client
   instruction after the syscall returns - to see how many bytes came back and what it
   does with them, since the 5,472 reads and 13 handler entries do not line up and one
   of the two numbers is about something other than the reply;
2. read `0xa8(r31)`: who sets it, and what it means. If it is only cleared on a reply the
   client considers well-formed, then its writer is the real gate.

**Who writes `0xa8(r31)` (part 2, in progress).** The byte is not written inline in the
transport machine, so it has to be copied in as part of a larger record. Scanning the STUN
region for `0xa8(` references (in windows - a single large read silently decodes zero
instructions, the same trap as the capstone mode bug) gives:

```
0xb3d304  stw  r0, 0xa8(r31)
0xb3d674  lwz  r7, 0xa8(r31)
0xb3e494  lwz  r0, 0xa8(r29)
0xb3ee2c  stw  r0, 0xa8(r29)
0xb3f2d0  stw  r9, 0xa8(r31)     <- inside a struct initialiser
0xb3f4d4  lwz  r7, 0xa8(r29)
0xb3f4f0  stw  r7, 0xa8(r28)
```

`0xb3f2d0` is the interesting one: it sits in a run of stores that copies a whole result
structure into the object one field at a time -

```
0xb3f2b0  std  r0, 0xd0(r31)
0xb3f2bc  std  r9, 0xd8(r31)
0xb3f2c8  std  r0, 0xe0(r31)
0xb3f2c4  lwz  r9, 0x128(r1)     <- the status field, loaded first
0xb3f2d0  stw  r9, 0xa8(r31)     <- and stored into the object
0xb3f2d4  std  r0, 0xe8(r31)
```

It belongs to `0xb3e9e0` (817 instructions, one caller at `0xb3f71c` in `0xb3f6a4`) - a
**result-structure builder**. So the status byte the gate tests is one field of a record
produced elsewhere and copied wholesale, not something the receive path sets. That is
consistent with everything observed: the reply arrives, but the record that would carry
"a reply was accepted" is built somewhere we have not looked, and the transport machine
only consumes it.

**So the remaining trace is a single call chain:** `0xb3f6a4` -> `0xb3e9e0` (builds the
result) -> `0xb33ec0` (consumes it). Reading `0xb3f6a4` and whoever calls it should show
where a received datagram becomes that record - and therefore what the reply has to
contain to produce a status byte of 0.

**RUN CHAIN: the whole chain is dead. Zero hits at every level.** Watching all of it at
once, alongside addresses known to be live so the hook mechanism is demonstrably working:

```
0xb35c94  the reply consumer          :  0 hits
0xb33ebc  its transport call site     :  0 hits
0xb3e9e0  the result-record builder   :  0 hits
0xb3f6a4  the builder's only caller   :  0 hits
0x121128  the top of the chain        :  0 hits
0xb36fd8  the handler                 : 50 hits   <- live
0xb4695c  the request builder         : 60 hits   <- live
0xb34ed8  the field reader            : 37 hits   <- live
0xb35398  the receive path            :  5 hits   <- live
0x69ddb0  the gate getter             :400 hits   <- live
```

The chain from `0x121128` down to the consumer **never executes**, while the addresses
immediately around it fire constantly. So the result record is not built, the status
byte is never written, and the consumer is never called - which is a complete,
self-consistent explanation of the stall that needs no further speculation:

> the client receives the datagram and runs its receive path, but never produces the
> "a reply was accepted" record, so it keeps the probe loop open.

**What this rules out.** The whole reply-shaping avenue, definitively: the code that would
judge a reply is downstream of a builder that never runs, so no byte we could put in a
packet changes anything. That is why nine variants produced identical results, and it is
why this should not be attempted again.

**Where the work actually is now.** `0x121128` is 569 instructions with one caller, and
its first action is `lbz r3, 1(r3)` on the value built at `0x19e260` by
`lis r3,0x1a2` / `addic r3,r3,-0x3da0` - a **module-state flag tested against 0**, i.e. a
lifecycle check ("is the STUN module idle?"), not a packet test. Since the chain is
one-caller-deep, the most likely reading is that `0x121128` is a *startup/registration*
routine for the STUN module rather than a per-datagram path, and that the per-reply work
happens on a different, so-far-untraced branch.

Next session's first task, therefore, is no longer the reply body. It is to find what
*does* call the consumer in a working case - and the fastest way to get there is a
capture of a real 01.24 STUN exchange from the owner's own console, which would show
both sides of an exchange that the client actually accepts. Everything above is a
faithful reading of the 01.24 binary and every run so far, but it is inference from one
failing configuration, and this last result says the missing piece is not in the reply
path at all.

**The `ENOTCONN` noise is a red herring, now confirmed twice.** RUN SHAPEBIN logged 227
`Socket error ENOTCONN` and 614 `sys_net_infoctl` lines. The `infoctl` calls are
RPCS3's own socket polling (`cmd=53`, `cmd=9`) on its UDP socket, not our traffic, which
matches what the earlier 01.24 runs already found. Chasing socket-error counts will not
move this forward; the signal that matters is "did the client resolve a new hostname" and
"did anything connect to the BAP stand-in", neither of which has happened.

**Toolchain note.** `scripts/d1_eboot.py disasm` had never worked: it requested
capstone `CS_ARCH_PPC` + `CS_MODE_64`, which capstone does not support and which
silently yields zero instructions. Fixed to `CS_MODE_32 | CS_MODE_BIG_ENDIAN`, and the
address arguments now accept a `0x` prefix. `capstone` must be pip-installed for
`disasm`; nothing else in the tool needs it.

**Passing array arguments to these scripts.** `Start-LocalStack.ps1`/`Start-FinalBuildSmoke.ps1`
take `-StunArgs`, but it must be one **semicolon separated** token. Under `pwsh -File`
a second value of a `[string[]]` parameter binds to the *next* parameter instead of
joining the array, so `-StunArgs '--reply-index' '0'` silently passed `0` as
`-PpuHash` and preflight failed with `FAIL invalid PPU hash`.

**Core activity-host payloads (static encode/decode checks):** the first audit found 01.00
offsets that the 01.24 encoder silently omitted. The stand-in now derives these fields from
the selected schema:

| Field | 01.00 | 01.13/01.24 |
| --- | ---: | ---: |
| Join result / session ID | +0xf8 / +0xf9 | +0x100 / +0x101 |
| Activity scene world / map | +0x990 / +0x994 | +0x5c8 / +0x5d0 |
| Simulation-player owner | +0x18 | +0x3c |
| Simulation-player spawn flag | bool +0x48 | nested bool +0x6c/+0 |
| Game-mode trailing signed value / hash | +0x984 / +0x988 | +0x4c8 / +0x4cc |

The core preflight also roundtrips the world-state zone and peer masks, starter
gear, all three unlock tables, and both mission scripts against the saved 01.00
schema and the corrected 01.13/01.24 exports. It verifies serialization; the
01.24 client handlers and the wrapped spawn flag still need a game run.

The EBOOT-only `types_01.24.json` has no web-service ID table. The BAP stand-in
takes `--services-schema` to overlay the package-derived 01.24 map in the data
repo (`ps3_updates/services_01.24.json`). It covers all 94 service IDs;
`scripts/d1_build_preflight.py` checks their type handles before a run.

**Web services (CONFIRMED):**

- The table moved from the 012c tag 0x80a5806b (now an empty 60-byte record) to tag
  **0x80a5e21a in package 012f**. The investment root is still 0x80a58000, and most of its
  children now point into 012f.
- The record format (36 bytes, element class 0x80801820) is unchanged. There are 94 services,
  up from 66.
- **All 66 old services keep their wire ids and their type handles.**
- 28 are new:
  - 208, 209, 211;
  - 406;
  - 804;
  - 904, 905;
  - 1228–1233;
  - 1305–1307;
  - 1601, 1602, 1606, 1609–1614;
  - 1701, 1702;
  - 1801.
- Static response shapes put **208, 209, 211, 1305, 1306 and 1601** first on
  the capture list: their replies carry fields beyond the common result/status
  shape. The other 22 new replies have only a result field or a result plus a
  32-bit status field. This says nothing yet about which services the client
  calls or what values it expects.
- 19 old services changed a request or response payload (often only through a nested type).
  Those the stand-in answers: **104** universe (both), **503** sign-in (response `0x80801825`
  0xe0 → 0x3c8), **201/202/203/205** (responses). Service **206** (family fetch) is unchanged.
- Service 104 no longer carries the universe object: 01.13 has a result-only
  response and 01.24 adds a bool at +0x8. The stand-in now sends the universe
  object there only when the selected response schema has that struct.
- Service 503 still carries the universe object at +0x18. In 01.24 its flag
  override list grows from 20 to 50 entries and its optional fields expand.
  The former hand-written sign-in reply decoded a populated override list
  incorrectly on 01.24. It now uses the selected schema for that reply.
  The build preflight checks populated sign-in and service-104 replies on
  01.00, 01.13 and 01.24.
- `d1_types.py --packages <012f chain folder> --oodle <DLL> --service-tag 0x80a5e21a
  service-map services_01.24.json` exports the mapping and name hashes. The four update
  files `_1` through `_4` suffice because the table lives in `_4`; unrelated entries can
  refer back to the disc's `_0` package.

**Sign-on (CONFIRMED):**

- The nanopb success message (field 2) is identical, so the stand-in's reply format still
  fits.
- New optional fields:
  - request 20 (8 integers) and 21 (five strings of 64/64/128/32/32 bytes);
  - response 13 (string[64]) and 14 (u32).
- The request table is at 0x12e66dd and the response table at 0x12e6850. The handler is at
  0x28dbd4 (01.00: 0x126744). It decodes into a stack struct at r1+0x70:
  - field 13 is a **serialized blob, not a bare string**; its real location in the SignOn
    singleton is **+0x4460 (u64 pointer) and +0x4468 (u32 length)**, not +0x47cc as first
    written here. The store is at `0x28e170`/`0x28e174` (`std`/`stw` on the singleton base
    `r30`), guarded by `lbz r3, 0x508(r1)`; `0x28e6ec`/`0x28e6f0` write the pair again from
    another path. (The only literal `0x47cc` stores in the build are unrelated singletons at
    `0x1aa`, `0x1ab` and `0x1bc`; find them with `scripts/d1_offset_scan.py`.)
  - field 13's consumer is a **single accessor and a single caller**, which makes its purpose
    knowable offline:
    - `0x35af6c` is the accessor: `332614(3)` fetches the SignOn singleton, then
      `lwz r31, 0x4468(0x1aa49f8)` returns the blob **length** (0 if absent).
    - its one caller `0x490474` is a **3-iteration loop** (`cmpwi r30, 3`, `r31 += 0x1a0`), so
      the blob is parsed once per region. It first compares the incoming length against the
      cached `+0x468`; if it differs it reparses, else it skips — the same "only act when
      something changed" guard seen elsewhere in this codebase.
    - on a change it copies **0x178 bytes** into `+0x470` (`9a7540`, then `99e380`) and records
      the new signature: `+0x450` (u16 version), `+0x458` (bool), `+0x460` (f64), `+0x468`
      (length), `+0x46c` (u8). Only the success path bumps the object's own counter at
      `+0x5e8`, saturating rather than wrapping.
    - the actual decode is `0x490174` (also reached via `0x48ff28`): it dispatches on a type
      byte at `+0x1c`, and when that byte is 0 it takes the "write defaults" path, clearing
      `+0x454`/`+0x458`/`+0x460`/`+0x468`/`+0x46c` first.
    - the loop's caller is `0x5b4bb8`, which requires `475d50() == 3` (a count of **three**,
      matching three regions) and a guard byte at `+0x5080`; afterwards the same function walks
      **ten** 0x240-byte records (`cmpwi r31, 0xa`) through the expiry helpers
      `48eedc`/`48f2dc`/`48ef24`, which compare `+0x21c`/`+0x238` counters — those are timers,
      not host names.

    **Consequence for the stand-in:** an absent or empty field 13 is handled (length 0, the
    cached signature cleared and defaults written), so field 13 is not *required* for the
    client to continue. But the 0x178-byte record is the only sign-on-supplied configuration
    01.24 has left now that the Demonware lobby step is gone (see `Demonware lobby (LIKELY
    removed)` below). The region's `0x1a0` stride suggests one blob per region holding that
    region's endpoints. **The blob's contents are still undecoded**; what is confirmed here is
    the plumbing, not the schema.
  - field 14 is wrapped in an address record (u32, then u16 4 at +0x12) and handed to the
    `network_demonware` module (0x1326e8 returns it; 0x132cb4 validates and stores it at
    +0x184). Its STUN/relay code (0x132cf4) compares it with the module's own address (+0x154)
    and records whether they match, then passes it with a port to 0x1197bc.

  This is LIKELY the client's public IPv4 address as the sign-on server saw it, used for NAT
  traversal. `signon_probe.py --public-ip ADDR` sends it (optional; the byte order is not
  confirmed). **This is the one new field the failing 01.24 runs already send** (as
  `127.0.0.1`), and it feeds the STUN/relay/NAT path, so it is now the strongest single
  candidate for the "Passed Verifying Content to Preparing, then black screen with no BAP"
  stall. Its byte order is still unconfirmed, and a wrong one may be worse than omitting it:
  `0x132cf4` compares the value with the module's own address at `+0x154` and records the
  match. **Next experiment for the next PC session: run the same manifest with field 14
  omitted entirely (the `--public-ip` flag already makes it optional), so only one variable
  changes.**
- `SignOn?platform=%s&build=%s` and the two sign-on domains are unchanged.
- New: `stun1`–`stun8.signon.deadorbit.net` / `.gravityshavings.net` and
  `destiny-stun.{us,eu,jp,au}.demonware.net`. The DNS sinkhole already answers every name
  locally.

**Demonware lobby (LIKELY removed):**

- 01.24 has none of 01.00's `panther-ps3-{lobby,auth,auth3}.{prod,cert,dev}.demonware.net`
  names. It also lacks the strings of the 01.00 lobby client module around 0x42c9d8
  (`online_demonware memory`, `failed to connect to lobby`, `platform account id changed`,
  `BLUS30843`). 01.13 still has all of them.
- The Demonware library (version 44: bdCore, bdSocket, bdLobby/bdAuthService, bdNet) is still
  linked, and its strings now mention NAT punch, QoS probes and STUN.
- So the final client probably goes from sign-on straight to BAP. The auth3 → lobby 3074 step
  that 01.00 needs would be gone.
- How (and whether) 01.24 reaches a Demonware auth server is UNKNOWN. Candidates are response
  field 13 or the title configuration. **Check on the first boot:** log DNS queries and
  connections after sign-on.

**BAP (LIKELY unchanged):** the message names (`shello`, `sub req`/`sub rsp`, `qz->c`, `ws`)
are all present. A `network_bap_nat_punch_queue` was added.

**Error ids:** 01.24 drops the `error.*` string table (248 names in 01.00 → 1). An error code on
screen is therefore no longer found by string. Map it through the 01.00 table where the meaning
carried over.

**Patches for RPCS3 (CONFIRMED addresses; the PPU hash was derived offline):**

- The embedded sign-on CA is the identical 2,390-byte PEM ("Tiger Root Authority" + GeoTrust) at
  **0x12d3740**, used at 0x113894 (01.00: 0x11b7218).
- The content-manifest signature flag is **0x18dca64** (initially 1), read only at 0xc65d14,
  which has the same `cmplwi 0x108` shape as 01.00's 0xb6f3cc (01.00: 0x179d5f4).
- `game_ca_patch.BUILDS["01.24"]` holds both and the calculated key
  `PPU-ac58999f254e8d9dc817426c8c84b068ee8caaea`.
  `d1_ppu_hash.py` matches the known 01.00 key exactly. The algorithm follows
  [RPCS3's executable loader](https://github.com/RPCS3/rpcs3/blob/master/rpcs3/Emu/Cell/PPUModule.cpp):
  hash every ELF program header's type and flags, then each nonempty LOAD segment's
  address, memory size and file bytes. The first live boot should confirm the 01.24
  key in `RPCS3.log`.

**Packages and DLC (CONFIRMED strings):**

- The patch folder is built as `%s/game/%s/USRDIR/%08x/` with `/dev_hdd0` and the constant
  0xa0000001 (0xc951c, a case of the path-root switch at 0xc8758), i.e.
  `/dev_hdd0/game/<title>/USRDIR/a0000001/`.
- Other new path strings: `packages/%d\`, `%s_%s.pkg`, `unpatchable\packages`,
  `client_bootstrap_patchable` / `client_bootstrap_unpatchable`, and
  `Patchable/Unpatchable package registration failed with result %s (%d)` (0x366b88).
- New DLC strings:
  - `DLC Patch/CDN/Cache/Entitlement/Marketplace Containers`;
  - `dlc_installer error 0-7`, `dlc_url 0-7`;
  - `Apply quick patch (package='%s', is_asset='%s')`;
  - `World-transition requires DLC.` / `... requires permissions.`;
  - `bootflow:illegal_dlc_media`;
  - `bootflow:check_media: NPDRM check failed`;
  - `offer_key.edat`, `destiny.edat`.

**DLC entitlement gate in 01.24 (CONFIRMED, `0x36e92c`; the 01.00 twin is `0x334004`):**
this is the function that emits the two strings above. It is the gate that
[`bap_server.py`](../server/bap_server.py) already works around on 01.00 through
`--self-flags` (see `0x4105c4 world_transition_dlc_check`, the unlock-expression evaluator
that feeds it, and the orbit checkpoint's world 125/126 runs), so this is a relocation, not a
new mechanism. What is new is the shape in 01.24:

- `+0x660`, `+0x668`, `+0x670` are three optional DLC reference slots on the transition
  object, tested as `clrlwi`/`cmpdi` against 0 in that order, producing a **priority code**:
  0 = all three set, 1 = `+0x668` and `+0x660`, 2 = `+0x670` only, and `r28` stays -1 when none
  is set. The priority, not the slot, selects the path.
- `2a8080(4)` fetches the entitlement/permission service; if it is absent the check is skipped.
  Otherwise two virtual calls on it (vtable +0x2c and +0x8, the latter returning a bool at
  `+0x6b8c`) must both be true.
- `35cd7c` writes the resolved **licence id** into the caller's `r1+0x70`. `470384` returns
  **true when an id was resolved** and 0 only for -1 (it counts leading zeros of `id + 1`),
  i.e. it answers "does this transition name a DLC at all?". A resolved id takes the failure
  branch: `355d54(0x19, 0x17, 0x12fcc20)` ("World-transition requires DLC."), returning
  0x17 = 23. **Both pointers come from `lis r5, 0x130` + `addic r5, -0x33e0`/`-0x33c0`;
  `0x130cc20`/`0x130cc40` are the neighbouring UTF-16 Portuguese copies in a localization
  table, not the ones this code passes.**
- With no DLC named (id -1) the same service is asked again through `46db2c(licence)`, which
  returns non-zero to mean "permission granted". Zero reports
  `355d54(0x19, 0x18, 0x12fcc40)` ("World-transition requires permissions.") and returns
  0x18 = 24, matching the 01.00 observation that 0x17 is the DLC gate and 0x18 the next one.
- Only the success path sets `+0x30` of the transition object, resolves the activity handle
  through `337540`/`3375c0`, checks `332614(3)` and `359ec4() == 2`, and returns 1 at
  `0x36ec28`.

So the gates are 0x17 = "this world names a DLC you must own" and 0x18 = "you do not hold the
permission for it", and both read only client-side state: the transition object's DLC slots and
the local entitlement enumeration. The activity host is never consulted, which is why the
existing `--self-flags` answer for 01.00 carries over. The open item "DLC entitlement check" in
[`preservation-plan.md`](preservation-plan.md) is resolved for 01.24 as far as offline work
goes; what is left is a runtime confirmation that the 01.24 client evaluates the same unlock
expressions the stand-in already serves.
- New bootflow states: `check_media` and `confirm_account_transfer`.

## Reports

The data repo's `diff_*.md` and `types_*.json` were regenerated with the
current scanner and one-to-one matcher. To repeat that export:

```
python scripts/d1_types.py --elf <01.24 EBOOT.elf> schema types_01.24.json
python scripts/d1_builddiff.py --old-elf <01.00 EBOOT.elf> --new-elf <01.24 EBOOT.elf> --names 01.00,01.24 \
    --map-out handles_01.00_01.24.json --out diff_01.00_01.24.md
python scripts/ps3_update.py fetch BLES01857 --versions 01.13,01.20,01.22,01.23 \
    --match ps3_investment_globals_012f_1.pkg,ps3_investment_globals_012f_2.pkg,ps3_investment_globals_012f_3.pkg,ps3_investment_globals_012f_4.pkg --out <dir>
```

The final 012f chain is _1 (01.13), _2 (01.20), _3 (01.22) and _4 (01.23). 01.19's _5 is stale.
The extracted tag has 94 unique IDs and all 66 old wire IDs and request/response
handles match the saved 01.00 map exactly. All 188 handles are present in the
corrected 01.24 type export. The derived map is in `d1xenia-data`; the raw
update packages remain in local scratch storage.

**First-run manifest question (offline count):** The existing stand-in builds its content
manifest from the disc's package folder. With the `en` language filter that yields 160
package IDs. The non-superseded update inventory has 310 `en` or language-neutral IDs,
152 of them absent from the disc set (312 distinct IDs in the union). Six required update
filenames have the special `_unp1`/`_unp2` suffix and do not have a hex package ID in the
name; those are unpatchable bootstrap/UI packages. A sampled 01.23 package
(`ps3_investment_globals_012f_4.pkg`) retains the same header ID/value shape used by the
disc manifest generator. Whether the client registers these update-only IDs from the
content manifest or separately from installed patch metadata is **UNKNOWN**. On the first
run, preserve the sign-on manifest reply and package-registration lines before changing
the manifest; this count narrows a likely failure if the new content does not load.
