# LESSONS

Everything here was paid for once. The pattern across nearly all of it: the failure was rarely the
game's. It was a wrong baseline, an unverified assumption, or reading silence as information.

Add to this whenever something costs more than a few minutes. Prefer adding a guard (a refusal, a
default, a control) over adding prose.

## Controls

A control proves the instrument. It never proves the technique, the search space, or the question.
Most of this file is that sentence, learned the hard way.

- Every "I searched and found nothing" needs a search that finds something, in the same run. If the
  control comes back empty too, the scanner is broken and the result is unciteable. A vtable scan
  scored 0 references and its known-used control also scored 0; that is the only reason it was caught.
- State the axis the control protects, in words, before writing it. "This proves the tool finds edges"
  is not a control; "this proves it finds edges materialised by lea" is. A callgraph shipped 7/7
  passing and later scored 4/11 because the original seven were all direct calls. Coverage counts
  mislead.
- Invert every control. A control that only ever passes is decoration. Two ways a control passes while
  testing nothing: the branch no input reaches (fix by manufacturing the input), and the code the
  subject never executes (fix only by choosing a different subject; a canary claim about an epilogue
  was verified against a function that faults 19 instructions before its epilogue).
- A one-shot control cannot see a per-call leak. 62 of 70 controls stayed green through a leak that
  killed activities after 131 encodes, because every control encoded each message exactly once.
  "Produces the right bytes" and "can keep producing them" are different axes.
- Reproduce the observed number, not the observed shape. "It leaks" is satisfied by a much weaker fix;
  "dies on exactly the 132nd encode" cannot pass by accident.
- A control in an adjacent window is a control for a different experiment. Five watch windows were
  burned on a quiescent subsystem; the control fired in one window and not the next, so every NO-HIT
  was uninterpretable. Generate the traffic and put the control in the same window. Same rule one
  level down: three stable reads is a control against a race, not against a wrong address, because a
  wrong address is perfectly stable.
- A control must isolate the variable you suspect. "Structs vs no structs" would have confirmed a
  false story; "signature vs no signature, structs held constant" separated them.
- Ask memory whether it happened; never ask the planner whether it planned. A field named for an
  intention gets rendered as an accomplishment by the first person who formats it. Compute derived
  predicates from a read-back.
- A sampled verifier must be shown to cover the bytes you cite it for. "835,584 sampled bytes match"
  was true about bytes that structurally excluded the guarded address, on every snapshot, forever.
- A new tool is not evidence until it reproduces a known result. A report read probe_id at the wrong
  struct index and printed NEVER FIRED for three runs, indistinguishable from the failure it was built
  to detect.
- The general test for any instrument: if this tool were broken in its most likely way, what would its
  output look like? If the answer is "exactly like a finding", it needs a control that separates the
  two before anyone quotes it.

- A truncated handoff is indistinguishable from a shorter decision, and the truncation is usually in
  our own orchestration rather than in the work. Twice in one day a workflow passed a result to the
  next agent through a fixed-size slice: a 4000 character cut dropped 17 of 31 manifest rows, and a
  60000 character cut dropped four of six classifier buckets. Neither receiving agent could see that
  anything was missing, because a list that ends is exactly what a shorter list looks like. The
  second one was caught, and only because the receiver had been told to check that each contributor
  reported its positive control firing: four buckets reported nothing at all, the rule said treat
  that as controls-not-fired, and the judge re-derived those 55 files from the tree instead of
  trusting a silence. Two guards, both cheap: never hand a manifest to another agent through a fixed
  slice, and make every multi-contributor stage state its expected contributor count so a short
  arrival is loud. Note what the control actually protected here - not the subject, but the harness
  carrying it.

- In a parallel lane, another agent's fresh work reads exactly like history. Two agents in one
  workflow were asked whether a shipped binary shells out to Python. The first confirmed it and
  wrote the Rust port. The second, running later, found that port on disk, concluded "this was
  already ported, the premise is FALSE", and reported the brief as wrong. Both were describing the
  same file minutes apart; `git log` on it returns nothing, because it was created that hour. The
  general shape is the one this file keeps relearning: current state is not provenance. When a lane
  reports that something "already exists" and that finding would change a plan, date it - `git log`
  the file, or check whether a sibling could have produced it - before believing the tree was always
  that way. Cheap guard: tell every parallel lane what its siblings are creating, not only which
  files they own.

