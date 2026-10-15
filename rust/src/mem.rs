//! Emulated flat memory of the original program. All game data lives here at its original
//! linear address so that pointer arithmetic, memory-image save files and tables inside code
//! segments behave exactly as in the DOS binary. Nothing from the game is embedded in the executable:
//! the files are read from the game directory at start-up.

pub const MEM_SIZE: usize = 0x100000;
pub const CODE_BASE: u32 = 0x10000; // segment 0x1000
pub const DGROUP_SEG: u32 = 0x1f64;
pub const DGROUP_BASE: u32 = DGROUP_SEG * 16; // 0x1f640
pub const DGROUP_SIZE: u32 = 0xc540;
pub const HEAP_BASE: u32 = 0x2bb80; // segment 0x2bb8
pub const HEAP_END: u32 = 0xa0000;  // conventional memory end

pub struct Mem {
    pub b: Vec<u8>,
}

impl Mem {
    pub fn new() -> Mem { Mem { b: vec![0u8; MEM_SIZE] } }
    #[inline] pub fn ds(off: u16) -> u32 { DGROUP_BASE + off as u32 }
    #[inline] pub fn lin(seg: u16, off: u16) -> u32 { (seg as u32) * 16 + off as u32 }
    #[inline] pub fn u8(&self, a: u32) -> u8 { self.b[a as usize] }
    #[inline] pub fn i8(&self, a: u32) -> i8 { self.b[a as usize] as i8 }
    #[inline] pub fn u16(&self, a: u32) -> u16 { let a = a as usize; u16::from_le_bytes([self.b[a], self.b[a + 1]]) }
    #[inline] pub fn i16(&self, a: u32) -> i16 { self.u16(a) as i16 }
    #[inline] pub fn u32(&self, a: u32) -> u32 { let a = a as usize; u32::from_le_bytes([self.b[a], self.b[a+1], self.b[a+2], self.b[a+3]]) }
    #[inline] pub fn i32(&self, a: u32) -> i32 { self.u32(a) as i32 }
    #[inline] pub fn set_u8(&mut self, a: u32, v: u8) { self.b[a as usize] = v; }
    #[inline] pub fn set_i8(&mut self, a: u32, v: i8) { self.b[a as usize] = v as u8; }
    #[inline] pub fn set_u16(&mut self, a: u32, v: u16) { let a = a as usize; let x = v.to_le_bytes(); self.b[a] = x[0]; self.b[a + 1] = x[1]; }
    #[inline] pub fn set_i16(&mut self, a: u32, v: i16) { self.set_u16(a, v as u16); }
    #[inline] pub fn set_u32(&mut self, a: u32, v: u32) { let a = a as usize; self.b[a..a + 4].copy_from_slice(&v.to_le_bytes()); }
    #[inline] pub fn set_i32(&mut self, a: u32, v: i32) { self.set_u32(a, v as u32); }
    /// far pointer stored as (offset, segment) at `a` → linear address
    #[inline] pub fn farptr(&self, a: u32) -> u32 { Mem::lin(self.u16(a + 2), self.u16(a)) }
    #[inline] pub fn set_farptr(&mut self, a: u32, lin: u32) { self.set_u16(a, (lin & 0xf) as u16); self.set_u16(a + 2, (lin >> 4) as u16); }
    pub fn slice(&self, a: u32, n: usize) -> &[u8] { &self.b[a as usize..a as usize + n] }
    pub fn slice_mut(&mut self, a: u32, n: usize) -> &mut [u8] { &mut self.b[a as usize..a as usize + n] }
    pub fn copy(&mut self, dst: u32, src: u32, n: usize) { self.b.copy_within(src as usize..src as usize + n, dst as usize); }
    pub fn fill(&mut self, a: u32, n: usize, v: u8) { for x in &mut self.b[a as usize..a as usize + n] { *x = v; } }
    pub fn write(&mut self, a: u32, data: &[u8]) { self.b[a as usize..a as usize + data.len()].copy_from_slice(data); }
    pub fn strlen(&self, a: u32) -> usize { let mut n = 0; while self.b[a as usize + n] != 0 { n += 1; } n }
    pub fn cstr(&self, a: u32) -> &[u8] { let n = self.strlen(a); &self.b[a as usize..a as usize + n] }
    pub fn cstr_string(&self, a: u32) -> String { self.cstr(a).iter().map(|&c| c as char).collect() }
    pub fn strcpy(&mut self, dst: u32, src: u32) { let n = self.strlen(src) + 1; self.copy(dst, src, n); }
    pub fn strcmp(&self, a: u32, b: u32) -> i16 {
        let mut i = 0u32;
        loop {
            let x = self.u8(a + i); let y = self.u8(b + i);
            if x != y { return if x < y { -1 } else { 1 }; }
            if x == 0 { return 0; }
            i += 1;
        }
    }
}

/// Simple first-fit heap over emulated memory (replaces malloc/free/_fmalloc).
pub struct Heap {
    free: Vec<(u32, u32)>, // (start, len), paragraph aligned
}
impl Heap {
    pub fn new() -> Heap { Heap { free: vec![(HEAP_BASE, HEAP_END - HEAP_BASE)] } }
    /// Allocate `n` bytes, returns linear address (paragraph aligned, with a 16-byte header storing the size).
    pub fn alloc(&mut self, n: u32) -> Option<u32> {
        let need = (n + 16 + 15) & !15;
        for i in 0..self.free.len() {
            let (s, l) = self.free[i];
            if l >= need {
                if l == need { self.free.remove(i); } else { self.free[i] = (s + need, l - need); }
                return Some(s);
            }
        }
        None
    }
    pub fn free(&mut self, a: u32, size_hint: u32) {
        let need = (size_hint + 16 + 15) & !15;
        self.free.push((a, need));
        self.free.sort();
        // coalesce
        let mut out: Vec<(u32, u32)> = Vec::new();
        for (s, l) in self.free.drain(..) {
            if let Some(last) = out.last_mut() { if last.0 + last.1 == s { last.1 += l; continue; } }
            out.push((s, l));
        }
        self.free = out;
    }
}
