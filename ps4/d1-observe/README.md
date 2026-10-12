# d1-observe

Replay-first protocol timeline debugger for the Destiny 1 preservation project.
It is read-only over run artifacts: it does not launch shadPS4, send packets,
change responder behavior or write guest memory.

The immutable event stream is authoritative. A single deterministic Rust
reducer feeds both the JSON CLI and native egui UI, preventing the human and
agent views from developing different causal models.

Deep implementation/evidence record: `../historicaldocs/design/conversation-debugger.md`.
Visual smoke capture: `docs/head-of-line-golden.png`.

## Build

```sh
cargo test
cargo build --release
```

The native GUI is a default feature. A CLI-only build is available for minimal
hosts:

```sh
cargo build --release --no-default-features
```

## Inputs

The preferred input is JSONL matching
`capture-rig/send-receive-event.schema.json`, extended with client-owned queue
events:

- `client_request_queued` with `details.expected_reply_type` when known;
- `client_reply_matched` or `client_request_retired`;
- `client_no_recipient`;
- `connection_timeout`;
- consumer, QueueZ, predicate, bootflow and fault events.

It also reads the JSON object emitted by the existing
`capture-rig/send-receive-ledger.py --json`. Passing a legacy run directory
invokes that adapter read-only when it can be discovered:

```sh
d1-observe validate runs/20260807-165310-operator-504
d1-observe --json timeline runs/20260807-165310-operator-504
```

Legacy server/emulator logs do not contain client queue insert/remove events.
The tool therefore displays their timeline but refuses to infer a queue head
from server receipt alone.

## Agent CLI

```sh
d1-observe --json validate SOURCE
d1-observe --json timeline SOURCE
d1-observe --json queue SOURCE --at EVENT_OR_INDEX
d1-observe --json explain SOURCE --at EVENT_OR_INDEX
d1-observe --json unanswered SOURCE
d1-observe --json why-stuck SOURCE
d1-observe --json first-divergence RUN_A RUN_B
d1-observe export-incident SOURCE --at EVENT_OR_INDEX
```

Human output is the default; `--json` is stable and deterministic.
`why-stuck --json` always returns an object with `proven`, `diagnosis`,
`reason`, `evidence_event` and warnings. Missing queue instrumentation is an
explicit `proven: false` result, not a bare `null` and not a guessed cause.

## Native replay GUI

```sh
d1-observe gui SOURCE
d1-observe gui RUN_A --compare RUN_B
```

The GUI provides a virtualized semantic timeline, in-flight queue at the
selected event, event details, boot/world/QueueZ state, causal diagnosis and
A/B first divergence. Stepping pauses only the replay view. It does not pause
the game, because doing so would change protocol timeout behavior.

## Golden controls

The committed fixtures protect the mistakes already paid for:

- pre-fix 0x000C queue-head timeout;
- the queue-drained positive-control run after a matching 0x000D reply;
- post-0x000D promotion of 0x0020, which then blocks a later 504 response;
- fragmented/coalesced TCP frames;
- QueueZ object insertion plus family refusal as `partial_failure`; and
- the selected-character `slot B` predicate, speculative manual write and
  following guest fault, with the value bounded to a resolved-object pointer
  but its concrete object type and legitimate writer retained as unknown.

## Privacy

Primary events carry types, sequences, lengths, safe identifiers and digests.
Do not store OAuth/JWT bodies, tokens, API keys or plaintext message 0x0015.