- Two individually-correct cleanup decisions can delete a capability between them. An audit archived
  two fixture generators BECAUSE their output was committed. Hours later, an untracking pass dropped
  that output BECAUSE a generator existed. Each step cited the other as its safety net, and the
  result was a fresh clone with neither the fixture nor the tool that makes it. Nothing in either
  decision was wrong on its own, and no reviewer looking at one would have objected. The guard is to
  state the invariant rather than the action: for every derived artifact, EITHER the output or a
  working generator must reach a fresh clone, and whichever you remove, check the other is still
  there in the same change. It was caught only by re-reading the bootstrap doc as a stranger would
  follow it, top to bottom, after the edits - which is worth doing after any cleanup pass, because
  the break is invisible from inside either decision.

- House style must stop at the boundary of someone else's bytes. A scrub pass flattened em dashes
  across "docs and shipping code" and caught one character inside `proto/shadnet.proto`, a file
  ATTRIBUTION.md declares vendored VERBATIM from another project with a pinned sha256 and an
  upstream commit. One comment character made the attribution false, and made a claim of verbatim
  vendoring into a claim we could not support. A control existed for exactly this and had been
  failing for a day, dismissed in two separate reports as a pre-existing failure that belonged to
  someone else. Two rules: any formatting sweep must exclude vendored and third-party paths by
  construction rather than by reviewer attention, and a failing control is never "pre-existing" -
  it is either fixed or it is a finding with an owner. The recovery was one command, because the
  verbatim bytes were still in git history.

## Silence and negative results

- A negative result is only as good as the premise that chose its search space. A clean "0 of 101"
  blocked a deliverable for days; the table searched was the wrong protocol layer. Write down what you
  are assuming about where the answer lives, not only what it looks like.
- An instrument that fails like a real negative silently deletes half your search space. Replay
  faulted at step 8 on about 45% of the binary (a stack-canary guard pointer into host TLS) and
  reported it as "this function can't be replayed". Recognition rule: look at the shape of the fault
  address. Host-shaped (0x55...) means the harness cannot run it; guest-shaped (0x2.../0x3...) means we
  did not capture that region, which is actionable.
- A NO-HIT taken while the subsystem is idle is not a measurement.
- A negative from a poisoned connection is not a result. BAP head-of-line blocks: one unanswered
  request times out and blocks every later reply. Four "refutations" were taken in that state and one
  of them was the actual fix. Before attributing a failure to a payload, check no earlier request is
  outstanding. Scoped instance: once OLIVE (error.investment_peer_manager) has fired it accumulates
  across attempts, so a later attempt failing earlier is the same poison, deeper, not a regression in
  what you just changed. Reboot, and arm everything before the boot.
- A wrong refutation is more expensive than an open question, because an open question keeps getting
  looked at. When recording something as refuted, record the conditions it was refuted under.
- One sock.read() is not one frame. TCP preserves byte order, not message boundaries; the responder
  once treated each read as one BAP frame, so the first coalesced or fragmented read failed the GCM
  open - and because the receive nonce advances only on success, every later frame then failed
  permanently. [R] The defect surfaces only when traffic gets busy, so it reads exactly like the
  message in flight at that moment being malformed. Guards: a persistent accumulator that splits
  exactly 6 + declared_length bytes per frame, one nonce advance per authenticated complete frame,
  and logging that distinguishes socket reads from extracted frames. Any semantic result taken after
  a framing loss is transport-confounded and must not be reused.
  (historicaldocs/ingress/round3/process/BAP-TCP-FRAMING-INCIDENT.md)
- Silence has several causes. A reply 2 bytes short parks the client in a resumable header read with
  no timeout, no log and no error branch, which is identical to being ignored. Ask "am I stalling the
  parser?" as readily as "was I rejected?"
- "No error" is not "healthy". A run reported as stable was sitting on an error dialog with a log that
  had stopped growing. Check the log is still growing, and check the crash folder.
