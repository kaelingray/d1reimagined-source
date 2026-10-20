"""SYNTHETIC diagnosis patch: make 0x907930 start the game engine without game-state authority.

0x90797B `je 0x90799a` (skip when 0xBB89F0 says we are not the authority) -> two NOPs.
    engstart.py apply | revert | show
"""
import sys
src = open(r'F:\d1\tools\d1-winguest.py', encoding='utf-8').read().split('def main():')[0]
g = {}
exec(compile(src, 'wg', 'exec'), g)
p, base = g['open_client']()
SITE, ORIG, NEW = 0x90797B, bytes.fromhex('741d'), bytes.fromhex('9090')
cur = p.read(base + SITE, 2)
print(f'{SITE:#x}: {cur.hex()}')
cmd = sys.argv[1] if len(sys.argv) > 1 else 'show'
if cmd == 'apply' and cur == ORIG:
    g['patch_code'](p, base + SITE, NEW)
elif cmd == 'revert' and cur == NEW:
    g['patch_code'](p, base + SITE, ORIG)
elif cmd != 'show' and cur not in (ORIG, NEW):
    sys.exit('unexpected bytes; nothing written')
print(f'{SITE:#x}: now {p.read(base + SITE, 2).hex()}')
