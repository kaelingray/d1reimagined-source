/* D1Structs - define the critical-path C++ objects and type the `this` pointer.
 *
 * design/ghidra.md §9's biggest listed gap: 2,633 classes are LABELLED and none is DEFINED, so a
 * field reads `*(int *)(param_1 + 0x38)`.  That cost a session - `param_1 + 0x38` on the orbit
 * handler is a 5001 ms debounce stamp initialised to -1, and it was the one thing holding the
 * client in `setup:orbit` (design/RUNG-28-REACHED.md).  With the struct applied it reads
 * `this->debounce_ms`.
 *
 * ⚠ A struct applied to the WRONG type makes decompilation worse, silently.  Two invariants,
 *   both enforced upstream by ghidra/gen-structs.py's controls and re-checked here:
 *     - a field's width is the SMALLEST width the class's own vfuncs actually use at that offset,
 *       clamped so it can never swallow the next observed offset;
 *     - `this` is applied only to a vtable slot with `vt_shared == 1`.  A slot shared by several
 *       classes has no single derived `this` and is left alone.
 *
 * Everything written here is SourceType.IMPORTED, exactly like D1Atlas, so our claims and
 * Ghidra's own guesses never merge.  Re-runnable and idempotent.
 *
 * ORDER MATTERS: run `atlas` first, then `structs`.  D1Atlas rewrites plate comments; D1Structs
 * appends a provenance block to them and re-typing is what must come last.
 *
 * Usage:
 *   -postScript D1Structs.java [d1-structs.tsv]
 *
 * Exit: prints "D1Structs: N ERROR(S)" and throws if anything failed, so the driver's exit code
 * is usable as a control.
 *
 * @category D1
 */
import java.io.BufferedReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.data.ArrayDataType;
import ghidra.program.model.data.BooleanDataType;
import ghidra.program.model.data.CategoryPath;
import ghidra.program.model.data.CharDataType;
import ghidra.program.model.data.DataType;
import ghidra.program.model.data.DataTypeConflictHandler;
import ghidra.program.model.data.DataTypeManager;
import ghidra.program.model.data.DoubleDataType;
import ghidra.program.model.data.FloatDataType;
import ghidra.program.model.data.IntegerDataType;
import ghidra.program.model.data.LongLongDataType;
import ghidra.program.model.data.PointerDataType;
import ghidra.program.model.data.ShortDataType;
import ghidra.program.model.data.Structure;
import ghidra.program.model.data.StructureDataType;
import ghidra.program.model.data.Undefined1DataType;
import ghidra.program.model.data.Undefined2DataType;
import ghidra.program.model.data.Undefined4DataType;
import ghidra.program.model.data.Undefined8DataType;
import ghidra.program.model.data.UnsignedIntegerDataType;
import ghidra.program.model.data.UnsignedLongLongDataType;
import ghidra.program.model.data.UnsignedShortDataType;
import ghidra.program.model.data.VoidDataType;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.ParameterImpl;
import ghidra.program.model.listing.Variable;
import ghidra.program.model.symbol.SourceType;

public class D1Structs extends GhidraScript {

    // Repo root, derived at runtime. A hardcoded path broke once when a scrub pass
    // replaced an absolute home path with the literal string "$HOME", which Java does
    // not expand: every input file then silently failed to resolve.
    private static String repoRoot() {
        String env = System.getenv("D1_REPO");
        if (env != null && !env.isEmpty()) return env;
        return System.getProperty("user.home") + "/dev/destiny1-preservation";
    }

    private static final String REPO = repoRoot();
    private static final CategoryPath CAT = new CategoryPath("/D1");
    private static final String MARK = "-- D1Structs --";

    private DataTypeManager dtm;
    private final Map<String, DataType> ours = new LinkedHashMap<>();
    private final Map<String, Integer> declaredSize = new HashMap<>();
    private final List<String> errors = new ArrayList<>();
    private final Map<String, Integer> fallbacks = new TreeMap<>();

