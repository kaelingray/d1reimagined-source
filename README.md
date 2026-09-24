# Destiny 1 Xbox 360 → Xenia research

This workspace tracks a staged attempt to run a personally owned Xbox 360 copy of Destiny in Xenia Canary. The first target is a reproducible boot result with a complete log. Playable gameplay depends on additional emulator and service research.

## Current status (2026-09-24)

The USB capture is archived at `D:\Destiny360\00_ORIGINAL_DUMP\USB_CAPTURE_2026-09-22`. All 53 Xbox Content files (16,199,192,576 bytes) matched the USB by SHA-256; `name.txt` also matched. The capture includes the 6.9 GB installed disc game and seven compatibility packs.

The Xbox refused to copy Destiny Content 1–3 and its 173 MB Title Update through its Storage menu. With the console HDD connected directly to the PC, FATXplorer mounted its Content partition read-only. The complete Destiny folder (11 files, 15,049,613,312 bytes) was copied to `C:\Users\lukem\Destiny360\00_ORIGINAL_DUMP\HDD_CAPTURE_2026-09-23`, then to `D:\Destiny360\00_ORIGINAL_DUMP\HDD_CAPTURE_2026-09-23` after reconnecting the archive drive. Every file in both copies matched the mounted HDD by SHA-256. This includes the three previously blocked content files and title update. See `notes/inventory.md`.

