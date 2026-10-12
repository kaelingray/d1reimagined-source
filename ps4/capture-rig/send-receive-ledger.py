#!/usr/bin/env python3
"""Build a conservative send/receive conversation ledger from a D1 run.

This is a migration adapter for the project's current text logs.  Native
structured producers are the intended endpoint; this parser deliberately marks
uncovered layers as not_observed instead of manufacturing absence claims.
"""

from __future__ import annotations

import argparse
import collections
import json
import pathlib
import re
import sys
import tempfile
from dataclasses import asdict, dataclass, field
from typing import Any, Iterable


ANSI = re.compile(r"\x1b\[[0-9;]*m")
FRAME = re.compile(
    r"BAP TCP msg#(?P<ordinal>\d+) (?P<actual>\d+)B "
    r"\[type (?P<envelope>[0-9a-fA-F]{4}) len (?P<declared>\d+)"
)
INNER = re.compile(
    r"SECURED \((?P<body>\d+)B\): .*?Ok\(Inner \{ "
    r"msg_type: (?P<type>\d+), sequence: (?P<sequence>\d+) \}\)"
)
INVESTMENT = re.compile(
    r"INVESTMENT message id (?P<id>\d+) \((?P<body>\d+) body bytes\)"
)
SILENT = re.compile(r"D1_INV_SILENT: NOT answering investment id (?P<id>\d+)")
SECURED_REPLY = re.compile(r"--> BAP SECURED REPLY (?P<length>\d+)B")
PUSH_PLAIN = re.compile(r"push plaintext: (?P<hex>(?:[0-9a-fA-F]{2}[ ]*)+)$")
PUSH_WRITE = re.compile(r"--> BAP QUEUEZ [A-Z ]+ (?P<length>\d+)B")
BOOT_ENTER = re.compile(r"Entering state '(?P<state>[^']+)' for reason '(?P<reason>[^']+)'")
BOOT_LEAVE = re.compile(r"Leaving state '(?P<state>[^']+)' for reason '(?P<reason>[^']+)'")
WORLD = re.compile(r"successfully changed world to: ?(?P<world>.*)$")
QUEUEZ_FAIL = re.compile(
    r"failed to process update for \[family_type ENUM\((?P<family>\d+)\)"
)
FAMILY_UPDATE_FAIL = re.compile(
    r"ignoring family-update due to unhandled replication state, "
    r"family_type=ENUM\((?P<family>\d+)\), "
    r"family_root_soid=(?P<root>0x[0-9a-fA-F]+), "
    r"replication state=(?P<state>\d+)"
)
NO_HANDLER = re.compile(r"no secured handler for inner type 0x(?P<type>[0-9a-fA-F]+)")


@dataclass
class Event:
    schema_version: int
    run_id: str
    source: str
    source_sequence: int
    connection_id: str | None
    direction: str
    stage: str
    status: str
    correlation_id: str | None = None
    inner_type: int | None = None
    inner_sequence: int | None = None
    investment_id: int | None = None
    family: int | None = None
    body_length: int | None = None
    frame_length: int | None = None
    monotonic_ns: int | None = None
    socket_key: str | None = None
    wire_key: str | None = None
    details: dict[str, Any] = field(default_factory=dict)


@dataclass
class Conversation:
    correlation_id: str
    canonical_key: str
    occurrence: int
    flow: str
    label: str
    policy: str
    evidence: dict[str, str]
    server_received: str
    server_decrypted: str
    server_decision: str
    server_transmitted: str
    guest_received: str
    client_matched: str
    client_consumed: str
    family_applied: str
    outcome: str
    first_stop: str
    event_stages: list[str]


DEFAULT_EXPECTATIONS: dict[str, Any] = {
    "schema_version": 1,
    "inner": {
        "0x0079": {
            "policy": "required_reply",
            "expected_reply_type": "0x007a",
            "evidence": {
                "grade": "C",
                "claim": "client in-flight request/reply contract",
                "source": "historicaldocs/design/bap-protocol.md sections 3.1-3.2",
            },
        },
        "0x00fa": {
            "policy": "required_reply",
            "expected_reply_type": "0x00fb",
            "evidence": {
                "grade": "C",
                "claim": "project live keepalive contract",
                "source": "confirmed boot captures",
            },
        },
        "0x007b": {
            "policy": "server_push",
            "evidence": {
                "grade": "C",
                "claim": "client registered push-handler path",
                "source": "historicaldocs/design/bap-protocol.md sections 3.1-3.2",
            },
        },
        "0x000c": {
            "policy": "unknown",
            "evidence": {
                "grade": "C",
                "claim": "carries family class and root; invented 0x000d echo did not advance replication state",
                "source": "historicaldocs/design/character-and-orbit.md sections 26.57-26.58",
            },
        },
        "0x0015": {
            "policy": "unknown",
            "evidence": {
                "grade": "O",
                "claim": "client has survived this secret-bearing message unanswered",
                "source": "current character-signin captures",
            },
        },
        "0x0020": {
            "policy": "unknown",
            "evidence": {
                "grade": "C",
                "claim": "guessed adjacent reply regressed and is a recorded negative",
                "source": "historicaldocs/design/character-and-orbit.md",
            },
        },
    },
    "investment": {
        "206": {
            "policy": "required_reply",
            "evidence": {
                "grade": "C",
                "claim": "accepted response path",
                "source": "confirmed boot captures",
            },
        },
        "501": {
            "policy": "unknown",
            "evidence": {
                "grade": "I",
                "claim": "current silence-versus-response discriminator; response contract unknown",
                "source": "historicaldocs/design/character-and-orbit.md section 26.54",
            },
        },
        "503": {
            "policy": "required_reply",
            "evidence": {
                "grade": "C",
                "claim": "accepted response path",
                "source": "confirmed boot captures",
            },
        },
        "701": {
            "policy": "unknown",
            "evidence": {
                "grade": "C",
                "claim": "empty reply was harmful; valid response contract remains unknown",
                "source": "historicaldocs/design/character-and-orbit.md",
            },
        },
    },
}

