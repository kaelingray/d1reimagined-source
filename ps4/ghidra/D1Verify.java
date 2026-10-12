/* D1Verify - addressing positive control for the Destiny 1 eboot in Ghidra.
 *
 * Prints the memory map and reads back bytes at addresses whose contents we know
 * independently (straight out of the file, at fileoff = vaddr + 0x4000 for TEXT).
 * If a single byte disagrees the load is misconfigured and NOTHING in the database
 * may be cited.  Exits non-zero via an exception on mismatch.
 *
 * Run: analyzeHeadless <proj_loc> <proj> -process eboot.bin -noanalysis \
 *        -scriptPath <repo>/ghidra -postScript D1Verify.java
 *
 * @category D1
 */
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.mem.MemoryBlock;

public class D1Verify extends GhidraScript {

    // vaddr -> first bytes, taken from the raw file (see design/ghidra.md §2).
    private static final String[][] CONTROLS = {
        {"0x34c010", "5548 89e5 4157 4156 4155 4154 5350 4989"},  // command-queue pump
        {"0x260200", "5548 89e5 4157 4156 4155 4154 5348 81e4"},  // verdict function
        {"0x753540", "5548 89e5 4157 4156 4154 5348 81ec 7001"},  // change-world
        {"0xc2b470", "5548 89e5 4157 4156 5350 4989 f748 89fb"},  // ELF entry point
        {"0x260438", "41c6 4500 0141 8845 0141 c645 0200 488b"},  // header[0] = 1 store
    };

    @Override
    public void run() throws Exception {
        println("=== IMAGE ===");
        println("  name           : " + currentProgram.getName());
        println("  language       : " + currentProgram.getLanguageID());
        println("  compilerSpec   : " + currentProgram.getCompilerSpec().getCompilerSpecID());
        println("  imageBase      : " + currentProgram.getImageBase());
        println("  executableSHA  : " + currentProgram.getExecutableSHA256());
        Address ep = null;
        var eps = currentProgram.getSymbolTable().getExternalEntryPointIterator();
        if (eps.hasNext()) { ep = eps.next(); }
        println("  entryPoint     : " + ep);

        println("=== MEMORY MAP ===");
        for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
            println(String.format("  %-24s %s .. %s  size=0x%x  %s%s%s init=%s",
                b.getName(), b.getStart(), b.getEnd(), b.getSize(),
                b.isRead() ? "r" : "-", b.isWrite() ? "w" : "-", b.isExecute() ? "x" : "-",
                b.isInitialized()));
        }

        println("=== BYTE CONTROLS ===");
        int bad = 0;
        for (String[] c : CONTROLS) {
            Address a = toAddr(Long.decode(c[0]));
            String want = c[1].replace(" ", "").toLowerCase();
            byte[] got = new byte[want.length() / 2];
            String gotHex;
            try {
                currentProgram.getMemory().getBytes(a, got);
                StringBuilder sb = new StringBuilder();
                for (byte x : got) sb.append(String.format("%02x", x));
                gotHex = sb.toString();
            } catch (Exception e) {
                gotHex = "<UNREADABLE: " + e.getMessage() + ">";
            }
            boolean ok = gotHex.equals(want);
            if (!ok) bad++;
            println(String.format("  %-10s %s  want=%s got=%s", c[0], ok ? "PASS" : "**FAIL**", want, gotHex));
        }

        println("=== COUNTS ===");
        println("  functions      : " + currentProgram.getFunctionManager().getFunctionCount());
        println("  defined data   : " + currentProgram.getListing().getNumDefinedData());
        println("  symbols        : " + currentProgram.getSymbolTable().getNumSymbols());

        if (bad != 0) {
            throw new Exception("D1Verify: " + bad + " byte control(s) FAILED - the load is "
                + "misconfigured; do not cite anything from this database.");
        }
        println("D1Verify: all " + CONTROLS.length + " byte controls PASS.");
    }
}
