# Guardian armour models in 01.24

The transparent/refractive world Guardian has a profile-data cause: equipped
armour item indexes were present, but their separate model arrangement indexes
were absent. The client defaults the omitted signed field to -1. The inventory
preview resolves model art through another conversion path, so a correct preview
does not establish that the world character summary contains all required fields.

Keep the existing selective SPU colour-read patch and **Read Color Buffers**
enabled. This repair supplies missing profile fields; it does not change shaders,
the emulator binary, or that menu-flicker repair.

## Profile and native conversion

In the final schema, progression equipment starts at +0x1e8 with 20 slots of
0x30 bytes. Each `0x8080183c` slot carries instance ID at +0, investment item
index at +8, another index at +0xa, and **model arrangement index at +0xc**.
The latter is an optional signed 16-bit wire field. Peer progression starts at
+0x30; select progression starts at +0x68. Self inventory retains the same item
instances and equipped-slot references.

Final native function `0x46ed4c` reads the arrangement from the progression
slot, calls lookup `0x2a6474`, then uses `0x46e10c` to populate model references.
An omitted arrangement reaches that lookup as -1 and returns no model art.
The normalized character contains equipment at +0x5f0 and eight art handles
per slot at +0x178. World wrapper `0x720198`, builder `0x71f86c`, and armour
assembly `0x71f46c` consume this normalized data. The baseline produced
`0xffffffff` for all five applied armour-model roots.

Resolve the live player descriptor at global `0x1b29450` before reading it.
Use its reported data pointer, stride and count; the observed stride is 0x2930,
with normalized character at record +0x840 and applied appearance at +0x1510.
These native offsets are distinct from wire-object offsets.

## Verified starter arrangements

Each mapping was checked against the final client asset library, rooted at
client globals +0x144. Its item entries point to asset records containing a
`0x8080030a` array of class selector/arrangement pairs. These starter pieces use
the wildcard class selector -1. An arrangement index is not an item index or
an item hash.

| Class | Helmet item → model | Arms | Chest | Legs | Class item |
| --- | --- | --- | --- | --- | --- |
| Titan | 204 → 191 | 198 → 188 | 200 → 189 | 206 → 192 | 202 → 190 |
| Hunter | 16 → 8 | 10 → 5 | 12 → 6 | 18 → 9 | 14 → 7 |
| Warlock | 392 → 374 | 386 → 371 | 388 → 372 | 394 → 375 | 390 → 373 |

`--character-gear starter:01.24:CLASS` now supplies these selections to both
peer and select summaries. Subclass and Ghost fields retain their existing
behavior. The historical `starter:CLASS` fixture still uses 01.00 data.
Overrides choose a model by the replacement item; an unknown replacement
does not inherit the original piece's model.

Custom equipment can provide an explicitly verified arrangement in JSON:

```json
{"1": {"item": 204, "id": 257, "arrangement": 191, "dyes": []}}
```

Pass the file as `--character-gear @PATH`. The arrangement accepts integers
from -1 to 32767, including zero. Omitting it preserves the prior custom-gear
behavior. Arbitrary loot still needs build-specific model resolution; the
starter fixture does not establish a complete item catalogue.

## Evidence and limits

JOIN117 records the missing-field baseline. JOIN118 adds only those five
model selections in a private overlay: all five applied roots change from
`0xffffffff` to real handles, and the daylight emote at 12:59:28 BST shows
solid, normally proportioned Titan armour. Its 13:01:04 character preview
also has normal proportions. Equipment thumbnails still show loading
spinners, and the displayed level/light remain zero; those UI fields are
separate open work.

JOIN118 later terminates its RSX thread with `VK_ERROR_DEVICE_LOST` at
approximately 13:01:44 BST. That crash prevents accepting a later inventory-tab
check or continued mission stability from this run. Its successful model
capture precedes the crash.

JOIN119 uses the public `starter:01.24:titan` fixture with **no overlay**.
At 13:15:15, engine zero/current slice 112 has all five correct item/model
pairs and applied roots `80a6ec27`, `80a7264d`, `80a6bed2`, `80a6f551`,
`80a6f297`. The 13:15:37 emote capture visibly repeats solid Titan armour.
At 13:16:19 the inventory tab displays all seven starter entries, including
armour thumbnails, under **Materials**. This confirms the user's observation;
inventory category/equipment UI behavior still needs repair.

Returning from inventory to character reproduces `VK_ERROR_DEVICE_LOST` at
about 13:16:40. Both crash reports surface in Vulkan event waiting
(`vkutils/sync.cpp:611`); one surfaces on RSX, the other on a PPU job thread.
That reporting site does not identify the originating GPU fault. No renderer
change or crash suppression is shipped with this profile repair. Do not claim
complete combined menu/world stability from these successful armour views.

JOIN117, JOIN118 and JOIN119 native recordings fully decode without errors.
The private appearance directory contains trimmed moving footage for the
candidate and standard-fixture views; no game media or package data is committed.

JOIN120 repeats the public fixture, walks the recorded controller route to the
Wall, and returns `route_reached` at 13:28:48 BST. A host teleport follows that
arrival. The 13:29:00 and 13:30:14 native samples have current slice 24, engine
3, target 112, and the same five valid applied model roots. The new world unit
datum is `0x10fac63f`. The sampled indoor views show the lit Breach entrance;
two Down/emote attempts do not provide a third-person body view. **Indoor body
rendering remains unaccepted**, despite correct profile/model data. The existing
return transition remains unsettled; these samples do not establish why the
emote view is unavailable.

JOIN120 runs through traversal and these indoor samples without a recorded
Vulkan device-loss/fatal entry. It does not repeat the menu test. Its native
recording fully decodes, all eight owned stack identities and the private
observers/outer launcher are verified exited, and XInput is restored.

Regression coverage checks all three starter classes, replacement items,
custom signed indexes, and the actual optional-field wire encoding. Full
peer/select objects also roundtrip through the private final schema. These
checks establish serialization and package mappings; live acceptance remains
scoped to the captured class and views.
