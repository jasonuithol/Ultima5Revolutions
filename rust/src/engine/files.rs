//! ROOT file layer: load_file (raw or LZW into a heap segment), region loads/saves, disk selection.
use crate::game::Game;
use crate::g;

/// The far error hook (g_error_hook): disk-swap prompt in the original; here a no-op that only
/// reports (file errors in the port are programming/data errors, not missing floppies).
pub fn call_error_hook(gm: &mut Game) {
    eprintln!("[u5] file error (dos error {})", gm.g16(g::LAST_DOS_ERROR));
}
/// True when `name` is in the DGROUP list at 0x25ea. NOTE: that list holds the LZW-COMPRESSED
/// files (PROPORT.PCS, *.BIT, *.16); everything else (maps, .DAT, .DRV, fonts) is raw.
pub fn is_lzw_file_name(gm: &Game, name: &str) -> bool {
    let up = name.to_ascii_uppercase();
    let mut p = g::RAW_FILE_NAMES;
    loop {
        let ptr = gm.g16(p);
        if ptr == 0 { return false; }
        if gm.gstr(ptr).to_ascii_uppercase() == up { return true; }
        p += 2;
    }
}
/// load_file(name at linear addr) -> (size, segment). Raw-list files are loaded verbatim, others LZW-decompressed.
pub fn load_file(gm: &mut Game, name: u16) -> Option<(u32, u16)> {
    let nm = gm.gstr(name);
    load_file_named(gm, &nm)
}
pub fn load_file_named(gm: &mut Game, nm: &str) -> Option<(u32, u16)> {
    let raw = match gm.read_file(nm) { Some(d) => d, None => { gm.set_g16(g::LAST_DOS_ERROR, 2); call_error_hook(gm); return None; } };
    let data = if !is_lzw_file_name(gm, nm) { raw } else {
        match crate::lzw::decompress_file(&raw) { Ok(d) => d, Err(e) => { eprintln!("[u5] {nm}: {e}"); gm.set_g16(g::LAST_DOS_ERROR, 2); call_error_hook(gm); return None; } }
    };
    let seg = match gm.alloc_seg(data.len() as u32) { Some(s) => s, None => fatal_out_of_memory(gm) };
    gm.m.write((seg as u32) << 4, &data);
    Some((data.len() as u32, seg))
}
pub fn fatal_out_of_memory(gm: &mut Game) -> ! {
    let _ = gm;
    eprintln!("Out of Memory");
    std::process::exit(1);
}
/// file_exists(name) -> 1/0
pub fn file_exists(gm: &mut Game, name: u16) -> i16 {
    let nm = gm.gstr(name);
    if gm.resolve_path(&nm).is_some() { gm.set_g16(g::LAST_DOS_ERROR, 0); 1 }
    else { gm.set_g16(g::LAST_DOS_ERROR, 2); call_error_hook(gm); 0 }
}
/// dos_read_region(name,buf,count,offset) — Ghidra order (offset,count,buf,name). Returns bytes read, 0 on error.
pub fn dos_read_region(gm: &mut Game, offset: u16, count: u16, buf: u16, name: u16) -> u16 {
    gm.set_g16(g::LAST_DOS_ERROR, 0);
    let nm = gm.gstr(name);
    let data = match gm.read_file(&nm) { Some(d) => d, None => { gm.set_g16(g::LAST_DOS_ERROR, 2); call_error_hook(gm); return 0; } };
    let count = if count == 0 { 0xffff } else { count } as usize;
    let start = (offset as usize).min(data.len());
    let end = (start + count).min(data.len());
    let slice = &data[start..end];
    let dst = gm.ds(buf);
    gm.m.write(dst, slice);
    slice.len() as u16
}
/// dos_write_file(name,buf,count) — Ghidra order (count,buf,name)
pub fn dos_write_file(gm: &mut Game, count: u16, buf: u16, name: u16) -> u16 {
    let nm = gm.gstr(name);
    let data = gm.m.slice(gm.ds(buf), count as usize).to_vec();
    if gm.write_file(&nm, &data) { gm.set_g16(g::LAST_DOS_ERROR, 0); count }
    else { gm.set_g16(g::LAST_DOS_ERROR, 5); call_error_hook(gm); 0 }
}
/// load_file_region(name,buf,count,offset) — Ghidra order (offset,count,buf,name). Retries in the original; here one attempt.
pub fn load_file_region(gm: &mut Game, offset: u16, count: u16, buf: u16, name: u16) {
    let _ = dos_read_region(gm, offset, count, buf, name);
}
/// save_file_region(name,buf,count) — Ghidra order (count,buf,name)
pub fn save_file_region(gm: &mut Game, count: u16, buf: u16, name: u16) {
    let _ = dos_write_file(gm, count, buf, name);
}
pub fn select_disk(gm: &mut Game, disk: i16) {
    let d = if disk == 2 || disk == 5 { 1 } else { disk };
    gm.set_g8(g::CUR_DISK, d as u8);
    gm.set_g16(g::DISK_DRIVE_SET, 0xffff);
}
pub fn set_default_drive(gm: &mut Game, _letter: u8) -> i16 { gm.set_g16(g::LAST_DOS_ERROR, 0); 1 }
pub fn get_current_drive_letter(_gm: &mut Game) -> u8 { b'C' }
pub fn disk_error_hook(_gm: &mut Game) {}
