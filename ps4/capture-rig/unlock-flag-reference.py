#!/usr/bin/env python3
"""unlock-flag-reference.py - resolve `DestinyUnlockFlagDefinition` through everything that
references it, and read the unlock-expression bytecode out of the data.

Why this exists: the flag table is 7,731 rows of `{flagHash, unlockType: 0, isOffer: false}`.
It has no names and no types. A flag is only meaningful through its *reference sites*, and the
sites are unlock **expressions** - a little postfix stack machine whose operator enum is not
named anywhere in the eboot's strings. Both halves are recovered here:

  * every TYPED reference site (a named field, never a blind integer walk) → `unlock-flag-*.tsv`
  * the expression machine's operator arities, SOLVED from the corpus and asserted unique,
    plus the AND/OR assignment, PROVEN from a location-release pair that is degenerate under
    every other assignment.

Companion to `director-map.py` (which owns the director/destination/activity graph) and to
`historicaldocs/design/unlock-flag-reference.md` (which this file generates the tables for).

Outputs, under schemas/:
    unlock-operators.tsv          the 9 operators: arity, name, evidence
    unlock-expressions.tsv        every expression in the manifest, rendered infix
    unlock-flag-catalogue.tsv     all 7,731 flags, with the label their references give them
    unlock-flag-references.tsv    one row per TYPED reference
    unlock-activity-flags.tsv     the activity completion flags, by place/destination
    unlock-gates.tsv              what gates each book / location / vendor, resolved
    unlock-values.tsv             the SECOND hash space - `valueHash`, named via objectives
    unlock-hash-collisions.tsv    hashes that live in the flag table AND another table
    unlock-flag-reference.json    the catalogue, keyed by flag, for programmatic lookup

Controls run on every invocation; **exit 2 if any fails and no output is written.** Several are
paired negative controls - in particular the arity solver is re-run with a deliberately wrong
arity and must FAIL, because a balance check that cannot fail is not evidence.

⚠ REACH: `static-client-inference`, and the source is NOT our client - it is Bungie's archived
public manifest (a 2020 capture; our target is 01.29 / Oct 2016). Hashes are *believed* stable
across D1's life; that is believed, not measured. Nothing here has been confirmed against the
01.29 client. See historicaldocs/design/loadout-framework.md §5 and historicaldocs/design/unlock-flag-reference.md §0.
"""

import itertools
import json
import os
import sqlite3
import sys
import zipfile
from collections import Counter, defaultdict

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
OUT = os.path.join(HERE, "schemas")

MANIFEST = os.path.join(
    REPO,
    "manifest/data/world_en_world_sql_content_39adcbfdc2021172e8ccf4720ca76023.content",
)

# ---------------------------------------------------------------------------------------------
# Pinned expectations. Every one is a control: a count that changes is not automatically wrong,
# but it means the manifest changed and every downstream reading must be re-checked.
# ---------------------------------------------------------------------------------------------
TABLE_COUNTS = {
    "DestinyUnlockFlagDefinition": 7731,
    "DestinyActivityDefinition": 653,
    "DestinyDirectorBookDefinition": 9,
    "DestinyRecordBookDefinition": 5,
    "DestinyRecordDefinition": 168,
    "DestinyLocationDefinition": 150,
    "DestinyVendorDefinition": 95,
    "DestinyObjectiveDefinition": 1388,
    "DestinySandboxPerkDefinition": 1058,
    "DestinyInventoryItemDefinition": 8609,
    "DestinyBondDefinition": 4,
}

# The operator arities are NOT hand-written: `solve_arities()` derives them from the corpus and
# this table is the assertion that the derivation is stable. See §2 of the design doc.
ARITY = {1: 0, 2: 1, 3: 2, 4: 2, 8: 2, 10: 0, 11: 0, 12: 2, 21: 1}
OP_HISTOGRAM = {1: 3113, 2: 197, 3: 19, 4: 55, 8: 21, 10: 26, 11: 26, 12: 5, 21: 10}

OP_NAME = {
    1: "FLAG",       # push: the boolean state of `flagHash`
    2: "NOT",        # unary
    3: "AND",        # binary  - proven, see prove_and_or()
    4: "OR",         # binary  - proven, see prove_and_or()
    8: "CMP",        # binary comparison; op12 is its exact logical complement
    10: "VALUE",     # push: the integer unlock VALUE named by `valueHash`
    11: "CONST",     # push: the literal in `value`
    12: "NCMP",      # binary comparison, the complement of op8
    21: "COERCE",    # unary, [U] - only ever applied to CONST(1), only ever before a comparison
}

# Two named rows, both directions.
CONTROL_FLAG = 1631415873            # the shared gate on Moon / Venus / Mars / Reef
CONTROL_BOGUS = 1                    # must NOT resolve
CONTROL_XUR_LOCATION = 1438636527    # r1_s4_locations_npcs.xur - the AND/OR proof lives here
CONTROL_COMPLEMENT_PAIRS = [2784818374, 3028684324]   # rel0 == NOT rel1, no comparisons involved

# Joins that make the reference doc worth reading: a destination gate that is an activity's own
# completion flag. Both the flag and the activity are pinned so neither can drift silently.
CONTROL_GATE_JOINS = {
    2388787017: "A Stranger's Call",
    2617018872: "The Dreadnaught",
    3054678575: "Iron Temple",
    3230331915: "A Key Awaits",
    4228285953: "The Dark Beyond",
}


# ---------------------------------------------------------------------------------------------
# manifest access
# ---------------------------------------------------------------------------------------------
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
            with open(cache, "wb") as f:
                f.write(z.read(z.namelist()[0]))
    return sqlite3.connect(cache)


def rows(db, table):
    """⚠ Two traps, both measured (loadout-framework.md §4):
    1. `id` is the hash reinterpreted as a SIGNED 32-bit int. Never filter on it, never treat it
       as the hash. Parse the hash out of the JSON.
    2. The `json` column is DECLARED `BLOB` and STORES `TEXT`. Binding bytes raises
       'Invalid column type Text'."""
    for (j,) in db.execute(f'select json from "{table}"'):
        yield json.loads(j)


