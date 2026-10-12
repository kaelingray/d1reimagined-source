# CFW → decrypted-dump runbook (Destiny 1, PS4)

End-to-end procedure for taking a **10.00/10.01 PS4** from stock to a jailbroken state and
pulling the two shadPS4 content-gate artifacts - the **decrypted game** and the **decrypted
`sys_modules`** - entirely inside the isolated capture lane. Written from the run that produced the
01.00 base + 26 modules on 2026-08-05; the 01.29 dump follows the same game-dump steps.

**This box IS `203.0.113.84`.** It is the capture rig, the ifr/Clack server, and the patch-estate
host. The console lives on the *contained* segment and must never reach the internet (a firmware
update past **13.00** is irreversible - no downgrade).

---

## 0. Safety model (non-negotiable)

- Console sits on `enp194s0`, a **standalone isolated segment** (`10.99.99.0/24`), NOT bridged to
  the uplink. Containment is by **topology**, backed by an nft forward-drop guard + `ip_forward=0`.
  Verify before trusting: `sudo capture-rig/rig-mode.sh status` must say `CONTAINED`.
- On the console: **Settings → System → Automatic Downloads** - uncheck *System Software Update
  Files*, *Application Update Files*, *Featured Content*. Belt-and-suspenders over the network
  containment. Re-check after any restore.
- Never point the console DNS at a third-party resolver. Ours (`10.99.99.1`) is fail-closed.

## 1. Rig up (once)

```
sudo capture-rig/rig-mode.sh contained     # isolate enp194s0
systemctl status d1-console-dhcp           # fail-closed DHCP/DNS on the segment
```
GoldHEN's FTP + the console decrypt-on-read is the workhorse; wireshark-cli/ftp tooling already
installed. Console DNS queries log to `/var/log/d1-console-dns.log` (research bonus).

## 2. Jailbreak (Vue System Backup → GoldHEN)

Staged in `cfw-staging/` (all GPL/public, verified): `VueSystemBackup.7z`, `GoldHEN_v2.4b18.7z`.

1. Write `cfw-staging/usb-ready/` to a USB (exFAT, MBR). Layout: `PS4/EXPORT/BACKUP/…/archive.dat`
   (the restore image), `payload.bin` (= GoldHEN), plus recovery saves.
2. Console: **Settings → System → Back Up and Restore → Restore PS4** → pick the backup. **Wipes
   the console**, reboots into Vue + the fake-activated account `1111111111111111`.
3. Network the console - **STATIC IP** (DHCP is unreliable here, see Gotchas):
   IP `10.99.99.20` / mask `255.255.255.0` / gateway `10.99.99.1` / DNS `10.99.99.1`. The internet
   test will FAIL ("IP obtained, internet failed") - that is the correct contained result.
4. Open **PS Vue** → OK past the PSN prompt → run the exploit. It loads `payload.bin` (GoldHEN).
5. **GoldHEN Settings → enable FTP Server** (port **2121**). That's all we need. (BinLoader
   optional; harmless.)

Confirm from the box: `nc -z 10.99.99.20 2121` and `curl ftp://10.99.99.20:2121/` lists the PS4 root.

## 3. Dump `sys_modules` (deliverable 2) - no game needed

GoldHEN's FTP **decrypts SELF/sprx on read**, so system modules come off as plain `\x7fELF`.
The 26 shadPS4-required modules live in `/system/common/lib/`. Pull + verify:

```
# pulls all 26, verifies each is \x7fELF, writes SHA256SUMS + MANIFEST.txt
# (the loop used on 2026-08-05 is reproducible; see dumps/sys_modules/)
```
Result landed: `dumps/sys_modules/` - 26 files, 6.8 MB, all decrypted. Record the console firmware
(10.01) in the manifest; if a module misbehaves in shadPS4, note it first.

## 4. Dump the decrypted game (deliverable 1)

The decrypted game filesystem only exists as a sandbox mount **while the game is running**.

1. Install the game version you want on the console (base from disc; patched via §5).
2. **Launch Destiny.** Reaching the splash screen is enough - the mount appears at
   process start. (If it hangs at splash, disabling **game debug at the GoldHEN level** let it
   reach the menu; not required for the dump.)
