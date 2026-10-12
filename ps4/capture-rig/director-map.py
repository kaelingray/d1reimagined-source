#!/usr/bin/env python3
"""director-map.py - extract the director/destination/activity/unlock graph from the archived
D1 world manifest.

Why this exists: the client's own reflection tables say the family-4/type-4 QueueZ object is
`{u64 key, array[200] of {u64 id, u6 value}, array[32] of {u8,u64,u16,u16}}` - a sparse keyed
id -> small-state set. The manifest is where the ids and the states are *named*:
`DestinyDirectorBookDefinition` nodes carry `states[].state` (small ints), and every
`DestinyActivityDefinition` carries a `completionFlagHash` into `DestinyUnlockFlagDefinition`.

Outputs (written next to this script, under schemas/):
    director-places.tsv          11 places
    director-destinations.tsv    24 destinations
    director-books.tsv           9 books, one row per NODE
    director-activities.tsv      653 activities
    director-unlock-flags.tsv    7731 unlock flags
    director-map.json            everything, joined

Controls run on every invocation; exit 2 if any fails. A count that changes is not automatically
wrong, but it means the manifest changed and every downstream reading must be re-checked.

⚠ REACH: `static-client-inference` at best, and the source is NOT our client - it is Bungie's
archived public manifest (a 2020 capture; our target is 01.29 / Oct 2016). Hashes are *believed*
stable across D1's life; that is believed, not measured. Nothing here has been confirmed against
the 01.29 client. See historicaldocs/design/loadout-framework.md §5 for how badly era inference can mislead.
"""

import json
import os
import sqlite3
import sys
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
OUT = os.path.join(HERE, "schemas")

MANIFEST = os.path.join(
    REPO,
    "manifest/data/world_en_world_sql_content_39adcbfdc2021172e8ccf4720ca76023.content",
)

# Pinned expectations. Each is a positive control: it asserts we are reading the manifest we
# think we are reading, in the way we think we are reading it.
CONTROLS = {
    "DestinyPlaceDefinition": 11,
    "DestinyDestinationDefinition": 24,
    "DestinyDirectorBookDefinition": 9,
    "DestinyActivityDefinition": 653,
    "DestinyActivityBundleDefinition": 310,
    "DestinyUnlockFlagDefinition": 7731,
}
# Two named rows, both directions: one that must resolve, one that must NOT be found by a
# plausible-but-wrong lookup. See loadout-framework.md's "an item is a hash, never a name".
CONTROL_DESTINATION = (2468353467, "Old Russia, Earth")
CONTROL_PLACE = (3747705955, "Earth")


def open_manifest(path):
    """The .content file is a ZIP with one DEFLATE member which is a SQLite 3 database."""
    if not os.path.exists(path):
        sys.exit(f"manifest not found: {path}")
    cache = os.path.join(
        os.environ.get("CLAUDE_JOB_DIR", "/tmp"), "tmp", "d1-world-en.sqlite"
    )
    os.makedirs(os.path.dirname(cache), exist_ok=True)
    if not os.path.exists(cache) or os.path.getmtime(cache) < os.path.getmtime(path):
        with zipfile.ZipFile(path) as z:
            member = z.namelist()[0]
            with open(cache, "wb") as f:
                f.write(z.read(member))
    return sqlite3.connect(cache)


def rows(db, table):
    """⚠ The `json` column is DECLARED BLOB and STORES TEXT. Binding bytes raises
    'Invalid column type Text'. And `id` is the hash reinterpreted as SIGNED 32-bit - never
    filter on it, never treat it as the hash. Parse the hash out of the JSON.
    See loadout-framework.md §4 traps 1 and 2."""
    for (j,) in db.execute(f'select json from "{table}"'):
        yield json.loads(j)


def tsv(path, header, records):
    with open(path, "w") as f:
        f.write("\t".join(header) + "\n")
        for r in records:
            f.write("\t".join("" if c is None else str(c) for c in r) + "\n")
    return len(records)


