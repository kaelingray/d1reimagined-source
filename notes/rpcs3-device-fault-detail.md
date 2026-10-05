# Vulkan device-loss evidence

JOIN128, JOIN129 and JOIN130 all report a GPU address fault at `0x0` with
`access_write`, despite the depth candidate eliminating the previously observed
floating-point fragment source in JOIN129. The reporting CPU thread varies between
PPU, RSX and SPU; that is where the asynchronous device loss surfaces, not proof of
which GPU command caused it.

The original RPCS3 fault report ignores both fault-query return values, prints
every allocated record, and omits address precision. The reported address cannot
be interpreted as an exact null pointer from this output alone. Vulkan defines
a precision value and a corresponding possible address range, with the reported
address serving as a hint within that range. See the primary
[address information specification](https://docs.vulkan.org/refpages/latest/refpages/source/VkDeviceFaultAddressInfoKHR.html).

`patches/rpcs3-device-fault-detail.patch` adds diagnostic detail without changing
draws, shaders, device-loss severity or recovery:

- Reject failed count/detail queries with their `VkResult`, rather than printing
  unfilled output as evidence. Handle a partial detail result explicitly.
- Limit records to the returned number written, bounded by the allocated capacity.
- Log query result, incomplete status and each address precision.
- Calculate the specification's possible range for a nonzero power-of-two precision;
  label invalid precision as an unknown range.

The query contract and partial-return behavior follow
[vkGetDeviceFaultInfoEXT](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetDeviceFaultInfoEXT.html).
The patch is an isolated increment against the private renderer checkout; reverse
application checks pass. It does not diagnose the originating resource by itself
and is not a graphics fix. No raw fault logs or game assets enter Git.

The full private Release build completes and is deployed with matching SHA-256
`e2dd5ad46a008da4bf439e4e5f0afc3dec4af76e0c6f18f1a4d80c28ea906d5a`.
The prior executable is backed up, the depth experiment is disabled, and all owned
test stacks are exited. Existing linker/post-build warnings remain. Native fault
report execution was outstanding at deployment; compilation alone does not validate
a driver failure report.

JOIN132 now exercises it at native **3:01.701927**: the detail query succeeds
(`VkResult=0`, `incomplete=0`) and reports `access_write`, address `0x0`, precision
`0x1000`, possible range **`0x0–0xfff`**. This confirms a near-zero GPU write report
without identifying the originating command. The failed run has no accepted world
or menu interaction. Ordered cleanup is complete and the depth marker is disabled.
The subsequent [presentation ownership repair](rpcs3-present-capture-lifetime.md)
addresses a concrete media-sync lifetime defect found during this investigation;
native causality and complete graphics acceptance remain unproven.

JOIN135 subsequently loads privately scoped core validation and reports device loss
in `0xd06922000–0xd06922fff` without a preceding VUID. JOIN136 enables GPU-assisted
validation and logs descriptor and depth-layout mismatches without pre-shutdown
device loss. The longer instrumented runtime is not a repair. Its final GDB socket
fatal belongs to controlled debugger teardown. See the
[descriptor binding investigation](rpcs3-descriptor-binding-cache.md).

Next: retest the isolated descriptor cache repair, then trace the remaining depth
feedback-loop transitions and originating GPU commands.
Keep recording-enabled and recording-disabled controls separate, and preserve
world/body/menu acceptance checks independently of absence of malformed shaders.
