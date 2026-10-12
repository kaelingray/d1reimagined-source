# SignOn and entitlement - how the client signs on and what it thinks it owns

**State.** The `/SignOn` request/response schema is recovered from the 01.29 eboot and confirmed live
(the client accepted our `response_type=0` body on first attempt, 2026-08-06). `sku_config.txt`
obfuscation is solved and round-trip verified, but rewriting the file is blocked by an unconditional
RSA-2048 signature. Entitlement is a set of 32-bit offer keys chosen between two local stores by
`ownership_authority` (`0x26A4A74`); in orbit the owned set is 100% locally determined by the
emulator, and the one server channel that could bind it - SignOn response field 10 - is only live in
the `content_check`-through-`prepare_for_orbit` window and has never been sent.

**Reach.** SignOn schema: `static-client-inference` promoted to `synthetic-generic`/"the client acts
on it" by the 2026-08-06 live runs; everything downstream of that gate is `synthetic_ancestor`.
Entitlement control flow: `static-client-inference`; its live timeline and offer-key set:
`observed-live` (`synthetic_ancestor` x2). Bungie.net API: `web-session proposal`, unverified except
the eboot string checks.

---

## Table of contents

1. SignOn transport, HTTP status handling, and the codec
2. SignOn request message schema
3. SignOn response message schema (the deliverable)
4. The `response_type` discriminator
5. Minimum viable success body and the traps
6. Live confirmation of the SignOn gate
7. `sku_config.txt` - format, obfuscation, RSA gate, and the `signon_url` override
8. Entitlement - offer keys, two stores, `ownership_authority`
9. Why entitlement is LOCAL in orbit, and the field-10 server path never sent
10. Consumers of the owned set
11. Bungie.net public API as a data source (short)
12. Open [U] gates
13. Dead claims
14. Sources

---

## 1. SignOn transport, HTTP status handling, and the codec

### 1.1 Transport `[O]`

The client POSTs to
`https://www{1..8}.<signon-host>/SignOn?platform=orbis&build=52553.16.09.28.1551.evil`
with `content-type: application/octet-stream`. Host literals in the binary:
`signon.deadorbit.net` (`0x161a86b` file) and `signon.gravityshavings.net` (`0x161a850`),
templated by `https://www%d.%s/%s` (`0x161a880`) and `SignOn?platform=%s&build=%s` (`0x161a834`).

### 1.2 HTTP status handling `[C]`

`c_signon_service_client::on_request_failed` at `0x2abcc0`:

| Status | Client behaviour | String |
| --- | --- | --- |
| `500` | permanent-ish internal error | `Signon server request failed with an internal server error.` |
| `503`, `509` | back off retries immediately | `Server busy: status %d.  Backing off retries immediately.` |
| anything else non-2xx | retry | `Signon server request failed with status %d.  Will retry.` |

The success path needs **HTTP 200** with the protobuf as the raw body. `509` (not a standard code) is
treated identically to `503`.

### 1.3 The codec is table-driven, not protoc-generated `[C]`

Standard protobuf tag/varint/length-delimited encoding on the wire, but there are **no**
`google.protobuf` symbols, no `.proto` descriptor strings, and no protoc-generated parse methods. One
generic reader and one generic writer are driven by static field-descriptor arrays.

| Function | Role |
| --- | --- |
| `0xa1470` | stream/reader ctor over `(buf, len)` |
| `0xa2020` | `parse(stream, descriptor, out)` - calls `0xa2060` then `0xa17d0` |
| `0xa2060` | `clear(descriptor, out)` - resets flags/counts, memcpys defaults |
| `0xa17d0` | the wire parse loop |
| `0xa2c20` / `0xa2c80` | writer ctor / `encode(writer, descriptor, msg)` |
| `0x18ed0a0` | field-type handler table, 16 slots |

Codec error strings live at `0x1603e10`-`0x1603ee1` (file `0x1607e10`+):
`end-of-stream`, `io error`, `varint overflow`, `invalid wire_type`, `parent stream too short`,
`missing required field`, `invalid data_size`, `bytes overflow`, `string overflow`,
`invalid field descriptor`, `array overflow`, `callback failed`, `invalid field type`.

The two signon descriptor tables: request `0x18f8b40`, response `0x18f8ce0`. Schema is read directly
from these tables, not inferred from a switch chain.

### 1.4 Descriptor record layout `[C]`

23 bytes, packed, one per field, ordered by ascending field number, terminated by a record whose
field number is `0`. Derived from `0xa2060` (`+5`,`+6`,`+7`,`+0xb`,`+0xf`) and `0xa2100`-`0xa2151`
(stride `0x17`).

```c
struct field_desc {           // 23 bytes, no padding
    uint32_t field_number;    // +0x00   0 terminates the array
    uint8_t  type;            // +0x04   high nibble = cardinality, low nibble = field type
    uint8_t  pre_pad;         // +0x05   bytes consumed before this field's data (flag + alignment)
    int8_t   flag_delta;      // +0x06   signed offset from data ptr back to the presence flag / count
    uint32_t size;            // +0x07   element size in bytes
    uint32_t capacity;        // +0x0b   array capacity (repeated only)
    void    *aux;             // +0x0f   sub-descriptor (type 6) or default-value blob
};
```

**Cardinality (high nibble)** `[C]`:

| Value | Meaning | Storage |
| --- | --- | --- |
| `0x00` | required | no presence flag |
| `0x10` | optional | `bool` presence flag at `data + flag_delta` |
| `0x20` | repeated | `uint64` count at `data + flag_delta`, `capacity` elements of `size` |
| `0x30` | callback | struct holds `{ void *fn; void *ctx; }` |

**Field type (low nibble)** `[C]`, from the handlers in `0x18ed0a0`:

| Nibble | Handler | Meaning | Canonical wire type | In-struct storage |
| --- | --- | --- | --- | --- |
| `0` | `0xa22e0` | varint | 0 | integer of `size` bytes |
| `1` | `0xa2430` | zigzag varint (`sint32/64`) | 0 | integer |
| `2` | `0xa2560` | fixed 4 bytes | 5 | `uint32`/`float` |
| `3` | `0xa2610` | fixed 8 bytes | 1 | `uint64`/`double` |
| `4` | `0xa26c0` | bytes | 2 | `{ uint64 len; uint8 data[size-8]; }` |
| `5` | `0xa27f0` | string | 2 | `char[size]`, NUL-terminated, max `size-1` chars |
| `6` | `0xa2920` | nested message | 2 | inline sub-struct, `aux` = sub-descriptor |
| `7` | *(null)* | invalid | - | - |
| `8`-`14` | `0xa36e0`...`0xa3930` | present but unused by either signon table `[U]` | - | - |

Offset walk `[C]`:

```
data(0)   = base + pre_pad(0)
flag(i)   = data(i) + flag_delta(i)
total(i)  = (cardinality == 0x20) ? size(i) * capacity(i) : size(i)
data(i+1) = data(i) + total(i) + pre_pad(i+1)
```

Cross-check `[C]`: for the response table the walk yields struct size exactly `0x7b8`. Parse function
`0x2a99d0` places the message at `rbp-0x920` and its next stack object at `rbp-0x168`, i.e.
`0x920 - 0x168 = 0x7b8`. Field 14's flag/data offsets `0x7b1`/`0x7b4` match `0x2a9c7b`/`0x2a9c84`;
field 4's callback slot `0x4c8`/`0x4d0` matches `0x2a9a71`/`0x2a9a6a`. Three independent hits.

### 1.5 The `aux` pointers are relocations, not file bytes `[C]`

Every `aux` field reads as **zero** in the file image. They are `R_X86_64_RELATIVE` entries in
`DT_SCE_RELA` (dynlibdata `+0xbdd0`, size `0x1b0918`, 73825 entries, 70682 of type 8). Re-deriving
these tables without applying the relocations wrongly concludes nested-message fields have no
sub-descriptor. Recovered response-tree values:

```
0x18f8d06 -> 0x18f8e10   (f2  sub-descriptor)
0x18f8d1d -> 0x15ab4a0   (f3  sub-descriptor)
0x18f8d34 -> 0x15a7ba0   (f4  sub-descriptor)
0x18f8d79 -> 0x15ab410   (f7  sub-descriptor)
0x18f8da7 -> 0x15ab3e0   (f10 sub-descriptor)
0x18f8ec0 -> 0x15a8fd0   (f2.9 sub-descriptor)
```

### 1.6 Parser behaviours worth knowing `[C]`

- Tag decode is textbook protobuf: varint tag at `0xa1da0`, `wire_type = tag & 7`,
  `field_number = tag >> 3` (`0xa185b`/`0xa185e`). A tag of `0` ends the message.
- Unknown field numbers are skipped, not fatal (`0xa1912` to `0xa15a0`). Extra fields are safe to omit
  and safe to add.
- The wire type is NOT validated for singular fields (cardinality `0x00`/`0x10`); the handler runs on
  the descriptor's declared type. It IS checked for repeated (must be 2 for packed) and callback
  (must be `<= 5`). Emit the canonical wire type anyway.
- Packed repeated is accepted for types 0-3 only (`cmp eax, 3` at `0xa19c1`); non-packed accepted for
  all repeated fields. Exceeding `capacity` -> `array overflow`.
- `missing required field` is raised in a final pass (`0xa1e80`) over all cardinality-`0x00` records
  whose presence bit was never set, unless the field has a non-null `aux` default or its type is `6`
  (a missing required nested message is tolerated).
- String fields need `len + 1 <= size`; bytes fields need `len <= size - 8`.

---

## 2. SignOn request message schema

Descriptor **`0x18f8b40`**, struct size `0x6d6` (1750 bytes), encoded into a `0x26d2`-byte buffer.
Encoder at `0x2ab2c0`+; `lea rsi, [rip+...]` -> `0x18f8b40` and `call 0xa2c80` at `0x2ab3b6`; failure
logs `Error encoding signon request` at `0x2ab41f`. Schema `[C]`, field 1 and 2 values `[O]`.

