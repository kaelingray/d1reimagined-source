# BAP wire transport - envelope, handshake, header classes, message ids, span, and routing

**State.** The plaintext transport is solved end to end `[C]`: envelope, base header, the
`0x1e/0x1f/0x19/0x1a` handshake, all 58 message ids, and all eight header classes (class 3
`observed-live`). The AH push route is solved: the client consumes BAP **message 9**, not 100,
and `recipient_id` is the AH id. Open: class 5/6 unknown header fields, `0x1a` plaintext bytes
28-47, and live confirmation of the RSAT envelope span fix (§8).

Pinned client: CUSA00219 patch 01.29. Pinned eboot SHA-256:
`7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954`
(`vaddr = file_offset - 0x4000`). Licence to cite static reads: `d1-eboot controls` 22/22 PASS.

## Contents

1. The envelope (6 bytes)
2. The inner base header (6 bytes) and recipient matching
3. Header classes - all eight, with layouts for 3/5/6
4. The handshake state machine (`_channel_starting`, `_authenticating`, the sealed hello)
5. The receive state machine and the client-side probe contract
6. The message id table - all 58 ids
7. Push delivery and establish-client routing (message 9 vs 100)
8. The envelope span - what makes the RSAT response array non-empty
9. Open `[U]`s, consolidated
10. Dead claims
11. Sources

---

## 1. The envelope - 6 bytes, `0xB2D9F0` `[C]`

Reach: `static-client-inference` with one `observed-live` cross-check (the 140-byte client hello
in `runs/20260806-152657-bap-hold/`).

```c
recv(conn, 6, &conn[0x16d0], &conn[0x16cc], &err);      // 0xb2ed40, resumable by offset
if (got != 6) { err stays 0; return false; }            // "Failed to read envelope offset '%d'"
conn[0x16d6] = conn[0x16d0];                            // byte 0 -> version
conn[0x16d7] = conn[0x16d1];                            // byte 1 -> mode
movbe *(u32*)&conn[0x16d8] = *(u32*)&conn[0x16d2];      // bytes 2..5 -> BIG-ENDIAN size
if (conn[0x16d6] != 1) {                                // 0xb2db2e: cmp cl, 1
    log("Failed to parse envelope version '%d' (expected %d)", byte0, 1);
    err = 0x0c;                                         // _connection_failure_envelope_version_mismatch
}
switch (conn[0x16d7]) {                                 // 0xb2dbce
    case 0: case 2: conn[0x16c8] = 3; break;            // -> read base header
    case 1:         conn[0x16c8] = 2; break;            // -> read 16-byte signature first
    default:        err = 0x0e; break;                  // _connection_failure_unknown_envelope_type
}
```

**Layout: `u8 version(=1)` · `u8 mode(0/1/2)` · `u32be payload_size`.** `[C]`

- Byte 0 is compared against the literal `1`, and `1` is what the log prints as "expected".
- **Mode 1 is the signed/secured path**: `0xB2DC50` reads **16 more bytes** into `conn+0x16E0`
  (`"Failed to read signature offset '%d'"` at `0xB2DDAD`) before the header. Modes 0 and 2 skip it.
- Observed hello: `01 02 00 00 00 86` = version 1, mode 2, size 134; 134 + 6 = 140 = the captured
  frame. `[C] observed-live`
- **A reply with mode 0 or mode 2 is accepted; echoing 2 is the safe choice.** `[C]`
- **Mode is not a discriminator for anything else.** `conn[0x16d7]` is written once at `0xB2DB17`
  and read exactly once, at `0xB2DBC8`, purely to pick the next receive state. A grep over the
  full disassembly of the BAP module (`0xB28000`..`0xB34000`) finds no other reference. Mode 2
  does NOT mean "sealed with the session key"; the sealing that exists lives inside message
  bodies (§4.5), not in the envelope. `[C]`

Socket behaviour `[C]`: `0xB2ED40` is resumable by offset (byte counters at `conn+0x1700` for the
header, `conn+0x16CC` for the envelope), so splitting a frame across TCP segments is fine - and a
short frame parks the client silently with **no timeout, no log, no error branch** until the 10 s
BAP connect timeout (`_connection_failure_suicide`). A peer close is loud:
`_connection_failure_failed_to_read_endpoint_closed` (string at `0x167A75F`). Do not close the
socket after answering.

## 2. The inner base header - 6 bytes, `0xB32810` `[C]`

```c
bool decode_base_header(const u8 *src, size_t len, hdr *dst) {   // 0xb32810
    if (len < 6) return false;
    movbe *(u32*)(dst+2) = *(u32*)(src+2);     // sequence, big-endian
    u16 t = bswap16(*(u16*)src);               // type, big-endian
    if (t < 0x130) return true;
    log("networking:investment:bap:header_base: decoded invalid bap type %d", t);
    return false;
}
```

**Base header = `u16be type` ‖ `u32be sequence`, 6 bytes, `type < 0x130`.** `[C]`
The encoder is the three-instruction `0x559DE0` (`word[dst] = type; dword[dst+2] = seq`), an
independent confirmation of the field offsets. Observed hello inner header: `00 1e 00 00 00 00`
= type `0x001E`, sequence 0. `[C]`

After the base header the receiver reads `conn[0x1784]` more bytes, sized by the received type's
header class (§3). `0xB2E06B` short-circuits when that size is 6, i.e. a type with header size 6
has no bytes beyond the base header. The captured hello is exactly `6 + 6 + 128 = 140`, so type
`0x1E` has header size 6. `[C]`

### 2.1 Recipient matching - the sequence is ECHOED, never incremented `[C]`

`update_receiving_header` (`0xB2E030`) walks two in-flight queues (`conn+0x1C18`, stride `0x198`,
2 iterations) and accepts an entry only if both hold:

```
0xb2e116  call qword [rax+0x18]        ; in-flight message object -> expected reply TYPE
0xb2e119  movzx ecx, word [r13+0x1788] ; received type
0xb2e121  cmp eax, ecx / jne next
0xb2e125  mov eax, [r12+0xe]           ; in-flight stored SEQUENCE
0xb2e12a  cmp eax, [r13+0x178a]        ; received sequence
0xb2e131  je  accept
```

No match ⇒ `"update_receive() failed to find recipient for header"` ⇒
`_connection_failure_failed_to_find_message_recipient`. **The reply carries the request's own
sequence value verbatim.** `[C]`

### 2.2 The "recipient registry" is the client's own in-flight-request queue `[C]`

There is nothing to register and no API to trigger; an entry exists exactly as long as a request
the client sent is outstanding. Reading `0xB2E0DC`..`0xB2E145` literally:

```c
q = conn + 0x1c18;
for (i = 0; i < 2; i++, q += 0x198) {          // exactly two queues
    if (*(int*)(q - 8) <= 0) continue;          // queue empty -> skip
    e = ((void**)q)[*(int*)(q - 0xc)];          // HEAD entry only, not a scan
    obj = conn->msg_objs[*(int*)(e + 4)];       // conn + 0x80 + idx*8
    if (obj->vtable[0x18](obj) != rx.type) continue;   // FIRST test: expected REPLY TYPE
    if (*(u32*)(e + 0xe)  != rx.sequence) continue;    // SECOND test: the sequence it sent with
    accept(e);
}
// neither queue matched:
h = conn[0x1138];                               // the push-notification handler
if (!h || !h->vtable[0](h, rx.type))            // accepts by TYPE ONLY, no sequence
    -> "update_receiving_header() failed to find recipient for header [sequence %d][type %s]"
    -> _connection_failure_failed_to_find_message_recipient
```

Consequences, all corroborated by the `0x001A`-too-early runs
(`runs/20260806-153855-bap-securehello/`, `runs/20260806-153950-bap-seq1/`) `[C]`:

1. Keyed on `(expected-reply-type, sequence)`, **type tested first** - a wrong type `jne`s away
   before the sequence compare is ever executed, which is why seq 0 and seq 1 behaved identically
   in those runs.
2. The 4 bytes at `e+0x0E` are a **sequence, not a handle** - written when the client sends; the
   hello carried `00 00 00 00`, so the value to echo is 0.
3. A recipient for a given response exists only after the client has sent the matching request.
   `0x001A` has no recipient until `enter(_authenticating)` has sent `0x0019`, which happens only
   after `_channel_starting` succeeds - we cannot answer `0x001A` before answering `0x001F`.
4. Only the **head** of each queue is examined, so responses must arrive in order per queue.

The `conn[0x1138]` fallback is the **push** path: server-initiated notifications are legal and
are dispatched by type alone, no sequence, no in-flight entry. `0x001A` is not one of them. `[C]`

## 3. Header classes - all eight `[C]`

