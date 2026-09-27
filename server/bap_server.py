"""Loopback Destiny BAP (Bungie session server) stand-in: answers the session hello.

After content registration the client connects to the address from the SignOn reply
(127.1.1.127:12850) and sends `shello req`. Framing, from BLES01857 01.00:

  outer   u8 1 (version), u8 kind, u32be length of the rest            (0x7c413c)
          kind 2 = plaintext; kind 1 = encrypted with the shello session key (0x7c3fbc)
  header  kind 1 (client->server): u16be type, u32be id                (size 6, 0x81684)
          kind 2 (server->client): u16be type, u32be id, u16be status  (size 8, 0x82e30)
          The message codec's method 1 picks the header kind (0x41501c table).
  body    message payload.

Message names come from 0x7c6ef4: 0x19 "shello req", 0x1a "shello rsp", 0x12/0x13 client
config request/response, and so on. shello req (descriptor 0x11f3e00) is protobuf
{1 bytes SignOn token, 2 int32 mode, 3 optional uint64, 4 optional bool} (0x7c648c).

shello rsp (codec 0x41676c) is u32be N (N <= 0x5c) followed by N bytes, sealed as in
0x7c7794 and opened by 0x7c7660:
  u32be N | IV[16] | AES-128-CBC(key, IV, plaintext + PKCS#7) | HMAC-SHA256(mac_key, u32|IV|ct)
The AES key is SignOn session key 2 and the MAC key session key 3 (0x7c75f8). The
plaintext is nonce[12] || session_key[16] (0xb0cd30); the reply status must be 200.

Kind 1 frames (after shello) are AES-128-GCM with that session key, no AAD, 16-byte tag
first: tag[16] || ciphertext(inner header || payload) (0xb0ce4c / 0xb0cf58, 0x7c743c).
Each direction has its own 12-byte nonce: the client sends with the reply nonce's last byte
XOR 1 and receives with the nonce unchanged (0xb0cd30 with direction 0); after each message
the nonce is incremented as a little-endian counter from byte 0.

Encrypted requests seen so far: 0x79 "qz_reg req" (empty payload), then 0x12e "rrc req"
(40 bytes of address data: LAN IPv4, port 3074 little-endian, masks, the STUN-reported
address and a u64). Their replies have header kind 2 and decoders that ignore the payload.
Then 0x0a "c->ws req": a web-service call (codec 0xb4e208/0xb4e444) encoded with the
engine's bit-packed type serializer: 16-bit service id (503 = 0x01f7 in the first call),
the service's request structure, then a flag bit for an optional blob. The reply 0x0b
uses the same layout with the service's response type (descriptor +0x10). Without the
type definitions, --ws-zero-bytes answers with the service id followed by zero bytes,
which deserializes as an all-zero response structure and no blob.
"""

import argparse
import datetime
import hashlib
import hmac
import ipaddress
import itertools
import json
import secrets
import socket
import struct
import sys
import threading
import time
from pathlib import Path

from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

sys.path.insert(0, str(Path(__file__).resolve().parent))
import investment  # noqa: E402
from local_paths import SIGNON  # noqa: E402
import signon_proto  # noqa: E402

# Root object types per family (object pools at mgr+0x2a3c0 in 0x41a0d0's manager):
# family -> (type hash, type handle). The self (4) root object is what the sign-in step
# looks up (0x293108); select_character (3) type 0 is its roster header.
ROOT_OBJECTS = {4: (0x88190FE1, 0x80801803, lambda soid: {0: {0: soid}}),
                3: (0xA2AC2C74, 0x80801951, lambda soid: {0: soid})}
SELECT_CHARACTER_OBJECT = (0xF65ABD52, 0x80801954)
SELF_CHARACTER_OBJECT = (0x242B2F61, 0x80801743)
# Peer family (0): up to 20 peers, kind 0 = account summary keyed by the account soid (+8 = its
# character soid), kind 1 = character summary (id, class, appearance like the self character).
# The player manager's character cache (0x2998e0) builds a player's appearance and blueprint
# (player record +0x8b0 / +0x1428, needed to spawn) from these two objects.
PEER_ACCOUNT_OBJECT = (0x1D7BB8F6, 0x80801921)
PEER_CHARACTER_OBJECT = (0xA0A1BC80, 0x808017FD)


def default_ws_response(schema, wire, values=None):
    """Service reply with its response type encoded by default (result error_none, other
    fields absent or zero) and no blob; None if the service is not in the schema."""
    entry = (schema or {}).get("services", {}).get(str(wire))
    if not entry:
        return None
    w = investment.BitWriter()
    w.put(wire, 16)
    investment.encode_into(w, schema, int(entry[1], 16), values)
    w.put(0, 1)  # no blob
    return w.data()


def character_from_request(schema, character_id, request_payload):
    """Profile of a created character: its id plus the class and appearance structs of the
    service-501 request (request offsets 0 and 4 -> character offsets 8 and 0xc)."""
    profile = {0: character_id}
    request_handle = int(schema["services"]["501"][0], 16)
    if f"{request_handle:#x}" in schema:
        request = investment.decode(schema, request_handle, request_payload)
        profile.update({8: request[0], 12: request[4]})
    return profile


def load_store(path):
    try:
        return json.loads(path.read_text(encoding="utf-8")) if path else {}
    except FileNotFoundError:
        return {}


def save_character(path, account_id, character_id, request_payload):
    """Remember one created character per local account (JSON; kept outside Git)."""
    store = load_store(path)
    store[f"{account_id:#x}"] = {"id": character_id, "request": request_payload.hex()}
    path.write_text(json.dumps(store, indent=1), encoding="utf-8")


def parse_flag_spec(spec):
    """"2" (every flag) or "2@A0-511,B0-15" - value, then index ranges in account table A
    (+0x12a8, 1024 flags), account table B (+0x16a8+1, 256 flags) and character table C
    (self character object +0x3bc0, 1024 flags)."""
    value, _, ranges = str(spec).partition("@")
    out = {"A": set(), "B": set(), "C": set()}
    if not ranges:
        out = {"A": set(range(1024)), "B": set(range(256)), "C": set(range(1024))}
    for part in filter(None, ranges.split(",")):
        table, span = part[0].upper(), part[1:]
        lo, _, hi = span.partition("-")
        out[table].update(range(int(lo), int(hi or lo) + 1))
    return int(value), out


def account_flag_values(spec):
    """Layered specs separated by ";" - later ones win, e.g. "1;2@B0" = every flag false
    (1) except B0 true (2). Returns {"A": {index: value}, "B": {...}}."""
    values = {"A": {}, "B": {}, "C": {}}
    for part in filter(None, str(spec).split(";")):
        value, chosen = parse_flag_spec(part)
        for table, indexes in chosen.items():
            values[table].update(dict.fromkeys(indexes, value))
    return values


def account_flags(schema, spec):
    """Self root account flag tables, read by the unlock-expression evaluator (0xb674ec /
    0xb67564). Values are tri-state (the evaluator writes 2 for true). Setting all to 2
    unlocks destinations but also trophies, message-of-the-day and restriction flags."""
    values = account_flag_values(spec)
    def slots(handle, table):
        return {f["off"]: values[table][i] for i, f in enumerate(schema[handle]["fields"])
                if i in values[table]}
    return {0x12a8: slots("0x80804595", "A"),
            0x16a8: {0: False, 1: slots("0x808047d8", "B")}}


def family_blob(schema, family_type, root, character_id=None, character_profile=None,
                version=1, flags=1, active_id=None, self_flags=None, peer=False):
    """qz payload for one family. For self (4), `active_id` fills root +0x60, the active
    character: the user manager copies it to its entry +0x20 (0x2942fc) once a self
    character object with that id exists (0x29305c), and the bootflow's character step
    (0x32f748 via 0x325350) proceeds only when it is set."""
    objects = []
    if schema and family_type in ROOT_OBJECTS:
        type_hash, handle, values = ROOT_OBJECTS[family_type]
        fields = values(root)
        if character_id is not None and family_type in (3, 4):
            character_list = {0: 1, 8: [character_id]}
            if family_type == 3:
                fields[8] = character_list
            else:
                fields[0][8] = character_list
                if active_id is not None:
                    fields[0][0x60] = active_id
        if family_type == 4 and self_flags is not None:
            fields[0].update(account_flags(schema, self_flags))
        objects.append(investment.obj(type_hash, root, investment.encode(schema, handle, fields)))
        if family_type == 3 and character_id is not None and character_profile is not None:
            type_hash, handle = SELECT_CHARACTER_OBJECT
            objects.append(investment.obj(type_hash, character_id,
                                          investment.encode(schema, handle, character_profile)))
        if family_type == 4 and character_id is not None and character_profile is not None:
            type_hash, handle = SELF_CHARACTER_OBJECT
            profile = dict(character_profile)
            if self_flags is not None:
                chosen = account_flag_values(self_flags)["C"]
                profile[0x3bc0] = {f["off"]: chosen[i] for i, f in
                                   enumerate(schema["0x80804595"]["fields"]) if i in chosen}
            objects.append(investment.obj(type_hash, character_id,
                                          investment.encode(schema, handle, {0: profile})))
    if peer and schema and family_type == 0 and character_id is not None and character_profile is not None:
        type_hash, handle = PEER_ACCOUNT_OBJECT
        objects.append(investment.obj(type_hash, root, investment.encode(schema, handle, {0: root, 8: character_id})))
        type_hash, handle = PEER_CHARACTER_OBJECT
        summary = {off: character_profile[off] for off in (0, 8, 12) if off in character_profile}
        objects.append(investment.obj(type_hash, character_id, investment.encode(schema, handle, summary)))
    # flags bit 0 = full snapshot: required for a new subscription (0x41b75c, slot state 2).
    # Updates to an existing subscription use a delta and the next family version.
    return investment.qz_payload([investment.family(family_type, root, objects,
                                                     version=version, flags=flags)])

