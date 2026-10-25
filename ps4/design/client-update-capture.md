# Bounded client-update diagnostics

The normal Activity Host log captures the first payload of each inbound type per
connection and every udt-18 incident. One logged udt-21 or udt-47 payload does not
mean that later client-owned state is unchanged.

For a contained diagnostic run, set `D1_AH_CAPTURE_CLIENT_UPDATES=128` to log up to
128 additional changed payloads, shared across udt 21 (client authoritative data)
and udt 47 (high water). The hard cap is 256 per connection. Zero, absent or invalid
values leave the normal capture policy in place. This records private raw data;
keep the resulting logs out of public releases and evidence packages.

`D1_AH_CAPTURE_CLIENT_UPDATES_AFTER_S=120` delays extra captures until 120 seconds
after the first received 21/47 payload. Choose the delay for the approach being
tested so changing counters do not consume the budget before the trigger. The
normal first payload is still recorded immediately. The first sample of each
type in the selected window establishes a fresh baseline; subsequent exact
unchanged payloads are skipped. Each type keeps at most 2 KiB of comparison data. Larger payloads
remain eligible without retaining them, are explicitly truncated to 2 KiB in
the log, and consume the same bounded budget.

This changes diagnostic events only. It sends no acknowledgement, changes no
authority and advances no objective. In particular, udt 21 remains fire and
forget. A changed update alone is not a decoded player-trigger event.

The pending gameplay check is a controlled approach to Enter the Wall with the
correct tutorial activity, region and cast, correlating these updates with the
client-owned trigger. Unit tests establish filtering, delay, shared budget,
truncation and unchanged default behavior; they do not establish trigger delivery
or mission progression. A newly built host and a native run are still required.
Existing schema-codec tests also require their separately supplied, pinned game
executable; diagnostic unit tests do not replace that acceptance requirement.
