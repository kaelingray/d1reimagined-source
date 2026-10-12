# Format description languages - the decision and the rule

**Status:** decision recorded; **no grammar written yet, deliberately.** Source: a web-session
proposal, adopted with corrections from local evidence.

## The rule (this is the part that matters)

> **Accumulation is measured per format, not per project. No single format may be described twice.**
>
> **Wire formats** live in Rust (`deku`/`binrw`), because the server must **emit** them.
> **File and content formats** may live in `.ksy` (Kaitai), because they are **read-only** and the
> visualizer is the better excavation instrument.
> A format that appears to need both descriptions is a design error - resolve which surface it
> belongs to; do not maintain both.

The rule is mechanical, which is the point: the next proposal (ImHex patterns, Construct, 010
templates, WSGD) is answerable without re-running the argument. *Which surface, and is that format
already described somewhere?*

## Why it's two tools and not one: the two surfaces have opposite requirements

The original framing - "pick one format description language" - was wrong, and that is why it looked
intractable. There are two surfaces:

| | **Surface 1: the wire** | **Surface 2: content & files** |
|---|---|---|
| What | D1's network protocol: handshake, control, session, replication | decrypted SELF/sprx, 26 sys_modules, eboot ELF, PKG, Tiger content packages |
| Must **write**? | **Yes** - a private server is mostly about producing responses | **No, permanently.** We never repack a PS4 PKG or a Tiger package |
| Verification | round-trip: `encode(decode(x)) == x` | does the layout explain the bytes |
| Loop shape | driven by observed buffers and refusals | hex-editor: unknown blob → conjecture → check → revise |
| Winner | **`deku`/`binrw`** | **Kaitai `.ksy`** |

Requirements test, applied in order - note that **tests 1 and 4 point in opposite directions**, which
is the whole reason the answer is two tools:

1. Can it **write** the format, in our language? → eliminates Kaitai for Surface 1
2. Can our shipping binary use the output without FFI? → eliminates Spicy for the Rust server
3. Is any single format described more than once? → eliminates duplicates
4. Does the excavation loop have a visualizer? → eliminates `binrw`/`deku` for Surface 2

## Kaitai: the honest reason it loses Surface 1

Not "serialization is experimental" - that's stale. **v0.11 (2025-09-07) shipped serialization for
Java and Python only** (`-w`/`--read-write`), and its "decent Rust support" is **parse-side**. There
is no Rust writer, and the serialization guide says other languages "will follow." Same conclusion,
accurate reason.

Second strike on the same axis: `type: bX` doesn't work in `instances` unless X is a multiple of 8 -
exactly when you'd want bit-sized fields (kaitai_struct#564). A bitpacked protocol hits this.

Where Kaitai is genuinely best-in-class, and why it wins Surface 2: the **Web IDE** and `ksv` show a
parse tree over multi-level-highlighted hex, and **v0.11 shows a partial object tree when parsing
fails** - the single most useful RE affordance available, because it tells you *how far you got*.
`binrw` gives you `Result::Err` and a debugger. Also `--opaque-types` (hand-implement the
encrypted/compressed sub-blobs console formats are full of), the `.dot` target (format diagrams as a
build artifact), and `elf.ksy` already in the format gallery - and our eboot **is** a plain
FreeBSD/Orbis ELF, so that's a real starting point.

## Spicy: not adopted, and not on quality

Spicy is well-built (Corelight-maintained, BSD, incremental stream parsing, error recovery/resync,
Zeek 5.0+ integration built in). It loses because **the value it uniquely delivers is D1-specific
analyzers running inside Zeek, which we are not doing.** What we want from Zeek - `dns.log`,
`ssl.log`, `x509.log`, `conn.log` - needs zero Spicy; those analyzers exist and are maintained.
Spicy emits C++, so using it in the Rust server means FFI across our most safety-critical boundary.

Two things lower the cost of declining: **D1's transport is ordinary UDP**, so Zeek's biggest
structural gift (TCP reassembly) mostly doesn't apply; and Rust pcap ingestion is well-trodden
(`pcap-file`/`pcap-parser`, `etherparse`).

Revisit only if we ever decide to write analyzers *inside* Zeek.

## Why we are not extending Zeek - recorded so it isn't re-argued

Zeek stays for standard protocols. D1-specific parsing goes in a small Rust `pcap → JSONL` tool that
**links the same crate the server uses.** The argument isn't tidiness:

> If the capture analyzer and the server use different parsers, a mismatch between them is a **silent
> bug in our understanding** - surfacing months later as mysterious behaviour, not as a parse error.

Sharing the parser makes the analysis tool a **continuous test of the server's model**: run it over
archived captures in CI, and a grammar change that breaks a known-good capture fails the build. That
is regression testing for protocol understanding, and it is unavailable with two grammars in two
languages.

## Kaitai's two deferred roles

**Differential oracle.** A `.ksy` → Python reader run against the same captures as the Rust parser,
with CI asserting agreement, gives differential verification of the protocol model. Structurally
identical to our two-client design: independent implementations, and **disagreement is information**.
This violates the no-twice rule *in letter*, so if adopted, write the exception explicitly rather
than letting it drift.

**Publication artifact.** Documentation outranks the reference server in our deliverable hierarchy. A
`.ksy` is a language-agnostic machine-checkable spec anyone can generate a reader from in 13
languages, plus `.dot` diagrams. **This is the answer to contributor gravity** - the Destiny RE scene
is C#-shaped, and a `.ksy` hands them a C# reader without our implementation language being chosen
by it.