| # | Wire | Type | Card. | Limit | Meaning |
| --- | --- | --- | --- | --- | --- |
| 1 | 0 | uint32 | optional | - | `[O]` observed `= 13`. Protocol/message version. `[I]` |
| 2 | 2 | bytes (callback) | callback | streamed | `[O]` the PSN OAuth JSON (`{"access_token":...,"id_token":...}`), 664 bytes observed |
| 3 | 2 | string | optional | 7 chars | `[U]` |
| 4 | 2 | bytes (callback) | callback | streamed | `[U]` |
| 5 | 2 | string | optional | 63 chars | `[U]` |
| 6 | 2 | string | optional | 63 chars | `[U]` |
| 7 | 0 | bool | optional | - | `[U]` |
| 8 | 0 | bool | optional | - | `[U]` |
| 11 | 2 | string | optional | 254 chars | `[U]` |
| 12 | 2 | bytes | optional | 256 B | `[U]` |
| 13 | 0 | uint64 | optional | - | `[U]` |
| 15 | 0 | uint32 | **repeated** | 64 | `[I]` echo of response field 10 |
| 17 | 2 | string | optional | 63 chars | `[U]` |
| 18 | 2 | string | optional | 255 chars | `[U]` |
| 19 | 2 | string | optional | 7 chars | `[U]` |
| 20 | 2 | message | optional | - | 8 optional varints: `u64,u32,u64,u64,u64,u64,bool,u32` `[U]` |
| 21 | 2 | message | optional | - | 5 optional strings: 63,63,127,31,31 chars `[U]` |

Fields **9, 10, 14, 16 do not exist** in the request. `[C]`

`[I]` The reauth/refresh request re-packs bits from response field 8 into a request field:
`0x2aaf7a`-`0x2aafa8` reads the client bools derived from response f8 bits 2, 5 and 6 and rebuilds
`(b2<<2) | (b5<<5) | (b6<<6)`. So f8 round-trips.

---

## 3. SignOn response message schema (the deliverable)

Descriptor **`0x18f8ce0`**, struct size **`0x7b8`** (1976 bytes). Parsed and consumed by
`c_signon_service_client::process_response` at **`0x2a99d0`** (log:
`Parsing signon server response (%d bytes)`). Schema `[C]`.

Top-level fields. "Complains if absent" = the client emits a diagnostic naming the field.

| # | Tag byte | Wire | Type | Card. | Name | Meaning | Complains if absent |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `0x08` | 0 | uint32 | required | `response_type` | Discriminator, 0-3 only. See §4. | `Unknown signon server response type: %d` if > 3 |
| 2 | `0x12` | 2 | message | optional | `success` | Success payload, 640 B. Required for `response_type=0`. | - (but see §5) |
| 3 | `0x1a` | 2 | message | optional | `queue` | Queue / server-offline payload, 544 B. | - |
| 4 | `0x22` | 2 | message | optional | `client_config` | `cookie` + `social_matchmaking_id` | - |
| 5 | `0x28` | 0 | uint64 | optional | `silo_id` | OR-ed with the executable type; logged `Silo ID combined with executable type is %llu` | yes: `Signon server did not report a silo id` |
| 6 | `0x32` | 2 | string `<=63` | optional | `content_build_version` | `strncpy(this+0x420c, ..., 64)` | yes: `did not report a content build version` |
| 7 | `0x3a` | 2 | message | optional | `denial` | Denial payload, 280 B. | - |
| 8 | `0x40` | 0 | uint32 | optional | `flags` | Bitfield; bits 2-6 consumed. See §3.6. | - |
| 10 | `0x52` | 2 | message | optional | `id_list` | Wrapper around one repeated uint32 (`<=64`) | - |
| 12 | `0x60` | 0 | uint64 | optional | `server_unix_time` | `Client clock offset from server clock: [%lld] seconds` = `now - this` | - |
| 13 | `0x6a` | 2 | string `<=63` | optional | `environment` | `Server environment: %s`; also registered as config var `environment_string` | - |
| 14 | `0x70` | 0 | uint32 | optional | `client_external_ip` | Client's public IPv4 as seen by the server | - |

Top-level fields **9 and 11 do not exist**. Field 1 is the only required top-level field, and it has
no default; see §5.1 for why that is not what you would expect. `[C]`

### 3.1 Field 2 - `success` (descriptor `0x18f8e10`, 640 B) `[C]`

Consumed by `0x2aa350`. **All seven of sub-fields 1-7 are cardinality `0x00` (required) with no
default.** If you emit field 2 at all, omitting any of them errors with `missing required field`.

| # | Tag | Wire | Type | Card. | Limit | Meaning | Evidence |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `0x0a` | 2 | bytes | required | 16 B | `[U]` parsed but never read by this build | no `[r14+0x08/0x10]` access in `0x2aa350` |
| 2 | `0x12` | 2 | bytes | required | 16 B | `[I]` session key material A | copied to `this+0x41bf`, handed out with the token by the accessor at `0x2ab9e9` |
| 3 | `0x1a` | 2 | bytes | required | 16 B | `[I]` session key material B | copied to `this+0x41cf`, same accessor `0x2aba02` |
| 4 | `0x22` | 2 | bytes | required | 512 B | `[C]` `security_token` | heap-allocated by size; failure logs `Could not allocate %d bytes for security token` |
| 5 | `0x28` | 0 | uint64 | required | - | `[C]` `token_expiry_unix` (absolute) | `0x2aa5a3` to `this+0x4200` |
| 6 | `0x30` | 0 | uint32 | required | - | `[C]` `bap_server_ip` | `movbe this+0x41f0` at `0x2aa574`; §3.7 |
| 7 | `0x38` | 0 | uint32 | required | u16 used | `[C]` `bap_server_port` | low 16 bits to `this+0x41f4` at `0x2aa584` |
| 9 | `0x4a` | 2 | message | optional | - | `[C]` three floats (`fixed32`); `[U]` purpose | `vmovss` loads at `0x2aa4a1`-`0x2aa4cc` to `this+0x41e4/41e8/41ec` |
| 11 | `0x58` | 0 | uint64 | optional | - | `[C]` `token_lifetime_seconds` - relative; if present the client uses `now() + this` and ignores sub-field 5 | `0x2aa58b`-`0x2aa5a1` |

Sub-fields **8 and 10 do not exist**. Sub-field 9's nested descriptor is `0x15a8fd0`: three required
`fixed32` fields (wire type **5**, not 0). On success this sets client state to **3 = `_success`**; on
token-allocation failure to **6 = `_permanent_error`**.

### 3.2 Field 3 - `queue` / server-offline (descriptor `0x15ab4a0`, 544 B) `[C]`

Consumed by `0x2aa6b0`. Sets client state to **2 = `_queued`** unconditionally (`0x2aa81e`), producing
`networking:signon: Entering signon queue`.

| # | Tag | Wire | Type | Card. | Limit | Meaning |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `0x08` | 0 | uint32 | required | - | `[U]` to `this+0x4250` (`[I]` queue position) |
| 2 | `0x10` | 0 | uint32 | required | - | `[C]` `time_remaining` seconds - logged `Sign-on Server OFFLINE. time_remaining[%us]` |
| 3 | `0x18` | 0 | uint32 | required | - | `[U]` to `this+0x4258` |
| 4 | `0x22` | 2 | string | required | 255 chars | `[C]` message text, `strncpy` 0x100 to `this+0x425c` and `this+0x4684` |
| 5 | `0x28` | 0 | bool | required | - | `[C]` `server_offline` - gates the `Sign-on Server OFFLINE` log |
| 6 | `0x30` | 0 | uint32 | optional | - | `[I]` enum; value `1` sets a client bool at `this+0x424e` |
| 7 | `0x3a` | 2 | bytes | optional | 256 B | `[U]` copied wholesale (len+payload) to `this+0x4360` |

### 3.3 Field 4 - `client_config` (descriptor `0x15a7ba0`, 32 B) `[C]`

Log: `client_config received with cookie=%lld and social_matchmaking_id=%d` (`0x2aa262`).

| # | Tag | Wire | Type | Card. | Meaning |
| --- | --- | --- | --- | --- | --- |
| 1 | `0x08` | 0 | int64 | required | `[C]` `cookie` (the `%lld`) to `this+0x4468` |
| 2 | `0x10` | 0 | int32 | required | `[C]` `social_matchmaking_id` (the `%d`) to `this+0x4470` |
| 3 | `0x1a` | 2 | bytes | callback | DO NOT EMIT. See below. |

`[C]` Sub-field 3 is a callback field: `fn = 0x2aa340` (thunk to `0x1db2e0` to `0x1db000`),
`ctx = the signon client`. The callback reads varints from the sub-stream and requires the very first
varint to equal `0x123456` (`cmp dword [rbp-0x24], 0x123456` at `0x1db02e`); it then loops reading
varints and dispatching each through `0x351c80`. A callback returning false aborts the whole parse with
`callback failed`. Omit sub-field 3 and the callback is never invoked.

### 3.4 Field 7 - `denial` (descriptor `0x15ab410`, 280 B) `[C]`

Consumed by `0x2aa8f0`. Logs `Signon denied: reason [%d]`, `Denial message: %s`,
`Account suspension end time %s`.

| # | Tag | Wire | Type | Card. | Limit | Meaning |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `0x08` | 0 | uint32 | required | - | `[C]` `denial_reason` - see mapping below |
| 2 | `0x10` | 0 | uint32 | optional | - | `[U]` |
| 3 | `0x18` | 0 | uint64 | optional | - | `[C]` `suspension_end_time` (unix); formatted via `ctime`-alike `0x129d430` |
| 4 | `0x22` | 2 | string | optional | 199 chars | `[C]` `denial_message` to `this+0x4816` |
| 5 | `0x2a` | 2 | string | optional | 49 chars | `[U]` to `this+0x49a6` |

`[C]` `denial_reason` -> client state, from the 10-entry table at `0x15ab770` (indexed by `reason - 1`;
anything outside 1-10 falls back to state 7 `_denied`):

| reason | state | name | matching diagnostic |
| --- | --- | --- | --- |
| 1 | 8 | `_suspended` | `Signon server reported that the user is suspended` |
| 2 | 7 | `_denied` | `Signon server denied the login` |
| 3 | 7 | `_denied` | (same) |
| 4 | 9 | `_prelaunch` | `Signon server reported that the title is in prelaunch` |
| 5 | 12 | `_platform_error` | `Signon server returned a platform error` |
| 6 | 13 | `_configuration_error` | `Signon server returned a configuration error` |
| 7 | 14 | `_commercialization_error` | `Signon server returned a commercialization error` |
| 8 | 15 | `_version_mismatch_error` | `Signon server returned a version mismatch error` |
| 9 | 7 | `_denied` | `Signon server denied the login` |
| 10 | 11 | `_internal_server_error` | `Signon server returned an internal server error` |

Both sub-field 4's and sub-field 5's presence flags must be set for `Denial message: %s` to be logged
(`0x2aa9aa`/`0x2aa9b5`), but both strings are copied out regardless.