DEFAULT_EXPECTATIONS_PATH = pathlib.Path(__file__).with_name(
    "send-receive-expectations.json"
)
VALID_POLICIES = {"required_reply", "one_way", "server_push", "unknown"}


def clean(line: str) -> str:
    return ANSI.sub("", line).strip()


def logged_wire_key(line: str, frame_length: int) -> str | None:
    """Return a privacy-preserving identity from a logged encrypted frame prefix."""
    if "B:" not in line:
        return None
    prefix_text = line.split("B:", 1)[1].split("…", 1)[0].split("...", 1)[0]
    tokens = re.findall(r"\b[0-9a-fA-F]{2}\b", prefix_text)
    if len(tokens) < 22:
        return None
    prefix = bytes.fromhex("".join(tokens[:22]))
    return f"{frame_length}:{prefix[:6].hex()}:{prefix[6:22].hex()}"


def emit(
    events: list[Event],
    run_id: str,
    source: str,
    line_number: int,
    connection_id: str | None,
    direction: str,
    stage: str,
    status: str,
    **kwargs: Any,
) -> Event:
    event = Event(
        schema_version=1,
        run_id=run_id,
        source=source,
        source_sequence=line_number,
        connection_id=connection_id,
        direction=direction,
        stage=stage,
        status=status,
        **kwargs,
    )
    events.append(event)
    return event


def parse_push_plaintext(text: str) -> tuple[int, int, int] | None:
    match = PUSH_PLAIN.search(text)
    if not match:
        return None
    raw = bytes.fromhex(match.group("hex"))
    if len(raw) < 14 or raw[:2] != b"\x00\x7b":
        return None
    sequence = int.from_bytes(raw[2:6], "big")
    family_count = int.from_bytes(raw[6:10], "big")
    family = int.from_bytes(raw[10:14], "big")
    if family_count < 1:
        return None
    return sequence, family, len(raw)


def parse_responder(path: pathlib.Path, run_id: str) -> list[Event]:
    events: list[Event] = []
    connection_number = 0
    connection_id: str | None = None
    current_corr: str | None = None
    current_type: int | None = None
    current_sequence: int | None = None
    pending_frame: Event | None = None
    pending_push_corr: str | None = None

    for line_number, raw_line in enumerate(path.read_text(errors="replace").splitlines(), 1):
        line = clean(raw_line)
        if "=== BAP TCP CONNECT" in line:
            connection_number += 1
            connection_id = f"bap-{connection_number}"
            current_corr = None
            emit(
                events, run_id, "server_bap", line_number, connection_id,
                "local", "connection_open", "ok",
            )
            continue

        match = FRAME.search(line)
        if match:
            # Post-framefix this line names an extracted frame, not a socket read.
            pending_frame = emit(
                events, run_id, "server_bap", line_number, connection_id,
                "c2s", "server_frame_extracted", "ok",
                frame_length=int(match.group("actual")),
                details={
                    "frame_ordinal": int(match.group("ordinal")),
                    "envelope": match.group("envelope").lower(),
                    "declared_payload_length": int(match.group("declared")),
                },
            )
            current_corr = None
            continue

        match = INNER.search(line)
        if match:
            current_type = int(match.group("type"))
            current_sequence = int(match.group("sequence"))
            conn = connection_id or "bap-unknown"
            current_corr = f"c2s:{conn}:{current_type:04x}:{current_sequence}"
            if pending_frame is not None and pending_frame.correlation_id is None:
                pending_frame.correlation_id = current_corr
                pending_frame.inner_type = current_type
                pending_frame.inner_sequence = current_sequence
            emit(
                events, run_id, "server_bap", line_number, connection_id,
                "c2s", "server_decrypt_ok", "ok",
                correlation_id=current_corr,
                inner_type=current_type,
                inner_sequence=current_sequence,
                body_length=int(match.group("body")),
            )
            emit(
                events, run_id, "server_bap", line_number, connection_id,
                "c2s", "server_dispatch", "ok",
                correlation_id=current_corr,
                inner_type=current_type,
                inner_sequence=current_sequence,
            )
            pending_frame = None
            continue

        match = INVESTMENT.search(line)
        if match and current_corr:
            investment_id = int(match.group("id"))
            for event in reversed(events):
                if event.correlation_id == current_corr:
                    event.investment_id = investment_id
                    event.body_length = int(match.group("body"))
            emit(
                events, run_id, "server_bap", line_number, connection_id,
                "c2s", "server_message_identified", "ok",
                correlation_id=current_corr,
                inner_type=current_type,
                inner_sequence=current_sequence,
                investment_id=investment_id,
                body_length=int(match.group("body")),
            )
            continue

        match = SILENT.search(line)
        if match and current_corr:
            emit(
                events, run_id, "server_bap", line_number, connection_id,
                "c2s", "server_decision_silent", "silent",
                correlation_id=current_corr,
                inner_type=current_type,
                inner_sequence=current_sequence,
                investment_id=int(match.group("id")),
            )
            continue

        match = NO_HANDLER.search(line)
        if match and current_corr:
            emit(
                events, run_id, "server_bap", line_number, connection_id,
                "c2s", "server_decision_unsupported", "failed",
                correlation_id=current_corr,
                inner_type=int(match.group("type"), 16),
                inner_sequence=current_sequence,
            )
            continue

        match = SECURED_REPLY.search(line)
        if match and current_corr:
            frame_length = int(match.group("length"))
            emit(
                events, run_id, "server_bap", line_number, connection_id,
                "s2c", "server_socket_write", "ok",
                correlation_id=current_corr,
                inner_type=current_type,
                inner_sequence=current_sequence,
                frame_length=frame_length,
                wire_key=logged_wire_key(line, frame_length),
            )
            continue

        push = parse_push_plaintext(line)
        if push:
            sequence, family, body_length = push
            conn = connection_id or "bap-unknown"
            pending_push_corr = f"s2c:{conn}:007b:{sequence}"
            emit(
                events, run_id, "server_bap", line_number, connection_id,
                "s2c", "server_push_built", "ok",
                correlation_id=pending_push_corr,
                inner_type=0x007B,
                inner_sequence=sequence,
                family=family,
                body_length=body_length,
            )
            continue

        match = PUSH_WRITE.search(line)
        if match and pending_push_corr:
            push_event = next(
                event for event in reversed(events)
                if event.correlation_id == pending_push_corr and event.stage == "server_push_built"
            )
            frame_length = int(match.group("length"))
            emit(
                events, run_id, "server_bap", line_number, connection_id,
                "s2c", "server_socket_write", "ok",
                correlation_id=pending_push_corr,
                inner_type=0x007B,
                inner_sequence=push_event.inner_sequence,
                family=push_event.family,
                frame_length=frame_length,
                wire_key=logged_wire_key(line, frame_length),
            )
            pending_push_corr = None
            continue

        if "SECURED DECRYPT FAILED" in line:
            emit(
                events, run_id, "server_bap", line_number, connection_id,
                "c2s", "server_decrypt", "failed",
                correlation_id=current_corr,
            )
        elif "BAP TCP read error:" in line:
            emit(
                events, run_id, "server_bap", line_number, connection_id,
                "local", "connection_close", "failed",
                details={"kind": "read_error", "message": line.split("BAP TCP read error:", 1)[1].strip()},
            )
        elif "BAP TCP peer closed" in line:
            emit(
                events, run_id, "server_bap", line_number, connection_id,
                "local", "connection_close", "ok",
                details={"kind": "peer_closed"},
            )

    return events