- A missing diagnostic is not evidence the branch was not taken. Three variants, all of which look
  like "the game isn't getting there": the filter is too tight so the site is never called; the sink
  never drains (memcpy into a ring buffer, no output call); or there is no sink call at all, the site
  formats into a dead stack buffer that is never read. The third is undetectable from the log side.
  Before treating a missing diagnostic as evidence, name the function that would have output it.
- [U] with a mechanism is a finding. [U] with a list of guesses is an unfinished search.

## Ask the right question, of the right subject

- State the access mode before choosing the tool. A clean scan with passing controls still answers the
  wrong question if its matching rule cannot represent the target. Who references an address:
  `d1-eboot xref`. Who touches `class + N` register-relative, including disp 0 and address escapes:
  `d1-eboot field-xref`. Which instruction wrote this at this instant: `d1-live watch`. An indexed
  element of a heap pool (`*(base+8) + i*stride + N`): neither static tool; go through the named
  accessor or watch at runtime. `mov rax,[rdi+8]` contains no address, so an absolute xref can never
  match it at any confidence. Before running a scan, say what the target instruction looks like in
  bytes and check the matching rule could match those bytes.
- A correct measurement of the wrong subject is indistinguishable from a correct answer. An offline
  replay of f(393) was right in every number; the live client evaluates f(0). Replay the input the
  live client actually uses.
- Loaded is not sent. The responder logs a fixture's exact byte count when it READS the file, and
  that line is not evidence the client received anything. Loading, staging, baseline-building and
  transmitting are four events; only the send line proves the last one. Before interpreting any
  on-screen negative, find the send line for the object you are asking about.
  ⚠ The original example here ("families 3 and 4 had never been transmitted at all") was itself
  WRONG, and the way it was wrong is the very next lesson: the push path logs its sends as
  `--> BAP QUEUEZ CHARACTER`, the census grepped only the ANSWER-path label `--> BAP FAMILY n`,
  and a scanner that cannot see a label concludes the event never happened. The push had fired in
  186 of 270 runs. The principle stands; the cited evidence did not. A "was it sent" census must
  enumerate every send-path label first, and prove it can see a known send of each kind in the same
  window.
- Delivered is not consumed, and "consumed" is one word for three events: codec decode, object
  insertion into the registry, and family replication are separate verdicts with separate
  diagnostics. [R] A generic consumer-failed verdict collapsed them while the class-4 object had in
  fact reached the registry and cleared its investment predicate; only the family update was being
  refused. Name the stage that failed, never the pipeline.
  (historicaldocs/ingress/round3/process/WORKLOG.md)
- Order samples by sequence before any state claim, and guard an object-presence read with a
  same-read liveness check. [R] Three research-lane retractions had one shape: "the record was
  cleared after write" came from polls compared without sequence order; "the family-3 push did not
  land" came from a post-teardown read; "family-4 is refused at state 1" was a race that a guarded
  later read showed at state 3 with the object populated. An absence read after teardown is not an
  absence, and a refusal read once is not a wall.
  (historicaldocs/ingress/round3/process/RETRACTIONS-AND-ELIMINATIONS.md)
- Registered is not invoked. A function in a dispatch table, vtable, or binding registry has a
  mechanism, not a caller. Finding the setter for the activity lifetime phase inside the BlamScript
  binding table produced a published conclusion that the field was script-driven and therefore
  cloud-only and unservable; one `xref` on the dispatcher showed a single call site, in the handler
  for an Activity Host message we can already send. Table membership answers "how would this be
  called"; only an xref on the dispatcher answers "who calls it". This is built-but-unwired read
  backwards, and it is the more dangerous direction because the wrong answer is a surrender.
- An xref on a pool or struct base is blind to interior-offset access, and the blindness reads as
  "nothing uses this". The "runtime function bindings" pool looked write-only under `xref 0x25c0c40`
  and yielded a confident "the phase writer is dead code"; the reader never names the base at all,
  it reads `base+0x18/+0x2c/+0x30/+0x44` as four separate rip-relative absolutes. Before believing a
  base-address negative, ask whether a consumer would name the base or an interior field.
