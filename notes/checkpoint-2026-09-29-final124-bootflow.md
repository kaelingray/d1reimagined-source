# 01.24 bootflow investigation, 2026-09-29

This continues [the first 01.24 smoke runs](checkpoint-2026-09-28-final124-run.md).
The working target remains the 01.00 milestone: character creation and Dock 13.
As of this checkpoint, 01.24 reaches the Taken King title, accepts an automated
Cross press, authenticates with the local SignOn service, downloads our
diagnostic content manifest, opens packages, and remains on a black loading
screen. It has not connected to the local BAP service. **Do not treat the
spinner or package-open count as proof that sign-in succeeded.**

## New direct observation: the exact boot step

`scripts/rpcs3_final124_bootflow.py` reads the final executable's bootflow
singleton through the persistent RPCS3 GDB daemon. It is a read-only,
01.24-specific snapshot tool. The singleton pointer is at `0x01abc89c`; the
current and target stage numbers are at object `+0x118` and `+0x128`.
The 38 stage names are a pointer array at `0x012f7f00`.

In the stalled 01.24 sessions the current and target stages are both **19,
`bootflow:bap_signin`**. The stage object is at singleton `+0x80 + 19*4`;
its `+0x30` substate provides a second discriminator. The local BAP listener
at `127.1.1.127:12850` records **zero incoming connections**. The client is
therefore blocked within this boot step before the transport handshake, or in
a prerequisite that this step drives. The exact causal dependency has not
yet been proven.

The stage's tick function is `0x363138`, which delegates to `0x3629ec` until
the stage's state `+0x30` reaches `0x15`. Substate `0x0c` calls `0x531fe0`
to work on resource slot 3. The eight resource slot states start at
`0x01ada1b0` (12 bytes per slot), and their names at `0x01315588`.
Slot 3 is `client_bootstrap_patchable`. The resource states in these runs
distinguish a still-pending `1` from a later `2`; inspection of `0x531fe0`
and `0x531e78` shows the async request is issued before state `1`, then a
completion path can move it to `2`. `0x531fe0` also handles terminal states
`4` and `5`; their precise meanings are not yet mapped.

The other key addresses are: bootflow tick `0x3569f4`, global bootflow
dispatcher `0x340068`, resource name table `0x01315588`, resource state
table `0x01ada1b0`, stage 19 object `0x01abdca0`, and its tick
`0x363138`. These are **01.24** EBOOT guest addresses only. The stage 19
vtable at `0x013e85e0` points through a PPU function descriptor at
`0x018558a0` to `0x363138`.

Example during an active run:

```powershell
python scripts/rpcs3_final124_bootflow.py `
  --out local/RESEARCH/ps3_updates/MY_RUN-bootflow.json
python scripts/rpcs3_thread_snapshot.py `
  --out local/RESEARCH/ps3_updates/MY_RUN-thread-snapshot.json
```

These samples are sequential, not atomic. Save JSON under `local/`, because
it can contain transient game state. Do not commit them.

## Manifest and package-chain experiment

The original diagnostic manifest picked the newest package file per ID.
`ps3_client_bootstrap_patchabl_0172_2.pkg` is only 38 KB, with 949 entries
but most data blocks referring to earlier patch levels: 68 blocks use patch
0, 49 use patch 1, and one uses patch 2. Its disc `_0` base is 17.919 MB
and `_1` is 11.894 MB. This is direct header/block-table evidence that
the `_2` package is an overlay, not a standalone package. The local
`d1_pkg.py` reader extracted its root tag successfully once `_0`, `_1`,
and `_2` were placed together in a scratch folder under
`local/RESEARCH/ps3_updates/bootstrap_chain` using the user's Oodle DLL.
That offline success does not prove the game has registered the chain
correctly.

The two shared-manifest files (`0430` and `0431`) are the disc's only
marker-zero packages. Their `_4` update copies also have marker zero.
`build_manifest_probe.py` now accepts marker-zero files only when named by
`--replace`; automatic latest-package selection still skips them. The
script also supports `--chain` entries before the selected final entry for
one ID, to probe whether the game needs explicit base and intermediate
registrations. The output manifest remains under `local/`.

