#!/usr/bin/env python3
"""d1-wincount - entry hit counters in the live 56578 client (shadPS4 on Windows).

    d1-wincount.py install            # hook every TARGET, zero the counters
    d1-wincount.py read               # print the counters
    d1-wincount.py remove             # restore the original bytes exactly

A diagnostic, not a fix: it never changes what a function does. Each hook relocates the function's
first whole instructions (checked: push/mov only, no rip-relative operand, no branch) into a stub
in a page allocated within +-2 GB of the image, and that stub does `lock inc qword [counter]`, runs
the relocated bytes and jumps back. The entry gets a 5-byte `jmp stub` plus NOP padding. State
(page address, original bytes) is kept in d1-wincount.json beside this file so `remove` restores
exactly what was there; `install` refuses if a hook is already present.
"""
import ctypes, ctypes.wintypes as wt, importlib.util, json, os, struct, sys
import capstone

HERE = os.path.dirname(os.path.abspath(__file__))
STATE = os.path.join(HERE, 'd1-wincount.json')
spec = importlib.util.spec_from_file_location('winguest', os.path.join(HERE, 'd1-winguest.py'))
wg = importlib.util.module_from_spec(spec); spec.loader.exec_module(wg)

# The udt-5 sensor_auth_update path on build 56578 (see notes/ps4-retarget-2026-10-12.md).
TARGETS_SENSOR_PATH = [
    (0x29DDF0, 'udt-5 RX handler (dispatch table idx 6)'),
    (0xBB1D00, 'sim-queue case: resolve world by ah_id'),
    (0x2A1C40, 'sensor frame decoder (gen check 0x2A1E88)'),
    (0x2D0CD0, 'named-globals walker (name-ref lookup)'),
    (0x2D2F30, 'setter post-pass (caller of the dispatch)'),
    (0x2CED30, 'post-pass dispatch (guards record+0xA8 / +0xB0)'),
    (0x11DE50, 'phase setter (memcpy record+0x1AC, 0x504)'),
]
# Second pass: inside the walker (0x2D0CD0), which runs per frame but never reaches the post-pass
# 0x2D2F20 -> 0x2D2F30. Its early exits sit around these calls.
TARGETS_WALKER = [
    (0x2D0CD0, 'named-globals walker (entry)'),
    (0x2CEC30, 'walker: name-ref -> record lookup'),
    (0x345620, 'walker: called after the record resolves'),
    (0x2C9AE0, 'replicated decode (value block; has other callers)'),
    (0x2D2F20, 'walker -> post-pass bridge'),
]
# Third pass: does the walker reach its frame terminator (has_entry = 0 -> 0x2D1600)?
TARGETS_TERMINATOR = [
    (0x2D0CD0, 'named-globals walker (entry)'),
    (0x2D1600, 'walker terminator: called when has_entry = 0'),
    (0x2D2F20, 'walker -> post-pass bridge'),
]
# Forced-spawn pass: how far does the spawner (01.29 FUN_00411830 -> 0x412800) get once the
# getter hook, the reject NOP and the spawn word are in place?
TARGETS_SPAWN = [
    (0x412800, 'spawner (per tick)'),
    (0x415700, 'spawn gate (accepts phase 3/6)'),
    (0x10CB60, 'lifetime +0x674/+0x678 reader'),
    (0x32DBF0, 'construction candidate (01.29 FUN_0032D180)'),
    (0x32E340, 'construction candidate 2'),
]
TARGETS = {'walker': TARGETS_WALKER, 'term': TARGETS_TERMINATOR, 'spawn': TARGETS_SPAWN}.get(os.environ.get('D1_COUNT_SET', ''), TARGETS_SENSOR_PATH)
# D1_COUNT_ADDRS=0x10c500,0x240840 counts ad-hoc function entries instead of a named set.
if os.environ.get('D1_COUNT_ADDRS'):
    TARGETS = [(int(x, 16), 'ad hoc') for x in os.environ['D1_COUNT_ADDRS'].split(',')]
STUB = 0x40

k32 = ctypes.WinDLL('kernel32', use_last_error=True)
k32.VirtualAllocEx.restype = ctypes.c_void_p
k32.VirtualAllocEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t, wt.DWORD, wt.DWORD]
k32.VirtualFreeEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t, wt.DWORD]
k32.VirtualProtectEx.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t, wt.DWORD, ctypes.POINTER(wt.DWORD)]
k32.FlushInstructionCache.argtypes = [wt.HANDLE, ctypes.c_void_p, ctypes.c_size_t]
md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)


