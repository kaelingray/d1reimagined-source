# RPCS3 development environment

Current target: Destiny 1 PS3 BLES01857 01.24. See the
[October 6 handoff](HANDOFF-2026-10-06.md) for build identities and acceptance.

Native tests use a private patched RPCS3 tree based on commit
`b5a174e933a083389e2f851005e474ccb3e676d9`, Windows, MSVC x64 Release and Vulkan.
Core validation uses the Khronos validation layer; GPU-assisted shader
instrumentation is disabled for the current comparison. A completed CPU audit
or successful build does not establish correct character/world rendering.

`D1_LOCAL` selects the private runtime and `-DataRepo` / `D1_DATA_REPO` select
the private data checkout. Keep game dumps, updates, firmware, keys, recordings,
certificates and machine settings outside this source repository.

Use exact process identities from run manifests for input, inspection and
cleanup. Back up the deployed executable before an idle replacement. Restore
the original title/watch files, disable diagnostic controls and return to
XInput after each automated keyboard test. The companion capture worker must
remain enabled; recording failures must preserve sources without stopping the game.