- `d1-eboot xref`'s `fn` column attributed a frame-pointer-omitted function to the preceding
  prologue, so a non-prologue helper was rendered as part of its neighbour. This made the binding
  lookup at `0x1d2030` print as `fn 0x1d1e80`, the registrar, turning pool reads into apparent pool
  writes. **FIXED 2026-08-11** - attribution now goes through the same two-pass function index the
  atlas uses (`functions::Index::containing`), and an address in no function prints `fn -` rather
  than a neighbour's name. The generalisable half stands: **the tree already had the right answer
  in a different module.** `atlas --addr` had been FPO-correct the whole time and `field-xref` even
  carried a docstring naming this exact defect in `xref`; the fix was wiring, not new logic. Before
  writing an analysis, check whether a sibling subcommand already answers it correctly.
- Three variants of one lesson landed in a single session, in both directions: registered-is-not-
  invoked said "unreachable" about a live path, and base-address-xref said "unused" about a live
  pool. Both were negatives produced by a matching rule that could not represent the target. When a
  scan returns "nothing", the first question is never "what does this prove" but "could this scan
  have seen it".
- A clean result to the wrong question is the most deceptive failure here. Three independent scans
  hunted one field, all with passing controls, all blind differently: name-matching across reused
  local names; computed displacements (`element + 0x18 + i*4`, where the text 0x1c never appears); and
  one opcode/register encoding out of many. A byte-pattern field scan must enumerate opcode by
  register pair by displacement width, or use field-xref instead of hand-rolling it. When a descriptor
  says a field is an inline array, search the array base, never the element offset.
- When a field holds a pointer, re-ask the question one indirection down. "Zero stores to +8" is the
  correct answer when +8 is a heap handle written once by an allocator, and useless. The escape rows
  are the finding: they name the allocator and the destructor and redirect the search to the pointee.
- A watchpoint traps on the access, not on the value becoming interesting. "It has never been non-zero
  so any hit is the answer" is two claims in one sentence; the only writer was a memcpy of zeros out
  of a block that had just been memset. memcpy/memset of a containing block fires it constantly. If
  you need "when does this become non-zero", you need the hit plus a read of the value at the hit.
- Before arming a vtable slot, establish what the slot is. c_game_system_interface has six virtuals in
  a 3-up/3-down lifecycle, so slot 4 is the destructor and two live NO-HITs read as "the system is
  never ticked" were structurally guaranteed. There is no per-frame hook anywhere in that set.
- Depth of analysis is not evidence of relevance. A rigorous diagnosis of the wrong object was harder
  to question precisely because it was deep. Spend thirty seconds on "what is this object called, and
  who reads it?" before the next hour of tracing.
- Co-occurring symptoms share a trigger, not a cause. Four item defects appeared together and were
  written up under one cause; there were four independent defects in three objects. Before attributing
  symptom S to cause C, name the instruction that reads C on the path that produces S.
- Two instruments disagreeing is a finding, not noise. Two agents returned opposite conclusions about
  one field and both cited real instructions: two partial scans, not a contradiction.

## A rendering is not a value

- A log line is a rendering at one point in a pipeline. A resolver printed 2.0.77.10:780 for our
  10.77.0.2:3075; the printf renders pre-swap bytes and the connect block swaps and uses them
  correctly. "Fixing" the printer broke the socket call. Ask which consumer the printf stands next to;
  here the one that mattered printed its own argument four lines further down the same log.
- "Measurement beats inference" is only true if you measured the thing that matters. That principle
  was invoked by name while measuring a debug string.
- An "N sites agree" claim is refuted by finding one that disagrees, not by finding fewer than N.
- A server-side receipt proves you built it, never that the client kept it. Read the consumer's
  memory, not your own log.
- A black screen is not a state report. The run that "went backwards" was two rungs further than ever
  with zero errors; the renderer was failing every frame while the game thread ran perfectly. Read
  state transitions, then the error census, then the live object registry.
- Never read a multi-byte integer by eye. A SOID transcribed by looking at it produced a false lead
  days later. Parse with struct.unpack or `--as u64`. SOIDs here are maximally easy and maximally
  consequential to confuse.
