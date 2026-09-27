# Draft RPCS3 report: SPU DMA writes to video memory missed by the surface cache

Status: **draft, not filed.** Publishing it needs the user's go-ahead. It would be an issue
and possibly a pull request against RPCS3/rpcs3, based on
[`patches/rpcs3-destiny-fix.patch`](../patches/rpcs3-destiny-fix.patch).

## Title

Surface cache misses SPU DMA writes to local memory (Destiny: black blocks and overexposure after moving)

## Environment

- RPCS3 source `b5a174e9` (v0.0.42-20043), Windows 11, Vulkan renderer.
- Game: Destiny (BLES01857 01.00).
- Settings: Write Color Buffers, Read Color Buffers, Write Depth Buffer, Read Depth Buffer and
  Handle RSX Memory Tiling enabled. Without the read/write options the frame is broken from the
  start.

## Symptom

In gameplay the frame shows fixed black or magenta rectangles when the player moves, for
example x 0-640 / y 312-540 of the 1024x624 internal frame, and the rest of the image
overexposes. It recovers after standing still for about a minute. Strict rendering mode, strict
texture flushing, the FIFO accuracy settings and Force CPU Blit do not help.

## Cause

- Destiny's SPU lighting jobs write the HDR light buffer (0xCEB60000, 1024x624 RGBA16F,
  pitch 8192) directly in local memory via SPU DMA puts. The RSX then adds its own lights on
  top (additive draws), and a later pass samples the buffer.
- At the end of each frame the game reuses the same memory for HUD targets: 0xCEB60000 as
  1280x127 ARGB8, and 0xCEDD0000 (row 312 of the light buffer) as 1280x228 ARGB8, both at pitch
  8192. Blits copy the top of the frame into the first target.
- When the light buffer is bound again, the surface cache rebuilds it from the overlapping
  cached surfaces (the HUD targets and the split-off pieces of the old light buffer).
  `render_target::memory_barrier` only reloads a source from guest memory if its `test()`
  fails. `test()` compares three 8-byte samples (top-left, centre, bottom-right).
- Dark interiors are mostly zero before and after the SPU write, so the samples match and stale
  surface data replaces the SPU's lighting. The HUD rectangles come out black, and the rest
  accumulates onto the previous frame, which gives the overexposure.

Evidence: a one-frame dump of every render target showed the zeros already in the first
light-pass output. Guest memory under the same rectangle held the SPU's values at that time.

## Fix (tested)

- New file `rsx/Common/local_mem_writes.h` keeps a write generation per 4 KB page of local
  memory (0xC0000000-0xCFFFFFFF).
- `spu_thread::do_dma_transfer` bumps the pages of each put into video memory. It does so
  after the write, through a scope guard. The inline put path of `do_list_transfer` does the
  same.
- `render_target_descriptor::sync_tag` stores the sum of the generations over the surface's
  pages, and `test()` fails when that sum has changed. The existing barrier logic then skips
  the stale source and reloads from memory.
- Result: two full movement passes with 14/14 frames clean. More samples alone (33 instead of
  3) improved things but still failed a few frames.

The same patch also makes NV3089 blits from local memory to main memory
(`dst.dma == HOST_BUFFER && src.dma == FRAME_BUFFER`) take the CPU path. The G-buffer copies the
SPUs read must be in guest memory immediately. TEST250 showed it is not needed once the write
tracking is in place (14/14 clean frames without it), so it is no longer part of the fix patch.

## Open questions for maintainers

- PPU stores to unprotected video memory are not tracked either. Should the generation counter
  also cover other writers?
- The per-page sum costs about 1250 atomic loads per 5 MB surface for each `test()` or
  `sync_tag`. A coarser granularity or a dirty bitmap might be preferable.

---

# Draft 2: per-descriptor-set storage pools exhaust memory (Destiny, ~14 GB/min)

Status: **draft, not filed.** Tested in TEST246-249.

## Symptom

In Destiny's first mission, private commit grows by about 14-15 GB per minute. Working set
grows only about 0.6 GB per minute. The process dies silently, or with a
`simple_array<VkDescriptorImageInfo>::reserve` verification failure, about 7-12 minutes into
gameplay. This happens with official build v0.0.42-20043 on Windows with an RTX 5070. Lowering
the VRAM allocation limit does not help.

## Analysis

- RPCS3's own Vulkan pools, texture cache and surface cache stay flat at about 500 MB.
- The growth is new 16 MB segments of one NT heap: about 7 per second, mostly free blocks
  between live blocks of 0x60600 bytes.
- 0x60600 bytes is 16448 × 24, the size of `vk::descriptor_set::m_image_info_pool` and
  `m_buffer_info_pool` as reserved in `descriptor_set::init()` on first use
  (`m_pool_size = max_cache_size + max_overflow_size`). With `m_buffer_view_pool` that is about
  0.9 MB per descriptor set.
- The game has about 185k pipelines/programs (created at boot, none destroyed). During gameplay
  about 270 descriptor sets per second are initialized for the first time (counted), which gives
  270 × 0.9 MB ≈ 14 GB per minute.

## Fix (candidate)

The storage pools must not reallocate before a flush, because pending writes point into them.
Keep that guarantee, but with a small threshold: reserve `max_storage_size (256) +
max_overflow_size (64)` entries and let `storage_cache_pressure()` flush at bind once 256 entries
are used. `max_cache_size` stays 16384 for the pending-write list, which holds no external
pointers. This relies on the same assumption as before: at most 64 entries are pushed between
binds.

Result: growth went from about 14.5 GB/min to about 1.5 GB/min (TEST246). The remainder is the
per-table `VkDescriptorPool`: every program's descriptor table keeps its own pool, about 90 KB
of driver and storage memory, for the whole session.

## Fix 2 (tested)

`vk::glsl::release_idle_descriptor_tables()` runs once per presented frame. Every 64 frames it
releases the descriptor set storage and the pool of tables that have not been committed for 256
frames. The next `commit()` allocates a new set and rewrites all descriptors from the template.
TEST249: private commit stays flat (12.9 GB) during a 30-minute soak, with tens of thousands of
tables released.

Open question: a shared pool (with set revalidation) might be the cleaner design. The idle
release keeps the existing per-table ownership model.

