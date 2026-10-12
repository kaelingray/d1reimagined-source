"""Minimal GDB remote-serial-protocol client for RPCS3's built-in stub (loopback only).

RPCS3 (Emu/GDB.cpp) pauses the emulator on connect and supports ?, qfThreadInfo/
qsThreadInfo, H, p/P (64 = pc, 67 = lr, r0-r31 = 0-31), g, m/M, Z0/z0 and vCont;c/s
(not plain c); a stop is reported as S05 without a thread id.
"""
import socket, time
class Gdb:
    def __init__(self, host="127.0.0.1", port=2345, timeout=10):
        self.s = socket.create_connection((host, port), timeout=timeout); self.buf = b""
    def _read(self):
        while True:
            i = self.buf.find(b"$")
            if i >= 0:
                j = self.buf.find(b"#", i)
                if j >= 0 and len(self.buf) >= j + 3:
                    pkt = self.buf[i + 1:j]; self.buf = self.buf[j + 3:]
                    self.s.sendall(b"+"); return pkt.decode("latin1")
            chunk = self.s.recv(65536)
            if not chunk: raise EOFError
            self.buf += chunk
    def cmd(self, data, wait=True):
        pkt = data.encode("latin1")
        self.s.sendall(b"$" + pkt + b"#" + b"%02x" % (sum(pkt) & 0xFF))
        return self._read() if wait else None
    def mem(self, addr, n):
        return bytes.fromhex(self.cmd(f"m{addr:x},{n:x}"))
