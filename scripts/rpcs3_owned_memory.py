"""Read 01.24 guest memory without pausing an exactly identified Windows RPCS3.

Only the emulator identity from a run manifest is accepted. The open process
handle stays bound to that process even if its PID is later reused. This uses
the custom build's 4-GiB-aligned guest allocation and verifies EBOOT strings
before accepting a base. No memory writes or debugger operations are used.
"""

import ctypes
import json
import os
from pathlib import Path

FILETIME_EPOCH_TICKS = 504911232000000000
MARKERS = ((0x012FCC40, b"World-transition requires permissions.\0"),
           (0x012E4350, b"activity checksum mismatch\0"))


def emulator_identity(manifest):
    matches = [p for p in manifest.get("processes", []) if p.get("tag") == "rpcs3"]
    if len(matches) != 1:
        raise ValueError("run manifest must identify exactly one emulator")
    identity = matches[0]
    if (not isinstance(identity.get("id"), int) or identity["id"] <= 0
            or not identity.get("executable") or not identity.get("startTimeUtcTicks")):
        raise ValueError("emulator identity is incomplete")
    int(identity["startTimeUtcTicks"])
    return identity


def creation_ticks(high, low):
    return ((high << 32) | low) + FILETIME_EPOCH_TICKS


class OwnedMemory:
    def __init__(self, manifest_path):
        if os.name != "nt":
            raise OSError("direct process reads require Windows")
        from ctypes import wintypes as wt
        identity = emulator_identity(json.loads(Path(manifest_path).read_text(encoding="utf-8-sig")))
        self.kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        k = self.kernel
        k.OpenProcess.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]
        k.OpenProcess.restype = wt.HANDLE
        k.CloseHandle.argtypes = [wt.HANDLE]
        k.GetProcessTimes.argtypes = [wt.HANDLE] + [ctypes.POINTER(wt.FILETIME)] * 4
        k.QueryFullProcessImageNameW.argtypes = [wt.HANDLE, wt.DWORD, wt.LPWSTR, ctypes.POINTER(wt.DWORD)]
        k.ReadProcessMemory.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_void_p,
                                       ctypes.c_size_t, ctypes.POINTER(ctypes.c_size_t)]
        self.handle = k.OpenProcess(0x0410, False, identity["id"])
        self.base = None
        if not self.handle:
            raise ctypes.WinError(ctypes.get_last_error())
        try:
            times = [wt.FILETIME() for _ in range(4)]
            if not k.GetProcessTimes(self.handle, *(ctypes.byref(t) for t in times)):
                raise ctypes.WinError(ctypes.get_last_error())
            actual = creation_ticks(times[0].dwHighDateTime, times[0].dwLowDateTime)
            name, length = ctypes.create_unicode_buffer(32768), wt.DWORD(32768)
            if not k.QueryFullProcessImageNameW(self.handle, 0, name, ctypes.byref(length)):
                raise ctypes.WinError(ctypes.get_last_error())
            if (actual != int(identity["startTimeUtcTicks"])
                    or os.path.normcase(name.value) != os.path.normcase(identity["executable"])):
                raise ValueError("emulator process identity no longer matches this run")
            # vm.cpp reserves at the first available 4-GiB boundary above 8 GiB.
            # Verify both strings; never infer a base from alignment alone.
            for candidate in range(0x300000000, 0x800000000000, 0x100000000):
                if all(self._host_read(candidate + va, len(value)) == value for va, value in MARKERS):
                    self.base = candidate
                    break
            if self.base is None:
                raise ValueError("verified 01.24 guest memory was not found")
        except BaseException:
            self.close()
            raise

    def _host_read(self, address, size):
        buffer, received = ctypes.create_string_buffer(size), ctypes.c_size_t()
        ok = self.kernel.ReadProcessMemory(self.handle, ctypes.c_void_p(address), buffer,
                                          size, ctypes.byref(received))
        return buffer.raw if ok and received.value == size else None

    def read(self, address, size):
        if not self.handle:
            raise ValueError("process reader is closed")
        if address < 0 or size < 0 or size > 64 * 1024 * 1024 or address + size > 0x100000000:
            raise ValueError("guest read is outside the 32-bit address space or size limit")
        data = self._host_read(self.base + address, size)
        if data is None:
            raise OSError(f"guest read failed at {address:#x}, size {size}")
        return data

    def close(self):
        if getattr(self, "handle", None):
            self.kernel.CloseHandle(self.handle)
            self.handle = None

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()
