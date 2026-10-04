# 01.24 launch failures and evidence ownership

`FINAL124_JOIN93` failed on 4 October 2026 before SignOn or BAP progress. Its
manifest reported “rpcs3 exited during stack launch” because exact process
identity lookup returned no match. That lookup alone does not distinguish an
actual exit from an identity inspection failure. No retained-handle exit code
was recorded, and RPCS3 stdout/stderr had not been redirected, so the referenced
`FINAL124_JOIN93-rpcs3.err` was not an available diagnostic source.

The wrapper then marked an 87,865,802-byte shared `RPCS3.log` snapshot as captured
without checking whether that file had been written by the new launch. A copied
file or a `captured` summary flag alone must therefore not be used as proof that
JOIN93 started the emulator or executed guest code. The previous-run snapshot
remains private; it does not establish the cause of the launch failure.

## Changes

`Start-LocalStack.ps1` retains the `Process` handle returned by each launch and
checks its `HasExited` value when startup verification fails. An actual exit
includes both the signed decimal exit code and its eight-digit hexadecimal
representation. A live process whose identity cannot be verified is reported
as an identity verification failure. Diagnostic code never obtains a replacement
process by PID and does not grant additional cleanup authority.

RPCS3 launches remain hidden. Their console streams now go to the private,
run-specific `<Name>-rpcs3-stdout.log` and `<Name>-rpcs3.err` files. Failure
diagnostics include the last eight lines from at most 8,192 bytes per stream.
RPCS3 stderr can include nonfatal driver/compiler output, so its existence alone
does not fail an otherwise running emulator. The existing debugger startup
deadline and verified process lifecycle still determine startup success.

A ready debugger listener does not imply that the Destiny game window already
exists. JOIN95 reached stack readiness while PPU startup was still running; the
scheduled title input then failed because no matching game window existed.
`Send-RpcsKey.ps1 -IfWindowReady` now returns false before posting when that
window is absent. The run wrapper retries in its normal observation loop,
within the original boot deadline, and counts only actual input. It supplies
the manifest identity, which is checked again around window lookup and input;
identity failures still propagate. If no game window appears by the boot
deadline, the wrapper reports that no input was posted and preserves the run's
diagnostics.

`Invoke-Final124Run.ps1` selects a shared log only if its last-write UTC timestamp
is at least the exact emulator creation timestamp from the ownership manifest.
It checks both supported log locations. Missing or older logs produce an
explicit capture failure instead of publishing old evidence under the new run
name. This timestamp gate rejects a previous run's untouched log; it does not
replace process ownership checks or establish guest progress by itself.

## Validation

`scripts/Test-Final124Run.ps1` uses temporary files and hidden PowerShell/Python
fixtures. It covers actual exit-code reporting, a live retained process, bounded
output, stale-log rejection, a fresh alternate log location, and a failed launch
whose shared log belongs to an earlier run. Existing process identity, cleanup,
active-writer, interrupted-launch, and retained-stack cases are included. The
suite also exercises delayed window readiness without counting or posting the
missing-window attempt, and verifies that readiness mode cannot suppress an
input ownership failure. The fixtures do not start RPCS3, RPCN, or any network
service.
