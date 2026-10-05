# Individual inventory items in 01.24

Character inventory summaries and equipped IDs do not supply the client's
individual item records. The self family also has an item-object pool. The
earlier starter fixture populated the summaries and equipped references but
left this pool empty. JOIN119 showed all seven starter entries under Materials
and loading equipment thumbnails; those observations prompted this trace.

## Wire and native lookup

In BLES01857 01.24, self family 4, kind 3 is `0x80801732`, size **0xa8**,
with object fingerprint **0x0eb6ac38**. The runtime type reference at
`0x18eb6e4` resolves to that handle and its descriptor at `0x115b2c0`.
Its fingerprint is descriptor +4. Do not confuse the hash of a static metadata
entry with the live type it resolves to.

The item lookup adapter `0x482aa0` selects kind 3 through `0x7fce0` and
calls `0x47e3cc` with the adapter's family and the instance ID. The self
equipment conversion `0x46f12c` uses this lookup context through `0xc57b98`.
The cache builder `0xc347e8` and statistics builder `0xc50910` also resolve
equipped IDs to these records. The investment item index is record +0x24.

The minimal record currently supplied contains:

| Offset | Value |
| --- | --- |
| +0 | Item instance ID, matching the object header and equipped reference |
| +8 | Owner structure: character ID at +0, owner kind 0 at +8 |
| +0x18 | Quantity structure: quantity at +0 |
| +0x24 | Investment item index |

These are wire-object offsets, separate from the character inventory-entry
layout. Talent grids, perks, rolled statistics, dyes and other full item
attributes are not yet reconstructed by this change.
This restores the CLI starter fixture, not database-backed loot/equip operations.
Deleting an instance from an existing subscription will also require the verified
qz removal path; omission from a delta alone is not an item-deletion protocol.

`character_item_objects()` derives records from the character's **merged**
inventory, after equipment and raw profile overlays. It scans the sparse entries,
excludes zero-instance entries and rejects duplicate instance IDs.
The family builder includes these objects alongside the account and character
on both snapshots and deltas. A schema without a verified item fingerprint
retains the existing payload instead of guessing a type hash.

## Evidence

JOIN121 repeats the existing public starter fixture before this server change.
The live self-family descriptors confirm kind 3's handle, stride and capacity
of 646 records; every record's instance ID is zero. This is direct evidence of
the missing objects, independent of the UI symptoms.

JOIN122 receives seven objects in its real encrypted self-family update.
At 16:52 BST, native memory contains instances 0x100–0x105 and 0x10b with
indexes 710, 204, 198, 200, 206, 202 and 862, quantity one and the expected
character owner. The client accepts them and reaches `activity:in_world`.
UI categorization and thumbnails require a separate visual check; receipt in
the pool alone is not proof that they are fixed. The 16:57 character/inventory
views retain loading equipped icons and the Materials listing. The cache adapter
at `0x1ab6250` points to an initialized, valid cache: cache +8/+9 are both one.
At about 17:00, the RSX thread reports Vulkan device loss. No additional menu
pass is inferred from the frozen native state after that error.

## Fixed inventory ranges

JOIN122's live client item definitions give the correct default bucket at +0x40:
subclass 6, armour 7 through 11, and Ghost 14. The definitions themselves do not
classify the armour as Materials. `0xc3dc90` reads the bucket's start and capacity
and scans those fixed positions in the self inventory, independent of its +0.

The investment-globals root's +0x84 handle resolves to the bucket-range resource.
Getter `0x1cf92c` returns resource +4; count is +0x84 and rows start +0x88 with
stride 0x14. Row +0 is a **byte** bucket index, +4 is start, +8 is capacity,
and +0x10 is owner kind (zero character, one account).

The root's +0xdc handle resolves to the equipped-slot table, returned by
`0x1d0250` as resource +4. It contains 17 rows with stride 0x20; each row's
+0xd is its bucket. Together these tables establish:

