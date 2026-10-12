# Contributing

This subtree is maintained in the public D1 Reimagined source repository.
Use the [root contribution guide](../CONTRIBUTING.md) and
[current contributor checkpoint](../docs/STATUS.md). Submit issues and pull
requests there. The supplied upstream snapshot guidance below records its
original workflow; this public branch is updated with ordinary commits, and
no private repository history is imported.

Thanks for your interest. Issues and merge requests are welcome. This page explains how
contributions actually land here, because this repository works a little differently than most.

## How this repository is published

This public repository is a snapshot of a private working tree. Development, testing and live
verification happen against real hardware and captures that cannot be published. Each release of
the snapshot replaces the previous history.

What that means for you:

* Merge requests are reviewed here, but they are applied to the internal tree by a maintainer
  and appear in the next snapshot. Your MR will not be merged through the button.
* Credit is preserved. Applied contributions are recorded in `d1-server/ATTRIBUTION.md` and in
  the commit message of the snapshot that carries them.
* Branches you fork from may be rewritten between snapshots. Small, self-contained MRs survive
  this well; long-running branches do not.

## What makes a good contribution

* **Issues:** a precise observation beats a broad report. Client behavior, wire captures of your
  own sessions, reproducible boot results, and corrections to the protocol docs are all valuable.
* **Code:** keep it small and testable. Server code is Rust; every wire format lives in exactly
  one crate, and load-bearing tools carry a `controls` subcommand that exits 2 on failure. New
  code should follow that pattern.
* **Docs:** the documentation records evidence, not just conclusions. Claims carry confidence
  markers and provenance. If you correct a document, say how you know.

## AI-assisted contributions

AI-assisted work is accepted. Much of this project is built with AI tooling and it would be
strange to pretend otherwise. The standard is understanding, not authorship: you are expected to
know what your submission does, why it is correct, and be able to answer questions about it in
review. A generated MR that its submitter cannot explain will be closed, politely.

## Licensing and sourcing rules

These are strict because the project depends on them:

* Code contributions are accepted under **GPL-2.0-or-later**, the license of `d1-server/`.
* Do not port from, copy from, or extract facts from **AGPL or unlicensed** code, including
  other server reimplementations. Absence of a license is not permission. GPL-compatible and
  permissive sources are fine, with credit named in `d1-server/ATTRIBUTION.md`.
* The shipped client, observed on your own hardware, outranks all prior art as a specification
  source.
* **Owned media only.** Never include game, patch or firmware assets, in any form, in any
  contribution. This includes fixtures and test data derived from game content.

## Conduct

Be professional and be specific. Disagreements about protocol behavior are settled by evidence
on the wire, which has the pleasant property of ending arguments.
