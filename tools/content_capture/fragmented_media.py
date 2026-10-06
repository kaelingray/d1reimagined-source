"""Locate complete MOV/MP4 fragments without changing the original recording."""
import struct


def completed_fragment_prefix(path):
    """Return a recoverable prefix only when an unfinished tail follows full fragments.

    Top-level boxes are skipped by length; media payloads are never loaded into
    memory. A complete initial moov and at least one moof/mdat pair are required.
    Zero-length boxes cannot establish a completed interrupted fragment.
    """
    with path.open("rb") as stream:
        stream.seek(0, 2)
        length = stream.tell()
        stream.seek(0)
        offset = complete = 0
        have_moov = pending_moof = False
        while offset < length:
            if length - offset < 8:
                break
            stream.seek(offset)
            size, kind = struct.unpack(">I4s", stream.read(8))
            if offset == 0 and kind != b"ftyp":
                return None
            header = 8
            if size == 1:
                if length - offset < 16:
                    break
                size = struct.unpack(">Q", stream.read(8))[0]
                header = 16
            if size < header or size > length - offset:
                break
            if kind == b"moov":
                have_moov = True
            elif kind == b"moof":
                if not have_moov or pending_moof:
                    break
                pending_moof = True
            elif kind == b"mdat" and pending_moof:
                complete = offset + size
                pending_moof = False
            offset += size
        if complete and (offset != length or pending_moof):
            return {"retained_bytes": complete, "original_bytes": length,
                    "discarded_tail_bytes": length - complete,
                    "method": "complete MOV/MP4 fragments"}
    return None