Reach: `static-client-inference` for classes 5 and 6; **`observed-live` for class 3** (the
client's own BAP message 8 in `runs/20260808-195418-F2-STATE3`). Implemented in
`d1-server/crates/d1-bap/src/conversation.rs` (`HeaderExtras`, `write_header_with`,
`BapConversation::push_with_header`).

### 3.1 How a header class is chosen

```c
obj      = g_bap_message_singletons[type];   // 0x559DD0 -> table @0x26A0DB0 (BSS)
hdr_idx  = obj->vfunc[1]();                  // vtable +0x08 - a per-message-class constant
hdr_cls  = g_bap_header_classes[hdr_idx];    // 0x55C1E0 -> table @0x19234A0
size     = hdr_cls->vfunc[1]();              // vtable +0x08 - THE HEADER SIZE
conn[0x1784] = size;                         // via decode_bap_base_header 0xB2EE60
```

⭐ **`vfunc[1]` (`+0x08`) on the message class is the same number §6's table calls the
"category". The `category` column IS the header-class index** - 1 request, 2 response,
3 notification, 4-7 server-originated - so `capture-rig/bap-message-table.tsv`'s `category`
needs no translation. Examples: msgs 6/16 → class 1 (6 B); 7/17 → class 2 (8 B, carries
status); 8 → class 3 (14 B); 102/110/150 → class 5 (24 B); 100/153/160 → class 6 (32 B).

⚠ Contradiction flag, unresolved by any banner: the source's class-index example table also
listed message **160** on the class-3 row alongside message 8, while its own class-6 row and the
generated message table (§6) both put 160 in category 6. The class-6 assignment is the one backed
by the generated table with controls; the class-3 listing of 160 is `[U]` and likely a slip.

`c_investment_bap_message_activity_host_to_client_notification::vfunc[1]` is literally
`0x691B0: mov eax, 6; ret` - message 100 is class 6 by direct read. `[C]`

⚠ **Identical-code folding trap.** `0x691B0` is simultaneously the size thunk of header classes
0, 1 and 4, because `mov eax, 6; ret` is one function to the linker. Likewise `0xB1BA0`
(`mov eax, 0x20; ret`) is named `bdHMacSHA256::vfunc[4]` by the atlas - a coincidence of value,
not a Demonware connection. Read what a thunk returns, never what it is named.

### 3.2 The eight classes

The header-class table `0x19234A0` holds eight 8-byte singletons at `0x1923460 + 8*i`, with a
NULL at index 8 (its own terminator control). Vtables are contiguous, 20 quadwords apart
(18 slots + `{0, typeinfo}`); resolution needs `DT_SCE_RELA` (slots read as zero in the file
image). Every size fn and flags fn is a two-instruction constant thunk. `[C]`

| idx | class | vtable | size fn | size | flags fn | flags |
|---|---|---|---|---|---|---|
| 0 | `c_bap_message_header` | `0x1922E50` | `0x691B0` | 6 | `0x14D0` | `0x00` |
| 1 | `c_bap_message_client_request_header` | `0x1922EF0` | `0x691B0` | 6 | `0x14D0` | `0x00` |
| 2 | `c_bap_message_client_response_header` | `0x1922F90` | `0x55D580` | **8** | `0x55D570` | **`0x18`** |
| 3 | `c_bap_message_client_activity_request_header` | `0x1923030` | `0x55CAA0` | **14** | `0x1D3880` | `0x04` |
| 4 | `c_bap_message_client_notification_header` | `0x19230D0` | `0x691B0` | 6 | `0x14D0` | `0x00` |
| 5 | `c_bap_message_service_request_header` | `0x1923170` | `0x55A460` | **24** | `0x55CF20` | `0x43` |
| 6 | `c_bap_message_service_activity_request_header` | `0x1923210` | `0xB1BA0` | **32** | `0x55D220` | `0x4F` |
| 7 | `c_bap_message_response_header` | `0x19232B0` | `0x55D580` | **8** | `0x55D570` | **`0x18`** |

### 3.3 Flag bit `0x10` = "carries a status"; only classes 2 and 7 have it `[C]`

Across all eight classes, bit `0x10` set ⟺ a real status accessor exists (vtable slots 14/15 at
`+0x70`/`+0x78`: `0x55D7B0 mov [rdx+6], si` / `0x55D7C0 movzx eax, word [rsi+6]` for classes 2
and 7; stubs `0x50FE20 ret` / `0x1DF0 xor eax,eax; ret` for 3, 5, 6). `0x4F & 0x10 == 0`, so
class 6 does NOT carry a status and the client never demands `0xC8` on a class-6 frame.
`d1-bap`'s `HEADER_FLAG_HAS_STATUS = 0x10` gloss stands; asserted as
`conversation::tests::only_classes_2_and_7_carry_a_status`. (See §10 for the retracted
"capability mask" claim.)

### 3.4 Layouts of classes 3, 5, 6 `[C]`

Encoders and decoders mirror each other instruction for instruction (class 3: `0x55CAB0` /
`0x55CB20`; class 5: `0x55CF40` / `0x55CFC0`; class 6: `0x55D240` / `0x55D2D0`). All call the
shared base codec (`0x55D550`/`0xB327E0` encode, `0xB32810` decode) for `u16be type @ +0x00` and
`u32be sequence @ +0x02`. `FUN_002A2BF0(p)` is `*(u64*)p = bswap64(*(u64*)p)`.

```
class 3 - c_bap_message_client_activity_request_header - 14 bytes
  +0x00  u16be  type
  +0x02  u32be  sequence
  +0x06  u64be  activity_host_id          [C] observed-live, §3.5

class 5 - c_bap_message_service_request_header - 24 bytes
  +0x00  u16be  type
  +0x02  u32be  sequence
  +0x06  u64be  [U]
  +0x0E  u64be  [U]
  +0x16  u16be  [U]                       (rol word [+0x16], 8)

class 6 - c_bap_message_service_activity_request_header - 32 bytes
  +0x00  u16be  type
  +0x02  u32be  sequence
  +0x06  u64be  connection index          [C] - the relay reads it, §7.2
  +0x0E  u64be  [U]
  +0x16  u16    [U]  NEVER byte-swapped - copied verbatim by BOTH codecs
  +0x18  u64be  [I] activity_host_id      §3.6
```

**Tiling control:** every field is exposed by a setter/getter vtable slot pair, and the pairs
cover each class's byte count with no gap and no overlap
(`2+4+8 = 14`, `2+4+8+8+2 = 24`, `2+4+8+8+2+8 = 32`). Slot pairs: 4/5 (`+0x20`/`+0x28`,
accessor `0x55D1C0`/`0x55D1D0`) → `+0x06` on classes 5/6; 6/7 (`0x55D1E0`/`0x55D1F0`) →
`+0x0E`; 8/9 → class 3 `+0x06` (`0x55D1C0`/`0x55D1D0`) and class 6 `+0x18`
(`0x55D530`/`0x55D540`); 10/11 (`0x55D200`/`0x55D210`) → `+0x16`; 12/13
(`0x55D7D0`/`0x55D7E0`) → `+0x02`; 14/15 → status (stubs except classes 2/7). `[C]`

⚠ **`+0x16` is the one field where byte order is not a choice.** Class 5 swaps it; class 6 does
not (`0x55D240`/`0x55D2D0` copy the two bytes verbatim). `d1-bap` emits it native-endian for
class 6 and big-endian for class 5, matching the client; the distinguishing test is
`header_class_5_is_24_bytes_and_swaps_the_field_that_class_6_does_not`.

### 3.5 Class 3, proved on a real wire `observed-live` `[C]`

The client's own BAP message 8 join request, first 14 bytes:

```
00 08 | 00 00 00 03 | d1 00 00 00 00 00 0a 01
 type      sequence            u64be @+0x06
  = 8         = 3        = 0xD100000000000A01
```

and, same run: `networking:activity_client: ... Sent join request: [AH D1000000:00000A01][nonce
1303812511]`. `%08X:%08X` of `0xD100000000000A01` is exactly `D1000000:00000A01`. **The class-3
u64 at `+0x06` is the activity-host id, big-endian - on the wire, not inferred.** This is the
method's control for classes 5 and 6, which are the same three instructions in the same encoder
family. Regression test: `header_class_3_reproduces_the_live_message_8_header`, with a
little-endian negative control (the trap that cost a run once - see
historicaldocs/design/RUNG-30-REACHED.md §3.1).

Bonus from the same frame `[C]`: message 8's body (codec `0x55EC70`, inner type 1) is
`01 | 00 00 00 03 | 00 00 00 ee | 00 00 00 00 | <payload>` = inner type 1, udt 3, len 238,
aux 0; **the join nonce is a big-endian i32 at payload offset 0** (`0x4DB6999F = 1303812511`),
followed by the AH id (u64be), then machine id and player id as raw MSB-first byte strings.

### 3.6 Class-6 field meanings

- `+0x06` **connection index** `[C]` - three confirmations in §7.2.
- `+0x18` `[I]` very likely the activity-host id: the semantic slot pair (8/9) that reads
  class 3's AH id is the same pair class 6 wires to `+0x18`. ⚠ No message-100 frame has ever
  been on any wire, so nothing has confirmed it; a later session recorded these fields as
  "still zero and still unread" - both statements are compatible ( `[I]` by slot-pair analogy,
  unconfirmed on wire), noted here rather than smoothed. `HeaderExtras::default()` is all zeros
  and is what ships; `HeaderExtras::class6_activity_host(id)` exists for the experiment.
- `+0x0E`, `+0x16` on class 6: `[U]`, zero.

### 3.7 What ships in `d1-bap`

```rust
pub struct HeaderExtras { pub field_06: u64, pub field_0e: u64, pub field_16: u16, pub field_18: u64 }
impl HeaderExtras {
    pub fn activity_host(ah_id: u64) -> Self;        // class 3   [C]
    pub fn class6_activity_host(ah_id: u64) -> Self; // class 6   [I]
}
impl BapConversation {
    pub fn push_with_header(&self, msg_type: u16, sequence: u32,
                            extras: &HeaderExtras, body: &[u8]) -> Result<Vec<u8>, PushError>;
}
```

`push`/`push_with_sequence` keep their signatures (all-zero extras); `can_write_header` accepts
all eight classes; `PushError::UnwritableHeader` now means only "unknown message id".
`cargo test -p d1-bap --lib conversation::` = 22 tests; three injected mutations (class-3 AH id
little-endian, class-6 `+0x16` swapped like class 5, `field_18` little-endian) were each caught.

## 4. The handshake state machine `[C]`

State names live in a relocated pointer array at `0x1953BE0` (slots read as zero in the file
image; resolve via `DT_SCE_RELA`):

| n | name | n | name |
|---|---|---|---|
| 0 | `_none` | 5 | `_authenticating` |
| 1 | `_acquiring_server` | 6 | `_setup_primary_register_subscriber` |
| 2 | `_connecting` | 7 | `_setup_primary_register_relay_client` |
| 3 | `_listening` | 8 | `_connected` |
| 4 | `_channel_starting` | 9 | `_dead` |

`change_state(conn, n)` = `0xB2B690` (dispatches the `enter` handler via `0xB305F0`, stores `n`
into `conn[0x1144]`). Enter table `0xB306B4` (7 entries), update table `0xB2B81C` (8 entries),
both indexed by `n - 1`. Cross-checked five ways (`update[3] → change_state(5)`,
`update[4] → change_state(6)`, `update[5] → change_state(7)`, `update[6] → change_state(8)`,
`0xB2C65F → change_state(9)`). `[C]`

| n | state | enter | update | on success |
|---|---|---|---|---|
| 4 | `_channel_starting` | `0xB306E0` | `0xB2BD40` | → 5 |
| 5 | `_authenticating` | `0xB30840` | `0xB2BDE0` | → 6 |
| 6 | `_setup_primary_register_subscriber` | `0xB3066D` | `0xB2BEC0` | → 7 (sets `conn[0x2538]=1`) |
| 7 | `_setup_primary_register_relay_client` | `0xB30687` | `0xB2BF60` | → 8 `_connected` |

### 4.1 `enter(_channel_starting)` - the `0x1E` hello `[C]`

`0xB306E0` → `0xB316C0`: sets `slot[0x24] = 5` (request kind), `slot[0x28] = 0x1E`, builds a
128-byte payload: `*(u64*)buf = 0x01594E4954534544` (`"DESTINY" 0x01`, movabs at `0xB31748`),
then words 4..63 (bytes 8..127) from the Numerical-Recipes LCG `s = s*0x19660D + 0x3C6EF35F`
(seeded per-run from `rand32()` at `0x911D0`), each word `s >> 16`. Both captures show it in the
clear; the two runs differ from byte 8 onward. `[C]` disassembly and wire agree.

### 4.2 `update(_channel_starting)` - no crypto, no payload read `[C]`

`0xB2BD40` (158 bytes, quoted whole in the source): `update_sending`, `update_receiving`, then
the entire exit condition is `protocol_source_status(conn+0x90) == 2` (`0xB31CA0`; 2 = response
decoded), `protocol_source_reset` (`0xB2ACA0`), `change_state(conn, 5)`. **It stores nothing
from the response.** The status lives at `slot[+0x14]` of the 24-byte protocol-source request
slot (`+0x00` kind, `+0x04` type, `+0x14` status) and is written 2 at `0xB31FA7`.

### 4.3 The `0x1F` response frame - 8-byte header, status 200, 128-byte payload `[C]`

The `0x1E → 0x1F` pairing is `[C]` **by direct table read**: the request-kind → reply-type
table at `0x15D1B40`, indexed by `kind - 1`, five u32 entries:

| kind | reply type | meaning |
|---|---|---|
| 1 | `0x1A` | login / secure-hello response |
| 2 | `0xFB` | echo rsp |
| 3 | `0x7A` | queuez register response (`[I]` name via §6; the id itself is table-read `[C]`) |
| 4 | `0x12F` | register relay client rsp |
| 5 | **`0x1F`** | channel-startup response (the kind `0x1E` registers, `0xB3172C`) |

`0x1F` is header class 2 (`c_bap_message_client_response_header`), whose decode `0x55C880` →
`0x55C9F2` copies 8 bytes, runs the base decode, then `rol word [dst+6], 8` - so the layout is
**`u16be type` ‖ `u32be sequence` ‖ `u16be status`**. `[C]` In the protocol-source response
handler (`0xB31E83`..`0xB31EFF`), if the header class's flags carry bit `0x10` the status is read
via `vtable[0x78]` and compared: `0xB31EF9: cmp r12w, 0xc8` - **status must be `0x00C8` = 200**,
else `"protocol_source: protocol response status failed %s, status: %s"`. `[C]`

`channel_startup_response::decode_payload` = `0x561910`: `if (payload_size != 0x80) return
false` (`0x561937`), `if (cap <= 0x7f) fail`, then `memcpy(dst, src, 0x80)` (`0x561A53`). **No
magic, no `"DESTINY"` check - a length gate and a memcpy.** `[C]` The 128 bytes land in a stack
buffer (`rbp-0xA30`, cap `0x800`) that nothing reads afterwards on this path, so the content is
discarded by this build - `[I]`, read not traced. Echoing the client's own 128 hello bytes back
is the recommended content (`[I]` on the choice).

**The frame to send:**

```
envelope   01 02 00 00 00 88            # version 1, mode 2, payload_size = 0x88 = 136
header     00 1F 00 00 00 00 00 C8      # u16be type=0x1f · u32be seq=0 (echoed) · u16be status=200
payload    <128 bytes, opaque>          # total frame = 6 + 8 + 128 = 142 bytes
```

Three load-bearing literals: header size 8 (`0x55D580: mov eax,8; ret`), status `0x00C8`
(`0xB31EF9`), payload size `0x80` (`0x561937`). A 6-byte header + no payload parks the client
silently in the header read (§1's resumable-read stall) - that exact stall was observed in
`runs/20260806-154837-bap-startrsp`.

The run is self-diagnosing: a wrong type produces `failed to find recipient for header`; a right
type with a wrong body produces `"Failed to read payload [type %s][size %d][offset %d]"` or
`"failed to decode response type %s"`.

### 4.4 `enter(_authenticating)` - the `0x19` login request `[C]`

`0xB30840` calls `generate_login_request` (`0xB317F0`) with OUT pointers `conn+0x2539` (16-byte
AES key) and `conn+0x2549` (16-byte HMAC key); failure logs `"Failed to generate login request"`
/ `_connection_failure_failed_to_generate_login_message`. `0xB317F0` sets `slot[0x24] = 1`,
`slot[0x28] = 0x19`, plus a small discriminator from the table `{1,2,3,3}` at `0x167C1FC`
indexed by `conn[8]`, and fills the body from
`0x2AB8E0(g_signon_client /*0x263D7A0*/, buf, 0x200, &len, key1, key2)` (a
`networking:signon:client:` routine; its own error string
`"networking:signon:client: Token size %d is too large for buffer (%d)"` at `0x1616370`):

```c
memcpy(buf, signon->token, n);
*out_len = signon[0x4030];                        // token length
*(u64*)(key1+0) = *(u64*)(signon + 0x41bf);       // 16-byte blob #1  -> AES key
*(u64*)(key1+8) = *(u64*)(signon + 0x41c7);
*(u64*)(key2+0) = *(u64*)(signon + 0x41cf);       // 16-byte blob #2  -> HMAC key
*(u64*)(key2+8) = *(u64*)(signon + 0x41d7);
```

**The login request carries the signon security token verbatim, and both channel keys are two
adjacent 16-byte blobs from the signon response.** `[C]` for the copy; `[I]` that they are signon
protobuf `f2.2`/`f2.3` and the token `f2.4` (the consumer was traced, not the parser). A
displacement scan of the entire `.text` for `+0x2539`/`+0x2549` finds exactly two sites each:
`0xB3087C`/`0xB30883` (here) and `0xB30DE5`/`0xB30DEC` (the response decryptor). **Because we
author both blobs (currently zeros), the seal on the reply is fully reproducible by us - there is
no RSA wall here.** `[C]` mechanism. The client-side `0x19` encoder is
`secure_hello_request::decode` = `0x5612E0`; its payload schema is `[U]` (only needed if the
login stalls).

### 4.5 The sealed `0x1A` response `[C]`

`update(_authenticating)` (`0xB2BDE0` → `0xB30D90`): same status==2 gate, then
`ctx_init` (`0xB32F10`), `ctx_set_keys(&ctx, conn+0x2539, conn+0x2549)` (`0xB32F40`, 16 B each),
`unseal` (`0xB32FB0`) - failure logs `"Received unreadable hello response"`, fatal `0x25`
(`_connection_failure_failed_to_establish_secure_channel`) - then
`install_channel_key(conn+0x1EB8, msg+0x0C, msg)` (`0xB32D50`), OOM logs
`"Ran out of memory setting the encryption key"`.

**Sealed body layout `[C]`:** `u32be sealed_len ‖ iv[16] ‖ ct[C] ‖ hmac[32]`, with
`sealed_len = C + 48` (the length field does not count itself). HMAC input is
`sealed_len_be ‖ iv ‖ ct` - the length IS covered (mac data starts at `buf+0`, length `L-28` =
`4+16+C`). Cipher/MAC: **libtomcrypt AES-128-CBC + HMAC-SHA256, PKCS#7 padding** - identified
from libtomcrypt descriptor structs `0x19F8D38` (`{name="aes", ID=6, min_key=0x10,
max_key=0x20, block=0x10, rounds=0x0a}`) and `0x19F9060` (`{name="sha256", ID=0,
hashsize=0x20, blocksize=0x40}`), and from the call shapes: `cbc_start` `0x6C664`,
`cbc_decrypt` `0x6C67F`, `cbc_done` `0x6C68F`, `hmac_init/process/done`
`0x6D519`/`0x6D52E`/`0x6D54C` + 32-byte memcmp; key length pinned by
`0xB32F4D: mov edx, 0x10` → `0x6C530`. PKCS#7 pinned by the decoder rejecting `pad == 0`,
`pad > 0x10`, `pad > ctlen`, and checking every trailing pad byte. `[C]`

Decoder detail: `out = buf + 0` - **the plaintext is written back over the header**, which is
why the caller reads the session key at `msg + 0x0C` = plaintext + 12. So plaintext `[0:12]` is
channel state and `[12:...]` is key material; the 16-byte session-key length is `[I]`
(`0x6CF80`/`0x6CC60` unopened).

**Frame geometry is uniquely determined `[C]` literals, `[I]` arithmetic:** the `0x1A` reply
uses the same 8-byte class-2 header with status `0x00C8` (its `vfunc[8]` is also `0x59710` →
class 2), and `secure_hello_response::decode_payload` (`0x561AE0`) imposes:

```c
L = movbe u32 [src];
if (L != srclen - 4)  fail;        // 0x561b11 - payload is exactly 4 + L
if (L <  0x5d)        fail;        // 0x561b16 - sealed_length >= 93
if (cap < srclen)     fail;        // 0x561b97; caller passes cap = 0x64 = 100 (0xb31f38)
```

With `L = C + 48` and C a multiple of 16: `L >= 93` ⇒ `C = 48`; `L + 4 <= 100` ⇒ `L = 96`.
**Payload exactly 100 bytes, `sealed_length = 96`, ciphertext exactly 48 bytes, plaintext 32-47
bytes PKCS#7-padded to 48. Total frame `6 + 8 + 100 = 114`.** What occupies plaintext bytes
28-47 is `[U]`. (The earlier 96-byte plan - 28-byte plaintext, `L = 80` - is rejected twice by
these gates; see §10.)

### 4.6 After the key install

The natural reading is that the connection switches to **envelope mode 1** (the 16-byte
signature path at `0xB2DC50`), with the secured receive states of §5 doing GCM decrypt. How the
plaintext `[0:12]` channel counter advances is `[U]`. Note the receive-probe work (§5) observed
the secured path as **GCM** (`0xB32C90` verify/decrypt), while the `0x1A` body seal is CBC+HMAC;
these are different layers (hello body seal vs per-frame channel crypto), not a contradiction,
but the channel cipher identification rests on the probe doc's reading and is `[U]` at the
instruction level here.

Failure-reason enum (`0x167A234` onward, one string per index): `0x0C`
envelope_version_mismatch · `0x0E` unknown_envelope_type / unknown_message_type · `0x0F`
failed_to_decode_bap_header · `0x25` failed_to_establish_secure_channel (index `[I]`, counted
from the string block; the numeric literals are `[C]` from code).

## 5. The receive state machine and the probe contract

Status of the probe kit: installed on the execution box and build-verified; runtime acceptance
run not performed. Static result 2026-08-07: **38/38 probes passed** against the pinned eboot
and tracer source.

### 5.1 The receive-state split `[C]`

The 11-site ladder at `0xB2E0DC`..`0xB2E47B` is **not** the secured receive path - it is the
state-4/pre-channel recipient path (which is why it fired for plaintext types `0x001F`/`0x001A`
and for no mode-1 traffic; see §10):

```text
BAP receive state machine 0xB2D500

state 2  0xB2DC50  signature/decrypt setup
state 3  0xB2DEC0  unsealed header read
state 4  0xB2E020  pre-channel recipient selection   <- old 11 probes
state 5  0xB2E4C0  unsealed payload

mode 1 skips states 3-5

state 6  0xB2E660  GCM decrypt + inner-header recovery
state 7  0xB2E940  secured recipient selection        <- secured ladder
```

An absence from `0xB2E0DC` after mode 1 is expected and says nothing about secured QueueZ
delivery.

### 5.2 Secured-path ladder

| Probe | Address | Direct observation |
|---|---:|---|
| `secured_decrypt_entry` | `0xB2E660` | mode-1-only state-6 entry |
| `secured_decrypt_result` | `0xB2E71C` | `AL` from GCM verify/decrypt `0xB32C90` |
| `secured_decrypt_success` | `0xB2E724` | success-only; `R13` begins with the 6-byte plaintext inner header |
| `secured_decrypt_rearm_pump` | `0xB2E801` | common normal return; re-arms the group |

The success record carries plaintext `u16be type || u32be sequence` - the required same-run
guest-side control. **If the responder wrote a QueueZ `0x007B` frame and the trace lacks all
mode-1 decrypt records, that is invalid instrumentation, not evidence the game ignored it.**

Secured recipient selection:

```text
0xB2E940  secured recipient-selection entry
    +-- scan two in-flight queues
    |     0xB2E9C9  expected type returned by live object vtable+0x18
    |     0xB2E9D5  type matched; load stored sequence
    |     0xB2EB86  type and sequence matched; recipient kind = request
    |     0xB2EBAC  call request helper 0xB2F270
    +-- push fallback
    |     0xB2EA10  AL from push-handler vtable+0 type acceptance
    |     0xB2EA14  push accepted; recipient kind = push
    |     0xB2EA30  call push helper 0xB2F550
    |     0xB2EA35  AL preserved on the jump to the common verdict
    +-- 0xB2EA3A  no request or push recipient accepted the type

0xB2EBB1  common AL consumer verdict after 0xB2F270 or 0xB2F550
0xB2EBB5  success-only cleanup
0xB2ED20  common normal return/re-arm pump
```

The live expected-type probe is the authoritative way to identify a response. **Adjacent
message-number guesses are prohibited** (see §10 for the refuted ones). The client policy
confirmed `0x000D` is the numeric response to `0x000C`, but its handler is a **no-op**.

### 5.3 QueueZ acceptance-to-population ladder

The QueueZ push consumer is `0x565140` (body content and replication semantics: see
design/queuez-replication.md; this section keeps only the transport-side probe chain):

```text
0x565140  QueueZ update-notification consumer
0x5653CE  call 0x56AF60: begin/apply family update
0x5653D3  test AL                         family accepted/refused
0x5653D7  load object_count
0x5653E5  accepted AND object_count > 0  dispatch list to 0x5655B0

0x56B5F0  apply one object
0x56B7CD  call 0x56BAB0                  find/allocate registry slot
0x56B7D2  RAX -> R13                     slot result; NULL is a named failure
0x56B833  call 0x2C9000                  RSAT decode directly into slot
0x56B838  decode returned                caller does not branch on AL
0x56B83B  call 0x72650                   finalize stream
0x56B844  compare decoded [R13] key with expected wire SOID
0x56B8B0  key-mismatch branch
0x56B881  changed existing object -> lifecycle update 0x56D3F0
0x56B968  new valid object -> lifecycle added 0x56D290
0x56B753  common object-apply return/re-arm pump
```

Captured values: family, root, flags, object count, the wire object record `{u32 instance,
unaligned u64 SOID, u32 RSAT length}`, registry slot, decoded key, expected SOID. This
distinguishes in one run: family refused / accepted with no objects / list dispatched but apply
not entered / slot allocation failure / decode reached but key failed / existing unchanged /
existing updated / new added. The old `class4_added` probe at `0x56D306` is only one class-4
lifecycle subtype; `0x56B881`/`0x56B968` are the general path for every QueueZ family/type.

These boundaries are load-bearing: a BAP push can be consumed while its family update is
refused; a family update can be accepted with `object_count == 0`; an object can be decoded
into a slot but rejected before the lifecycle callback. None substitutes for another.

### 5.4 Rearm, guards, overlap safety

Five independent mutual-rearm groups (pre-channel receive; secured decrypt; secured recipient
selection; QueueZ push/family acceptance; QueueZ per-object application). Each entry arms its
downstream common-return pump; each pump re-arms the entry and upstream decision sites; no
probe re-arms itself. Global record budget 60,000; budget exhaustion is a confounded run. Every
site has a byte guard against the pinned eboot; the verifier requires probe ID/address/guard
parity with the runtime tracer and rejects overlap with every active patch range (this parity
check caught a real integration error: catalog logical IDs 1-11 vs tracer 31-41).

```text
python3 -B capture-rig/verify-client-bap-probe-sites.py \
  dumps/patch-01.29-CUSA00219/eboot.bin \
  --catalog capture-rig/client-bap-probe-sites.json \
  --tracer-source ~/dev/shadPS4-d1trace/src/core/d1_account_trace.cpp \
  --patch-range START:END
```

Negative control: `--patch-range 0xB2EA10:0xB2EA12` correctly aborts on
`secured_push_type_result`.

### 5.5 Runtime acceptance criteria

A QueueZ push is end-to-end accepted only when ONE same-run trace shows all of:

1. `secured_decrypt_entry`, successful `secured_decrypt_result`, `secured_decrypt_success` for
   plaintext type `0x007B` and its sequence;
2. `secured_push_type_result(AL=1)`, `secured_push_matched`, successful common consumer result;
3. `queuez_family_update_result(AL=1)` with the expected family/root;
4. `queuez_family_objects_dispatch` with `object_count > 0`;
5. `queuez_object_apply_entry` per advertised object;
6. non-NULL `queuez_object_slot_result`;
7. `queuez_rsat_decode_returned` and matching decoded/wire key;
8. `queuez_object_added` or `queuez_object_updated`, then the named game-state predicate.

No disconnect, bootflow transition or descriptor snapshot substitutes for those stages.

## 6. The message id table - all 58 ids

Generated by `capture-rig/bap-message-table.py` (9 controls, exit 2 on failure); full rows in
`capture-rig/bap-message-table.tsv`. Reach: `static-client-inference`, cross-checked against
`observed-live` - 18 of the 58 ids are constants our server already puts on the wire
(marked ✅live below), and all 18 reproduce. **This is the control: a static reading checked
against observed bytes, not against itself.**

### 6.1 Where the id lives `[C]`

Every BAP message is a `c_investment_bap_message_*` C++ class. **The message id is vtable slot
0**, a one-instruction constant thunk:

| slot | contents |
|---|---|
| `+0x00` | `mov eax, <MESSAGE ID> ; ret` |
| `+0x08` | `mov eax, <CATEGORY> ; ret` = the header-class index (§3.1) |
| `+0x48` | pointer to a C string, the message's own name |
| `+0x70` | on a request: `mov eax, <its RESPONSE's id>` ⚠ means something else on a response |
| `+0x78` | the paired category |

A registry of all 58 message vtables sits at `0x1922110` (58 entries, `0x1D0` bytes). Registry
order is not id order (echo is entry 4, id `0xFA`); the registry gives membership, the vtable
gives the id.

### 6.2 The activity-host set

Ours to implement (they cross the client's wire): **6** `client_to_activity_host_manager_request`
(client → us), **7** `..._response` (us → client), **8** `client_to_activity_host_notification`
(client → us), **16**/`0x10` `client_to_bap_get_activity_host_proxy_request`, **17**/`0x11` its
response, **100**/`0x64` `activity_host_to_client_notification` (⚠ a relay hop, §7), **102**/`0x66`
`activity_host_to_client_log_notification`.

Bungie-internal, evidence of the architecture, NOT ours to implement: 150/`0x96`
`bap_to_activity_host_manager_request`, 151/`0x97` its response, **153**/`0x99`
`bap_to_activity_host_socket_handoff_request` (BAP hands the AH a socket), 160/`0xA0`
`bap_to_activity_host_notification`, 110/112 `service_to_world_server_request` /
`activity_host_to_world_server_response`.

### 6.3 The full table

| id | hex | category | rsp | class |
|---:|---|---|---|---|
| 6 | 0x06 | request | 0x7 | `client_to_activity_host_manager_request` |
| 7 | 0x07 | response | - | `client_to_activity_host_manager_response` |
| 8 | 0x08 | notification | - | `client_to_activity_host_notification` |
| 9 | 0x09 | (computed) | - | `bap_to_client_notification` |
| 10 | 0x0a | request | 0xb | `client_to_world_server_request` |
| 11 | 0x0b | response | - | `client_to_world_server_response` |
| 12 | 0x0c | request | 0xd | `client_to_bap_subscription_request` |
| 13 | 0x0d | response | - | `client_to_bap_subscription_response` |
| 14 | 0x0e | request | 0xf | `client_to_bap_unsubscribe_request` |
| 15 | 0x0f | response | - | `client_to_bap_unsubscribe_response` |
| 16 | 0x10 | request | 0x11 | `client_to_bap_get_activity_host_proxy_request` |
| 17 | 0x11 | response | - | `client_to_bap_get_activity_host_proxy_response` |
| 18 | 0x12 | request | 0x13 | `client_to_bap_client_config_request` ✅live |
| 19 | 0x13 | response | - | `client_to_bap_client_config_response` ✅live |
| 20 | 0x14 | (computed) | - | `bap_to_client_server_shutdown_notification` |
| 21 | 0x15 | request | 0x16 | `client_to_bap_update_purchased_offers_request` ✅live |
| 22 | 0x16 | response | - | `client_to_bap_update_purchased_offers_response` |
| 23 | 0x17 | request | 0x18 | `client_to_bap_account_id_translation_request` ✅live |
| 24 | 0x18 | response | - | `client_to_bap_account_id_translation_response` ✅live |
| 26 | 0x1a | response | - | `client_to_bap_secure_hello_response` ✅live |
| 27 | 0x1b | (computed) | - | `bap_to_client_log_notification` |
| 30 | 0x1e | request | 0x1f | `client_to_bap_channel_startup_request` ✅live |
| 31 | 0x1f | response | - | `client_to_bap_channel_startup_response` ✅live |
| 32 | 0x20 | request | 0x21 | `bap_to_client_user_message_request` ✅live |
| 33 | 0x21 | response | - | `bap_to_client_user_message_response` ✅live |
| 34 | 0x22 | request | 0x23 | `client_to_bap_update_matchmaking_skill_request` |
| 35 | 0x23 | response | - | `client_to_bap_update_matchmaking_skill_response` |
| 36 | 0x24 | request | 0x25 | `client_to_bap_friends_update_request` |
| 37 | 0x25 | response | - | `client_to_bap_friends_generic_response` |
| 38 | 0x26 | request | 0x27 | `client_to_bap_party_invite_request` |
| 39 | 0x27 | response | - | `client_to_bap_party_invite_response` |
| 40 | 0x28 | request | 0x29 | `client_to_bap_check_party_invite_request` |
| 41 | 0x29 | response | - | `client_to_bap_check_party_invite_response` |
| 42 | 0x2a | request | 0x2b | `client_to_bap_update_matchmaking_core_request` |
| 43 | 0x2b | response | - | `client_to_bap_update_matchmaking_core_response` |
| 44 | 0x2c | request | 0x2d | `client_to_bap_clan_request` |
| 45 | 0x2d | response | - | `client_to_bap_clan_response` |
| 46 | 0x2e | request | 0x2f | `client_to_bap_nat_punch_request` |
| 47 | 0x2f | cat 4 | - | `bap_to_client_nat_punch_relay_intro_notification` |
| 100 | 0x64 | cat 6 | - | `activity_host_to_client_notification` |
| 102 | 0x66 | cat 5 | - | `activity_host_to_client_log_notification` |
| 110 | 0x6e | cat 5 | - | `service_to_world_server_request` |
| 112 | 0x70 | cat 7 | - | `activity_host_to_world_server_response` |
| 121 | 0x79 | request | 0x7a | `client_to_bap_queuez_register_request` ✅live |
| 122 | 0x7a | response | - | `client_to_bap_queuez_register_response` ✅live |
| 123 | 0x7b | cat 4 | - | `queuez_to_client_update_notification` |
| 124 | 0x7c | cat 4 | - | `queuez_to_client_subscription_lost_notification` ✅live |
| 150 | 0x96 | cat 5 | - | `bap_to_activity_host_manager_request` |
| 151 | 0x97 | cat 7 | - | `bap_to_activity_host_manager_response` |
| 153 | 0x99 | cat 6 | - | `bap_to_activity_host_socket_handoff_request` |
| 160 | 0xa0 | cat 6 | - | `bap_to_activity_host_notification` |
| 171 | 0xab | request | - | `client_to_xetrov_request` ✅live |
| 250 | 0xfa | request | 0xfb | `client_to_bap_echo_request` ✅live |
| 251 | 0xfb | response | - | `client_to_bap_echo_response` ✅live |
| 300 | 0x12c | request | 0x12d | `client_to_bap_initiate_relay_connection_notification` |
| 301 | 0x12d | cat 4 | - | `bap_to_client_request_relay_connection_notification` |
| 302 | 0x12e | request | 0x12f | `client_to_bap_register_relay_client_request` ✅live |
| 303 | 0x12f | response | - | `client_to_bap_register_relay_client_response` ✅live |

Message 9 is header class 0 (`d1-bap` `MESSAGE_CLASS` row `(9, 0, "bap_to_client_notification")`,
`HEADER_LEN[0] = 6`). `[C]` - and made falsifiable by the pinned live frame test (§7.5).

The handshake-era string enum (jump table `0xB32714`, `0xB326F4` for `0x12C`..`0x12F`) carries
the shipped short names (`c->b start req`, `b->c shello rsp`, ...). ⚠ Its `b->c`/`c->b`
prefixes are NOT reliable direction markers - `0x19`/`0x1A` are labelled `b->c` yet the client
originates `0x19`. Treat them as opaque labels. That enum also lumped `0x1B`-`0x1D` under
`b->c log not`, whereas the generated table has `0x1B` as `bap_to_client_log_notification` and
`0x1C`/`0x1D` handled outside the class system (§6.4) - prefer the generated table.

### 6.4 Ids on the wire with no message class

`0x19` `SECURE_HELLO_REQUEST` and `0x1C`/`0x1D` `SERVER_CHALLENGE`/`_ANSWER` are handled outside
the message-class system - they are the transport handshake, below this layer. Their absence
from the registry is expected and is itself a consistency check.

⚠ Naming debt: `0x000C`/`0x000D` are `client_to_bap_subscription_request`/`_response` here,
while `d1-bap` also aliases `0x000C` as `TYPE_QUEUEZ_SUBSCRIBE` / `TYPE_FAMILY_SUBSCRIBE_REQUEST`
and `0x000D` as `TYPE_FAMILY_SUBSCRIBE_RESPONSE`. Same ids, three names in our code; reconcile
before adding anything new.

⛔ **An id is not a schema.** Nothing in this table says what is inside message 6 or 16. Body
decoders located: `get_activity_host_proxy_request` `0x562B90`, `_response` `0x562D60`,
`activity_host_manager_request` `0x55AC00`, `client_to_activity_host_manager_response`
`0x55AF10`, BAP AH address resolver `0x594E70`. (Body schemas: design/activity-host.md.)

## 7. Push delivery and the establish-client routing - message 9 vs 100

Reach: `static-client-inference` for the chain (eight client functions read end to end),
`observed-live` for the decisive measurement (the client's own relay, captured six times in
`runs/20260808-234535-CODEC-LIVE2`). Everything the server emits stays `synthetic_ancestor`.

> ⭐⭐ **BAP message 100 `activity_host_to_client_notification` is not a client-facing message.**
> Its push policy's deliver step reads the class-6 header's `+0x06` u64 as a **connection
> index** and re-emits the identical body as **BAP message 9 `bap_to_client_notification`** on
> that connection. **Message 9 is what the client consumes.** In Bungie's design the client and
> the Activity Host never talk directly:
>
> | hop | id | class |
> |---|---:|---|
> | client → BAP | 8 | `client_to_activity_host_notification` |
> | BAP → AH | 160 | `bap_to_activity_host_notification` |
> | AH → BAP | 100 | `activity_host_to_client_notification` |
> | **BAP → client** | **9** | **`bap_to_client_notification`** |
>
> We push directly, so we must send **9**. `d1-activity-host` does, by default
> (`D1_AH_BAP_PUSH_ID`, default 9; `D1_AH_BAP_PUSH_ID=100` reproduces the failure).

### 7.1 Receive and enqueue - `0x563590` `[C]`

`c_investment_bap_message_push_handler::vfunc[2]` is the receive dispatch for every push:

```c
id     = hdr->msg_type;
policy = FUN_00564100(id);          // push policy, linear scan of the 13-entry set at 0x1925A90
class  = FUN_00559DD0(id);          // message class, (&DAT_026A0DB0)[id]
if (policy->vfunc[1]() && policy->vfunc[2]()) { class->vfunc[3](decode...); policy->vfunc[3](...); }
if (payload_len < 0x20001) {        // 131,072 ceiling
    memcpy(elem + 8, hdr, 0x84);    // the 132-byte header struct
    elem[0x12] = payload_len;
    memcpy(elem + 0x13, payload);
    FUN_00BAB380(elem);             // INSERT INTO THE SIMULATION QUEUE
}
```

- For any id on the accept-set the enqueue is unconditional. Because class 6's flags lack bit
  `0x10` (§3.3), no `0xC8` status is ever demanded on a message-100 frame.
- `push_policy vfunc[1]`: message 100 = `0x14D0` (`xor al,al` = false, body NOT decoded at
  receive); message 9 and message 123 `queuez_to_client_update_notification` = `0x1F70`
  (`mov al,1` = true, decoded). Same slot, opposite constants, opposite behaviour - 123 is the
  positive control on the slot reading.
- The one loud drop: payload ≥ `0x20001` logs `"Received a push payload that is larger than we
  can insert in simulation queue."` Largest observed push is 7,297 B, so it is latent.
- Drain: `handle_message_internal` (`0x563170`) decodes (`class->vfunc[3]`, failure
  `"Failed to decode push payload [type %s][size %d]."`) then calls **`policy->vfunc[5]` - the
  deliver step** (failure `"Failed to handle decoded push payload [type %s][size %d]."`).
- Nothing gates on element order in the queue itself - but the router does (§7.4).

### 7.2 The two deliver steps `[C]`

| | push-policy vtable | slot 1 | slot 5 (deliver) |
|---|---|---|---|
| 9 `bap_to_client_notification` | `0x1925B10` | `0x1F70` true | `0x5642E0` |
| 100 `activity_host_to_client_notification` | `0x1925DB0` | `0x14D0` false | `0x564C20` |

**Message 100's deliver step is a RELAY** (`0x564C20`):

```c
class  = FUN_00559DD0(hdr->msg_type);
hdrcod = FUN_0055C1E0(class->vfunc[1]());       // header codec for class 6
idx    = hdrcod->vfunc[5](hdr);                 // 0x55D1D0 = mov rax,[rsi+6]; ret
if (FUN_00558AA0(idx) == 3)                     // that connection is ESTABLISHED
     FUN_005598F0(idx, 9, out_hdr, payload, 0x1DF11, seq);   // policy 9 = MESSAGE 9
return 1;                                       // ALWAYS
```

Three independent confirmations that `idx` is a connection index: (1) `0x55D1D0` is class-6
slot 5, the get half of the `+0x06` pair; (2) `FUN_00558AA0(idx)` indexes
`DAT_026A0CE8 + 8 + idx*0x18` - the same table `FUN_005598F0`'s first argument indexes - and
returns 3 from `c_network_server_bap_connection::vfunc[3]`; (3)
`d1_activity_host::protocol::CONNECTION_SLOT_NAMES` = `["OUT PRIMARY", "OUT FAH", "OUT GAH0",
"OUT GAH1"]` - we left `+0x06` at zero, index 0 is `OUT PRIMARY`, and every push came back to
`d1-responder`, captured 1:1 (four pushes → four inbound message-9 frames, seqs 462-465; two
pushes → two frames, seqs 58-59; each carrying
`[u8 1][u64be 0xD1A0000000000001][u32be udt][u32be len][payload]` with OUR AH id and OUR
payload bytes). `observed-live`. `return 1` unconditionally: **a message 100 that forwards
nowhere is silent in both directions.**

**Message 9's deliver step is the real one** (`0x5642E0`):

```c
if (body[0] == 1) return FUN_0075CB40(body);    // -> mgr = FUN_007669A0(); FUN_0075CB70(mgr, body)
else log "push_policy_bap_to_client_notification: unknown client notification type %s"
```

### 7.3 The recipient match - `recipient_id` IS the activity-host id `[C]`

`FUN_0075CB70`:

```c
slot = FUN_0075AB60(mgr, *(u64*)(body+1), *(i32*)(body+9) == 4);
//                        ^ recipient_id        ^ udt 4 takes a different branch
if (slot != -1) {
    if (!FUN_00594960(mgr + 0x640 + slot*0x1F0, recipient_id, body))
        log "world_controller:activity_manager: Failed to handle activity host notification "
            "[msg type %d][AH %s], activity manager slot=%s";
}
return true;                       // slot == -1 falls straight through, NO diagnostic
```

`FUN_0075AB60` walks the **three** activity-manager slots at `mgr + 0x5B8 + i*0x1F0` and
compares the 8 bytes with `FUN_000685A0`:

| branch | compared against |
|---|---|
| `udt != 4` | `c_activity_client::vfunc[2]` = `0x2992A0` = `[ac + 0xC98]`, the ESTABLISHED activity-host id; requires the `EST` bit (`vfunc[3]`) first |
| `udt == 4` | the same, PLUS a pre-establish branch: when `EST` is clear but `ac+0x130 & 8` is set (join request sent), it compares `FUN_0029B230(ac) + 8` - the AH id of the pending join request |

Delivery (`FUN_00594960(slot, ah_id, body)`): if `*(u32*)(body+9) < 0x31` (udt < 49) and a
recipient is installed at `slot->[8]`, call `slot->[8]->vfunc[2]()` = `0x2996C0`, a
two-instruction thunk (`add rdi,-8; jmp 0x299430`) into
`c_activity_client::receive_message_from_host`. The message-7 response path (`0x594C60`)
reaches the same terminus, which is why the udt-10 startup response worked while pushes did not.

**Body codec shared by 9 and 100** (`0x55E690` encode, `0x55E710` → `0x55E130` decode; classes:
9 typeinfo `0x1923850` vtable `0x1923810` slot 0 `0x55E670 mov eax,9`; 100 typeinfo `0x19237E0`
vtable `0x19237A0` slot 0 `0x55E0F0 mov eax,0x64`; slots 2/3/4/6 identical:
`0x55E690`/`0x55E710`/`0x559D30`/`0x55E750`):

```
+0x00  u8    kind (only 1 exists)
+0x01  u64be recipient_id            (byteswapped in place via 0x2A2BF0)
+0x09  u32be user_defined_type
+0x0D  u32be payload_size
+0x11  payload
```

**The bytes after the header are identical for 9 and 100; the message id is the only variable.**
That is what made the fix one line, not a new codec. `[C]`

Three silent-drop sites, all identical from outside: (1) `0x564C20` returns 1 whether or not it
forwarded; (2) `FUN_0075CB70` returns true when the slot walk returns -1, no diagnostic;
(3) `FUN_00594960` returns 0 (which IS logged) only if a slot was already found.

The failure this diagnosed: `error.starting_private_activity_instance_timed_out_waiting_for_
join_response` ("elderberry") = AH status **26** (`reason_id = ah_status + 0x11`, `26 + 0x11 =
0x2B`), the first wait for `ac+0x130 & 0x10` - NOT status 28
`timed_out_waiting_to_establish_client`. The establish gate `FUN_00298EF0` was never evaluated;
its four message-derived terms (udt 1 `world+0x50A8` via `0x1057B80`; udt 4 `EST` bit
`ac+0x130 & 0x20` set at `0x29C185`; udt 0 `ac+0x64AA` written only at `0x10579F0`; udt 11
`ac+0x131 & 1` set at `0x29C730`) were all false because no handler ran. `[C]` The
gate-flag details live with design/activity-host.md.

### 7.4 ⚠ Order is mandatory AT THE ROUTER

"Arrival-completeness, not order" is true of the establish gate and **false of the router**:
`FUN_0075AB60` matches udt 0, 1 and 11 only through `[ac+0xC98]`, which is zero until udt 4
`join_result` has been accepted (and the `EST` bit set). So **udt 0, 1 and 11 are unroutable -
silently - until the join_result has been applied.** Pushing `4 → 0 → 1 → 11` as four frames on
one socket should satisfy this but is not guaranteed; the discriminator is the status line
(`EST-Y/N`, `AH->`, `MEM-0/1`). If udt 4 lands but 0/1/11 raced ahead, the fallback cue is the
client's own **udt 17 `activity_client_request_state_refresh`**, which `0x29AB80` sends
automatically from inside the accept branch of `0x29BD10` (`[U]` whether it fires - mechanism
identified, never observed).

⚠ Diagnosis asymmetry: once we push 9 instead of 100, the inbound relayed message-9 frames
STOP. Their absence then means success where their presence meant misroute. Read the responder's
startup line naming the push id, not the silence.

### 7.5 What ships

`d1-bap` `tables.rs`: `OURS_TO_PUSH` `[9, 100, 123]`. `activity_host.rs`:
`BapToClientNotification` (one codec, two ids), `BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION`,
`AH_TO_CLIENT_UDT_NAMES`, and the pinned live frame `CAPTURED_MESSAGE_9_FRAME` (the 177-byte
udt-1 frame, seq 464) with a negative: its body must NOT also decode at `+8`, `+14`, `+24` or
`+32` - that is what makes "message 9 is header class 0" falsifiable. `d1-activity-host`
`push.rs`: `push_bap_message_id()` (`D1_AH_BAP_PUSH_ID`, default 9), `emit_as`; `controls.rs` 42
controls. `d1-responder`: inner-`0x0009` handler, decoded and named, fire-and-forget.
`recipient_id` must be the AH id (`state.rs`, `let recipient = self.ah_id;` - correct);
`d1-responder`'s `D1_AH_RECIPIENT` defaulting to the account SOID is wrong and only matters if
`D1_AH_PUSH_100` is turned on (off by default).

Build/verify: whole-workspace `cargo build --release` (never `-p`), `cargo test --release`,
`d1-eboot controls` 22/22, `d1-activity-host controls` 42/42.

## 8. The envelope span - what makes the RSAT response array non-empty

**ANSWERED `[C]` 2026-08-07** (offline: the client's own native code driven under Unicorn plus
static disassembly; no emulator launched). This is the investment `server_message` layer riding
on BAP - the wrapper around every RSAT-encoded response body.

> **The response's array is non-empty if and only if ONE bit - read from the RSAT bitstream
> immediately after the response object's bits - is 1, and the 16-bit byte-length that follows
> is ≥ 1. No schema participates.** The array is a hardcoded wrapper tail in the generic RSAT
> codec; the client's own name for it is "decoded **optional variable payload** would exceed
> out_payload_buffer".

The wire grammar of what `0x261FB0` decodes, in full `[C] static-client-inference` (decoder
`0x261FB0`, mirrored in encoder `0x261BE0`):

```
[16 bits]   message id                    -> 0x27FDD0(id) -> envelope+0x00 (definition INDEX)
[n  bits]   the response object, per schema tag             -> envelope+0x08
[ 1 bit ]   SPAN PRESENT                                    <== THE GATE
[16 bits]   SPAN LENGTH IN BYTES          only if present   -> envelope+0x18
[len*8  ]   SPAN BYTES, verbatim          only if present   -> envelope+0x10
            zero-pad to a byte boundary
```

Derived twice, from opposite directions, without collaboration, byte-for-byte identical
(decoder side: presence bit `0x2626C0 call 0x73430`, length `0x2626D4 mov esi,0x10 / call
0x72C70`, payload `0x2627AE shl edx,3 / call 0x730B0`, id `0x26212E`; encoder side: presence
`0x261E11 test ebx,ebx / setg al` on `[r15+0x18]`, length `0x261E6C mov edx,0x10`, payload
`0x261EA1`/`0x261EA5 shl edx,3 / call 0x72E30`, id `0x261C93`). A pure-Python bit model
reproduces the native encoder byte-for-byte. `[C]` convergence.

### 8.1 Every envelope write site, with its positive control `[C]`

`capture-rig/oodle/span-writesite-audit.py` tracks arg4 into `r15` at `0x261FD0` and exits
non-zero if it fails to find the two writes already known to exist:

```
+0x00  message index : 0x262118 (init 0) · 0x26213a  mov word [r15], ax      <- control, found
+0x08  object ptr    : 0x262110 (init 0) · 0x2624da  mov [r15+8], rdx        <- control, found
+0x10  SPAN PTR      : 0x262108 (init 0) · 0x26278a (empty) · 0x26279f (assign)
+0x18  SPAN COUNT    : 0x262100 (init 0) · 0x2626de (assign) · 0x262792 (empty)
```

All four non-init span writes sit AFTER `call 0x2C9000` (the schema field walker) at `0x2625A9`,
on the single presence-bit path. Call histogram over the whole function: `0x73430` (1 bit) x1,
`0x72C70` (N bits) x2, `0x730B0` (blob) x1, `0x2C9000` (fields) x1, `0x27FDD0` (id→index) x1.
**Therefore no schema - swept or unswept - can populate the span.** A mechanical result, not a
coverage result. `[C]`

### 8.2 The gate downstream, and why `length >= 1` `[C]`

```
0x570033  mov eax, dword ptr [rbx + 0x18]     ; the COUNT, not the pointer
0x570036  test eax, eax
0x570038  jle 0x57006e                        ; count <= 0 -> handle B never built
```

Hand-built bitstreams decoded through the native `0x261FB0` on message 206's own schema
(**SYNTHETIC**):

| wire | span ptr | span count | handle B built? |
|---|---|---|---|
| `00ce10` (what d1-bap shipped) | NULL | 0 | no → crash |
| `00ce180000` (present, length 0) | non-NULL | 0 | no → still crash |
| `00ce18000d50` (present, length 1, payload `aa`) | non-NULL | 1 | yes |
| `00ce10000d50` (same stream, the one bit cleared) | NULL | 0 | no |

The last row is the isolating control. ⚠ `present=1, length=0` is a trap: a non-NULL pointer
that looks like progress in a pointer-oriented probe, while `0x570038` tests the count.

### 8.3 Codec `0x7B` consumes the span - and refuses a count of 0 `[C]`

The span bytes reach codec `0x7B`'s transform `0x568F30` unmodified as a `{ptr, u32 len}` pair
(built at `0x57003A`/`0x57003E`/`0x570042`, stored as handle B via `0x1D3190`, returned by
`0x78970`, read at `0x570AAE`/`0x570AB1`). **The span content must itself be a QueueZ object
stream** (format detail: design/queuez-replication.md):

```
[0:4]  u32be family_count
per family (queuez_family_header_for's tail, 21 bytes):
  +0x00 u32 family · +0x04 u64 root_soid · +0x0C u32 family_version
  +0x10 u8  flag   · +0x11 u32 object_count
per object if object_count != 0 (the 0x569020 branch):
  u32 instance_type · u64 object_soid · u32 payload_len · payload
```

Driven offline through the native transform: count-0 span (`00000000`) → **refused**, with the
client's own diagnostic `"networking:investment:push:queuez: invalid queuez transaction count
%d"` (`0x569069 xor bl,bl`; string `0x162D0C5`); count-1 clean 21-byte record, and all three
real `d1-bap` bodies (`queuez_empty_family_body_for(4, acct)` 25 B,
`queuez_object_body(...)` 50 B, `queuez_character_body(...)` 49 B) → **accepted**, `out_len ==
span len`, every u32/u64 byte-swapped, the u8 flag untouched. **`d1_bap::queuez_object_body` IS
codec `0x7B`'s format** (the earlier apparent mismatch was a 4-byte framing slip: the outer
count had been counted into the first element). `[C]`

⚠ The caller **ignores** the transform's boolean (`0x570AC5` never tests `al`) - a refused span
is silent at the call site; the only signal is the diagnostic string, which the two-gate log
trap suppresses unless patched.

### 8.4 Proposed 206 bodies (SYNTHETIC, verified encode→decode→re-encode fixed point)

The presence bit sits at bit 4 of the body, so **the span must be bit-packed, never
byte-appended** - the u16 length and every span byte straddle byte boundaries. This is why the
append-style `D1_206_TAIL` hook (`d1-responder/src/main.rs:877`, emits `10 || tail`) can never
express the fix; the smallest enabling change is a body-override form (`D1_206_BODY=<hex>`).
Construction: `bits = 0001` (the zero-valued response object of schema `0x80801869`) `+ 1`
(presence) `+ u16be(len(span)) + span bytes MSB-first + zero pad`.

| span | proposed 206 body |
|---|---|
| FIRST SHOT - `queuez_empty_family_body_for(4, acct)`, 25 B | `1800c800000008000000268800000000000008000000000800000000` |
| `queuez_object_body(4, acct, ACCOUNT_INSTANCE_TYPE, acct, acct_rsat)`, 50 B | `1801900000000800000026880000000000000800000000080000000b2573adfe88000000000000080000004f440000000000000400` |
| A/B discriminator only - `00000000` (u32be count 0), 4 B | `18002000000000` |

⚠ These strings embed the account SOID `0xD100000000000001` - they must be GENERATED from the
session's SOID, never pinned. ⚠ `18002000000000` is an A/B step, not a goal state (codec `0x7B`
refuses count 0). ⚠ Never send a span under 4 bytes: `0x568F30` has no minimum-length check and
reads a u32 count out of the caller's uninitialised 64 KB stack buffer.

The change target: `d1_bap::investment_206_response` (`d1-server/crates/d1-bap/src/lib.rs:1101`)
currently passes `&[0x10]` = the 4-bit leaf `0001`, presence bit 0, three pad bits.

Live success signal: `msg 0x003b count=25` in the envelope probe - a non-zero count. **The
absence of a crash is NOT success**: four separate interventions removed the `0x570AAE` crash
while leaving the gate closed.

### 8.5 Ruled out, each with its positive control `[C]`

| ruled out | control |
|---|---|
| No response schema populates the span by itself: all 181 anchored descriptors with a `kind & 0x100` field swept (161 covered, zero self-populated); 402 sampled of 4374 no-array tags (270 covered, zero); all-zero and all-`0xff` raw inputs, zero | per-tag control P2: attaching a 1-byte span to the same tag must return `span_count == 1` - 161/161 array tags, 270/270 controls |
| The 20 uncovered array tags are not a hole: every span write is on the one presence-bit path after the field walker (§8.1) | the same scan finds the `+0x00`/`+0x08` writes; the script exits non-zero otherwise |
| `kind & 0x100` is not a repeated/array marker: `0x101` emits the same 10 bytes for element counts 1, 2 and 5; `0x105` is `0x005`'s stream shifted right by exactly one leading 1 bit | the survey shows length does vary across the low kinds (1/4/8/13/16/32/64-bit leaves distinct) |
| `0x80801869` is not too small to carry a span: lengths 1..4096 round-trip, including 21 bytes at the live arena capacity `0x1FF6A` | over-long inputs fail closed (len 64 with 4 bytes supplied; len `0x7000` @ cap `0x400`; len `0xffff` @ cap `0x400`) |
| The old append-style tail failure reproduces offline: `00ce10` + the exact 25-byte tail → `span_count = 0` | same run: `00ce18...` with the same 25 bytes → `span_count = 25`, bytes intact |
| Our bytes were never malformed: 503's pinned vector reproduces byte-for-byte, its span legitimately `{0,0}` | the oracle's 7-point self-test passes before every sweep |

Other pinned mechanics `[C]`: `0x261FB0`'s callers pass the arena as arg4 (`rcx` → `r15`) and
the bump cursor as arg6; the span lands immediately after the decoded object with NO alignment
padding (503 → arena+0x3E8; 206 → arena+0x21). The 16-bit length is read into an i32 and the
encoder's presence test is signed `setg`; spans above `0x7FFF` untested (`[U]` unsigned read),
the live arena capacity `0x1FF6A` binds first. `envelope+0x00` comes from `0x27FDD0(wire id)`
(`-1` rejected); the table is content-loaded and cannot be evaluated offline, so the
class-id ↔ message mapping (`0x4C`↔503 / `0x3B`↔206) **remains `[U]`** - a stubbed oracle
"confirming" it was circular and does not count.

Offline tooling (all Unicorn, no emulator): `capture-rig/oodle/rsat-oracle.py`,
`sweep-envelope-span.py`, `span-writesite-audit.py`, `span-tag-inventory.py`,
`queuez-codec-oracle.py`, `probe-envelope-span.py`, `rsat-array-encoding.py`,
`rsat-schema-encode-oracle.py`, `rsat-kind-wire-survey.py`, `rsat-encoder-disasm.py`.

Taint: every specific byte string in this section is SYNTHETIC (the client's own native code
executed offline, stronger than a mock but not an observation of the live client); downstream
inherits `synthetic_ancestor` until the live run confirms.

## 9. Open `[U]`s, consolidated

| # | unknown | note / what would close it |
|---|---|---|
| 1 | Class 6 `+0x0E`, `+0x16`; class 5 `+0x06`, `+0x0E`, `+0x16` | a run or retail capture; ⚠ secured BAP is encrypted, a passive tap cannot; nothing sends class 5 to the client (102 is not on the push accept-set) |
| 2 | Whether class 6 `+0x18` really is the activity-host id (`[I]`) | one push with `class6_activity_host(ah_id)` vs one with `default()`, compared on the guest log |
| 3 | What validates the body's `recipient_id` downstream of the simulation queue | §7 shows it is not the BAP layer; the queue-element consumer is unread |
| 4 | `0x1A` plaintext bytes 28-47 | the plaintext must pad to 48 ct bytes; `[0:12]` channel state and `[12:28]` session key (`[I]`) are read, the rest is not |
| 5 | Whether the channel switches to envelope mode 1 after `install_channel_key`, and how the `[0:12]` counter advances | read `0x6CF80` / `0x6CC60` and the mode-1 branch; §5's state 6 (GCM) is the probe-side view |
| 6 | The `0x0019` login request's own payload schema (`0x5612E0`) | only needed if the login stalls |
| 7 | Whether the 128 channel-startup payload bytes are genuinely ignored (`[I]`) | read, not traced |
| 8 | Live confirmation of the §8 span fix; span content beyond a baseline family; `error.no_investment_connection` sufficiency | one run with the 25-byte-span body, tracer read, `span_count != 0` |
| 9 | `envelope+0x00`'s real value / the `0x4C`↔503, `0x3B`↔206 mapping | probe `0x27FDD0`'s return live |
| 10 | Whether message 9 is accepted on the AH connection specifically (`[I]` yes - global tables) | falsifiable free: if no delivery, push 9 from `d1-responder` on `OUT PRIMARY` |
| 11 | Whether the client's udt-17 `request_state_refresh` fires after a successful join | `0x29AB80` is on the accept branch of `0x29BD10`; the AH would log it as a `ClientNotification` |
| 12 | Message 160 `bap_to_activity_host_notification` | the hop we bypass; only interesting for a real BAP relay |
| 13 | `bootflow:investment_signin` and the ~40 `c_investment_bap_message_*` descriptor bodies | not started |
| 14 | What `kind & 0x100` means to the schema compiler (`[I]`; wire shape settled) | 20 array-bit tags remain unswept - irrelevant to the span gate, not to schema modelling |
| 15 | The runtime acceptance run for the §5 probe kit | never performed; static 38/38 only |
| 16 | Message 160's header class (3 vs 6 - the §3.1 contradiction) | read its vtable slot `+0x08` directly |

## 10. Dead claims

Each line: the claim, and what killed it. None of these may be carried forward as live.

1. **"The sealed AES/HMAC body is the reply the client waits for in `_channel_starting`."**
   Killed by the `0x001A` runs (`failed to find recipient for header`, identical at seq 0 and
   seq 1) and by `update(_channel_starting)` containing no decrypt call at all; the sealed body
   is the `0x001A` reply to `0x0019`, one round trip later.
   (historicaldocs/design/bap-protocol.md banner, §3.2, §4.2)
2. **"A 6-byte header (and a 96-byte total frame) suffices for the responses."** Killed: `0x1F`
   and `0x1A` are header class 2 = 8 bytes with mandatory status `0x00C8`, and `0x561AE0`'s size
   gates force the `0x1A` frame to exactly 114 bytes (sealed_length 96, ciphertext 48).
   (historicaldocs/design/bap-protocol.md §A.0, §A.6)
3. **"`0x1E → 0x1F` is only an inference from string pairing."** Retired, upward: `[C]` by
   direct read of the kind→reply table `0x15D1B40`.
   (historicaldocs/design/bap-protocol.md §A.2.4)
4. **"Class 6 has flag `0x10` set; the flag byte is a capability mask; d1-bap's HAS_STATUS gloss
   is wrong."** Killed by arithmetic: `0x4F & 0x10 == 0`; across all eight classes bit `0x10`
   set ⟺ a real status accessor exists.
   (historicaldocs/design/bap-header-classes.md §2.1)
5. **"`0x55F6B0` gates message 100's receive and makes a wrong `recipient_id` a silent drop."**
   Killed: wrong class (`c_investment_bap_message_policy_...`, not the push policy), and the
   receive path never reaches a policy for message 100 at all.
   (historicaldocs/design/bap-header-classes.md §6.1)
6. **"`FUN_0029B6D0` is a recipient check."** Killed: it is `x < 0x31 && x == 7`, a predicate
   over the 49-entry AH udt space, true only for udt 7 `sensor_message`.
   (historicaldocs/design/bap-header-classes.md §6.1)
7. **"The BAP message id cannot be read from the vtable."** Killed: it is vtable slot 0, a
   `mov eax, id; ret` thunk; the earlier pass checked slots 2 and 3 and never slot 0.
   (historicaldocs/design/bap-message-table.md §1.1)
8. **"FNV-1 of eleven spellings matched 0 of 101 rows, so the ids are unknowable."** Killed:
   true result about the wrong table - `bap-message-idspace.tsv` is the investment id space
   (ids 1701+); transport ids are small integers.
   (historicaldocs/design/bap-message-table.md §1.1)
9. **"`kind & 0x100` is a repeated/array marker."** RETRACTED: it is an optional-presence bit;
   wire length is constant across element counts, one extra leading bit.
   (historicaldocs/design/span-sweep.md §10.1)
10. **"Message 206's schema has no array, so no byte string can ever fill the span."** The
    question does not apply: the span is wrapper-level and schema-independent; the one-byte
    schema `0x80801869` carries spans of 1..4096 bytes.
    (historicaldocs/design/span-sweep.md §10.2)
11. **"The span may be a list of u16."** Superseded: an opaque length-prefixed byte blob; the
    16-bit field is a byte count (`0x2627A3` advances by bytes, `0x2627AE shl edx,3`).
    (historicaldocs/design/span-sweep.md §10.3)
12. **"`span = 00000000` (count 0) is a safe first shot."** Killed by driving `0x568F30`
    itself: refused, `invalid queuez transaction count 0` - and the caller discards the refusal.
    (historicaldocs/design/span-sweep.md §9a)
13. **"The `0x4C`↔503 / `0x3B`↔206 class mapping was re-confirmed."** No: the stubbed
    `0x27FDD0` oracle was circular; the mapping remains `[U]` and the earlier retraction stands.
    (historicaldocs/design/span-sweep.md §9d, §10.4)
14. **"The 11-site ladder at `0xB2E0DC`..`0xB2E47B` observes secured receive."** Killed: it is
    the state-4 pre-channel path; mode 1 skips states 3-5 entirely.
    (historicaldocs/design/client-bap-receive-probes.md)
15. **"`0x0020 → 0x0021` adjacent-numbering guesses; 501-as-span."** Refuted; only the live
    expected-type probe identifies a response.
    (historicaldocs/design/client-bap-receive-probes.md)
16. **"Sending `0x000D` would advance replication."** Refuted: `0x000D` is the numeric response
    to `0x000C` but its handler is a no-op.
    (historicaldocs/design/client-bap-receive-probes.md)
17. **"BAP message 100 is the client-facing AH push."** Killed by the six-times-captured live
    relay: 100's deliver step forwards the identical body as message 9 on the connection indexed
    by header `+0x06`; the client consumes 9.
    (historicaldocs/design/establish-client-gate.md headline, §2.3)
18. **"`recipient_id` should be the account SOID"** (`D1_AH_RECIPIENT`'s old default). Killed:
    `FUN_0075AB60` compares it with the established or pending activity-host id, never an
    account. (historicaldocs/design/establish-client-gate.md §3.2)
19. **"Elderberry is status 28 `timed_out_waiting_to_establish_client`."** Killed: the observed
    reason maps to status 26, the first wait for any join response; the establish machine was
    never reached. (historicaldocs/design/establish-client-gate.md §3.1)
20. **"AH message order does not matter" (unscoped).** Scoped, not preserved whole: true of the
    establish gate (arrival-completeness), false of the router - udt 0/1/11 are silently
    unroutable until udt 4 is applied.
    (historicaldocs/design/establish-client-gate.md §4)
21. **"Header classes 3/5/6 field layouts are `[U]`."** Closed 2026-08-08: all three read,
    implemented, class 3 confirmed `observed-live`.
    (historicaldocs/design/bap-header-classes.md, closing
    historicaldocs/design/bap-protocol.md §A.7)

## 10b. The response-envelope instruction table, for re-checking `[C]` `static-client-inference`

Read 2026-08-07. Rescued from the docstring of `capture-rig/oodle/dis-eboot.py`, which was its only
copy; that script is superseded by `d1-eboot disasm` and archived to `historicaldocs/python-capture/`.
Section 8 above carries the wire *grammar*; this table carries the *instruction addresses* that
grammar was read from, plus the definition-row mechanics, so a future reader can re-derive rather
than trust.

`0x261FB0` decode and `0x261BE0` encode, both driven from the same 0x20-byte handle:

```
handle +0x00 u16  definition-table INDEX      +0x10 ptr span bytes
       +0x08 ptr  decoded C object            +0x18 i32 span byte count

  decode                                        encode
    0x26211F  stream init                         0x261C1D  esi = word[handle+0]  (the INDEX)
    0x26212E  read u16 <- wire message id         0x261C27  P = [mgr_vt+0x4D0](index)
    0x262135  0x27FDD0(msgid) -> INDEX            0x261C36  tag = [P+0x18]  (dir 1, response)
    0x26213A  word[handle+0] = INDEX              0x261C46  tag = [P+0x14]  (dir 0, request)
    0x2621CF  mgr = 0x25C9A0()                    0x261C93  write u16 = word[P+0x10] (message id)
    0x2621DE  P = [mgr_vt+0x4D0](INDEX)           0x261CE7  0x2C8FF0 encode object per `tag`
    0x2621F9  tag = [P+0x18]  (dir 1)             0x261DF4  ebx = [handle+0x18]
    0x2622A3  tag = [P+0x14]  (dir 0)             0x261E11  write 1 BIT = (count > 0)
    0x2625A9  0x2C9000 decode object per `tag`    0x261E59  write u16 = count
    0x2626C0  0x73430 read 1 BIT (span present)   0x261EA1  write count BYTES from [handle+0x10]
    0x2626D9  read u16 -> [handle+0x18] count
    0x26279F  [handle+0x10] = arena + out_n
    0x2627B8  0x730B0 read count BYTES
```

`edx` (arg3) is the DIRECTION: 1 = response schema, 0 = request schema, anything else = tag
`0xFFFFFFFF`. Shim `0x569C30` hardcodes 1, so the BAP envelope path always decodes responses.

The definition row: `[mgr_vt+0x4D0]` = `0x25A330`, stride **0x40**, returns `row + 0x10`; so
`P+0x10` u16 message id, `P+0x14` request tag, `P+0x18` response tag. `0x25C9A0` and `0x25A320`
return the SAME object (`0x25C970` copies the pointer from `0x2639AE8` into `0x2639AE0`).

⚠ The archived script also carried a standing warning worth keeping: a linear sweep desyncs on the
first non-instruction byte and capstone then STOPS, so a whole-image disassembly returns a truncated
stream and every "0 references found" it produces is a lie. Use `d1-eboot xref` or `d1-eboot reloc`
for cross-references, and always carry a positive control.

## 11. Sources

| melted doc | taken from it |
|---|---|
| historicaldocs/design/bap-protocol.md | §1-2 envelope and base header; §2.1-2.2 recipient matching; §4 handshake states, `0x1E`/`0x1F`/`0x19`/`0x1A`, sealed-body format, libtomcrypt identification, frame geometry; failure-reason enum; the string-enum type names |
| historicaldocs/design/bap-header-classes.md | §3 entire: the eight classes, flags, layouts of 3/5/6, tiling control, class-3 `observed-live` proof, class-6 `[I]`, `HeaderExtras`; the §2.1 and §6.1 corrections (dead claims 4-6); parts of §7.1 (push receive, policy slot 1, simulation queue) |
| historicaldocs/design/bap-message-table.md | §6 entire: id location, registry `0x1922110`, the 58-row table, AH set, no-class ids, naming debt, body-decoder addresses; dead claims 7-8 |
| historicaldocs/design/span-sweep.md | §8 entire: span grammar, write sites, count gate, codec `0x7B` format and refusals, proposed bodies, negative sweep with controls, traps, tooling; dead claims 9-13 |
| historicaldocs/design/client-bap-receive-probes.md | §5 entire: receive-state split, secured ladder, QueueZ ladder, rearm/guard discipline, acceptance criteria; dead claims 14-16 |
| historicaldocs/design/establish-client-gate.md | §7 entire: 9-vs-100 relay, connection index, recipient match, shared body codec, silent-drop sites, router ordering, ships list; dead claims 17-20 |

Related live docs: design/activity-host.md (AH message bodies, udt handlers, establish gate) ·
design/queuez-replication.md (QueueZ family/object semantics) · design/architecture.md (where
BAP sits among the three server layers) · design/instruments-static.md (`d1-eboot`, atlas) ·
design/instruments-live.md (tracer, probes in practice) · design/signon-and-entitlement.md (the
signon token and key blobs the login request consumes).
