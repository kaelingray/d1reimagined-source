# Descriptor allocation cache retirement — 2026-10-07

`descriptor_pool::allocate()` batches sets for one layout. When another layout is
requested, or caching is explicitly disabled, it allocates directly without using
the remaining batch. Those allocations can exhaust the pool and invoke
`next_subpool()` while the batch still contains handles from that pool. A later
request for the cached layout then returns an old handle. The asynchronous reset
invalidates it. Vulkan resets implicitly free all allocated sets
([reset requirements](https://docs.vulkan.org/refpages/latest/refpages/source/vkResetDescriptorPool.html)).

[rpcs3-descriptor-allocation-cache.patch](../patches/rpcs3-descriptor-allocation-cache.patch)
drops unissued cached handles on the allocating thread before retiring a subpool.
The asynchronous reset callback does not touch the cache, which may already
belong to the new pool. Batch allocation, pool reuse, autoscaling and the existing
callback-owner retention remain in place. The cached layout stays available to
the allocation that triggered a rollover.

[rpcs3_descriptor_allocation_audit.py](../scripts/rpcs3_descriptor_allocation_audit.py)
extracts the complete actual pool class and its allocation, creation, autoscaling,
switch and reset methods into C++20 fixtures. Fake Vulkan sets record their pool,
layout and reset generation. The original implementation fails **5 of 20**
checks; the repaired implementation passes **all 20**. Checks cover normal batch
reuse, mixed layouts, uncached exhaustion, delayed resets, single-layout rollover,
reclaimed pools and growth while old pools are still busy. A fixture was corrected
during development: immediate completion permits small-pool reuse, so it cannot
require growth. The final growth fixture retains pending callbacks until the end.

The existing retirement audit now extracts just its actual callback branch and
supplies mocks for the later diagnostic additions. All **23 retirement checks**
pass against the current renderer. Neither audit executes Vulkan or proves pixels.

```powershell
python scripts/rpcs3_descriptor_allocation_audit.py --source <private-rpcs3>/rpcs3/Emu/RSX/VK/vkutils --out <new-audit-directory>
# In that directory, with MSVC configured:
cl /nologo /std:c++20 /EHsc /W4 descriptor-allocation-audit.cpp /Fe:descriptor-allocation-audit.exe
.\descriptor-allocation-audit.exe
```

The patch adds five lines to one file against the exact pre-change private tree.
An isolated fixture applies it and reproduces the tested source bytes. Its SHA-256
is `1f74dc7698b4b015050ba9dee87e61acbe571221403a5035e9ed5f349549df60`.
Earlier renderer changes and unrelated private modifications are excluded.

**Native cause remains unproved.** The inspected graphics descriptor tables each
own a pool and allocate a single layout. That ordinary path does not establish
mixed-layout or uncached spill. The cache defect is demonstrated by the allocator
contract, but it must not be presented as the cause of the observed reset errors,
flicker or character distortion. Native build/deployment results are recorded in
the [October 7 checkpoint](checkpoint-2026-10-07-renderer.md).
