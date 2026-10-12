/* D1AnalysisOptions - print and tune the auto-analysis option set.
 *
 * Ghidra's defaults assume a binary it must discover from scratch.  We hand it
 * 47,306 function starts, so the expensive discovery analyzers are pure cost and
 * the expensive *interpretation* analyzers (switch tables, stack, non-returning)
 * are what we actually want.  This script makes that choice explicit and logged
 * rather than implicit.
 *
 * Usage:
 *   -preScript D1AnalysisOptions.java              # print every option, change nothing
 *   -preScript D1AnalysisOptions.java tune         # print, then apply the tuning below
 *
 * @category D1
 */
import ghidra.app.script.GhidraScript;
import ghidra.framework.options.Options;
import ghidra.program.model.listing.Program;

public class D1AnalysisOptions extends GhidraScript {

    /** Turned OFF: discovery we do not need, or cost we cannot afford at 47k functions. */
    private static final String[] OFF = {
        "Decompiler Parameter ID",          // decompiles every function; hours on this image
        "Call Convention ID",               // ditto -- also decompiler-driven, 47k times
        "Aggressive Instruction Finder",    // guesses code in data; we have the atlas
        "Function Start Pre Search",        // ditto
        "Function Start Search",            // ditto
        "Function Start Search After Code", // ditto
        "Function Start Search After Data", // ditto
        "Function ID",                      // FID libraries do not cover a 2016 PS4 title
        "Embedded Media",
        "Create Address Tables",
        "Shared Return Calls",              // rewrites tail-call targets into thunks; our atlas
                                            // already records tail entry (ATLAS §4.2) and this
                                            // analyzer would silently move our boundaries
    };

    /** Kept ON explicitly: these are what make the decompiled C readable. */
    private static final String[] ON = {
        "Stack",
        "Decompiler Switch Analysis",
        "Non-Returning Functions - Discovered",
        "Non-Returning Functions - Known",
        "Reference",
        "Data Reference",
        "ASCII Strings",
        "Demangler GNU",
        "Subroutine References",
        "x86 Constant Reference Analyzer",
    };

    @Override
    public void run() throws Exception {
        Program p = currentProgram;
        Options o = p.getOptions(Program.ANALYSIS_PROPERTIES);
        boolean tune = getScriptArgs().length > 0 && "tune".equals(getScriptArgs()[0]);

        println("=== ANALYSIS OPTIONS (" + (tune ? "TUNING" : "listing only") + ") ===");
        for (String n : o.getOptionNames()) {
            println(String.format("  %-60s = %s", n, o.getValueAsString(n)));
        }
        if (!tune) return;

        println("=== APPLYING D1 TUNING ===");
        for (String n : OFF) {
            if (setIf(o, n, false)) println("  OFF  " + n);
            else println("  ??   " + n + "  (no such option in this Ghidra build)");
        }
        for (String n : ON) {
            if (setIf(o, n, true)) println("  ON   " + n);
            else println("  ??   " + n + "  (no such option in this Ghidra build)");
        }
    }

    private boolean setIf(Options o, String name, boolean v) {
        for (String n : o.getOptionNames()) {
            if (n.equals(name)) { o.setBoolean(n, v); return true; }
        }
        return false;
    }
}
