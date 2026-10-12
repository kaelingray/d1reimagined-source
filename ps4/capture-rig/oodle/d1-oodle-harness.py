#!/usr/bin/env python3
import hashlib
import os
import struct
import sys

# Unicorn is not packaged for this host's Python. Search a durable path first, then the original
# /tmp location (still live - another lane uses it). Recreate either with:
#     python3 -m pip install --target ~/.local/lib/d1-tools unicorn
# ⚠ Paths are derived from this file's own location / expanded env vars - a 2026-08-10 scrub pass
# replaced literal home paths with the un-expanded string "$HOME/…", which open() cannot read, and
# every consumer of this harness (extract-entry.py, item-index.py, content-table.py) died on import.
for _p in (os.path.expandvars("$HOME/.local/lib/d1-tools"), "/tmp/d1-unicorn"):
    if os.path.isdir(os.path.join(_p, "unicorn")):
        sys.path.insert(0, _p)
        break
from unicorn import (Uc, UC_ARCH_X86, UC_MODE_64, UC_HOOK_CODE, UC_HOOK_INTR,
                     UC_HOOK_INSN_INVALID)
from unicorn.x86_const import *

_ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
EBOOT = os.path.join(_ROOT, "dumps", "patch-01.29-CUSA00219", "eboot.bin")
PKG = os.path.join(_ROOT, "dumps", "patch-01.29-CUSA00219", "packages",
                   "ps4_investment_globals_012c_1.pkg")

PAGE = 0x1000
STACK = 0x4000000
STACK_SIZE = 0x200000
# The slot `call()` writes the STOP sentinel into - i.e. the outermost return address. Everything
# strictly below it is legitimate frame space for the code under emulation; anything at or above it
# belongs to the harness. The memset guard below is expressed against this, not against the whole
# stack mapping.
RET_SLOT = STACK + STACK_SIZE - 0x10008
HEAP = 0x5000000
HEAP_SIZE = 0x1000000
INPUT = 0x7000000
OUTPUT = 0x7100000
STOP = 0x7fff000
CANARY_PTR = 0x7ffe000


def align(x, a=PAGE):
    return (x + a - 1) & -a