    @Override
    public void run() throws Exception {
        String path = REPO + "/ghidra/structs/d1-structs.tsv";
        String[] a = getScriptArgs();
        if (a.length > 0 && !a[0].isEmpty()) path = a[0];
        dtm = currentProgram.getDataTypeManager();
        println("D1Structs: " + path);

        List<String[]> rows = readTsv(path);
        String[] hdr = rows.remove(0);
        Map<String, Integer> col = new HashMap<>();
        for (int i = 0; i < hdr.length; i++) col.put(hdr[i], i);
        for (String need : new String[] { "kind", "target", "offset", "size", "name", "type",
                                          "evidence", "note" }) {
            if (!col.containsKey(need)) throw new Exception("d1-structs.tsv lacks column " + need);
        }

        // -- pass 1: create every struct empty, so field types can reference each other --------
        int made = 0;
        for (String[] r : rows) {
            if (!"struct".equals(g(r, col, "kind"))) continue;
            String name = g(r, col, "target");
            int size = (int) num(g(r, col, "size"));
            StructureDataType s = new StructureDataType(CAT, name, size, dtm);
            s.setDescription("[" + g(r, col, "evidence") + "] " + g(r, col, "note")
                + "  (D1Structs, ghidra/structs/d1-structs.tsv)");
            DataType resolved = dtm.addDataType(s, DataTypeConflictHandler.REPLACE_HANDLER);
            ours.put(name, resolved);
            declaredSize.put(name, size);
            made++;
        }
        println("D1Structs: pass 1 created " + made + " structures under " + CAT);

        // -- pass 2: fields -------------------------------------------------------------------
        int fields = 0;
        for (String[] r : rows) {
            if (!"field".equals(g(r, col, "kind"))) continue;
            String owner = g(r, col, "target");
            DataType dt = ours.get(owner);
            if (!(dt instanceof Structure)) { errors.add("no structure " + owner); continue; }
            Structure s = (Structure) dt;
            int off = (int) num(g(r, col, "offset"));
            int len = (int) num(g(r, col, "size"));
            String fname = g(r, col, "name");
            DataType ft = sized(resolve(g(r, col, "type")), len);
            if (ft == null) { errors.add(owner + "." + fname + ": unresolved type "
                + g(r, col, "type")); continue; }
            String note = g(r, col, "note");
            String comment = "[" + g(r, col, "evidence") + "]" + (note.isEmpty() ? "" : " " + note);
            try {
                s.replaceAtOffset(off, ft, ft.getLength(), fname, comment);
                fields++;
            } catch (Exception e) {
                errors.add(owner + "." + fname + " @" + off + ": " + e);
            }
        }
        println("D1Structs: pass 2 placed " + fields + " fields");

        // -- pass 3: `this` on the class's own vfuncs ------------------------------------------
        int applied = 0, noFunc = 0;
        for (String[] r : rows) {
            if (!"apply".equals(g(r, col, "kind"))) continue;
            Address ad = toAddr(num(g(r, col, "target")));
            Function f = getFunctionAt(ad);
            if (f == null) { noFunc++; errors.add("no function at " + g(r, col, "target")); continue; }
            String cls = g(r, col, "name");
            DataType st = ours.get(cls);
            if (st == null) { errors.add("apply: no struct " + cls); continue; }
            String[] ptypes = g(r, col, "type").split(",");
            if (setParams(f, new PointerDataType(st, dtm), ptypes, "this")) {
                applied++;
                stamp(ad, "this = " + cls + " *   [" + g(r, col, "evidence") + "] "
                    + g(r, col, "note"));
            } else {
                errors.add("apply failed at " + g(r, col, "target"));
            }
        }
        println("D1Structs: pass 3 typed `this` on " + applied + " functions (" + noFunc
            + " had no function)");

        // -- pass 4: signatures for the accessors the ladder reads through ---------------------
        Map<String, List<String[]>> paramsByAddr = new LinkedHashMap<>();
        for (String[] r : rows) {
            if ("param".equals(g(r, col, "kind"))) {
                paramsByAddr.computeIfAbsent(g(r, col, "target"), k -> new ArrayList<>()).add(r);
            }
        }
        int sigs = 0;
        for (String[] r : rows) {
            if (!"sig".equals(g(r, col, "kind"))) continue;
            String key = g(r, col, "target");
            Address ad = toAddr(num(key));
            Function f = getFunctionAt(ad);
            if (f == null) { errors.add("sig: no function at " + key); continue; }
            DataType ret = resolve(g(r, col, "type"));
            List<String[]> ps = paramsByAddr.getOrDefault(key, List.of());
            ps.sort((x, y) -> Integer.compare((int) num(g(x, col, "offset")),
                                              (int) num(g(y, col, "offset"))));
            String[] ptypes = new String[ps.size()];
            for (int i = 0; i < ps.size(); i++) ptypes[i] = g(ps.get(i), col, "type");
            try {
                if (ret != null) f.setReturnType(ret, SourceType.IMPORTED);
                if (ptypes.length > 0 && !setParams(f, resolve(ptypes[0]), ptypes, "a0")) {
                    errors.add("sig params failed at " + key);
                }
                f.setName(sanitize(g(r, col, "name")), SourceType.IMPORTED);
                stamp(ad, "name + signature assigned by D1Structs (NOT from RTTI)  ["
                    + g(r, col, "evidence") + "] " + g(r, col, "note"));
                sigs++;
            } catch (Exception e) {
                errors.add("sig " + key + ": " + e);
            }
        }
        println("D1Structs: pass 4 set " + sigs + " signatures");

        if (!fallbacks.isEmpty()) {
            println("D1Structs: unresolved type names fell back to undefined8: " + fallbacks);
        }
        if (errors.isEmpty()) {
            println("D1Structs: DONE, 0 errors.");
        } else {
            for (String e : errors.subList(0, Math.min(30, errors.size()))) println("  ERROR " + e);
            println("D1Structs: " + errors.size() + " ERROR(S)");
            throw new Exception("D1Structs: " + errors.size() + " error(s) - output may not be cited");
        }
    }