- A trace field can record the request, not the act. shadPS4's network trace keeps the full
  requested buffer in payload_hex even when the send was partial; only the result-count prefix
  reached the wire. [R] Decode to the result bound before citing bytes as sent.
  (historicaldocs/ingress/round3/process/WORKLOG.md)
- Compare against the right baseline. A complaint that "disappeared" was reported as progress twice
  when the predecessor run had never produced it either. Diff the exact predecessor.

## Search the tree before deriving anything

This is the oldest lesson here and the most expensive to re-earn. Three of the five gates that reached
orbit were already solved in this repo; FNV-1 was re-derived by trial after being confirmed in two
docs for two days; four capabilities were built and switched off, including the entire Activity Host.

- Before concluding we lack a capability, grep for it. An unwired encoder produces no traffic at all,
  so it is invisible from the wire.
- A doc that predicts your failure by name is not background reading. Read the retraction and "what is
  not true" sections of neighbouring docs first; they exist because someone already paid.
- Check the disk before synthesising. We began fabricating a content manifest while the package ids
  and GUIDs sat in the package headers. Observation is usually cheaper and carries better provenance.
- A partial read looks complete. A positive control on the instrument is not a control on coverage.

## Guards, not prose

- A lesson without a guard is a lesson you pay for twice. The missing-D1_MANIFEST_FILE failure was
  written up, then cost another run, because only the prose existed. When you write a lesson, ask what
  would have prevented it and put that in the code.
- A checked-in artifact that describes runs/ decays. The diagnostic catalogue's SEEN column was built
  from 146 run logs while the tree held 248; regenerating flipped 148 rows from NOT-SEEN to SEEN and
  every one was a citable false negative. There is no moment at which such a file becomes wrong, it
  just stops being a measurement. Stamp it with a provenance line and ship a --check-stale.
- Make "cannot tell" the worst exit code, not a pass. An unanswerable freshness question is exactly
  how this project reads silence as a negative result.
- A tool that regenerates its own input will eat its own output. A struct generator rescanned a corpus
  it had just retyped, found zero offsets for every class, and proposed deleting them all. Any tool
  that writes into its own corpus needs a monotonicity or shrink check.
- A control that only ever runs against the same input set is barely a control. Widening the run
  corpus exposed a regex that had been silently discarding twelve subsystems since the tool was built.

## Serving discipline

- Answering wrongly is worse than not answering, but it is not universal. Measured one variable at a
  time: silence is correct for one-way messages and for responses we cannot author, and wrong for a
  request that genuinely wants content. Do not generalise either way without measuring.
- An unsolicited push is not free. A QueueZ push defaulted to ON and killed the client five messages
  in. It was the timing, not the payload: before the client subscribes, an unasked-for object is fatal.
- A re-publish path that rebuilds from the client's echo silently drops every field that is ours.
  Three fixes in a row worked (their fields are re-derived from self on both paths) while the fourth
  silently did not, and both paths printed the same server receipt. Enumerate every field that is
  yours to assert and re-apply all of them; do not spot-check, because the fields that survive are
  exactly the ones you will happen to test.
- A substitution looks like an augmentation. Serving family 0 alongside family 2 made the nameplate
  worse, because the widget chooses the rich fill over the compact one. Before adding a second data
  source, ask whether the consumer merges or chooses. A chooser silently discards the source you
  already got right.
- Passing a gate's non-null test is not satisfying it. A plausible-but-wrong value opened a gate and
  crashed three frames later in a consumer that dereferenced it. That is still a useful
  identification; report the advance and the crash as one result.
- Poke the input to a resolution, never its output. The safe hops write values the client writes
  itself on its own exit path, so every downstream consumer already handles them. Forcing the output
  also deletes the evidence: the force-account-ready patch made the cached ready byte true and
  thereby suppressed the network request that would have named the real dependency, regressing the
  connection. [R] A cached predicate poked true stops asking.
  (historicaldocs/ingress/round3/process/RETRACTIONS-AND-BOUNDARIES.md)

## Probes and watchpoints