CAPTURE_DIR = SIGNON / "captures" / "bap"
SHELLO_REQ, SHELLO_RSP = 0x19, 0x1A
QZ_REG_REQ, QZ_REG_RSP = 0x79, 0x7A
ECHO_REQ, ECHO_RSP, RRC_REQ, RRC_RSP = 0xFA, 0xFB, 0x12E, 0x12F
WS_REQ, WS_RSP = 0x0A, 0x0B
SUB_REQ, SUB_RSP = 0x0C, 0x0D
QZ_UPDATE = 0x7B
ACCT_ID_TRANS_REQ, ACCT_ID_TRANS_RSP = 0x17, 0x18
CLIENT_CONFIG_REQ, CLIENT_CONFIG_RSP = 0x12, 0x13
# Activity host request (sent when joining an activity): 8-byte request; the reply decoder
# (0x416f14) copies exactly 16 raw bytes. Their meaning is still unknown.
AHP_REQ, AHP_RSP = 0x10, 0x11
# b->c notification push (type 9; decoder 0x416064): u8 flag, u64 id (flag 1), u32 subtype,
# u32 length, data. Subtype 1 appears to reach the ahp consumer 0x42a5a0 (16 bytes).
BC_NOT = 0x09
# Activity host proxy connection (address from the ahp reply): the client sends
# c->ah mgr req (0x06, 3103 bytes: u8 subtype + body); the reply c->ah mgr rsp (0x07) must be
# exactly 137 bytes, subtype 2 + 136 bytes (decoder 0x415be4). Contents still unknown.
AH_MGR_REQ, AH_MGR_RSP = 0x06, 0x07
# Client -> activity host message relayed by the proxy: u64 host id | u8 1 | u32 type |
# u32 length | u32 0 | body (type 3 = host join request carrying account and character ids).
AH_NOT = 0x08
# Activity host messages are typed structs in the investment bit format (0x25bd0c /
# 0x25bc88). Client -> host types come from the table at *(0x179eba4), host -> client types
# from *(0x179eba8); the schema is exported with scripts/rpcs3_tiger_types.py (outside Git).
# Join: the client sends c->ah not type 3 with a 0x80800e98 struct and keeps a copy
# (0x259c84; its first i32 is a random token). The host answers with b->c not type 4 (handler 0xd0d560 -> 0x256ad8), a
# 0x80800a58 struct: @0 the request echoed (its first i32 must match the copy), @0xf8 a
# result (0 = joined, stored as bits - 1 in 3 bits), @0xf9 char[128] activity session id,
# then i32s @0x17c..0x188, u8 @0x18c, u16s @0x18e/0x190 (kept at 0x6930/0x6934), i32 @0x194.
# Next the client waits for its entity indices: b->c not type 0 (handler 0xd0d020), a
# 0x80800d64 struct holding a 384-word bitmap (0x808043a7) that is ORed into the client's
# index set (+0x5bf8) and marks the indices received (+0x61f8).
# The client counts as established (0x258324) once it also has host flag 0x100 (type 11,
# body ignored, handler 0x25715c) and an activity state: type 1 (handler 0xd0d250), a
# 0x80800e76 struct whose 0xa20-byte body is copied into the simulation (0x1c95f0) and must
# have bool @0 set and either bool @1 clear or i32 @0x10 > 0. Loading the activity then
# needs the map name (0x305a2c): state @0x60 (0x80800cc5) @0x994 (0x8080409f) = char[40],
# looked up as a resource key (0x1f6564, e.g. "orbit"; ":scenario_fah" etc. for parts).
# The world id is the i16 at state @0x60 @0x990 (0x3254ec; checked for DLC by 0x4105c4,
# "World-transition requires DLC."); service 105 carries the world the client asked for.
AH_JOIN_REQUEST, AH_JOIN_RESPONSE, AH_ENTITY_INDICES = 3, 4, 0
AH_STATE, AH_ESTABLISH = 1, 11
STATE_TYPE = 0x80800E76
JOIN_REQUEST_TYPE, JOIN_RESPONSE_TYPE = 0x80800E98, 0x80800A58
ENTITY_INDICES_TYPE, ENTITY_WORDS = 0x80800D64, 384
ACTIVITY_SCHEMA = SIGNON / "activity_schema.json"


def char_array(text, size=128):
    """A char[size] field (type 0x80804056 -> 0x80804008: size i8 fields, one per byte)."""
    data = text.encode("latin1")[:size - 1]
    return {0: dict(enumerate(data))}


def join_request(schema, body):
    return investment.decode(schema, JOIN_REQUEST_TYPE, body)