### 3.5 Field 10 - `id_list` (descriptor `0x15ab3e0`, 264 B) `[C]`

One field: `1`, tag `0x08`, repeated uint32, capacity 64. Packed (tag `0x0a`) also accepted. Copied to
`this+0x4580` with the count at `this+0x4680`, then re-exported by the accessor at `0x2ac0e0` into a
signon-manager global (`0x263d2e8+0x184`, count at `0x263d56c`).

**This is the OWNED OFFER-KEY LIST** (`[U]` closed 2026-08-08). `0x2a7090` case 2 clears `0x263d46c` to
`0xFF`, calls `0x2ac0e0(&response, &n, &p)` and copies the `n` u32s in; `0x2a8a40` then scans that array
for `is_offer_key_owned(key)` **whenever `ownership_authority == 1`**, which `bootflow:content_check`
sets. The keys are the same space as the two compiled-in literals `0xE0000001` (full sku) and
`0xE0000002` (paid tier). Full mechanism and consumers: §8-§10. `d1-responder` has never emitted this
field, so `bootflow:package_registration` runs against an empty authoritative ownership list; and
`bootflow:prepare_for_orbit` flips the authority to `2`, after which the client reads a locally
determined set instead - so field 10 does **not** reach orbit.

### 3.6 Field 8 - `flags` bitfield `[C]` mechanics, `[I]` meanings

`uint32`. The client extracts single bits into bools (`0x2a9dd9`-`0x2a9e3a`):

| bit | client field | notes |
| --- | --- | --- |
| 2 | `this+0x4474` | `[I]` re-encoded into the reauth request |
| 3 | `this+0x4477` | `[U]` |
| 4 | `this+0x4476` | `[U]` |
| 5 | `this+0x457b` | `[I]` re-encoded into the reauth request |
| 6 | `this+0x4579` | `[I]` re-encoded into the reauth request |

`[C]` `response_type = 3` reads only bits 2, 5 and 6 (`0x2a9fe2`-`0x2aa01a`). `[I]` Given
`cannot refresh content rights...` / `cannot refresh email...` nearby, these are "needs refresh"
signals. Send `0` (or omit) unless you want to trigger a refresh flow.

### 3.7 Integer encoding of the two IP address fields `[I]`

Derived through the formatting path in `bap_connection` (`0xb2b87d`-`0xb2b8f8`): the stored word is
byte-swapped and printed as `%d.%d.%d.%d` from LSB upward of the swapped value.

`f2.6` (and by symmetry `f14`) is the IPv4 address packed big-endian into the varint:
`a.b.c.d` -> `(a<<24)|(b<<16)|(c<<8)|d`. E.g. `203.0.113.84` -> `0xC0A80154` -> `3232235860`. `f2.7` is
the port in plain host order (e.g. `3074`), only the low 16 bits used.

`[C]` Field 2.6/2.7 really are the BAP server address. The signon client exposes them as `0x1db2a0`
(address) and `0x1db2b0` (port, overridable by the global at `0x25c1030`). The only caller of both is
`0xb2bacb`, inside `bap_connection::resolve_transport_address` at `0xb2b840` (strings
`networking:server:bap: ...`, including `Failed to resolve valid transport ip address`). They are the
fallback source, used when no address is explicitly configured (`[r15+0x14]==0 && !([r15+0x10]&2)`).

`[C]` Field 14 really is the client's external address. `0x2a9c99`-`0x2a9ca8` builds a 24-byte
`network_address` (`{ uint32 addr @0; uint16 family=4 @0x12 }`) from field 14 and passes it to
`0x2df930`, which logs `networking:stun: Client external address from signon: [%s].` or
`Invalid external address from signon, will not be saved.` This is the value referenced by
`Attempting to update our external address to: [%s] obtained from SignOnResponse: [%s]` and the
STUN-bypass paths. See §6 for confirmation. See design/bap-transport.md for the BAP layer these
addresses feed.

---

## 4. The `response_type` discriminator `[C]`

Field **1**, varint, `uint32`. Gate at `0x2a9cb4`: `cmp eax, 3 / jbe` - anything `> 3` logs
`Unknown signon server response type: %d` and does nothing. Jump table at `0x2aa32c`:

| value | target | payload field | resulting client state | meaning |
| --- | --- | --- | --- | --- |
| **0** | `0x2a9d55` | field 2 | `3 _success` (or `6 _permanent_error` on token alloc failure) | SUCCESS - the one we want |
| **1** | `0x2a9e46` | field 3 | `2 _queued` | queue / server offline |
| **2** | `0x2a9f1f` | field 7 | per `denial_reason` table (§3.4) | denied |
| **3** | `0x2a9f46` | none (reads field 8 only) | `10 user_action_required` | user must act (e.g. accept an EULA) |

`[C]` Client state enum (name table at `0x18f9130`, 16 entries):
`0 _none, 1 _pending, 2 _queued, 3 _success, 4 _server_unavailable, 5 _retriable_error,
6 _permanent_error, 7 _denied, 8 _suspended, 9 _prelaunch, 10 user_action_required,
11 _internal_server_error, 12 _platform_error, 13 _configuration_error,
14 _commercialization_error, 15 _version_mismatch_error`.

Alternate response shapes to avoid emitting accidentally - all four reachable from a single varint:

- `08 01` -> queue. Client enters `_queued`, sits in the signon queue retrying.
- `08 02` -> denial. With no field 7 the reason reads as `0` -> falls through to state `7 _denied`.
- `08 03` -> `user_action_required`.
- Omitting field 1 entirely -> struct is zero-initialised, behaves as `0` (success). `08 00` is also
  what a zeroed struct looks like, which is why the placeholder produced success-path complaints rather
  than an "unknown type" complaint. See §5.1.

---

## 5. Minimum viable success body and the traps

### 5.1 The observed log order is diagnostic, and it exonerates nothing `[C]`/`[I]`

A run logged, in this order:

```
Signon server did not report a silo id
Signon server did not report a content build version
Unable to parse signon response
Error processing signon request
```

Explained by the code:

1. `0x2a9aa5` calls the parser and **discards its return value** - no instruction between the `call` and
   the next `call` at `0x2a9aaa`. Parse errors are recorded in the stream object's error slot and, as
   far as found, never read. `[C]` for the discard, `[I]` for "never read anywhere".
