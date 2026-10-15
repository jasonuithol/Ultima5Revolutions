//Export memory map, function list, decompiled C and disassembly listing.
//@category U5
import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.*;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.*;
import ghidra.program.model.symbol.*;
import java.io.*;
import java.util.*;

public class ExportAll extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        String outDir = args.length > 0 ? args[0] : "/tmp/ghidra_export";
        new File(outDir).mkdirs();
        String base = currentProgram.getName().replace('.', '_');

        // Memory map
        try (PrintWriter pw = new PrintWriter(new FileWriter(outDir + "/" + base + "_memmap.txt"))) {
            for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
                pw.printf("%s %s-%s size=%d init=%b exec=%b write=%b\n", b.getName(), b.getStart(), b.getEnd(), b.getSize(), b.isInitialized(), b.isExecute(), b.isWrite());
            }
        }

        // Functions + decompile
        DecompInterface ifc = new DecompInterface();
        DecompileOptions opts = new DecompileOptions();
        ifc.setOptions(opts);
        ifc.openProgram(currentProgram);
        FunctionManager fm = currentProgram.getFunctionManager();
        try (PrintWriter flist = new PrintWriter(new FileWriter(outDir + "/" + base + "_functions.txt"));
             PrintWriter dec = new PrintWriter(new FileWriter(outDir + "/" + base + "_decompiled.c"))) {
            for (Function f : fm.getFunctions(true)) {
                long size = f.getBody().getNumAddresses();
                Set<String> callers = new TreeSet<>();
                for (Function c : f.getCallingFunctions(monitor)) callers.add(c.getName());
                Set<String> callees = new TreeSet<>();
                for (Function c : f.getCalledFunctions(monitor)) callees.add(c.getName());
                flist.printf("%s %s size=%d callers=%s callees=%s proto=%s\n", f.getEntryPoint(), f.getName(), size, callers, callees, f.getPrototypeString(false, false).replace("\n"," "));
                DecompileResults r = ifc.decompileFunction(f, 60, monitor);
                dec.printf("\n// ===== FUNCTION %s @ %s size=%d =====\n", f.getName(), f.getEntryPoint(), size);
                if (r != null && r.decompileCompleted() && r.getDecompiledFunction() != null) {
                    dec.print(r.getDecompiledFunction().getC());
                } else {
                    dec.printf("// DECOMPILE FAILED: %s\n", r == null ? "null" : r.getErrorMessage());
                }
            }
        }
        ifc.dispose();

        // Listing
        Listing listing = currentProgram.getListing();
        try (PrintWriter pw = new PrintWriter(new FileWriter(outDir + "/" + base + "_listing.asm"))) {
            SymbolTable st = currentProgram.getSymbolTable();
            ReferenceManager rm = currentProgram.getReferenceManager();
            for (CodeUnit cu : listing.getCodeUnits(true)) {
                Address a = cu.getMinAddress();
                MemoryBlock mb = currentProgram.getMemory().getBlock(a);
                if (mb == null || !mb.isInitialized()) continue;
                if (mb.getName().startsWith("DGROUP")) continue;
                Symbol[] syms = st.getSymbols(a);
                for (Symbol s : syms) pw.printf("%s:\n", s.getName());
                StringBuilder bytes = new StringBuilder();
                try { for (byte b : cu.getBytes()) bytes.append(String.format("%02x", b & 0xff)); } catch (Exception e) {}
                String refs = "";
                for (Reference ref : rm.getReferencesFrom(a)) {
                    if (ref.isMemoryReference()) {
                        Symbol s = st.getPrimarySymbol(ref.getToAddress());
                        if (s != null) refs += " ;-> " + s.getName();
                    }
                }
                pw.printf("%-12s %-20s %s%s\n", a, bytes, cu.toString(), refs);
            }
        }
        println("Exported to " + outDir);
    }
}