def parse_emulator(path: pathlib.Path, run_id: str, server_events: list[Event]) -> list[Event]:
    events: list[Event] = []
    push_by_family: dict[int, list[Event]] = collections.defaultdict(list)
    diagnostic_occurrence: dict[int, int] = collections.defaultdict(int)
    last_diagnostic: dict[int, tuple[str, Event | None]] = {}
    for event in server_events:
        if event.stage == "server_push_built" and event.family is not None:
            push_by_family[event.family].append(event)

    def begin_family_diagnostic(family: int) -> tuple[str, Event | None]:
        candidates = push_by_family.get(family, [])
        if len(candidates) == 1:
            return candidates[0].correlation_id or "", candidates[0]
        diagnostic_occurrence[family] += 1
        return f"diag:{run_id}:queuez:{family}:{diagnostic_occurrence[family]}", None

    for line_number, raw_line in enumerate(path.read_text(errors="replace").splitlines(), 1):
        line = clean(raw_line)
        match = BOOT_ENTER.search(line)
        if match:
            emit(
                events, run_id, "guest_log", line_number, None,
                "local", "state_entered", "ok",
                details={"state": match.group("state"), "reason": match.group("reason")},
            )
            continue
        match = BOOT_LEAVE.search(line)
        if match:
            emit(
                events, run_id, "guest_log", line_number, None,
                "local", "state_left", "ok" if match.group("reason") == "normal_flow" else "failed",
                details={"state": match.group("state"), "reason": match.group("reason")},
            )
            continue
        match = WORLD.search(line)
        if match:
            emit(
                events, run_id, "guest_log", line_number, None,
                "local", "world_changed", "ok",
                details={"world": match.group("world").strip()},
            )
            continue
        match = FAMILY_UPDATE_FAIL.search(line)
        if match:
            family = int(match.group("family"))
            correlation_id, corr = begin_family_diagnostic(family)
            last_diagnostic[family] = (correlation_id, corr)
            emit(
                events, run_id, "guest_log", line_number,
                corr.connection_id if corr else None,
                "s2c", "client_family_update_result", "failed",
                correlation_id=correlation_id,
                inner_type=0x007B,
                inner_sequence=corr.inner_sequence if corr else None,
                family=family,
                details={
                    "diagnostic": "family update ignored in unhandled replication state",
                    "family_root_soid": match.group("root").lower(),
                    "replication_state": int(match.group("state")),
                    "correlation": (
                        "unique same-family push"
                        if corr else "unresolved among multiple same-family pushes"
                    ),
                },
            )
            continue
        match = QUEUEZ_FAIL.search(line)
        if match:
            family = int(match.group("family"))
            association = last_diagnostic.pop(family, None)
            if association is None:
                association = begin_family_diagnostic(family)
            correlation_id, corr = association
            emit(
                events, run_id, "guest_log", line_number,
                corr.connection_id if corr else None,
                "s2c", "client_push_diagnostic", "failed",
                correlation_id=correlation_id,
                inner_type=0x007B,
                inner_sequence=corr.inner_sequence if corr else None,
                family=family,
                details={
                    "diagnostic": "queuez failed to process update",
                    "scope": "does not by itself prove object rejection",
                    "correlation": (
                        "unique same-family push"
                        if corr else "unresolved among multiple same-family pushes"
                    ),
                },
            )
            continue
        if "update_receiving_header() failed to find recipient" in line:
            emit(
                events, run_id, "guest_log", line_number, None,
                "s2c", "client_header_matched", "failed",
                details={"diagnostic": "failed to find recipient"},
            )
    return events