2. The consumer runs regardless, over a zero-initialised struct: `response_type` reads `0` (no "unknown
   type" line), field 5 absent -> silo-id complaint, field 6 absent -> content-build complaint.
3. `0x2a99d0` returns the dispatch flag, not the parse flag. For `response_type = 0` that flag is set
   only if field 2 was present (`0x2a9d5f` to `0x2aa350`).
4. Caller `0x2abad0` tests it at `0x2abb75` and, on false, logs `Unable to parse signon response` and
   sets state `6 _permanent_error`.

**Consequence:** `Unable to parse signon response` does NOT mean the bytes were malformed. It means "the
response was not actionable". A byte-perfect protobuf that omits field 2 produces the identical four log
lines.

### 5.2 Minimum viable `response_type = 0` body

Mandatory: field 1, field 2 with all of sub-fields 1-7, plus fields 5 and 6 to silence the two
complaints. Fields 12/13/14 are optional niceties.

```
08 00                                  # f1  response_type = SUCCESS

12 <len>                               # f2  Success {
   0a 10 <16 bytes>                    #   f2.1 unused_blob      (required)
   12 10 <16 bytes>                    #   f2.2 session_key_a    (required)
   1a 10 <16 bytes>                    #   f2.3 session_key_b    (required)
   22 <len> <token bytes, <=512>       #   f2.4 security_token   (required)
   28 <varint>                         #   f2.5 token_expiry_unix (required)
   30 <varint>                         #   f2.6 bap_server_ip    (required, BE-packed)
   38 <varint>                         #   f2.7 bap_server_port  (required, host order)
                                       # }
28 <varint>                            # f5  silo_id
32 <len> <ascii>                       # f6  content_build_version, <=63 chars
60 <varint>                            # f12 server_unix_time  (avoids a bogus clock-offset log)
6a <len> <ascii>                       # f13 environment
70 <varint>                            # f14 client_external_ip (BE-packed) - lets STUN be bypassed
```

Worked example - silo `12345`, build `"52553.16.09.28.1551.evil"`, BAP `203.0.113.84:3074`:

```
f5 : 28 b9 60
f6 : 32 18 35 32 35 35 33 2e 31 36 2e 30 39 2e 32 38 2e 31 35 35 31 2e 65 76 69 6c
f2.6: 30 d4 82 a0 85 0c        (0xC0A80154 = 3232235860)
f2.7: 38 82 18                 (3074)
```

### 5.3 Traps, in the order you are likely to hit them `[C]`

1. Omitting any of `f2.1`-`f2.7` -> `missing required field`. All seven are cardinality `0x00` with no
   default. `f2.1` is required even though the client never reads it.
2. Omitting field 2 on `response_type=0` -> `Unable to parse signon response`, state
   `_permanent_error`, with no hint that the payload message was the problem.
3. Emitting `client_config.3` -> the callback demands a leading varint `0x123456` and aborts the entire
   parse with `callback failed`. Send fields 1 and 2 only.
4. Over-long strings/bytes -> `string overflow` / `bytes overflow`. Strings need `len + 1 <= size`, so a
   `char[64]` field takes at most 63 chars.
5. `f2.9` uses wire type 5 (fixed32 floats), not varint.
6. `f2.11` silently overrides `f2.5`. If you send both, the client computes `now() + f2.11` and ignores
   the absolute expiry.
7. `id_list` (f10.1) capacity is 64. More than 64 elements -> `array overflow` -> whole parse fails.
8. HTTP 503 or 509 triggers immediate retry back-off; 500 is treated as internal server error. Return
   200.

### 5.4 Compilable `.proto`

Wire-compatible with the client's codec; `required`/`optional` mirror the descriptor cardinality nibble,
so this needs **proto2**. The client reads no field names; only numbers and types are load-bearing.

```proto
syntax = "proto2";
package d1.signon;

// ---------- request (descriptor 0x18f8b40) ----------
message SignOnRequest {
  optional uint32 version              =  1;  // observed = 13
  optional bytes  platform_token       =  2;  // PSN OAuth JSON
  optional string unk3                 =  3;  // <= 7 chars
  optional bytes  unk4                 =  4;
  optional string unk5                 =  5;  // <= 63
  optional string unk6                 =  6;  // <= 63
  optional bool   unk7                 =  7;
  optional bool   unk8                 =  8;
  optional string unk11                = 11;  // <= 254
  optional bytes  unk12                = 12;  // <= 256 B
  optional uint64 unk13                = 13;
  repeated uint32 id_list_echo         = 15;  // max 64
  optional string unk17                = 17;  // <= 63
  optional string unk18                = 18;  // <= 255
  optional string unk19                = 19;  // <= 7
  optional Unk20  unk20                = 20;
  optional Unk21  unk21                = 21;

  message Unk20 {
    optional uint64 a = 1; optional uint32 b = 2; optional uint64 c = 3;
    optional uint64 d = 4; optional uint64 e = 5; optional uint64 f = 6;
    optional bool   g = 7; optional uint32 h = 8;
  }
  message Unk21 {                 // all optional strings
    optional string a = 1;        // <= 63
    optional string b = 2;        // <= 63
    optional string c = 3;        // <= 127
    optional string d = 4;        // <= 31
    optional string e = 5;        // <= 31
  }
}

// ---------- response (descriptor 0x18f8ce0) ----------
message SignOnResponse {
  enum Type { SUCCESS = 0; QUEUED = 1; DENIED = 2; USER_ACTION_REQUIRED = 3; }

  required uint32       response_type         =  1;   // 0..3 only
  optional Success      success               =  2;   // required when response_type == 0
  optional Queue        queue                 =  3;   // response_type == 1
  optional ClientConfig client_config         =  4;
  optional uint64       silo_id               =  5;   // else: "did not report a silo id"
  optional string       content_build_version =  6;   // <= 63; else: "did not report a content build version"
  optional Denial       denial                =  7;   // response_type == 2
  optional uint32       flags                 =  8;   // bitfield; send 0
  optional IdList       id_list               = 10;
  optional uint64       server_unix_time      = 12;
  optional string       environment           = 13;   // <= 63
  optional uint32       client_external_ip    = 14;   // (a<<24)|(b<<16)|(c<<8)|d

  message Success {                      // descriptor 0x18f8e10 - 1..7 ALL REQUIRED
    required bytes  unused_blob        =  1;  // <= 16 B; parsed but never read by the client
    required bytes  session_key_a      =  2;  // <= 16 B
    required bytes  session_key_b      =  3;  // <= 16 B
    required bytes  security_token     =  4;  // <= 512 B
    required uint64 token_expiry_unix  =  5;
    required uint32 bap_server_ip      =  6;  // (a<<24)|(b<<16)|(c<<8)|d
    required uint32 bap_server_port    =  7;  // low 16 bits used
    optional Tuning tuning             =  9;
    optional uint64 token_lifetime_sec = 11;  // if set, OVERRIDES token_expiry_unix
    message Tuning {                          // descriptor 0x15a8fd0 - wire type 5!
      required fixed32 a = 1;                 // float
      required fixed32 b = 2;                 // float
      required fixed32 c = 3;                 // float
    }
  }

  message Queue {                        // descriptor 0x15ab4a0
    required uint32 unk1           = 1;
    required uint32 time_remaining = 2;   // seconds
    required uint32 unk3           = 3;
    required string message        = 4;   // <= 255
    required bool   server_offline = 5;
    optional uint32 unk6           = 6;   // value 1 is special
    optional bytes  unk7           = 7;   // <= 256 B
  }

  message ClientConfig {                 // descriptor 0x15a7ba0
    required int64 cookie                = 1;
    required int32 social_matchmaking_id = 2;
    // field 3 is a CALLBACK field: a varint stream whose first varint must be 0x123456.
    // DO NOT EMIT - a malformed blob aborts the whole parse with "callback failed".
  }

  message Denial {                       // descriptor 0x15ab410
    required uint32 reason              = 1;   // 1..10, see mapping table
    optional uint32 unk2                = 2;
    optional uint64 suspension_end_unix = 3;
    optional string message             = 4;   // <= 199
    optional string unk5                = 5;   // <= 49
  }

  message IdList { repeated uint32 ids = 1; }  // descriptor 0x15ab3e0, max 64
}
```

---

## 6. Live confirmation of the SignOn gate `[C]`

Runs `runs/20260806-140313-signon-f2/` and `runs/20260806-141341-signon-repro/`, execution lane,
2026-08-06. The spec was encoded exactly as §5.2 and the client accepted it on the first attempt; two
independent cold-boot runs produced the identical eleven-state ladder,
`obtained from SignOnResponse: [succeeded]`, and zero occurrences of the four failure lines. This
promotes the schema from `static-client-inference` to "the client acts on it".

What we sent: 143 bytes from `d1-responder` (`signon_success()` in
`d1-server/crates/d1-responder/src/main.rs`): top-level fields 1, 2, 5, 6, 12, 13, 14, all seven of
`f2.1`-`f2.7` present, `f4` deliberately omitted to avoid the `f4.3` callback trap. `f2.1/2.2/2.3` were
16 zero bytes each, `f2.6/f2.7` = `10.77.0.2:3074`, `f6` echoed the client's own `build=` query value.

Confirmed at once `[C]`: `environment` (f13) round-trips; `bap_server_ip`/`bap_server_port` are read and
believed (`[10.77.0.2:3074] obtained from SignOnResponse: [succeeded]`); `client_external_ip` (f14)
bypasses STUN (all 16 `stun{1..8}.signon.*` lookups failed and the client proceeded anyway because f14
was present); none of the four failure lines appeared.

Also newly observed `[O]`: Demonware initialises for the first time
(`bdAuthPS4.cpp:312: Unknown SCE IssuerID[100], default to PROD environment` - our synthetic PSN
`id_token` carries an unrecognised issuer and it falls back to PROD rather than failing);
`bdUPnP.cpp:407: Failed to send discovery packet` (no router in the namespace); the client binds its
own UDP 3074 but makes no outbound BAP connection before the next gate, so `content_check` runs before
BAP `[C]`.

**Reach warning:** the client advanced on bytes we invented, including a placeholder security token and
three 16-byte zero blobs standing in for `[I]` `secure_hello` key material. This gate is
`synthetic-generic`, so every finding downstream inherits `synthetic_ancestor`; nothing may be promoted
to protocol contract without capture or independent confirmation, least of all anything touching
authentication. If those blobs are real key material, expect a second wall at BAP `secure_hello`.

The gate this opened onto: `bootflow:content_check` then
`cleanup` for reason `error.content_manifest_not_provided` (user-visible codename "termite"). Tracked
in design/content-and-packages.md. The client made exactly four HTTP requests (PSN `oauth/token`,
`/SignOn`, two shadNet `friendList`), all answered `200 OK`, and asked for no manifest.

---

## 7. `sku_config.txt` - format, obfuscation, RSA gate, and the `signon_url` override

**Status:** obfuscation solved and round-trip verified. Rewriting the file is blocked by an RSA-2048
signature (§7.6). Subject: `dumps/patch-01.29-CUSA00219/sku_config.txt` (440 bytes). Reader:
`dumps/patch-01.29-CUSA00219/eboot.bin`.

Address convention: guest vaddr. Segment-only PS4 ELF; first `LOAD` maps file offset `0x4000` ->
vaddr `0x0`:

```
file_offset = vaddr + 0x4000                    (text/rodata, vaddr < 0x18bf0dc)
file_offset = vaddr - 0x18c0000 + 0x18c4000     (RW data)
```

Runtime addresses are these + `0x800000000`.

### 7.1 File layout - 440 bytes, three regions `[C]`

| Range | Size | Contents |
|---|---:|---|
| `[0:4]` | 4 | container version, u32 little-endian = `1` |
| `[4:8]` | 4 | u32 LE = `0` - `[I]` signature/algorithm selector |
| `[8:264]` | 256 | RSA-2048 signature over the payload |
| `[264:440]` | 176 | AES-128-CBC ciphertext, 11 blocks |

Raw header of our file: `01 00 00 00 00 00 00 00`. The "8-byte header + 432 obfuscated bytes" reading is
a trap: the encrypted region is 176 bytes starting at offset 264; the 256 bytes between are a signature.
The `0x108` (= 264) header size is not inferred - the validator reads exactly that many bytes:

```
1dc100:  mov  esi,0x108          ; read 264 bytes -> stack buffer at rbp-0x138
1dc105:  call 0x8b9b0            ; file_read(file, 264, buf)
1dc112:  mov  r12,[rbp-0x280]    ; file size (440)
1dc119:  add  r12,-0x108         ; payload size = 176
1dc14d:  mov  esi,0x108
1dc152:  call 0x8b5b0            ; seek back to 0x108  ("failed to reset read pointer")
```

### 7.2 The obfuscation: AES-128-CBC, libtomcrypt, hardcoded key and IV `[C]`

```
algorithm   AES-128-CBC, decrypt, NO padding (payload is an exact multiple of 16; NUL-filled)
key         54 00 00 00 4d 04 6e 79 00 05 03 63 72 03 74 05     @ vaddr 0x15a7400  (read-only)
iv          04 74 00 04 6c 6c 01 02 03 04 05 06 04 02 01 06     @ vaddr 0x18c2c70  (RW, no relocation)
```

Both are fixed compiled-in constants. No KDF, no per-file IV/nonce, nothing derived from title ID or
console. These 32 bytes are the entire secret. Title-wide, not per-file - confirmed on evidence:
`dumps/base-CUSA00219/sku_config.txt` decrypts cleanly under the same key and IV, identical
440-byte/264-offset/176-payload layout; its only differing line is
`base_content_version=41345-3-1213246`. `[U]` why the key has this shape (five `00` bytes out of 16);
possibly typed by hand, possibly a transformed string (contains ASCII fragments `M`, `ny`, `cr`, `t`).
Proven the key by bijective round-trip in §7.4.

Library provenance `[C]`: libtomcrypt v1.2, statically linked. Cipher descriptor at vaddr `0x19f8d38`
is an `ltc_cipher_descriptor` `{name -> "aes", ID = 6, min_key = 16, max_key = 32, block_size = 16,
default_rounds = 10}`, matching `rijndael_desc`; the `name` field is a relocated pointer (SCE
dynlibdata RELA, addend `0x16b2080` -> string `"aes"`). Leftover Windows build path in `.rodata`:
`C:\bungie\external_libraries\libtom\v1.2\misc\crypt\crypt_register...`. AES is stock: standard `Td0`
table (`Td0[0] = 0x51f4a750`) at file offset `0x15fd150`, no custom S-box.

Call chain:

| vaddr | role |
|---|---|
| `0x1ddb00` | sku_config read path: file size -> `alloc` (`0xb4d00`) -> read payload -> plaintext sniff -> deobfuscate |
| `0x1ddbc4` | `call 0xef140` - the deobfuscate call: `(buf, 176, 176, buf)` |
| `0xef140` | the deobfuscator: ctor `0x6c520`, then `set_key(obj, 0x15a7400, 16)` = `0x6c530`, then `0x6c620` |
| `0x6c530` | `set_key`: stores keylen at `obj+8`, `memcpy(obj+0x10, key, 16)`, `register_cipher(&aes_desc)` -> `obj+4` |
| `0x6c620` | `cbc_start(cipher_idx, IV = 0x18c2c70, key = obj+0x10, keylen = obj[8], 0, ctx)` -> `cbc_decrypt` (`0xfa6a00`) -> `cbc_done` (`0xfa6bc0`) |
| `0x160095f` | the literal `"version"` used for the plaintext sniff (§7.5) |

`set_key` copying the key literally is confirmed by resolving the PLT slot at `0x1a07c98` through the
SCE dynlibdata RELA table to NID `Q3VBxCXhUHs` = `memcpy`.

### 7.3 The decrypted plaintext `[C]`

```
version=2\r\n
sku_type=north_america_digital\r\n
ui_language=1117\r\n
audio_language=1117\r\n
default_language=0\r\n
title_id=CUSA00219_00\r\n
base_content_version=52553-6-2940354\r\n
```

Then `\x00` x 13 to fill the final AES block. 176 bytes total. Format: CRLF-terminated `key=value`
lines; delimiter is `0x3d` (`=`) at vaddr `0x1dc744` (`mov esi,0x3d` feeding `strchr` at `0x129dad0`).
`[O]` The retail file contains no signon URL override and no `application_password_signature` - only
these seven options.

### 7.4 Decrypt / re-encrypt recipe

```bash
KEY=54000000'4d046e79'00050363'72037405
IV=04740004'6c6c0102'03040506'04020106

# decrypt
tail -c 176 sku_config.txt | openssl enc -d -aes-128-cbc -K $KEY -iv $IV -nopad > payload.txt

# re-encrypt (input MUST be a multiple of 16 bytes; NUL-pad, do not use PKCS#7)
openssl enc -e -aes-128-cbc -K $KEY -iv $IV -nopad -in payload.txt > payload.enc
```

`[C]` Re-encrypting the recovered plaintext reproduces the original 176 ciphertext bytes byte for byte
(a bijection, not a "looks like text" judgement). Payload length is `filesize - 264`, not fixed at 176,
so a longer config is fine as long as it is a multiple of 16.

### 7.5 The option table: FNV-1 hashes, and the `signon_url` option name `[C]`

Option names are never compared as strings. The parser hashes the text left of the `=` and looks the
hash up in a 13-entry table. No option-name string literals exist in the binary.

Hash function `[C]` - FNV-1 (32-bit), lowercased. NOT FNV-1a:

```python
def sku_option_hash(name):
    h = 0x811c9dc5                             # FNV-1 32-bit offset basis
    for ch in name.lower():                    # tolower() per char, vaddr 0x64e20
        h = ((h * 0x01000193) & 0xffffffff) ^ ord(ch)   # multiply THEN xor
    return h
```

Loop at vaddr `0x1dc7f5`-`0x1dc83f`; the compiler pre-computed the first multiply as literal
`0x050c5d1f` at `0x1dc80a`. FNV-1a (`h = (h ^ c) * prime`) produces 0/7 table hits, so anyone testing
FNV-1a would wrongly conclude the hash is unknown.

Table - vaddr `0x25c1040`, 13 records x 12 bytes, `{u32 option_id, u32 name_hash, u8 required}`. Lives
in BSS, populated by the initializer at vaddr `0x1de3d4`-`0x1de53a`. Lookup loop `0x1dc84e`-`0x1dc88b`,
bound `cmp ecx,0xc` = ids 0..12.

| id | hash | required | name | how established |
|---:|---|:---:|---|---|
| 0 | `0x811c9dc5` | no | *(empty string)* | `[C]` hash of `""` - sentinel |
| 1 | `0x7e51c1bb` | yes | `version` | `[C]` hash match |
| 2 | `0xdf4b6cdf` | yes | `sku_type` | `[C]` hash match |
| 3 | `0x53268974` | yes | `ui_language` | `[C]` hash match |
| 4 | `0x9680e892` | yes | `audio_language` | `[C]` hash match |
| 5 | `0x397363a9` | yes | `default_language` | `[C]` hash match |
| 6 | `0xed89014f` | no | **`signon_url`** | `[C]` hash match (preimage) |
| 7 | `0x09fa5755` | yes | `base_content_version` | `[C]` hash match |
| 8 | `0x43fa5b90` | no | `group_overrides` | `[I]` preimage found, name reads oddly |
| 9 | `0xc07e999a` | no | `demo_mode` | `[C]` hash match (preimage) |
| 10 | `0x477fab87` | yes | `title_id` | `[C]` hash match |
| 11 | `0xed58a8a0` | no | `[U]` unknown | no preimage found |
| 12 | `0xc90b5600` | no | `application_password_signature` | `[C]` hash match (preimage) |

Ids 1-5, 7 and 10 are cross-validated: their names are the literal keys in the decrypted retail file
(§7.3) and hash to their table constants under FNV-1. Id 8 `group_overrides`: a single hit out of ~1.66M
candidates (~0.08% spurious chance), tagged `[I]`. Id 11: ~12.6M candidates tried, no hit, genuinely
`[U]`; optional, so it does not block writing a file.

Constraints on the interesting options `[C]`:

- **`signon_url`: maximum 2 occurrences.** At vaddr `0x1dcbd3`: `cmp eax,0x2; jl ok` against the running
  count; a third occurrence errors with `maximum of %d signon url overrides is supported`. This is the
  signon URL override the project needs, and it is optional. `[U]` The expected value syntax (bare host?
  full URL with scheme? path?) has not been determined; the option-6 value parser is the next read.
- `application_password_signature`: exactly 40 characters. At vaddr `0x1dce5a`:
  `strnlen(value, 0x29); cmp eax, 0x28`. `[I]` 40 hex chars = a SHA-1 digest.

### 7.6 The blocker: an RSA-2048 signature, enforced unconditionally `[C]`

The signature validator is at vaddr **`0x1dc0b0`**. Its caller (vaddr `0x1dbe87`) invokes it
unconditionally - no debug flag, environment check or skip branch:

```
1dbe87:  call 0x1dc0b0            ; validate_signature(file, &errcode)
1dbe8c:  test al,al
1dbe8e:  je   0x1dbee2            ; -> log "failed to validate signature", abort the load
1dbe90:  ...                      ; only now is the config read/parsed (0x1dc480)
```

The validator hashes the payload and calls the verifier at vaddr **`0xeef90`**:

```
[0:4]  little-endian u32 must == 1     (0xeefc8..0xeefec)
[4:8]  little-endian u32 must == 0     (0xeff2..0xef011)
hash   SHA-256 over the 176 payload bytes
       hash descriptor @0x19f9060 -> name "sha256", hashsize 0x20, blocksize 0x40
verify libtomcrypt rsa_verify_hash_ex(
           sig      = header + 8,   siglen = 0x100 (256)   -> RSA-2048
           hash     = digest,       hashlen = 0x20 (32)
           padding  = 3,            [I] LTC_PKCS_1_PSS
           hash_idx = sha256,       saltlen = 0x10 (16)
           &stat, key)
       accepted iff stat == 1       (0xef107)
```

`[C]` The hash is taken over the still-encrypted payload, not the plaintext - the signature covers
ciphertext. Public key from a table at vaddr `0x25ab528`, populated at runtime by the registrar at
vaddr `0xeedd0` (`register_key(blob, len)`; rejects `len < 0x81`; accepts at most one key). `[U]` The
key blob itself was not extracted.

Consequences:

1. `[C]` A modified `sku_config.txt` cannot be signed. Forging an RSA-2048 PSS signature requires
   Bungie's private key. This is the intended check, not a weak one.
2. `[C]` The plaintext path does not help. The loader sniffs for an unobfuscated file (compares the
   first 7 bytes of payload against literal `"version"` at vaddr `0x160095f`, `0x1ddb8e`-`0x1ddbb2`, and
   skips decryption on a match, logging `using plaintext sku config`) - **but that sniff happens
   downstream of `0x1dc0b0`**, on an already signature-checked payload. A plaintext config still needs a
   valid signature over its bytes.
3. `[I]` Therefore "point the client at our signon server by editing `sku_config`, no code patching"
   does not work on unmodified code. The feature exists and its option name is known (`signon_url`,
   `<=2` occurrences, optional), but the file carrying it is signed.

Open, in rough order of promise:

- `[U]` Intervene in the emulator instead of the file. shadPS4 is ours; hooking the file read or forcing
  `0x1dc0b0` to return true is an emulator-side change, keeps the signed retail file intact, and is
  very likely the cheapest real path. Not yet tried.
- `[U]` Does a failed load fall back to defaults, or is it fatal? Strings
  `sku config successfully read, but is invalid` and `failed to read sku config for reason (%d)`; error
  codes 1 and 5 set at `0x1dbed6` / `0x1dbeca`. If a built-in default sku config exists, its contents
  matter. Not traced.
- `[U]` `signon_url` value syntax.
- `[O]` Neither `sku_config` we hold carries a `signon_url` line (checked both `dumps/base-CUSA00219/`
  and `dumps/patch-01.29-CUSA00219/`), so value syntax cannot be settled by example from the current
  estate. `[U]` A different region's SKU (EU/JP) or a dev/cert build is where a populated `signon_url`
  would live.

**`sku_config.txt` is NOT on the entitlement path** - see §9.5.

---

## 8. Entitlement - offer keys, two stores, `ownership_authority`

Built 2026-08-08 from the decompiled 01.29 eboot, descriptor tables, and two live runs. Provenance:
`d1-eboot controls` 22/22 PASS (build
`7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954`); `d1-oracle profile controls`
11/11 PASS; every `xref` cited ran all four reference kinds with four positive controls passing. Reach:
`static-client-inference` for control flow; `observed-live` for §9's timeline and offer-key set
(`runs/20260808-210401-AH-JOIN`, `runs/20260808-205039-LVL40-AH`, identical), which is
`synthetic_ancestor` x2. Two independent derivations agree (this and the parallel string+syscall-trace
derivation of run `20260808-210910-AH-RAWCODEC`).

### 8.1 The one-line answer

Entitlement is a set of 32-bit offer keys. The client builds that set in exactly two places and picks
between them with a single global, `ownership_authority` (`0x26A4A74`):

- `authority == 1` -> the SIGNON response's field 10 `id_list`. Set by `bootflow:content_check`. We
  control this and we send nothing.
- `authority != 1` -> the LOCAL set, built from the PS4 add-on-content APIs. Set to `2` by
  `bootflow:prepare_for_orbit`, and that is the value in force in orbit and for the whole activity
  ladder.

In orbit the owned-offer-key set is 100% locally determined by the emulator. No server message can add
to it. Measured live, our set is exactly `{0xE0000002 paid tier, 0xE0000001 full sku}` - the base game
is already granted; no expansion is, and none can be granted from our side of the wire in orbit. The
missing grant is an emulator fact: shadPS4's `sceAppContentGetEntitlementKey` returns 16 zero bytes and
the client discards every add-on whose key is not `0xExxxxxxx`.

### 8.2 An "offer key" is a `0xNxxxxxxx` tagged u32 `[C]`

`0x2B0940(key, mask)` classifies by high nibble: `mask 1` = zero, `2` = `0xD...`, `4` = `0xE...`,
`8` = `0xC...`, `0x10` = `0xB...`, `0x20` = `0xA...`. Every ownership site passes `mask 4`, so the space
in play is `0xE0000000..0xEFFFFFFF`. Two compiled-in literals:

| key | meaning | where named |
|---|---|---|
| `0xE0000001` | full sku (base game) | `0x59E3D0`; `0x79A380` `content_check::update`; `0x9E5E60` `cui::c_orbit_data_component::vfunc[14]` |
| `0xE0000002` | paid tier (PS Plus) | `0x59E3D0`; `0x79A380` |

These are the only offer-key literals in the 26 MB text segment (grep of the 47,304-file decompile: 3
files, 7 sites). Every other offer key must come from data.

### 8.3 The owned set has exactly two backing stores `[C]`

| store | address | written by | read by |
|---|---|---|---|
| **local** | `0x26A4970` (64 x u32) + count `0x26A4A70` | `0x59E3D0` only (the local enumeration); `0x59E990` zeroes the count | `0x59ECC0` (send), `0x59F250`, `0x59F2A0`, `0x59F4F0` |
| **signon** | `0x263D46C` (64 x u32) + count `0x263D56C` | `0x2A7090` (signon response field 10) and `0x2A88F0` (pre-reauth copy of the local set) | `0x2A8A40`, `0x2A8AA0`/`0x2A8AE0` |

`d1-eboot xref 0x26a4970` -> 7 references, 4 controls PASS; `xref 0x263d46c` -> 4 references. No third
store, and nothing in the BAP layer writes either one (§9.2).

### 8.4 The query - `0x59F250` `[C]`

```c
bool is_offer_key_owned(u32 key) {          // 0x59F250
    if (ownership_authority /*0x26A4A74*/ == 1)
        return signon_list_contains(key);   // 0x2A8A40 -> scans 0x263D46C[0..64)
    for (i = 0; i < count /*0x26A4A70*/; i++)
        if (local[i] /*0x26A4970*/ == key) return true;
    return false;
}
```

`0x59F2A0` ("is all owned content present locally?") splits on the same global the same way.

### 8.5 Where the local set comes from - the PS4, not the network `[C]`

`0x59E3D0` case 5, in full:

```c
for (h = dlc_enum_first(4, &it); h != -1; h = dlc_enum_next(&it)) {
    k = 0xB8170(h);                       // the add-on's offer key
    if ((u32)(k + 0x1FFFFFFF) > 1)        // i.e. k is NOT 0xE0000001 and NOT 0xE0000002
        local[n++] = k;                   //   "recording locally determined offer key 0x%X"
}
if (platform_user_is_paid_tier())   local[n++] = 0xE0000002;   // "(paid tier)"
trial = 0x295E30();
if (!trial)                         local[n++] = 0xE0000001;   // "(full sku)"
0x2F0D20(trial);                    // [0x266F558 + 0x104] = trial ? 1 : 2
count = n;
```

Suppliers:

- `0x295E30` -> `0x295F90` is
  `sceAppContentAppParamGetInt(0 /*SCE_APP_CONTENT_APPPARAM_ID_SKU_FLAG*/, &v); return v != 3;` =>
  "this is a trial SKU" iff the PS4 SKU flag is not `FULL` (3). In shadPS4 this returns
  `ORBIS_APP_CONTENT_APPPARAM_SKU_FLAG_FULL = 3` unconditionally
  (`src/core/libraries/app_content/app_content.cpp`), so the base game is granted and the trial flag is
  clear. Confirmed live: `recording locally determined offer key 0xE0000001 (full sku)`.
- The add-on walk is `c_dlc_enumerate_entitlements_task` (`0xBC0A0`):
  `sceAppContentGetAddcontInfoList` -> for each entry with `status == 0` (Installed),
  `sceAppContentGetEntitlementKey(label, key16)` -> the 16 bytes are hex-formatted and `strtoul`'d,
  then:
  ```c
  if ((parsed & 0xF0000000) != 0xE0000000) parsed = 0;   // DISCARD
  ```

Where the expansions die on shadPS4: `sceAppContentInitialize` scans
`<addon_dir>/<TITLE_ID>/*/sce_sys/param.sfo` and fills `entitlement_label` and `status` but never
`addcont_info[i].key`, which stays the all-zero default. `sceAppContentGetEntitlementKey` returns 16
zero bytes, the parse yields `0`, and the add-on is dropped - so even with the expansion PKGs installed,
D1 would still own nothing. `[C]` on the client side (discard test in the decompile); `[C]` on the
emulator side (the field is never assigned in the source).

---

## 9. Why entitlement is LOCAL in orbit, and the field-10 server path never sent

### 9.1 What happened live `[O]` `observed-live`

`runs/20260808-210401-AH-JOIN` and `runs/20260808-205039-LVL40-AH`, byte-for-byte the same ordering:

```
bootflow:enumerate_content_rights        authority 0 (initial)
    -> "requesting ownership refresh from bap"
    -> idle -> waiting_to_issue_local_entitlement_enumeration -> waiting_for_local_entitlement_enumeration
    -> "recording locally determined offer key 0xE0000002 (paid tier)"
    -> "recording locally determined offer key 0xE0000001 (full sku)"
    -> "swallowing a refresh request; we are not through signon"   (authority != 2)
    -> idle,  success = 1        => the bootflow state PASSES
bootflow:account_signin
bootflow:content_check                   "changing ownership authority to 1"   (SIGNON)
bootflow:package_registration            <- runs with authority 1, against a list we never sent
bootflow:bap_signin / investment_signin / profile_setup
bootflow:prepare_for_orbit               "changing ownership authority to 2"   (BAP/local)
    -> local enumeration again -> same two keys
    -> "sending purchased offers request"   (BAP 0x0015)
    -> "Received update purchased offers response"   (our empty 0x0016)
    -> idle
setup:orbit                              <- authority 2 for the rest of the session
```

The measured owned set is `{0xE0000002, 0xE0000001}` and nothing else. No DLC line ever prints.

`bootflow:enumerate_content_rights` is not a gate we are failing `[C]`. Handler
`c_world_controller_state_bootflow_enumerate_content_rights_handler`: on_enter `0x79B870` sets
authority 0 then requests refresh; update `0x79B8A0` returns while busy (`0x26A4A78 != 0`), on success
(`0x26A4A80`) requests state 8, else logs `local entitlement enumeration failed` and requests state 25
(cleanup, reason `0x8E` = `error.entitlement_enumeration_failed`). Because authority is `0`, `0x59ECC0`
takes the "swallowing a refresh request; we are not through signon" branch, which sets `success = 1`
unconditionally. It is a data-collection step, not a check. A gate that succeeds with an empty answer
looks identical to a gate never reached - which is why this sat invisibly in every run's boot log; it
was found only by starting from the operator's screen ("Buy Rise of Iron") and working backwards.

### 9.2 Can the server supply it? One channel, closed at orbit

**Signon response field 10 `id_list` IS the owned-offer-key list `[C]`** (closes §3.5's earlier `[U]`).
`0x2A7090` case 2 (signon success):

```c
memset(&signon.offer_keys /*0x263D46C*/, 0xFF, 0x100);
signon.offer_key_count /*0x263D56C*/ = 0;
0x2AC0E0(&signon_response, &n, &p);        // n = msg[+0x4680], p = msg + 0x4580   <- FIELD 10
for (i = 0; i < n; i++) signon.offer_keys[i] = p[i];
signon.offer_key_count = n;
```

That array is exactly what `0x2A8A40` scans for `is_offer_key_owned` when authority is 1. `d1-responder`
has never emitted field 10 - `signon_success()` writes fields 1, 2, 4, 5, 6, 12, 13, 14 and stops. So
during `content_check` -> `package_registration` -> ... -> `prepare_for_orbit` the authoritative owned
set is empty, including `0xE0000001`. The place it visibly costs us is `0x1E3A40`, the DLC/entitlement
container registration loop, per-container guard
`if (container[+0xC4] != 0 || is_owned_signon(container->offer_key))`.

**BAP `0x0016 update_purchased_offers_response` CANNOT grant `[C]`.** Dispatched by a two-instruction
thunk in `c_investment_bap_message_policy_client_to_bap_update_purchased_offers_request`'s vtable:

```asm
0x00560080  48 89 d7        mov  rdi, rdx        ; rdi = the DECODED response message
0x00560083  e9 b8 f4 03 00  jmp  0x59f540        ; ownership_manager::on_response
```

`0x59F540` in full: log `Received update purchased offers response`, then
`if (state != 0) { state = 0; success = 1; }`. It never dereferences `rdi`. The message name is honest -
`client_to_bap_update_purchased_offers` is the client TELLING the service what it owns (`0x59ECC0` packs
`count` + the 64-entry local array into a 0x128-byte request). Not a grant channel. Our empty
header-only reply is correct.

**No HTTP entitlement request at all `[C]` `observed-live`.** The eboot carries
`sce::Toolkit::NP::WebAPI::EntitlementsGroup` and URL templates
`/v1/users/me/entitlements?service_label=%u` and `/v1/users/me/entitlements/%s?service_label=%u`
(`0x16B8939`, `0x16B8987`), plus `&entitlement_type=service`. The client never calls them. Across both
recent runs the complete HTTP set is 2 x `/SignOn`, 2 x `/2.0/oauth/token`,
2 x `cdn.deadorbit.net/manifest.bin`, and shadNet session/friend/presence calls. Zero 404s. No
unimplemented entitlement endpoint is waiting.

### 9.3 The content manifest describes offers; it does not own them `[C]`

`0x2AFC40` / `0x2AFD80` (the "DLC container" table every ownership consumer resolves against) are the
CDN content manifest's `entries` array: `count = manifest_buffer + 0x4408`,
`element i = manifest_buffer + 0x4410 + i*0xF8`. An `Entry` is an offer descriptor: `+0x00` offer key,
`+0x09` a 49-byte product label, `+0xC0` product type, `+0xC4` a bool ("no entitlement required"),
`+0xC6` a 17-byte secondary label; it is what `0x59D150` hands to the PS-Store open call (`0x2B4C60` /
`0x2B4CE0`). Our served manifest declares no entries and no offers (top-level fields
`{3: 1033 packages, 5: 1 guid}`), so `0x2AFC40()` is 0 and `0x2B08B0(key)` returns NULL. Populating
`entries` would let the client name an offer and open a store page; it would not make the account own
anything. See design/content-and-packages.md for the manifest.

### 9.4 The trial popup is probably NOT entitlement - an honest, unresolved contradiction `[C]`/`[I]`/`[U]`

`0x9E5E60`'s orbit trial flag is `!is_offer_key_owned(0xE0000001)`, stored to `this+0x8E` (dirty bit
`0x80`). At orbit authority is 2, so it reads the local set, which provably contains `0xE0000001`
(`observed-live`, twice). This model predicts `this+0x8E == 0` ("not trial") - which contradicts the
operator's on-screen "Maximum trial level reached". Two readings survive, no banner resolves it:

1. `[I]` The on-screen text is not `+0x8E`; it is a `cui::s_activity_requirements_not_met_entry` (§10),
   an unlock-state failure wearing commercial wording. More likely, because the same screen shows a
   Venus popup and an unselectable Earth, which are unlock-flag symptoms.
2. `[U]` Something resets the local set between `prepare_for_orbit` and the render - but no writer exists
   for that in §8.3's census, so this would falsify the census.

The falsifier, one command: with the client in orbit, read `cui::c_orbit_data_component + 0x8E` (and
`0x26A4A70` / `0x26A4A74` / `0x26A4970` in the same process; a heap pointer needs `d1guest read --abs`).
`+0x8E == 1` => reading 2 and the census is wrong. `+0x8E == 0` => the trial text is §10's requirement
list and entitlement is not the operator's bug.

### 9.5 `sku_config.txt` is NOT on this path `[C]`/`[I]`

- The trial decision is `sceAppContentAppParamGetInt(SKU_FLAG)` - a PS4 system call (`0x295F90`),
  reached from `0x59E3D0` and nowhere else (`xref 0x295e30` -> 1 reference). It reads no file.
- The owned-offer-key stores have exactly the writers in §8.3 (`xref`, four kinds, four controls PASS).
  The sku_config reader lives at `0x1DB000`-`0x1DE600`; none of its functions appears in either writer
  set.
- `sku_config`'s own effect is observable and unrelated: `networking:sku_manager: finalizing
  sku_type=2 (type_string=north_america_digital), UI=1117, audio=1117` and
  `base content version: 52553-6-2940354` - languages and a content-version string.
- The `demo_mode` option (table id 9, FNV-1 `0xC07E999A`, §7.5) is optional and absent from both files
  we hold, so it is at its compiled-in default. `[I]` it cannot cause a symptom that appears with the
  file we ship.

=> The RSA-2048 signature on `sku_config.txt` is not blocking entitlement.

---

## 10. Consumers of the owned set `[C]`

| consumer | what it asks | effect |
|---|---|---|
| `0x9E5E60` `cui::c_orbit_data_component::vfunc[14]` | `busy ? 0 : !is_offer_key_owned(0xE0000001)` -> stored to `this+0x8E`, dirty bit `0x80` | the orbit screen's TRIAL flag |
| `0x54F150` | for an investment definition's offer key at `def+0x10`: `!busy && !is_offer_key_owned(k) && 0x2B08B0(k)` -> `0x59D0E0(k, ...)` | the "Buy X now" store flow. Callers: `0x9785C0`, `cui::c_investment_vendor_wares_component::vfunc[14]`, `cui::s_investment_material_requirement` |
| `0x1E3A40` | `is_owned_signon(container.offer_key)` | registers / skips a DLC content container (authority 1, i.e. the signon list) |
| `0x7916E0` case `0x10` | `0x59F2A0()` - "every owned key resolves in the content system" | a boot-error condition |
| `0x78CD50` | copies the local set into the signon manager before reauth | seeds the claim the server answers |

The trial text and the "Buy Rise of Iron now" text are not in the eboot (grep for `trial`, `Trial`,
`Rise of Iron`, `Pay to unlock` -> 0 hits each). They are content strings; the eboot names their
container, `s_investment_activity_unlock_string_client_definition` /
`...unlock_strings_label_client_definition`, selected by the two predicates above and by `0x9D92A0`.

The other half - requirements, which are unlock state, not ownership `[C]`. `0x9D92A0` is
`cui::c_static_properties_sequence<ui::c_shared_ptr<cui::s_activity_requirements_not_met_entry>, 4>`,
the builder of the on-screen "you can't do this because..." list, called by
`cui::c_orbit_data_component::vfunc[14]`. Its flag word at `this+0x20`:

```c
flags |= 0x1000;  if (0x54F410(activity)) flags &= ~0x1000;   // bit 0x1000 = "no unpurchased prerequisites"
light = 0x276280(); cap = 0x276220();
flags = (flags & ~2) | (cap  < light ? 2 : 0);
flags = (flags & ~4) | (light < required_light && required_light != -1 ? 4 : 0);   // bit 4 = under-light
...
for each requirement in {fireteam, leader} iterators:
    if (!0x28E4B0(req)) continue;              // not displayable
    if ( 0x28DE40(ctx, req)) continue;         // SATISFIED -> skip
    emit cui::s_activity_requirements_not_met_entry{ string_index, hash }   // max 4
```

`0x28DE40(ctx, req)` is `0x28DEA0(unlock_state + 4, req)` - it evaluates the requirement against the
family-4/type-4 unlock state (`0x8080102E`), served key-only. With no unlock flags set, every
requirement is unmet and the list fills with whatever four strings the content names first. See
design/queuez-replication.md and design/character-and-investment.md for the unlock/investment state.
Re-ranking, honestly: the director symptoms (only Earth, Earth not selectable, a Venus popup) and most
requirement text are UNLOCK-FLAG problems, not entitlement problems. Entitlement explains the expansion
upsell cleanly and nothing else cleanly.

### 10.1 If we ever want a real expansion grant, ranked by how much is ours

1. **shadPS4's `addcont_info[].key`.** The add-on scan already reads each DLC's `param.sfo`; it never
   sets `key`. Populating it (from the SFO, or a `D1_*` override) is the only route that reaches the
   orbit-time owned set. `[U]` It needs the real 16-byte entitlement key, observable only from a retail
   console with the DLC installed, or a retail content manifest's `entries` array. Do not invent one:
   `(k & 0xF0000000) == 0xE0000000` is the only check, so a made-up key would be accepted and be
   `synthetic-generic` forever.
2. **Signon field 10 `id_list`.** Cheap (one `pb_bytes(&mut v, 10, ...)` in `signon_success`) and
   correct-by-construction, but only binds while `ownership_authority == 1` (content_check through
   prepare_for_orbit). Worth sending `[0xE0000001, 0xE0000002]` so `package_registration` stops running
   against an empty list; expect no visible change in orbit.
3. **Manifest `entries`.** Lets the client name offers and open store pages. Grants nothing.
4. **BAP `0x0016`.** Dead end, proven at `0x560083`.

### 10.2 The rung-33 `+0x6E4C` bypass (adjacent, not entitlement) `[C]`

The ladder gate the activity ladder actually trips at rung 33 is not entitlement. `0x7A7420` (rung 33's
single task): if `profile + 0x6E4C == 0` it runs the gate pair that produces
`0x17 activity_failed_to_load_missing_dlc` ("World-transition requires DLC.") and
`0x18 activity_failed_to_load_missing_permissions`; if the byte is set it falls straight into
`setup:activity_world_transition` ("Starting world-transition."). The field is pinned three ways:
schema profile field `[15]` @`+0x6CD0` -> `0x808018BB` (404 B), its field `[7]` @`+0x17C` kind `0x102`
(bool), `0x6CD0 + 0x17C = 0x6E4C`; field `[8]` @`+0x17D = 0x6E4D` also bool; 28 read sites across 31
functions; codec round-trips writing `1` to `+0x6E4C`/`+0x6E4D` but REFUSES `+0x6E4E` (the negative
that proves the offsets). It does not touch offer-key ownership (`0x59F250` has no `+0x6E4C` path).
Shipped as `d1-oracle profile build --all-content` (default false); it is a client-behaviour switch we
chose, so downstream is `synthetic_ancestor`. See design/character-and-investment.md and
d1-server/crates/d1-oracle/src/profile.rs.

---

## 11. Bungie.net public API as a data source (short)

Source: web-session proposal; time-sensitive. The public Bungie.net Platform API is REST/JSON over
HTTPS at `https://www.bungie.net/Platform`, with an API key and OAuth 2.0, built for third-party apps
(DIM, Ishtar, etc). **It is NOT the protocol the client speaks** - the console goes through Demonware
for auth and lobby and Bungie's own game-service path for the rest.

Verified on the 01.29 eboot 2026-08-06, confirming §1 of the proposal by measurement:

| Checked | Result |
|---|---|
| `/Platform/` routes in the eboot | 0 |
| `https://` strings, total | 3: `https://www%d.%s/%s`, `https://auth.api.%s.ac.playstation.net/2.0/oauth/token`, `https://graph.facebook.com` |
| `bungie.net` literals | 2, both non-API: `www.bungie.net`, `help.bungie.net` |

The single templated URL `https://www%d.%s/%s` is consistent with the `www1-8.signon.*` hostnames (§1),
i.e. Bungie's own game-service path, not the public API. Treat every API endpoint shape as a hint about
Bungie's internal data model, never a wire specification.

What of the proposal is still relevant:

- **The manifest as a hash Rosetta stone.** The manifest is a set of compressed SQLite databases of
  static hash-keyed definitions (activities, items, classes, vendors, progression) via
  `GetDestinyManifest`, in seven languages. Our dumps are also hash-keyed. Known name<->hash pairs let
  us hash names with candidate functions (FNV-1a 32 and variants first, with and without case
  normalisation) and look for collisions - a hit identifies the hash algorithm AND hands us a large
  rainbow table. `[U]` Worth an attempt: manifest assets are content-hashed URLs under
  `/common/destiny_content/sqlite/`; historical paths from forums/Wayback may still resolve. Era
  mismatch: the live manifest is final-state D1 (Age of Triumph, 2.9.x); we target 2.4.1 / 01.29. The
  manifest's job is decoder, not authority - where they disagree, our dump wins.
- **The deadline `[C]`.** The D1 API is legacy and unsupported, works by inertia.
  `ReadDestinyVendorsAndAdvisors` is marked OBSOLETE - D1 only; `ReadDestinyInventoryAndVault` reads D1
  inventory. April 2025: D1 apps (DIM, Ishtar Legacy) broke returning only equipped gear; filed as
  `Bungie-net/api` #1960, Bungie unsure of cause. An unmaintained surface investigated by people who do
  not fully understand it is one deploy from silence.
- **Where the D1 docs live.** `platformlib` (Bungie's own machine-readable enumeration of the legacy
  surface, referenced by the BungieNetPlatform changelog; URL not yet found);
  `DestinyDevs/BungieNetPlatform` (community wiki + git repo, the real D1 API archive - clone with full
  history); a community D1 OpenAPI spec (`yogarine/bungie-d1-sdk-php` is OpenAPI-Generator output
  crediting DestinyDevs); forum threads as era-matched primary sources; `Bungie-net/api` for the
  D1->D2 removal diff and residual enums.
- **B2 shape hints `[I]`:** vendors/advisors were a distinct service surface (obsolete D1-only scope);
  inventory and vault shared one scope; the definition/instance split (manifest holds static
  definitions we can have rather than reimplement).
- **Safety:** ordinary API-consumer behaviour (registered app key + Bungie.net account), PSN-safe lane
  only, our own account only, rate-limit politely.
- **Non-goals:** not implementing the public API; not scraping other players' data; not treating API
  shapes as wire specification.

Note: D1's auth and lobby are Demonware, so Layer B splits - B1 Demonware, B2 Bungie game services;
this proposal is entirely about B2. See design/architecture.md.

---

## 11b. Signon staging of the registry ready byte `[C]`

Salvaged 2026-08-11 while retiring `historicaldocs/ingress/`. **`0x5859B3`** stages the registry ready byte during
signon, and the distinction matters: it **copies an existing value and does not originate it**. The
sole originating writer is `0x585820`, and only delivering the account object sets it
(`design/character-and-investment.md`). A session that finds `0x5859B3` by xref and treats it as the
writer will chase the wrong site. Reach `static-client-inference`; source
`historicaldocs/ingress/round3/inventory/catalogs/KEY-ADDRESSES.md`.

## 12. Open [U] gates

- `[U]` Semantics of response `f2.1`, `f2.2`, `f2.3` (three 16-byte blobs). `f2.2`/`f2.3` are handed
  out with the security token by the accessor at `0x2ab9a0`+; `[I]` key material for the BAP
  `secure_hello` handshake (cf. `c_investment_bap_message_client_to_bap_secure_hello_request`,
  `cookie failed HMac test.`), but the accessor's callers were not traced. `f2.1` is genuinely unread
  in this build. 16 zero bytes for all three likely suffice for a first-light test, but it touches auth,
  so it needs unconditional confirmation.
- `[U]` `f2.9`'s three floats (stored to `this+0x41e4/41e8/41ec`); readers not chased.
- `[U]` `f8` bit meanings beyond "bits 2/5/6 round-trip into the reauth request".
- `[U]` `f3.1`, `f3.3`, `f3.6`, `f3.7` and `f7.2`, `f7.5` in the queue/denial payloads. Only relevant
  if those paths are deliberately exercised.
- `[U]` Most of the request's optional fields (3-21). Fields 20 and 21 are shaped like
  telemetry/hardware-info blocks.
- `[U]` Codec field types 8-14 (`0xa36e0`...`0xa3930`). Unused by both signon tables.
- `[I]` Whether the stream error slot is read anywhere. `0x2a99d0` confirmed to discard the parse return
  value; not exhaustively proven no other path inspects `stream+0x18`.
- `[U]` `signon_url` value syntax (bare host? full URL with scheme? path?); the option-6 value parser is
  the next read. Neither `sku_config` we hold carries a `signon_url` line, so it cannot be settled by
  example from the current estate.
- `[U]` Does a failed `sku_config` load fall back to defaults, or is it fatal? Contents of any built-in
  default sku config.
- `[U]` sku_config table id 11 (hash `0xed58a8a0`): no preimage found.
- `[U]` The expansion offer keys themselves. They exist only in data we do not hold - a retail
  entitlement key or a retail content manifest's `entries` array. `0xBC0A0` hex-formats
  `sceAppContentGetEntitlementKey`'s 16 bytes and requires `0xExxxxxxx`, so the value is knowable from
  one retail console read and nothing else here.
- `[U]` Whether `+0x6E4C` changes anything on screen. No run has been made with it set.
- `[U]` The exact `platformlib` URL; whether historical manifest content paths still resolve; whether
  the community D1 OpenAPI spec is still reachable; whether Software Heritage has ingested the community
  repos.
- Virtual-dispatch consumers of `0x59F250`/`0x2A8A40` are invisible to the direct-call callgraph
  (`eboot-callgraph.tsv` is direct `call`/`jmp` only); both return zero `xref` hits despite having
  callers (tail-merged/register-indirect). `[U]` for a specific edge is a tail-merge signal, not an
  absence.

---

## 13. Dead claims

Each retracted or corrected claim, in one line, with what killed it.

- **"The entitlement decision is local and `d1-server` was never on this path" / "`d1-server` cannot fix
  this and never could. No request for entitlements ever reaches it."** (historicaldocs/design/entitlement-is-local.md §0)
  Killed by historicaldocs/design/entitlement-and-content-rights.md §3.1: SignOn response field 10
  `id_list` IS a server channel and binds while `ownership_authority == 1`
  (`content_check` through `prepare_for_orbit`). The headline holds only for orbit; the server is on the
  path before `prepare_for_orbit`.
- **"Install the additional-content packages so `sceAppContentGetAddcontInfoList` reports them - this is
  the honest route."** (historicaldocs/design/entitlement-is-local.md §6 item 1) Killed by
  historicaldocs/design/entitlement-and-content-rights.md §1.4 (self-corrected in
  historicaldocs/design/entitlement-is-local.md §5bis item 2): shadPS4 never populates
  `addcont_info[].key`, so `sceAppContentGetEntitlementKey` returns 16 zero bytes and the client
  discards every add-on whose key fails `(key & 0xF0000000) == 0xE0000000`, even with the PKGs present.
  The emulator change is required, not an alternative.
- **"'Maximum trial level reached' / the trial popup is an entitlement symptom."**
  (historicaldocs/design/entitlement-is-local.md §5) Killed by the live owned set `{0xE0000001 full sku,
  0xE0000002 paid tier}` (observed-live, twice): the base game is granted, so `0x9E5E60`'s trial flag
  computes to 0 = not trial. Re-ranked as unlock state (`cui::s_activity_requirements_not_met_entry`
  from `0x9D92A0`, family 4/4). One live read of `c_orbit_data_component + 0x8E` settles it; the
  contradiction is left open in §9.4.
- **"The director symptoms (only Earth, Earth unselectable, Venus popup) are entitlement problems."**
  (historicaldocs/design/entitlement-is-local.md §1/§5) Killed by
  historicaldocs/design/entitlement-and-content-rights.md §5: they are unlock-flag (family 4/4)
  problems, evaluated by `0x28DE40 -> 0x28DEA0` against the family-4/type-4 unlock state; entitlement
  cleanly explains only the expansion upsell.
- **"D1 endpoint definitions are in `Bungie-net/api` git history."** (an earlier claim, corrected in
  historicaldocs/design/bungie-api.md §4) Killed: that repo appears created for D2 (README frames
  Components as a new D2 concept; OpenAPI extensions are D2-shaped). Bungie never published a formal D1
  spec; the D1-era API was documented by the community.

Note (not a dead claim, a resolution): SignOn response field 10's meaning was tagged `[U]`
("entitlement / experiment / content-rights ids") in historicaldocs/design/signon-protocol.md §4.5. That
guess was CONFIRMED correct - it is the owned-offer-key list (§3.5, §9.2). Carried forward as `[C]`.

---

## 14. Sources

| Melted doc (now under historicaldocs/design/) | What was taken |
|---|---|
| historicaldocs/design/signon-protocol.md | §1 transport + HTTP status; §1.3-1.6 codec, descriptor layout, relocations, parser behaviours; §2 request schema; §3 response schema and all sub-field tables (success/queue/client_config/denial/id_list/flags/IP encoding); §4 discriminator + state enum; §5 minimum viable body, traps, `.proto`; §6 live confirmation. Field 10's `[U]`, now closed. |
| historicaldocs/design/sku-config-format.md | §7 in full: file layout, AES-128-CBC obfuscation (key/IV/call chain/libtomcrypt), decrypted plaintext, decrypt/re-encrypt recipe, FNV-1 option table and `signon_url` override, the RSA-2048 signature gate and its open items. |
| historicaldocs/design/entitlement-and-content-rights.md | §8 offer keys, two stores, `ownership_authority`, local-set derivation; §9.2-9.3 server channels (field 10, BAP 0x0016, HTTP, manifest entries); §9.4 the honest trial-flag contradiction; §9.5 sku_config not on path; §10 consumers, unlock-vs-ownership, expansion-grant ranking, and the rung-33 `+0x6E4C` bypass. Authoritative where it corrects entitlement-is-local. |
| historicaldocs/design/entitlement-is-local.md | §9.1 the live boot arc and the "gate that succeeds with an empty answer" lesson; the shadPS4 `addcont_count = 0` / `app_content.cpp` source read. Its §0/§5/§6 headline claims are retracted in §13. |
| historicaldocs/design/bungie-api.md | §11 in full: the API-is-not-the-wire trap (verified by eboot string counts), the manifest as a hash Rosetta stone, the legacy-surface deadline, where D1 docs live, B2 shape hints, safety and non-goals. Its "Bungie-net/api git history" claim is retracted in §13. |
