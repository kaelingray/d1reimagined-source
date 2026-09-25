"""Hold RPCS3's GDB stub for a whole emulator session and serve trace requests locally.

RPCS3's stub (127.0.0.1:2345) serves only the first debugger connection of an emulator
session: after that client disconnects the emulator stays paused and later connections
are refused or never answered. This daemon connects once, keeps the game running, and
accepts one-line commands on a loopback control port (default 127.0.0.1:2346):

  hits N SECONDS ADDR[,ADDR...] [REG:LEN,...]
                                  break on ADDRs until N hits or SECONDS pass, then clear
                                  them; per hit: pc, r3-r8, lr, ctr, saved-LR chain, 16
                                  bytes at *r4, for 0x41c0e0 the soid at *r6, and LEN bytes
                                  at each listed register (e.g. r5:512; rN:0 prints the register value)
  mem ADDR LEN                    hex dump of guest memory (big-endian guest addresses)
  ping                            liveness check

Each reply ends with a line "." . Usage:
  python scripts/rpcs3_gdb_daemon.py            (start right after launching RPCS3)
  python scripts/rpcs3_gdb_daemon.py send "hits 40 60 0x41c0e0"
"""
import socket
import struct
import sys
import time

sys.path.insert(0, __file__.rsplit("\\", 1)[0].rsplit("/", 1)[0])
from rpcs3_gdb import Gdb  # noqa: E402

CONTROL = ("127.0.0.1", 2346)
LOOKUP = 0x41C0E0


class Session:
    def __init__(self):
        self.g = Gdb(timeout=10)
        self.g.cmd("?")
        self.running = False
        self.resume()

    def cmd(self, data):
        """Send a command and return its reply, dropping stray stop packets (S/T) that
        a late interrupt can leave queued (they once desynchronized the session and left
        the emulator paused)."""
        reply = self.g.cmd(data)
        while reply[:1] in ("S", "T") and not data.startswith(("?", "vCont")):
            reply = self.g._read()
        return reply

    def wait_stop(self):
        while True:
            reply = self.g._read()
            if reply[:1] in ("S", "T"):
                return reply

    def resume(self):
        self.g.cmd("vCont;c", wait=False)
        self.running = True

    def pause(self):
        if self.running:
            self.g.s.sendall(b"\x03")
            self.g.s.settimeout(10)
            self.wait_stop()
            self.running = False

    def reg(self, num):
        try:
            return int(self.cmd(f"p{num:x}"), 16) & 0xFFFFFFFF
        except ValueError:
            return None

    def threads(self):
        out, r = [], self.cmd("qfThreadInfo")
        while r and r[0] == "m":
            out += r[1:].split(",")
            r = self.cmd("qsThreadInfo")
        return out

    def read(self, addr, size):
        return bytes.fromhex(self.cmd(f"m{addr:x},{size:x}"))

    def chain(self, sp, depth=8):
        out = []
        for _ in range(depth):
            try:
                nxt = struct.unpack(">Q", self.read(sp, 8))[0] & 0xFFFFFFFF
                if not nxt:
                    break
                out.append(hex(struct.unpack(">Q", self.read(nxt + 16, 8))[0] & 0xFFFFFFFF))
                sp = nxt
            except Exception:
                break
        return out

    def hits(self, limit, seconds, addrs, dumps=()):
        lines = []
        self.pause()
        for a in addrs:
            self.cmd(f"Z0,{a:x},4")
        end = time.time() + seconds
        count = 0
        try:
            while count < limit and time.time() < end:
                self.g.s.settimeout(max(0.5, end - time.time()))
                self.g.cmd("vCont;c", wait=False)
                self.running = True
                try:
                    self.wait_stop()
                except (TimeoutError, OSError):
                    break
                self.running = False
                self.g.s.settimeout(10)
                for tid in self.threads():
                    self.cmd(f"Hg{tid}")
                    pc = self.reg(64)
                    if pc not in addrs:
                        continue
                    r = {i: self.reg(i) for i in (1, 3, 4, 5, 6, 7, 8)}
                    lr, ctr = self.reg(67), self.reg(68)
                    text = " ".join(f"r{i}={v:#x}" for i, v in r.items() if i != 1 and v is not None)
                    if r[4] and 0x10000 <= r[4] < 0xF0000000:
                        try:  # e.g. the sockaddr of connect/sendto
                            text += f" m4={self.read(r[4], 16).hex()}"
                        except Exception:
                            pass
                    for reg_no, size in dumps:
                        value = self.reg(reg_no)
                        if size == 0:  # rN:0 prints the register itself (any GPR, e.g. a loop index)
                            text += f" r{reg_no}={value:#x}" if value is not None else f" r{reg_no}=?"
                            continue
                        try:
                            text += f" m{reg_no}={self.read(value, size).hex()}"
                        except Exception:
                            text += f" m{reg_no}=?"
                    if pc == LOOKUP and r[6]:
                        text += f" soid={struct.unpack('>Q', self.read(r[6], 8))[0]:#x}"
                    lines.append(f"{time.strftime('%H:%M:%S')} pc={pc:#x} {text} lr={lr:#x} "
                                 f"ctr={ctr:#x} chain {self.chain(r[1])}")
                    count += 1
        finally:
            self.pause()
            for a in addrs:
                self.cmd(f"z0,{a:x},4")
            self.resume()
        return lines + [f"hits {count}"]

    def mem(self, addr, size):
        self.pause()
        try:
            return [self.read(addr, size).hex()]
        finally:
            self.resume()


def serve():
    session = Session()
    print(f"attached; control on {CONTROL[0]}:{CONTROL[1]}", flush=True)
    with socket.socket() as server:
        server.bind(CONTROL)
        server.listen(1)
        while True:
            conn, _ = server.accept()
            with conn:
                line = conn.makefile().readline().split()
                try:
                    if line[:1] == ["hits"]:
                        dumps = [(int(d.split(":")[0].lstrip("r")), int(d.split(":")[1], 0))
                                 for d in line[4].split(",")] if len(line) > 4 else []
                        out = session.hits(int(line[1]), float(line[2]),
                                           [int(a, 16) for a in line[3].split(",")], dumps)
                    elif line[:1] == ["mem"]:
                        out = session.mem(int(line[1], 16), int(line[2], 0))
                    else:
                        out = ["pong"]
                except Exception as exc:  # report and keep the stub session alive
                    out = [f"error: {exc!r}"]
                print(" ".join(line), "->", len(out), "lines", flush=True)
                print("\n".join(out), flush=True)  # keep results even if the client is gone
                conn.sendall(("\n".join(out) + "\n.\n").encode())


def send(command):
    with socket.create_connection(CONTROL, timeout=10) as conn:
        conn.settimeout(None)  # a "hits" command lasts as long as its own time limit
        conn.sendall(command.encode() + b"\n")
        data = b""
        while not data.endswith(b"\n.\n"):
            chunk = conn.recv(65536)
            if not chunk:
                break
            data += chunk
    print(data.decode().rstrip("\n.").rstrip())


if __name__ == "__main__":
    if sys.argv[1:2] == ["send"]:
        send(" ".join(sys.argv[2:]))
    else:
        serve()