@dataclass
class StreamFrame:
    frame_length: int
    wire_key: str | None
    source_sequence: int
    monotonic_ns: int | None


class BapStreamDecoder:
    """Recover framed BAP identities from traced TCP segments without plaintext."""

    def __init__(self) -> None:
        self.prefix = bytearray()
        self.frame_length: int | None = None
        self.consumed = 0
        self.prefix_has_gap = False
        self.confounded = False

    def _finish(self, sequence: int, monotonic_ns: int | None) -> StreamFrame:
        assert self.frame_length is not None
        key = None
        if len(self.prefix) >= 22 and not self.prefix_has_gap:
            key = (
                f"{self.frame_length}:{bytes(self.prefix[:6]).hex()}:"
                f"{bytes(self.prefix[6:22]).hex()}"
            )
        frame = StreamFrame(self.frame_length, key, sequence, monotonic_ns)
        self.prefix.clear()
        self.frame_length = None
        self.consumed = 0
        self.prefix_has_gap = False
        return frame

    def feed(
        self,
        payload: bytes,
        unavailable: int,
        sequence: int,
        monotonic_ns: int | None,
    ) -> list[StreamFrame]:
        frames: list[StreamFrame] = []
        offset = 0
        while offset < len(payload) and not self.confounded:
            if self.frame_length is None:
                needed = 6 - len(self.prefix)
                take = min(needed, len(payload) - offset)
                self.prefix.extend(payload[offset:offset + take])
                self.consumed += take
                offset += take
                if len(self.prefix) < 6:
                    continue
                declared = int.from_bytes(self.prefix[2:6], "big")
                self.frame_length = 6 + declared
                if declared == 0 or self.frame_length < 6:
                    self.confounded = True
                    break

            remaining = self.frame_length - self.consumed
            take = min(remaining, len(payload) - offset)
            if len(self.prefix) < 22:
                prefix_take = min(take, 22 - len(self.prefix))
                self.prefix.extend(payload[offset:offset + prefix_take])
            self.consumed += take
            offset += take
            if self.consumed == self.frame_length:
                frames.append(self._finish(sequence, monotonic_ns))

        gap = unavailable
        if gap and not self.confounded:
            if self.frame_length is None:
                self.confounded = True
            else:
                remaining = self.frame_length - self.consumed
                if self.consumed < 22:
                    self.prefix_has_gap = True
                if gap < remaining:
                    self.consumed += gap
                elif gap == remaining:
                    self.consumed += gap
                    frames.append(self._finish(sequence, monotonic_ns))
                else:
                    # The omitted suffix crossed at least one unknown frame header.
                    self.consumed += remaining
                    frames.append(self._finish(sequence, monotonic_ns))
                    self.confounded = True
        return frames


