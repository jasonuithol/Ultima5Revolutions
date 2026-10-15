//Prescript: lay out memory for ULTIMA.EXE + one overlay (or ROOT), load DATA.OVL, set DS, name overlay vectors.
//@category U5
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.lang.*;
import ghidra.program.model.data.*;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import java.io.*;
import java.nio.file.*;
import java.util.*;
import java.math.BigInteger;

public class SetupOverlay extends GhidraScript {
    static final String GAMEDIR = "/home/jason/Projects/Ultima 5/";
    static final String[] NAMES = {"TOWN","MAINOUT","DUNGEON","INTRO","FLAMES","NPC","COMBAT","BLCKTHRN","LOOKOBJ","DNGLOOK","OUTSUBS","SHOPPES","ENDGAME","SJOG","CMDS","CAST","TALK","CAST2","ZSTATS","COMSUBS","SHOPPES2","SHOPPES3","FONT","DATA"};
    static final int[] START = {0x081d,0x081d,0x081d,0x081d,0x0a29,0x0a29,0x0a29,0x0a29,0x0a29,0x0a29,0x0a29,0x0a29,0x0a29,0x0bf8,0x0bf8,0x0bf8,0x0bf8,0x0e1e,0x0e1e,0x0e1e,0x0e1e,0x0e1e,0x0e1e,0x0f64};
    static final int[] END   = {0x09a4,0x09e8,0x0a12,0x0a29,0x0a2b,0x0b5c,0x0bf8,0x0af0,0x0b46,0x0b64,0x0ac3,0x0b9c,0x0ad8,0x0e1e,0x0dc9,0x0e0f,0x0d29,0x0f3a,0x0f4f,0x0f64,0x0ed0,0x0ebc,0x0f08,0x1bb8};
    static final int BASE = 0x1000;
    static final int[] NRELOC = {0,0,0,2,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,3};
    void applyRelocs(Memory mem, byte[] file, int n) throws Exception {
        for (int i = 0; i < n; i++) {
            int off = (file[i*4]&0xff) | ((file[i*4+1]&0xff)<<8);
            int sg  = (file[i*4+2]&0xff) | ((file[i*4+3]&0xff)<<8);
            if (off == 0 && sg == 0) continue;
            Address a = seg(sg + BASE, off);
            int v = mem.getShort(a) & 0xffff;
            mem.setShort(a, (short)(v + BASE));
            println(String.format("reloc %04x:%04x : %04x -> %04x", sg + BASE, off, v, (v + BASE) & 0xffff));
        }
    }
    static final int OVL_AREA_START = 0x181d, OVL_AREA_END = 0x1f64, DGROUP = 0x1f64, DATA_SIZE = 0xbd50, DGROUP_PARAS = 0x1bb8 - 0x0f64;

    Address seg(int segment, int off) {
        return currentProgram.getAddressFactory().getDefaultAddressSpace().getAddress((long)segment * 16 + off);
    }
    Address segAddr(int segment, int off) throws Exception {
        return currentProgram.getAddressFactory().getAddress(String.format("%04x:%04x", segment, off));
    }

