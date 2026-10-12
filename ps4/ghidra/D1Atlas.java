/* D1Atlas - seed Ghidra from our own eboot atlas.
 *
 * Ghidra must NOT start from zero on this image: we already know 47,306 function
 * starts, 2,633 RTTI classes with vtables, and 638 resolved PS4 imports.  This
 * script pours all of that in.
 *
 * Phases
 *   1  disassemble at every atlas function start (one batched command, flow-following)
 *   2  create a function at every atlas start
 *   3  name PLT stubs from eboot-imports.tsv        (so calls read `memcpy(...)`)
 *   4  name functions from eboot-atlas.tsv          -- STRONG name_source classes only
 *   5  label typeinfo + vtable from eboot-types.tsv
 *   6  set the ELF entry point
 *
 * ⚠ NAME PROVENANCE - the reason phase 4 filters.
 *   `name_source` is the atlas's own statement of how the name was obtained.  Only
 *   these are identities:
 *       rtti         function found in a vtable of a class with intact RTTI
 *       rtti-vptr    function stores a known class vptr (ctor/dtor)
 *       source-file  __FILE__ literal referenced by the function
 *       schema-tag   descriptor-table tag referenced by the function
 *   These are NOT identities and are skipped by default:
 *       string-ref   "this function mentions that string" -- 2,511 rows, weakest class
 *       import       "this function calls sceFoo" -- a caller, not the callee
 *       name-hash    "this function mentions that FNV-1 hash"
 *   ⚠ The `~` prefix is NOT a reliable discriminator: string-ref, import, schema-tag
 *   and name-hash rows are ALL `~`-prefixed.  Filter on name_source, never on `~`.
 *   Names imported by this script get SourceType.IMPORTED; Ghidra's own guesses stay
 *   ANALYSIS, so you can always tell them apart in the GUI.
 *
 * Usage (args are positional, all optional; defaults are the repo paths):
 *   -postScript D1Atlas.java <atlas.tsv> <types.tsv> <imports.tsv> [strong,classes,csv]
 *
 * @category D1
 */
import java.io.BufferedReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.listing.Function;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.SourceType;

public class D1Atlas extends GhidraScript {

    // Repo root, derived at runtime. A hardcoded path broke once when a scrub pass
    // replaced an absolute home path with the literal string "$HOME", which Java does
    // not expand: every input file then silently failed to resolve.
    private static String repoRoot() {
        String env = System.getenv("D1_REPO");
        if (env != null && !env.isEmpty()) return env;
        return System.getProperty("user.home") + "/dev/destiny1-preservation";
    }

    private static final String REPO = repoRoot();

    private String atlasPath  = REPO + "/capture-rig/eboot-atlas.tsv";
    private String typesPath  = REPO + "/capture-rig/eboot-types.tsv";
    private String importPath = REPO + "/capture-rig/eboot-imports.tsv";
    private Set<String> strong = new HashSet<>(List.of("rtti", "rtti-vptr", "source-file", "schema-tag"));

    private long t0;

    private void lap(String what) {
        long now = System.currentTimeMillis();
        println(String.format("[%6.1fs] %s", (now - t0) / 1000.0, what));
    }

    @Override
    public void run() throws Exception {
        t0 = System.currentTimeMillis();
        String[] a = getScriptArgs();
        if (a.length > 0 && !a[0].isEmpty()) atlasPath = a[0];
        if (a.length > 1 && !a[1].isEmpty()) typesPath = a[1];
        if (a.length > 2 && !a[2].isEmpty()) importPath = a[2];
        if (a.length > 3 && !a[3].isEmpty()) {
            strong = new HashSet<>(List.of(a[3].split(",")));
        }
        println("D1Atlas: atlas=" + atlasPath);
        println("D1Atlas: strong name_source classes = " + strong);

        MemoryBlock text = null;
        for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
            if (b.isExecute() && b.isInitialized()) { text = b; break; }
        }
        if (text == null) throw new Exception("no executable initialised block");
        println("D1Atlas: text block " + text.getName() + " " + text.getStart() + ".." + text.getEnd());
        AddressSet textSet = new AddressSet(text.getStart(), text.getEnd());

        // ---- read the atlas ------------------------------------------------
        List<String[]> rows = readTsv(atlasPath);
        String[] hdr = rows.remove(0);
        Map<String, Integer> col = new HashMap<>();
        for (int i = 0; i < hdr.length; i++) col.put(hdr[i], i);
        for (String need : new String[] { "vaddr", "anchored", "name", "name_source", "class", "vslot", "source_file" }) {
            if (!col.containsKey(need)) throw new Exception("atlas is missing column '" + need + "'");
        }
        lap("read atlas: " + rows.size() + " rows");

        // ---- phase 1: disassemble ------------------------------------------
        AddressSet starts = new AddressSet();
        int outOfText = 0;
        for (String[] r : rows) {
            Address ad = toAddr(Long.decode(get(r, col, "vaddr")));
            if (!textSet.contains(ad)) { outOfText++; continue; }
            starts.addRange(ad, ad);
        }
        println("D1Atlas: " + starts.getNumAddresses() + " starts inside text (" + outOfText + " outside, skipped)");
        DisassembleCommand dis = new DisassembleCommand(starts, textSet, true);
        dis.applyTo(currentProgram, monitor);
        lap("phase 1 disassemble: " + currentProgram.getListing().getNumCodeUnits() + " code units");