def parse_network_trace(
    run_dir: pathlib.Path, run_id: str, server_events: list[Event]
) -> tuple[list[Event], dict[str, Any]]:
    path = run_dir / "network-trace.jsonl"
    if not path.exists():
        return [], {
            "guest_socket_trace": "not_observed",
            "dropped": None,
            "reason": "network-trace.jsonl absent",
        }
    records: list[dict[str, Any]] = []
    dropped: int | None = None
    start = False
    malformed = 0
    for line_number, line in enumerate(path.read_text(errors="replace").splitlines(), 1):
        if not line.strip():
            continue
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            malformed += 1
            continue
        if item.get("schema_version") != 1:
            malformed += 1
            continue
        if item.get("kind") == "start":
            start = True
        elif item.get("kind") == "socket-attempt":
            item["_line"] = line_number
            records.append(item)
        if item.get("kind") in {"stats", "stop"}:
            dropped = int(item.get("dropped", 0))
    status = "ok" if start and dropped == 0 and malformed == 0 else "confounded"
    health: dict[str, Any] = {
        "guest_socket_trace": status,
        "dropped": dropped,
        "malformed": malformed,
    }
    events: list[Event] = []
    bap_sockets = {
        str(record.get("socket_key"))
        for record in records
        if record.get("operation") == "connect"
        and isinstance(record.get("endpoint"), dict)
        and record["endpoint"].get("address") == "10.77.0.2"
        and int(record["endpoint"].get("port", -1)) == 3075
    }
    decoders: dict[tuple[str, str], BapStreamDecoder] = collections.defaultdict(BapStreamDecoder)
    for record in sorted(records, key=lambda item: int(item.get("sequence", 0))):
        socket_key = str(record.get("socket_key"))
        operation = record.get("operation")
        if socket_key not in bap_sockets or operation not in {"send", "sendmsg", "recv", "recvmsg"}:
            continue
        result = int(record.get("result", 0))
        if result <= 0:
            continue
        try:
            payload = bytes.fromhex(str(record.get("payload_hex", "")))
        except ValueError:
            malformed += 1
            status = "confounded"
            continue
        captured = int(record.get("payload_captured", len(payload)))
        available = int(record.get("payload_available", result))
        if captured != len(payload) or captured > available:
            malformed += 1
            status = "confounded"
            continue
        direction = "s2c" if operation in {"recv", "recvmsg"} else "c2s"
        # Send records capture the requested buffer, which may exceed a partial
        # successful send. Only the first `result` bytes crossed the boundary.
        usable = payload[:min(len(payload), result)]
        unavailable = result - len(usable)
        if unavailable < 0 or (direction == "s2c" and available != result):
            malformed += 1
            status = "confounded"
            continue
        decoder = decoders[(socket_key, direction)]
        frames = decoder.feed(
            usable,
            unavailable,
            int(record.get("sequence", 0)),
            int(record["monotonic_ns"]) if record.get("monotonic_ns") is not None else None,
        )
        for frame in frames:
            emit(
                events, run_id, "guest_socket", frame.source_sequence, None,
                direction, "guest_socket_recv" if direction == "s2c" else "guest_socket_send", "ok",
                frame_length=frame.frame_length,
                monotonic_ns=frame.monotonic_ns,
                socket_key=socket_key,
                wire_key=frame.wire_key,
                details={"correlation": "wire_key" if frame.wire_key else "unavailable"},
            )

    by_wire: dict[str, list[Event]] = collections.defaultdict(list)
    for event in server_events:
        if event.stage == "server_socket_write" and event.wire_key:
            by_wire[event.wire_key].append(event)
    used: set[int] = set()
    matched = 0
    for event in events:
        if event.stage != "guest_socket_recv":
            continue
        if not event.wire_key:
            continue
        candidates = by_wire.get(event.wire_key, [])
        server_event = next((candidate for candidate in candidates if id(candidate) not in used), None)
        if server_event is None:
            continue
        used.add(id(server_event))
        matched += 1
        event.correlation_id = server_event.correlation_id
        event.connection_id = server_event.connection_id
        event.inner_type = server_event.inner_type
        event.inner_sequence = server_event.inner_sequence
        event.investment_id = server_event.investment_id
        event.family = server_event.family

    send_confounded = any(
        decoder.confounded for (unused_socket, direction), decoder in decoders.items()
        if direction == "c2s"
    )
    receive_confounded = any(
        decoder.confounded for (unused_socket, direction), decoder in decoders.items()
        if direction == "s2c"
    )
    health.update({
        "guest_socket_trace": status,
        "malformed": malformed,
        "bap_sockets": len(bap_sockets),
        "bap_frames_sent": sum(event.stage == "guest_socket_send" for event in events),
        "bap_frames_received": sum(event.stage == "guest_socket_recv" for event in events),
        "wire_matches": matched,
        "send_stream_reassembly": "confounded" if send_confounded else "ok",
        "receive_stream_reassembly": "confounded" if receive_confounded else "ok",
    })
    return events, health


def load_expectations(path: pathlib.Path | None) -> dict[str, Any]:
    chosen = path
    if chosen is None and DEFAULT_EXPECTATIONS_PATH.exists():
        chosen = DEFAULT_EXPECTATIONS_PATH
    if chosen is None:
        data = DEFAULT_EXPECTATIONS
    else:
        data = json.loads(chosen.read_text())
    if not isinstance(data, dict):
        raise ValueError("expectations root must be an object")
    if data.get("schema_version") != 1:
        raise ValueError("expectations schema_version must be 1")
    for namespace in ("inner", "investment"):
        items = data.get(namespace)
        if not isinstance(items, dict):
            raise ValueError(f"expectations {namespace} must be an object")
        for key, item in items.items():
            if not isinstance(item, dict):
                raise ValueError(f"expectations {namespace}.{key} must be an object")
            if item.get("policy") not in VALID_POLICIES:
                raise ValueError(
                    f"expectations {namespace}.{key} has invalid policy {item.get('policy')!r}"
                )
            evidence = item.get("evidence")
            if not isinstance(evidence, dict) or not all(
                isinstance(evidence.get(field), str) and evidence[field]
                for field in ("grade", "claim", "source")
            ):
                raise ValueError(
                    f"expectations {namespace}.{key} evidence needs grade, claim, and source"
                )
    return data


def expectation_for(events: list[Event], expectations: dict[str, Any]) -> tuple[str, dict[str, str]]:
    investment_id = next((e.investment_id for e in events if e.investment_id is not None), None)
    if investment_id is not None:
        item = expectations.get("investment", {}).get(str(investment_id), {})
        return item.get("policy", "unknown"), item.get("evidence", {})
    inner_type = next((e.inner_type for e in events if e.inner_type is not None), None)
    if inner_type is not None:
        item = expectations.get("inner", {}).get(f"0x{inner_type:04x}", {})
        return item.get("policy", "unknown"), item.get("evidence", {})
    return "unknown", {}


def first_value(events: Iterable[Event], field_name: str) -> Any:
    for event in events:
        value = getattr(event, field_name)
        if value is not None:
            return value
    return None


