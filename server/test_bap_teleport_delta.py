"""Type-21 slice deltas arriving before live host teleports, as observed in JOIN88."""

import json
import socket
import tempfile
import threading
import time
import unittest
from pathlib import Path
from unittest.mock import patch

import bap_server as bap
from test_ah_session import FakeClient, PEER, schema


class TeleportDeltaSession(unittest.TestCase):
    target = [24, 0x68935472]

    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.folder = Path(folder.name)
        self.control = self.folder / "control.jsonl"
        self.control.write_text("", encoding="utf-8")
        self.activity_schema = schema()
        path = self.folder / "schema.json"
        path.write_text(json.dumps(self.activity_schema), encoding="utf-8")
        self.args = bap.parse_args([
            "--capture-dir", str(self.folder), "--schema", str(self.folder / "absent.json"),
            "--activity-schema", str(path), "--ah-join", "--ah-state", "--ah-roster", "0",
            "--ah-world-state", "112", "--ah-group-switch", "--ah-heartbeat", "0.005:world",
            "--ah-control", str(self.control), "--ah-teleport-finish-delay", "0.01",
        ])

        def world_data(_schema, _request, zone, *unused, **fields):
            return json.dumps({"zone": zone, **fields}).encode()

        for mocking in (
            patch.object(bap, "world_state_delta", side_effect=world_data),
            patch.object(bap, "activity_state", return_value=b"activity"),
            patch.object(bap, "client_slice_record", side_effect=lambda _s, body: json.loads(body)),
            patch.object(bap, "client_slice_state", return_value="test delta"),
            patch.object(bap, "TELEPORT_SETTLE_SECONDS", 0.04),
            patch.object(bap, "log"),
        ):
            mocking.start()
            self.addCleanup(mocking.stop)

    def start(self):
        server, sock = socket.socketpair()
        sock.settimeout(2)
        thread = threading.Thread(target=bap.handle, args=(server, ("127.0.0.1", 0), self.args), daemon=True)
        thread.start()
        self.addCleanup(thread.join, 2)
        self.addCleanup(sock.close)
        client = FakeClient(sock)
        client.notify(bap.AH_JOIN_REQUEST, bap.investment.encode(
            self.activity_schema, bap.JOIN_REQUEST_TYPE, {0: 7, 8: 9, 0x10: {0: PEER}}))
        self.collect(client, 0.02)
        return client

    def report(self, client, **fields):
        client.notify(bap.AH_CLIENT_STATE, json.dumps(fields).encode())

    def collect(self, client, duration, until=None):
        worlds = []
        deadline = time.monotonic() + duration
        while time.monotonic() < deadline:
            client.notify(15, b"\0")
            for _, subtype, data in client.pushes(0.002):
                if subtype == bap.AH_ESTABLISH:
                    worlds.append(json.loads(data))
            if until and any(until(world) for world in worlds):
                break
        return worlds

    def queue(self, client):
        with self.control.open("a", encoding="utf-8") as stream:
            stream.write(json.dumps({"teleport": f"{self.target[0]}:{self.target[1]:#x}"}) + "\n")
        started = self.collect(client, 1, lambda world: world.get("teleport") == [1, *self.target])
        self.assertTrue(any(world.get("teleport") == [1, *self.target] for world in started), started)
        return started

    def test_target_sent_before_live_teleport_completes_on_state_only_arrival(self):
        client = self.start()
        self.report(client, current=[112, 0x811C9DC5], target=self.target, state=1)
        accepted = self.collect(client, 1, lambda world: world.get("target") == 24)
        self.assertTrue(any(world.get("target") == 24 for world in accepted), accepted)
        started = self.queue(client)
        self.report(client, state=4)
        later = self.collect(client, 0.15)
        finished = [world for world in later if world.get("teleport") == [3, *self.target]]
        self.assertTrue(finished, later)
        self.assertTrue(any(world.get("teleport") is None for world in later), later)
        initial = next(world for world in started if world.get("teleport") == [1, *self.target])
        self.assertEqual(finished[0]["version"], initial["version"])
        self.assertTrue(any(world.get("teleport") is None and world["version"] > initial["version"]
                            for world in later), later)

    def check_invalidated_target(self, invalid):
        client = self.start()
        self.report(client, current=[112, 0x811C9DC5], target=self.target, state=1)
        self.collect(client, 0.02)
        self.queue(client)
        # Invalidate an already matched target, then send the same state-only arrival delta.
        self.report(client, target=invalid)
        self.report(client, state=4)
        worlds = self.collect(client, 0.13)
        self.assertTrue(worlds)
        self.assertFalse(any(world.get("teleport") == [3, *self.target] for world in worlds), worlds)
        self.assertTrue(all(world.get("teleport") == [1, *self.target] for world in worlds), worlds)

    def test_different_target_does_not_finish_the_previous_teleport(self):
        self.check_invalidated_target([32, 0x12345678])

    def test_explicitly_cleared_target_does_not_finish_the_previous_teleport(self):
        self.check_invalidated_target([-1, 0x811C9DC5])

    def test_zero_clear_record_does_not_finish_a_skywatch_teleport(self):
        self.target = [0, 0x811C9DC5]
        self.check_invalidated_target([0, 0])

    def test_target_cache_does_not_cross_activity_host_connections(self):
        previous = self.start()
        self.report(previous, target=self.target, state=1)
        self.collect(previous, 0.02)
        previous.sock.close()
        client = self.start()
        self.queue(client)
        self.report(client, state=4)
        worlds = self.collect(client, 0.13)
        self.assertTrue(worlds)
        self.assertFalse(any(world.get("teleport") == [3, *self.target] for world in worlds), worlds)

    def test_target_change_cancels_a_pending_delayed_finish(self):
        self.args.ah_teleport_finish_delay = 0.1
        client = self.start()
        self.queue(client)
        self.report(client, target=self.target, state=4)
        pending = self.collect(client, 0.015)
        self.assertFalse(any(world.get("teleport") == [3, *self.target] for world in pending), pending)
        self.report(client, target=[-1, 0x811C9DC5])
        worlds = self.collect(client, 0.15)
        self.assertFalse(any(world.get("teleport") == [3, *self.target] for world in worlds), worlds)


if __name__ == "__main__":
    unittest.main()