class Harness:
    def __init__(self):
        self.mu = Uc(UC_ARCH_X86, UC_MODE_64)
        # ⚠ AVX IS NOT AVAILABLE IN THIS UNICORN (2.1.4). Destiny's RSAT codecs are AVX-compiled
        # and hit `vxorps ymm0,ymm0,ymm0` at 0x105DF95, which raises UC_ERR_INSN_INVALID - this reads
        # like a decode/mapping fault but is a missing CPU feature. Measured: AVX fails on the
        # default model AND on HASWELL/SKYLAKE_CLIENT, and `UC_X86_REG_XCR0` does not exist in this
        # build, so the YMM state cannot be enabled. Setting a CPU model does NOT fix it - an earlier
        # comment here claimed it did; that was wrong.
        # Handled instead by the fail-closed AVX shim below (UC_HOOK_INSN_INVALID).
        self.mu.mem_map(0, 0x3000000)
        self.mu.mem_map(STACK, STACK_SIZE)
        self.mu.mem_map(HEAP, HEAP_SIZE)
        self.mu.mem_map(INPUT, 0x100000)
        self.mu.mem_map(OUTPUT, 0x100000)
        self.mu.mem_map(STOP & -PAGE, PAGE)
        self.mu.mem_map(CANARY_PTR & -PAGE, PAGE)
        self.mu.mem_write(STOP, b"\xcc")
        self.mu.mem_write(CANARY_PTR, struct.pack("<Q", 0x1122334455667788))
        self.heap_next = HEAP
        self.depth = 0
        self.steps = 0
        self.trace = False
        self._load_eboot()
        # Stack-protector indirection used by these functions.
        self.mu.mem_write(0x1A06C78, struct.pack("<Q", CANARY_PTR))
        self.ymm = {}                      # shim-visible YMM state, only what we emulate
        self.mu.hook_add(UC_HOOK_INSN_INVALID, lambda *a: self._avx(*a))
        # ⚠ Dispatch through the ATTRIBUTE, not the bound method. `hook_add(..., self._code)`
        # captures the method object at registration, so a caller that later does `H._code = mine`
        # gets its wrapper registered as a SECOND hook while the original still runs - the original
        # handler then executes TWICE per instruction, `_ret` pops the stack twice, and RIP lands on
        # garbage. That presented as "the guest dispatched to a null import" and silently corrupted
        # every run of trace-investment-schema.py. One hook, resolved late.
        self.mu.hook_add(UC_HOOK_CODE, lambda *a: self._code(*a))
        self.mu.hook_add(UC_HOOK_INTR, self._intr)

    def _load_eboot(self):
        data = open(EBOOT, "rb").read()
        self.mu.mem_write(0x0000000, data[0x004000:0x004000 + 0x18BF0DC])
        self.mu.mem_write(0x18C0000, data[0x18C4000:0x18C4000 + 0x149088])
        dyn = 0x1A0D100
        rela_off = dyn + 0xBDD0
        rela_sz = 0x1B0918
        for p in range(rela_off, rela_off + rela_sz, 24):
            off, info, add = struct.unpack_from("<QQq", data, p)
            if (info & 0xFFFFFFFF) == 8 and 0 <= off <= 0x2FFFFFF:
                self.mu.mem_write(off, struct.pack("<Q", add & 0xFFFFFFFFFFFFFFFF))

    def _ret(self, rax=0):
        rsp = self.mu.reg_read(UC_X86_REG_RSP)
        target = struct.unpack("<Q", self.mu.mem_read(rsp, 8))[0]
        self.mu.reg_write(UC_X86_REG_RAX, rax & 0xFFFFFFFFFFFFFFFF)
        self.mu.reg_write(UC_X86_REG_RSP, rsp + 8)
        self.mu.reg_write(UC_X86_REG_RIP, target)

    def alloc(self, size, alignment=16):
        p = align(self.heap_next, max(16, alignment))
        self.heap_next = p + size
        if self.heap_next > HEAP + HEAP_SIZE:
            raise RuntimeError("scratch heap exhausted")
        self.mu.mem_write(p, b"\0" * size)
        return p

    def _code(self, uc, address, size, _):
        self.steps += 1
        if self.steps > 50_000_000:
            raise RuntimeError("instruction limit")
        if self.trace and (self.steps < 100 or address < 0x100000):
            print("TRACE", hex(address), uc.mem_read(address, size).hex())
        if address == STOP:
            uc.emu_stop()
            return
        if address in (0xF8ED00,):
            self._ret(0)
            return
        if address == 0x1273000:  # aligned allocation
            n = uc.reg_read(UC_X86_REG_RDI)
            a = uc.reg_read(UC_X86_REG_RSI)
            self._ret(self.alloc(n, a))
            return
        if address == 0x1273090:  # free
            self._ret(0)
            return
        if address in (0x129D080, 0x129D370):  # memmove/memcpy imports
            dst = uc.reg_read(UC_X86_REG_RDI)
            src = uc.reg_read(UC_X86_REG_RSI)
            n = uc.reg_read(UC_X86_REG_RDX)
            uc.mem_write(dst, bytes(uc.mem_read(src, n)))
            self._ret(dst)
            return
        if address == 0x129D240:  # memset import
            dst = uc.reg_read(UC_X86_REG_RDI)
            val = uc.reg_read(UC_X86_REG_RSI) & 0xFF
            n = uc.reg_read(UC_X86_REG_RDX)
            # ⚠ GUARD: a memset through an UNPOPULATED pointer in our own fake context will happily
            # zero the stack, wiping the return address - the emulation then "dispatches to 0" and
            # looks like an unresolved import in the guest. That is a harness artefact, not a client
            # defect, and it cost real time to tell apart. Refuse and say so.
            rsp = uc.reg_read(UC_X86_REG_RSP)
            # ⚠ FIXED 2026-08-08: the old test was `dst < STACK+STACK_SIZE and dst+n > STACK`,
            # i.e. "touches the stack mapping at all". That is over-broad - a callee zeroing its own
            # stack frame is completely legitimate and was being refused, which made the whole
            # harness unusable for the investment-globals decode path. The property we actually need
            # is "stays inside the frame window": at or below the outermost return slot, and not
            # below the red zone.
            in_stack = dst < STACK + STACK_SIZE and dst + n > STACK
            if in_stack and (dst + n > RET_SLOT or dst + n < rsp - 128):
                raise RuntimeError(
                    f"memset would clobber the STACK: dst={dst:#x} n={n:#x} rsp={rsp:#x} "
                    f"- the destination pointer is unpopulated scaffolding, fix the caller's context")
            uc.mem_write(dst, bytes([val]) * n)
            self._ret(dst)
            return
        if address == 0x129D0B0:
            raise RuntimeError("stack_chk_fail")
        # Oodle's optional assert/log callback is an unresolved function pointer.
        if address == 0:
            # Log WHO called. A null import is an unrelocated pointer; without the call site you
            # cannot tell an optional callback (safe to stub) from a load-bearing one (not safe).
            try:
                ret = struct.unpack("<Q", uc.mem_read(uc.reg_read(UC_X86_REG_RSP), 8))[0]
            except Exception:
                ret = 0
            print(f"WARN called null import; stubbing  (return address {ret:#x})")
            self._ret(0)

    def _avx(self, uc, _user=None):
        """Emulate the handful of AVX encodings Destiny's codecs use. FAIL CLOSED: anything not
        explicitly recognised returns False so Unicorn still raises, rather than silently skipping
        an instruction and producing a plausible-but-wrong trace."""
        rip = uc.reg_read(UC_X86_REG_RIP)
        b = bytes(uc.mem_read(rip, 16))
        # VEX.256 vxorps ymmN, ymmN, ymmN  ->  zero ymmN      (c5 <vvvv> 57 /r)
        if b[0] == 0xC5 and b[2] == 0x57:
            reg = (b[3] >> 3) & 7
            self.ymm[reg] = b"\x00" * 32
            uc.reg_write(UC_X86_REG_RIP, rip + 4)
            return True
        # VEX.256 vmovaps [rsp+disp8], ymmN                    (c5 fc 29 44 24 <disp8>)
        if b[:3] == b"\xc5\xfc\x29" and b[3] == 0x44 and b[4] == 0x24:
            reg = (b[3] >> 3) & 7
            val = self.ymm.get(reg)
            if val is None:
                return False                    # never invent register contents
            uc.mem_write(uc.reg_read(UC_X86_REG_RSP) + b[5], val)
            uc.reg_write(UC_X86_REG_RIP, rip + 6)
            return True
        print(f"AVX shim: UNHANDLED encoding at {rip:#x}: {b[:8].hex()}")
        return False

    def _intr(self, uc, intno, _):
        print("INT", hex(intno), "at", hex(uc.reg_read(UC_X86_REG_RIP)))
        if intno in (3, 0x41):
            # Assertions are diagnostic; keep going past the two-byte int.
            return
        raise RuntimeError("interrupt %x" % intno)

    def call(self, addr, *args):
        regs = [UC_X86_REG_RDI, UC_X86_REG_RSI, UC_X86_REG_RDX,
                UC_X86_REG_RCX, UC_X86_REG_R8, UC_X86_REG_R9]
        rsp = RET_SLOT
        self.mu.mem_write(rsp, struct.pack("<Q", STOP))
        for i, a in enumerate(args[:6]):
            self.mu.reg_write(regs[i], a)
        for i, a in enumerate(args[6:]):
            self.mu.mem_write(rsp + 8 + i * 8, struct.pack("<Q", a & 0xFFFFFFFFFFFFFFFF))
        self.mu.reg_write(UC_X86_REG_RSP, rsp)
        self.mu.reg_write(UC_X86_REG_RBP, 0)
        self.steps = 0
        self.mu.emu_start(addr, STOP + 1)
        return self.mu.reg_read(UC_X86_REG_RAX)