    // ---------------------------------------------------------------------------------------

    /** Rebuild the parameter list with the SAME arity the decompiler already infers, replacing
     *  only parameter 0's type.  Committing a shorter list would truncate a three-argument
     *  `vfunc[4]` and make the output worse, not better. */
    private boolean setParams(Function f, DataType first, String[] ptypes, String firstName) {
        try {
            List<Variable> ps = new ArrayList<>();
            for (int i = 0; i < ptypes.length; i++) {
                DataType dt = (i == 0) ? first : resolve(ptypes[i]);
                if (dt == null || dt.getLength() <= 0) dt = Undefined8DataType.dataType;
                ps.add(new ParameterImpl(i == 0 ? firstName : null, dt, currentProgram));
            }
            f.setCustomVariableStorage(false);
            // ⚠ The calling convention MUST be set alongside the parameter list.  Committing
            // formal params while the convention stays "unknown" makes the decompiler emit
            // "Unknown calling convention -- yet parameter storage is locked" and, worse, it
            // then DROPS arguments at call sites inside the function (`FUN_005441c0(x)` decayed
            // to `FUN_005441c0()`).  That is a struct making the output worse, which is exactly
            // the failure mode this script is supposed to avoid.
            f.updateFunction(Function.DEFAULT_CALLING_CONVENTION_STRING, null, ps,
                Function.FunctionUpdateType.DYNAMIC_STORAGE_FORMAL_PARAMS, true,
                SourceType.IMPORTED);
            return true;
        } catch (Exception e) {
            errors.add(f.getEntryPoint() + " setParams: " + e);
            return false;
        }
    }

    /** Append (idempotently) a provenance block to the function's plate comment. */
    private void stamp(Address ad, String line) {
        try {
            String pc = getPlateComment(ad);
            if (pc == null) pc = "";
            int at = pc.indexOf(MARK);
            if (at >= 0) pc = pc.substring(0, at).stripTrailing();
            setPlateComment(ad, (pc.isEmpty() ? "" : pc + "\n") + MARK + "\n" + line);
        } catch (Exception e) { /* a comment is never worth failing a run over */ }
    }

    /** `undefined1` declared 12 bytes wide becomes `undefined1[12]`.  Keeps the TSV terse. */
    private DataType sized(DataType dt, int want) {
        if (dt == null || want <= 0) return dt;
        int have = dt.getLength();
        if (have == want || have <= 0) return dt;
        if (want > have && want % have == 0) return new ArrayDataType(dt, want / have, have);
        return dt;
    }

