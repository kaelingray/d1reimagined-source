# Peer mesh, platform layer, and multiplayer authority - canonical reference

**State:** everything here is `static-client-inference` against the pinned 01.29 eboot
(SHA-256 `7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954`) except where a line
carries `observed-live` (BAP wire facts) or a dated DNS check. **No two-instance run has ever
happened.** The peer mesh has never carried a packet between two of our instances; the whole
authority model below is what the client *demands*, not what has been *served*.

This document supersedes and consolidates seven design docs, now archived under
`historicaldocs/design/` (see the Sources table at the end). Banners in those docs override their
bodies; every claim killed by a banner is listed in "Dead claims" and is **not** live anywhere in
this document.

## Contents

1. [Architecture and lineage](#1-architecture-and-lineage)
2. [The server surface - twelve consultation points](#2-the-server-surface---twelve-consultation-points)
3. [Identity and key material](#3-identity-and-key-material)
4. [The sessions API plan (C7)](#4-the-sessions-api-plan-c7)
5. [Authority model](#5-authority-model)
6. [The two-instance harness](#6-the-two-instance-harness)
7. [Vision: anticheat, comp client, paired recorder](#7-vision-anticheat-comp-client-paired-recorder)
8. [Open unknowns](#8-open-unknowns)
9. [Dead claims](#9-dead-claims)
10. [Sources](#10-sources)

**Evidence tags:** `[C]` confirmed, `[O]` observed not reproduced, `[R]` reported, `[I]` inference,
`[U]` unknown. The authority sections also use the finer variants from the source dossiers:
`[C-client]` (confirmed in the pinned eboot), `[C-Bungie]` (stated in primary published Bungie
material), `[C-project]` (measured by this project). Reach tags (`observed-live`,
`static-client-inference`, `synthetic-minimal`, `synthetic_ancestor`, ...) per CLAUDE.md.

---

## 1. Architecture and lineage

### 1.1 The stack - Bungie above, Demonware below

Bungie licensed Demonware for transport and high-level networking and wrote the Halo-lineage
replication layers above it. The GDC 2011 talk ("I Shot You First", Aldridge) explicitly scopes out
sockets, NAT traversal, and "high-level networking" - those exclusions are exactly the `bd*`
symbols. `[C]`

```
GAME CODE            Bungie C++ (Tiger engine)
GAME INTERFACE       scrapes game state for replication      <- covered by the talk
PRIORITIZATION       decides what is most important to send
REPLICATION          implements the Tribes protocols
CHANNEL MANAGER      flow + congestion control               <- named, not covered
TRANSPORT            bdSocket, bdPlatformSocket, bdSocketRouter,   <- DEMONWARE
                     bdUnicastConnection, bdPacket/bd*Chunk, bdUPnP
HIGH-LEVEL NET       bdAuth, bdLobby, bdSecurityID/Key(Map), bdDTLS,  <- DEMONWARE
                     bdCrypto, bdJSON, bdHTTPWrapper, bdSCEHTTPLibWrapper
```

Demonware context `[C]`: Irish, founded 2003, acquired by Activision May 2007; main CoD backend is
Erlang; players' UPnP tables showed `DemonwarePortMapping` on ports 1200+ during Destiny sessions.
`[I]` Bungie almost certainly declined Demonware's State Engine; they had their own Tribes-derived
replication.

Measured in the 01.29 eboot: **72 distinct Demonware `.cpp` build paths** (bdCore 20, bdSocket 18,
bdLobby 13, bdConnection 8, bdNet 7, bdPlatform 6); path form
`c:\bungie\external_libraries\DemonWare\45\...`. `45` is confirmed as a directory level; what it
denotes is `[U]`. **Both `bdBitBuffer` and `bdByteBuffer` exist as separate named classes** (plus
`bdPacketBuffer`) - bit packing is confirmed at the service-protocol level, not just in gameplay
replication. `[C]` The `BD_*` strings are asserts and config constants (`BD_ASSERT`,
`BD_MAX_MESSAGE_SIZE`, `BD_NAT_OPEN|MODERATE|STRICT`), **not** an error-code enum.

### 1.2 The front door is TCP + AES-128-GCM, not DTLS - `observed-live`

Measured on the wire, not inferred: the client dials **TCP** `10.77.0.2:3075`, sends a
6-byte-envelope framing (`u8 version=1`, `u8 mode`, `u32be length`), completes a channel-start then
a secure-hello handshake sealed with AES-128-CBC + HMAC-SHA256, then switches to a secured channel
using **AES-128-GCM** (12-byte directional nonce, 16-byte tag *before* the ciphertext, no AAD). A
real captured packet authenticates under keys we issued. Full detail: `design/bap-transport.md` and
`d1-server/crates/d1-bap`.

Practical consequence: **do not start a UDP/DTLS listener expecting BAP.** We did, on port 3075,
and it captured nothing while the client was talking TCP to the same address.

### 1.3 The SCTP-like `bd*Chunk` stack serves NO layer - settled `[C]`

The eboot contains a literal SCTP four-way association handshake
(`bdInitChunk` / `bdInitAckChunk` / `bdCookieEchoChunk` / `bdCookieAckChunk`, plus
DATA/SACK/HEARTBEAT chunks, reliability windows, and a DTLS-flavoured variant). The standing `[U]`
about which layer it serves is settled: **none. It is compiled in and unreachable from the game.**
Measured by enumerating every function per Demonware source file and xref'ing each entry point,
with three passing positive controls:

| family | funcs | call sites | reached from |
|---|---:|---:|---|
| `bdSocketRouter.cpp` (control) | 21 | 14 | external: `0x2EA9D0` (pass) |
| `bdSecurityKeyMap.cpp` (control) | 5 | 8 | external: `0x2ED840`, `0x2ED8D0`, `0x2ED9C0` (pass) |
| `bdQoSProbe.cpp` (control) | 24 | 29 | external: `0x2EC8F0`, `0x2ECA00`, `0x2ECC90` (pass) |
| **`bdDTLSAssociation.cpp`** | 23 | 42 | **`bdSocketRouter` only** (`0xF88930`, `0xF89CE0`, `0xF8A480`, `0xF8A9E0`, `0xF8C470`) |
| `bdUnicastConnection.cpp` | 28 | 33 | `bdConnectionStore` only |
| `bdConnectionStore.cpp` | 6 | 12 | `bdNet.cpp` only |
| `bdReliableSend/ReceiveWindow.cpp` | 3+3 | 6 | within `bdConnection` only |

**The peer secure channel is `bdSocketRouter` -> `bdDTLSAssociation`** (`bdDTLSInit`,
`bdDTLSInitAck`, `bdDTLSCookieEcho`, `bdDTLSCookieAck`, `bdDTLSData`, `bdDTLSError`; RTTI
`0x19F8390`-`0x19F85E0`). `bdNetImpl` (`0x5120` bytes, allocated at `0x2ED8D0`) constructs both
halves; only the router half is used. **Reliability for game traffic is Bungie's own**, not
Demonware's: `networking:connection` (`0xAFD070`, `0xAFE060`, `0xAFE870`, `0xAFE9A0`) implements
sequence numbers and send/receive windows over `channel`/`channel_manager`.

> **Instrument failure recorded with this finding:** an earlier scan xref'd RTTI vtable addresses
> and got 0 references - and so did the positive control. Vtable pointers are loaded at
> `vtable+0x10`, not at the RTTI-reported address. A "0 references" result without a passing
> positive control is worthless.

> **Unresolved contradiction, recorded not reconciled:** the adopted architecture doc says the game
> never calls the `bdNetImpl` connection-store half, but recon found `0x2E7F70` -> `0x2ED4F0`
> **constructs and queries** a `bdNetImpl`-vptr singleton at `[0x29353D8]` on the secure-address
> path. One `d1-eboot rtti` walk of that vtable settles whether it is `bdNetImpl` or a subclass
> sharing the slot. `[U]`

### 1.4 Demonware estate, codename `panther`, and the DNS reality

Three claims, kept separate:

| claim | status |
|---|---|
| `panther` is Destiny's Demonware codename; `panther-ps4-auth3.prod` / `panther-ps4-lobby.prod` existed in Demonware's DNS estate | `[O]`, historical (reverse-DNS collections) |
| those hostnames appear as literals in the 01.29 eboot | `[C]` **FALSE** - no `panther`, `auth3`, `-lobby`, `lsg`, `loginqueue`, `umbrella` or `uno` literals anywhere; runtime URL construction (`https://www%d.%s/%s`) fully explains the absence |
| our client contacts them, STUN -> auth3 -> lobby | `[U]`, and no longer testable against live infrastructure |

The full name surface is **22**, not 18: 16 `stunN.signon.{deadorbit,gravityshavings}.net`, 2 bare
`signon.*`, and 4 `stun.{us,eu,jp,au}.demonware.net` (the only `demonware.net` literals in the
binary). Our runs resolved only the 16, so the four regional Demonware STUN names are a cheaper
progress signal than `panther`.

DNS check 2026-08-06 (read-only, operator-authorised):

| name | result |
|---|---|
| `panther-ps4-auth3 / -lobby / -cs .prod.demonware.net` | NXDOMAIN - decommissioned |
| `stun.{us,eu}.demonware.net` | 185.34.107.128 - Demonware STUN **live** |
| all 16 `stunN.signon.{deadorbit,gravityshavings}.net` | 16/16 resolve; odd -> 172.97.59.36, even -> 205.209.23.38 |
| `cdn.deadorbit.net` | live via Akamai (`a1165.g.akamai.net` -> 23.219.78.42/46) |
| `signon.{deadorbit,gravityshavings}.net`, `upload.deadorbit.net` | NXDOMAIN |

Sixteen names, two hosts: `deadorbit` and `gravityshavings` alias the same odd/even pair - an
answering surface only needs two endpoints, and the split says the client expects a redundant pair.
Bungie's D1 STUN and CDN were still live in August 2026 while Demonware's `panther` auth/lobby
names were already gone: the preservation-urgency argument as measurement.

### 1.5 The Tribes lineage - Layer C's architecture is published `[C]`

Lineage: "The TRIBES Engine Networking Model" (host-client, cheat-resistant, semi-reliable
delivery). Three replication protocols: **state data** (eventually-current, intermediates dropped),
**events** (transient justifications, no reliability guarantee), **control data** (subsampled
inputs, client -> host, ~20 bits per player, hand-packed). Host holds nearly all authority;
client-reported position is a claim, not a fact. Packets are priority-filled under a bandwidth
budget - **partial reads are the normal case, not an error** `[I]`; do not model gameplay packets
as a fixed schema.

Where Destiny diverges from Reach `[I]`: topology. Reach was pure P2P with host migration; Destiny
added P2P bubbles plus dedicated physics and activity hosts. Aldridge -> what a peer link carries
(transfers by lineage); Truman -> who hosts what (Destiny-specific).

### 1.6 Prior art and licensing

* **AGPL-3.0 and unlicensed prior art are off the table** - not used, not ported from, **not read**
  (operator decision 2026-08-06). "No LICENSE file" is not permission; unlicensed is stricter than
  AGPL, not looser. ⇒ `design/licensing.md`.
* ⭐ Independently of licence, the Demonware-era prior art that exists implements **TCP +
  3DES-EDE3-CBC + HMAC-SHA1**, which is **not our client's transport** (section 1.4: an SCTP-shaped
  association over UDP, with DTLS). The code would not have fitted, and anchoring on it would have
  pulled the model toward the wrong transport.
* Usable instead: GPL-compatible or permissive sources evaluated per need, public prose, and above
  all **our own eboot** - 103 `bd*` symbols, 72 build paths, 1,291 diagnostic strings of
  first-party evidence.
* RPCS3's RPCN client (GPL-2.0, licence-clean to read) is protocol-incompatible with shadNET above
  command 11 (rooms relocated to 100-115; only 18 of 91/71 protobuf messages share a name; only
  `RecordScoreRequest` is field-for-field identical). There is nothing to port. `[C]`
* Legal note `[C]`, non-blocking: the Bnetd precedent argues for keeping the framing exactly what
  it is - preservation of an owned copy against services we run ourselves. See
  `design/licensing.md`.

---

## 2. The server surface - twelve consultation points

> **The one-line answer:** the Activity Host does not run the peer mesh. It **scores** it,
> **admits** to it, **evicts** from it, and **introduces** peers to each other when NAT blocks
> them. Everything else the consoles do by themselves.

Join, secure-channel setup, membership, host election, handoff, migration, boot/mayday and
replication contain **no synchronous service call anywhere** - but the election's dominant score
term is a service fact peers gossip to each other, so the mesh is self-driving **on a
service-seeded input**. Reach: `static-client-inference`, licensed by `d1-eboot controls` 22/22
PASS on the pinned eboot.

### 2.1 Host election is wholly peer-local `[C]`

The scorer is `0xB7A350` - `(session, refusing_mask, decision) -> best_index`. For each peer `i` in
`[0, decision->peer_count)` (count at `decision+0x24`), skipped if bit `i` is in `refusing_mask`:

```
score(i) = popcount20(decision->connectivity[i])            // decision + 0x2FC + i*4
         + 10000  if bit i in decision->in_AH_membership    // decision + 0x34C
         + 1000   if bit i in decision->not_leaving_region  // decision + 0x350
```

Max wins; strict `<` keeps the lowest index on ties. Peer indices are computed identically
everywhere with zero communication: `0xB92FC0` insertion-sorts the peer list by `memcmp` over each
peer's **0x24-byte secure address** (`membership + 0x30 + i*0x168`); that total order *is* the
index space. The decision is a **0x4B0-byte blob** broadcast as session message type **10**.

Score inputs: `connectivity[i]` is peer i's own claim of reach; `in_AH_membership` is a
**self-report of a service relationship** (each peer sets only its own bit, `0x75DCC0` ->
activity-client vfunc `+0x18`, merged in `0xB872E0`); `not_leaving_region` likewise (`0x75DD30` ->
`0x779FA0`).

Conclusion at `0xB961E0`: become host when
`(unchanged_mask | refusing_mask) == (1<<peer_count)-1` and `0xB7A350(...) == our_index`; then
`0xB92BD0` sends host-reestablish, disbanding peers it could not map (`~mask & 0xFFFFF`).
`election-refuse` (`0xB88070`) withholds consent via `refusing_mask` until a soft timer
(`[0x34DB20]+0xF8`, a tuning block) expires; `peer %s timed out, marking as refused` folds silence
into the same mask. Failsafe: `local_8d8 >= DAT_027A49B8` -> `host election hit failsafe timer,
aborting` -> `0xB6F390`. The failsafe constant is config-resident `[U]`.

**The election makes ZERO service calls.** `[C]`

### 2.2 The session ceiling is 20 peers, not 16 `[C]`

Five independent confirmations: `0xB94560` initialises the AH-registered mask to `0xFFFFF` and ANDs
every mask with `0xFFFFF`; `0xB92FC0` allocates `connectivity[20]` and `timestamps[20]`; `0xB79750`
has `connectivity` at `+0x2FC` and the next field at `+0x34C` (a gap of exactly `20*4`), with the
peer-address array `+0x28 .. +0x2F8` = exactly `20 * 0x24`; `0xB7EB90` loops `while (u != 0x14)`;
`0xB961E0` uses a 20-bit rotate guard (`uVar16 = uVar16*2 & 0xFFFFE`).

**20 is the session ceiling, not the fireteam or match size.** "3v3" remains `[U]` - see §5.6.

### 2.3 The twelve consultation points - build these; build nothing in §2.5

| # | consultation | where | asks for | blocks / fails |
|---|---|---|---|---|
| **C1** | **Activity Host over BAP** | `activity_client` `0x2996D0`-`0x29CA40`; transport `0x594E70` | startup request, `join request [AH %s][nonce %d] identity:[account %lld][character...]`, reservation request, peer-reservation release, leave request, client heartbeat, state refresh request, connectivity failure report, membership ack, client identity data, client-authoritative data | join blocks on join result from AH; nonce mismatch or denial -> `Join request was denied [reason...]`; silence -> never established |
| **C2** | **AH endpoint discovery** | BAP 16/17 `get_activity_host_proxy` `0x562B90`/`0x562D60`; `AH IP resolution successful: '%s -> %s'` `0x594E70` | where is the AH for this instance | without it C1 has no address and the AH election term is permanently 0 for everyone |
| **C3** | **AH-membership -> the election** | `0x75DCC0` -> activity-client vfunc `+0x18`; consumed by `0xB7A350` (+10000), gated in `0xB94560` | am I registered with the AH | local read, non-blocking - but with an AH present and nobody registered, `0xB94560` prints `Not stable enough to select a new host because we have an AH and no eligible peers ('0x%x') are registered ('0x%x')` and handoff never completes |
| **C4** | **NAT relay brokerage** | BAP `register_relay_client_request` `0x562950`, `initiate_relay_connection_request` `0x5622B0`, `request_relay_connection_notification` `0x562490`; client `0x56EF70` (`RelaySessionId`, `RelayServer`) | a relay server + session id for an unreachable peer | `channel_manager` `0xAFA7E0`/`0xAFB7F0`: `nat relay use was requested, but the relay is not available` -> channel stays unsecured, join times out |
| **C5** | **NAT punch introduction** | `0x59BD80` send, `0x59BF20` receive -> `0x59C420` forward | tell peer X to punch back at us | strict-NAT pairs never open directly; falls through to C4 |
| **C6** | **STUN - external address + NAT type** | `networking:stun` `0x2DF4A0`-`0x2E2590` | external IP, NAT open/moderate/strict | server list comes from signon, which also supplies a fallback external address. Failures degrade (`defaulted to moderate/strict`) - non-blocking, but a wrong external address makes every peer address unroutable |
| **C7** | **PSN managed session - the PSN Session WebAPI over HTTP, NOT `sceNpMatching2`** | `managed_session` `0x2B5470`-`0xB69C60`; keys at `0x2B5B30`; `POST /v1/sessions` `0x2BC8C0`, `POST /v1/sessions/{id}/members` `0x2BC830`, `GET /v1/sessions/{id}/sessionData` `0x2B3C40` | create/join PSN session, store/fetch the 78-byte session descriptor, add/remove players, game start/end, host-migration session | a store-and-forward HTTP surface in `d1-responder` (§4); the client mints its own keys. Failure sites: `host-reestablish managed session creation FAILED! disconnecting!` (`0xB95910`); `failed to create managed session for client` (`0xB6BBE0`) |
| **C8** | **Bungie matchmaking over BAP** | `online:matchmaking:bungie` `0x595E80`-`0x59B390` | create/update/delete session advertisement, MM configuration request (activity hash), session search | `Lost primary BAP connection, resetting all online request handlers`. Required to *form* a session, not to run one |
| **C9** | QoS probes | `logic:session_tracker` `0x13380`/`0xB14610`; Demonware `bdQoSProbe` `0x2EC8F0`/`0x2ECA00`/`0x2ECC90` | ping/bandwidth of a candidate host | **peer-to-peer, not a service.** Timeout -> `marking session unsuitable` |
| **C10** | Session admission constants | `0xB74170`, `0xB83DB0` | silo id, content revision, protocol version, executable type | compared peer-to-peer, but every value is server-issued at signon/manifest time. Mismatch -> `Join from peer '%s' failed; the silo ID '%llu' doesn't match` |
| **C11** | **AH boot authority** | `0xB70150`, `0xB71DC0`, `0x73500`, `0xB739C0` | `activity_host_boot_reason`, boot nonce | the AH can evict a machine and the session obeys (`Applying pending boot ... by disconnecting session`) |
| **C12** | Region stability | `0x75DD30` -> `0x779FA0` -> `0xB7A350` (+1000) | am I leaving the matchmaking region | local read, non-blocking |

Everything else in `networking:session` - 268 strings across ~90 functions (268 of 275 strings
peer-local) - is peer-local.

### 2.4 Secure addresses

A **secure address is a 0x24-byte locally-minted handle, not a routable address.** `0x2EFAB0`
keeps a 32-slot table at `[0x266E080]`; registering a peer mints
`id = (random>>8 & 0xFFFFE0) + slot` bound to a `bdCommonAddr`. Resolution is `0x2EE780`. The
secure connection shim at `0x2E4A30`-`0x2EFE50` calls `bdSocketRouter::sendTo` (`0x2EF370`),
`::getStatus` (`0x2EEC70`) and `bdSecurityKeyMap` add/remove (`0x2ED840` -> `0xF875A0`,
`0x2ED8D0` -> `0xF876C0`, `0x2ED9C0` -> `0xF87820`).

Where the key material inside comes from is §3 - and note the source doc's §4 conclusion
("PSN-issued") is **retracted**; see Dead claims.

### 2.5 Peers handle this alone - DO NOT BUILD IT

1. Host election, end to end - `0xB92FC0` (decision), `0xB872E0` (merge), `0xB961E0` (conclude),
   `0xB7A350` (score), `0xB88070` (refuse). Zero service calls.
2. Deterministic peer ordering - `memcmp` sort of secure addresses, `0xB92FC0`.
3. Host handoff and migration - `0xB94560`, `0xB92810`, `0xB85010`, `0xB85530`, `0xB859D0`,
   `0xB79030`, `0xB71710`, `0xB70570` (speculative).
4. Host re-establish after loss - `0xB85C20`, `0xB86500`, `0xB86CA0`, `0xB95910`, `0xB92BD0`.
5. Join negotiation and admission - `0xB74170`, `0xB83DB0`, `0xB75B00`, `0xB6F750`, `0xB93960`,
   `0xB6E0D0`. Silo/revision/version checks are peer-to-peer.
6. Membership state and its checksum - `0xB81BA0`, `0xB7FC90`, `0xB883D0`, `0xB89590`, `0xB7EB90`.
7. Channel establishment, refusal, reconnect, and the decision to want a relay -
   `0xAEA3F0`-`0xAFBB70`.
8. Reliable transport windowing - `0xAFD070`, `0xAFE060`, `0xAFE870`, `0xAFE9A0`.
9. Connectivity-failure detection and peer booting - `0xB77520`, `0xB77E40`, `0xB88930`,
   `0xB731C0`, `0xB73710` (reporting to the AH is C1; deciding is local).
10. Mayday - `0xB777F0` / `0xB84BC0`, in-band + OOB, peer-to-peer only.
11. Team picking and composition - `0x417380`, `0x4189B0`, `0xB095D0`, `0xB0B460`.
12. All replication - the three views of §5.4.
13. QoS probing of candidate hosts - `bdQoSProbe`, peer-to-peer.

### 2.6 The mesh blocks here until a service answers - BUILD THESE

Ranked with the 2026-08-08 correction applied (see Dead claims: C7 is no longer the hardest; **C1
is now the hardest**):

1. **Activity Host over BAP (C1)** - startup, join+nonce, reservations, heartbeat, state refresh,
   membership acks. The hardest item; owned by the Activity Host lane
   (`design/activity-host.md`).
2. **AH endpoint discovery (C2)** - `get_activity_host_proxy` (BAP ids 16 -> 17).
3. **AH reservations (C1)** - `citizen join ... refused, no reservation found for amb-nonce=%s`
   (`0xB74170`): a join fails at the peer, for lack of a service fact.
4. **AH registration as the election's dominant term (C3)** - +10000.
5. **PSN managed session (C7)** - still gating (no managed session -> no secure connection -> no
   join, no election, no re-establish), but the deliverable is ~6 plain-HTTP endpoints in
   `d1-responder` that store and return an opaque 78-byte blob (§4). Roughly the easiest of the
   nine.
6. **NAT relay brokerage (C4)** and **NAT punch introduction (C5)** - the only escape from
   strict-NAT pairs.
7. **STUN, seeded by signon (C6)**.
8. **Bungie matchmaking advertise/search/config (C8)** - required to form a session.
9. **AH boot authority (C11)**.

---

## 3. Identity and key material

### 3.1 The C7 verdict: the client mints its own key material `[C]`

> **Nobody issues it.** Not our server, not shadPS4, not a Bungie service, not PSN. The game client
> mints its own peer key material, locally, with Demonware's MT19937 PRNG. Re-derived from the
> pinned eboot three ways (disassembly, string reads, RNG-constant check). This retracts the
> archived peer-mesh doc's §4 conclusion.

The chain, disassembled:

```
0x2B5B30  managed-session create/join            "creating psn session as host, %s"
   └─ (host only) 0x2E8080
         ├─ copies 0x24 bytes  [0x266DEAD] (host-migration) or [0x266DD13] (active)
         │                     into  record[0x08 .. 0x2C]
         └─ tail-calls 0x2ED840
               ├─ 0xF64E90   bdSecurityID  default-ctor  -> 8 bytes of 0x01 (unset sentinel)
               ├─ 0xF653C0   bdSecurityKey default-ctor  -> 16 bytes of 0x01
               ├─ 0xF875A0 = bdSecurityKeyMap::initKey
               │     ├─ 0x1265BD0  Demonware random singleton
               │     ├─ 0x103E390(rng, &securityID, edx=8)      8 RANDOM BYTES
               │     ├─ 0x103E390(rng, &securityKey, edx=0x10)  16 RANDOM BYTES
               │     ├─ log "Created bdSecurityID: %s"
               │     └─ log "Created bdSecurityKey: %s"
               └─ on success: record[0x00] = securityID ; record[0x2C]/[0x34] = securityKey
```

`initKey` takes no input - no socket, no callback, no response - and always returns true. The RNG
core carries MT19937's `0x9908B0DF` at vaddr `0x1044F6C`, seeded from `sceKernelGettimeofday` via
`bdPlatformTrulyRandom-ps4.cpp`. Source-path proof read out of the image at `0x16AF22A`:
`c:\bungie\external_libraries\DemonWare\45\bdSocket\bdSecurityKeyMap.cpp`, function `initKey`.
`initKey` has exactly **one call site** in all 35,834 functions (`0x2ED87F`), reached only from
`0x2B5B30` and only when `is_host != 0`: a fresh pair per session creation, and again per host
migration.

### 3.2 The 78-byte session record

The record `0x2E8080` assembles is **78 (`0x4E`) bytes**, not 36 - the 36 (`0x24`) bytes are the
secure address *inside* it:

```
offset  size   field                     rendered as
+0x00   0x08   bdSecurityID              %02X%02X%02X%02X:%02X%02X%02X%02X   (0x2E95E0)
+0x08   0x24   secure address            (the 0x24-byte bdCommonAddr the mesh memcmp-sorts by)
+0x2C   0x10   bdSecurityKey             (0x2E9480 renders the ID/session pair for logs)
+0x3C   0x12   PSN online-session id     {%03x-%08x-%04x-%04x-%02x%02x-%02x%02x%02x%02x%02x%02x}
=====   0x4E   78 bytes
```

The two `%s` in `failed to create/register session keys, %s/%s` are the bdSecurityID and the
18-byte online-session id. The message is a disjunction: `create` is `0x2E8080` -> `initKey` (host
only); `register` is `0x2E8170` -> `0x2ED8D0` -> `bdSecurityKeyMap::registerKey` (`0xF876C0`,
`"Putting bdSecurityID: %s"`), which installs a **remote** peer's pair.

**How the joiner gets the host's key: store-and-forward of an opaque blob over HTTP.** Host
(`0x2B5B30` -> `0x2BC8C0`): `POST /v1/sessions`, `multipart/mixed`, boundary literal
`BUNGIEr0x0rz`, three parts (`session-request` JSON, `session-image` JPEG, `session-data` =
the 78-byte record verbatim as `application/octet-stream`). Joiner (`0x2B3C40` -> `0x2BC710`):
`GET /v1/sessions/{id}/sessionData` into a buffer sized exactly `0x4E`; latched by
`memcpy(obj+0x280, obj+0x1C0, 0x8C)`; extracted at `0x2B4060` (`memcpy(dst, obj+0x2B8, 0x4E)`);
handed onward at `0x2B3230` into the join request; `0xB621A0` copies it into the managed-session
params; `0x2B5A30` passes it to `0x2B5B30`'s client branch, which formats the host's session id
from `record+0x3C` and calls `0x2BC830` = `POST /v1/sessions/{id}/members`. `registerKey` then
installs the host's `{securityID, key}` and `bdSocketRouter`/`bdDTLSAssociation` opens the secure
channel.

The same 78-byte record has a **second carrier**: `0x2E8B70` writes `{u32 0x4E}{record}` into a
peer-to-peer message consumed by `0x2E89E0`/`0x2E8940`/`0x2E8A60`/`0x2E9A10`, driven from
`world_controller:slice_set_transition_manager` (citizen/region join). Whether it is an alternative
to the WebAPI or strictly downstream of it is `[U]` (§8).

### 3.3 Identity, across every layer `[C]` unless tagged

Identity conflation is what the harness contract forbids, and it is silent when it happens:

| generic concept | D1's actual concept | where | evidence |
|---|---|---|---|
| npid (string) | `SceNpId`, **0x30 bytes binary** | decoded `0x2B79F0`, printed `0x2B7B90` | `[C]` |
| account id (u64) | **two distinct u64s**: platform account id AND Bungie account SOID, bridged by BAP 23/24 | encode `0x55A090`, decode `0x55A810` | `[C]` |
| - | character SOID (u64) | `identity:[account %lld][character %lld]` | `[C]` |
| member id (u32) | **two different index spaces**: peer index 0..19 and player index (membership stride `0xE8`) | `mem #%3d: us/host/ldr #%2d/#%2d/#%2d peer/plyr %2d/%2d` | `[C]` |
| addr/port | the 0x24-byte secure address - locally minted, **not routable**; 32-slot table at `[0x266E080]` | mint `0x2EFAB0`, resolve `0x2EE780` | `[C]` |
| - | machine id, a string rendered from the bdSecurityID + addresses (`7DF825C9:E1D6EFA5-10.77.0.2:127.0.0.1`) | join-request string | `[I]` |
| team id | D1-only, at `membership + player*0xE8 + 0x1D08` | `0xB97FC0` | `[C]` |
| is_owner | **host** and **leader** are distinct roles, both elected/delegated peer-side | `us/host/ldr`; `delegate_leadership` | `[C]` |
| room id | no analogue. D1 has **three network sessions** (§3.4) and a 10-slot managed-session table (stride `0x918`, base `[0x02889F30]`) | `0xB61F60`, `0xB63D00` | `[C]` |

The identity bridge we already answer: BAP 23/24
`client_to_bap_account_id_translation_request/response`, wire shape
`[u16 count][u8 kind_a][u8 kind_b]` then per kind an array of u64 account SOIDs (kind `0xFF`) or of
0x30-byte `SceNpId` (kind 7). "7 = PS4" is `[I]` - no positive control (§8).

**The conflation that would silently ruin a two-instance run:** peer ordering is a `memcmp` over
the 0x24-byte secure address, and the secure address and bdSecurityID come out of the emulator's
own PRNG seeded from wall-clock time. Two shadPS4 instances producing the same entropy would fuse
into one identity, and the failure would look like a networking bug. Mitigations (both required):
preflight asserts distinctness at all four layers (NP online id, D1 account SOID, peer index,
endpoint) and refuses otherwise; and fixture hygiene -
`capture-rig/six-player-fixtures.py` is correct (seeded HMAC-SHA256, disjoint `0xE1`/`0xE2` SOID
prefixes, asserts no SOID equals `0xD100000000000001`), but
`capture-rig/single-participant-fixture.py` is **defective**: it mints under the banned `0xD1...`
prefix, has no banned-SOID assertion, uses plain SHA-256 rather than HMAC, and its documented
invocation does not match the file name. Fix before any two-instance work.

### 3.4 Three network sessions, and the fireteam-join sequence

D1 has **three network sessions, not two** - index `0 = fireteam`, `1 = posse` (the matchmade
activity session several fireteams share), `2 = group` (the public/region shared world, `[I]`) -
each double-buffered CURRENT/TARGET (`0x772900` does `[slot+8] ^= 1`). Channel-owner names in the
image: `fireteam0, fireteam1, posse0, posse1, group0, group1, simulation`. Manager at
`[0x26B1850]`, slot stride `0x131C8`, instance stride `0x98E0`. All three multiplex onto the one
10-slot managed-session table, and thence onto PSN sessions. Caution: "posse" also means, inside
`team_picker`, a bucket of players sharing a fireteam id - two different nouns.

**B joins A's fireteam** (condensed; consultation points marked):

| # | step | site(s) | consult | who answers |
|---|---|---|---|---|
| 1 | A invites: get session 0; state in 4..10 (`session+0x8C48`); read `session+0x9808` = managed-session index; fetch the 78-byte descriptor via `0xB67010` | `0xB25810`, `0xB67010`, `0x2B31F0` | - | client |
| 2 | Platform carries it: `POST /v1/sessions/{id}/invitations`, multipart `invitation-request` JSON + `invitation-data` binary = the descriptor. **A's bdSecurityID AND bdSecurityKey ride inside** | `0x2B3DB0`, `0x2C2F70` | C7 | us (`d1-responder`, §4) |
| 3 | B accepts: `0x2B3230(inviter SceNpId[0x30], invitee player id[8], invitee SceNpId[0x30], descriptor[0x4E], &invitation_type)` | `0xB25510`, `0x951940` | C7 | us (blob passthrough) |
| 4 | B queues the join (`queued new local to fireteam join to [%s]` or the fireteam-cascade variant; `atomic_cascade_join_data`, `0x5C` bytes at param offset `0x01F0`) | `0xB1F610`, `0x771AA0` | - | client, peer-local |
| 5 | B needs a local secure address, else `unable to get a local secure address, networking unavailable` | `0x2E8110`, `0xB6BBE0` | C6 | us (signon f14) + emulator |
| 6 | B creates its client managed session: `0x2B5B30` client branch formats A's session id from `descriptor+0x3C`, then `POST /v1/sessions/{id}/members`. Failure -> `failed to create managed session for client` | `0xB6BBE0`, `0xB64090`, `0x2B5B30`, `0x2BC830` | C7 | us (`d1-responder`) |
| 7 | B installs A's key, opens the channel: `registerKey` (`0xF876C0`) then `bdSocketRouter` -> `bdDTLSAssociation` | `0x2ED8D0`, `0x2EF370` | - | peer-local |
| 8 | NAT, if direct fails: STUN, punch-intro, relay fallback | `0x59BD80`/`0x59BF20`/`0x59C420`; `0x56EF70` | C6, C5, C4 | signon + BAP relay (not implemented) |
| 9 | A admits: `0xB73E00` reads `active_join_controls` at `session+0x6C28` (`[0]` open direct slots, `[1]` open alternate, `[2]` allow-direct, `[3]` allow-alternate, `[4]` closed-flags mask low 5 bits, `[5]` allow-matchmaking) | `0xB73E00`, `0xB6D8E0` | - | peer-local; the host decides |
| 10 | Admission constants compared peer-to-peer (silo, revision, version, executable type) | `0xB74170`, `0xB83DB0` | C10 | values ours, comparison theirs |
| 11 | Membership republished, checksummed | `0xB80EF0`, `0xB81BA0`, `0xB7FC90` | - | peer-local |
| 12 | Teams, once the posse session exists | `0xB273F0` -> `0xB97FC0` -> `0xB980C0` | - | client decides, alone |

Session parameters that move during a join (offsets from `0x773FC0(0)`):
`remote_session_join_data` @`0x0FD8` and `_2` @`0x11B0` (0x78 each - two concurrent joins; the
78-byte descriptor fits), `remote_session_join_result` @`0x1388` (0x10), `active_join_controls`
@`0x0100`, `atomic_cascade_join_data` @`0x01F0`. **The client is the authority on session
parameters; the server can never set one** - they replicate peer-to-peer over the game's session
channel (`0x25`/`0x26`), never over BAP (see `design/architecture.md`).

**Second invite route, cheaper to test first:** BAP 38/39
`client_to_bap_party_invite_request/response` and 40/41 `check_party_invite` exist alongside the
platform route; a two-instance harness would need no PSN invitation plumbing if they carry the
descriptor. Whether they do is `[U]` (§8).

**Teams:** `0xB97FC0` -> `0xB980C0` picks one of four modes from session properties:
`props+0xBC <= 1` no teams; `== 2` free-for-all (unique index per player, 0..19);
`props+0x3B != 0` with a user selection -> user-selected teams win (reads
`membership + player*0xE8 + 0x1D0B`, gated on the state byte at `+0x1D09` not in `{2, -1}`);
otherwise `team_picker` `0x417380`, balancing skill and keeping fireteam-mates together. Team
configuration is data: `props+0xC4` = team count (max 8), `props+0xC8 + t*0xC` min-per-team,
`props+0xCC + t*0xC` max-per-team; refuses unless `min*teams <= players <= max*teams`. Result is a
byte at `membership + player*0xE8 + 0x1D08`. The bounds come from the Oodle-compressed activity
definition - "3v3" is `[U]` (§5.6).

---

## 4. The sessions API plan (C7)

### 4.1 What C7 actually is

**C7 is a small HTTP service in our tree that stores and returns an opaque 78-byte blob.** No
crypto, no key derivation, no protobuf, no PSN. shadPS4 hands us the surface:
`sceNpWebApiSendRequest` (`np_web_api_internal.cpp:780`) builds
`full_url = GetShadNetWebApiServer() + request->userPath` over **plain libhttp**, and the setting
is a configurable string (`emulator_settings.h:214`, default `http://srv.shadps4.net:31315`).
Point it at `d1-responder`. C7 is therefore **roughly the easiest of the nine consultation points**,
not the hardest (the archived ranking is retracted; C1 is now the hardest).

Honest caveat: all of this is `static-client-inference`. The client's parsers for the
`POST /v1/sessions` **response** were not disassembled - we know what it sends and what buffer size
it expects back, not which JSON keys it requires (`[U]`, §8).

### 4.2 Endpoints, from the client's own compiled literals

(`/%s/` is a version, near-certainly `v1` - `[U]`, closable in one run.)

| verb + path | client site | server obligation |
|---|---|---|
| `POST /v1/sessions` | `0x2BC8C0` | parse `multipart/mixed` boundary `BUNGIEr0x0rz`; store the `session-data` part verbatim (78 bytes); return a session id the client can render/parse |
| `GET /v1/sessions/{id}/sessionData` | `0x2B3C40` -> `0x2BC710` | return exactly the 78 bytes stored; the receive buffer is sized `0x4E` |
| `PUT /v1/sessions/{id}/changeableSessionData` | literal present | store/return; not yet on a traced path |
| `POST /v1/sessions/{id}/members` | `0x2BC830` | the client-side join; add the member, return success |
| `DELETE /v1/sessions/{id}/members/{npid}` | `0x2BC000` | remove |
| `GET`/`PUT` `/v1/sessions/{id}` (+`?fields=@default`) | `0x2BC080`, `0x2BE1A0` | get/update session |
| `GET /v1/users/{npid}/sessions` | `0x2BC7A0` | list |
| `POST /v1/sessions/{id}/invitations` | `0x2BA5B0`, `0x2C2F70` | multipart with `invitation-request` JSON and an `invitation-data` binary part; store the blob |
| `GET /v1/users/{npid}/invitations/{id}/invitationData` | `0x2B3DB0` family | return the blob |
| `PUT /v1/sessions/{id}/sessionImage` | literal present | accept and discard a JPEG |

Request-body constants the client sends, measured `[C]`: `sessionPrivacy` in
`{0:"INVALID", 1:"public", 2:"private"}` - D1 writes 1 unconditionally; `sessionType` in
`{0:"INVALID", 1:"owner-bind", 2:"owner-migration"}` - D1 writes 1; `sessionMaxUser` - D1 writes
**1** (surprising for a six-person fireteam; re-check against a logged body before building on it).
Managed-session creation passes `private_slots = 0, public_slots = 0` unconditionally.
**`sessionPrivacy` is not how a D1 private match is made.**

**The session-id shape is a real constraint.** The client renders the 18 bytes at `record+0x3C` as
`{%03x-%08x-%04x-%04x-%02x%02x-%02x%02x%02x%02x%02x%02x}` (40 chars) and uses that string as the
`{id}` in the member/join URL. `d1-responder` has observed the client PUT back
`000-00000000-0000-0000-0000-000000000000` after failing to parse a non-PSN-shaped id. Best current
reading `[I]`: the client derives the session id from its own 18 bytes rather than adopting one we
mint - accept a client-chosen id rather than assigning one. Confirm before relying on it (§8).

Controls (the axis is agreement with the client): a stored-then-returned `session-data` blob must
be byte-identical and exactly 78 bytes (positive control: a 77- or 79-byte body is rejected); the
multipart parser must round-trip a fixture built from the client's exact boundary and
`Content-Description` keys; the session-id renderer/parser must round-trip the `%03x-...` format
for a known 18-byte input.

**Reach discipline:** everything crossed on our own stored blob is `synthetic-minimal` and every
finding downstream is `synthetic_ancestor`. The blob is opaque to us - we are a post office, not a
party - but that does not make the taint go away.

### 4.3 D1 never calls `sceNpMatching2` - the closure evidence, with its caveats

`[I]`-strong (deliberately not `[C]`):

* Library `U` (the Matching2 import library) = **28 imports**, PLT `0x129EFA0`-`0x129F150`.
* Full callgraph closure over those imports: **26 reachers, all in `0xFD45D0`-`0xFDFBE0`
  (the Toolkit Matching cluster), 0 game-side.**
* **Positive control:** the same walk over all 38 import libraries finds **32 of 38** have
  game-side reachers; library `U` is one of six that do not. The method discriminates.
* Region-entry test: **0 game-side direct entries** into the Toolkit Matching
  (`0xFD4000`-`0xFE0000`), WebAPI (`0xFF0000`-`0xFF3000`) or Trophy (`0xFE6000`-`0xFE8000`)
  clusters. Game code calls imports directly (e.g. `0x2C00F0` calls `sceNpWebApiInitialize`),
  bypassing the Toolkit entirely. Per-import controls: `sceNpTrophyUnlockTrophy` -> 10 game-side
  reachers, `sceNpWebApi` -> 9, `sceNpAuth` -> 1, `sceNpMatching2` -> 0.
* Endpoint literals confirm the actual carrier: `/%s/sessions`, `/%s/sessions/%s/members`,
  `/%s/sessions/%s/invitations`, `BUNGIEr0x0rz`, `session-data`, `session-request`,
  `invitation-data`, `sessionPrivacy`, `owner-bind`, `creating psn session as host, %s`,
  `failed to create/register session keys, %s/%s` - all present.

**Why not `[C]` - three caveats, all recorded in the source doc's own terms:**

1. The Toolkit is virtual-dispatch throughout: the Matching cluster's only external entry is
   `0xFD1B40`, which itself has zero callers of any kind, so direct-call analysis cannot see a
   construct-and-drive by design. (Two recon agents disagreed on exactly this; the region-entry
   test above is what adjudicated between their controls.)
2. **Instrument defect D1:** `d1-eboot`'s NID oracle used the wrong base64 alphabet (`+/` instead
   of PS4's `A-Za-z0-9+-`). Of the 86 imports whose NID contains `-`, 0 resolved; of the 552
   without, 511 did - **13.5% of all imports silently unnameable**, including
   `sceNpMatching2JoinRoom` (`CSIMDsVjs-g`), `ContextStop` (`-f6M4caNe8k`), `DestroyContext`
   (`Nz-ZE7ur32I`), `GetUserInfoList` (`qeF-q5KDtAc`), `SetRoomDataExternal` (`q7GK98-nYSE`). Its
   four pinned NID controls all used `-`-free NIDs, so the control could not detect it.
3. **Instrument defect D2:** `d1-eboot xref`'s RIP-relative scanner drops operands from
   instructions with a trailing immediate (`cmp m8, imm8`: the `disp32` is not the last four
   bytes). `xref 0x266deac` reported 0 references with all four positive controls passing, while
   `disasm` shows three real sites (`0x2E8088`-adjacent, `0x2E8110`, `0x2E8124`). **Any "this
   global is never touched" conclusion drawn from `xref` since the crate shipped is suspect.**

The consequence is asymmetric, which is why building on the negative is safe: if Matching2 is dead,
the sessions API (§4.1) is right; if Matching2 turns out to be live, the sessions API is *still*
right (the WebAPI path is separately proven) and the room protocol becomes additional work. **There
is no branch on which the sessions API is wrong.** The one-command experiment that converts the tag:
one boot-to-orbit run, then grep the run log for `Lib_NpMatching2` (with `Lib_NpWebApi` present in
the same grep as the positive control proving the log surface is on).

### 4.4 The room protocol: built, and left unarmed

`crates/shadnet-server` implements all sixteen Matching2-shaped room commands (100-115) on operator
instruction: split stream + per-connection notification outbox, shared `ServerState`, all 34
`ErrorType` values, `proto/PROVENANCE.md`; 33 controls (`shadnet-server controls`, exit 2 on
failure), 71 tests in the crate, 358 across the workspace. **Building it is not evidence for it.**
Defaults unchanged: `matching2_enabled = false`, room commands answer `Unsupported(33)`, launchers
untouched. On current evidence D1 will never send one of these commands; the branch is settled by
the grep experiment above.

Two facts from that lane worth keeping even if the branch dies:

* **Provenance trap in `GetWorldInfoList(111)`:** uniquely, with `matching2_enabled = false` the
  emulator **fabricates** a reply (a single world `{world_id=1, lobbies_num=0}`, `ORBIS_OK`)
  instead of failing. If D1 is ever observed advancing past a world-list gate in that
  configuration, the advance was caused by shadPS4, not by our server - `synthetic_ancestor` twice
  over. Only `ContextStart`/`ContextStop` reach the socket regardless of the flag.
* **The shadPS4 P2P stubs (`p2p_sockets.cpp:140-215`, all send/recv -> -1) block only the
  Matching2 path, i.e. the path D1 does not use.** D1's own mesh is `bdSocketRouter` ->
  `bdDTLSAssociation` over ordinary `sceNet` sockets - the guest observably binds UDP `3074`,
  `2001`, `2002` through the posix socket layer, which is implemented. Two recon agents called the
  stubs plan-stopping; they were right about the code and wrong about the consequence.

### 4.5 What we are NOT building

| not building | why |
|---|---|
| Host migration | deferred by decision; the Reach public beta warped players to `0,0,0` on migration, and the emulator drops `ROOM_OWNER_CHANGED 0x1105` anyway |
| Six participants before two work | two is the smallest configuration that can falsify anything; `six-player-fixtures.py` stays on the shelf |
| Elimination rules | behind Oodle; `team_picker` reads its bounds from the compressed activity definition; "3v3" is not attested in the owned build |
| Room handlers (armed) | contingent on the `Lib_NpMatching2` grep; today no evidence anything asks |
| `matching2_enabled = true` | the trigger has not fired; turning it on also arms the 31314/UDP expectation and the UPnP path for nothing |
| `GetRoomMemberDataExternalList (114)` | D1 does not import the NID; it will never be sent |
| Extending `shadnet.proto` (join-side `room_password`, `SetRoomMemberDataInternal` message) | forks the verbatim-vendored guarantee ATTRIBUTION.md rests on; ask upstream first |
| shadPS4's `SHAD` handshake / 21-byte STUN ping, server side | a shadPS4-private protocol; every finding through it is synthetic; moot while the room branch is contingent |
| Everything in §2.5 | 268 of 275 `networking:session` strings are peer-local; building any of it is building a thing the client already does |

### 4.6 Acceptance criteria for the first two-instance session

> **FORBIDDEN AS EVIDENCE: "both instances reached the same screen" and "it did not crash."**
> Neither discriminates between the system working and the system silently doing nothing.

Each criterion names the observation, the instrument, and the negative control:

* **A1 - Two distinct participants, distinct at every layer** (NP online id, D1 account SOID,
  character SOID, peer index, endpoint), observed in both instances' logs and asserted by
  preflight. Negative control: configure both with the same npid; preflight must refuse (exit 2).
* **A2 - Session callbacks fire on both sides, with a server-attributable cause**: A's 78-byte blob
  arriving at B, byte-identical to what A POSTed, with the store logged on one side and
  `registerKey` (`Putting bdSecurityID: %s`) on the other. Negative control: serve a **77-byte**
  blob; the join must fail at a named site (`failed to create managed session for client`,
  `0xB6BBE0`), not merely stall.
* **A3 - Bidirectional per-plane packets** (session/control, replication, voice) non-zero in both
  directions with per-plane timestamps. Negative control: a single-instance run must show the peer
  plane at zero.
* **A4 - Exactly one elected Physics Host**, agreed in both logs; `0xB7A350`'s winner must equal
  the peer index each side computed independently, and each side's `memcmp` ordering over the
  0x24-byte secure addresses must agree. Negative control: the two secure addresses must be
  different - identical addresses produce a degenerate order that would look like agreement.
* **A5 - One action, authoritatively applied, observed by the other peer**: one input on A, applied
  by the elected host, observed by B as a state change B did not author. Mandatory negative
  control: the same input with the peer link severed must NOT appear on B.

After A1-A5, the fault matrix F03-F10 becomes runnable at two participants; F01 needs 3+ and F02 is
degenerate at two (see `historicaldocs/design/p2p-fault-and-acceptance-matrix.md`).

---

## 5. Authority model

### 5.1 Three authority planes

D1 private PvP is not one server and not pure peer-to-peer. Three planes:

1. **World/BAP services** create and persist identities, characters, loadouts, activity instances,
   and results.
2. A cloud **Activity Host** runs the activity/gametype script and maintains a minimal
   authoritative set of activity sensors and session state.
3. One participating console is elected **Physics Host** and runs the detailed Tiger/Havok
   simulation, with Bungie's replication protocol carrying state, events, and control data between
   peers. `[C-Bungie]`

The clients already contain the full simulation and Havok integration (build path
`d:\bungie\external_libraries\havok\2012-2-0_8\...` `[C-client]`) but will not spontaneously form a
valid match: we must supply the control-plane contracts that place peers in one activity, establish
secure membership, assign authority and entity identities, provide an Activity Host, start the
match, and persist its outcome. We should **not** build a server-side Havok replacement - the
missing work is the session/transport/replication surface, not the collision solver.

### 5.2 The Activity Host is a SERVICE - conflict resolved `[C]`

The old `[CONFLICT]` ("one client becomes the activity host, `ipahp`") is closed: **`AHP` =
activity host *proxy*.** The argv proof: the session host asks BAP to **start** an Activity Host by
sending a process command line - message `8 request_activity_host`, 0x100F bytes, containing
`/ah_type:group|fireteam /ah_package:... /ah_slice_set_names:...` (`0x5946B0` -> `0x5949B0`),
carried as `client_to_activity_host_manager_request` (BAP id 6). You do not send argv to another
player's PS4. BAP replies with an opaque 64-bit AH ID; each peer resolves ID -> IPv4 + port via
`get_activity_host_proxy` (BAP 16 -> 17, decoder `0x594E70`) and opens a separate BAP connection in
slot 1/2/3 (`OUT FAH` / `OUT GAH0` / `GAH1`). Full contract: `design/activity-host.md`.

Message ids (see `design/bap-transport.md` for the full table): 6 -> 7 manager request/response,
8 client notification, 16 -> 17 proxy resolution, 100/102 AH-to-client push, 150/151/153/160
Bungie-internal. **153 `bap_to_activity_host_socket_handoff_request` is the architectural
give-away**: BAP hands the AH a socket; the AH is a separate listener. Residual `[U]`: whether the
resolved address is the AH process itself or a BAP-side relay - either way it is infrastructure,
not a console.

Sizing, from bytes: the largest blob the AH ever authors is `global_activity_state` at
**0x6B0 = 1,712 bytes**, and there are no transforms anywhere in the 49-message surface. An AH is
simply not carrying a world - the strongest binary-level support for the Physics Host / Activity
Host boundary.

Two corrections recorded from the resolution (kept so they are not re-derived wrongly):
`0x783E90` does no transport work - it is a static-initialiser string table (1,057 instructions,
zero calls, zero jumps); and the "host" that IS a peer is the **fireteam/posse host**
(`c_world_controller_state_setup_activity_host_setup_handler::vfunc[7]`, `0x7A5250` - "As HOST,
waiting for peers to be done with setup"). Conflating that host with the Activity Host is what
created the conflict.

**The first measured AH requirement (rung 28):** the client left `setup:orbit` for
`setup:activity_session_creation` and blocked with
`Waiting for session peer properties to be in the expected state (no AH ids). FT [EXPECTED], POSSE [WAITING]`.
`[C]` The client demands an Activity Host id on BOTH the fireteam and the posse session -
`s_activity_host_connection_information`, parameter `+0x390` in each set, 0x20 bytes, AH id (u64)
at value `+0x08`. It also emitted a real matchmaking configuration request (nonce
`0x6da56ada96ee61b7`, activity hash `0xdf283afb`). Reach caveat: the transition was produced by
three writes into a live client - `synthetic_ancestor` x2; evidence about what the client *wants*,
never about what a real service sent.

### 5.3 Host authority, in the lineage's own words `[C-Bungie]`

From the GDC 2011 transcript (mined 2026-08-08): the host has almost all the authority -
authoritative over object health, damage events, projectile positions; clients can exert some
limited authority over their own bipeds. Semi-reliable delivery, persistent state plus transient
events, highly scalable to available bandwidth. Prediction is what you do between updates.
**Explicitly NOT lockstep** (lockstep was Halo campaign/Firefight and is "unacceptable" for
multiplayer FPS) and **explicitly NOT dedicated servers**. Host migration was a public-beta
disaster on Reach (players warped to `0,0,0`); our session subsystem carries 21 `election` and 21
`handoff` strings plus `speculatively host-migrating to peer %d`; deferring migration is
well-founded.

### 5.4 Aldridge's three categories are literally three C++ classes `[C]`

Three `Si` siblings of `c_replication_scheduler_client`, arbitrated by `c_replication_scheduler`
(ti `0x1952FF0`):

| category | class | typeinfo | slot 0 |
|---|---|---|---|
| persistent **state** | `c_replication_entity_database_view` | `0x1954640` | `0xB59FA0` |
| transient **events** | `c_replication_event_manager_view` | `0x1954700` | `0xB5E7C0` |
| **control** (input) | `c_replication_control_view` | `0x1954490` | `0xB4C110` |
| (roster) | `c_replication_player_view` | `0x19547B0` | `0xB5F840` |

Mirrored in the definition tables: `c_simulation_entity_definition` (4 concrete),
`c_simulation_event_definition` (**20 concrete** - `damage_caused`, `refereed_death`,
`weapon_fire`, `melee_strike`, `predicted_death`, `predicted_hard_ping`, ...),
`c_simulation_queue_element_..._base_type` (~59). Dispatcher `0xBB1BC0`, 34 opcodes;
`player_update_apply` is opcode `0x0C` (`0xBAA790`). Client authority is real and bounded exactly
as described: `claim_authority_over_abandoned_entity_slots`, `abdicate_authority`,
`entity_slot_authority_mask`, `send_client_authoritative_peer_identity_update`.
`networking:simulation:view` establishment compares a 16-byte content signature (`this+0x8D`) and
kills the view on mismatch - a schema-agreement check, not a MAC `[I]`.

### 5.5 Elimination ownership - the hypothesis ledger

From the elimination authority dossier (targets fixed-roster private Elimination on the owned
01.29 install). What the owned install proves: a controlled string scan (positive controls: the
QueueZ diagnostic at `0x162D0C5` and 519 PVP package filenames) found 151 mechanic-related eboot
strings - Elimination 2, revive 7, round 2, score 62, spawn 27, team 28, timer 24, exact
special-ammo spelling 0. The two Elimination matches are RTTI-style UI names
(`c_thud_elimination_objective_data`, properties accessor at `0x1662B40`/`0x1662B70`) - they prove
an Elimination objective/UI data type, not authority or wire layout. No package filename contains
Elimination/Trials/Osiris/special-ammo/revive despite the PVP filename control (a naming result
only; payloads are compressed and were not claimed absent). Public patch notes after the 2016-10-10
build date are not promoted as owned-build facts.

Ownership hypotheses (each `[I]`/`[U]` row is a named experiment, not a fact):

| mechanic | proposed owner(s) | status |
|---|---|---|
| roster and teams | World/BAP identities; AH binds activity roster; clients render | `[C]` systems exist; `[I]` exact split |
| Physics Host election | participant/session layer | `[C-Bungie]` topology; `[U]` algorithm (now largely closed by §2.1) |
| movement/projectiles/collisions | Physics Host with client prediction; Havok local | `[C-Bungie]`/`[C-client]`; wire form `[U]` |
| damage/death | Physics Host authoritative; clients present result | `[I]` |
| revive eligibility/progress | PH validates proximity/timing; AH may consume round sensor state | `[I]` |
| round timer | AH script owns canonical clock; clients display | `[I]` |
| round winner and score | AH evaluates sensor state supplied by session/PH | `[I]` |
| spawn selection/respawn | AH script/config plus PH placement/physics validation | `[I]` |
| special-ammo crate schedule | AH script/config schedules; PH validates pickup | `[I]`; no exact eboot string |
| match completion | AH produces result; World/BAP persists | `[I]` |
| host migration | session elects new PH; AH survives or coordinates | `[C-client]` vocabulary; `[U]` sequence |

Dossier discipline (kept verbatim in spirit): content names are not packet fields; UI RTTI is not
authority evidence; a locally running timer/score does not prove the canonical owner; "Physics
Host" and "Activity Host" are never shortened to an unqualified "host" in experiment records.

Vertical-slice evidence bar (not "six clients loaded", not "no crash"): six distinct fixture
identities and two teams of three; one unique elected Physics Host and five Physics Clients;
completed AH reservation/startup/join/entity-index establishment; one action sent, authoritatively
applied, and observed by another peer; one death and revive-state transition; one round outcome and
transition; final match completion with AH and World consequences identified separately. Host
migration is a later criterion.

### 5.6 The standing "3v3" caveat `[U]`

**"3v3" is not attested in the owned build.** Not in the eboot, not in any symbol, not in any
readable entry. `maxPlayers 6` comes from a November 2020 manifest, four years newer than the
October 2016 install. `team_picker` (`0x417380`) is fully dynamic
(`%i players is outside the %i to %i range`) and takes its bounds from the activity definition,
which is behind the Oodle wall (721 of 998 blocks in `globals_pvp_0169` are compressed,
`historicaldocs/design/package-container-pvp-content.md` §8.3). Team count and team size are `[U]`.

### 5.7 What the activity-launch lane already delivers for PvP

| built for activity launch | PvP reuse |
|---|---|
| BAP correlation table (the orphaned-response fix) | full - PvP has more subscription churn |
| AH proxy + manager messages (6/7, 16/17, 100/102) | full - same control plane for a patrol zone and a 3v3 |
| `d1-activity-host` crate skeleton | full - Elimination adds round/score/revive on top of the same connection/reservation/membership frame |
| session-parameter delivery incl. `s_activity_host_connection_information` | full (`error.private_activity_host_session_parameter_mismatch`) |
| the rung ladder + state-machine tooling | full - rung 29 is `setup:matchmaking` |
| matchmaking configuration answer | partial - solo answers it trivially; PvP needs real content |

Net-new for PvP: six distinct account SOIDs; Physics Host election and P2P replication
(client-side - we must not break it, we do not carry it); shadPS4's PS4 network primitives; six
participants; Elimination's own rules (behind Oodle).

---

## 6. The two-instance harness

**Status: designed and built, deliberately left unarmed.** The one-emulator guard holds until an
explicit activation review, and that review is the operator. No two-instance run has happened.

### 6.1 Invariants (the dry-run contract)

One run ID, two distinct instance IDs, two distinct participant IDs. Each participant has separate
user/config/save/controller/log/trace/pcap and provenance roots; nothing mutable is shared. Both
share one isolated virtual LAN; forwarding and DNS egress denied before launch and re-checked
after. Addresses and every transport-plane port deterministic and noncolliding. Capture starts
before either participant and stops after both; nonzero drops fail the run. Shutdown targets only
resolved PIDs recorded for this run - no broad `pkill` pattern. Each trace clock carries an anchor
into the common run time domain; without anchors the correlator refuses cross-instance causality.

### 6.2 The topology

```
 outer netns "d1lan"  - NO physical interface, NO veth to the host, ip_forward=0
 ├── bridge d1lan0                    10.77.0.1
 │     ├── shadnet-server             10.77.0.1:31313/TCP (+31314/UDP only if rooms are ever armed)
 │     ├── d1-responder               10.77.0.1:443, :3075 (BAP), :<webapi>  <- the C7 surface
 ├── inner netns "A"  veth -> bridge  10.77.0.2   shadPS4 #1 (3074/2001/2002 all its own)
 └── inner netns "B"  veth -> bridge  10.77.0.3   shadPS4 #2
```

**Two namespaces are mandatory, not a preference:** the guest binds UDP 3074, 2001 and 2002 on
`0.0.0.0` (observed) and none is parameterised by us; two guests in one namespace collide at bind
time, and a failed bind produces silence that reads as "the client never tried P2P". Containment is
unweakened: no physical interface, no host veth, `ip_forward=0`, fail-closed DNS in all three
namespaces, no default route in the inner namespaces. `[I]` - designed, not built; whether nested
veth creation works unprivileged under `unshare -r` is untested.

Forced server changes: `shadnet-server` must bind `10.77.0.1`, not loopback (loopback is
per-namespace - a loopback-bound server is invisible to both instances). Use literal IPs, not
names. Serving two distinct participants: `d1-responder`'s identity is process-global env read
inline at 10+ call sites, and `/SignOn` does not look at the request body - the cheap route is
**two `d1-responder` processes with different env, one per inner namespace**, with the shared
session store (the C7 blob) extracted into the shared component; decide before writing the storage
layer.

### 6.3 The local-IP fix is load-bearing

shadPS4's `NetUtilInternal::RetrieveIp()` probed `1.1.1.1:80` over TCP; with no default route that
fails and upstream fabricated `ORBIS_OK` + `"127.0.0.1"`, which parses cleanly - so Demonware's own
error strings could never fire. All 28 `sceNetCtlGetInfo` call sites were enumerated; four request
`IP_ADDRESS (0xE)`, and one is **`0x2E9FC0` - Demonware bdCore, `networking:demonware`** - feeding
the answer straight into `sceNetInetPton`, with `0xAE9940` republishing it as `insecure_ip`. The
implemented fix keeps the connect-probe but switches it to `SOCK_DGRAM` (route lookup and
source-address selection with no packet and no hang), returns `ORBIS_NET_CTL_ERROR_NOT_AVAIL` on
failure, and uses `getifaddrs` only as fallback. (The originally-prescribed getifaddrs-first fix
failed its own host-arm control - see Dead claims. Whether signon f14 additionally dominates the
peer-advertised address remains `[U]`.) Detail: `status/shadps4.md`.

### 6.4 `d1-harness` - the contract made executable, and left cold

`d1-server/crates/d1-harness`: 51 controls (`d1-harness controls`, exit 2 = do not cite), 10
end-to-end tests, and **no subcommand that launches anything**. Commands: `layout` (deterministic
from a seed, creates nothing), `env --slot a|b`, `preflight --stage layout|namespace|arm`
(**exit 3 = REFUSED**), `pid` / `shutdown` (ledger, shutdown by re-verified pid only).

What the layout pins:

* **Ports carry a scope** - `lan` endpoints globally unique; `host` ports unique by number;
  `namespace-local` ports may repeat only while `netns_separation` is declared (the only thing that
  makes both guests binding 3074 legal). The prototype `capture-rig/two-instance-layout.py` gave
  participant B `game_port=3075`, which collides with `d1-responder`'s BAP listener, and its
  self-test passed; it now carries a retraction banner and a control here re-asserts the clash.
* **Participant A is anchored to `run-boot.sh`** - same account SOID, character SOID, 206 body,
  503 body, profile and manifest; a control parses `run-boot.sh` and compares.
* **Participant B's `D1_503_RAW` is derived, not invented** - A's pinned 45-byte body carries the
  account SOID as a 64-bit run at exactly one bit offset (36, re-derived by search, uniqueness
  asserted); B's body is that template with its own SOID spliced in, leaving the family-5 root at
  bit 165 intact.
* **Identity separated at four layers end to end** - NP npid, shadNet `user_id`, `d1-responder
  --account-id`, D1 account/character SOIDs. One shared `shadnet-server` in
  `--identity-mode per-client` refuses a second login for a live npid (`LoginAlreadyLoggedIn`), so
  conflation becomes an observed refusal.

The preflight **refuses what it cannot evaluate** - never skips, never passes an unknown; 21 named
refusal reasons, every one driven to fire by a control, plus a saturating positive arm. The
containment probe is a pair of packet-free UDP `connect()` route lookups: toward the peer must
succeed selecting this participant's own source address, toward `1.1.1.1` must fail.

Two refusals need operator work before arming: **controller separation is unresolved**
(`capture-rig/virtual-pad.py` hardcodes one USB VID/PID `0x045E/0x028E`, so two pads are one device
to SDL; candidate mechanisms - per-participant `/dev/input/eventN` bind mounts, or distinct USB ids
with a supplied `SDL_GAMECONTROLLERCONFIG` - are both `[U]`), and **no uninstrumented control run
is recorded**. Also checked because it is silent: shadPS4 has no `--user-dir` flag and resolves
`$PWD/user` first, so a stray `user/` in the working directory merges both participants'
identities.

The operator arming sequence (steps 0-3 safe today, everything from step 4 gated on the activation
review) is preserved verbatim in `historicaldocs/design/two-instance-harness-design.md`; it is
deliberately not scripted - a one-command arming script is exactly what the one-emulator guard
exists to prevent. `--expect-sha` on the emulator binary is not optional in practice: the fork
carries uncommitted changes, so a git hash cannot tell two binaries apart.

---

## 7. Vision: anticheat, comp client, paired recorder

> **THIS SECTION IS VISION. Nothing in it is built, decided, or scheduled. `[U]` throughout by the
> source document's own banner**, except lines citing measured results recorded elsewhere. Writing
> it down was not committing to it. The near-term project queue outranks all of it.

* **The reframing that survives scrutiny:** not "an anticheat" but a **conformance/divergence
  harness** - replay what a client asserted against what the server and its peers observed at the
  same causal point, and flag the mismatch. Its two uses are offline forensics and, more valuably,
  validation of our own reconstruction ("does our server behave the way Bungie's did?"). The
  earlier framing "replay with timing linked = anticheat in one shot" is **retracted**: the model
  is host-authoritative with bounded client authority, explicitly not lockstep and not
  deterministic rollback - there is no single replay that "is" the anticheat.
* **The P2P blind spot** is the real insight: peer-to-peer interactions the host never adjudicates
  are invisible to server-side checks. The proposal is sampled P2P telemetry, correlated **by
  identity (nonce/sequence join), not by clock**, with select memory pieces - and multi-party
  corroboration so no single modified client controls the record. Telemetry would carry the same
  reach tags as findings (client-asserted = synthetic-grade; peer/server-derived = observed-grade).
* **Evidence, never verdicts:** no individual detector is trusted enough to ban; the server owns
  enforcement; a false positive is not symmetric with a false negative; the default for an
  uncertain case is collect more evidence. Client-side anticheat on an open-source emulator is
  bypassable permanently - that is the premise, not a bug, and it is why behavioural evidence
  (aim-trajectory analysis in the XGuardian style, arXiv 2601.18068) sits above host-integrity
  evidence. Nothing is calibrated for this title, and Destiny's own aim assist is a real risk to
  the whole behavioural lane.
* **Rollback is not attainable** (shadPS4 has no savestate subsystem); what is attainable is a
  flight recorder with keyframes - snapshot pairs at matched causal events, stills not video.
* **The comp client** (egui; match setup for the 21 launchable Crucible activities 393-413 and 33
  enumerated PvP maps, a Trials tracker, roster state, post-match summary) is the piece most likely
  to be built first because it is useful with zero anticheat attached. Scope discipline: a comp
  client that only sets up and tracks matches is finishable.
* **Licence gate before any prior-art code is read**: the candidate sensor codebase's licence is
  unstated, which is a blocker, not a detail; AGPL and unlicensed material stay excluded; a paper
  is not a licence grant for its reference implementation.

Full text: `historicaldocs/design/anticheat-vision.md`.

---

## 8. Open unknowns

Ranked by what would close them. (Tier-2/3 detail preserved in
`historicaldocs/design/PLAN-B-SHADNET-MATCHING2.md` §9.)

1. **Does D1 ever call `sceNpMatching2`?** One boot-to-orbit run, then grep the run log for
   `Lib_NpMatching2` with `Lib_NpWebApi` as the positive control. Converts §4.3 from `[I]`-strong
   to settled and formally retires the room protocol. `[U]`
2. **Did the managed-session WebAPI POST succeed in existing runs, or did the client take an
   offline branch?** Both `Asynchronous session creation completed successfully` and `...failed`
   exist at `0xB69A10`, and `0xB621A0` carries `Creating '%s' offline session`. The answer is
   already in `runs/` logs - highest value-per-effort item. `[U]`
3. **What `apiGroup` and version string does D1 pass?** (`/%s/sessions`; shadPS4 logs
   `apiGroup='{}' path='{}'`.) Pin before writing a single route. `[U]`
4. **Does the BAP invite route (38-41) carry the 78-byte descriptor**, or only trigger the platform
   invite? If it carries it, a two-instance harness needs no PSN invitation plumbing. `[U]`
5. **`sessionMaxUser = 1`** - surprising for a six-person fireteam; check against a logged
   multipart body. `[U]`
6. **Does the client adopt a server-assigned session id, or impose its own?** (`0x2E8F60` renders,
   `0x2BC830` uses the string as the URL id; decompile `0x2BC8C0`'s response parser.) `[U]`
7. **Which JSON keys does the client require in the `POST /v1/sessions` response?** Parsers at
   `0x2B6960` / `0x105B330`. `[U]`
8. **Is the `0x2E8B70` peer-to-peer descriptor carrier an alternative to the WebAPI or strictly
   downstream?** If alternative, C7 is cheaper still. `[U]`
9. **Is `[0x266DEAC]` (the host-migration descriptor) ever written?** `[I]` only reads found - and
   the xref defect (§4.3) blinds the scanner to exactly the store form that would set it. Live
   control: `d1guest read 0x266deac` must read 0 while `[0x266DD12]` reads non-zero in the same
   read.
10. **The `bdNetImpl` contradiction** (§1.3) - one `d1-eboot rtti` walk of the `[0x29353D8]`
    vtable. `[U]`
11. **Platform enum `7 = PS4`** in BAP 23/24 - no positive control. `[U]`
12. **Team count / team size** ("3v3") - behind Oodle, or a retail capture. `[U]`
13. **The election failsafe constant** `DAT_027A49B8` and the refusal soft timer
    `[0x34DB20]+0xF8`. `[U]`
14. **`active_join_controls` off-by-one**: `0xB73E00` reads `session+0x6C28 + 0xE1`;
    `capture-rig/session-params.py` records the value at `+0xE2`; consistent only if the code's
    pointer is `value_base - 1`. Pin before anyone writes a parser. `[U]`
15. **Are the five closed-flag bits, the five managed-session creation flag bits (`& 0x1F`) and the
    five `cui::s_fireteam_privacy_option` entries the same five?** Suggestive, unproven. `[U]`
16. **Is session index 2 really `group`?** `[I]`; one live read of
    `network_session_parameters(2)` in patrol vs a strike settles it.
17. **Does signon f14 dominate the peer-advertised external address** over the
    `sceNetCtlGetInfo`-in-Demonware path (§6.3)? `[U]`
18. **Can the nested-namespace topology be built unprivileged?** Untested. `[U]`
19. **Whether D1 private matches use host migration** (before designing session state) - confirm
    against the Truman material. `[U]`
20. **How could `sessionPrivacy` ever become `"private"` (2)?** The enum member exists; the only
    writer found sets 1. `[U]`

Unresolved instrument disagreement, recorded not reconciled: shadnet-server's `--peer-port`
defaults to 3074 (the port the guest is observed to bind) because no request field carries a port
and shadPS4's `GetP2PConfiguredPort()` returns 0 at HEAD - two instruments disagree about the
peer port.

---

## 8b. The `bdLog` ABI, and the name bindings it recovered

Salvaged 2026-08-11 while retiring `historicaldocs/ingress/`; reach `static-client-inference`, `[C]` on the
pinned 01.29 eboot. This is the mechanism behind `design/instruments-static.md` §2.7's rule that a
log message naming `Class::method` does not name the body that emitted it.

**The call shape:** `bdLog(type, baseChannel, channel, file, function, line, format, ...)`, with
`bdLogMessage` at **`0xF78C90`** and the argument assignment at **`0xF78CA8`..`0xF78D45`**. The
`function` argument is `__FUNCTION__` (unqualified), the `file` argument is a `__FILE__` literal,
and the format string is prose about some other object. Attribute by the arguments, never by the
message text.

**Its two positive controls**, both `[C]`: `0x10421F2` resolves to `bdPlatformTrulyRandomInit`
(direct), and `0xF695D6` resolves to `handleGetAccessTokenReply` in `bdAuthPS4.cpp` **through one
hop of register laundering** - which is the control that proves the laundering tracker works
rather than merely not crashing.

**Five name bindings promoted by that method** and recorded nowhere else in the live docs
(`static-client-inference`, `[C]` for the binding, `[I]` for any behaviour inferred from the name):

| address | binding |
|---|---|
| `0xF84BE0` | `bdIPDiscoveryClient::getPublicAddress` |
| `0xF999B0` | `bdUnicastConnection::sendInitAck` |
| `0xF96F60` | `bdUnicastConnection::receive` |
| `0x103DE00` | `bdSocket::create` |
| `0x1040C90` | `bdBase64::decode` |

⚠ **`0x103D880` is UNADJUDICATED and must not be cited either way `[U]`.** The round-2
verification note corrects it to **`getCypherAlgorithmKeySize`** (noting that the
`bdCypher::getAlgorithmKeySize` spelling at `0x16BB76C` is printf-format text, and is a standing
negative control). Our own generated `capture-rig/eboot-atlas.tsv` labels the same address
`bdhashtiger192::vfunc[3]`. **Nobody has reconciled these two.** The atlas label is a derived-tool
attribution and the note is a static read; settle it with `d1-eboot disasm` at the address plus an
RTTI walk before either spelling is used.

## 9. Dead claims

Every claim below was asserted in a source document and later killed by a banner, a measurement, or
a superseding document. **None of them is live. Do not re-adopt them.**

| # | dead claim | where it lived | what killed it |
|---|---|---|---|
| 1 | "Peer key material is platform-issued (PSN); the single most important open question for a two-instance run" | peer-mesh doc §4 | Retracted 2026-08-08: the client mints its own key material locally with Demonware's MT19937 (`bdSecurityKeyMap::initKey`, `0xF875A0`, no input, always returns true), re-derived by disassembly three ways |
| 2 | "The 36 (`0x24`) bytes are `{securityID, key}`" | peer-mesh doc §4 | The 0x24 bytes are the **secure address**; the full record is **78 (`0x4E`) bytes** with ID/key as separate fields |
| 3 | "The managed-session transport is `sceNpMatching2`" | peer-mesh doc §4/C7 (original) | It is the **PSN Session WebAPI over plain HTTP** (`POST /v1/sessions` etc.); import-closure + region-entry evidence, §4.3 |
| 4 | "C7 (the PSN managed session) is the hardest blocker" | peer-mesh doc §7 ranked list | Plan B §1.4: C7 is a ~6-endpoint store-and-forward HTTP surface, roughly the **easiest** of the nine; **C1 is now the hardest** |
| 5 | "BAP, the front door, is SCTP-shaped and DTLS-wrapped" (as applied to the client-service channel) | networking-architecture §2 (original reading) | Measured on the wire: BAP is **TCP with AES-128-GCM**; a UDP/DTLS listener on 3075 captured nothing while the client talked TCP to the same address |
| 6 | "`bdBitBuffer` is unconfirmed as a name" (rev 2's own correction) | networking-architecture rev 2 | Measured: both `bdBitBuffer` and `bdByteBuffer` exist as separate named classes |
| 7 | "`[U]`: which layer does the `bd*Chunk` association stack serve" | networking-architecture §2 | Settled 2026-08-08: **none** - compiled in, unreachable from game code, with three passing positive controls |
| 8 | "A `BD_*` error-code diff will date the Demonware generation" | networking-architecture §7.3 plan | The `BD_*` strings are asserts and config constants, not the error enum |
| 9 | "The `panther` hostnames appear as literals in the eboot" (implied by the prediction) | networking-architecture §2 | `[C]` FALSE - no such literals; runtime URL construction explains absence; the codename itself stands as `[O]` historical |
| 10 | "One client becomes the activity host (`ipahp`)" `[CONFLICT]` | pvp-host doc §5 / status/ROADMAP.md wording | Resolved 2026-08-08: the AH is a **service**; `AHP` = activity host **proxy**; the argv proof (BAP 6 carries a process command line; 16 -> 17 resolves an IPv4+port; BAP 153 hands the AH a socket) |
| 11 | "`0x783E90` is the ipahp startup state machine" (transport-relevant) | pvp-host doc §5 original | It is a static-initialiser string table: 1,057 instructions, zero calls, zero jumps |
| 12 | "shadNET's room protocol is the surface; flip `matching2_enabled`; implement ~12 room handlers" | the briefing Plan B was launched with | Recon overturned it: D1 never sends the commands; the surface is ~6 HTTP endpoints in `d1-responder`; `matching2_enabled` stays false |
| 13 | "The shadPS4 P2P stubs are a hard blocker for D1" | two recon agents inside Plan B | Right about the code, wrong about the consequence: the stubs block only the Matching2 path, which D1 does not use; D1's mesh runs over ordinary `sceNet` sockets |
| 14 | "Replace the connect-probe with `getifaddrs()` first-match" (the §6.4 prescribed fix) | Plan B §6.4 body | Failed its own host-arm control: five non-loopback interfaces, first-match returns `10.99.99.1` not `203.0.113.84`. The probe was kept, switched to `SOCK_DGRAM` |
| 15 | "Both instances would advertise 0.0.0.0 via `sceNpSignalingGetLocalNetInfo`" | Plan B §6.4 body | Dead code: D1 imports no `sceNpSignaling*` NID; the real consumer is `sceNetCtlGetInfo` inside Demonware (`0x2E9FC0`), which makes the fix load-bearing for a different reason |
| 16 | "Replay with timing linked = anticheat in one shot" | anticheat vision §2 (retracted framing) | The model is host-authoritative, not lockstep and not deterministic rollback; the salvageable shape is a conformance/divergence harness |
| 17 | `two-instance-layout.py`'s port plan | the layout prototype | Not realizable (B's `game_port=3075` collides with the BAP listener; 3074 is guest-bound, not allocatable); its self-test passed while wrong; superseded by `d1-harness` with a control re-asserting the clash |
| 18 | "`historicaldocs/design/observability-tooling.md`'s weighting of passive pcap for the service layer" | networking-architecture §6.2 | If we run auth/keys, plaintext is free by construction on our own lanes; retail-console capture remains irreplaceable for Bungie's responses only |

Also binding, from the sources' own banners: the anticheat vision document is `[U]` throughout and
must never be cited as a plan; and "both instances reached the same screen" / "it did not crash"
are **forbidden as evidence** for any two-instance result (§4.6).

---

## 10. Sources

| archived source | what this document took from it |
|---|---|
| `historicaldocs/design/networking-architecture.md` | §1 wholesale: the Demonware/Halo lineage, the adopted-with-verification stack, the BAP TCP/GCM correction, the panther/DNS estate, the Tribes model, prior-art and licensing posture, the bd*Chunk settlement banner |
| `historicaldocs/design/peer-mesh-and-the-server-surface.md` | §2 wholesale: the election, the 20-peer ceiling, the twelve consultation points, the two lists, bdDTLSAssociation, secure addresses; its §4 is RETRACTED (Dead claims 1-3) and only the plumbing trace survives |
| `historicaldocs/design/PLAN-B-SHADNET-MATCHING2.md` | §3 and §4 wholesale: the C7 verdict, the 78-byte record, the sessions API spec, the Matching2 closure evidence and its §10 instrument defects, the identity table, the fireteam-join sequence, the do-not-build list, the acceptance criteria, §6's topology corrections |
| `historicaldocs/design/pvp-host-authority-hypothesis.md` | §5.1-5.4, 5.6-5.7: the three-plane model, the resolved AH-is-a-service conflict (§5 banner), rung 28, the §13 update (Aldridge taxonomy as C++ classes, message ids, reuse table, the 3v3 caveat) |
| `historicaldocs/design/elimination-authority-dossier.md` | §5.5: the content-scan results, the ownership hypothesis ledger, the vertical-slice evidence bar, the discipline list |
| `historicaldocs/design/two-instance-harness-design.md` | §6: the invariants, activation checklist, and the built-and-cold `d1-harness` state |
| `historicaldocs/design/anticheat-vision.md` | §7, compressed, under its own `[U]`-throughout banner |

Live companions: `design/bap-transport.md` (the BAP wire format and message table),
`design/activity-host.md` (the AH contract C1/C2/C11 bind to), `design/architecture.md` (the
overall server-role split), `design/licensing.md`, `historicaldocs/design/package-container-pvp-content.md`
(launchable activities, PvP maps, the Oodle wall).
