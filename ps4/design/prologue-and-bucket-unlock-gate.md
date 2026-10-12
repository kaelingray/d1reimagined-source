# The prologue substage gate and the bucket-unlock check - a frontier note

**State (2026-08-11).** Banked from a decompiled-corpus crawl (a subagent, 2026-08-11) that went
looking for "the other piece" of the `character:prologue` verification state - the input the
prologue handler consults before it decides whether to advance or walk itself back to orbit. The
crawl found a coherent mechanism and, in passing, a **contradiction** with `eboot-map.md` §2.6.

⚠ **Reach: `static-client-inference`, NOT verified live.** Every address below is read out of the
shipped binary or the decompiled corpus. The crawl's own second-argument check (U2 below) was left
**one `d1-eboot disasm` command short of closure** when it ended, so the load-bearing pointer math
is `[I]`, not `[C]`. This is a prediction with a pedigree, not a result. It inherits
`synthetic_ancestor` from everything upstream. **Confirm U1 before citing any of this as contract.**

The canonical unlock machinery - the index space, the setter, the expression VM, the availability
plane and its readers, and the 205/503 wire object - lives in **`grants-and-unlocks.md`** (§2.1-2.6,
§4.1). This note does not restate it; it names the two *consumers* the crawl added and the servable
channel that already exists to feed them.

---

## 1. The two consumers, over one plane

Both mechanisms below read the **derived unlock availability plane** that
`grants-and-unlocks.md` §2.5 documents: `[[0x26A34B0 + 0x38] + 0xC]`, a `byte[0x2000]` table where
an owned flag reads `0x02`. Neither consumer reads the item estate; that is the crux of the
contradiction in §3.

### 1.1 The bucket-unlock check `[I]`

`cui::c_inventory_bucket_data_component::vfunc[14]` at **`0x9EB2B0`** decides whether an inventory
bucket renders locked. Its inputs:

* **`bucket_def + 0x38`** - a `u16` unlock-flag index. `0xFFFF` (`-1`) means **ungated**; any other
  value is looked up in the availability plane, and the bucket is locked unless the plane byte is
  `owned == 2`.
* **`bucket_def + 0x3C / +0x40`** - the locked-state string (self-relative), shown when the bucket
  is gated and the flag is not owned.

⛔ **The item estate is NOT an input.** A bucket's lock state is a function of one unlock-flag index
and the plane, not of what items sit in the bucket, not of family 4/3 (inventory), and not of family
4/4. This is the direct refutation of the item-estate theory (§3).

### 1.2 The prologue substage selector `FUN_002646D0` `[I]`

The prologue handler's index comes from **`FUN_002646D0`** (`0x2646D0`), which walks a **content
substage table** (stride `0x18`) of **unlock expressions**, evaluating each against the same
availability plane through the expression VM (`grants-and-unlocks.md` §2.4). It returns the index of
the first substage whose expression is satisfied.

* On **exhaustion** (no row's expression is satisfied) it returns **`0xFFFF`**.
* `0xFFFF` drives the handler to **substage 5**, which is the **only branch that requests a state
  transition** - and that transition falls through `cleanup` to `orbit`. So an unserved prologue
  reads as "walked itself back to orbit", exactly the `character:prologue -> cleanup -> orbit`
  shape.

The consequence is the useful one: **advancing the prologue is a matter of making one substage row's
unlock expression evaluate true**, i.e. of owning the right flag on the plane - which the server can
do. It is not a matter of shipping an object we have never sent.

## 2. The servable channel already exists

Both consumers read the plane, and the plane is fed by the **205/503 unlock object `0x80801941`**
(`grants-and-unlocks.md` §4.1), which carries a **sparse flag array**:

* `+0x054` - `u32` count.
* `+0x058` - up to **50 x `{u16 index; u8 value}`**, stride `4`. `value == 2` marks a flag owned.

So the fix for either consumer is a flag on the wire, in the object the server already builds and the
grant pipeline already proves end to end (`grants-and-unlocks.md` head: run
`20260809-075545-RUNG33-UNLOCK-98`). No new format, no new object - **find the flag index each
consumer wants and add it to the sparse array.**

## 3. The contradiction with `eboot-map.md` §2.6 `[U]`

`eboot-map.md` §2.6 (lines ~294-296) reads the prologue index as coming from
`0x574220(4) -> 0x2646D0`, where **`0x574220` looks up family 4 / type 4** - "an object we have
never sent" - and concludes the index stays `0xFFFF` because that object is absent.

This crawl reads the SAME `0x2646D0` as an **unlock-expression walk over the plane**, with no family
4/4 dependency. Both readings are static and neither is live-verified (§2.6 itself is tagged `[U]`,
"no run has reached state 24 yet"). They cannot both be the whole story:

* If §2.6 is right, the prologue is gated on **an object** (family 4/4) and the fix is to ship it.
* If this crawl is right, the prologue is gated on **a flag on the plane** and the fix is a sparse
  array entry in an object we already send.

⚠ **Do not treat either as settled.** The item-estate/family-4/4 reading is **contested**, not
retracted - a pointer in `eboot-map.md` §2.6 now says so. The experiment that decides it is U1.

## 4. What to do next - the deciding experiments `[U]`

* **U1 (decides §3).** Read `bucket_def + 0x38` for the three ammo buckets - hashes **1498876634**
  (Primary), **2465295065** (Special), **953998645** (Heavy) - by replaying the resolver on a
  rung-36 snapshot: `d1-replay replay runs/snapshots/rung36-in-world-moonbase.snap --func 0x548440`,
  which reaches `0x5492b0`; walk the bucket-def table at stride `0x48` and read `+0x38`. A real
  `u16` flag index there (not `0xFFFF`) confirms the bucket-unlock check consults the plane, and by
  extension supports the plane reading of the prologue selector. ⚠ `guardian-in-world-moonbase.snap`
  is **uncitable** (its `d1-replay controls` exits 2); use a `rung36-*` snapshot.
* **U2 (left one command from closure).** Confirm `0x28ca90`'s second argument is `rec + 0x38`
  (i.e. that the bucket-unlock check passes the flag index, not something else, into the plane
  lookup): `d1-eboot disasm 0x9eb2b0` and read the call setup.
* **U4.** Does the substage table `FUN_002646D0` walks carry a **default row** with an empty
  (always-true) expression? If it does, an unserved prologue selects that row and advances; if it
  does not, exhaustion is `0xFFFF -> substage 5 -> orbit` and a flag must be served. This decides
  whether "the prologue exits in one tick" means "no row applied" or "a NULL content context".

## 5. Cross-references

* Machinery (plane, VM, setter, 205/503 object): `grants-and-unlocks.md` §2.4-2.6, §4.1.
* The contested family-4/4 reading: `eboot-map.md` §2.6.
* The geared estate that was wrongly suspected as the prologue input: `items-and-gear.md` §6,
  `geared-visuals-run-1.md` §5b.
* The in-world spine that this gate sits just before: `GUARDIAN-IN-WORLD.md`.

## 6. Dead claims

* **"The prologue is gated on the geared item estate / family 4/3 inventory."** Killed by §1.1: the
  bucket-unlock check reads one `u16` flag index and the plane, never the items. The item estate
  being resident does nothing for the prologue.
* **"Family 4/4 as the prologue gate is settled `[C]`."** Downgraded to **contested `[U]`** by §3.
  It may still be correct; it is no longer unopposed, and it was never live-verified.