def build_conversations(
    events: list[Event], expectations: dict[str, Any], health: dict[str, Any]
) -> list[Conversation]:
    grouped: dict[str, list[Event]] = collections.defaultdict(list)
    for event in events:
        if event.correlation_id:
            grouped[event.correlation_id].append(event)

    occurrence: dict[str, int] = collections.defaultdict(int)
    conversations: list[Conversation] = []
    for corr, rows in grouped.items():
        rows.sort(key=lambda e: (e.source, e.source_sequence))
        stages = [e.stage for e in rows]
        inner_type = first_value(rows, "inner_type")
        inner_sequence = first_value(rows, "inner_sequence")
        investment_id = first_value(rows, "investment_id")
        family = first_value(rows, "family")
        if corr.startswith("s2c:"):
            flow = "server_push"
        elif corr.startswith("diag:"):
            flow = "diagnostic"
        else:
            flow = "request"
        if investment_id is not None:
            label = f"investment {investment_id}"
            canonical = f"investment:{investment_id}"
        elif family is not None:
            label = f"QueueZ family {family}"
            canonical = f"queuez:{family}"
            if flow == "diagnostic":
                label += " diagnostic"
                canonical = f"queuez_diagnostic:{family}"
        elif inner_type is not None:
            label = f"inner 0x{inner_type:04x}"
            canonical = f"inner:{inner_type:04x}"
        else:
            label = "unidentified"
            canonical = "unidentified"
        occurrence[canonical] += 1
        policy, evidence = expectation_for(rows, expectations)

        received = "--" if flow in {"server_push", "diagnostic"} else (
            "yes" if "server_frame_extracted" in stages else "not_observed"
        )
        decrypted = "--" if flow in {"server_push", "diagnostic"} else (
            "yes" if "server_decrypt_ok" in stages else (
                "failed" if "server_decrypt" in stages else "not_observed"
            )
        )
        if flow == "diagnostic":
            decision = "diagnostic"
        elif "server_decision_silent" in stages:
            decision = "silent"
        elif "server_decision_unsupported" in stages:
            decision = "unsupported"
        elif "server_socket_write" in stages:
            decision = "reply" if flow == "request" else "push"
        else:
            decision = "open"
        transmitted = "yes" if "server_socket_write" in stages else (
            "--" if flow == "diagnostic" else
            "--" if decision == "silent" else "not_observed"
        )
        guest_received = "yes" if "guest_socket_recv" in stages else "not_observed"
        if guest_received != "yes" and (
            health.get("guest_socket_trace") == "confounded"
            or health.get("receive_stream_reassembly") == "confounded"
        ):
            guest_received = "confounded"
        client_matched = "failed" if any(
            e.stage == "client_header_matched" and e.status == "failed" for e in rows
        ) else "not_observed"
        consumer = "failed" if any(
            e.stage == "client_consumer_result" and e.status == "failed" for e in rows
        ) else "not_observed"
        family_applied = "failed" if any(
            e.stage == "client_family_update_result" and e.status == "failed" for e in rows
        ) else "not_observed"
        push_diagnostic_failed = any(
            e.stage == "client_push_diagnostic" and e.status == "failed" for e in rows
        )
        if decrypted == "failed":
            outcome, first_stop = "confounded", "server_decrypt"
        elif consumer == "failed":
            outcome, first_stop = "failed", "client_consumer_result"
        elif family_applied == "failed":
            outcome, first_stop = "partial_failure", "client_family_update_result"
        elif push_diagnostic_failed:
            outcome, first_stop = "partial_failure", "client_push_diagnostic; scope unknown"
        elif client_matched == "failed":
            outcome, first_stop = "failed", "client_header_matched"
        elif decision == "silent":
            outcome, first_stop = "open", "client consequence not observed"
        elif policy == "required_reply" and decision == "open":
            outcome, first_stop = "open", "server reply not observed; absence unproven"
        elif flow == "server_push" and transmitted == "yes":
            outcome, first_stop = "open", "guest/client acceptance not observed"
        else:
            outcome, first_stop = "open", "no terminal verdict"

        conversations.append(Conversation(
            correlation_id=corr,
            canonical_key=canonical,
            occurrence=occurrence[canonical],
            flow=flow,
            label=label,
            policy=policy,
            evidence=evidence,
            server_received=received,
            server_decrypted=decrypted,
            server_decision=decision,
            server_transmitted=transmitted,
            guest_received=guest_received,
            client_matched=client_matched,
            client_consumed=consumer,
            family_applied=family_applied,
            outcome=outcome,
            first_stop=first_stop,
            event_stages=stages,
        ))
    conversations.sort(key=lambda c: (
        0 if c.flow == "request" else 1,
        c.canonical_key,
        c.occurrence,
    ))
    return conversations


def parse_run(run_dir: pathlib.Path, expectations: dict[str, Any]) -> dict[str, Any]:
    run_dir = run_dir.resolve()
    responder = run_dir / "responder.log"
    emulator = run_dir / "emulator.log"
    if not responder.exists():
        raise ValueError(f"{run_dir}: responder.log missing")
    run_id = run_dir.name
    events = parse_responder(responder, run_id)
    if emulator.exists():
        events.extend(parse_emulator(emulator, run_id, events))
    network_events, health = parse_network_trace(run_dir, run_id, events)
    events.extend(network_events)
    conversations = build_conversations(events, expectations, health)
    boot = [
        event.details for event in events
        if event.stage in {"state_entered", "state_left", "world_changed"}
    ]
    return {
        "schema_version": 1,
        "run_id": run_id,
        "run_dir": str(run_dir),
        "coverage": {
            "server_text_log": "migration_adapter",
            **health,
            "client_bap_probe": "not_observed",
        },
        "conversations": [asdict(c) for c in conversations],
        "boot_events": boot,
        "events": [asdict(event) for event in events],
    }


def cell(value: Any, width: int) -> str:
    text = str(value)
    return text if len(text) <= width else text[: width - 1] + "…"


