# d1-trace - unified four-stream trace viewer

Open a `runs/<stamp>/` directory and see its four evidence streams - HTTP, BAP frames, UD2 probe
records, guest diagnostics - on **one time-ordered, cross-correlated, searchable timeline**.
Read-only: it runs nothing and writes nothing into any client. Design: `historicaldocs/design/trace-viewer.md`.

## Build

```sh
cd ~/dev/destiny1-preservation/d1-trace
cargo build --release            # GUI (egui/eframe =0.36.1, glow)
cargo build --release --no-default-features   # headless CLI only
cargo test --release             # 92 tests (83 unit + 9 end-to-end)
target/release/d1-trace controls # 52 pinned invariants - EXIT 2 = do not cite the output
```

Standalone crate, **outside** the `d1-server` workspace on purpose (feature-unification trap; mirrors
`d1-observe`).

## Use

```sh
# GUI
target/release/d1-trace <run-dir>
target/release/d1-trace gui <run-dir> --query 'probe:item_load_state_primary rax&0xffffffff==2' --matches-only

# Headless (no display) - verify ingest / run a query in a script
target/release/d1-trace summary <run-dir>          # counts, anchor rate, probe names, notes
target/release/d1-trace search  <run-dir> 'world_tag'
target/release/d1-trace decode  <trace.bin> [--json]  # decode a raw account-trace.bin (native)
target/release/d1-trace blob    <trace.bin.blob>      # world_launch commands (native); also
                                                      #   --dump N | --diff A B | --extract N --out F
target/release/d1-trace controls                  # ⭐ run before citing anything
```

⭐ **`d1-trace controls` is the house pattern (`d1-eboot controls`).** Exit 2 = a pinned invariant
broke and the output may not be cited. It exists because anchoring is scraped from the probe
`explain` prose, and a cosmetic reword of those labels was measured to take 1061 anchors → 0
silently, exit 0, with every unit test green (`historicaldocs/design/trace-viewer.md` §7 F4).
The prose is now produced by this crate's own `probe_decode` module from the same constants the
scraper reads, so producer and consumer cannot drift; the controls stay as belt and braces.

Probe records are decoded **natively** (`probe_decode.rs`, the byte-faithful port of
`capture-rig/decode-account-trace.py`; `d1-trace decode` reproduces that tool's output and
refusals exactly - verified by golden diff over 193 real traces plus 18 refusals). The `D1BL`
blob sidecar is also decoded **natively** (`blob_decode.rs`, the byte-faithful port of
`capture-rig/world-launch-command.py`, same goldens-and-refusals discipline), so the ingest path
spawns **no Python at all**; only the live `d1-dash` lane still runs capture-rig scripts.

## Search grammar (space-separated terms, ANDed)

| term | meaning |
|---|---|
| `stream:<http\|bap\|probe\|guest>` | restrict to one lane |
| `probe:<substr>` | probe events whose name contains `<substr>` |
| `<field>[&<mask>]<op><value>` | register/field predicate - **probe events only** |
| bare word | case-insensitive substring over the summary **and detail**, any lane |

Fields: `rax`…`r15`, `rip`, `rflags`, `guest_vaddr`, `seq`, `probe_id`.
Ops: `== != <= >= < >`. Values/masks: `0x…` or decimal.

Flagship: `probe:item_load_state_primary rax&0xffffffff==2` - "did it ever return 2?" - then the
selected event's **nearby events (all streams)** panel shows the `item_tag_resolved` on that
iteration and the surrounding BAP/HTTP/guest context.

Warnings are loud, not silent: a `probe:` name that matches nothing in this run, a single `=`
(`rax=2`), and an unsatisfiable mask/value pair each print a `warning:` line naming the *query* as
the cause.

## Ordering - no shared wall clock, and no invented one

`responder.log` and `emulator.log` carry no timestamps (measured). The unified `order` is the guest
probe `sequence` (the spine); BAP frames anchor onto it by matching `(response type, seq)` against
the guest's own rx probes (`=` exact); a responder row **strictly between two anchors** inherits that
bracket (`~`); everything else - guest diagnostics, unanchored frames, and anything *outside* the
outermost anchor - is `≈`. Every row shows its confidence marker.

⚠ **A run with no probe spine gets no unified timeline at all.** 133 of 200 run dirs on this box are
in that state; the tool groups by lane, orders within lane, marks every row `≈`, and says
`⚠ NOT OBSERVED`. It does not invent a cross-stream order. See `historicaldocs/design/trace-viewer.md` §1 and §7.