def tsv(path, header, records):
    with open(path, "w") as f:
        f.write("\t".join(header) + "\n")
        for r in records:
            f.write("\t".join("" if c is None else str(c) for c in r) + "\n")
    return len(records)


# ---------------------------------------------------------------------------------------------
# the expression machine
# ---------------------------------------------------------------------------------------------
def find_expressions(db, tables):
    """Every unlock expression in the manifest, with its owner.

    ⚠ An object is an unlock expression only if its `steps[]` carry `stepOperator`. Several
    unrelated things in this manifest also have a `steps` key - `DestinyTalentGridDefinition`
    has 11,175 of them and `DestinyProgressionDefinition` has 251. A `'steps' in o` test picks
    up 12,750 non-expressions and silently poisons every histogram downstream."""
    found = []

    def walk(o, table, path, owner, owner_name):
        if isinstance(o, dict):
            st = o.get("steps")
            if isinstance(st, list) and all(
                isinstance(s, dict) and "stepOperator" in s for s in st
            ) and (st or "Expression" in path):
                found.append((table, path, owner, owner_name, st))
            for k, v in o.items():
                walk(v, table, f"{path}.{k}" if path else k, owner, owner_name)
        elif isinstance(o, list):
            for v in o:
                walk(v, table, path + "[]", owner, owner_name)

    for t in tables:
        for o in rows(db, t):
            name = (
                o.get("activityName") or o.get("bookName") or o.get("itemName")
                or o.get("displayName") or o.get("contentIdentifier") or ""
            )
            walk(o, t, "", o.get("hash"), name)
    return found


def balances(steps, arity):
    """Postfix evaluation depth check: never underflow, finish with exactly one value."""
    depth = 0
    for s in steps:
        a = arity.get(s["stepOperator"])
        if a is None or depth < a:
            return False
        depth += 1 - a
    return depth == 1


def solve_arities(shapes, ops):
    """Derive each operator's arity from the corpus alone.

    Brute force over arity in {0,1,2} for every operator, keep the assignments under which EVERY
    expression is stack-balanced. (`shapes` is the deduplicated set of operator sequences - 10 of
    them for 3,065 expressions, which is what makes 3^9 assignments cheap.)

    ⚠ Balance alone does NOT determine the answer: exactly TWO assignments survive. They differ
    only on {8, 11, 12} - the alternative reads op11 as a unary that consumes the value op10
    pushed, and op8/op12 as unaries carrying their second operand inline. That machine is
    perfectly coherent; it is broken by `sentinel_ok()`, not by the stack."""
    ops = sorted(ops)
    ok = []
    for combo in itertools.product((0, 1, 2), repeat=len(ops)):
        arity = dict(zip(ops, combo))
        if all(balances([{"stepOperator": o} for o in sh], arity) for sh in shapes):
            ok.append(arity)
    return ok


def sentinel_ok(arity, steps):
    """The tie-breaker, and it is a measured regularity rather than a preference.

    `value` is the step's inline operand: the flag index for op1, the literal for op11, unused
    (0) for op2/op10/op21 - and **exactly -1** for op3, op4, op8 and op12, on all 100 of their
    occurrences, with no exceptions in either direction. -1 is a sentinel meaning "this step has
    no inline operand", i.e. it takes both operands off the stack. Requiring
    `value == -1  <=>  arity == 2` leaves exactly one of the two balanced assignments.

    If a future manifest shows an arity-2 operator with `value != -1`, this argument is dead and
    the two-solution ambiguity is back. That is why the rejected solution is printed, not hidden."""
    for s in steps:
        if (s.get("value") == -1) != (arity.get(s["stepOperator"]) == 2):
            return False
    return True


def render(steps, op_name=None, flag_label=None):
    """Render a postfix expression as infix. Returns '' for the empty (unconditional) expression."""
    op_name = op_name or OP_NAME
    st = []
    for s in steps:
        op, a = s["stepOperator"], ARITY[s["stepOperator"]]
        nm = op_name.get(op, f"op{op}")
        if a == 0:
            if op == 1:
                h = s["flagHash"]
                st.append(flag_label(h) if flag_label else f"flag:{h}")
            elif op == 10:
                st.append(f"value:{s['valueHash']}")
            elif op == 11:
                st.append(str(s["value"]))
            else:
                st.append(f"{nm}?")
        elif a == 1:
            x = st.pop()
            st.append(f"NOT {x}" if op == 2 else f"{nm}({x})")
        else:
            b, aa = st.pop(), st.pop()
            st.append(f"({aa} {nm} {b})" if op in (3, 4) else f"({aa} {nm} {b})")
    return st[0] if len(st) == 1 else " <UNBALANCED> ".join(st)


def evaluate(steps, env, binops, cmp_atoms):
    """Boolean evaluation used only by the proofs. `binops` maps op3/op4 to 'AND'/'OR';
    `cmp_atoms` supplies a truth value for each comparison operator occurrence."""
    st = []
    for s in steps:
        op = s["stepOperator"]
        if op == 1:
            st.append(env[s["flagHash"]])
        elif op in (10, 11):
            st.append(None)          # operands of a comparison; value irrelevant to the proof
        elif op == 21:
            st.append(None if st.pop() is None else None)   # unary, still a comparison operand
        elif op == 2:
            st.append(not st.pop())
        elif op in (8, 12):
            st.pop(), st.pop()
            st.append(cmp_atoms[op])
        elif op in (3, 4):
            b, a = st.pop(), st.pop()
            st.append((a and b) if binops[op] == "AND" else (a or b))
        else:
            raise AssertionError(f"unmodelled operator {op}")
    assert len(st) == 1
    return st[0]


def _truth_table(steps, flags, binops):
    """All values `steps` can take as the free flags and the free comparison atom vary."""
    out = set()
    for bits in itertools.product((False, True), repeat=len(flags)):
        env = dict(zip(flags, bits))
        for c8 in (False, True):
            out.add(evaluate(steps, env, binops, {8: c8, 12: not c8}))
    return out


