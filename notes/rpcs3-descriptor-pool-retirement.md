# Descriptor pool retirement — 2026-10-06

The existing idle-table sweep destroys pools after 256 unused presentation frames,
checked every 64 frames. That interval is a retention policy, not GPU completion
evidence. Destroying a pool invalidates every set it allocated; submitted uses must
have finished ([Vulkan requirement](https://docs.vulkan.org/refpages/latest/refpages/source/vkDestroyDescriptorPool.html)).

There is a second lifetime to protect. `descriptor_pool::next_subpool()` queues a
GC callback capturing its owner through `this`. The driver manager runs those
callbacks after GPU completion, outside the resource manager's lock. Async work
can also postpone their GC scope. A completed GPU watermark alone therefore does
not establish that the pool's host callbacks have finished. Releasing their owner
by age can invalidate both GPU descriptors and a pending callback pointer.
This source defect is established; its contribution to JOIN136/137 remains unproven.

`patches/rpcs3-descriptor-pool-retirement.patch` keeps the existing sweep policy and
adds two nonblocking requirements: a completed submission strictly later than the
table's last use, and no pending pool-reset callbacks. The strict comparison adds
one submission of margin and avoids treating the initial zero watermark as completed
work. Each queued reset increments an atomic owner-retention count, then decrements
it only after `reset()` returns. This also covers delayed GC scopes without guessing
their eventual event ID or waiting for the driver-manager thread.

Every table commit records its current submission scope, including reuse of an
unchanged descriptor set. Allocation can submit work while recovering fragmentation;
the dirty path records the resulting scope after allocation and descriptor setup.
Idle tables remain eligible on later sweeps. Released tables still recreate a pool
and rewrite their descriptor template when used again. Normal descriptor writes,
shader translation and draw execution are unchanged.

The inspected submission path flushes descriptor writes on the externally synchronized
RSX caller before dispatching an asynchronous queue submission. The offloader calls
the submit-only overload. The storage-fill/pending-write gap is therefore **not**
established as a concurrent flush race on that normal path.

`scripts/rpcs3_descriptor_retirement_audit.py` extracts the actual sweep, release,
commit and callback-enqueue branch into C++20 lifetime mocks. The exact previous
source fails **13 of 23** checks; the repaired source passes **all 23**. Fixtures cover
the zero bootstrap, incomplete GPU use, equal/later watermarks, unchanged-set reuse,
allocation submission reentry, multiple callbacks, owner retention while reset runs,
GPU completion with delayed cleanup, reactivation, sweep cadence and registry
compaction. They do not execute Vulkan or establish visual acceptance.

Reproduction with a private renderer checkout:

```powershell
python scripts/rpcs3_descriptor_retirement_audit.py --source <private-rpcs3> --out <new-audit-directory>
# From that directory, with MSVC configured:
cl /nologo /std:c++20 /EHsc /W4 descriptor-retirement-audit.cpp /Fe:descriptor-retirement-audit.exe
.\descriptor-retirement-audit.exe
```

The public patch contains only the four scoped renderer edits; prior unrelated
private source changes are not included. It reverse-applies to the tested private
checkout. Native build and run results are recorded separately below.

The Release build succeeds and deploys SHA-256
`b5eba7b0da6984c03bd739b37b1661d2e38275502f50db3a845a0dd411813660`.
The [October 6 checkpoint](checkpoint-2026-10-06-renderer.md) records JOIN138's startup
failure, JOIN139's native reclamation and separate host validation-DLL access violation,
plus the control with that layer absent. No combined graphics repair is established.

The earlier full validation messages contain VUIDs in continuation-line specification
links even when the first callback line has none. Capture now recognizes bounded
continuations across log reads and resets their context per owned run. It retains
normalized IDs and counts only, with warnings and unrelated entries excluded.
All **37** capture tests pass. Earlier delivered packages remain unchanged;
their omitted identifiers reflect the previous first-line extraction.
