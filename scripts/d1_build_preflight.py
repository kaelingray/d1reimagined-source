"""Check the stand-in's core 01.00/update-build payloads against exported type schemas."""

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "server"))
import bap_server as bap_server  # noqa: E402
import investment  # noqa: E402


def check(expected, actual, path):
    """Every value explicitly supplied to the encoder must survive decoding."""
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            raise ValueError(f"{path}: expected a struct")
        for offset, value in expected.items():
            if offset not in actual:
                raise ValueError(f"{path}/{offset:#x}: field omitted")
            check(value, actual[offset], f"{path}/{offset:#x}")
    elif isinstance(expected, list):
        if not isinstance(actual, list) or len(expected) != len(actual):
            raise ValueError(f"{path}: array length changed")
        for index, value in enumerate(expected):
            check(value, actual[index], f"{path}/{index}")
    elif expected != actual:
        raise ValueError(f"{path}: {expected!r} became {actual!r}")


def roundtrip(schema, handle, values, label):
    check(values, investment.decode(schema, handle, investment.encode(schema, handle, values)), label)


def activity_checks(schema):
    request = investment.encode(schema, bap_server.JOIN_REQUEST_TYPE, {0: 1})
    reply = investment.decode(schema, bap_server.JOIN_RESPONSE_TYPE,
                              bap_server.join_response(schema, request, "d1local-activity"))
    fields = schema[f"{bap_server.JOIN_RESPONSE_TYPE:#x}"]["fields"]
    result_offset = next(f["off"] for f in fields if f["kind"] == 3 and f["width"] == 3
                         and f["min"] == 1)
    if reply[0][0] != 1 or reply[result_offset] != 0 or reply[result_offset + 1][0][0] != ord("d"):
        raise ValueError("join response lost its request, result, or session ID")

    state = investment.decode(schema, bap_server.STATE_TYPE,
                              bap_server.activity_state(schema, "act_cosmo_m0_highway", 126,
                                                        0x79130968, 22, (-1, 0x811C9DC5)))[0]
    scene = state[0x60]
    scene_fields = schema["0x80800cc5"]["fields"]
    world_offset = next(f["off"] for f in scene_fields if f["kind"] == 4 and f["width"] == 16
                        and f["min"] == 0x8000)
    map_offset = bap_server.field_offset(schema, "0x80800cc5", "0x8080409f")
    if scene[world_offset] != 126 or scene[map_offset][0][0] != ord("a"):
        raise ValueError("activity state lost its world or map name")

    for label, spec in (("simulation player", bap_server.sim_player(
                         schema, 0x4786C0E0, 5, 0x0A3D4A2C8FBEA6BA, True)),
                        ("game mode", bap_server.game_mode(schema, 0x4786C0E0, 3, 3))):
        roundtrip(schema, spec[1], spec[2], label)

    request_values = investment.decode(schema, bap_server.JOIN_REQUEST_TYPE, request)
    world_delta = investment.BitReader(bap_server.world_state_delta(schema, request_values, zone=112))
    if world_delta.get(1) != 1:
        raise ValueError("world state has no presence bit")
    world = investment.decode_from(world_delta, schema, bap_server.WORLD_STATE_TYPE)
    if (world.get(0x5028) != 1 or world.get(0x502C) != 1
            or world.get(0x2E48, {}).get(0, {}).get(0, {}).get(0) != 112):
        raise ValueError("world state lost its peer masks or zone")
    return result_offset, world_offset, map_offset


def investment_checks(schema):
    layout = bap_server.flag_layout(schema)
    counts = {table: entry["count"] for table, entry in layout.items()}
    values = bap_server.account_flag_values("2", counts)
    account = bap_server.account_flags(schema, "2", layout, values)
    roundtrip(schema, 0x80801803, {0: {0: 7, **account}}, "account flags")
    c = layout["C"]
    character_flags = {field["off"]: values["C"][i] for i, field in enumerate(c["fields"])}
    roundtrip(schema, 0x80801743, {0: {0: 9, c["offset"]: character_flags}}, "character flags")

    gear = bap_server.character_extras(bap_server.gear_spec("starter:titan"),
                                       layout=bap_server.character_layout(schema))
    for label, handle, object_values in (("peer gear", 0x808017FD, gear["peer"]),
                                         ("select gear", 0x80801954, gear["select"]),
                                         ("self gear", 0x80801743, {0: gear["self"]})):
        roundtrip(schema, handle, object_values, label)
    return counts


