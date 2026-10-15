//Prescript for a .DRV raw binary: create functions at the 38 jump-table entries; set DS.
//@category U5
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.*;
import ghidra.program.model.listing.*;
import ghidra.program.model.mem.*;
import ghidra.program.model.symbol.*;
import ghidra.program.model.lang.*;
import ghidra.app.cmd.disassemble.DisassembleCommand;
import java.math.BigInteger;

public class DrvSetup extends GhidraScript {
    @Override
    public void run() throws Exception {
        Memory mem = currentProgram.getMemory();
        MemoryBlock b = mem.getBlocks()[0];
        b.setExecute(true); b.setWrite(true); b.setRead(true);
        Address base = b.getStart();
        ProgramContext ctx = currentProgram.getProgramContext();
        // DS: unknown at this point; drivers mostly use CS-relative data. Leave DS unset.
        FunctionManager fm = currentProgram.getFunctionManager();
        for (int i = 0; i < 38; i++) {
            Address a = base.add(i * 3);
            new DisassembleCommand(a, new AddressSet(a, a.add(2)), false).applyTo(currentProgram, monitor);
            Instruction ins = currentProgram.getListing().getInstructionAt(a);
            if (ins == null) continue;
            Address tgt = ins.getFlows().length > 0 ? ins.getFlows()[0] : null;
            if (tgt == null) continue;
            new DisassembleCommand(tgt, null, true).applyTo(currentProgram, monitor);
            Function f = fm.getFunctionAt(tgt);
            if (f == null) f = createFunction(tgt, String.format("drv_%02d_%04x", i, tgt.getOffset() - base.getOffset()));
            else f.setName(String.format("drv_%02d_%04x", i, tgt.getOffset() - base.getOffset()), SourceType.USER_DEFINED);
            Function jf = fm.getFunctionAt(a);
            if (jf == null) jf = createFunction(a, String.format("jmp_%02d", i));
            addEntryPoint(tgt);
        }
        println("DRV setup done");
    }
}