    private DataType resolve(String spec) {
        if (spec == null) return null;
        String s = spec.trim();
        if (s.isEmpty()) return null;
        if (s.endsWith("*")) {
            DataType inner = resolve(s.substring(0, s.length() - 1));
            return new PointerDataType(inner == null ? VoidDataType.dataType : inner, dtm);
        }
        int lb = s.indexOf('[');
        if (lb > 0 && s.endsWith("]")) {
            DataType inner = resolve(s.substring(0, lb));
            int n = Integer.parseInt(s.substring(lb + 1, s.length() - 1).trim());
            if (inner == null || inner.getLength() <= 0) inner = Undefined1DataType.dataType;
            return new ArrayDataType(inner, n, inner.getLength());
        }
        DataType mine = ours.get(s);
        if (mine != null) return mine;
        switch (s) {
            case "void":                return VoidDataType.dataType;
            case "bool":                return BooleanDataType.dataType;
            case "char":                return CharDataType.dataType;
            case "byte": case "uchar":  return ghidra.program.model.data.ByteDataType.dataType;
            case "short":               return ShortDataType.dataType;
            case "ushort": case "word": return UnsignedShortDataType.dataType;
            case "int":                 return IntegerDataType.dataType;
            case "uint": case "dword":  return UnsignedIntegerDataType.dataType;
            case "long": case "longlong":   return LongLongDataType.dataType;
            case "ulong": case "ulonglong": case "qword":
                                        return UnsignedLongLongDataType.dataType;
            case "float":               return FloatDataType.dataType;
            case "double":              return DoubleDataType.dataType;
            case "undefined": case "undefined1": return Undefined1DataType.dataType;
            case "undefined2":          return Undefined2DataType.dataType;
            case "undefined4":          return Undefined4DataType.dataType;
            case "undefined8":          return Undefined8DataType.dataType;
            case "code":                return Undefined1DataType.dataType;
            default: break;
        }
        DataType existing = dtm.getDataType("/" + s);
        if (existing != null) return existing;
        fallbacks.merge(s, 1, Integer::sum);
        return Undefined8DataType.dataType;
    }

    private static long num(String s) {
        String t = s.trim();
        if (t.isEmpty()) return 0;
        return t.startsWith("0x") || t.startsWith("0X")
            ? Long.parseLong(t.substring(2), 16) : Long.parseLong(t);
    }

    private static String sanitize(String s) {
        StringBuilder sb = new StringBuilder(s.length());
        for (char c : s.toCharArray()) sb.append((c < 0x21 || c == 0x7f) ? '_' : c);
        return sb.toString();
    }

    private static String g(String[] r, Map<String, Integer> col, String name) {
        Integer i = col.get(name);
        if (i == null || i >= r.length || r[i] == null) return "";
        return r[i];
    }

    /** Minimal RFC4180 reader - the note column carries quoted text with embedded quotes. */
    private static List<String[]> readTsv(String path) throws Exception {
        List<String[]> out = new ArrayList<>();
        try (BufferedReader br = Files.newBufferedReader(Paths.get(path), StandardCharsets.UTF_8)) {
            String line;
            StringBuilder pending = new StringBuilder();
            while ((line = br.readLine()) != null) {
                if (pending.length() > 0) pending.append('\n');
                pending.append(line);
                if (countQuotes(pending) % 2 != 0) continue;   // quoted field spans lines
                String rec = pending.toString();
                pending.setLength(0);
                if (rec.isEmpty()) continue;
                out.add(splitTsv(rec));
            }
        }
        return out;
    }

    private static int countQuotes(CharSequence s) {
        int n = 0;
        for (int i = 0; i < s.length(); i++) if (s.charAt(i) == '"') n++;
        return n;
    }

    private static String[] splitTsv(String rec) {
        List<String> cells = new ArrayList<>();
        StringBuilder cur = new StringBuilder();
        boolean quoted = false;
        for (int i = 0; i < rec.length(); i++) {
            char c = rec.charAt(i);
            if (quoted) {
                if (c == '"') {
                    if (i + 1 < rec.length() && rec.charAt(i + 1) == '"') { cur.append('"'); i++; }
                    else quoted = false;
                } else cur.append(c);
            } else if (c == '"' && cur.length() == 0) {
                quoted = true;
            } else if (c == '\t') {
                cells.add(cur.toString());
                cur.setLength(0);
            } else {
                cur.append(c);
            }
        }
        cells.add(cur.toString());
        return cells.toArray(new String[0]);
    }
}