    @Override
    public void run() throws Exception {
        String which = getScriptArgs().length > 0 ? getScriptArgs()[0] : "ROOT";
        int ovl = -1;
        for (int i = 0; i < NAMES.length; i++) if (NAMES[i].equals(which)) ovl = i;
        Memory mem = currentProgram.getMemory();

        // Remove uninitialized / placeholder blocks created by the MZ loader
        for (MemoryBlock b : mem.getBlocks()) {
            if (!b.isInitialized() || b.getName().equals("DATA")) { mem.removeBlock(b, monitor); }
        }
        if (ovl >= 0) {
            // startup code CODE_4 at 181d lives in the overlay area; drop it in overlay programs
            for (MemoryBlock b : mem.getBlocks()) if (b.getName().equals("CODE_4")) mem.removeBlock(b, monitor);
        }
        // DATA.OVL => DGROUP at 1f64:0000
        byte[] dataFile = Files.readAllBytes(Paths.get(GAMEDIR + "DATA.OVL"));
        int dataHdr = 16; // 3 relocation records, padded to a paragraph
        byte[] data = Arrays.copyOfRange(dataFile, dataHdr, dataFile.length);
        MemoryBlock db = mem.createInitializedBlock("DGROUP_DATA", segAddr(DGROUP, 0), new ByteArrayInputStream(data), data.length, monitor, false);
        db.setRead(true); db.setWrite(true); db.setExecute(false);
        MemoryBlock sb = mem.createUninitializedBlock("DGROUP_BSS_STACK", segAddr(DGROUP, data.length), DGROUP_PARAS * 16L - data.length, false);
        sb.setRead(true); sb.setWrite(true);
        MemoryBlock hb = mem.createUninitializedBlock("HEAP", segAddr(0x2bb8, 0), 0x40000L, false);
        hb.setRead(true); hb.setWrite(true);
        applyRelocs(mem, dataFile, 3);

        // Overlay area
        int ovlStartLin = OVL_AREA_START * 16, ovlEndLin = OVL_AREA_END * 16;
        if (ovl < 0) {
            // ROOT: CODE_4 (181d:0000-051f) stays; rest uninit
            MemoryBlock c4 = mem.getBlock("CODE_4");
            long from = c4 != null ? c4.getEnd().getOffset() + 1 : ovlStartLin;
            MemoryBlock u = mem.createUninitializedBlock("OVL_AREA", seg(0, (int) from), ovlEndLin - from, false);
            u.setExecute(true); u.setRead(true); u.setWrite(true);
        } else {
            int s = (START[ovl] + BASE) * 16, e = (END[ovl] + BASE) * 16;
            byte[] codeFile = Files.readAllBytes(Paths.get(GAMEDIR + NAMES[ovl] + ".OVL"));
            int nrel = NRELOC[ovl];
            int hdr = nrel > 0 ? ((nrel * 4) + 15) & ~15 : 0;
            byte[] code = Arrays.copyOfRange(codeFile, hdr, codeFile.length);
            int len = Math.min(code.length, e - s);
            if (s > ovlStartLin) { MemoryBlock u = mem.createUninitializedBlock("OVL_AREA_LO", seg(0, ovlStartLin), s - ovlStartLin, false); u.setExecute(true); u.setRead(true); u.setWrite(true);}
            MemoryBlock ob = mem.createInitializedBlock(NAMES[ovl] + "_OVL", seg(0, s), new ByteArrayInputStream(code, 0, len), len, monitor, false);
            ob.setExecute(true); ob.setRead(true); ob.setWrite(true);
            if (e < ovlEndLin) { MemoryBlock u = mem.createUninitializedBlock("OVL_AREA_HI", seg(0, e), ovlEndLin - e, false); u.setExecute(true); u.setRead(true); u.setWrite(true);}
            if (nrel > 0) applyRelocs(mem, codeFile, nrel);
        }

        // Default DS = DGROUP for every address
        ProgramContext ctx = currentProgram.getProgramContext();
        Register ds = ctx.getRegister("DS");
        ctx.setValue(ds, seg(BASE, 0), hb.getEnd(), BigInteger.valueOf(DGROUP));

        // Vectors: scan 1778 segment for CALLF 172e:02ec ; dw ovl ; JMPF seg:off
        MemoryBlock tbl = mem.getBlock(seg(0, 0x17780));
        byte[] t = new byte[(int) tbl.getSize()];
        mem.getBytes(tbl.getStart(), t);
        SymbolTable st = currentProgram.getSymbolTable();
        FunctionManager fm = currentProgram.getFunctionManager();
        Listing listing = currentProgram.getListing();
        int nvec = 0;
        try (PrintWriter pw = new PrintWriter(new FileWriter("/home/jason/Projects/Ultima5Revolutions/ghidra/vectors_" + which + ".txt"))) {
        for (int i = 0; i + 12 <= t.length; i++) {
            if ((t[i]&0xff)==0x9a && (t[i+1]&0xff)==0xec && (t[i+2]&0xff)==0x02 && (t[i+3]&0xff)==0x2e && (t[i+4]&0xff)==0x17 && (t[i+7]&0xff)==0xea) {
                int ovn = ((t[i+5]&0xff) | ((t[i+6]&0xff)<<8)) - 1;
                int toff = (t[i+8]&0xff) | ((t[i+9]&0xff)<<8);
                int tseg = (t[i+10]&0xff) | ((t[i+11]&0xff)<<8);
                Address v = tbl.getStart().add(i);
                try {
                Address v7 = v.add(7);
                Address target = segAddr(tseg, toff);
                String tname = String.format("%s_%04x", NAMES[ovn], toff);
                // disassemble CALLF, override fallthrough to skip the dw
                new DisassembleCommand(v, new AddressSet(v, v.add(4)), false).applyTo(currentProgram, monitor);
                Instruction ins = listing.getInstructionAt(v);
                if (ins != null) ins.setFallThrough(v7);
                listing.createData(v.add(5), WordDataType.dataType);
                new DisassembleCommand(v7, new AddressSet(v7, v7.add(4)), false).applyTo(currentProgram, monitor);
                Function vf = fm.getFunctionAt(v);
                if (vf == null) vf = fm.createFunction("vec_" + tname, v, new AddressSet(v, v.add(11)), SourceType.USER_DEFINED);
                else vf.setName("vec_" + tname, SourceType.USER_DEFINED);
                st.createLabel(v.add(5), "ovlnum_" + tname, SourceType.USER_DEFINED);
                if (ovn == ovl) {
                    new DisassembleCommand(target, null, true).applyTo(currentProgram, monitor);
                    Function tf = fm.getFunctionAt(target);
                    if (tf == null) tf = createFunction(target, tname);
                    if (tf != null) tf.setName(tname, SourceType.USER_DEFINED);
                    addEntryPoint(target);
                } else {
                    st.createLabel(target, tname, SourceType.USER_DEFINED);
                }
                pw.printf("%s %d %s %s\n", v, ovn, NAMES[ovn], target);
                nvec++;
                } catch (Exception ex) { println("vector error at " + v + ": " + ex); }
            }
        }
        }
        // main() and crt0 at the start of the code segment are not reached by any call (entered from the PLINK stub)
        for (int[] e : new int[][]{{0x0000}, {0x017e}}) {
            Address a = segAddr(BASE, e[0]);
            new DisassembleCommand(a, null, true).applyTo(currentProgram, monitor);
            if (fm.getFunctionAt(a) == null) createFunction(a, e[0] == 0 ? "main" : "crt0");
            addEntryPoint(a);
        }
        // name overlay manager entry
        Function om = fm.getFunctionAt(segAddr(0x172e, 0x02ec));
        if (om != null) { om.setName("plink_ovl_load", SourceType.USER_DEFINED); }
        else { Function f = createFunction(segAddr(0x172e, 0x02ec), "plink_ovl_load"); }
        println("Setup done for " + which + ", vectors=" + nvec);
    }
}