| Runs | Selection | Direct result |
| --- | --- | --- |
| 010 | 311-entry union, disc shared manifests | Black spinner; 22 guest threads sampled twice, mostly at stable waits. No BAP connection. |
| 011–016 | Breakpoint/trace experiments around bootflow, package registration and network calls | Same spinner and no BAP; see limitation below. |
| 017 | Union plus `_4` shared manifests | Both updated shared files opened; still no BAP. Post-sign-in screenshot was not preserved. |
| 018 | Same as 017, RPCS3 `PPU Debug: true` | Same spinner and 325 package opens; the breakpoint experiment still reported zero hits. RPCS3 config was restored afterward. |
| 019 | Same as 017, live bootflow snapshot | Stage 19 `bap_signin`, substate **12**, resource slot 3 `client_bootstrap_patchable` state **1**, handle `0x01878002`; no BAP connection. Two snapshots agreed. |
| 020 | 160-entry disc manifest with only `0151` replaced by update `_4` | Same visible spinner, but stage 19 reached substate **16** and slot 3 state **2**. It stayed there on later samples; no BAP connection. |
| 021 | Full union but override `0172` back to disc `_0` | Game froze at PPU `0x008570a4`, read access violation at `0x8`; 327 package opens. Disc bootstrap is not compatible with the rest of this manifest as tested. |
| 022 | Full union, `_4` shared manifests, explicit `0172` `_0` + `_1` + `_2` chain | Game opened all three bootstrap files, then stayed at stage 19 substate **12**, resource state **1**, no BAP. Chain registration alone did not resolve the wait. |

Run 023 (the same explicit bootstrap chain but disc `_0` shared manifests)
was in progress when this document was drafted. Record its result below.

This comparison narrows the first hang to content/resource setup **inside
the BAP sign-in step**, but does not prove that `0172` alone is the cause:
run 020 also changes hundreds of other package choices from run 019. Run
023 is intended to isolate the two shared-manifest choices. Avoid assuming
the unanswered 29-byte `0e 02` STUN packet is the blocker: the 01.00 client
also sent it periodically during an otherwise working session
([older trace](test-log.md)). The noisy `ENOTCONN` infoctl entries start
before SignOn and remain unproven as a cause.

## Debugger limitation

`scripts/rpcs3_gdb_daemon.py` now parses RPCS3's one-page
`qfThreadInfo` response and provides a read-only `snapshot` command.
The separate `rpcs3_thread_snapshot.py` saves all guest PCs and short
stack chains. Both worked. In contrast, RPCS3's GDB stub replied `OK`
to `Z0` breakpoints but produced zero hits, even at a frequently sampled
main-loop syscall instruction and with `PPU Debug: true`. This does not
prove the code path was never executed. Use memory snapshots and logs for
now; investigate whether LLVM needs an interpreter/debugger mode before
trusting `Z0` tracing. The 01.24 main thread frequently samples at
`0x000fe158`, a syscall in a timed wait, with a stack through `0x002bcd44`.

## Local evidence and resume

Raw per-run logs are at
`local/SIGNON_LOCAL/logs/FINAL124_SMOKE0{10..23}_*-{rpcs3,signon,stun,bap,dns-queries,...}.log`.
Each completed run has a structured index with per-file SHA-256, package
opens, network requests, STUN packet shapes, errors and screenshot hashes:
`local/RESEARCH/ps3_updates/run-index/FINAL124_SMOKE0{10..23}_*.json`.
Bootflow and thread snapshots, generated manifests, and the offline
bootstrap package extracts are adjacent under `local/RESEARCH/ps3_updates/`.
The screenshots are under
`local/RPCS3_FINAL124_SMOKE001/screenshots/BLES01857/` and named in
their run indexes. Only code and this note belong in Git. The `local/`
tree, decrypted ELFs, packages, account material, keys, and Oodle DLL must
remain untracked.

To continue a controlled test, launch with an explicit manifest file:

```powershell
pwsh -NoProfile -File scripts/Start-FinalBuildSmoke.ps1 `
  -DataRepo ../d1xenia-data `
  -Rpcs3Dir local/RPCS3_FINAL124_SMOKE001 `
  -GameUsrdir local/01_WORKING/ps3_destiny/PS3_GAME/USRDIR `
  -ManifestFile local/RESEARCH/ps3_updates/manifest-final-bootstrap-chain.bin `
  -Name FINAL124_NEXT -Run
$env:D1_RPCS3=(Resolve-Path local/RPCS3_FINAL124_SMOKE001).Path
pwsh -NoProfile -File scripts/Invoke-RpcsStep.ps1 -Keys X -WaitSeconds 80
python scripts/rpcs3_final124_bootflow.py `
  --out local/RESEARCH/ps3_updates/FINAL124_NEXT-bootflow.json
```

Before another RPCS3 launch, stop and preserve the previous run's locked
`RPCS3.log` under its per-run name, then run
`scripts/summarize_final_smoke.py` as shown in the previous checkpoint.
The emulator profile's `PPU Debug` setting has been restored to `false`.

Next, inspect run 023's stage and resource states. If explicit bootstrap
registration still leaves slot 3 pending, examine the async handle behind
`0x01878002`, package chain dependencies and completion callback around
`0x15216c` / `0x531e78`; compare the _disc_ and _update_ bootstrap root
asset and the `0430/0431` shared-manifest chain. If it reaches substate 16,
inspect the separate wait there: stage tick calls `0x359288(2)` and polls
`0x3592d8`, whose state lives in the bootflow auxiliary object at
`0x01abc8b8` -> pointer `+0x80`. Record each changed input and its live
stage result before broadening the manifest or changing network replies.

## Run 023 result

Pending at draft time.
