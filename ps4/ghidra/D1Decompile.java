/* D1Decompile - bulk decompiler driver.
 *
 * Writes one .c file per requested function and a decompile-index.tsv with per-function
 * timing, so "is bulk decompilation of 500 functions practical?" is answered by
 * measurement rather than by feel.
 *
 * Usage:
 *   -postScript D1Decompile.java <outDir> <addrSpec> [threads] [timeoutSec]
 *
 *   <addrSpec>  either a comma-separated list of hex addresses (0x34c010,0x260200)
 *               or the path to a file with one address per line (# comments allowed).
 *   [threads]   parallel decompiler processes, default 8.  Each is a separate native
 *               `decompile` child process; RSS is roughly 150-400 MB each on this image.
 *   [timeoutSec] per-function decompiler timeout, default 120.
 *
 * @category D1
 */
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicLong;

import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class D1Decompile extends GhidraScript {

    private record Job(Address addr, Function fn) {}
    private record Res(String addr, String name, long ms, int bytes, boolean ok, String err) {}

    @Override
    public void run() throws Exception {
        String[] a = getScriptArgs();
        if (a.length < 2) throw new Exception("usage: D1Decompile <outDir> <addrSpec> [threads] [timeoutSec]");
        Path outDir = Paths.get(a[0]);
        Files.createDirectories(outDir);
        int threads = a.length > 2 && !a[2].isEmpty() ? Integer.parseInt(a[2]) : 8;
        int timeout = a.length > 3 && !a[3].isEmpty() ? Integer.parseInt(a[3]) : 120;

        List<String> specs = new ArrayList<>();
        Path p = Paths.get(a[1]);
        if (Files.isRegularFile(p)) {
            for (String l : Files.readAllLines(p, StandardCharsets.UTF_8)) {
                l = l.trim();
                if (l.isEmpty() || l.startsWith("#")) continue;
                specs.add(l.split("\\s+")[0]);
            }
        } else {
            for (String s : a[1].split(",")) if (!s.isBlank()) specs.add(s.trim());
        }

        ConcurrentLinkedQueue<Job> queue = new ConcurrentLinkedQueue<>();
        List<String> missing = new ArrayList<>();
        for (String s : specs) {
            Address ad = toAddr(Long.decode(s));
            Function fn = getFunctionAt(ad);
            if (fn == null) fn = getFunctionContaining(ad);
            if (fn == null) { missing.add(s); continue; }
            queue.add(new Job(ad, fn));
        }
        println("D1Decompile: " + queue.size() + " functions, " + missing.size() + " with no function ("
            + String.join(",", missing.subList(0, Math.min(20, missing.size()))) + ")");
        println("D1Decompile: threads=" + threads + " timeout=" + timeout + "s out=" + outDir);

        ConcurrentLinkedQueue<Res> results = new ConcurrentLinkedQueue<>();
        AtomicInteger done = new AtomicInteger();
        AtomicLong totalMs = new AtomicLong();
        long wall0 = System.currentTimeMillis();
        int total = queue.size();

        List<Thread> pool = new ArrayList<>();
        for (int t = 0; t < threads; t++) {
            Thread th = new Thread(() -> {
                DecompInterface d = new DecompInterface();
                DecompileOptions opts = new DecompileOptions();
                d.setOptions(opts);
                d.toggleCCode(true);
                d.toggleSyntaxTree(true);
                d.setSimplificationStyle("decompile");
                if (!d.openProgram(currentProgram)) {
                    println("D1Decompile: openProgram FAILED: " + d.getLastMessage());
                    return;
                }
                try {
                    Job j;
                    while ((j = queue.poll()) != null) {
                        long s0 = System.nanoTime();
                        String c = null, err = null;
                        try {
                            DecompileResults r = d.decompileFunction(j.fn(), timeout, monitor);
                            if (r != null && r.decompileCompleted() && r.getDecompiledFunction() != null) {
                                c = r.getDecompiledFunction().getC();
                            } else {
                                err = (r == null) ? "null result" : String.valueOf(r.getErrorMessage());
                            }
                        } catch (Throwable e) {
                            err = e.toString();
                        }
                        long ms = (System.nanoTime() - s0) / 1_000_000L;
                        totalMs.addAndGet(ms);
                        String fname = String.format("%s_%s.c", j.addr().toString(), safe(j.fn().getName()));
                        int bytes = 0;
                        if (c != null) {
                            String hdr = "/* " + j.fn().getName() + "\n * entry   : 0x" + j.addr()
                                + "\n * body    : " + j.fn().getBody().getMinAddress() + " .. "
                                + j.fn().getBody().getMaxAddress()
                                + "\n * decompile ms : " + ms + "\n */\n";
                            byte[] b = (hdr + c).getBytes(StandardCharsets.UTF_8);
                            bytes = b.length;
                            try { Files.write(outDir.resolve(fname), b); } catch (Exception e) { err = e.toString(); }
                        }
                        results.add(new Res(j.addr().toString(), j.fn().getName(), ms, bytes, c != null, err));
                        int n = done.incrementAndGet();
                        if (n % 50 == 0 || n == total) {
                            println(String.format("D1Decompile: %d/%d  wall=%.1fs", n, total,
                                (System.currentTimeMillis() - wall0) / 1000.0));
                        }
                    }
                } finally {
                    d.dispose();
                }
            }, "decomp-" + t);
            th.start();
            pool.add(th);
        }
        for (Thread th : pool) th.join();

        double wall = (System.currentTimeMillis() - wall0) / 1000.0;
        List<Res> rs = new ArrayList<>(results);
        rs.sort((x, y) -> Long.compare(y.ms(), x.ms()));
        int ok = 0;
        for (Res r : rs) if (r.ok()) ok++;

        try (PrintWriter pw = new PrintWriter(Files.newBufferedWriter(
                outDir.resolve("decompile-index.tsv"), StandardCharsets.UTF_8))) {
            pw.println("addr\tname\tms\tbytes\tok\terror");
            for (Res r : rs) {
                pw.printf("%s\t%s\t%d\t%d\t%s\t%s%n", r.addr(), r.name(), r.ms(), r.bytes(), r.ok(),
                    r.err() == null ? "" : r.err().replace('\t', ' ').replace('\n', ' '));
            }
        }

        println("=== D1Decompile SUMMARY ===");
        println(String.format("  functions   : %d requested, %d decompiled, %d failed, %d had no function",
            specs.size(), ok, rs.size() - ok, missing.size()));
        println(String.format("  wall clock  : %.1f s with %d threads", wall, threads));
        println(String.format("  cpu time    : %.1f s summed over functions", totalMs.get() / 1000.0));
        if (!rs.isEmpty()) {
            long[] ms = rs.stream().mapToLong(Res::ms).sorted().toArray();
            println(String.format("  per function: min=%d ms  median=%d ms  p90=%d ms  max=%d ms  mean=%.0f ms",
                ms[0], ms[ms.length / 2], ms[(int) (ms.length * 0.9)], ms[ms.length - 1],
                (double) totalMs.get() / ms.length));
            println("  slowest:");
            for (int i = 0; i < Math.min(5, rs.size()); i++) {
                println(String.format("    %-10s %-60s %d ms", rs.get(i).addr(),
                    truncate(rs.get(i).name(), 60), rs.get(i).ms()));
            }
        }
        println("  index       : " + outDir.resolve("decompile-index.tsv"));
    }

    private static String truncate(String s, int n) { return s.length() > n ? s.substring(0, n) : s; }

    private static String safe(String s) {
        StringBuilder sb = new StringBuilder();
        for (char c : s.toCharArray()) {
            sb.append((Character.isLetterOrDigit(c) || c == '_' || c == '.') ? c : '_');
        }
        String r = sb.toString();
        return r.length() > 120 ? r.substring(0, 120) : r;
    }
}
