#!/usr/bin/env python3
# SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
# SPDX-License-Identifier: GPL-2.0-or-later
"""Decompress one Oodle-compressed `.pkg` block using Destiny's own decoder.

⛔ SUPERSEDED 2026-08-11 by `d1-server/crates/d1-oracle/src/oodle.rs`, which calls the same three
eboot entry points through the Rust harness and needs no Python and no `unicorn` wheel:

    d1-oracle oodle controls                          # four controls, one INVERTED, exit 2
    d1-oracle oodle block --pkg <name> --block <n> -o blk.bin
    d1-oracle oodle entry --tag 0x80A64196 -o out.bin # spans blocks, follows the generation chain

⭐ The Rust reproduces this tool's own printed digest for the pinned block - 262,144 bytes, 16
chunks, plain sha1 `b7af28cbb6410f85ebf1b110e9a58af30594c1f3` - asserted in
`crates/d1-oracle/tests/oodle_probe.rs`.

⭐ ONE PORTING FINDING worth carrying: the Rust harness's `memset` guard is STRICTER than this
one's. `OodleLZDecoder_DecodeSome` zeroes a 0x20-byte frame local and returns to `0x1270A85`; Python
permits any memset inside the frame window, the Rust harness requires the return site to be
registered, and without it the decode faults on the first chunk in a way that reads like a client
defect. The address is `oodle::DECODE_SOME_MEMSET_RETURN` and a test proves it is load-bearing.

Kept as the historical implementation and the diff partner. Do not extend it.

Composes with the Rust container tool, which is where the `.pkg` format is described:

    d1-pkg extract-block <pkg> <index> -o blk.z      # stored bytes, SHA-1 checked
    oodle-decompress.py blk.z blk.bin                # this script
    d1-pkg verify-plain <pkg> <index> --plain blk.bin

`d1-oodle-harness.py` in this directory is the original exploratory harness and keeps its own
`main()`; this script is the pipeline-friendly wrapper around the same three entry points.

A block decompresses to **up to** 0x40000 bytes - usually exactly that, but not always, so the
success condition is consuming every stored byte rather than reaching a target size. Entries may run
past the end of a block and continue into the next one.
"""

import argparse
import hashlib
import importlib.util
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
BLOCK_PLAIN_SIZE = 0x40000


def load_harness():
    """Import the sibling harness as a module (its filename is not a valid identifier)."""
    spec = importlib.util.spec_from_file_location(
        "d1_oodle_harness", os.path.join(HERE, "d1-oodle-harness.py"))
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def decompress(stored, plain_size=BLOCK_PLAIN_SIZE, verbose=False):
    """Run the client's decoder over `stored`, returning up to `plain_size` bytes.

    Returns (plain, consumed, chunks). `consumed` should equal len(stored) for a whole block; a
    short read means the decode stopped early, which is a failure worth surfacing rather than
    quietly truncating.
    """
    m = load_harness()
    h = m.Harness()
    h.mu.mem_write(m.INPUT, stored)

    need = h.call(0x1267120, 0)                       # query scratch size
    scratch = h.alloc(need, 16)
    decoder = h.call(0x12671A0, 0, plain_size, scratch)  # init
    result = h.alloc(0x20, 16)

    dec = comp = chunks = 0
    # Terminate on **input** exhaustion, not on reaching plain_size. Blocks decompress to *up to*
    # 0x40000, not always exactly: ps4_globals_pvp_0169_3 block 561 consumes all 65479 stored bytes
    # at 114688 plain bytes. Looping on `dec < plain_size` calls DecodeSome once more with zero
    # input left and it reads unmapped memory - which looks like a decoder fault and is not one.
    while dec < plain_size and comp < len(stored):
        h.mu.mem_write(result, b"\0" * 0x20)
        ok = h.call(0x12672E0, decoder, m.OUTPUT, dec, plain_size, plain_size - dec,
                    m.INPUT + comp, len(stored) - comp, 0, 0, result)   # DecodeSome
        v = struct.unpack("<8I", h.mu.mem_read(result, 0x20))
        if not ok or v[0] == 0:
            break
        dec += v[0]
        comp += v[1]
        chunks += 1
        if verbose:
            print(f"  chunk {chunks}: +{v[0]} plain, +{v[1]} stored "
                  f"({dec}/{plain_size}, {comp}/{len(stored)})", file=sys.stderr)
    return bytes(h.mu.mem_read(m.OUTPUT, dec)), comp, chunks


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0],
                                 formatter_class=argparse.RawDescriptionHelpFormatter,
                                 epilog=__doc__)
    ap.add_argument("stored", help="compressed block, e.g. from `d1-pkg extract-block`")
    ap.add_argument("out", help="where to write the decompressed block")
    ap.add_argument("--plain-size", type=lambda x: int(x, 0), default=BLOCK_PLAIN_SIZE)
    ap.add_argument("-v", "--verbose", action="store_true", help="per-chunk progress")
    args = ap.parse_args()

    stored = open(args.stored, "rb").read()
    plain, consumed, chunks = decompress(stored, args.plain_size, args.verbose)
    with open(args.out, "wb") as f:
        f.write(plain)

    print(f"{len(plain)} bytes from {len(stored)} stored, {chunks} chunks, "
          f"consumed {consumed}/{len(stored)}")
    print(f"plain sha1 {hashlib.sha1(plain).hexdigest()}")
    # The success condition is consuming all the stored bytes, NOT reaching plain_size: a block
    # decompresses to *up to* 0x40000 and short ones are normal.
    if consumed != len(stored):
        print("WARNING: decode left %d stored bytes unconsumed - output is suspect"
              % (len(stored) - consumed), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