| Equipped slot | Bucket | Inventory start |
| --- | --- | --- |
| 0, subclass | 6 | 40 |
| 1, helmet | 7 | 43 |
| 2, gauntlets | 8 | 53 |
| 3, chest | 9 | 63 |
| 4, legs | 10 | 73 |
| 5, class item | 11 | 83 |
| 6 / 7 / 8, weapons | 2 / 3 / 4 | 93 / 103 / 113 |
| 9 / 10 / 11 | 12 / 13 / 14 | 123 / 143 / 153 |
| 12 / 13 / 14 / 15 / 16 | 25 / 27 / 28 / 29 / 35 | 210 / 220 / 230 / 240 / 268 |

The earlier fixture packed all seven entries at positions 0 through 6, inside
bucket 5's range 0–19. That placement explains their appearance on the Materials
tab. The verified final layout now puts each equipped item at its bucket start.
Older/unknown layouts retain their existing behavior; final slots without a
verified bucket are rejected instead of placing them in another category.

Inventory +0 is a **mutation serial**, not item count. Native updater `0x109ccac`
scans the supplied bucket range, writes the old serial to entry +0x14 and
increments inventory +0 when changing or adding an entry. The optional fixed
array in `0x80804a18` has 320 entries, serialized independently of that serial.
Therefore the individual-object builder must scan populated positions even when
the serial is zero or smaller than their positions.

JOIN123 repeats the public fixture with both changes and no raw overlay.
At 17:17 BST, native memory has all seven records and inventory positions
40, 43, 53, 63, 73, 83 and 153, with the same equipped IDs. At 17:19:23,
current slice 112/engine zero retains the five correct armour arrangements,
valid applied roots and a physical Guardian unit.

The 17:20:41 character capture shows the subclass, Ghost and all five armour
icons, alongside a normally proportioned Titan preview. The 17:21:21 inventory
capture shows **Materials 0/20**, with no armour entries there. This accepts
categorization and the populated equipped thumbnails in the captured view.
Unset weapon slots still show loading indicators, and level/light remain zero.
The subclass tutorial now recognizes the equipped subclass; an upgrade has not
been verified.

At approximately 17:21:25, the RSX thread again reports `VK_ERROR_DEVICE_LOST`.
Malformed live fragment programs with the previously recorded float-looking
words precede the crash. The subsequent screenshot attempt fails. The frozen
state cannot prove return-to-world behavior or combined menu/world stability.
Its recorder was not stopped before the screenshot failure interrupted the
input sequence; the resulting MP4 lacks its `moov` metadata and is unusable.
The screenshots and native log remain the evidence for this inventory pass.
JOIN122's separate native recording fully decodes and preserves the earlier
broken Materials view.

Both owned test stacks and their outer launchers are verified exited, and
XInput is restored. No renderer configuration or shader patch was changed.

The regression tests decode the full family wire payload, check every starter
instance and its fields, verify fixed bucket positions and matching equipped IDs,
verify the delta path, exercise sparse merged overrides with serial zero and
zero-instance entries, and reject duplicate identities. The actual final-build
preflight also roundtrips the individual records.

Keep the SPU colour-read patch, Read Color Buffers and the existing
[world-armour arrangements](guardian-appearance-0124.md). This server change
does not alter the renderer or establish a fix for Vulkan device loss.

## Repeat views and usable footage

JOIN124 repeats the repaired public fixture before the new shader parser patch.
Its 17:35:35 native recording shows solid outdoor armour, resolved equipment
icons and Materials 0/20, and is finalized before the next tab return. At
17:38:35 the world emote again shows solid armour after the menu. The native
log has no fatal entry through native time 13:16.760, but malformed programs and
program growth remain. A brief successful return does not settle the crash.

JOIN125 repeats these views with the [NOP-literal parser correction](rpcs3-fragment-start-literals.md),
including further tab cycles and a fresh world/movement view. Its own malformed
programs persist; the two changes must not be conflated.

The reviewed private before/after clips are `join122-inventory-before.mp4`
(18 seconds) and `join124-inventory-after.mp4` (17.77 seconds), under
`APPEARANCE/2026-10-05`. They retain real rendered motion and native game audio,
export as H.264/AAC and pass full decoding. JOIN125's bounded 24.50-second native
round-trip recording also fully decodes. See the [Episode 02 draft](devlogs/02-2026-10-05-inventory.md)
for the milestone and honest remaining limits. No binary media is committed.