- ⛔ **`d1guest read` takes a RAW eboot vaddr (`0x26B5730`), the same convention as `poke` - never
  pre-add the 0x800000000 base.** A pre-based address is silently re-based onto an empty page and
  reads as ZEROS - a clean, confident, wrong answer. This burned 2026-08-11 for over an hour: the
  rung-34 done mask was read as 0 across three runs while `poke`'s before-value (raw vaddr, correct
  convention) showed `0x1FF` in the same seconds - and the discrepancy WAS visible and got explained
  away as "it filled between my read and my poke". Two whole failure theories (a cine-object timing
  window, a dead session) were built on those zero reads and are retracted. The tells, in order:
  a read that contradicts a poke's before-value is an ADDRESSING bug, not a race; an all-zeros read
  of something that must be nonzero is the wrong page, not state ("a garbage-looking live read IS
  garbage"); calibrate any raw read against a value you just wrote before trusting it. The fix that
  ends the class: the d1guest Rust port must take ONE address form and refuse ambiguous input.
- A multi-word CODE patch on a live client must be freeze-write-thaw: `kill -STOP` the emulator,
  write every byte, `kill -CONT`. Poking a 12-byte hook as three u32 pokes seconds apart leaves the
  function TORN between invocations - half new instruction spliced into the old prologue - and any
  guest thread that calls it in that window executes garbage (run `20260811-121021-GEARED-RADV`
  died at `Read from address 0x0` after write 1 of 3 landed on `FUN_0010C500`, a getter the rung-34
  task ladder polls every tick). The same recipe succeeding in an earlier session was luck, not
  safety: the hotter the function, the wider the window. Data pokes (single words) are exempt;
  guest-side timers freeze with the process, so a few stopped seconds are safe.
- You cannot positive-control a data watchpoint with a poke. A hardware debug register traps stores
  executed by the target's own CPU context; d1guest poke writes from outside the process and can never
  trip DR0-DR3. The instinctive control is structurally incapable of testing the axis: it looks like
  diligence and delivers nothing. A valid control is an address the guest demonstrably writes on its
  own. Re-audit any uncontrolled --rw write NO-HIT. Exec watches are unaffected.
- Prefer a breakpoint that enumerates over one that tests a guess. Arming a jump table's `jmp rax`
  makes the resolved case the payload: one window returns the whole distribution, the spread of cases
  is the positive control, and the absent case is the finding. Jump-table entries are offsets relative
  to the lea base, and check the absent case's target is unique; if shared, the NO-HIT is a tail-merge
  artifact.
- One-shot probes catch the first occurrence, which is usually not the interesting one. Make probes
  re-arm each other, never themselves, and bound them with a record budget.
- The re-arm pump must sit on a path that runs unconditionally, never on the call you are measuring. A
  six-probe chain anchored on the producer under investigation produced NEVER FIRED on every link,
  identical to "the chain is broken at the first link".
- Assert the armed count and the probe-ID parity in the build, never by eye. [R] A str.replace
  patcher whose pattern matched nothing no-opped silently, and the empty trace printed as "substate
  never entered"; a probe catalogue whose IDs drifted from the tracer's (1-11 vs 31-41) would decode
  real hits as the wrong events. Both are build-time parity failures; compare eboot bytes, addresses
  and IDs against the tracer before interpreting any trace.
  (historicaldocs/ingress/round3/process/RETRACTIONS-AND-ELIMINATIONS.md)
- Do not probe a hot, multi-threaded engine leaf. Probe the specific caller on the game tick instead.
- A tracer that dies armed kills its target. Never SIGKILL d1-live watch; Ctrl-C, SIGTERM and
  --max-seconds all disarm cleanly.
- Ask what property you actually need, not what is easy to read back. "The register is clear" is not
  "the target will survive detaching"; a queued signal killed the target one run in three. Never
  swallow the error of a step whose failure is destructive.

## Derivations and arithmetic

- "It came out pretty" is the tell, not the proof. Five hashes produced a consecutive run that "could
  not be coincidence"; there were eight equally self-consistent answers and two of them were
  consecutive. When a derivation has one free parameter, enumerate the parameter before claiming the
  output is forced.
- Name the free parameter out loud. Unstated means untested.
- A refutation can be as underdetermined as the claim it kills.
- Ship the weak true statement, not the strong pretty one.
- When a claim is about a whole table, sample the whole table. "Verified 4-for-4" described the first
  112 of 7,544 indices; running the client's own accessor over all of them found 633 discontinuities.
- Strictness is not accuracy. Only a holdout measures accuracy. A propagation rule that looked like
  +24,805 names measured 26% correct; the technique with the biggest yield was the least accurate,
  which is the normal relationship. A naming technique ships with a cross-validation or it does not
  ship.
- When you infer a category from one instance, dump the category. Listing all 3,161 members took
  thirty seconds and refuted the conclusion instantly.
- Statistical claims need the null hypothesis. "207 distinct byte values" carried zero information;
  for 432 uniform random bytes the expected count is about 209.
- "I could not measure it" is a deliverable. A plausible-but-wrong value renders as something and
  reads as success.

## Git, builds and agents

- Stage commits by dependency, not by authorship or by "what I edited". Committing just the files I
  touched left HEAD not compiling (14 errors) because a committed file referenced an uncommitted
  module from another lane. The working tree built fine throughout, so every local check passed. A
  green working tree is not evidence that HEAD builds. Check with:
  `git worktree add --detach /tmp/hc HEAD && (cd /tmp/hc/d1-server && cargo build --release)`
- "The agent finished" is not "the work is in git." Three crates and an entire agent's deliverables
  sat untracked after being reported done.
- Whole-workspace `cargo build --release` and `-p <crate>` produce different binaries because of
  feature unification. This cost a session to a silent TLS failure.
- Give sibling subagents unique scratch directories.

## Run hygiene

- ⛔ **The "AH hot-swaps without rebooting the client" fact is ORBIT-ONLY.** In-world, the client
  holds a live TCP connection to the AH; `kill -9` on the AH RSTs it and the session dies within
  seconds - the 20.5 s tolerance is for SILENCE, not for a reset (run `20260811-131842-GEARED-RADV-6`
  died this way mid-experiment). An in-world sensor/phase experiment therefore needs the AH armed
  correctly FROM BOOT, or an AH-side re-send knob - never a process swap.
- ⚠ **`sudo nsenter -t <pid> -n -- sudo -u <user> …` did NOT deliver the child into the target
  netns on this box** (the replacement landed in the HOST namespace and listened on the host's
  0.0.0.0:4110 - verify with `readlink /proc/<child>/ns/net` against the emulator's before trusting
  any nsenter'd service). Also: `pgrep -f | head -1` can grab a transient bystander; identify a
  process by reading its `/proc/<pid>/cmdline` and env before killing it.

- The environment is part of the experiment. A run launched without D1_MANIFEST_FILE served a 2-byte
  manifest and died three gates earlier than the thing under test, with a halt line that reads exactly
  like "the patch under test broke the boot". Launch through run-boot.sh, the only launcher that pins
  the known-good environment.
- pkill -f can match its own shell and kill the command that issued it, swallowing the output and
  leaving a plausible partial result. Use a [c]haracter class, and kill and start in separate
  invocations.
- An apostrophe inside bash -c '...', including in a comment, shifts every positional argument, and
  bash -n passes because the outer syntax stays valid. A syntax check is not a quoting check: assert
  the property you need.
- pgrep -x shadps4 matches nothing because the process renames itself shadPS4:Main. Both launchers'
  one-instance guards were silent no-ops for weeks. A guard that never fires looks identical to a
  guard that always passes.
- Start the virtual pad before entering the network namespace. udev hotplug is netlink and netlink is
  network-namespaced, so a pad created afterwards is never announced to the emulator.
- An agent-driven emulator must never claim the operator's physical pad. The 0xFFFF/0xFFFF ignore
  convention is a no-op, since it is a VID/PID match list matching no real device; build the list from
  real devices.
- Emulator runs need a quiet box (ulimit -c 0). Host contention once produced a convincing
  "deterministic crash" that was entirely self-inflicted.
- The game path must be absolute, and the patch XML must be well-formed (-- is illegal inside an XML
  comment). shadPS4 silently applies nothing rather than complaining.
- Read the crash report before theorising. The client uploads its own to runs/<run>/uploads/, a
  multipart wrapper around a ZIP whose crash_info.txt names the halt, the bootflow state and the
  build. It named a manifest failure exactly, in one read.
- Zeek silently discards every packet with a bad checksum, and NIC offloading makes them all bad.
  Without -C the dns/ssl/x509 logs are absent and the process still exits 0, which reads as "the
  client never spoke DNS or TLS". When a tool's default is to drop malformed input, its "clean run"
  and its "saw nothing" are the same output.
- Never move this tree over exFAT. It silently mangles 672 filenames containing ? or : and every count
  and hash check still passes.
- Hooks that patch a per-frame path must be applied only once the state they assume exists. Applying
  the in-world spawn patches during the ladder walk crashes the client before it arrives.

## Specifics worth not re-deriving

- Heap pointers are absolute; only eboot vaddrs get the base. d1guest read of a heap pointer without
  --abs reads unmapped memory that returns zeros, indistinguishable from "the structure was torn
  down". A whole causal theory was published on that; with --abs the same address read {1,1,0}.
- player[0] is `[[0x2725700]+8] + 0x80`. The pool header is at +0x0; reading it as player 0 is off by
  0x80 and yields plausible-looking garbage.
- Sentinels differ by width and the wrong one silently erases data. 0xFFFF is an empty content index,
  0xFF is an empty stat key, and 0x0000 is valid everywhere, so shipping 0 where 0xFFFF was meant
  resolves to a real record.
- A vtable pointer is loaded at vtable+0x10, not at the RTTI-reported address, and heap-constructed
  classes are referenced at V-0x10. Of 129 game-system vtables, 94 are referenced only at V, 33 only
  at V-0x10, and 0 at both. Always query both; asking only V loses 26% and reads as "never
  instantiated".
- An RTTI name gives you a class, and a class can own more than one vtable. Name to typeinfo to vtable
  is not a chain of custody for a method; confirm from the dispatch site.
- A tail-merged edge is invisible to an immediate scan, recorded four times. When an immediate scan
  returns [U] for a specific edge, that is a positive signal for a tail-merge, not an absence.
  Related: ICF-folded thunks hide literal arguments.
- The prologue count is not the function count. 11,446 functions have no standard prologue, so a
  prologue-only lookup names the previous function.
- Check name_source before believing an atlas name. The ~ prefix marks four different sources, and the
  string-ref class measured 30.8% accurate as an identity: it means "this function references that
  string", not "this is what it is".
- We can cause an over-read. A clan-tag copy bounds to 17 units against a 5-unit array and stops at
  the source's NUL, so an unterminated tag reads 22 bytes past the object. Four characters is a bound.
- bap means "bootstrap patchable"; the failure string is "failed to load bootstrap_patchable". It is a
  content step delivered over the Demonware transport, not an acronym we invented.
- Hop, don't drive. Reach a state by serving different objects (stays observed-live) before an
  engine-config jump, and before a memory poke (synthetic_ancestor twice over). Controller driving is
  for adjusting within a state. shadPS4 has no savestate subsystem; don't go looking for it.
- "Not load-bearing for the ladder" is not "not load-bearing". The cine-suppress poke was demoted
  because the done mask filled without it - and it was the SCREEN lever the whole time (RADV-6/7:
  black with audio; RADV-2/9: rendered). Before demoting a poke, name every consumer it was
  serving, not just the one you were measuring.
- A decaying poke needs a hold, and the operator's memory of "you have to hold it" is data. The
  [0x26A33E8] fly-in suppress byte is rewritten by the client (every ledger re-poke shows
  before=00); a one-shot poke wears off between teardowns. Loop it, or serve it.
- Gate scripts on state you can read, not on a log line another mechanism can remove. Holding the
  fly-in suppress from ladder start deletes the "Timeout entering the prologue-filler" line the
  script was waiting for; polling the done mask for 0x1FF was always the real condition.
- A poke value without 0x is parsed as DECIMAL and becomes garbage code. `poke 0x411E16
  909090909090` spliced a26b0aaad3... over the je (u64 also clobbered the next instruction's
  first 2 bytes). Hex-prefix every value; match the write width to the patch site; freeze first.
