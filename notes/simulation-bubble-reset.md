# 01.24 inactive bubble-roster reset experiment (2026-10-02)

The Ghost squad apply/update gate at `0x20f858` uses its bubble index and
`0x1d8e5c`: the corresponding bit must be set in simulation `+0x844` and clear
in `+0x850`. JOIN79 proved that setting the first bit on an owned test emulator
allowed the client to spawn `sq_ghost__ghost`. A wire-only experiment is now
available; it has not yet proved the retail ownership protocol.

The two simulation fields should not be treated as the payload of activity-host
message 27. Its handler `0xe2efa8` decodes one biased 32-bit value and stores it
at activity `+0x6444`. That handler does not write these simulation flags.

Offline 01.24 disassembly instead gives this path:

1. Type 5 includes 65 changed-slot bits and a roster delta (`0x80800cf7`).
   Each 12-byte slot has optional peer identity at `+0`, optional uint16
   generation at `+8`, and a required active boolean at `+0xa`.
2. In `0x1d90e0`, a changed slot whose generation differs from the previous
   record sets its bits in both flags (`0x1d9168`, `0x1d918c`).
3. An inactive slot exits at `0x1d919c`, retaining both bits. An active slot
   proceeds through the remote-state path and ultimately clears both bits
   (`0x1d9404`, `0x1d942c`).
4. For the current bubble, `0x1d8b6c` waits for `0x1df358` to report all
   baselines ready, calls the simulation reset callbacks, then clears only the
   `+0x850` bit (`0x1d8cbc`). The first bit remains set, satisfying the squad
   gate.

`bap_server.py --ah-roster 0 --ah-bubble-reset 14:1` therefore keeps the client
in active slot 0 while changing inactive slot 14 to generation 1 with no peer
identity. A comma-separated list supports further explicitly selected slots;
the client slot, duplicates, and out-of-range slots/generations are rejected.
Every simulation rebuild and heartbeat preserves the same reset generation.
Increasing it every heartbeat would repeatedly reset that bubble.

For an already running connection launched with `--ah-control FILE`, append
`{"bubble_reset":"14:2"}` as a complete JSON line. This updates only slot 14,
retains other reset slots, and rebuilds the simulation packet. The generations
belong to that connection; shared startup options are not changed. Invalid
updates, including any client-slot collision, are rejected as a whole.
Earlier file lines are skipped when a connection starts. Use a generation that
differs from the client's last record after a transition has cleared the flags.

The option is experimental and disabled by default. It sends ordinary type-5
messages and requires no client memory writes. It makes no claim that the
inactive flag means a particular retail authority owner. The generation must
differ from the saved record; generation 1 assumes its initial generation is
0. If a session has already processed that generation, use the next value.

There is another condition to observe: `0x1d99cc` clears both current-bubble
flags when simulation `+0xc` is false. Simulation start `0x1d8468` initializes
the flags to all ones only when that mode byte is true. A reset whose bits
immediately disappear can therefore require tracing the transition path.
`0x1daf04` simply converts its bubble-index argument to a byte; it does not
look up a roster peer on its own.

The mode input is four bytes copied through `0x35c174 -> 0x33600c -> 0x335dac`
from the engine's change-world options to simulation `+0xc`. Initial load
handler `0x3338a0` reads them from its event payload at `+0x683..686`. The normal
producer `0x36e384` explicitly writes zero at `+0x683` (`0x36e608`), as does the
restart producer `0x3340e0` (`0x334354`). Thus this client load path deliberately
starts with a false first byte; an incoming type-11 setter for it is not established.

JOIN85 read-only samples verified that inactive slot 14/generation 1 produced
bit 14 set in `+0x844` and clear in `+0x850` during slice 113, with the mode byte
false. After the 112 handover both masks were zero. No client RAM writes were
used. This proves the roster path can affect the gate, but not that a squad
spawned or that its ownership survives a slice transition. A fresh generation
after the transition is the next experiment.

For owned read-only samples, a fresh hook at `0x1df644` or `0x1df358` gives the
embedded replication state in r3. Subtract `0x18` to find the outer simulation
before reading its masks; hooks at `0x1d9440` or `0x1d90e0` give the outer pointer
directly. Do not reuse an address from a different process or load.

## Live result: JOIN87 and JOIN88 (2026-10-03)

Both runs reproduced a protocol-only highway Ghost spawn. After the client
reported current slice 112, generation 2 for inactive bubble slot 14 rearmed
the reset path. A subsequent `sq_ghost` generation ran the squad handler and
the client reported `sq_ghost__ghost` (group `0xecf574b5`, kind 2/index 29).
JOIN87 took about 70 seconds from reset to member report; JOIN88 took about
21 seconds. Sending the squad state does not establish that baseline readiness
has allowed it to apply immediately. Neither run wrote client RAM.

The startup reset in slice 113 did not survive the transition. Apply a fresh
generation after arrival; replaying generation 1 is insufficient. This verifies
the mechanism for the highway Ghost, not every bubble or retail authority policy.
Breach illumination and Fallen spawning remain separate acceptance checks.

Validation: seven tests check changed-slot bits,
inactive records without a peer, unchanged client slot, generation bounds,
complete-line control parsing, collision rejection, and stable generations
while the tick advances, including an encrypted session. Encoding and decoding with
the private corrected 01.24 type export also preserves these fields; without
the option, output matches the previous packet encoding byte for byte. Client
behavior has the bounded JOIN87/88 evidence above: post-transition reset flags,
squad generation handler `0x20cc38`, and upstream kind-2 Ghost member reports.