def prove_and_or(exprs, locations):
    """Which of op3/op4 is AND and which is OR.

    ⚠ Logic alone CANNOT answer this. (AND,OR) and (OR,AND) are De Morgan duals: swap them and
    complement, and every expression in the corpus stays satisfiable. Two filters are applied,
    the second of which is the one that bites.

    FILTER 1 - no expression may be constant. A gate that ignores all state is not a gate.
      This kills (AND,AND) and (OR,OR) - both make Xur's second release constant - and nothing
      else. Two candidates survive.

    FILTER 2 - every location's release list must be EXHAUSTIVE.
      `locationReleases[]` is an ordered first-match-wins list that answers "where is this
      thing?", so some release must always match. This is not an assumption bolted on to get
      the answer we wanted: 142 of the 150 locations in this manifest end in an *empty*
      (unconditional) expression, and 7 of the remaining 8 are exact complementary pairs - 149
      locations of support and no counterexample. The 150th is Xur, whose two releases are
      **The Reef** (Vestian Outpost) and **The Last City** (the Tower):

          rel0 = op4(op4(f1,f2),f3)                       -> The Reef, Asteroid Belt
          rel1 = op4( NOT rel0, op3(op3(f1,f2),f3) )      -> The Last City, Earth

      Under (op3=AND, op4=OR): rel0 v rel1  =  A v ~A v B  =  TRUE. Exhaustive.
      Under (op3=OR,  op4=AND): rel0 v rel1  =  (f1^f2^f3) v (~(f1^f2^f3) ^ (f1vf2vf3))
                                              =  f1 v f2 v f3 - FALSE when no flag is set, i.e.
                                                 Xur has nowhere to be. Not exhaustive.

    Returns the surviving assignments; the caller asserts there is exactly one.
    ⚠ This is `[I]`, not `[C]`: it is a proof *given* the exhaustiveness premise. The premise is
    a property of this manifest, not something we have watched the 01.29 client rely on."""
    survivors = []
    for a3, a4 in itertools.product(("AND", "OR"), repeat=2):
        binops = {3: a3, 4: a4}
        ok = True
        for _, _, _, _, st in exprs:
            fl = sorted({s["flagHash"] for s in st if s.get("flagHash")})
            if len(fl) > 12:
                continue                      # 2^12 x 2 is the practical truth-table budget
            if len(_truth_table(st, fl, binops)) == 1:
                ok = False
                break
        if not ok:
            continue
        for L in locations.values():
            rels = [(lr.get("activeUnlockExpression") or {}).get("steps", [])
                    for lr in L.get("locationReleases", [])]
            if not rels or any(not r for r in rels):
                continue                      # an empty expression is an unconditional fallback
            fl = sorted({s["flagHash"] for r in rels for s in r if s.get("flagHash")})
            if len(fl) > 12:
                continue
            for bits in itertools.product((False, True), repeat=len(fl)):
                env = dict(zip(fl, bits))
                for c8 in (False, True):
                    atoms = {8: c8, 12: not c8}
                    if not any(evaluate(r, env, binops, atoms) for r in rels):
                        ok = False
                        break
                if not ok:
                    break
            if not ok:
                break
        if ok:
            survivors.append((a3, a4))
    return survivors


def complementary(releases, binops, complement_cmp):
    """Is `rel0 == NOT rel1` over every assignment of the free flags?
    `complement_cmp` decides whether op12 is modelled as the complement of op8 (the claim) or as
    the same atom (the negative control)."""
    flags = sorted({s["flagHash"] for st in releases for s in st if s.get("flagHash")})
    for bits in itertools.product((False, True), repeat=len(flags)):
        env = dict(zip(flags, bits))
        for c8 in (False, True):
            atoms = {8: c8, 12: (not c8) if complement_cmp else c8}
            if evaluate(releases[0], env, binops, atoms) == evaluate(releases[1], env, binops, atoms):
                return False
    return True


# ---------------------------------------------------------------------------------------------
# typed reference extraction - a named field, never a blind integer walk
# ---------------------------------------------------------------------------------------------
def typed_references(db, flags):
    """Yield (flag_hash, table, json_path, kind, owner_hash, owner_name, context).

    ⚠ Why typed and not a walk: `director-map.py` finds every integer in every table that happens
    to be in the flag set. That is the right tool for *discovery* and the wrong one for a
    reference list - 4,294,967,296 hashes and 7,731 flags means collisions are expected, and
    there are 60 of them here. `DestinyInventoryItemDefinition.perkHashes[]` looks like 154
    references and is 154 sandbox perks. See `unlock-hash-collisions.tsv`."""
    refs = []

    def add(h, table, path, kind, owner, name, ctx=""):
        if h:
            refs.append((h, table, path, kind, owner, name, ctx))

    def steps_of(x):
        return (x or {}).get("steps", []) or []

    places = {p["placeHash"]: p.get("placeName", "") for p in rows(db, "DestinyPlaceDefinition")}
    dests = {d["destinationHash"]: d.get("destinationName", "")
             for d in rows(db, "DestinyDestinationDefinition")}

    for a in rows(db, "DestinyActivityDefinition"):
        add(a.get("completionFlagHash"), "DestinyActivityDefinition", "completionFlagHash",
            "activity_completion", a["activityHash"], a.get("activityName", ""),
            f"{places.get(a.get('placeHash'),'')} / {dests.get(a.get('destinationHash'),'')}")

    for b in rows(db, "DestinyDirectorBookDefinition"):
        for kind in ("isVisibleExpression", "isDefaultExpression"):
            for s in steps_of(b.get(kind)):
                add(s.get("flagHash"), "DestinyDirectorBookDefinition",
                    f"{kind}.steps[].flagHash", "director_book_" + kind[2:-10].lower(),
                    b["bookHash"], b.get("bookName", ""), "")

    for L in rows(db, "DestinyLocationDefinition"):
        for i, lr in enumerate(L.get("locationReleases", [])):
            for s in steps_of(lr.get("activeUnlockExpression")):
                add(s.get("flagHash"), "DestinyLocationDefinition",
                    "locationReleases[].activeUnlockExpression.steps[].flagHash",
                    "location_release", L["locationHash"], L.get("contentIdentifier", ""),
                    f"release {i}")

    for v in rows(db, "DestinyVendorDefinition"):
        s = v.get("summary", {})
        for h in s.get("unlockFlagHashes", []):
            add(h, "DestinyVendorDefinition", "summary.unlockFlagHashes[]", "vendor_unlock",
                v.get("hash"), s.get("vendorName", ""), "")
        for h in s.get("enabledUnlockFlagHashes", []):
            add(h, "DestinyVendorDefinition", "summary.enabledUnlockFlagHashes[]",
                "vendor_enabled", v.get("hash"), s.get("vendorName", ""), "")

    for rb in rows(db, "DestinyRecordBookDefinition"):
        for s in steps_of(rb.get("bookAvailableUnlockExpression")):
            add(s.get("flagHash"), "DestinyRecordBookDefinition",
                "bookAvailableUnlockExpression.steps[].flagHash", "record_book_available",
                rb.get("hash"), rb.get("displayName", ""), rb.get("contentIdentifier", ""))
        for pi, pg in enumerate(rb.get("pages", [])):
            for rw in pg.get("rewards", []):
                add(rw.get("claimedUnlockHash"), "DestinyRecordBookDefinition",
                    "pages[].rewards[].claimedUnlockHash", "record_book_claimed",
                    rb.get("hash"), rb.get("displayName", ""),
                    f"page {pi} {pg.get('displayName','')} item {rw.get('itemHash')}")
                for ex in rw.get("requirementUnlockExpressions", []):
                    for s in steps_of(ex):
                        add(s.get("flagHash"), "DestinyRecordBookDefinition",
                            "pages[].rewards[].requirementUnlockExpressions[].steps[].flagHash",
                            "record_book_requirement", rb.get("hash"),
                            rb.get("displayName", ""), f"page {pi} {pg.get('displayName','')}")

    for r in rows(db, "DestinyRecordDefinition"):
        for ar in r.get("actualRewards", []):
            for s in steps_of(ar.get("itemUnlockExpression")):
                add(s.get("flagHash"), "DestinyRecordDefinition",
                    "actualRewards[].itemUnlockExpression.steps[].flagHash",
                    "record_reward_test", r.get("hash"), r.get("displayName", ""),
                    r.get("contentIdentifier", ""))

    for it in rows(db, "DestinyInventoryItemDefinition"):
        for s in steps_of((it.get("equippingBlock") or {}).get("customDyeExpression")):
            add(s.get("flagHash"), "DestinyInventoryItemDefinition",
                "equippingBlock.customDyeExpression.steps[].flagHash", "item_custom_dye",
                it.get("itemHash"), it.get("itemName", ""), "")

    for bd in rows(db, "DestinyBondDefinition"):
        add(bd.get("providedUnlockHash"), "DestinyBondDefinition", "providedUnlockHash",
            "bond_provided", bd.get("hash"), bd.get("contentIdentifier", ""), "")

    unresolved = [r for r in refs if r[0] not in flags]
    return refs, unresolved


