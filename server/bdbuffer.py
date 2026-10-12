"""Demonware bdByteBuffer (typed) reader/writer, as used inside Destiny PS3 lobby messages.

Each value is preceded by a one-byte type tag when the buffer uses data types. Little-endian.
Type numbers follow Demonware's bdBitBufferDataType / bdByteBuffer tags; the ones seen in
the Destiny lobby traffic so far are marked. Lobby frames after authentication (AB 85) are
AES-CBC under the BDDATA keys; the plaintext is u32le length, then the message.
"""

import struct

NO_TYPE, BOOL, INT8, UINT8, WCHAR16, INT16, UINT16, INT32, UINT32, INT64, UINT64 = range(11)
RANGED_INT32, RANGED_UINT32, FLOAT32, FLOAT64, RANGED_FLOAT32 = 11, 12, 13, 14, 15
STRING, USTRING, MBSTRING, BLOB, NAN_TYPE, FULL_TYPE = 16, 17, 18, 19, 20, 21
ARRAY = 100  # array marker: element type + 100

_FIXED = {BOOL: "<?", INT8: "<b", UINT8: "<B", INT16: "<h", UINT16: "<H", INT32: "<i", UINT32: "<I",
          INT64: "<q", UINT64: "<Q", FLOAT32: "<f", FLOAT64: "<d"}


class Reader:
    def __init__(self, data, typed=True):
        self.data, self.pos, self.typed = data, 0, typed

    def left(self):
        return len(self.data) - self.pos

    def raw(self, n):
        out = self.data[self.pos:self.pos + n]
        if len(out) != n:
            raise ValueError("truncated bdByteBuffer")
        self.pos += n
        return out

    def value(self):
        """Read one typed value; returns (type, value)."""
        tag = self.raw(1)[0]
        if tag in _FIXED:
            fmt = _FIXED[tag]
            return tag, struct.unpack(fmt, self.raw(struct.calcsize(fmt)))[0]
        if tag in (STRING, USTRING, MBSTRING):
            end = self.data.index(b"\0", self.pos)
            s = self.data[self.pos:end]
            self.pos = end + 1
            return tag, s
        if tag == BLOB:
            size_tag, size = self.value()
            if size_tag != UINT32:
                raise ValueError(f"blob size tag {size_tag}")
            return tag, self.raw(size)
        if tag > ARRAY:
            elem = tag - ARRAY
            _, total = self.value()          # uint32 byte size of the array payload
            _, count = self.value()          # uint32 element count
            fmt = _FIXED.get(elem)
            if fmt:
                size = struct.calcsize(fmt)
                return tag, [struct.unpack(fmt, self.raw(size))[0] for _ in range(count)]
            return tag, self.raw(total)
        raise ValueError(f"unknown bdByteBuffer tag {tag} at {self.pos - 1}")

    def values(self):
        out = []
        while self.left():
            try:
                out.append(self.value())
            except ValueError as exc:
                out.append(("error", f"{exc}; rest {self.data[self.pos:self.pos + 32].hex()}"))
                break
        return out


class Writer:
    def __init__(self):
        self.out = bytearray()

    def fixed(self, tag, value):
        self.out += bytes([tag]) + struct.pack(_FIXED[tag], value)
        return self

    def string(self, s, tag=STRING):
        self.out += bytes([tag]) + (s.encode() if isinstance(s, str) else s) + b"\0"
        return self

    def blob(self, data):
        self.out += bytes([BLOB])
        self.fixed(UINT32, len(data))
        self.out += data
        return self

    def data(self):
        return bytes(self.out)