def main():
    db = open_manifest(MANIFEST)
    os.makedirs(OUT, exist_ok=True)

    failures = []

    def control(name, got, want):
        ok = got == want
        print(f"  {'PASS' if ok else 'FAIL'}  {name}  [{got}{'' if ok else f' != {want}'}]")
        if not ok:
            failures.append(name)

    print("CONTROLS:")
    for table, want in CONTROLS.items():
        got = db.execute(f'select count(*) from "{table}"').fetchone()[0]
        control(f"{table} row count", got, want)

    places = {p["placeHash"]: p for p in rows(db, "DestinyPlaceDefinition")}
    dests = {d["destinationHash"]: d for d in rows(db, "DestinyDestinationDefinition")}
    acts = {a["activityHash"]: a for a in rows(db, "DestinyActivityDefinition")}
    bundles = {b["bundleHash"]: b for b in rows(db, "DestinyActivityBundleDefinition")}
    flags = {u["flagHash"]: u for u in rows(db, "DestinyUnlockFlagDefinition")}
    books = list(rows(db, "DestinyDirectorBookDefinition"))

    control(
        "destination 2468353467 resolves by hash",
        dests.get(CONTROL_DESTINATION[0], {}).get("destinationName"),
        CONTROL_DESTINATION[1],
    )
    control(
        "place 3747705955 resolves by hash",
        places.get(CONTROL_PLACE[0], {}).get("placeName"),
        CONTROL_PLACE[1],
    )
    # Negative control: a hash we made up must NOT resolve. A lookup that always answers is not
    # a lookup - this is the "scan without a positive control" trap in reverse (ATLAS §4).
    control("bogus hash 1 does NOT resolve", dests.get(1) is None, True)
    # Every activity's completionFlagHash must be a real unlock flag OR exactly zero. The first
    # version of this control asserted "all 653 resolve" and FAILED at 564 - the other 89 carry
    # `completionFlagHash == 0` (playlists, Crucible, and other activities with no completion
    # flag). Zero of them are *unresolved*. Both halves are asserted so neither can drift
    # unnoticed: a rise in `zero` would silently shrink the campaign-flag set.
    nonzero = [a for a in acts.values() if a.get("completionFlagHash")]
    zero = len(acts) - len(nonzero)
    control("every NON-ZERO completionFlagHash resolves", sum(1 for a in nonzero if a["completionFlagHash"] in flags), len(nonzero))
    control("activities with completionFlagHash == 0", zero, 89)

    if failures:
        print(f"\nCONTROLS: {len(failures)} FAILED - output NOT written, do not cite.")
        return 2
    print(f"CONTROLS: {len(CONTROLS) + 5} PASS, 0 FAIL\n")

    def pname(h):
        return places.get(h, {}).get("placeName", "")

    def dname(h):
        return dests.get(h, {}).get("destinationName", "")

    n = tsv(
        os.path.join(OUT, "director-places.tsv"),
        ["place_hash", "place_name", "place_description"],
        sorted(
            [(p["placeHash"], p.get("placeName", ""), p.get("placeDescription", "")) for p in places.values()],
            key=lambda r: r[1],
        ),
    )
    print(f"director-places.tsv          {n}")

    n = tsv(
        os.path.join(OUT, "director-destinations.tsv"),
        ["destination_hash", "destination_name", "identifier", "place_hash", "place_name"],
        sorted(
            [
                (
                    d["destinationHash"],
                    d.get("destinationName", ""),
                    d.get("destinationIdentifier", ""),
                    d.get("placeHash"),
                    pname(d.get("placeHash")),
                )
                for d in dests.values()
            ],
            key=lambda r: (r[4], r[1]),
        ),
    )
    print(f"director-destinations.tsv    {n}")

    # One row per director NODE. `states[].state` is the small integer the family-4/type-4
    # array's 6-bit value plausibly carries. `uiModifier` rides alongside it.
    node_rows = []
    for b in books:
        for i, node in enumerate(b.get("nodes", [])):
            states = [s.get("state") for s in node.get("states", [])]
            bh = node.get("activityBundleHashes", [])
            # resolve the bundle -> its activities -> their completion flags
            act_hashes, flag_hashes, act_names = [], [], []
            for h in bh:
                for ah in bundles.get(h, {}).get("activityHashes", []):
                    act_hashes.append(ah)
                    a = acts.get(ah)
                    if a:
                        act_names.append(a.get("activityName", ""))
                        if a.get("completionFlagHash"):
                            flag_hashes.append(a["completionFlagHash"])
            node_rows.append(
                (
                    b["bookHash"],
                    b.get("bookName", ""),
                    i,
                    node.get("nodeDefinitionHash"),
                    node.get("styleHash"),
                    node.get("uiModifier"),
                    ",".join(str(s) for s in states),
                    ",".join(str(h) for h in bh),
                    ",".join(str(h) for h in act_hashes),
                    ",".join(str(h) for h in flag_hashes),
                    " | ".join(sorted(set(n for n in act_names if n))),
                )
            )
    n = tsv(
        os.path.join(OUT, "director-books.tsv"),
        [
            "book_hash", "book_name", "node_index", "node_definition_hash", "style_hash",
            "ui_modifier", "states", "activity_bundle_hashes", "activity_hashes",
            "completion_flag_hashes", "activity_names",
        ],
        node_rows,
    )
    print(f"director-books.tsv           {n}  ({len(books)} books)")

    n = tsv(
        os.path.join(OUT, "director-activities.tsv"),
        [
            "activity_hash", "activity_name", "completion_flag_hash", "activity_level",
            "activity_power", "tier", "min_party", "max_party", "max_players",
            "is_playlist", "is_matchmade", "destination_hash", "destination_name",
            "place_hash", "place_name", "activity_type_hash", "release_time",
        ],
        sorted(
            [
                (
                    a["activityHash"], a.get("activityName", ""), a.get("completionFlagHash"),
                    a.get("activityLevel"), a.get("activityPower"), a.get("tier"),
                    a.get("minParty"), a.get("maxParty"), a.get("maxPlayers"),
                    a.get("isPlaylist"), a.get("isMatchmade"),
                    a.get("destinationHash"), dname(a.get("destinationHash")),
                    a.get("placeHash"), pname(a.get("placeHash")),
                    a.get("activityTypeHash"), a.get("releaseTime"),
                )
                for a in acts.values()
            ],
            key=lambda r: (r[14], r[12], r[3] or 0, r[1]),
        ),
    )
    print(f"director-activities.tsv      {n}")

    # Which flags are actually referenced by an activity - the subset that matters first.
    referenced = {a["completionFlagHash"] for a in acts.values() if a.get("completionFlagHash")}
    n = tsv(
        os.path.join(OUT, "director-unlock-flags.tsv"),
        ["flag_hash", "unlock_type", "is_offer", "referenced_by_activity"],
        sorted(
            [
                (u["flagHash"], u.get("unlockType"), u.get("isOffer"), u["flagHash"] in referenced)
                for u in flags.values()
            ]
        ),
    )
    print(f"director-unlock-flags.tsv    {n}  ({len(referenced)} referenced by an activity)")

    # ---- the unlock-expression side: what actually gates a director book being VISIBLE ----
    # `isVisibleExpression.steps[]` is a little stack machine: stepOperator 1 carries a flagHash
    # (and a `value` that ranges 98..3850 - an index, NOT the 6-bit state, which caps at 63);
    # stepOperator 3 carries flagHash 0 and value -1, i.e. a combiner, not an operand.
    vis_rows = []
    for b in books:
        for kind in ("isVisibleExpression", "isDefaultExpression"):
            steps = (b.get(kind) or {}).get("steps", [])
            if not steps:
                # An EMPTY expression is the single most load-bearing row in this file:
                # it means the book is unconditionally visible. Emit it explicitly rather than
                # leaving it as an absence, so "no gate" can never be mistaken for "not extracted".
                vis_rows.append((b["bookHash"], b.get("bookName", ""), kind, "", "", "", "UNCONDITIONAL"))
                continue
            for s in steps:
                vis_rows.append(
                    (
                        b["bookHash"], b.get("bookName", ""), kind,
                        s.get("stepOperator"), s.get("flagHash"), s.get("value"),
                        "operand" if s.get("flagHash") else "combiner",
                    )
                )
    n = tsv(
        os.path.join(OUT, "director-book-visibility.tsv"),
        ["book_hash", "book_name", "expression", "step_operator", "flag_hash", "value", "role"],
        vis_rows,
    )
    print(f"director-book-visibility.tsv {n}")

    # ---- record books ("the unlock book") ----
    rb_rows = []
    for rb in rows(db, "DestinyRecordBookDefinition"):
        for gate in (rb.get("bookAvailableUnlockExpression") or {}).get("steps", []):
            rb_rows.append((rb.get("hash"), rb.get("displayName", ""), -1, "bookAvailable",
                            gate.get("stepOperator"), gate.get("flagHash"), gate.get("value"), ""))
        for pi, page in enumerate(rb.get("pages", [])):
            for rw in page.get("rewards", []):
                rb_rows.append((rb.get("hash"), rb.get("displayName", ""), pi, "claimedUnlock",
                                None, rw.get("claimedUnlockHash"), None, page.get("displayName", "")))
                for ex in rw.get("requirementUnlockExpressions", []):
                    for s in ex.get("steps", []):
                        rb_rows.append((rb.get("hash"), rb.get("displayName", ""), pi, "requirement",
                                        s.get("stepOperator"), s.get("flagHash"), s.get("value"),
                                        page.get("displayName", "")))
    n = tsv(
        os.path.join(OUT, "director-record-books.tsv"),
        ["book_hash", "book_name", "page_index", "kind", "step_operator", "flag_hash", "value", "page_name"],
        rb_rows,
    )
    print(f"director-record-books.tsv    {n}")

    # ---- the flag cross-reference: which definition NAMES each flag ----
    # 7,731 flags exist; `unlockType` is 0 for every single one and `isOffer` nearly always false,
    # so the table itself says nothing. A flag only becomes meaningful through what references it.
    def walk(o, path=""):
        if isinstance(o, dict):
            for k, v in o.items():
                yield from walk(v, f"{path}.{k}" if path else k)
        elif isinstance(o, list):
            for v in o:
                yield from walk(v, path + "[]")
        elif isinstance(o, int):
            yield path, o

    tables = [r[0] for r in db.execute("select name from sqlite_master where type='table'")]
    xref = {}
    for t in tables:
        if t == "DestinyUnlockFlagDefinition":
            continue
        for o in rows(db, t):
            owner = o.get("hash")
            name = o.get("activityName") or o.get("bookName") or o.get("itemName") or o.get("displayName") or ""
            for path, v in walk(o):
                if v in flags:
                    xref.setdefault(v, []).append((t, path, owner, name))
    xref_rows = []
    for u in sorted(flags):
        refs = xref.get(u, [])
        if not refs:
            xref_rows.append((u, 0, "", "", "", ""))
        for t, path, owner, name in refs:
            xref_rows.append((u, len(refs), t, path, owner, name))
    n = tsv(
        os.path.join(OUT, "director-unlock-xref.tsv"),
        ["flag_hash", "ref_count", "table", "json_path", "owner_hash", "owner_name"],
        xref_rows,
    )
    print(f"director-unlock-xref.tsv     {n}  ({len(xref)} of {len(flags)} flags referenced anywhere)")

    with open(os.path.join(OUT, "director-map.json"), "w") as f:
        json.dump(
            {
                "source": os.path.relpath(MANIFEST, REPO),
                "reach": "static-client-inference (public 2020 manifest, NOT our 01.29 client)",
                "places": places,
                "destinations": dests,
                "books": books,
                "activities": acts,
                "bundles": bundles,
                "referenced_completion_flags": sorted(referenced),
            },
            f,
            indent=1,
        )
    print(f"director-map.json            written")
    return 0


if __name__ == "__main__":
    sys.exit(main())