Test 001 launched the installed game in Xenia Canary `74c4e4acb` and reached the distorted “Press A to Play” title screen. Pressing A produced Destiny error **canary** (“No internet connection detected”). Gameplay was not reached. See `notes/test-log.md`. [Bungie's error explanation](https://help.bungie.net/hc/en-us/articles/360048716492-Error-Code-CANARY) describes a missing network connection; [Xenia's FAQ](https://github.com/xenia-canary/xenia-canary/wiki/FAQ) states that official Xbox Live connectivity is unsupported. The graphics corruption and missing service connection are separate observed blockers; their exact causes in this build have not been isolated.

Test 002 used a separate Xenia instance. The captured title update installed and patched the game from `v0.0.0.2` to `v0.0.23.2`, but the title screen remained distorted and pressing A produced the same **canary** error. Xenia crashed while importing the first of the three captured Destiny Content files; its log reports STFS block-chain read failures. The original HDD capture remains verified and intact. See `notes/test-log.md` for the exact paths and logs.

Test 003 used Vulkan instead of Direct3D 12; the title screen still had missing horizontal bands. Test 004 added the intact Content 3 package to another isolated Xenia instance. It booted to the title screen without a startup crash, though there is no evidence yet that the game used that package. See `notes/test-log.md`.

The Content 1 import failure is fixed in a local Xenia build. Its directory entries have names but zero stored name lengths; most files are contiguous, while three noncontiguous files need active records from both redundant STFS hash tables. The patch also makes a failed file read stop the import cleanly. In isolated tests the patched build imported Content 1, 2, and 3 successfully: 430 extracted files in total. With all three imported files and the title update present, Destiny still reaches the distorted title screen and shows **canary** after pressing A. Gameplay has not been reached. See `notes/content-analysis.md`, `notes/test-log.md`, `notes/network-blocker.md`, and `patches/xenia-content1-stfs.patch`.

## Next milestone

Test 010 recorded the sign-in, XNet, and XHttp calls through the canary screen. It found that Xenia always reports the Ethernet link as inactive, while profiles remain locally signed in and HTTP startup is unimplemented. Test 011 changed only the Ethernet-link answer to active; Destiny still showed canary. Test 012 changed only the reported sign-in state to online; canary still appeared. Test 013 returned success from the first missing XHttp startup call; Destiny still showed canary. See `notes/network-trace.md`. The real Xbox was routed briefly through the PC and reached the character screen while a limited network trace ran; it resolved Bungie sign-in and content hostnames after Xbox services. See `notes/console-network-capture.md`. A source comparison found useful Live-like code in the Xenia netplay fork, but its HTTP path remains stubbed and the paired Web Services project has no Destiny-specific backend. See `notes/network-options.md`. Keep the graphics investigation separate; Direct3D 12 and Vulkan both showed the same missing bands.

The user also supplied a PS3 game image to evaluate RPCS3 as a second route. The image mounted read-only with standard PS3 game files. An official RPCS3 build, disc firmware, and a working copy of the PS3 game are installed outside this repository. After first-run shader preparation, Destiny reached a visually intact "Press X to Play" title screen. With RPCS3's default disconnected network it showed **canary**; changing only Network Status to Connected advanced to **buffalo**, which requires PSN sign-in. The PS3 route improves the observed title rendering and passes the first network check. With a loopback-only RPCN server and a local throwaway account, Destiny also passes the PSN check (buffalo), requests an NP ticket, and stops at "Destiny servers are not available" while trying Demonware/Bungie HTTPS sign-on hosts. Gameplay still needs a Destiny-compatible service. See `notes/ps3-route.md` and `scripts/rpcn_local_account.py`. A loopback-only toolset in `server/` now also answers DNS and Demonware's STUN/NAT check (clearing error **marionberry**), so Destiny reaches its SSLv3 HTTPS sign-on without contacting any live service. Destiny trusts a root CA built into its executable as well as the firmware roots. An RPCS3 patch that swaps that embedded root for a private test CA lets the SSLv3 handshake complete, A local SignOn reply built from the game's own protobuf tables is accepted, and the client moves on to Demonware authentication, which is now the gate. See `server/README.md` and `notes/test-log.md` (PS3 comparisons 004–015).

## Capture procedure

1. On the Xbox 360, confirm Destiny launches from the physical disc and record the version shown in game, if available. Bungie identifies Update 2.3.1 as the final Xbox 360 branch.
2. In **Settings → System → Storage → Games and Apps → Destiny**, write down every Destiny item, its type, size, and storage device. Keep the console's existing installation intact.
3. Prepare the USB in Xbox 360 Storage. Formatting erases its current contents.
4. Use the Xbox storage **Copy** action to put the installed game and each Destiny update/add-on package on the USB. Check the USB in Xbox Storage before removing it, and confirm the original content is still on the console.
5. On Windows, copy the USB's contents to a separate archive outside this repository, including hidden files and any `$TitleUpdate` folder. If the disc installation appears as one main file plus a matching `.data` folder, preserve both together. Do not rename files inside the archive. Record the actual paths found; content may not all sit under one Destiny folder.
6. Generate a SHA-256 manifest for the archive using `scripts/New-Manifest.ps1`. Keep the manifest and a second copy of the archive on another drive if possible.

Do not put game dumps, profile data, update packages, or logs containing account details in Git. `.gitignore` excludes local capture directories as an extra guard.

## First Xenia experiment

After the archive is complete, make a working copy and open its game container in the pinned Xenia Canary build under `D:\Destiny360\XENIA`. Use `scripts/Start-XeniaTest.ps1` to keep a separate log and config snapshot for each run. Record the visible result in `notes/test-log.md`.

The installed build (`74c4e4acb`, 2026-09-21) generated `xenia-canary.config.toml` and already has `mount_cache = true`. Keep that initial setting for Test 001. Only compare with `false` later if the log gives a reason. The [existing Destiny compatibility report](https://github.com/xenia-project/game-compatibility/issues/269) marks cache mounting as required and records a title screen and Xbox Live sign-in blocker; use it as prior evidence, not as a prediction for the current build.

## Prepared PC environment

- Windows 11 Pro, Ryzen 5 7600X, NVIDIA RTX 5070, about 32 GB RAM.
- Official Xenia Canary `74c4e4a` is in `D:\Destiny360\XENIA`; the original downloaded archive is in `D:\Destiny360\TOOLS`.
- `D:` had about 173 GB free before setup. Confirm free space again before making multiple full copies.
- The pinned archive and executable hashes are recorded in `notes/environment.md`.

## References

- [Xenia Canary Quickstart](https://github.com/xenia-canary/xenia-canary/wiki/Quickstart)
- [Xenia Canary FAQ](https://github.com/xenia-canary/xenia-canary/wiki/FAQ)
- [Bungie Xbox 360 installation guide](https://help.bungie.net/hc/en-us/articles/360048720032-Installing-Reinstalling-and-Updating-Destiny-on-the-Xbox-360-Legacy-Console)
- [Bungie Destiny 1 legacy support](https://help.bungie.net/hc/en-us/articles/360049024352-Legacy-Support-Guide-for-Destiny-1)
- [Candidate Destiny data tools](notes/research-tools.md)