def blind_walk_hits(db, tables, flags):
    """The `director-map.py` method, reproduced here ONLY so the collisions can be counted and
    published. Not used to build the reference list."""
    hits = defaultdict(list)

    def walk(o, path):
        if isinstance(o, dict):
            for k, v in o.items():
                yield from walk(v, f"{path}.{k}" if path else k)
        elif isinstance(o, list):
            for v in o:
                yield from walk(v, path + "[]")
        elif isinstance(o, int):
            yield path, o

    for t in tables:
        if t == "DestinyUnlockFlagDefinition":
            continue
        for o in rows(db, t):
            for p, v in walk(o, ""):
                if v in flags:
                    hits[(t, p)].append(v)
    return hits


# ---------------------------------------------------------------------------------------------
def main():
    db = open_manifest(MANIFEST)
    os.makedirs(OUT, exist_ok=True)
    tables = [r[0] for r in db.execute("select name from sqlite_master where type='table'")]

    failures = []

    def control(name, got, want):
        ok = got == want
        print(f"  {'PASS' if ok else 'FAIL'}  {name}  [{got}{'' if ok else f' != {want}'}]")
        if not ok:
            failures.append(name)

    print("CONTROLS:")
    for t, want in TABLE_COUNTS.items():
        control(f"{t} rows", db.execute(f'select count(*) from "{t}"').fetchone()[0], want)

    flag_rows = list(rows(db, "DestinyUnlockFlagDefinition"))
    flags = {u["flagHash"] for u in flag_rows}
    control("distinct flagHash == row count", len(flags), len(flag_rows))
    control("unlockType is 0 for every flag", sorted({u.get("unlockType") for u in flag_rows}), [0])
    control("isOffer is false for every flag too", sorted({bool(u.get("isOffer")) for u in flag_rows}), [False])
    control(f"control flag {CONTROL_FLAG} resolves", CONTROL_FLAG in flags, True)
    control(f"bogus hash {CONTROL_BOGUS} does NOT resolve", CONTROL_BOGUS in flags, False)

    # ---- the expression machine -------------------------------------------------------------
    exprs = find_expressions(db, tables)
    nonempty = [e for e in exprs if e[4]]
    control("expressions found (non-empty)", len(nonempty), 3065)
    control("expressions found (incl. empty/unconditional)", len(exprs), 5614)

    hist = Counter(s["stepOperator"] for _, _, _, _, st in nonempty for s in st)
    control("operator histogram", dict(sorted(hist.items())), OP_HISTOGRAM)

    shapes = {tuple(s["stepOperator"] for s in st) for _, _, _, _, st in nonempty}
    control("distinct operator shapes", len(shapes), 10)
    balanced = solve_arities(shapes, hist.keys())
    control("arity: stack balance alone leaves TWO candidates", len(balanced), 2)
    all_steps = [s for _, _, _, _, st in nonempty for s in st]
    survivors = [a for a in balanced if sentinel_ok(a, all_steps)]
    control("arity: the value==-1 sentinel leaves exactly ONE", len(survivors), 1)
    control("arity solution matches the pin", survivors[0] if survivors else None, ARITY)
    rejected = [a for a in balanced if a not in survivors]
    control("the rejected candidate differs only on {8, 11, 12}",
            sorted(k for k in ARITY if rejected and rejected[0][k] != ARITY[k]), [8, 11, 12])
    print(f"        (rejected but stack-valid: {rejected[0] if rejected else None})")
    control(
        "every expression balances under the solved arity",
        sum(1 for _, _, _, _, st in nonempty if balances(st, ARITY)),
        len(nonempty),
    )
    # NEGATIVE control: a balance check that cannot fail is not evidence. Force op2 (proven
    # unary by the `(1,2)` shape under BOTH candidates) to be binary and require that
    # expressions now FAIL to balance.
    bad = {**ARITY, 2: 2}
    control(
        "balance check HAS POWER (op2 forced binary must break it)",
        sum(1 for _, _, _, _, st in nonempty if not balances(st, bad)) > 0,
        True,
    )
    # NEGATIVE control on the sentinel: it must reject something, or it is not a filter.
    control("sentinel HAS POWER (rejects the other stack-valid candidate)",
            sentinel_ok(rejected[0], all_steps) if rejected else None, False)

    # ---- AND / OR, proven from a degenerate-under-every-other-assignment location ------------
    locs = {L["locationHash"]: L for L in rows(db, "DestinyLocationDefinition")}
    # NEGATIVE control first: no-constant-expressions ALONE must leave the answer ambiguous.
    # Publishing this stops a future reader from believing the AND/OR call is cheaper than it is.
    noconst = [
        (a3, a4) for a3, a4 in itertools.product(("AND", "OR"), repeat=2)
        if all(len(_truth_table(st, sorted({s["flagHash"] for s in st if s.get("flagHash")}),
                                {3: a3, 4: a4})) > 1
               for _, _, _, _, st in nonempty
               if len({s["flagHash"] for s in st if s.get("flagHash")}) <= 12)
    ]
    control("AND/OR: 'no constant expression' alone leaves TWO (De Morgan duals)", len(noconst), 2)
    andor = prove_and_or(nonempty, locs)
    control("AND/OR: + location-release exhaustiveness leaves exactly ONE", len(andor), 1)
    control("op3 = AND, op4 = OR", andor[0] if andor else None, ("AND", "OR"))
    control("Xur's location content id", locs[CONTROL_XUR_LOCATION].get("contentIdentifier"),
            "r1_s4_locations_npcs.xur")
    # The premise's own support: the release lists really are exhaustive everywhere else.
    fallback = sum(1 for L in locs.values()
                   if any(not (lr.get("activeUnlockExpression") or {}).get("steps")
                          for lr in L.get("locationReleases", [])))
    empty_loc = sum(1 for L in locs.values() if not L.get("locationReleases"))
    control("locations with an unconditional fallback release", fallback, 141)
    control("locations with NO releases at all (vacuous, excluded)", empty_loc, 1)
    control("locations whose releases ALL carry an expression",
            len(locs) - fallback - empty_loc, 8)
    # And the premise has teeth at Xur specifically: his two releases are two different places.
    xur_dests = [lr.get("destinationHash") for lr in locs[CONTROL_XUR_LOCATION]["locationReleases"]]
    control("Xur's two releases are two DIFFERENT destinations",
            len(set(xur_dests)) == 2, True)

    binops = {3: "AND", 4: "OR"}
    for lh in CONTROL_COMPLEMENT_PAIRS:
        rel = [(lr.get("activeUnlockExpression") or {}).get("steps", [])
               for lr in locs[lh]["locationReleases"]]
        control(f"location {lh} releases are exact complements", complementary(rel, binops, True), True)

    # The five Rise of Iron locations: complementary ONLY IF op12 is the complement of op8.
    # Both directions are asserted, so this is a measurement of op12 and not an assumption.
    roi = [lh for lh, L in locs.items()
           if len(L.get("locationReleases", [])) == 2
           and any(s["stepOperator"] == 12
                   for lr in L["locationReleases"]
                   for s in (lr.get("activeUnlockExpression") or {}).get("steps", []))]
    control("locations using op12", len(roi), 5)
    rel_sets = [[(lr.get("activeUnlockExpression") or {}).get("steps", [])
                 for lr in locs[lh]["locationReleases"]] for lh in roi]
    control("op12 == NOT op8 makes all 5 pairs complementary",
            sum(1 for r in rel_sets if complementary(r, binops, True)), 5)
    control("op12 == op8 makes NONE of them complementary (negative control)",
            sum(1 for r in rel_sets if complementary(r, binops, False)), 0)

    # ---- typed references vs the blind walk -------------------------------------------------
    refs, unresolved = typed_references(db, flags)
    control("every TYPED flag reference resolves in the flag table", len(unresolved), 0)
    control("typed references", len(refs), 3728)
    typed_flags = {r[0] for r in refs}
    control("distinct flags with a typed reference", len(typed_flags), 446)
    control("flags with NO reference anywhere in the manifest", len(flags) - len(typed_flags), 7285)
    # both directions: a flag that IS referenced and a flag that is NOT.
    control("control flag is among the referenced", CONTROL_FLAG in typed_flags, True)
    unref_sample = sorted(flags - typed_flags)[0]
    control(f"sample unreferenced flag {unref_sample} really has no typed ref",
            unref_sample in typed_flags, False)

    perks = {p["perkHash"] for p in rows(db, "DestinySandboxPerkDefinition")}
    collide = flags & perks
    control("hashes in BOTH the flag table and the sandbox-perk table", len(collide), 60)
    perkhash_hits = [ph for it in rows(db, "DestinyInventoryItemDefinition")
                     for ph in it.get("perkHashes", []) if ph in flags]
    control("item perkHashes[] that a blind walk calls flag references", len(perkhash_hits), 154)
    control("...and that ALL resolve as real sandbox perks (so they are collisions)",
            sum(1 for p in perkhash_hits if p in perks), len(perkhash_hits))
    # negative control on the collision claim: the typed sites are not collisions.
    completion = {a["completionFlagHash"] for a in rows(db, "DestinyActivityDefinition")
                  if a.get("completionFlagHash")}
    control("no activity completion flag is also a sandbox perk", len(completion & perks), 0)

    dye = [r for r in refs if r[3] == "item_custom_dye"]
    control("customDye references", len(dye), 2852)
    control("...to exactly ONE distinct flag", len({r[0] for r in dye}), 1)

    # ---- the SECOND hash space: unlock VALUES -----------------------------------------------
    # Paired control. The public manifest zeroes several unlock fields. "All zero" could be a
    # reader bug, so a field of the SAME shape that is NOT zeroed is asserted alongside.
    grim = [g.get("unlockFlagHash", 0) for g in rows(db, "DestinyGrimoireCardDefinition")]
    control("Grimoire unlockFlagHash rows", len(grim), 834)
    control("...every one of them is ZERO in the public manifest", sum(1 for g in grim if g), 0)
    objs = list(rows(db, "DestinyObjectiveDefinition"))
    control("POSITIVE control for that: objective unlockValueHash is NOT zeroed",
            sum(1 for o in objs if o.get("unlockValueHash")), 1377)
    control("distinct unlock VALUE hashes named by an objective",
            len({o["unlockValueHash"] for o in objs if o.get("unlockValueHash")}), 1375)
    step_vh = sorted({s["valueHash"] for _, _, _, _, st in nonempty for s in st if s.get("valueHash")})
    control("distinct valueHash used by an expression", step_vh, [408185636, 2246811594])

    # ---- op1's `value`: a per-flag constant, but populated at only 2 of the 10 sites ----------
    idx, idx_n, idx_where, idx_conflict = {}, Counter(), defaultdict(set), []
    for t, p, _, _, st in nonempty:
        for s in st:
            if s["stepOperator"] == 1 and s.get("value"):
                if idx.setdefault(s["flagHash"], s["value"]) != s["value"]:
                    idx_conflict.append(s["flagHash"])
                idx_n[s["flagHash"]] += 1
                idx_where[s["flagHash"]].add(t)
    control("flags whose op1 `value` is non-zero anywhere", len(idx), 10)
    control("...and it is the SAME value at every such site (a function of the flag)",
            idx_conflict, [])
    control("...and it is unique per flag (no two flags share one)", len(set(idx.values())), len(idx))
    control("value range", (min(idx.values()), max(idx.values())), (98, 3850))
    # NEGATIVE half: it is NOT populated everywhere. Flag 2617018872 carries 3850 in the director
    # book and 0 in its location release, so `value` is a property of the SITE, not of the flag.
    control("op1 steps with value == 0 (so `value` is not a universal flag index)",
            sum(1 for _, _, _, _, st in nonempty for s in st
                if s["stepOperator"] == 1 and not s.get("value")), 248)
    control("flag 2617018872 carries a value at one site and 0 at another",
            sorted({s.get("value") for _, _, _, _, st in nonempty for s in st
                    if s["stepOperator"] == 1 and s["flagHash"] == 2617018872}), [0, 3850])

    # ---- gate -> activity joins --------------------------------------------------------------
    acts = list(rows(db, "DestinyActivityDefinition"))
    for h, want in CONTROL_GATE_JOINS.items():
        names = sorted({a.get("activityName", "") for a in acts if a.get("completionFlagHash") == h})
        control(f"gate {h} is the completion flag of", names, [want])

    if failures:
        print(f"\nCONTROLS: {len(failures)} FAILED - output NOT written, do not cite.")
        return 2
    print("CONTROLS: all PASS, 0 FAIL\n")

    # =========================================================================================
    # OUTPUT
    # =========================================================================================
    places = {p["placeHash"]: p.get("placeName", "") for p in rows(db, "DestinyPlaceDefinition")}
    dests = {d["destinationHash"]: d.get("destinationName", "")
             for d in rows(db, "DestinyDestinationDefinition")}
    atypes = {t["activityTypeHash"]: t.get("activityTypeName", "")
              for t in rows(db, "DestinyActivityTypeDefinition")}

    by_flag = defaultdict(list)
    for r in refs:
        by_flag[r[0]].append(r)

    # flag -> a short human label, from its highest-signal reference site
    KIND_RANK = ["activity_completion", "director_book_visible", "director_book_default",
                 "location_release", "vendor_unlock", "vendor_enabled",
                 "record_book_available", "record_book_requirement", "record_book_claimed",
                 "record_reward_test", "bond_provided", "item_custom_dye"]

    def label(h):
        rs = by_flag.get(h)
        if not rs:
            return "UNREFERENCED"
        rs = sorted(rs, key=lambda r: KIND_RANK.index(r[3]) if r[3] in KIND_RANK else 99)
        best = rs[0]
        names = sorted({x[5] for x in rs if x[3] == best[3] and x[5]})
        return f"{best[3]}:{' / '.join(names[:3])}" if names else best[3]

    n = tsv(
        os.path.join(OUT, "unlock-operators.tsv"),
        ["op", "name", "arity", "occurrences", "flag_hash_field", "value_hash_field",
         "value_field", "how_established"],
        [
            (1, "FLAG", 0, hist[1], "the flag being tested", "0", "an index, 98..3850, only populated at 2 of 10 sites",
             "push; the only step that carries a flagHash"),
            (2, "NOT", 1, hist[2], "0", "0", "0",
             "arity solved; semantics from complementary location-release pairs"),
            (3, "AND", 2, hist[3], "0", "0", "-1",
             "arity solved; AND/OR proven from the Xur location (every other assignment is degenerate)"),
            (4, "OR", 2, hist[4], "0", "0", "-1",
             "arity solved; AND/OR proven from the Xur location"),
            (8, "CMP", 2, hist[8], "0", "0", "-1",
             "arity solved; compares VALUE against CONST; [I] plausibly >="),
            (10, "VALUE", 0, hist[10], "0", "the unlock VALUE to read", "0",
             "push; the only step that carries a valueHash"),
            (11, "CONST", 0, hist[11], "0", "0", "the literal",
             "push; value is 1 (locations) or a threshold (SRL records)"),
            (12, "NCMP", 2, hist[12], "0", "0", "-1",
             "arity solved; PROVEN to be the exact complement of op8 (5 location pairs, both directions)"),
            (21, "COERCE", 1, hist[21], "0", "0", "0",
             "arity solved; [U] semantics - only ever applied to CONST(1), only ever before a comparison"),
        ],
    )
    print(f"unlock-operators.tsv          {n}")

    expr_rows = []
    for table, path, owner, owner_name, st in sorted(exprs, key=lambda e: (e[0], e[1], str(e[2]))):
        expr_rows.append((
            table, path, owner, owner_name, len(st),
            ",".join(str(s["stepOperator"]) for s in st),
            render(st) if st else "UNCONDITIONAL",
            ",".join(str(s["flagHash"]) for s in st if s.get("flagHash")),
            ",".join(str(s["valueHash"]) for s in st if s.get("valueHash")),
        ))
    n = tsv(
        os.path.join(OUT, "unlock-expressions.tsv"),
        ["table", "json_path", "owner_hash", "owner_name", "step_count", "operators",
         "infix", "flag_hashes", "value_hashes"],
        expr_rows,
    )
    print(f"unlock-expressions.tsv        {n}  ({len(nonempty)} non-empty)")

    n = tsv(
        os.path.join(OUT, "unlock-flag-references.tsv"),
        ["flag_hash", "kind", "table", "json_path", "owner_hash", "owner_name", "context"],
        sorted([(r[0], r[3], r[1], r[2], r[4], r[5], r[6]) for r in refs]),
    )
    print(f"unlock-flag-references.tsv    {n}")

    cat_rows = []
    catalogue = {}
    for u in sorted(flag_rows, key=lambda u: u["flagHash"]):
        h = u["flagHash"]
        rs = by_flag.get(h, [])
        kinds = sorted({r[3] for r in rs})
        row = (
            h,
            h - 2 ** 32 if h >= 2 ** 31 else h,        # the signed `id` column, for cross-checking
            int(bool(u.get("isOffer"))),
            len(rs),
            ",".join(kinds),
            len({r[3] for r in rs}),
            label(h),
            int(h in collide),
        )
        cat_rows.append(row)
        catalogue[str(h)] = {
            "flag_hash": h, "is_offer": bool(u.get("isOffer")), "ref_count": len(rs),
            "kinds": kinds, "label": row[6], "collides_with_sandbox_perk": h in collide,
            "references": [
                {"kind": r[3], "table": r[1], "path": r[2], "owner_hash": r[4],
                 "owner_name": r[5], "context": r[6]} for r in rs
            ],
        }
    n = tsv(
        os.path.join(OUT, "unlock-flag-catalogue.tsv"),
        ["flag_hash", "signed_id", "is_offer", "ref_count", "kinds", "distinct_kinds",
         "label", "collides_with_sandbox_perk"],
        cat_rows,
    )
    print(f"unlock-flag-catalogue.tsv     {n}  ({len(typed_flags)} referenced, "
          f"{len(flags)-len(typed_flags)} not)")

    act_rows = []
    for a in acts:
        h = a.get("completionFlagHash")
        if not h:
            continue
        act_rows.append((
            h, a["activityHash"], a.get("activityName", ""),
            places.get(a.get("placeHash"), ""), dests.get(a.get("destinationHash"), ""),
            atypes.get(a.get("activityTypeHash"), ""), a.get("activityLevel"),
            a.get("tier"), int(bool(a.get("isPlaylist"))), int(bool(a.get("isMatchmade"))),
            len(by_flag.get(h, [])),
            ",".join(sorted({r[3] for r in by_flag.get(h, []) if r[3] != "activity_completion"})),
        ))
    n = tsv(
        os.path.join(OUT, "unlock-activity-flags.tsv"),
        ["flag_hash", "activity_hash", "activity_name", "place", "destination",
         "activity_type", "activity_level", "tier", "is_playlist", "is_matchmade",
         "total_refs_to_flag", "other_ref_kinds"],
        sorted(act_rows, key=lambda r: (r[3], r[4], r[6] or 0, r[2])),
    )
    dist = len({r[0] for r in act_rows})
    print(f"unlock-activity-flags.tsv     {n}  ({dist} distinct flags - a flag is per MISSION, "
          f"not per activity row)")

    # the gate table: what gates each book / location / vendor, with the flags resolved
    setter = defaultdict(list)
    for a in acts:
        if a.get("completionFlagHash"):
            setter[a["completionFlagHash"]].append(a.get("activityName", ""))

    def flag_lbl(h):
        s = sorted(set(setter.get(h, [])))
        return f"complete[{'/'.join(s)}]" if s else f"flag:{h}"

    gate_rows = []
    for b in rows(db, "DestinyDirectorBookDefinition"):
        for kind in ("isVisibleExpression", "isDefaultExpression"):
            st = (b.get(kind) or {}).get("steps", [])
            gate_rows.append(("director_book", b["bookHash"], b.get("bookName", "") or "(unnamed)",
                              kind, len(b.get("nodes", [])),
                              render(st, flag_label=flag_lbl) if st else "UNCONDITIONAL",
                              ",".join(str(s["flagHash"]) for s in st if s.get("flagHash"))))
    for rb in rows(db, "DestinyRecordBookDefinition"):
        st = (rb.get("bookAvailableUnlockExpression") or {}).get("steps", [])
        gate_rows.append(("record_book", rb.get("hash"), rb.get("displayName", ""),
                          "bookAvailableUnlockExpression", len(rb.get("pages", [])),
                          render(st, flag_label=flag_lbl) if st else "UNCONDITIONAL",
                          ",".join(str(s["flagHash"]) for s in st if s.get("flagHash"))))
    for lh, L in sorted(locs.items()):
        for i, lr in enumerate(L.get("locationReleases", [])):
            st = (lr.get("activeUnlockExpression") or {}).get("steps", [])
            if not st:
                continue
            gate_rows.append(("location_release", lh, L.get("contentIdentifier", ""),
                              f"release[{i}] -> book {lr.get('directorBookHash')}",
                              len(L.get("locationReleases", [])),
                              render(st, flag_label=flag_lbl),
                              ",".join(str(s["flagHash"]) for s in st if s.get("flagHash"))))
    for v in rows(db, "DestinyVendorDefinition"):
        s = v.get("summary", {})
        for field in ("unlockFlagHashes", "enabledUnlockFlagHashes"):
            hs = [h for h in s.get(field, []) if h]
            if hs:
                # ⚠ These are a bare ARRAY, not an expression: the manifest never says whether the
                # client ANDs or ORs them. Do not render a combiner we have not measured.
                gate_rows.append(("vendor", v.get("hash"), s.get("vendorName", ""), field, len(hs),
                                  "[combiner UNKNOWN] " + " ?? ".join(flag_lbl(h) for h in hs),
                                  ",".join(str(h) for h in hs)))
    n = tsv(
        os.path.join(OUT, "unlock-gates.tsv"),
        ["gated_thing", "owner_hash", "owner_name", "expression_field", "size",
         "gate_resolved", "flag_hashes"],
        gate_rows,
    )
    print(f"unlock-gates.tsv              {n}")

    val_rows = []
    for o in objs:
        if o.get("unlockValueHash"):
            val_rows.append((o["unlockValueHash"], "objective", o["hash"],
                             o.get("contentIdentifier", ""),
                             (o.get("displayDescription", "") or "").replace("\t", " ")[:100],
                             int(o["unlockValueHash"] == o["hash"])))
    for vh in step_vh:
        used = [(t, p, ow, nm) for t, p, ow, nm, st in nonempty
                if any(s.get("valueHash") == vh for s in st)]
        val_rows.append((vh, "expression_operand", "", "",
                         f"read by {len(used)} expressions, e.g. {used[0][0]}.{used[0][1]}", 0))
    n = tsv(
        os.path.join(OUT, "unlock-values.tsv"),
        ["unlock_value_hash", "source", "owner_hash", "content_identifier", "note",
         "value_hash_equals_owner_hash"],
        sorted(val_rows),
    )
    print(f"unlock-values.tsv             {n}  (a SECOND hash space - not the flag space)")

    n = tsv(
        os.path.join(OUT, "unlock-flag-index-candidates.tsv"),
        ["flag_hash", "value", "occurrences", "populated_in", "label"],
        [(h, v, idx_n[h], ",".join(sorted(idx_where[h])), label(h))
         for h, v in sorted(idx.items(), key=lambda kv: kv[1])],
    )
    print(f"unlock-flag-index-candidates.tsv {n}  (the only flags whose `value` is populated)")

    # Every field in the manifest whose NAME says "unlock", with how many rows carry a non-zero
    # value. The rows that read 0-of-N are the shape of what the public API withholds - and the
    # rows that do NOT read 0-of-N are the paired positive control that proves the reader works.
    STRIPPED = [
        ("DestinyGrimoireCardDefinition", "unlockFlagHash",
         lambda o: [o.get("unlockFlagHash", 0)]),
        ("DestinyGrimoireCardDefinition", "statisticCollection[].rankCollection[].unlockFlagHash",
         lambda o: [r.get("unlockFlagHash", 0) for s in o.get("statisticCollection", [])
                    for r in s.get("rankCollection", [])]),
        ("DestinyInventoryItemDefinition", "bountyResetUnlockHash",
         lambda o: [o.get("bountyResetUnlockHash", 0)]),
        ("DestinyInventoryItemDefinition", "questTrackingUnlockValueHash",
         lambda o: [o.get("questTrackingUnlockValueHash", 0)]),
        ("DestinyVendorDefinition", "sales[].licenseUnlockHash",
         lambda o: [s.get("licenseUnlockHash", 0) for s in o.get("sales", [])]),
        ("DestinyVendorDefinition", "unlockValueHash", lambda o: [o.get("unlockValueHash", 0)]),
        ("DestinySpecialEventDefinition", "unlockEventHash", lambda o: [o.get("unlockEventHash", 0)]),
        ("DestinySpecialEventDefinition", "activeUnlockValueHash",
         lambda o: [o.get("activeUnlockValueHash", 0)]),
        ("DestinyTriumphSetDefinition", "lockdownUnlockHash", lambda o: [o.get("lockdownUnlockHash", 0)]),
        ("DestinyBondDefinition", "providedUnlockValueHash",
         lambda o: [o.get("providedUnlockValueHash", 0)]),
        ("DestinyBondDefinition", "providedUnlockHash", lambda o: [o.get("providedUnlockHash", 0)]),
        ("DestinyObjectiveDefinition", "unlockValueHash", lambda o: [o.get("unlockValueHash", 0)]),
        ("DestinyActivityDefinition", "completionFlagHash", lambda o: [o.get("completionFlagHash", 0)]),
    ]
    strip_rows = []
    for table, field, get in STRIPPED:
        vals = [v for o in rows(db, table) for v in get(o)]
        nz = [v for v in vals if v]
        strip_rows.append((table, field, len(vals), len(nz), len(set(nz)),
                           "STRIPPED - every value is 0" if not nz else "populated"))
    n = tsv(
        os.path.join(OUT, "unlock-stripped-fields.tsv"),
        ["table", "field", "rows", "non_zero", "distinct_non_zero", "verdict"],
        strip_rows,
    )
    stripped_n = sum(1 for r in strip_rows if r[3] == 0)
    print(f"unlock-stripped-fields.tsv    {n}  ({stripped_n} of {n} unlock fields are all-zero "
          f"in the public manifest)")

    hits = blind_walk_hits(db, tables, flags)
    typed_paths = {(r[1], r[2]) for r in refs}
    coll_rows = []
    for (t, p), vs in sorted(hits.items(), key=lambda x: -len(x[1])):
        is_typed = (t, p) in typed_paths
        coll_rows.append((t, p, len(vs), len(set(vs)), int(is_typed),
                          "typed reference" if is_typed else "HASH COLLISION - not a reference",
                          ",".join(str(x) for x in sorted(set(vs))[:6])))
    n = tsv(
        os.path.join(OUT, "unlock-hash-collisions.tsv"),
        ["table", "json_path", "blind_walk_hits", "distinct_hashes", "is_typed_reference",
         "verdict", "sample_hashes"],
        coll_rows,
    )
    print(f"unlock-hash-collisions.tsv    {n}")

    with open(os.path.join(OUT, "unlock-flag-reference.json"), "w") as f:
        json.dump(
            {
                "source": os.path.relpath(MANIFEST, REPO),
                "reach": "static-client-inference (public 2020 manifest, NOT our 01.29 client)",
                "operators": {str(k): {"name": OP_NAME[k], "arity": ARITY[k],
                                       "occurrences": hist[k]} for k in sorted(ARITY)},
                "counts": {
                    "flags": len(flags), "referenced": len(typed_flags),
                    "unreferenced": len(flags) - len(typed_flags),
                    "expressions": len(exprs), "expressions_non_empty": len(nonempty),
                    "sandbox_perk_collisions": len(collide),
                },
                "flags": catalogue,
            },
            f,
            indent=1,
        )
    print("unlock-flag-reference.json    written")
    return 0


if __name__ == "__main__":
    sys.exit(main())