def print_text(report: dict[str, Any]) -> None:
    coverage = report["coverage"]
    print(f"run: {report['run_id']}")
    print(
        "coverage: server={server_text_log} guest-socket={guest_socket_trace} "
        "client-bap={client_bap_probe} dropped={dropped}".format(**coverage)
    )
    headers = ["conversation", "policy", "server-rx", "decision", "server-tx", "guest-rx", "match", "consume", "family", "outcome", "first stop"]
    widths = [22, 18, 9, 11, 9, 12, 12, 12, 10, 15, 34]
    print(" ".join(cell(h, w).ljust(w) for h, w in zip(headers, widths)))
    print(" ".join("-" * w for w in widths))
    for row in report["conversations"]:
        values = [
            f"{row['label']}#{row['occurrence']}",
            row["policy"],
            row["server_received"],
            row["server_decision"],
            row["server_transmitted"],
            row["guest_received"],
            row["client_matched"],
            row["client_consumed"],
            row["family_applied"],
            row["outcome"],
            row["first_stop"],
        ]
        print(" ".join(cell(v, w).ljust(w) for v, w in zip(values, widths)))


def conversation_signature_counts(
    report: dict[str, Any],
) -> dict[str, collections.Counter[tuple[Any, ...]]]:
    out: dict[str, collections.Counter[tuple[Any, ...]]] = collections.defaultdict(
        collections.Counter
    )
    for row in report["conversations"]:
        signature = (
            row["server_received"], row["server_decrypted"], row["server_decision"],
            row["server_transmitted"], row["guest_received"], row["client_matched"],
            row["client_consumed"], row["family_applied"], row["outcome"], row["first_stop"],
        )
        out[row["canonical_key"]][signature] += 1
    return dict(out)


def format_signature(signature: tuple[Any, ...]) -> str:
    names = (
        "server-rx", "decrypt", "decision", "server-tx", "guest-rx",
        "match", "consume", "family", "outcome", "first-stop",
    )
    return ", ".join(f"{name}={value}" for name, value in zip(names, signature))


def print_diff(left: dict[str, Any], right: dict[str, Any]) -> int:
    a = conversation_signature_counts(left)
    b = conversation_signature_counts(right)
    changed = 0
    print(f"diff: {left['run_id']} -> {right['run_id']}")
    for key in sorted(set(a) | set(b)):
        if a.get(key) != b.get(key):
            changed += 1
            print(f"  {key}")
            left_counts = a.get(key, collections.Counter())
            right_counts = b.get(key, collections.Counter())
            for signature in sorted(set(left_counts) | set(right_counts), key=repr):
                old_count = left_counts.get(signature, 0)
                new_count = right_counts.get(signature, 0)
                if old_count == new_count:
                    continue
                rendered = format_signature(signature)
                if old_count:
                    print(f"    - {old_count} x {rendered}")
                if new_count:
                    print(f"    + {new_count} x {rendered}")
    if not changed:
        print("  no conversation changes")
    return changed


