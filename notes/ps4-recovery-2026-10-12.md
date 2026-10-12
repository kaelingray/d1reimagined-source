# Exact 01.29 recovery checkpoint — 12 October 2026

The exact CUSA00219 01.29 executable remains missing. No native PS4 boot or
gameplay acceptance is claimed. The verified content recovery and contained
server checks from [11 October](ps4-transition-2026-10-11.md) still apply.

## Repack space outside file payloads

The original US repack was opened read-only through the separately installed
LGPL-3.0 [LibOrbisPkg reader](https://github.com/maxton/LibOrbisPkg). Its logical
inner PFS image is 52,812,709,888 bytes, with 2,120 contiguous, uncompressed
regular files. The new platform adapter exports the complement of their payload
extents; the Rust scanner checks bounds, reconstructs the complete plan and
requires the full known later executable as a positive control.

All **73,943,032 nonpayload bytes in 2,070 regions** were scanned. No ELF magic,
SELF magic, target/later build marker or `.evil` build suffix appeared there.
The 29,276,248-byte known later SELF passed its pinned SHA-256 and signature
controls. This covers logical PFS metadata/padding, not physical disk free
sectors or every embedded content payload, and does not decrypt the retail update.

`d1-byte-scan` passes **9 controls and 3 unit tests on Windows and Linux**.
Four actual CLI rejection checks also pass: insufficient export budget, omitted
region, altered positive-control bytes and existing output. Outputs are private,
created fresh, and cannot install or accept a game executable.

## Original second package recovered for inspection

A scoped read-only Recycle Bin metadata check found the original second package
from the user's owned `01.29` folder. It was inspected in place; nothing was
restored, moved or deleted. It is an **18,478,727,168-byte EU CUSA00568** base
package (APP_VER 01.00, VERSION 01.01), with 258 regular files. Its image key
does not open the US 01.29 update. No key or personal Recycle Bin path is recorded
in this repository.

Only its executable was extracted into private staging. The original SELF is
27,053,108 bytes, SHA-256
`a524cb42c8094d87a5bf408a608019ef694c01f292b57a41cb0b6acb08282785`.
The existing, unmodified GPL-3.0
[SocraticBliss/ps4_unfself](https://github.com/SocraticBliss/ps4_unfself), commit
`794ed5460ea6d82def0ed661d05d8171fa09ce1b`, reports all eight segments clear of
encryption/compression flags, as for the known later US control.

Conversion of disposable copies produces an EU ELF of 27,017,844 bytes, SHA-256
`6094c3d08d0e95acf89e1717ef0a0e2cd2aa2b5f7abbaf11cddcb808da8b632c`.
Its second LOAD metadata is offset `0x1714000`, address `0x1710000`, size
`0x129ff0`. The later control reproduces its known LOAD metadata and build
`56578.17.04.12.1251.evil`. Neither matches the exact target's 29,148,264-byte
ELF, hash `7271fcb926401df8defb126cb8eb2b247134138b81e401bbacb2ab60791b7954`,
or LOAD metadata (`0x18c4000`, `0x18c0000`, `0x149088`). No EU build date is
inferred from package metadata; no `.evil` build label was found in that input.
Converter completion is not a structural/native acceptance test.

## Evidence and next step

Private receipts are retained under `cfw-staging/2026-10-12/` in
`repack-slack-review`, `scanner-negative-controls`, `deleted-input-review` and
`self-conversion-review`. No extracted byte streams, keys, DLLs or converter
code are committed. The separately licensed tools remain outside the repository.

Continue with the exact 01.29 executable identity gate. These additional routes
rule out specific local recovery candidates; they do not establish that every
possible recovery route is exhausted. The game is still not launchable with the
accepted exact-build inputs currently available.
