# Local Xenia content importer build

The source checkout at `D:\Destiny360\XENIA_BUILDS\xenia-canary-src` is pinned to Xenia Canary commit `74c4e4acbad21bb16c2266fcff7d6420c6a8fc19` on local branch `codex/destiny-content1-stfs`. The portable changes are in `patches/xenia-content1-stfs.patch`. They apply to the STFS reader and virtual file extractor only. No game binaries or content files are in this repository.

The patch fixes zero stored filename lengths, honors the contiguous-file flag, selects allocated entries in redundant hash-table copies when only one copy is allocated, and returns an error if extraction reads no bytes. A temporary test-only installer entry point was removed before the final build. The patch was checked against the source checkout with `git apply --reverse --check`; the final build completed successfully.

The final executable is `D:\Destiny360\XENIA_TEST009_FULL_CONTENT\xenia_canary.exe` (17,136,640 bytes, SHA-256 `B0664D0952AD2756BA98D0DDD1A76E2AACC1D1C51FBB7503F792E503FACAFA66`). The source patch is 5,304 bytes, SHA-256 `C232A5FD9E3C44F56DEA408A17BA98096E917F1E3209CAD39A24A73390EB9747`. The build log is `D:\Destiny360\LOGS\Xenia-patched-final-build.txt`.

The build used Visual Studio 2026 C++ Build Tools, CMake, and LunarG Vulkan SDK 1.4.357.0 installed in copy-only mode at `D:\Destiny360\TOOLS\VulkanSDK-1.4.357.0`. The SDK's official installer and verified hash are in `D:\Destiny360\TOOLS`; no system-wide SDK path was required. `build-vs18` is the local CMake build tree and is intentionally excluded from the portable patch.

Validation: the normal Xenia content installer returned success for all three captured content packages in isolated tests (430 extracted files total). See `notes/content-analysis.md` and `notes/test-log.md`. The game still stops at canary with all three content files present.
