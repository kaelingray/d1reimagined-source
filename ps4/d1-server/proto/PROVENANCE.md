# `proto/` - provenance

Two files live here. Neither is ours, and neither may be hand-edited.

## `shadnet.proto`

Vendored **verbatim** from the shadPS4 emulator, `src/shadnet/shadnet.proto`.

| | |
|---|---|
| sha256 | `61f4fef46c15b81c273ea0c29f5f7fb303542cf98d6fcbcbe7f0329594edac87` |
| upstream commit | `8bdec718b6512ac8d17a44efa0039c06c6810f6e` |
| upstream | <https://github.com/shadps4-emu/shadPS4> |
| licence | GPL-2.0-or-later (header: rpcsn Project 2019-2026, shadNET Project 2026) |
| messages | 71 |

⚠ The local working tree at `~/dev/shadPS4-d1trace` carries the **same file, byte for
byte**, at tree commit `1a51a41696a17a9a3b67ad6658dda547ddca4a98` - i.e. the schema has not moved
since it was vendored. That is checked, not assumed.

**The hash above is not load-bearing.** It is re-derived at run time, from the file itself, and
compared against the copy recorded in `ATTRIBUTION.md`:

```
cargo run -p shadnet-server -- controls
#   PASS  the compiled schema's sha256 is the one ATTRIBUTION.md records (re-derived, not transcribed)
```

`crates/shadnet-proto/build.rs` compiles this path with `prost`, and
`crates/shadnet-server/src/lib.rs` embeds the same path with `include_str!`, so the file the
controls hash is the file the types are generated from.

### Why vendor rather than reimplement

The schema is the authority. Generating from it means our encoding cannot silently drift from the
client's - and drift in a protobuf schema is not a compile error anywhere else. Coupling to the
**schema** is checkable with a hash; coupling to an implementation would not be.

## `shadps4-client-constants.h`

A **verbatim excerpt** of six enum blocks, so the Rust enums in `shadnet-wire` can be checked
against the client's own text instead of against numbers somebody typed in.

| source | sha256 |
|---|---|
| `src/shadnet/client.h` (`PacketType`, `CommandType`, `NotificationType`, `ErrorType`) | `9e2fcfcf401f0949bcfec7073e31296721a1840046b5e869f892d3dc5faa277f` |
| `src/core/libraries/np/np_types2.h` (`OrbisNpMatching2Event`, `OrbisNpMatching2EventCause`) | `31e801dff01898ceaf3fbc079c9229231f204ab459ad327cc602364e3d30580d` |

Taken from `~/dev/shadPS4-d1trace` at `1a51a41696a17a9a3b67ad6658dda547ddca4a98`.

`shadnet-server controls` re-derives both hashes **and** asserts each block still occurs verbatim in
the upstream file, whenever the shadPS4 tree is present (`SHADPS4_SRC`, default
`~/dev/shadPS4-d1trace`). When it is not present that check reports **SKIP** - never
PASS. A skipped check is not a passed check; that is how the last two instrument defects in this
project hid under a green board.
