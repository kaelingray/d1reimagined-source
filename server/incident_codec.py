"""Bounded codec for native 01.24 activity incident envelopes."""
import investment

INCIDENT_HANDLE = 0x8080097D
MAX_PAYLOAD = 8192


def validate_schema(schema, handle):
    """Require the measured list/list/byte-body layout, not a guessed schema."""
    root = schema.get(f"{handle:#x}", {})
    if [f.get("off") for f in root.get("fields", [])] != [0, 104, 212]:
        raise ValueError("incident schema must contain the 01.24 fields at 0, 104 and 212")
    for field in root["fields"]:
        if field.get("kind") != 1 or field.get("arr") != 0:
            raise ValueError("incident schema has an unexpected nested-list layout")
        nested = schema.get(f"{field['nested']:#x}", {}).get("fields", [])
        if len(nested) != 2 or [f.get("off") for f in nested] != [0, 4]:
            raise ValueError("incident list schema must contain count and slots")
        count, array = nested
        if (count.get("kind") != 5 or not 1 <= count.get("width", 0) <= 32
                or count.get("min") != 0 or count.get("opt")
                or array.get("kind") != 1 or array.get("arr") != 1
                or array.get("width") != 0 or array.get("opt")):
            raise ValueError("incident list schema has an unexpected count or array")
        slots = schema.get(f"{array['nested']:#x}", {}).get("fields", [])
        if not 1 <= len(slots) <= MAX_PAYLOAD:
            raise ValueError("incident list capacity is missing or exceeds the parser bound")
    _validate_schema_tree(schema, handle)


def _validate_schema_tree(schema, handle, ancestors=()):
    if handle in ancestors or len(ancestors) >= 8:
        raise ValueError("incident schema is recursive or exceeds the nesting bound")
    fields = schema.get(f"{handle:#x}", {}).get("fields", [])
    if not fields or len(fields) > MAX_PAYLOAD:
        raise ValueError("incident schema has missing or excessive fields")
    for field in fields:
        if field["kind"] == 1:
            if field["arr"] not in (0, 1):
                raise ValueError("unsupported incident array mode")
            _validate_schema_tree(schema, field["nested"], (*ancestors, handle))
        elif (field["kind"] not in (2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0x23)
              or (field["kind"] not in (2, 11, 0x23) and not 1 <= field["width"] <= 64)):
            raise ValueError("incident schema contains an unsupported primitive")


def _validate_values(schema, handle, values):
    for field in schema[f"{handle:#x}"]["fields"]:
        if field["kind"] != 1 or field["off"] not in values:
            continue
        value = values[field["off"]]
        if field["arr"] == 1:
            slots = schema[f"{field['nested']:#x}"]["fields"]
            if values.get(field["width"]) != len(value):
                raise ValueError("incident count exceeds schema slot capacity")
            _validate_values(schema, field["nested"], {f["off"]: v for f, v in zip(slots, value)})
        else:
            _validate_values(schema, field["nested"], value)


def decode_incident(schema, handle, payload):
    reader = investment.BitReader(payload)
    values = investment.decode_from(reader, schema, handle)
    _validate_values(schema, handle, values)
    remainder = len(payload) * 8 - reader.position
    if remainder >= 8 or (remainder and reader.get(remainder)):
        raise ValueError("incident payload has unconsumed data or nonzero padding")
    types = tuple(values[0][4])
    if any(not isinstance(value, int) or value < 0 for value in types):
        raise ValueError("incident type IDs must be nonnegative integers")
    return types, bytes(values[212][4])


def encode_incident(schema, types, body):
    """Encode a host incident envelope; the caller must establish the opaque body's meaning."""
    validate_schema(schema, INCIDENT_HANDLE)
    root = schema[f"{INCIDENT_HANDLE:#x}"]["fields"]
    lists = [schema[f"{f['nested']:#x}"]["fields"] for f in root]
    type_slots = schema[f"{lists[0][1]['nested']:#x}"]["fields"]
    body_slots = schema[f"{lists[2][1]['nested']:#x}"]["fields"]
    if not isinstance(types, (list, tuple)) or not 1 <= len(types) <= len(type_slots):
        raise ValueError("host incident needs a nonempty bounded type chain")
    for index, value in enumerate(types):
        field = type_slots[index]
        if (type(value) is not int or value < 0
                or not 0 <= value + field['min'] < 1 << field['width']):
            raise ValueError("host incident type ID does not fit its schema slot")
    if not isinstance(body, bytes) or len(body) > len(body_slots):
        raise ValueError("host incident body exceeds its schema capacity")
    for fields, count in ((lists[0], len(types)), (lists[2], len(body))):
        if count >= 1 << fields[0]['width']:
            raise ValueError("host incident count does not fit its schema field")
    # The second list carries no entries for this explicit host-event path.
    values = {0: {0: len(types), 4: list(types)}, 104: {0: 0, 4: []},
              212: {0: len(body), 4: list(body)}}
    payload = investment.encode(schema, INCIDENT_HANDLE, values)
    if len(payload) > MAX_PAYLOAD:
        raise ValueError("host incident envelope exceeds the parser bound")
    return payload

