"""Trace breakpoint hits in RPCS3 via its GDB stub (config: GDB Server 127.0.0.1:2345).

usage: python scripts/rpcs3_gdb_trace.py SECONDS ADDR[,ADDR...] [--once] [--mem ADDR:LEN,...]
RPCS3's stub pauses the emulator on connect, supports vCont;c/s, Z0/z0, p/g/m, and
replies S05 (no thread id) on a stop. Each hit logs the thread whose PC is on a
breakpoint, r1-r10, LR and a back-chain walk of saved LRs.
"""
import struct
import sys
import time

from rpcs3_gdb import Gdb

secs = float(sys.argv[1])
addrs = [int(a, 16) for a in sys.argv[2].split(",")]
once = "--once" in sys.argv
g = Gdb(timeout=10)
print("?", g.cmd("?"), flush=True)


def threads():
    out, r = [], g.cmd("qfThreadInfo")
    while r and r[0] == "m":
        out += r[1:].split(",")
        r = g.cmd("qsThreadInfo")
    return out


def reg(tid, num):
    g.cmd(f"Hg{tid}")
    v = g.cmd(f"p{num:x}")
    try:
        return int(v, 16)
    except ValueError:
        return None


for a in addrs:
    print(f"Z0 {a:#x}", g.cmd(f"Z0,{a:x},4"), flush=True)
end = time.time() + secs
hits = 0
while time.time() < end:
    g.s.settimeout(max(1.0, end - time.time()))
    g.cmd("vCont;c", wait=False)
    try:
        stop = g._read()
    except (TimeoutError, OSError):
        break
    g.s.settimeout(10)
    found = False
    for tid in threads():
        pc = reg(tid, 64)
        if pc is None or (pc & 0xFFFFFFFF) not in addrs:
            continue
        found = True
        r = {f"r{i}": reg(tid, i) for i in (1, 3, 4, 5, 6, 7, 8, 9, 10)}
        lr = reg(tid, 67)
        chain, sp = [], r["r1"]
        for _ in range(12):
            try:
                nxt = struct.unpack(">Q", g.mem(sp, 8))[0] & 0xFFFFFFFF
                if not nxt:
                    break
                chain.append(hex(struct.unpack(">Q", g.mem(nxt + 16, 8))[0] & 0xFFFFFFFF))
                sp = nxt
            except Exception:
                break
        hits += 1
        regs = " ".join(f"{k}={v & 0xFFFFFFFF:#x}" for k, v in r.items() if v is not None)
        print(time.strftime("%H:%M:%S"), f"HIT {hits} thread {tid} pc={pc & 0xFFFFFFFF:#x} lr={(lr or 0) & 0xFFFFFFFF:#x} {regs} chain {chain}", flush=True)
        if once:
            g.cmd(f"z0,{pc & 0xFFFFFFFF:x},4")
            addrs = [a for a in addrs if a != pc & 0xFFFFFFFF]
    if not found:
        print("stop without breakpoint thread:", stop, flush=True)
print("done, hits", hits, flush=True)
g.s.sendall(b"\x03")
time.sleep(0.5)
g.s.settimeout(3)
try:
    g._read()
except Exception:
    pass
for a in addrs:
    try:
        g.cmd(f"z0,{a:x},4")
    except Exception:
        pass
g.s.settimeout(1)
g.cmd("vCont;c", wait=False)  # leave the emulator running
g.s.close()