def get_block(path, off, size):
    data = open(path, "rb").read()
    b = data[off:off + size]
    return b


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else PKG
    block_off = int(sys.argv[2], 0) if len(sys.argv) > 2 else 448512
    block_size = int(sys.argv[3], 0) if len(sys.argv) > 3 else 49820
    ranges = sys.argv[4:] or ["146768:128", "146896:128", "147024:128", "147152:128"]
    h = Harness()
    b = get_block(path, block_off, block_size)
    h.mu.mem_write(INPUT, b)
    need = h.call(0x1267120, 0)
    print("decoder memory", hex(need))
    dec_mem = h.alloc(need, 16)
    decoder = h.call(0x12671A0, 0, 0x40000, dec_mem)
    print("decoder", hex(decoder))
    result = h.alloc(0x20, 16)
    dec_pos = 0
    comp_pos = 0
    call_no = 0
    while dec_pos < 0x40000:
        call_no += 1
        h.mu.mem_write(result, b"\0" * 0x20)
        ok = h.call(0x12672E0, decoder, OUTPUT, dec_pos, 0x40000,
                    0x40000 - dec_pos, INPUT + comp_pos, len(b) - comp_pos,
                    0, 0, result)
        vals = struct.unpack("<8I", h.mu.mem_read(result, 0x20))
        print("decode", call_no, ok, "at", dec_pos, comp_pos, "out", vals)
        if not ok or vals[0] == 0:
            break
        dec_pos += vals[0]
        comp_pos += vals[1]
    print("totals", dec_pos, comp_pos, "of", len(b))
    out = bytes(h.mu.mem_read(OUTPUT, 0x40000))
    print("head", out[:64].hex())
    for spec in ranges:
        off, size = (int(x, 0) for x in spec.split(":"))
        print(spec, out[off:off + size].hex())


if __name__ == "__main__":
    main()