3. From the box, survey then pull the app0 mount with the hashed FTP tool:
   ```
   python3 capture-rig/ftp_dump.py survey /mnt/sandbox/pfsmnt/CUSA00219-app0
   python3 capture-rig/ftp_dump.py pull   /mnt/sandbox/pfsmnt/CUSA00219-app0  dumps/<name>
   ```
   - **Base (01.00):** mount `CUSA00219-app0`. Done → `dumps/base-CUSA00219/` (257 files, 18.61 GB).
   - **Patched (01.29):** mount `CUSA00219-app0-patch0-union` (base+patch merged = effective game).
4. **`param.sfo` is locked while the game runs** and isn't in `/user/appmeta` (icons only). Pull it
   from **`/system_data/priv/appmeta/CUSA00219/param.sfo`** (also `disc_info.dat`, `nptitle.dat`,
   `npbind.dat`). It carries the version (base = `01.00`) - drop it into the dump's `sce_sys/`.
5. Everything is hashed into `MANIFEST.sha256`; write a `README.txt` with version/title-id/source.

shadPS4 wants **decrypted** - which is exactly what this produces (it has no console keys, which is
why the modules must come from *your* console).

## 5. Install a patch (to reach 01.29)

Preserved patch estate on this box: `psn-acquisition/pkgs-us/patch-01.29/` (8 pieces, verified,
`c_date=20161010` = 2.4.1). **These are ONE PKG split into 4 GiB chunks**, not independent parts -
only `_0` carries the PKG magic `\x7fCNT`. They must be **concatenated** before install:

```
cat …_0.pkg …_1.pkg … …_7.pkg > cfw-staging/Destiny-01.29-CUSA00219-merged.pkg
# verify: size == sum of parts; first 4 bytes == 7f 43 4e 54
```
FAT32 can't hold a 4 GiB+ file, so put the merged `.pkg` on an **exFAT** USB (MBR), move it to the
console, and **Package Installer → install the single .pkg** (source: USB). Installing the split
pieces directly throws **`CE-35659-8`** ("data corrupted"); the merged single PKG installs clean.
Base game must already be installed and be the **same title ID** (`CUSA00219`) as the patch.

After install, relaunch Destiny and dump the `-app0-patch0-union` mount per §4.

## 6. Transfer & integrity

All dumps land under `dumps/` on this box, pulled over the contained link
(`console 10.99.99.20 → box 10.99.99.1`) - no internet path ever opens. Every file is SHA-256'd in a
per-dump manifest. Feed the decrypted game folder + `sys_modules/` to the shadPS4 fork at
`~/dev/shadPS4` (branch `codex/hindsight-vr`).

---

## Gotchas hit on the first run (save yourself the debugging)

1. **DHCP doesn't reach the console.** The box runs a second dnsmasq for the wifi AP on a wildcard
   `0.0.0.0:67` socket that swallows the console's DHCP broadcast. Fix: give the PS4 a **static IP**
   (§2.3). The console DOES broadcast fine - verified with `tcpdump -i enp194s0`.
2. **`param.sfo` reads 0 bytes** from the app0 mount (locked by the running game) and isn't in
   `/user/appmeta`. Get it from `/system_data/priv/appmeta/<TITLEID>/` (§4.4).
3. **Split patch PKGs → `CE-35659-8`.** Concatenate `_0.._N` into one `.pkg` first (§5). No online
   tool needed - that's all those tools do.
4. **GoldHEN cheat menu crashes Destiny.** Don't open it. Disabling game-debug at the GoldHEN
   level (not just per-game) let the game boot to menu.
5. **FAT32 4 GiB file limit.** Patch pieces are exactly 4 GiB; the merged PKG is 31 GB - both need
   **exFAT** (keep MBR partition for PS4 payload recognition).
6. **Decrypt-on-read is the whole trick.** GoldHEN FTP hands you decrypted ELF/sprx; no separate
   dumper (etaHEN) is required for either deliverable. etaHEN would not load via Vue's elfldr here
   anyway - didn't need it.

## Console facts (this unit)

fw **10.01** · MAC `xx:xx:xx:xx:xx:xx` · serial `MB068826550` · disc **CUSA00219** (US,
`UP0002-CUSA00219_00-DESTINYTHEGAME01`) · base version **01.00**.
