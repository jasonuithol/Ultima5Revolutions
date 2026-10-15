//! Populate the emulated memory from the original game files at their original addresses.
use crate::mem::{Mem, CODE_BASE, DGROUP_BASE};
use std::path::Path;

pub const OVERLAYS: [(&str, u32); 23] = [
    ("TOWN", 0x081d), ("MAINOUT", 0x081d), ("DUNGEON", 0x081d), ("INTRO", 0x081d),
    ("FLAMES", 0x0a29), ("NPC", 0x0a29), ("COMBAT", 0x0a29), ("BLCKTHRN", 0x0a29), ("LOOKOBJ", 0x0a29),
    ("DNGLOOK", 0x0a29), ("OUTSUBS", 0x0a29), ("SHOPPES", 0x0a29), ("ENDGAME", 0x0a29),
    ("SJOG", 0x0bf8), ("CMDS", 0x0bf8), ("CAST", 0x0bf8), ("TALK", 0x0bf8),
    ("CAST2", 0x0e1e), ("ZSTATS", 0x0e1e), ("COMSUBS", 0x0e1e), ("SHOPPES2", 0x0e1e), ("SHOPPES3", 0x0e1e), ("FONT", 0x0e1e),
];

/// Linear address of an overlay's load base (Ghidra convention: code base segment 0x1000).
pub fn overlay_base(name: &str) -> Option<u32> {
    OVERLAYS.iter().find(|(n, _)| *n == name).map(|(_, seg)| (0x1000 + seg) * 16)
}

pub fn load_images(mem: &mut Mem, dir: &Path) -> Result<(), String> {
    // ULTIMA.EXE load module (skip MZ header) at 0x10000: data tables inside the root code segment.
    let exe = std::fs::read(dir.join("ULTIMA.EXE")).map_err(|e| format!("ULTIMA.EXE: {e}"))?;
    let hdr_paras = u16::from_le_bytes([exe[8], exe[9]]) as usize;
    mem.write(CODE_BASE, &exe[hdr_paras * 16..]);
    // DATA.OVL = DGROUP initialised data; the file starts with a 16-byte PLINK relocation header
    // (3 records, paragraph padded) that is NOT part of the data image.
    let data = std::fs::read(dir.join("DATA.OVL")).map_err(|e| format!("DATA.OVL: {e}"))?;
    mem.write(DGROUP_BASE, &data[16..]);
    apply_relocs(mem, &data, 3);
    Ok(())
}

/// PLINK86 overlay relocation records: (u16 offset, u16 segment) pairs at the start of the file;
/// each patched word gets the load base segment (0x1000) added.
fn apply_relocs(mem: &mut Mem, file: &[u8], n: usize) {
    for i in 0..n {
        let off = u16::from_le_bytes([file[i * 4], file[i * 4 + 1]]);
        let seg = u16::from_le_bytes([file[i * 4 + 2], file[i * 4 + 3]]);
        if off == 0 && seg == 0 { continue; }
        let a = Mem::lin(seg + 0x1000, off);
        let v = mem.u16(a);
        mem.set_u16(a, v.wrapping_add(0x1000));
    }
}

/// Number of relocation records (header = records padded to 16 bytes) per overlay.
fn overlay_nreloc(name: &str) -> usize { if name == "INTRO" { 2 } else { 0 } }

/// Load one overlay's bytes at its original address (so that data tables inside it can be read).
pub fn load_overlay(mem: &mut Mem, dir: &Path, name: &str) -> Result<(), String> {
    let base = overlay_base(name).ok_or_else(|| format!("unknown overlay {name}"))?;
    let data = std::fs::read(dir.join(format!("{name}.OVL"))).map_err(|e| format!("{name}.OVL: {e}"))?;
    let n = overlay_nreloc(name);
    let hdr = if n > 0 { (n * 4 + 15) & !15 } else { 0 };
    mem.write(base, &data[hdr..]);
    if n > 0 { apply_relocs(mem, &data, n); }
    Ok(())
}