def patch(p, addr, data):
    old = wt.DWORD()
    if not k32.VirtualProtectEx(p.h, ctypes.c_void_p(addr), len(data), 0x40, ctypes.byref(old)):
        raise OSError(f'VirtualProtectEx {addr:#x} failed ({ctypes.get_last_error()})')
    p.write(addr, data)
    k32.VirtualProtectEx(p.h, ctypes.c_void_p(addr), len(data), old.value, ctypes.byref(wt.DWORD()))
    k32.FlushInstructionCache(p.h, ctypes.c_void_p(addr), len(data))


def prologue(p, addr):
    code, n, out = p.read(addr, 0x20), 0, []
    for ins in md.disasm(code, addr):
        # D1_COUNT_ANY=1 also allows mid-function block starts: any instruction that is position-
        # independent (no rip operand) and not control flow, memory operands included.
        anyok = os.environ.get('D1_COUNT_ANY') and 'rip' not in ins.op_str and not (
            ins.mnemonic.startswith('j') or ins.mnemonic in ('call', 'ret', 'loop', 'int3'))
        if not anyok and (ins.mnemonic not in ('push', 'mov', 'xor') or 'rip' in ins.op_str or '[' in ins.op_str):
            sys.exit(f'REFUSING {addr:#x}: {ins.mnemonic} {ins.op_str} is not a relocatable prologue instruction')
        out.append(ins); n += ins.size
        if n >= 5:
            return code[:n]
    sys.exit(f'REFUSING {addr:#x}: no 5-byte prologue')


def alloc_near(p, near):
    for k in range(1, 0x4000):
        for cand in (near - k * 0x10000, near + 0x3000000 + k * 0x10000):
            a = k32.VirtualAllocEx(p.h, ctypes.c_void_p(cand & ~0xFFFF), 0x1000, 0x3000, 0x40)
            if a:
                return a
    sys.exit('could not allocate a page within 2 GB of the image')


def hook(p, base, page, i, va, why, orig):
    fn = base + va
    stub = page + i * STUB
    counter = stub + STUB - 8
    body = bytes.fromhex('f048ff05') + struct.pack('<i', counter - (stub + 8))   # lock inc qword [rip+x]
    body += orig
    back = fn + len(orig)
    body += bytes.fromhex('e9') + struct.pack('<i', back - (stub + len(body) + 5))
    p.write(stub, body.ljust(STUB - 8, bytes.fromhex('cc')) + bytes(8))
    entry = bytes.fromhex('e9') + struct.pack('<i', stub - (fn + 5))
    patch(p, fn, entry + bytes.fromhex('90') * (len(orig) - 5))
    print(f'hooked {va:#x} ({why}): {len(orig)} bytes relocated, stub {stub:#x}')
    return {'va': va, 'why': why, 'orig': orig.hex(), 'counter': counter}


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else 'read'
    p, base = wg.open_client()
    st = json.load(open(STATE)) if os.path.exists(STATE) else None
    if cmd == 'install':
        if st and st.get('base') == base:
            sys.exit('hooks already installed in this process; run remove first')
        # Check every prologue BEFORE touching anything: a refusal mid-way used to leave the earlier
        # hooks live with no state file to remove them (2026-10-13, the lifetime getter 0x10C500).
        origs = [prologue(p, base + va) for va, _ in TARGETS]
        page = alloc_near(p, base)
        saved = []
        if wg.ntdll.NtSuspendProcess(p.h) != 0:
            sys.exit('NtSuspendProcess failed')
        try:
            for i, ((va, why), orig) in enumerate(zip(TARGETS, origs)):
                saved.append(hook(p, base, page, i, va, why, orig))
        finally:
            wg.ntdll.NtResumeProcess(p.h)
        json.dump({'base': base, 'page': page, 'hooks': saved}, open(STATE, 'w'), indent=1)
        return 0
    if not st or st.get('base') != base:
        sys.exit('no hooks recorded for this process')
    if cmd == 'read':
        for h in st['hooks']:
            print(f"{h['va']:#010x} {p.u(h['counter'], 'Q'):8d}  {h['why']}")
        return 0
    if cmd == 'remove':
        for h in st['hooks']:
            patch(p, base + h['va'], bytes.fromhex(h['orig']))
            print(f"restored {h['va']:#x}")
        os.remove(STATE)
        return 0
    sys.exit(f'unknown command {cmd}')


if __name__ == '__main__':
    sys.exit(main())