def entity_indices(schema, first, last):
    """Bitmap granting entity indices first..last-1 (bit order inside a word is unverified,
    so grant whole words: multiples of 32)."""
    words = [0] * ENTITY_WORDS
    for index in range(first, last):
        words[index // 32] |= 0x80000000 >> (index % 32)
    return investment.encode(schema, ENTITY_INDICES_TYPE, {0: {4 * i: w for i, w in enumerate(words)}})


def activity_state(schema, map_name=None, world=None, checksum=None, bubbles=None, zone=None,
                   bubble_states=None):
    values = {0: True, 0x10: 1}
    if zone is not None:
        # u32 @0x54 object index / i32 @0x58 name hash: the zone record the world controller
        # adopts on arrival (0x31d778 via activity object +0x50b0, slice set +0x130). The leave
        # check (0x31bfe0) then requires that object in the scenario object table; index -1
        # (hash 0x811c9dc5, FNV of "") is the game's default and skips the check.
        values[0x54], values[0x58] = zone
    if bubbles:
        # i32 @0x10 = bubble count (must equal the scenario's, def +0x30; 0x1c7608 "activity
        # client bubble count mismatch"), i8[64] @0x14 = per-bubble state index. The sim start
        # (0x1c7608) registers bubble i's zone objects in the scenario object table from it:
        # -1 leaves the base entry (index 8*i) absent, 0.. registers the bubble's base entry and
        # its sub-states (8*i + state + 1, 0x14a8e8); zone sets load only from present entries.
        states = dict(bubble_states or {})
        default = states.pop(None, -1)
        values[0x10] = bubbles
        values[0x14] = {i: states.get(i, default) for i in range(bubbles)}
    if checksum is not None:
        # u32 @0x5c: the activity checksum; 0x258550 compares it with the loaded scenario's
        # ("activity checksum mismatch" fails the world load).
        values[0x5C] = checksum
    scene = {}
    if map_name:
        scene[0x994] = {0: char_array(map_name, 40)[0]}
    if world is not None:
        scene[0x990] = world
    if scene:
        values[0x60] = scene
    return investment.encode(schema, STATE_TYPE, {0: values})


WORLD_STATE_TYPE = 0x808009ED
ROSTER_TYPE, ROSTER_SLOTS = 0x80800CF7, 65
AH_SIMULATION = 5
OBJECT_ID_TYPE, SIM_PLAYER_TYPE, GAME_MODE_TYPE = 0x808004B1, 0x808022A5, 0x808008A3
AH_MACHINES, MACHINES_TYPE = 43, 0x80800F1A


def machine_list(schema, machine_ids):
    """Host type 43 (0xd0e214): {5-bit count, up to 20 8-byte machine ids} - a **boot list**.
    Each listed id sets the flag of the matching entry of the host's machine list (host
    +0x6204 count, 10-byte records at +0x6208 = id + flag); the per-host update (0x30956c ->
    0x25afcc) then moves that machine to state 4 and the client is removed from the game
    world (error bass, TEST176)."""
    return investment.encode(schema, MACHINES_TYPE, {0: {0: len(machine_ids), 4: list(machine_ids)}})


def sim_packet(schema, peer_id, slot=0, generation=1, epoch=0, tick=0, objects=()):
    """Host type 5 (0x25b920 -> session receive -> event 0x13 -> 0x1c85e0): u8 epoch, roster
    present bit, 65 changed-slot bits, roster delta (present bit + type 0x80800cf7: 65 slots of
    {opt 8-byte peer id, opt u16, bool active}; changed slots add/remove peers via 0x1c8280),
    u32 host tick, then the replicated object stream (0x1ce4a4; only read when the session's
    simulation is active and the epoch matches): a start bit (0 = no objects), then per object
    a 1 bit, its id (type 0x808004b1), the u32 bit length of the rest (used to skip unknown
    objects), state A (reset bit + present bit + the record's +0xc type) and, when the record has
    a state B type (+8), a 0 bit for it; a 0 bit ends the list. `objects` holds (id values,
    state A type or None for a default baseline, state values, has state B). Replicated state is
    applied only once every object of the active bubbles has had a baseline (`0x1ce1b8`: record
    +0x15), so all of them must be listed."""
    w = investment.BitWriter()
    w.put(epoch, 8)
    w.put(1, 1)
    for i in range(ROSTER_SLOTS):
        w.put(1 if i == slot else 0, 1)
    w.put(1, 1)
    investment.encode_into(w, schema, ROSTER_TYPE, {4: {0xC * slot: {0: peer_id, 8: generation, 0xA: True}}})
    w.put(tick, 32)
    w.put(1 if objects else 0, 1)
    for object_id, state_type, state, has_b in objects:
        w.put(1, 1)
        investment.encode_into(w, schema, OBJECT_ID_TYPE, object_id)
        body = investment.BitWriter()
        body.put(1, 1)
        body.put(0 if state_type is None else 1, 1)
        if state_type is not None:
            investment.encode_into(body, schema, state_type, state)
        if has_b:
            body.put(0, 1)
        w.put(len(body.bits), 32)
        w.bits += body.bits
    if objects:
        w.put(0, 1)
    return w.data()


def sim_player(schema, group, index, machine_id, spawn=None):
    """Replicated state of simulation player INDEX (kind 0xc entities of the world's entity group
    GROUP; record +0xc type 0x808022a5 = the entity's 0x50-byte block at +0x144): the bools at +8
    (loading task 9, 0x3286a8) and +9 (spawn readiness 0x35793c) set, owner machine id at
    +0x18 (0x80800646, entity +0x15c), which 0x4ed7b4 -> 0x14a270 matches against the local
    player (pla +0x18) to bind it (pla +0x50)."""
    object_id = {0: group, 4: 0xC, 6: index}
    state = {0: {0: -1, 4: 0x811C9DC5, 8: True, 9: True}, 0x18: {0: machine_id}, 0x4C: -1}
    if spawn:
        # bool +0x48 (entity +0x18c, reached through entity +0x1ec/+0x1f0 by 0x4ec7b4): the spawn
        # processing (0x358688) sets a request flag on another component and waits for it.
        state[0x48] = True
    return object_id, SIM_PLAYER_TYPE, state, True


def game_mode(group, index, state):
    """Replicated state of the game-mode object (kind 0x10; type 0x808008a3 at entity +0x110).
    Its first field, a 4-bit state, is what loading task 9 (`0xb26e4c`, must be non-zero) and the
    spawn check `0x35793c` (jump table 0x11ed724 indexed by the value: only 3 continues) read."""
    values = {0: state, 8: 0x811C9DC5, 0xC: -1, 0x984: -1, 0x988: 0x811C9DC5}
    return {0: group, 4: 0x10, 6: index}, GAME_MODE_TYPE, values, False


def baseline_list(value):
    """Parse KEY/KIND/INDEX[/B],... (or @FILE holding that list) into (key, kind, index, has_b)."""
    if value.startswith("@"):
        value = Path(value[1:]).read_text(encoding="utf-8")
    return [tuple(int(y, 0) for y in (x.strip().split("/") + ["0"])[:4]) for x in value.split(",") if x.strip()]


def object_spec(value):
    """Parse --ah-object GROUP:KIND:INDEX:TYPE[:B]=JSON into an sim_packet object: JSON maps field
    offsets (decimal or 0x strings) to values, nested dicts for struct fields, e.g.
    0x992bb471:0x20:7:0x80802a1b={"0": 1}."""
    head, _, body = value.partition("=")
    parts = [int(x, 0) for x in head.split(":")]
    group, kind, index, state_type = parts[:4]
    has_b = bool(parts[4]) if len(parts) > 4 else False

    def keys(v):
        if isinstance(v, dict):
            return {int(k, 0): keys(x) for k, x in v.items()}
        if isinstance(v, list):
            return [keys(x) for x in v]
        return v

    return {0: group, 4: kind, 6: index}, state_type, keys(json.loads(body or "{}")), has_b


def default_baseline(key, kind, index, has_b=False):
    """A baseline without fields (reset to the type's defaults) for replicated object key/kind/index."""
    return {0: key, 4: kind, 6: index}, None, None, has_b


def world_state_delta(schema, request, zone=None, peer_byte=1, group_peer=0):
    """Body of host type 11 (0x25715c): one present bit, then a world-state delta (type
    0x808009ed, decoded with 0x1ca478 onto the host simulation's copy at +0xbc0 and posted to
    the world controller as RPC 6). Peer slot 0 is the joining client: its identity (@0x10)
    and player data (@0x38) from the join request, member mask @0x5028 and occupied mask
    @0x502c bit 0 (0x1c9ab8 finds the local slot by the 8-byte id). With `zone`, group record 0 (@0x2e48, 0x80800c6f)
    names that zone-set index with state 1, 5-bit peer field `group_peer`, and per-peer byte
    `peer_byte`, which
    the world controller copies to slice +0x310 when it matches its own +0x1d0 (0x31d06c)
    and which gates the zone-set switch (0x31c750)."""
    # Masks (bit per peer slot): @0x5028 = session members - the manager entry's member list
    # (0x81c0c4 via 0x1c9b98) that world-controller RPC 16 turns into the machine table, @0x502c
    # = identities to search for the local slot (0x1c9ab8).
    if not 0 <= group_peer < 20:
        raise ValueError("group peer must be a world-state peer slot (0-19)")
    values = {8: {0: {0: request[0x10], 0x28: {0: request[0x38]}}}, 0x5028: 1, 0x502C: 1}
    if zone is not None:
        records = {0xD8 * i: {0: -1} for i in range(1, 40)}  # unused groups: index -1
        records[0] = {0: zone, 8: {1: 1, 2: group_peer}, 0x28: {0x10: {0: peer_byte}}}
        values[0x2E48] = {0: records}
    w = investment.BitWriter()
    w.put(1, 1)
    investment.encode_into(w, schema, WORLD_STATE_TYPE, values)
    return w.data()


def join_response(schema, request_body, session_id="d1local-activity", result=0, extra=None):
    request = join_request(schema, request_body)
    values = {0: request, 0xF8: result, 0xF9: char_array(session_id)}
    values.update(extra or {})
    return investment.encode(schema, JOIN_RESPONSE_TYPE, values)


def bc_notification(subtype, data, ident=0):
    return struct.pack(">BQII", 1, ident, subtype, len(data)) + data
# Service replies that need more than the default: 104 carries the universe object at +8
# (type 0x80801941, as in the sign-in reply); an absent universe root gives pig (0xa2).
WS_REPLY_VALUES = {104: {8: {0: 0x7FFFFFFFFFFFFFFF, 8: 1}}}
# Universe +8 behaves like the server clock (the director compares character timestamps
# with it, and the client reads no calendar clock of its own); its unit is still unknown.
UNIVERSE_TIME_UNITS = {"one": lambda t: 1, "s": lambda t: int(t), "ms": lambda t: int(t * 1e3),
                       "us": lambda t: int(t * 1e6),
                       "filetime": lambda t: int((t + 11644473600) * 1e7)}


def universe_reply(args):
    """Service 104 reply values: the universe object (0x80801941) with its root soid, the
    server clock and, optionally, the i16 list at +0x10 (0x808045f0: u5 count + up to 20
    i16) - flag indexes applied over the content defaults of the universe unlock table."""
    universe = {0: UNIVERSE_SOID, 8: universe_time(args.universe_time)}
    if args.universe_flags:
        universe[0x10] = {0: len(args.universe_flags), 4: list(args.universe_flags)}
    if args.universe_values:
        universe[0x3C] = {0: len(args.universe_values),
                          4: [{0: index, 2: value} for index, value in args.universe_values]}
    return {8: universe}


def universe_time(unit, now=None):
    import time as _time
    return UNIVERSE_TIME_UNITS[unit](_time.time() if now is None else now)


def client_config_reply(value=1, version=1):
    """client config rsp (0x13): nanopb message at 0x11d0bf0 - required 1 uint64, required
    2 uint32, 3 a fixed64 callback field (omitted). Values are local placeholders."""
    return bytes([0x08]) + signon_proto.varint(value) + bytes([0x10]) + signon_proto.varint(version)


def acct_id_trans_reply(request, account_ids):
    """Reply to "acct id trans" (0x17): the request echoed - u16 count, u8 7, u8 0xff and
    `count` name entries (decoder 0x415764 requires the same header and reads the entries
    with 0x113090) - followed by one u64 account id per entry (64 bits each, MSB first)."""
    count = struct.unpack_from(">H", request)[0]
    if request[2:4] != b"\x07\xff" or len(account_ids) != count:
        raise ValueError("unexpected acct id trans request")
    return request + b"".join(struct.pack(">Q", a) for a in account_ids)
UNIVERSE_SOID = 0x7FFFFFFFFFFFFFFF
FAMILY_TYPES = {"peer": 0, "inspection": 1, "roster": 2, "select_character": 3, "self": 4,
                "universe": 5, "party_details": 6, "party_network_details": 7}  # 0xb6068c


class BitWriter:
    """MSB-first bit stream, as written by the engine's type serializer (0x25e080)."""

    def __init__(self):
        self.bits = []

    def put(self, value, width):
        self.bits += [(value >> (width - 1 - i)) & 1 for i in range(width)]

    def data(self):
        bits = self.bits + [0] * (-len(self.bits) % 8)
        return bytes(int("".join(map(str, bits[i:i + 8])), 2) for i in range(0, len(bits), 8))


def signin_response(account_id, result=0, word1=1, word2=1, optional_ids=True, opt_values=None,
                    universe_flags=(), universe_values=()):
    """Service 503 (investment sign-in) response, type 0x80801825:
    {u4 result, i32 detail} u64 +0x08, u32 +0x10, u32 +0x14, then struct 0x80801941 of six
    optional fields (presence bit each): u64, u64, three counted lists, u32. Lists and the
    last u32 are left absent here; the trailing bit is the ws blob flag (0).

    Primitive kinds are biased on the wire (seen in decoded memory): the kind-3 result enum
    decodes as bits - 1 and the signed kind-5 detail as bits - 0x80000000. The completion
    handler 0x295794 (via 0xbb8a40) needs result 0 and takes the user's root soid from +0x08;
    it then moves the local user entry from state 2 to 3."""
    w = BitWriter()
    w.put(0x01F7, 16)
    w.put(result + 1, 4); w.put(0x80000000, 32)
    w.put(account_id, 64); w.put(word1, 32); w.put(word2, 32)
    # The +0x18 struct is the universe object (0x41becc): its first u64 must be the
    # universe root soid the client subscribed to (0x7fffffffffffffff, rodata 0x11f4590).
    for value in (opt_values or (UNIVERSE_SOID, 1)):
        w.put(1 if optional_ids else 0, 1)
        if optional_ids:
            w.put(value, 64)
    # +0x10: optional list 0x808045f0 = u5 count + up to 20 i16 (biased by 0x8000).
    w.put(1 if universe_flags else 0, 1)
    if universe_flags:
        w.put(len(universe_flags), 5)
        for flag in universe_flags:
            w.put((flag + 0x8000) & 0xFFFF, 16)
    # +0x3c: optional list 0x808046e7 = u5 count + up to 20 0x8080185c {opt i16 flag index,
    # opt u2 tri-state value (bits = value + 1; 2 = true, 1 = false)}: universe flag overrides.
    w.put(1 if universe_values else 0, 1)
    if universe_values:
        w.put(len(universe_values), 5)
        for index, value in universe_values:
            w.put(1, 1); w.put((index + 0x8000) & 0xFFFF, 16)
            w.put(1, 1); w.put(value + 1, 2)
    for _ in range(2):
        w.put(0, 1)
    w.put(0, 1)
    return w.data()


def read_bits(data, start, count):
    """Unsigned MSB-first value of `count` bits at bit offset `start`."""
    value = 0
    for i in range(start, start + count):
        value = (value << 1) | ((data[i >> 3] >> (7 - (i & 7))) & 1)
    return value


def family_fetch_request(payload):
    """Service 206 request (type 0x80801774): u4 family type (biased +1), u64 root soid."""
    return read_bits(payload, 16, 4) - 1, read_bits(payload, 20, 64)


def family_fetch_response(family_type, root, version=1, blob=None):
    """Service 206 response (type 0x80801869, just {u4 result}) plus the ws blob that the
    client posts to its queuez handler (0xbb87b8 -> 0x51c7ec): a qz update payload holding
    the requested family, here with no objects."""
    blob = blob if blob is not None else qz_update([(family_type, root, version, 0, [])])
    w = BitWriter()
    w.put(206, 16)
    w.put(0 + 1, 4)          # result error_none, biased
    w.put(1, 1)              # blob present
    w.put(len(blob), 16)
    for byte in blob:
        w.put(byte, 8)
    return w.data()


def qz_update(families):
    """qz->c update payload (decoder 0x4198d8): u32 count, then per family
    u32 type, u64 root soid, u32 version, u8 flags, u32 object count, objects.
    Objects are (16-byte header, payload); the header's u32 at +0xc is the payload length."""
    out = struct.pack(">I", len(families))
    for family_type, root, version, flags, objects in families:
        out += struct.pack(">IQIBI", family_type, root, version, flags, len(objects))
        for header, payload in objects:
            out += header[:12] + struct.pack(">I", len(payload)) + payload
    return out
# The client's reply queue (0x7c6bc0) expects exactly four reply types: shello rsp, echo
# rsp, qz_reg rsp and rrc rsp. The last three decode with 0xa62a0, which ignores the
# payload, so an empty status-200 reply satisfies them. Request -> reply type.
UNSUB_REQ, UNSUB_RSP = 0x0E, 0x0F
EMPTY_REPLIES = {QZ_REG_REQ: QZ_REG_RSP, ECHO_REQ: ECHO_RSP, RRC_REQ: RRC_RSP, SUB_REQ: SUB_RSP,
                 UNSUB_REQ: UNSUB_RSP}
NAMES = {0x03: "goodbye not", 0x06: "c->ah mgr req", 0x07: "c->ah mgr rsp", 0x08: "c->ah not",
         0x09: "b->c not", 0x0A: "c->ws req", 0x0B: "c->ws rsp", 0x0C: "sub req", 0x0D: "sub rsp",
         0x0E: "unsubscribe req", 0x0F: "unsubscribe rsp", 0x10: "ahp req", 0x11: "ahp rsp",
         0x12: "client config req", 0x13: "client config rsp", 0x14: "shutdown not",
         0x15: "upo req", 0x16: "upo rsp", 0x17: "acct id trans req", 0x18: "acct id trans rsp",
         SHELLO_REQ: "shello req", SHELLO_RSP: "shello rsp", QZ_REG_REQ: "qz_reg req",
         QZ_REG_RSP: "qz_reg rsp", 0x7B: "qz->c update not", 0x7C: "qz->c sub lost not",
         ECHO_REQ: "echo req", ECHO_RSP: "echo rsp", 0x12C: "c->b irc not", 0x12D: "b->c rrc not",
         RRC_REQ: "rrc req", RRC_RSP: "rrc rsp"}


def seal(key, mac_key, plaintext, iv):
    pad = 16 - len(plaintext) % 16
    padded = plaintext + bytes([pad]) * pad
    encryptor = Cipher(algorithms.AES(key), modes.CBC(iv)).encryptor()
    body = iv + encryptor.update(padded) + encryptor.finalize()
    head = struct.pack(">I", len(body) + 32)
    return head + body + hmac.new(mac_key, head + body, hashlib.sha256).digest()


def unseal(key, mac_key, blob):
    (length,) = struct.unpack_from(">I", blob)
    if length + 4 != len(blob) or length < 0x30:
        raise ValueError("bad sealed length")
    mac = blob[-32:]
    if not hmac.compare_digest(mac, hmac.new(mac_key, blob[:-32], hashlib.sha256).digest()):
        raise ValueError("MAC mismatch")
    decryptor = Cipher(algorithms.AES(key), modes.CBC(blob[4:20])).decryptor()
    padded = decryptor.update(blob[20:-32]) + decryptor.finalize()
    pad = padded[-1]
    if not 1 <= pad <= 16 or padded[-pad:] != bytes([pad]) * pad:
        raise ValueError("bad padding")
    return padded[:-pad]


def shello_rsp_body(key, mac_key, nonce, session_key, iv):
    if len(nonce) != 12 or len(session_key) != 16:
        raise ValueError("nonce is 12 bytes, session key 16")
    blob = seal(key, mac_key, nonce + session_key, iv)
    if len(blob) - 4 > 0x5C:
        raise ValueError("sealed shello reply exceeds the client's 0x5c limit")
    return blob


class GcmChannel:
    """One direction of the post-shello encrypted stream."""

    def __init__(self, key, nonce):
        self.aead, self.nonce = AESGCM(key), bytearray(nonce)

    def _advance(self):
        for i in range(12):
            self.nonce[i] = (self.nonce[i] + 1) & 0xFF
            if self.nonce[i]:
                break

    def open(self, body):
        plain = self.aead.decrypt(bytes(self.nonce), body[16:] + body[:16], None)
        self._advance()
        return plain

    def seal(self, plain):
        out = self.aead.encrypt(bytes(self.nonce), plain, None)
        self._advance()
        return out[-16:] + out[:-16]


def channels(nonce, session_key):
    """(server receive, server send) channels for a session."""
    client_send = nonce[:11] + bytes([nonce[11] ^ 1])
    return GcmChannel(session_key, client_send), GcmChannel(session_key, nonce)


def encrypted_frame(channel, msg_type, msg_id, status, payload):
    body = channel.seal(struct.pack(">HIH", msg_type, msg_id, status) + payload)
    return struct.pack(">BBI", 1, 1, len(body)) + body


def encrypted_frame_kind(channel, msg_type, msg_id, payload):
    """Server push with a 6-byte header (header kinds 1 and 4: type, id; no status)."""
    body = channel.seal(struct.pack(">HI", msg_type, msg_id) + payload)
    return struct.pack(">BBI", 1, 1, len(body)) + body


def plain_frame(msg_type, msg_id, status, body):
    inner = struct.pack(">HIH", msg_type, msg_id, status) + body
    return struct.pack(">BBI", 1, 2, len(inner)) + inner


def exact(conn, count):
    data = bytearray()
    while len(data) < count:
        chunk = conn.recv(count - len(data))
        if not chunk:
            raise EOFError(f"closed with {len(data)}/{count} bytes")
        data.extend(chunk)
    return bytes(data)


def log(message):
    print(f"{datetime.datetime.now().isoformat(timespec='milliseconds')} {message}", flush=True)


def handle(conn, peer, args):
    tag = datetime.datetime.now().strftime("%Y%m%d-%H%M%S-%f")
    folder = args.capture_dir / tag
    folder.mkdir(parents=True)
    conn.settimeout(args.timeout)
    log(f"[{tag}] connection from {peer[0]}:{peer[1]}")
    rx = tx = None
    account_id = character_id = character_profile = active_id = None
    self_version = 1
    heartbeat_at = None  # next activity-host heartbeat push (--ah-heartbeat), set by the join
    sim_repeat = None  # simulation packet resent with each heartbeat (--ah-roster)
    sim_build = None  # (peer, slot, generation, objects, game-mode position, join time) for rebuilds
    baselines_stamp = None  # modification time of --ah-baselines-watch when last read
    game_mode_now = None  # game-mode state currently replicated (--ah-game-mode-schedule)
    world_params = None  # (join request, peer byte, group peer) of the --ah-world-state delta
    zone_now = zone_joined = None  # zone-set index last sent in the world state, and when joined
    confirm_repeat = None  # boot list resent with each heartbeat (--ah-boot)

    def push_self(number, reason):
        # Delta for the live self subscription: versions must advance by one (0x41b75c).
        nonlocal self_version
        if account_id is None:
            return
        self_version += 1
        push = encrypted_frame_kind(tx, QZ_UPDATE, 0, family_blob(
            args.schema, 4, account_id, character_id, character_profile,
            version=self_version, flags=0, active_id=active_id, self_flags=args.self_flags))
        conn.sendall(push)
        folder.joinpath(f"{number:03}-server-self-{reason}.bin").write_bytes(push)
        log(f"[{tag}] pushed self v{self_version} ({reason}): character "
            f"{character_id:#x}, active {active_id:#x}" if active_id else
            f"[{tag}] pushed self v{self_version} ({reason}), no active character")
    try:
        for number in itertools.count(1):
            head = exact(conn, 6)
            version, kind, length = struct.unpack(">BBI", head)
            if version != 1 or length > 1 << 20:
                raise ValueError(f"unexpected outer header {head.hex()}")
            body = exact(conn, length)
            folder.joinpath(f"{number:03}-client-kind{kind}.bin").write_bytes(head + body)
            if kind == 1 and rx is not None:
                body = rx.open(body)
                folder.joinpath(f"{number:03}-client-plain.bin").write_bytes(body)
            elif kind != 2 or length < 6:
                log(f"[{tag}] frame {number}: kind {kind}, {length} bytes (captured)")
                continue
            msg_type, msg_id = struct.unpack_from(">HI", body)
            payload = body[6:]
            log(f"[{tag}] frame {number} (kind {kind}): {NAMES.get(msg_type, hex(msg_type))} "
                f"id={msg_id} payload={len(payload)} bytes {payload[:48].hex()}")
            if kind == 1 and msg_type == WS_REQ and payload[:2] == bytes.fromhex("01f7") and args.ws_signin:
                account_id = int.from_bytes(payload[38:46], "big")
                if args.ws_markers:
                    body = signin_response(0x3333333333333333, word1=0x44444444, word2=0x55555555,
                                           opt_values=(0x1111111111111111, 0x2222222222222222))
                else:
                    body = signin_response(account_id, optional_ids=not args.ws_no_optional,
                                           opt_values=(UNIVERSE_SOID, universe_time(args.universe_time)),
                                           universe_flags=args.universe_flags,
                                           universe_values=args.universe_values)
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200, body)
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-rsp.bin").write_bytes(reply)
                log(f"[{tag}] sent sign-in rsp (account id {account_id:#x})")
                saved = load_store(args.store).get(f"{account_id:#x}") if args.store else None
                if saved and args.schema:
                    character_id = saved["id"]
                    character_profile = character_from_request(args.schema, character_id,
                                                               bytes.fromhex(saved["request"]))
                    log(f"[{tag}] restored stored character {character_id:#x}")
            elif kind == 1 and msg_type == WS_REQ and payload[:2] == bytes.fromhex("01f5") and args.ws_signin:
                # Service 501 creates a character; its response carries the new character soid.
                # Keep the id stable for this local account during this connection.
                character_id = (account_id or 1) + 1
                character_profile = character_from_request(args.schema, character_id, payload[2:])
                if args.store and account_id is not None:
                    save_character(args.store, account_id, character_id, payload[2:])
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200,
                                        default_ws_response(args.schema, 501, {8: character_id}))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-501.bin").write_bytes(reply)
                log(f"[{tag}] sent create-character rsp (id {character_id:#x})")
                # Creation changes the subscribed self profile: publish the new character
                # and make it the active one (the client does not send 504 after 501).
                active_id = character_id
                push_self(number, "after-501")
            elif kind == 1 and msg_type == WS_REQ and payload[:2] == bytes.fromhex("01f8") and args.ws_signin:
                # Service 504 selects a character: u64 character id, then a flag byte.
                # Reply error_none and publish it as the self root's active character.
                active_id = int.from_bytes(payload[2:10], "big")
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200, default_ws_response(args.schema, 504))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-504.bin").write_bytes(reply)
                log(f"[{tag}] sent select-character rsp (id {active_id:#x}, flag {payload[10:11].hex()})")
                push_self(number, "after-504")
            elif kind == 1 and msg_type == WS_REQ and payload[:2] == bytes.fromhex("00ce") and args.ws_signin:
                family_type, root = family_fetch_request(payload)
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200, family_fetch_response(
                    family_type, root, blob=family_blob(args.schema, family_type, root, character_id,
                                                       character_profile)))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-206.bin").write_bytes(reply)
                log(f"[{tag}] sent family fetch rsp: type {family_type} root {root:#x}, "
                    f"character {character_id:#x}" if character_id is not None else
                    f"[{tag}] sent family fetch rsp: type {family_type} root {root:#x}, no character")
            elif (kind == 1 and msg_type == WS_REQ and len(payload) >= 2 and args.ws_signin
                  and int.from_bytes(payload[:2], "big") in args.ws_no_reply):
                # Experiment switch: leaving 701 unanswered shows "Contacting Destiny Servers"
                # and fails sooner (comparison 059), so replies are the default.
                log(f"[{tag}] no reply for service {int.from_bytes(payload[:2], 'big')} (one-way)")
            elif (kind == 1 and msg_type == WS_REQ and len(payload) >= 2 and args.ws_signin
                  and default_ws_response(args.schema, int.from_bytes(payload[:2], "big"))):
                wire = int.from_bytes(payload[:2], "big")
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200, default_ws_response(
                    args.schema, wire, universe_reply(args) if wire == 104 else WS_REPLY_VALUES.get(wire)))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-{wire}.bin").write_bytes(reply)
                log(f"[{tag}] sent default reply for service {wire} (result error_none)")
            elif kind == 1 and msg_type == WS_REQ and len(payload) >= 2 and args.ws_zero_bytes:
                body = bytearray(args.ws_zero_bytes)
                body[0] = (args.ws_result & 0xF) << 4  # u4 result code, MSB-first (0 = error_none)
                reply = encrypted_frame(tx, WS_RSP, msg_id, 200, payload[:2] + bytes(body))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-ws-rsp.bin").write_bytes(reply)
                log(f"[{tag}] sent c->ws rsp for service {int.from_bytes(payload[:2], 'big')} "
                    f"(result {args.ws_result}, {args.ws_zero_bytes} bytes)")
                if args.qz_push:
                    families = []
                    for spec in args.qz_push:
                        name, version, flags = (spec.split(":") + ["1", "0"])[:3]
                        families.append((FAMILY_TYPES[name], args.qz_root, int(version, 0), int(flags, 0), []))
                    push = encrypted_frame_kind(tx, QZ_UPDATE, 0, qz_update(families))
                    conn.sendall(push)
                    folder.joinpath(f"{number:03}-server-qz-update.bin").write_bytes(push)
                    log(f"[{tag}] pushed qz update {args.qz_push} root={args.qz_root:#x}")
            if kind == 1 and msg_type == ACCT_ID_TRANS_REQ and args.ws_signin:
                # Replies must answer the oldest pending request first (0x7c3458); leaving
                # this unanswered made the next ws replies unmatched: BAP failure, centipede.
                # Entries are 36-byte names; the local test has one account.
                names = [payload[4 + 36 * i:40 + 36 * i].split(b"\0")[0].decode("latin1")
                         for i in range(struct.unpack_from(">H", payload)[0])]
                body = acct_id_trans_reply(payload, [account_id or 0] * len(names))
                reply = encrypted_frame(tx, ACCT_ID_TRANS_RSP, msg_id, 200, body)
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-18.bin").write_bytes(reply)
                log(f"[{tag}] sent acct id trans rsp {names} -> {account_id or 0:#x}")
            if kind == 1 and msg_type == AH_NOT and len(payload) >= 21:
                host_id, _flag, ah_type, ah_len = struct.unpack_from(">QBII", payload)
                body = payload[21:21 + ah_len]
                log(f"[{tag}] c->ah not host {host_id:#x} type {ah_type} len {ah_len} body {body.hex()}")
            if (kind == 1 and msg_type == AH_NOT and len(payload) >= 21 and ah_type == AH_JOIN_REQUEST
                    and args.ah_join and args.activity_schema):
                request = join_request(args.activity_schema, body)
                log(f"[{tag}] host join request {request.get(0, 0):#x} / {request.get(8, 0):#x} "
                    f"ids {request.get(0x10)}")
                data = join_response(args.activity_schema, body, args.ah_session, extra=dict(args.ah_join_fields))
                push = encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_JOIN_RESPONSE, data, host_id))
                conn.sendall(push)
                folder.joinpath(f"{number:03}-server-bcnot.bin").write_bytes(push)
                log(f"[{tag}] pushed host join response from host {host_id:#x} ({len(data)} bytes)")
                if args.ah_entities:
                    first, _, last = args.ah_entities.partition(":")
                    data = entity_indices(args.activity_schema, int(first, 0), int(last, 0))
                    push = encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_ENTITY_INDICES, data, host_id))
                    conn.sendall(push)
                    log(f"[{tag}] pushed entity indices {args.ah_entities} from host {host_id:#x}")
                if args.ah_state:
                    establish = b""
                    if args.ah_world_state:
                        parts = args.ah_world_state.split(":")
                        if len(parts) > 3:
                            raise ValueError("--ah-world-state expects ZONE[:PEERBYTE[:GROUPPEER]]")
                        zone = parts[0]
                        world_params = (request, int(parts[1] or "1", 0) if len(parts) > 1 else 1,
                                        int(parts[2], 0) if len(parts) > 2 else 0)
                        zone_now = None if zone == "peer" else int(zone, 0)
                        establish = world_state_delta(args.activity_schema, world_params[0], zone_now,
                                                      *world_params[1:])
                        zone_joined = time.time()
                    for subtype, data in ((AH_ESTABLISH, establish), (AH_STATE, activity_state(args.activity_schema, args.ah_map, args.ah_world, args.ah_checksum,
                                                                      args.ah_bubbles, args.ah_zone,
                                                                      args.ah_bubble_states))):
                        push = encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(subtype, data, host_id))
                        conn.sendall(push)
                    log(f"[{tag}] pushed host flag (type 11) and activity state (type 1) from host {host_id:#x}")
                if args.ah_heartbeat:
                    heartbeat_at = time.time() + args.ah_heartbeat[0]
                if args.ah_boot:
                    confirm_repeat = machine_list(args.activity_schema, [request[0x10][0]])
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_MACHINES, confirm_repeat, host_id)))
                    log(f"[{tag}] pushed boot list (type 43): client")
                if args.ah_roster:
                    slot, _, gen = args.ah_roster.partition(":")
                    peer = request[0x10][0]
                    objects = []
                    if args.ah_sim_player:
                        group, index, owner = args.ah_sim_player[:3]
                        spawn = args.ah_sim_player[3] if len(args.ah_sim_player) > 3 else None
                        objects.append(sim_player(args.activity_schema, group, index, owner, spawn))
                        log(f"[{tag}] simulation player {group:#x}:{index} owner {owner:016x}")
                    game_mode_at = None
                    if args.ah_game_mode:
                        group, index, state = args.ah_game_mode
                        if args.ah_game_mode_schedule:
                            state = args.ah_game_mode_schedule[0][0]
                        game_mode_at, game_mode_now = len(objects), state
                        objects.append(game_mode(group, index, state))
                        log(f"[{tag}] game mode {group:#x}:{index} state {state}")
                    for spec in args.ah_object:
                        objects.append(spec)
                        log(f"[{tag}] object {spec[0][0]:#x}:{spec[0][4]:#x}:{spec[0][6]} type {spec[1]:#x} state {spec[2]}")
                    sent = {(o[0][0], o[0][4], o[0][6]) for o in objects}
                    for key, kind, index, has_b in args.ah_baselines:
                        if (key, kind, index) not in sent:
                            objects.append(default_baseline(key, kind, index, has_b))
                    data = sim_repeat = sim_packet(args.activity_schema, peer, int(slot, 0),
                                                   int(gen or "1", 0), objects=objects)
                    sim_build = (peer, int(slot, 0), int(gen or "1", 0), objects, game_mode_at, time.time())
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_SIMULATION, data, host_id)))
                    log(f"[{tag}] pushed simulation packet (type 5): roster slot {slot} = client")
                for spec in args.ah_after_join:
                    subtype, _, data = spec.partition(":")
                    push = encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(int(subtype, 0), bytes.fromhex(data),
                                                                               host_id))
                    conn.sendall(push)
                    log(f"[{tag}] pushed b->c not subtype {subtype} ({len(data) // 2} bytes) after the join")
            if kind == 1 and msg_type == AH_NOT and heartbeat_at is not None and time.time() >= heartbeat_at:
                # In the activity (host session status 6, 0x308b94) the client declares the host
                # crashed (status 36, reason 0x67, "baboon") once the time since the host's last
                # message (0x25b210, host +0xba8) exceeds the configured timeout (config +0x2d10).
                seconds, subtype = args.ah_heartbeat
                if subtype >= 0:  # -1: only the machines/simulation repeats below
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(subtype, b"", host_id)))
                if confirm_repeat:
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_MACHINES, confirm_repeat, host_id)))
                if sim_build and args.ah_baselines_watch and args.ah_baselines_watch.exists():
                    # objects of a bubble that only loads after the join: pick up the list the
                    # dump script writes during the run and add baselines for the new ones
                    stamp = args.ah_baselines_watch.stat().st_mtime
                    if stamp != baselines_stamp:
                        baselines_stamp = stamp
                        peer_, slot_, gen_, objects_, at_, joined_ = sim_build
                        have = {(o[0][0], o[0][4], o[0][6]) for o in objects_}
                        new = [b for b in baseline_list("@" + str(args.ah_baselines_watch)) if b[:3] not in have]
                        if new:
                            objects_.extend(default_baseline(*b) for b in new)
                            sim_repeat = sim_packet(args.activity_schema, peer_, slot_, gen_, objects=objects_)
                            log(f"[{tag}] {len(new)} new baselines from {args.ah_baselines_watch}")
                if world_params and args.ah_zone_schedule:
                    # A zone change the way the real host moves a player between bubbles: a new
                    # world-state delta (type 11) naming another zone set in group record 0.
                    # The target zone set needs its base entry, i.e. a bubble state >= 0.
                    due = [z for z, sec in args.ah_zone_schedule if time.time() - zone_joined >= sec]
                    if due and due[-1] != zone_now:
                        zone_now = due[-1]
                        data = world_state_delta(args.activity_schema, world_params[0], zone_now, *world_params[1:])
                        conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_ESTABLISH, data, host_id)))
                        log(f"[{tag}] world state: zone set -> {zone_now}")
                if sim_build and sim_build[4] is not None and args.ah_game_mode_schedule:
                    peer_, slot_, gen_, objects_, at_, joined_ = sim_build
                    due = [st for st, sec in args.ah_game_mode_schedule if time.time() - joined_ >= sec]
                    if due and due[-1] != game_mode_now:
                        game_mode_now = due[-1]
                        group, index, _ = args.ah_game_mode
                        objects_[at_] = game_mode(group, index, game_mode_now)
                        sim_repeat = sim_packet(args.activity_schema, peer_, slot_, gen_, objects=objects_)
                        log(f"[{tag}] game mode {group:#x}:{index} -> state {game_mode_now}")
                if sim_repeat:
                    # the roster packet is dropped until the client's host session exists
                    conn.sendall(encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(AH_SIMULATION, sim_repeat, host_id)))
                heartbeat_at = time.time() + seconds
                log(f"[{tag}] pushed heartbeat subtype {subtype} from host {host_id:#x}")
            if kind == 1 and msg_type == AH_NOT and args.ah_not_push is not None and len(payload) >= 17:
                subtype, _, data = args.ah_not_push.partition(":")
                push = encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(int(subtype, 0),
                                                                           bytes.fromhex(data), host_id))
                conn.sendall(push)
                folder.joinpath(f"{number:03}-server-bcnot.bin").write_bytes(push)
                log(f"[{tag}] pushed b->c not from host {host_id:#x} subtype {subtype} ({len(data) // 2} bytes)")
            if kind == 1 and msg_type == AH_MGR_REQ and args.ah_mgr_reply is not None:
                reply = encrypted_frame(tx, AH_MGR_RSP, msg_id, 200, args.ah_mgr_reply)
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-07.bin").write_bytes(reply)
                log(f"[{tag}] sent c->ah mgr rsp ({len(args.ah_mgr_reply)} bytes)")
            if kind == 1 and msg_type == AHP_REQ and args.ahp_reply is not None:
                reply = encrypted_frame(tx, AHP_RSP, msg_id, 200, args.ahp_reply)
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-11.bin").write_bytes(reply)
                log(f"[{tag}] sent ahp rsp {args.ahp_reply.hex()} (request {payload.hex()})")
                if args.ahp_push is not None:
                    subtype, _, data = args.ahp_push.partition(":")
                    push = encrypted_frame_kind(tx, BC_NOT, 0, bc_notification(int(subtype, 0),
                                                                               bytes.fromhex(data)))
                    conn.sendall(push)
                    folder.joinpath(f"{number:03}-server-bcnot.bin").write_bytes(push)
                    log(f"[{tag}] pushed b->c not subtype {subtype} data {data}")
            if kind == 1 and msg_type == CLIENT_CONFIG_REQ and args.ws_signin:
                reply = encrypted_frame(tx, CLIENT_CONFIG_RSP, msg_id, 200, client_config_reply(*args.client_config))
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-13.bin").write_bytes(reply)
                log(f"[{tag}] sent client config rsp (request {payload.hex()})")
            if kind == 1 and msg_type in EMPTY_REPLIES:
                reply = encrypted_frame(tx, EMPTY_REPLIES[msg_type], msg_id, 200, b"")
                conn.sendall(reply)
                folder.joinpath(f"{number:03}-server-{EMPTY_REPLIES[msg_type]:02x}.bin").write_bytes(reply)
                log(f"[{tag}] sent {NAMES[EMPTY_REPLIES[msg_type]]} (empty, status 200)")
                if msg_type == SUB_REQ and len(payload) >= 9 and args.ws_signin:
                    # sub req payload: u8 family type, u64 root soid. Answer with that family
                    # (version 1, no objects) as a queuez update push.
                    family_type, root = payload[0], int.from_bytes(payload[1:9], "big")
                    push = encrypted_frame_kind(tx, QZ_UPDATE, 0,
                                                family_blob(args.schema, family_type, root, character_id,
                                                            character_profile, active_id=active_id,
                                                            self_flags=args.self_flags, peer=args.peer_family))
                    conn.sendall(push)
                    folder.joinpath(f"{number:03}-server-qz-update.bin").write_bytes(push)
                    log(f"[{tag}] pushed family type {family_type} root {root:#x} "
                        f"({len(push)} bytes, root object {'yes' if args.schema and family_type in ROOT_OBJECTS else 'no'})")
            if msg_type == SHELLO_REQ:
                fields = [(n, w, v if w == 0 else len(v)) for n, w, v in signon_proto.decode(payload)]
                log(f"[{tag}] shello req fields {fields}")
                nonce, session_key = secrets.token_bytes(12), secrets.token_bytes(16)
                reply = plain_frame(SHELLO_RSP, msg_id, 200, shello_rsp_body(
                    args.key, args.mac_key, nonce, session_key, secrets.token_bytes(16)))
                conn.sendall(reply)
                rx, tx = channels(nonce, session_key)
                folder.joinpath(f"{number:03}-server-shello-rsp.bin").write_bytes(reply)
                folder.joinpath("session.json").write_text(json.dumps(
                    {"nonce": nonce.hex(), "session_key": session_key.hex()}), encoding="utf-8")
                log(f"[{tag}] sent shello rsp ({len(reply)} bytes)")
    except (OSError, EOFError, ValueError, InvalidTag) as exc:
        log(f"[{tag}] {exc}")
    finally:
        conn.close()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bind", default="127.1.1.127")
    ap.add_argument("--port", type=int, default=12850)
    ap.add_argument("--timeout", type=float, default=120)
    ap.add_argument("--ws-zero-bytes", type=int, default=1024,
                    help="answer c->ws requests with the service id and this many zero bytes (0: no reply)")
    ap.add_argument("--ws-signin", action="store_true",
                    help="answer service 503 with a typed sign-in response carrying the request's account id")
    ap.add_argument("--ws-no-optional", action="store_true",
                    help="with --ws-signin, leave the optional u64 fields of the sign-in reply absent")
    ap.add_argument("--ws-markers", action="store_true",
                    help="with --ws-signin, fill the reply with distinct marker values to find copies in memory")
    ap.add_argument("--qz-push", nargs="*", default=[],
                    help="after the ws reply, push families as name[:version[:flags]] with no objects")
    ap.add_argument("--qz-root", type=lambda v: int(v, 0), default=0, help="root soid for pushed families")
    ap.add_argument("--ws-result", type=int, default=0,
                    help="ws result code in the first 4 bits (0 error_none, 1 unknown, 2 no_content, "
                         "3 moved, 4 not_found, 5 service_unavailable, 6 unauthorized, 7 timeout)")
    ap.add_argument("--capture-dir", type=Path, default=CAPTURE_DIR)
    ap.add_argument("--ws-no-reply", type=lambda v: {int(x) for x in v.split(",") if x},
                    default=set(),
                    help="comma-separated ws services to leave unanswered (experiments only)")
    ap.add_argument("--client-config", type=lambda v: tuple(int(x, 0) for x in v.split(",")), default=(1, 1),
                    metavar="V1,V2", help="client config rsp fields 1 (uint64) and 2 (uint32); meaning unknown")
    ap.add_argument("--universe-time", choices=sorted(UNIVERSE_TIME_UNITS), default="one",
                    help="value sent in universe +8 (suspected server clock): 1 or now in a unit")
    ap.add_argument("--universe-flags", type=lambda v: [int(x, 0) for x in v.split(",") if x], default=[],
                    help="experiment: i16 list in the universe object's +0x10 field (service 104)")
    ap.add_argument("--universe-flag-values", dest="universe_values", default=[],
                    type=lambda v: [tuple(int(y, 0) for y in x.split("=")) for x in v.split(",") if x],
                    help="universe flag overrides INDEX=VALUE,... (tri-state: 2 true, 1 false) in the "
                         "universe object's +0x3c list (sign-in and service 104)")
    ap.add_argument("--peer-family", action="store_true",
                    help="fill the peer family (0) subscription with the local account and character "
                         "summaries (the player's appearance and blueprint in activities come from it)")
    ap.add_argument("--self-flags",
                    help='account flags: "2" (all), "2@A0-511,B0-255" (ranges), layered with ";" '
                         '(e.g. "1;2@B0": all false, B0 true)')
    ap.add_argument("--ahp-reply", type=bytes.fromhex,
                    help="experiment: 16-byte hex payload for ahp rsp (0x11)")
    ap.add_argument("--ahp-push", help="experiment: SUBTYPE:HEX b->c not pushed after the ahp rsp")
    ap.add_argument("--ah-mgr-reply", type=bytes.fromhex,
                    help="experiment: hex payload for c->ah mgr rsp (0x07), normally 137 bytes")
    ap.add_argument("--ah-not-push",
                    help="experiment: SUBTYPE:HEX b->c not pushed (from that host) after c->ah not")
    ap.add_argument("--ah-join", action="store_true",
                    help="answer host join requests (c->ah not type 3) with a joined response")
    ap.add_argument("--ah-entities", metavar="FIRST:LAST",
                    help="with --ah-join, then grant entity indices FIRST..LAST-1 (b->c not type 0)")
    ap.add_argument("--ah-state", action="store_true",
                    help="with --ah-join, then send host type 11 and a minimal activity state (type 1)")
    ap.add_argument("--ah-map", help="with --ah-state, map name in the activity state (e.g. orbit)")
    ap.add_argument("--ah-world", type=int, help="with --ah-state, world id (i16) in the activity state")
    ap.add_argument("--ah-bubbles", type=int,
                    help="with --ah-state, bubble count of the scenario (states left at default)")
    ap.add_argument("--ah-bubble-states", metavar="DEFAULT[,BUBBLE=STATE...]",
                    type=lambda v: {(int(k, 0) if k else None): int(x, 0)
                                    for k, _, x in (("", "", t) if "=" not in t else t.partition("=")
                                                    for t in v.split(","))},
                    help="with --ah-bubbles, per-bubble state indices (default -1 = none loaded)")
    ap.add_argument("--ah-checksum", type=lambda v: int(v, 0),
                    help="with --ah-state, activity checksum (u32 @0x5c) matching the scenario")
    ap.add_argument("--ah-zone", type=lambda v: tuple(int(x, 0) for x in v.split(":")), metavar="INDEX:HASH",
                    help="with --ah-state, zone record (u32 @0x54, i32 @0x58); -1:0x811c9dc5 = none")
    ap.add_argument("--ah-world-state", nargs="?", const="peer", metavar="ZONE[:PEERBYTE[:GROUPPEER]]",
                    help="with --ah-state, the type-11 body carries a world-state delta: the client "
                         "in peer slot 0 and, with ZONE, group record 0 for that zone-set index; "
                         "GROUPPEER sets the record's 5-bit peer field (default 0, valid 0-19)")
    ap.add_argument("--ah-zone-schedule", default=[], metavar="ZONE@SECONDS,...",
                    type=lambda v: [tuple(int(y, 0) for y in x.split("@")) for x in v.split(",")],
                    help="with --ah-world-state ZONE and --ah-heartbeat, send a new world-state delta "
                         "naming each ZONE (zone-set index, 8*bubble for a bubble's base) SECONDS after "
                         "the join (e.g. 16@120,24@240); the bubbles need states >= 0 (--ah-bubble-states)")
    ap.add_argument("--ah-boot", action="store_true",
                    help="experiment: send host type 43 (boot list) naming the client's machine -> error bass")
    ap.add_argument("--ah-roster", metavar="SLOT[:GEN]",
                    help="with --ah-join, send a type-5 simulation packet with the client in roster SLOT")
    ap.add_argument("--ah-sim-player", type=lambda v: tuple(int(x, 0) for x in v.split(":")),
                    metavar="GROUP:INDEX:OWNER[:SPAWN]",
                    help="with --ah-roster, the type-5 packets also replicate simulation player INDEX "
                         "of entity group GROUP owned by player id OWNER (the local pla +0x18, e.g. "
                         "0x4786c0e0:5:0x0a3d4a2c8fbea6ba); SPAWN=1 sets the state's bool +0x48 (entity +0x18c), "
                         "which the spawner waits for")
    ap.add_argument("--ah-game-mode", type=lambda v: tuple(int(x, 0) for x in v.split(":")),
                    metavar="GROUP:INDEX:STATE",
                    help="with --ah-roster, replicate the game-mode object (kind 0x10) with this state "
                         "(non-zero ends the loading screen, 3 lets the spawner continue), e.g. 0x4786c0e0:3:3")
    ap.add_argument("--ah-game-mode-schedule", default=[], metavar="STATE@SECONDS,...",
                    type=lambda v: [tuple(int(y, 0) for y in x.split("@")) for x in v.split(",")],
                    help="with --ah-game-mode, step the game-mode state over time: the first state is sent "
                         "at the join, each later one from SECONDS after it (e.g. 1@0,2@20,3@40)")
    ap.add_argument("--ah-object", action="append", default=[], type=object_spec,
                    metavar="GROUP:KIND:INDEX:TYPE[:B]=JSON",
                    help="with --ah-roster, replicate this object with an explicit state of TYPE (fields "
                         "by offset, e.g. 0x992bb471:0x20:7:0x80802a1b={\"0\": 1}); repeatable")
    ap.add_argument("--ah-baselines", default=[], metavar="KEY/KIND/INDEX[/B],...|@FILE", type=baseline_list,
                    help="with --ah-roster, default baselines for these replicated objects (B = 1 when the "
                         "record has a state B type); @FILE reads the list from a file written by "
                         "scripts/rpcs3_replicated_objects.py")
    ap.add_argument("--ah-baselines-watch", type=Path, metavar="FILE",
                    help="with --ah-heartbeat, re-read this --ah-baselines list whenever it changes and "
                         "add default baselines for objects not sent yet (bubbles loaded after the join)")
    ap.add_argument("--ah-heartbeat", type=lambda v: (float(v.split(":")[0]), int((v.split(":") + ["11"])[1], 0)),
                    metavar="SECONDS[:SUBTYPE]",
                    help="with --ah-join, push an empty host message (default type 11; -1 = none) plus the "
                         "machines/simulation repeats every SECONDS")
    ap.add_argument("--ah-after-join", nargs="*", default=[], metavar="SUBTYPE:HEX",
                    help="experiment: further b->c not pushes after the join response")
    ap.add_argument("--ah-join-fields", default=[],
                    type=lambda v: [tuple(int(y, 0) for y in x.split("=")) for x in v.split(",") if x],
                    help="experiment: OFFSET=VALUE,... for the join response's trailing fields "
                         "(i32 @0x17c/0x184, biased i32 @0x180/0x188, u8 @0x18c, u16 @0x18e/0x190, i32 @0x194)")
    ap.add_argument("--ah-session", default="d1local-activity",
                    help="activity session id sent in the host join response")
    ap.add_argument("--activity-schema", type=Path, default=ACTIVITY_SCHEMA,
                    help="activity host message schema exported by scripts/rpcs3_tiger_types.py")
    ap.add_argument("--store", type=Path,
                    help="JSON file remembering created characters per account (outside Git)")
    ap.add_argument("--schema", type=Path, default=investment.DEFAULT_SCHEMA,
                    help="investment type schema exported by scripts/rpcs3_tiger_types.py (optional)")
    args = ap.parse_args()
    args.schema = investment.load_schema(args.schema) if args.schema.exists() else None
    args.activity_schema = (investment.load_schema(args.activity_schema)
                            if args.activity_schema.exists() else None)
    if not ipaddress.IPv4Address(args.bind).is_loopback:
        ap.error("listener must be loopback")
    args.key, args.mac_key = signon_proto.TEST_SESSION_KEYS[1], signon_proto.TEST_SESSION_KEYS[2]
    with socket.socket() as server:
        server.bind((args.bind, args.port))
        server.listen(8)
        log(f"BAP stand-in listening on {args.bind}:{args.port}")
        while True:
            conn, peer = server.accept()
            threading.Thread(target=handle, args=(conn, peer, args), daemon=True).start()


if __name__ == "__main__":
    main()
