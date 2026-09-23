# Destiny Content 1 container analysis

Date: 2026-09-23. This investigates Xenia's crash when importing the Xbox 360 `Destiny Content 1` CON package. All analysis used a working copy of the verified HDD capture. Original game data stays outside this repository.

## Observations

- The HDD source and both archives contain byte-identical copies of all 11 Destiny files (SHA-256 verified). The import failure alone does not indicate a bad copy.
- `Destiny Content 1` has a readable STFS directory table beginning at offset `0xDD000`. Entry names are present in the 40-byte name fields, but the six-bit stored name lengths are all zero. For example, `c0000359` and `packages` have flags byte `0x80`, and `360_investment_globals_012c_7.pkg` has `0x40`.
- The equivalent entries in `Destiny Content 2`, `Destiny Content 3`, and Compatibility Pack 13 have nonzero stored name lengths.
- In Xenia Canary commit `74c4e4acbad21bb16c2266fcff7d6420c6a8fc19`, `stfs_container_device.cc` uses the stored name length without a fallback, giving these Content 1 entries empty names. For each file it also follows hash-table block links, even when the STFS directory marks the file contiguous. During Test 002, Xenia reported truncated files and block-chain failures before the import crashed.

The first parser change exposed three noncontiguous files whose block links appeared to end early. Xenia's importer then repeatedly read zero bytes from the truncated entries and stalled.

## Independent read-only check

`Extract-STFS` at commit `3483c68e6edd35dc94636429deff3c45efdfad70` initially listed zero Content 1 entries because it also trusted the zero name lengths. A local working copy of its parser was changed to derive the length from NUL-terminated ASCII only when the stored length is zero. It then listed **only the first directory-table block**: 64 entries (58 files) with safe relative paths. It extracted 12,493,048 bytes from that first block to `D:\Destiny360\02_EXTRACTED\Content1_WORKAROUND`. This is a partial extraction, not a complete Content 1 inventory.

The list and extraction transcripts are `D:\Destiny360\LOGS\Content1-STFS-workaround-list.txt` and `D:\Destiny360\LOGS\Content1-STFS-workaround-extract.txt`. File count and output sizes match that partial listing. Its files were not all independently checked against STFS hashes, so treat that first-block extraction as research samples.

The patched Xenia reader traversed all five directory-table blocks and identified 275 files. Its first version warned that three noncontiguous files had incomplete chains; see `D:\Destiny360\LOGS\Content1-Xenia-patched-vfs-debug.txt`. Reading both redundant hash-table copies showed that the active record switches between copies within a table. Choosing the allocated record when only one copy is allocated recovered chains of 36/36, 39/39, and 1070/1070 blocks for those files. An independent read-only check found that each of these 1145 data blocks matches the SHA-1 in its selected hash record. The revised Xenia listing has no truncation warnings: `D:\Destiny360\LOGS\Content1-Xenia-patched-vfs-active-hash.txt`.

The normal Xenia content installer then imported Content 1 in a fresh isolated folder: 275 extracted files, 86,747,359 bytes, plus a 40,960-byte header copy. The three formerly truncated files have their expected full sizes. The installer also imported Content 2 (87 files, 4,045,557,760 bytes) and Content 3 (68 files, 1,869,400,064 bytes) into that isolated folder. The three install logs returned status `00000000`; see `notes/test-log.md`. These checks establish successful extraction, not that Destiny can use the files during gameplay.

The other candidate parser, `XBLA-Extract` at commit `bf45b921abac3048ef0733420c0550c57946ca45`, returned an impossible entry size for Content 1 and was not used to extract it. The unmodified `Extract-STFS` parser found plausible listings for Content 3, Compatibility Pack 13, and the title update, but errored on Content 2. No conclusion about Content 2 follows from that parser error.

## Xenia patch candidate

A local Xenia branch, `codex/destiny-content1-stfs`, adds a bounded NUL-terminated filename fallback, advances block numbers sequentially for files marked contiguous, selects the allocated record across redundant hash-table copies when they differ, and stops extraction on a failed or zero-byte read. The source checkout is `D:\Destiny360\XENIA_BUILDS\xenia-canary-src`; a portable source patch is at `patches/xenia-content1-stfs.patch`. A temporary diagnostic entry point used for automated import tests was removed from the final source patch. This fixes extraction of the three captured content files in the tested Canary commit. The distorted graphics and Destiny's network error remain separate blockers.