**Trigger for both:** the first D1 message type stable enough that we'd be annoyed to have it wrong.

## `deku` vs `binrw` - decided later, on evidence

**Do not pick yet.** There is no D1-proprietary wire format to describe; the live gate is STUN, which
is RFC 5389 and natively dissected by Wireshark. Picking a parser for a format we haven't seen is how
we end up with the wrong one plus a sunk cost defending it.

- **Bitpacked, sub-byte widths** (likely, Halo/Tiger lineage) → **`deku`**: the only one with
  bit-level primitives, and its slices-only design is a *fit* for UDP datagrams rather than the flaw
  binrw's author considers it.
- **Byte-aligned framing with structural conditionality**, offsets, magic dispatch → **`binrw`**:
  better error forwarding, attribute expressions are real Rust rather than code-in-strings.

**Model ignorance as typed values, not one catch-all.** A single `Unknown` variant is too coarse; the
useful shape distinguishes *why* we don't know:

```rust
enum Decode<T> {
    Known(T),
    UnknownTag { tag: u32, raw: Vec<u8> },          // well-framed, unrecognised
    Unsupported { kind: u32, raw: Vec<u8> },        // recognised, not implemented
    NeedState { baseline: BaselineId, raw_bits: BitVec }, // undecodable without prior state
}
```

**Reserve parse *failure* for malformed framing**, or for an unknown field whose boundary we cannot
trust. Preserving raw bytes for well-framed unknowns is proven practice - protobuf does exactly this
with unknown fields, round-tripping them through parse/reserialise.

**Recorded now because it will be argued later:** game replication formats frequently are **not
statically describable at all** - delta compression and state-dependent quantization mean field
widths a declarative grammar cannot see. Expect the declarative tool to carry handshake, control and
framing, and expect hand-written typed readers for the replication core. **A hand-written reader is
not a second description language, so that is not the accumulation this rule prohibits.**

## Prior art: what actually exists to reuse

Checked, because "Kaitai plugs into everything" partly doesn't survive inspection.

- **Wireshark - weak.** "Dissectors as a target language" has been an open umbrella issue since 2016
  and is still not a KSC target. `kaitai-to-wireshark` exists but its own README calls the output a
  prototype for further customization. **Don't plan on a free dissector from a `.ksy`;** hand-write
  Lua for the fields you care about, or use WSGD.
- **Ghidra - doesn't exist.** No maintained `.ksy` → Ghidra data-type path. Don't budget for it.
- **PS4 formats - exists, wrong language.** The scene's lingua franca is **010 Editor templates**
  (`.bt`), not `.ksy`, and there's no automated conversion. Adopting that knowledge means
  transcribing it - worth it only for programmatic access, not a one-time look.
- **ELF - directly usable.** `executable/elf.ksy` is in the gallery; our eboot is a plain ELF.
- **Destiny/Tiger tooling - C#, hand-written.** Charm and that scene don't consume `.ksy`.

## Halo Reach as a reference - useful, with a caveat

Reach is worth having, and Bungie's Tiger engine descends from Blam. But be precise about what
transfers:

- **Transfers: structural idioms.** Hash-keyed content identifiers, the tag/content-package model,
  and the general shape of a Bungie content pipeline. That's Surface 2 material, and it's real.
- **Does not transfer: bytes.** Reach is Xbox 360 - **PowerPC, big-endian**, a different console
  generation and a different engine revision. No format is byte-compatible, and any endianness
  intuition carried across will be wrong.
- Reach's RE community (Assembly and friends) is mature, so the *methods* and the *vocabulary* are
  reusable even where the layouts aren't.

So: excellent for Surface 2 idioms and for calibrating what Bungie tends to do. **Not** evidence about
D1's wire protocol, which is Demonware's anyway - see `historicaldocs/design/DEMONWARE-FINDING.md`.

## Anchoring: we have strings, so the pessimistic branch doesn't apply

A parallel session worked through what to do if the eboot were stripped - no magic bytes, no
`contents:` assertions, round-trip identity as the only correctness signal, hash-preimage recovery
against Bungie's public manifest. Good reasoning, and **the premise is false here**: the 01.29 eboot
has **83,621 strings** and all 18 STUN hostnames as literals. Static anchoring is available and cheap.

Two ideas from that branch are worth keeping anyway, on their own merits: the **NID import table is
unstrippable**, so `sceNetSendto`/`sceNetResolverStartNtoa` call sites are always an anchor; and
**hash-preimage recovery against the public D1 manifest** (FNV-1a-32 and variants, case-normalized) is
a cheap asymmetric bet for Surface 2, where content identifiers really are hash-keyed. Neither is
needed to get started.

## Build order

1. **Now:** this rule is recorded. Cost: one page. Value: the next tool proposal is answerable.
2. **On the first D1 message shape:** pick `deku` or `binrw` per the test above; parser lives in the
   server crate.
3. **Immediately after:** the `pcap → JSONL` tool linking that crate, plus the archived-capture
   regression test in CI. This is what makes the single-parser decision pay.
4. **On the first content-format need** (likely Layer B, extracting activity/mission definitions):
   one-time look → Web IDE/`ksv`, no committed grammar. Programmatic → `.ksy` in the repo.
5. **If the docs deliverable becomes real:** `.ksy` as publication artifact + `.dot` diagrams;
   consider the differential oracle then, since the grammar already exists.