        // ---- phase 2: create functions -------------------------------------
        int made = 0, existed = 0, failed = 0;
        for (Address ad : starts.getAddresses(true)) {
            if (monitor.isCancelled()) break;
            Function f = getFunctionAt(ad);
            if (f != null) { existed++; continue; }
            try {
                f = createFunction(ad, null);
                if (f != null) made++; else failed++;
            } catch (Exception e) {
                failed++;
            }
        }
        lap("phase 2 create functions: made=" + made + " existed=" + existed + " failed=" + failed
            + " total=" + currentProgram.getFunctionManager().getFunctionCount());

        // ---- phase 3: PLT stubs from imports --------------------------------
        int impNamed = 0, impMissing = 0;
        try {
            List<String[]> irows = readTsv(importPath);
            String[] ih = irows.remove(0);
            Map<String, Integer> ic = new HashMap<>();
            for (int i = 0; i < ih.length; i++) ic.put(ih[i], i);
            for (String[] r : irows) {
                String stub = get(r, ic, "plt_stub");
                String nm = get(r, ic, "name");
                String nid = get(r, ic, "nid");
                if (stub.isEmpty()) continue;
                if (nm.isEmpty()) nm = "nid_" + nid;
                Address ad = toAddr(Long.decode(stub));
                Function f = getFunctionAt(ad);
                if (f == null) {
                    try { f = createFunction(ad, null); } catch (Exception e) { /* fall through */ }
                }
                if (f == null) { impMissing++; continue; }
                if (setFuncName(f, nm, ad)) impNamed++;
                String got = get(r, ic, "got_slot");
                if (!got.isEmpty()) {
                    try { createLabel(toAddr(Long.decode(got)), "GOT_" + sanitize(nm), true, SourceType.IMPORTED); }
                    catch (Exception e) { /* ignore */ }
                }
                setPlateComment(ad, "PS4 import stub\nNID  : " + nid + "\nname : " + nm);
            }
        } catch (Exception e) {
            println("D1Atlas: WARNING imports phase failed: " + e);
        }
        lap("phase 3 imports: named=" + impNamed + " unplaceable=" + impMissing);

        // ---- phase 4: atlas names -------------------------------------------
        // ⚠ Atlas names are NOT unique: 9,749 strong rows carry only 6,761 distinct names
        // (549 functions are called `cui::c_property_value::c_typed_holder<bool>::ctor_or_dtor`).
        // Ghidra happily accepts duplicates, and then a call in the decompiled C names a
        // function you cannot navigate to.  So any name used more than once gets its address
        // appended.  Unique names stay clean.
        Map<String, Integer> nameUses = new HashMap<>();
        for (String[] r : rows) {
            String src = get(r, col, "name_source");
            String nm = get(r, col, "name");
            if (!nm.isEmpty() && strong.contains(src)) nameUses.merge(nm, 1, Integer::sum);
        }
        int ambiguous = 0;
        for (Map.Entry<String, Integer> e : nameUses.entrySet()) if (e.getValue() > 1) ambiguous++;
        println("D1Atlas: " + nameUses.size() + " distinct strong names; " + ambiguous
            + " are used by more than one function and will carry an address suffix");

        int named = 0, skippedWeak = 0, skippedUnnamed = 0, noFunc = 0, dupSuffixed = 0, nameFail = 0;
        Map<String, Integer> perSource = new HashMap<>();
        for (String[] r : rows) {
            if (monitor.isCancelled()) break;
            String src = get(r, col, "name_source");
            String nm = get(r, col, "name");
            String vaddr = get(r, col, "vaddr");
            Address ad = toAddr(Long.decode(vaddr));
            if (!textSet.contains(ad)) continue;
            Function f = getFunctionAt(ad);

            // Every row, strong or weak, gets a plate comment: that is provenance,
            // not identity, and it costs nothing to keep the weak evidence visible.
            StringBuilder pc = new StringBuilder();
            pc.append("atlas ").append(vaddr).append("  anchored=").append(get(r, col, "anchored"));
            if (!nm.isEmpty()) pc.append("\nname        : ").append(nm).append("   [").append(src).append("]");
            appendIf(pc, "class       : ", get(r, col, "class"));
            appendIf(pc, "vslot       : ", get(r, col, "vslot"));
            appendIf(pc, "source_file : ", get(r, col, "source_file"));
            appendIf(pc, "subsystem   : ", get(r, col, "subsystem"));
            appendIf(pc, "tags        : ", get(r, col, "tags"));
            appendIf(pc, "strings     : ", truncate(get(r, col, "strings"), 400));
            appendIf(pc, "warn        : ", get(r, col, "warn"));
            if (!strong.contains(src) && !nm.isEmpty()) {
                pc.append("\n!! name_source '").append(src).append("' is WEAK -- not applied as an identity.");
            }
            try { setPlateComment(ad, pc.toString()); } catch (Exception e) { /* ignore */ }

            if (nm.isEmpty()) { skippedUnnamed++; continue; }
            if (!strong.contains(src)) { skippedWeak++; continue; }
            if (f == null) { noFunc++; continue; }
            int before = named;
            String applied = nm;
            if (nameUses.getOrDefault(nm, 0) > 1) { applied = nm + "_" + vaddr.replace("0x", ""); dupSuffixed++; }
            if (setFuncName(f, applied, ad)) {
                named++;
                perSource.merge(src, 1, Integer::sum);
            } else {
                nameFail++;
            }
            if (named != before && (named % 2000) == 0) lap("  ... named " + named);
        }
        lap("phase 4 names: applied=" + named + " " + perSource
            + " | skipped_weak=" + skippedWeak + " skipped_unnamed=" + skippedUnnamed
            + " no_function=" + noFunc + " dup_suffixed=" + dupSuffixed + " name_failed=" + nameFail);

