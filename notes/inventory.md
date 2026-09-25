# Xbox 360 Destiny content inventory

Date: 2026-09-22 (photo inventory)
Console model: Xbox 360 E, model 1538 (confirmed from user's label photo; serial number deliberately omitted).
Disc edition:
Destiny version shown in game (if available):
USB capacity: Approximately 232 GB usable (224 GB free, 8.2 GB Games and Apps used on 2026-09-22); nominal capacity not shown.

| Item as shown in Xbox Storage | Type | Size | Original device | Copied to USB? | Notes |
| --- | --- | ---: | --- | --- | --- |
| Destiny | Installed Xbox 360 Game | 6.9 GB | Disc | Installed directly to USB | USB Storage Device item created 2026-09-22 21:30; USB showed 217 GB free afterwards |
| Destiny Title Update | Update | 173 MB | Hard Drive | No (user reports copy failed) | Created 2017-04-07 16:54 |
| Destiny Content 3 | Saved Game | 1.8 GB | Hard Drive | No (user reports copy failed) | Created 2014-12-30 08:44 |
| Destiny Content 2 | Saved Game | 3.9 GB | Hard Drive | No (user reports copy failed) | Created 2014-12-30 08:37 |
| Destiny Content 1 | Saved Game | 85 MB | Hard Drive | No (user reports copy failed) | Created 2015-05-18 20:08 |
| Destiny Compatibility Pack 13 | Downloaded Items | 15 MB | Hard Drive | Likely yes; verify item list | Created 2017-04-07 17:01 |
| Destiny Compatibility Pack 12 | Downloaded Items | 10 MB | Hard Drive | Likely yes; verify item list | Created 2016-07-23 16:48 |
| Destiny Compatibility Pack 11 | Downloaded Items | 799 MB | Hard Drive | Likely yes; verify item list | Created 2016-07-23 16:48 |
| Destiny Compatibility Pack 08 | Downloaded Items | 1.6 GB | Hard Drive | Likely yes; verify item list | Created 2015-12-06 21:40 |
| Destiny Compatibility Pack 07 | Downloaded Items | 2.0 GB | Hard Drive | Likely yes; verify item list | Created 2015-12-06 22:24 |
| Destiny Compatibility Pack 06 | Downloaded Items | 2.0 GB | Hard Drive | Likely yes; verify item list | Created 2015-12-06 23:20 |
| Destiny Compatibility Pack 05 | Downloaded Items | 1.9 GB | Hard Drive | Likely yes; verify item list | Created 2015-12-07 00:17 |

The Xbox screen showed 1.8 GB free while displaying these Hard Drive items. A later USB Storage Device photo showed 224 GB free and 8.2 GB of Games and Apps used, consistent with the seven Compatibility Packs (about 8.3 GB using rounded item sizes). Copy was selectable, but for Content 1–3 and the Title Update the console displayed: “Can't Transfer Item — This item can't be moved or copied.” The disc was then installed directly to USB, adding a 6.9 GB Installed Xbox 360 Game item and leaving 217 GB free. The four blocked items were captured directly from the HDD on 2026-09-23 as described below.

Windows archive path: `D:\Destiny360\00_ORIGINAL_DUMP\USB_CAPTURE_2026-09-22`
Archive copy completed on: 2026-09-22 21:50 BST
Manifest path: `D:\Destiny360\HASHES\usb_content_source_2026-09-22.csv` and `D:\Destiny360\HASHES\usb_content_archive_2026-09-22.csv`; 53 Content files, 16,199,192,576 bytes, zero SHA-256 differences. `name.txt` also hash matched.
Second archive copy path (if made):

Note whether the installed disc game has both a main file and a matching `.data` folder: Yes, under `Content\0000000000000000\415608F8\00004000\ED4E3CCA83E6F64008DDF957088F3E06` and the adjacent `.data` folder. The working copy at `D:\Destiny360\01_WORKING\test001` has all 54 files and the same total bytes as the archive.

Note whether the USB has a `$TitleUpdate` folder and record any Destiny Title Update path: No `$TitleUpdate` folder was present on the USB; the Xbox refused to copy its Title Update item.

## Direct HDD capture (2026-09-23)

- Source drive: Xbox 360 E internal 250 GB Seagate ST9250315AS, connected by SATA to the PC. Windows showed it as Disk 0, RAW; it was not initialized or formatted.
- Access: FATXplorer 3.0 beta mounted the HDD Content partition as `X:` with **Read-Only** selected. The FATXplorer mount driver was installed to expose this virtual volume; the Xbox HDD itself was not modified.
- Source folder: `X:\Content\0000000000000000\415608F8`. The destination preserves this relative folder structure under `C:\Users\lukem\Destiny360\00_ORIGINAL_DUMP\HDD_CAPTURE_2026-09-23`. The usual `D:` archive drive was disconnected to free a SATA port.
- `00000001` contains `1XXXXXXX` (88,133,632 bytes), `2XXXXXXX` (4,093,612,032 bytes), and `3XXXXXXX` (1,891,659,776 bytes), matching the Xbox's Destiny Content 1–3 items.
- `000B0000\tu00000002_00000000` is 181,317,632 bytes, matching the Xbox's 173 MB Destiny Title Update.
- `00000002` contains seven compatibility packages. All 11 files in the title folder were copied (15,049,613,312 bytes total).
- Source manifest: `C:\Users\lukem\Destiny360\HASHES\hdd_destiny_source_2026-09-23.csv`.
- Archive manifest: `C:\Users\lukem\Destiny360\HASHES\hdd_destiny_archive_2026-09-23.csv`.
- Verification: 11 source and 11 destination SHA-256 entries; zero path, size, or hash differences. Both manifest files have SHA-256 `0E54E9E3EBF9A0374778FDAAD5E81DA1AC76E9A01088A4BEAAC8EA1217E419F2`.
- The Xbox drive was unmounted, the PC was shut down, and `D:` was reconnected. The complete capture was copied to `D:\Destiny360\00_ORIGINAL_DUMP\HDD_CAPTURE_2026-09-23` on 2026-09-23. Both original manifests were copied to `D:\Destiny360\HASHES`.
- D: archive manifest: `D:\Destiny360\HASHES\hdd_destiny_d_archive_2026-09-23.csv`. It contains the same 11 paths, 15,049,613,312 bytes, and SHA-256 hashes as the Xbox source and C: copy. All three manifest files on `D:` have identical SHA-256 `0E54E9E3EBF9A0374778FDAAD5E81DA1AC76E9A01088A4BEAAC8EA1217E419F2`.
- The verified C: copy remains available as a second copy.

## Appendix: original capture procedure and first Xenia experiment (moved from the README, 2026-09-26)

### Capture procedure

1. On the Xbox 360, confirm Destiny launches from the physical disc and record the version shown in game, if available. Bungie identifies Update 2.3.1 as the final Xbox 360 branch.
2. In **Settings → System → Storage → Games and Apps → Destiny**, write down every Destiny item, its type, size, and storage device. Keep the console's existing installation intact.
3. Prepare the USB in Xbox 360 Storage. Formatting erases its current contents.
4. Use the Xbox storage **Copy** action to put the installed game and each Destiny update/add-on package on the USB. Check the USB in Xbox Storage before removing it, and confirm the original content is still on the console.
5. On Windows, copy the USB's contents to a separate archive outside this repository, including hidden files and any `$TitleUpdate` folder. If the disc installation appears as one main file plus a matching `.data` folder, preserve both together. Do not rename files inside the archive. Record the actual paths found; content may not all sit under one Destiny folder.
6. Generate a SHA-256 manifest for the archive using `scripts/New-Manifest.ps1`. Keep the manifest and a second copy of the archive on another drive if possible.

Do not put game dumps, profile data, update packages, or logs containing account details in Git. `.gitignore` excludes local capture directories as an extra guard.

### First Xenia experiment

After the archive is complete, make a working copy and open its game container in the pinned Xenia Canary build under `D:\Destiny360\XENIA`. Use `scripts/Start-XeniaTest.ps1` to keep a separate log and config snapshot for each run. Record the visible result in `notes/test-log.md`.

The installed build (`74c4e4acb`, 2026-09-21) generated `xenia-canary.config.toml` and already has `mount_cache = true`. Keep that initial setting for Test 001. Only compare with `false` later if the log gives a reason. The [existing Destiny compatibility report](https://github.com/xenia-project/game-compatibility/issues/269) marks cache mounting as required and records a title screen and Xbox Live sign-in blocker; use it as prior evidence, not as a prediction for the current build.

### Prepared PC environment

- Windows 11 Pro, Ryzen 5 7600X, NVIDIA RTX 5070, about 32 GB RAM.
- Official Xenia Canary `74c4e4a` is in `D:\Destiny360\XENIA`; the original downloaded archive is in `D:\Destiny360\TOOLS`.
- `D:` had about 173 GB free before setup. Confirm free space again before making multiple full copies.
- The pinned archive and executable hashes are recorded in `notes/environment.md`.

### References

- [Xenia Canary Quickstart](https://github.com/xenia-canary/xenia-canary/wiki/Quickstart)
- [Xenia Canary FAQ](https://github.com/xenia-canary/xenia-canary/wiki/FAQ)
- [Bungie Xbox 360 installation guide](https://help.bungie.net/hc/en-us/articles/360048720032-Installing-Reinstalling-and-Updating-Destiny-on-the-Xbox-360-Legacy-Console)
- [Bungie Destiny 1 legacy support](https://help.bungie.net/hc/en-us/articles/360049024352-Legacy-Support-Guide-for-Destiny-1)
- [Candidate Destiny data tools](notes/research-tools.md)