def self_test() -> None:
    if DEFAULT_EXPECTATIONS_PATH.exists():
        assert load_expectations(None) == DEFAULT_EXPECTATIONS

    reply_frame = bytes.fromhex(
        "010100000018" "00112233445566778899aabbccddeeff" "aaaaaaaaaaaaaaaa"
    )
    push_frame = bytes.fromhex(
        "010100000048" "ffeeddccbbaa99887766554433221100" + "bb" * 56
    )
    request_frame = bytes.fromhex(
        "010100000016" "102132435465768798a9bacbdcedfe0f" "cccccccccccc"
    )
    reply_prefix = " ".join(f"{byte:02x}" for byte in reply_frame[:22])
    push_prefix = " ".join(f"{byte:02x}" for byte in push_frame[:22])
    responder_log = f"""
=== BAP TCP CONNECT from 10.77.0.2:12345 ===
BAP TCP msg#1 80B [type 0101 len 74 (+6 = 80)]
SECURED (58B): 00 0a 00 00 00 10 ... -> Ok(Inner {{ msg_type: 10, sequence: 16 }})
INVESTMENT message id 206 (52 body bytes)
--> BAP SECURED REPLY 30B: {reply_prefix} … (+8 more)
BAP TCP msg#2 80B [type 0101 len 74 (+6 = 80)]
SECURED (58B): 00 0a 00 00 00 11 ... -> Ok(Inner {{ msg_type: 10, sequence: 17 }})
INVESTMENT message id 501 (52 body bytes)
D1_INV_SILENT: NOT answering investment id 501
push plaintext: 00 7b 00 00 00 00 00 00 00 01 00 00 00 04 d1 00
--> BAP QUEUEZ CHARACTER 78B: {push_prefix} … (+56 more)
BAP TCP peer closed after 100B
""".strip()
    emulator_log = """
world_controller:state_manager: Entering state 'character:signin' for reason 'normal_flow'.
networking:investment:server_object_manager:queuez_family_update_begin: ignoring family-update due to unhandled replication state, family_type=ENUM(4), family_root_soid=0xD100000000000001, replication state=1
networking:investment:push:queuez: failed to process update for [family_type ENUM(4), family_root_soid 0x1, version 0, flags 0x01, object_count 1]
world_controller:state_manager: Leaving state 'character:signin' for reason 'error.lost_bap_connection'.
""".strip()
    with tempfile.TemporaryDirectory() as directory:
        root = pathlib.Path(directory) / "self-test-run"
        root.mkdir()
        (root / "responder.log").write_text(responder_log)
        (root / "emulator.log").write_text(emulator_log)
        trace = [
            {
                "schema_version": 1,
                "kind": "start",
                "title_id": "CUSA00219",
                "app_version": "01.29",
            },
            {
                "schema_version": 1,
                "kind": "socket-attempt",
                "sequence": 1,
                "monotonic_ns": 1,
                "socket_key": 123,
                "operation": "connect",
                "result": 0,
                "endpoint": {"address": "10.77.0.2", "port": 3075},
            },
            {
                "schema_version": 1,
                "kind": "socket-attempt",
                "sequence": 2,
                "monotonic_ns": 2,
                "socket_key": 123,
                "operation": "send",
                "result": len(request_frame),
                "payload_available": len(request_frame),
                "payload_captured": len(request_frame),
                "payload_hex": request_frame.hex(),
            },
            {
                "schema_version": 1,
                "kind": "socket-attempt",
                "sequence": 3,
                "monotonic_ns": 3,
                "socket_key": 123,
                "operation": "recv",
                "result": 10,
                "payload_available": 10,
                "payload_captured": 10,
                "payload_hex": reply_frame[:10].hex(),
            },
            {
                "schema_version": 1,
                "kind": "socket-attempt",
                "sequence": 4,
                "monotonic_ns": 4,
                "socket_key": 123,
                "operation": "recv",
                "result": len(reply_frame[10:]) + len(push_frame),
                "payload_available": len(reply_frame[10:]) + len(push_frame),
                "payload_captured": len(reply_frame[10:]) + len(push_frame),
                "payload_hex": (reply_frame[10:] + push_frame).hex(),
            },
            {
                "schema_version": 1,
                "kind": "stop",
                "attempts": 4,
                "enqueued": 4,
                "written": 4,
                "dropped": 0,
                "suppressed_wouldblock": 0,
            },
        ]
        (root / "network-trace.jsonl").write_text(
            "".join(json.dumps(item) + "\n" for item in trace)
        )
        report = parse_run(root, load_expectations(None))
    rows = {row["canonical_key"]: row for row in report["conversations"]}
    assert rows["investment:206"]["server_decision"] == "reply"
    assert rows["investment:206"]["guest_received"] == "yes"
    assert rows["investment:501"]["server_decision"] == "silent"
    assert rows["investment:501"]["outcome"] == "open"
    assert rows["queuez:4"]["server_transmitted"] == "yes"
    assert rows["queuez:4"]["guest_received"] == "yes"
    assert rows["queuez:4"]["client_consumed"] == "not_observed"
    assert rows["queuez:4"]["family_applied"] == "failed"
    assert rows["queuez:4"]["outcome"] == "partial_failure"
    assert rows["queuez:4"]["first_stop"] == "client_family_update_result"
    assert report["coverage"]["guest_socket_trace"] == "ok"
    assert report["coverage"]["bap_frames_sent"] == 1
    assert report["coverage"]["bap_frames_received"] == 2
    assert report["coverage"]["wire_matches"] == 2

    exact_gap = BapStreamDecoder()
    gap_frames = exact_gap.feed(push_frame[:22], len(push_frame) - 22, 1, 1)
    assert len(gap_frames) == 1
    assert gap_frames[0].wire_key is not None
    assert not exact_gap.confounded

    crossed_gap = BapStreamDecoder()
    crossed_gap.feed(push_frame[:22], len(push_frame) - 21, 1, 1)
    assert crossed_gap.confounded

    duplicate_pushes = [
        Event(
            schema_version=1,
            run_id="duplicate",
            source="server_bap",
            source_sequence=index,
            connection_id="bap-1",
            direction="s2c",
            stage="server_push_built",
            status="ok",
            correlation_id=f"s2c:bap-1:007b:{index}",
            inner_type=0x007B,
            inner_sequence=index,
            family=4,
        )
        for index in (1, 2)
    ]
    ambiguous_log = "\n".join([
        "ignoring family-update due to unhandled replication state, "
        "family_type=ENUM(4), family_root_soid=0x1, replication state=1",
        "failed to process update for [family_type ENUM(4), family_root_soid 0x1]",
        "ignoring family-update due to unhandled replication state, "
        "family_type=ENUM(4), family_root_soid=0x1, replication state=1",
        "failed to process update for [family_type ENUM(4), family_root_soid 0x1]",
    ])
    with tempfile.TemporaryDirectory() as directory:
        ambiguous_path = pathlib.Path(directory) / "emulator.log"
        ambiguous_path.write_text(ambiguous_log)
        ambiguous_events = parse_emulator(ambiguous_path, "duplicate", duplicate_pushes)
    family_results = [
        event for event in ambiguous_events
        if event.stage == "client_family_update_result"
    ]
    assert len(family_results) == 2
    assert all(
        event.correlation_id and event.correlation_id.startswith("diag:duplicate:queuez:4:")
        for event in family_results
    )
    assert len({event.correlation_id for event in family_results}) == 2
    print("send_receive_ledger self-test: PASS")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("run_dir", nargs="?", type=pathlib.Path)
    parser.add_argument("--expectations", type=pathlib.Path)
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--events-out", type=pathlib.Path)
    parser.add_argument("--diff", type=pathlib.Path, metavar="OTHER_RUN")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        self_test()
        return 0
    if args.run_dir is None:
        parser.error("run_dir is required unless --self-test is used")
    try:
        expectations = load_expectations(args.expectations)
        report = parse_run(args.run_dir, expectations)
        if args.events_out:
            with args.events_out.open("w") as stream:
                for event in report["events"]:
                    stream.write(json.dumps(event, sort_keys=True) + "\n")
        if args.diff:
            other = parse_run(args.diff, expectations)
            print_diff(report, other)
        elif args.json:
            print(json.dumps(report, indent=2, sort_keys=True))
        else:
            print_text(report)
    except (OSError, ValueError, AssertionError, json.JSONDecodeError) as error:
        print(f"send_receive_ledger: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