        // ---- phase 5: RTTI types --------------------------------------------
        int tiLabels = 0, vtLabels = 0;
        try {
            List<String[]> trows = readTsv(typesPath);
            String[] th = trows.remove(0);
            Map<String, Integer> tc = new HashMap<>();
            for (int i = 0; i < th.length; i++) tc.put(th[i], i);
            for (String[] r : trows) {
                String nm = get(r, tc, "name");
                if (nm.isEmpty()) continue;
                String ti = get(r, tc, "typeinfo");
                if (!ti.isEmpty()) {
                    try {
                        createLabel(toAddr(Long.decode(ti)), "typeinfo_" + sanitize(nm), true, SourceType.IMPORTED);
                        setPlateComment(toAddr(Long.decode(ti)),
                            "RTTI typeinfo\nclass   : " + nm + "\nkind    : " + get(r, tc, "kind")
                            + "\nbases   : " + get(r, tc, "bases") + "\nmangled : " + get(r, tc, "mangled"));
                        tiLabels++;
                    } catch (Exception e) { /* ignore */ }
                }
                for (String vt : get(r, tc, "vtables").split(",")) {
                    if (vt.isBlank()) continue;
                    try {
                        Address va = toAddr(Long.decode(vt.trim()));
                        createLabel(va, "vtable_" + sanitize(nm), true, SourceType.IMPORTED);
                        setPlateComment(va, "vtable for " + nm + "  (" + get(r, tc, "slots") + " slots)");
                        vtLabels++;
                    } catch (Exception e) { /* ignore */ }
                }
            }
        } catch (Exception e) {
            println("D1Atlas: WARNING types phase failed: " + e);
        }
        lap("phase 5 types: typeinfo=" + tiLabels + " vtables=" + vtLabels);

        // ---- phase 6: entry point --------------------------------------------
        Address ep = toAddr(0xc2b470L);
        try {
            currentProgram.getSymbolTable().addExternalEntryPoint(ep);
            Function f = getFunctionAt(ep);
            if (f == null) f = createFunction(ep, null);
            if (f != null) setFuncName(f, "_ELF_entry", ep);
        } catch (Exception e) {
            println("D1Atlas: WARNING entry point: " + e);
        }
        lap("phase 6 entry point set at " + ep);

        println("D1Atlas: DONE  functions=" + currentProgram.getFunctionManager().getFunctionCount()
            + " symbols=" + currentProgram.getSymbolTable().getNumSymbols());
    }

    // -- helpers -----------------------------------------------------------

    private boolean setFuncName(Function f, String raw, Address ad) {
        String nm = sanitize(raw);
        if (nm.isEmpty()) return false;
        try {
            f.setName(nm, SourceType.IMPORTED);
            return true;
        } catch (Exception e) {
            try {
                f.setName(nm + "_" + ad.toString(), SourceType.IMPORTED);
                return true;
            } catch (Exception e2) {
                return false;
            }
        }
    }

    /** Ghidra rejects whitespace and a few control characters in symbol names. */
    private static String sanitize(String s) {
        StringBuilder sb = new StringBuilder(s.length());
        for (char c : s.toCharArray()) {
            if (c < 0x21 || c == 0x7f) sb.append('_');
            else sb.append(c);
        }
        return sb.toString();
    }

    private static void appendIf(StringBuilder sb, String label, String v) {
        if (v != null && !v.isEmpty()) sb.append('\n').append(label).append(v);
    }

    private static String truncate(String s, int n) {
        return (s != null && s.length() > n) ? s.substring(0, n) + " ..." : s;
    }

    private static String get(String[] r, Map<String, Integer> col, String name) {
        Integer i = col.get(name);
        if (i == null || i >= r.length || r[i] == null) return "";
        return r[i];
    }

    private static List<String[]> readTsv(String path) throws Exception {
        List<String[]> out = new ArrayList<>();
        try (BufferedReader br = Files.newBufferedReader(Paths.get(path), StandardCharsets.UTF_8)) {
            String line;
            while ((line = br.readLine()) != null) {
                if (line.isEmpty()) continue;
                out.add(line.split("\t", -1));
            }
        }
        return out;
    }
}