def service_checks(schema):
    """Every mapped service must have a decodable default stand-in reply."""
    for wire, pair in schema["services"].items():
        payload = bap_server.default_ws_response(schema, int(wire))
        if payload is None:
            raise ValueError(f"service {wire}: no default response")
        reader = investment.BitReader(payload)
        if reader.get(16) != int(wire):
            raise ValueError(f"service {wire}: response wire ID changed")
        investment.decode_from(reader, schema, int(pair[1], 16))
        if reader.get(1):
            raise ValueError(f"service {wire}: unexpected response blob")
    if "503" in schema["services"]:
        expected = {0: {0: 0, 4: 0}, 8: 0x123456789, 16: 1, 20: 1,
                    24: {0: bap_server.UNIVERSE_SOID, 8: 1,
                         0x10: {0: 2, 4: [1, 2]},
                         0x3C: {0: 1, 4: [{0: 3, 2: 1}]}}}
        payload = bap_server.signin_response(0x123456789, universe_flags=(1, 2),
                                             universe_values=((3, 1),), schema=schema)
        reader = investment.BitReader(payload)
        if reader.get(16) != 503:
            raise ValueError("service 503: response wire ID changed")
        actual = investment.decode_from(reader, schema, int(schema["services"]["503"][1], 16))
        check(expected, actual, "service 503 sign-in")
        if reader.get(1):
            raise ValueError("service 503: unexpected response blob")
    if "104" in schema["services"]:
        args = argparse.Namespace(universe_time="one", universe_flags=(1, 2),
                                  universe_values=((3, 1),))
        values = bap_server.universe_reply(args, schema)
        payload = bap_server.default_ws_response(schema, 104, values)
        reader = investment.BitReader(payload)
        if reader.get(16) != 104:
            raise ValueError("service 104: response wire ID changed")
        actual = investment.decode_from(reader, schema, int(schema["services"]["104"][1], 16))
        if values is not None:
            check(values, actual, "service 104 universe")
        else:
            fields = schema[schema["services"]["104"][1]]["fields"]
            has_bool = any(field["off"] == 8 and field["kind"] == 2 for field in fields)
            if (has_bool and actual.get(8) is not False) or (not has_bool and 8 in actual):
                raise ValueError("service 104: unexpected response field")
        if reader.get(1):
            raise ValueError("service 104: unexpected response blob")
    return len(schema["services"])


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--activity-schema", type=Path, required=True)
    ap.add_argument("--investment-schema", type=Path, required=True)
    ap.add_argument("--services-schema", type=Path, required=True,
                    help="service ID map; required for an EBOOT-only type export")
    ap.add_argument("missions", nargs="*", type=Path)
    args = ap.parse_args(argv)
    try:
        activity = investment.load_schema(args.activity_schema)
        investment_schema = bap_server.with_service_map(
            investment.load_schema(args.investment_schema), investment.load_schema(args.services_schema))
        result_offset, world_offset, map_offset = activity_checks(activity)
        print(f"PASS activity join/state/player/game mode/world: result {result_offset:#x}, "
              f"world {world_offset:#x}, map {map_offset:#x}")
        counts = investment_checks(investment_schema)
        service_count = service_checks(investment_schema)
        print(f"PASS services/flags/starter gear: {service_count} encodable services, "
              + ", ".join(f"{name}={count}" for name, count in counts.items()))
        for path in args.missions:
            mission = json.loads(path.read_text(encoding="utf-8"))
            timed, reactive = bap_server.expand_mission(mission, activity)
            bap_server.validate_mission_states(activity, timed, reactive)
            print(f"PASS {path}: {len(timed) + len(reactive)} state changes")
    except (OSError, KeyError, TypeError, ValueError, StopIteration) as exc:
        print(f"FAIL {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
