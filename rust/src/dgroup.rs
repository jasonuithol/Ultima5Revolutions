//! The original program's data segment (DGROUP, DS=0x1f64) modelled as a flat byte array.
//! DATA.OVL is loaded verbatim at offset 0; BSS and the stack follow up to 0xC540.
//! Every global keeps its original offset so that memory-image files (SAVED.GAM etc.)
//! and index arithmetic across structures behave exactly as in the original.

pub const DGROUP_SIZE: usize = 0xC540;
pub const DATA_OVL_SIZE: usize = 0xBD50;

pub struct Dgroup {
    pub mem: Vec<u8>,
}

impl Dgroup {
    /// `data_ovl` is the raw file; its first 16 bytes are a relocation header, not data.
    pub fn from_data_ovl(data_ovl: &[u8]) -> Dgroup {
        let mut mem = vec![0u8; DGROUP_SIZE];
        let src = &data_ovl[16..];
        let n = src.len().min(DGROUP_SIZE);
        mem[..n].copy_from_slice(&src[..n]);
        Dgroup { mem }
    }
    #[inline] pub fn u8(&self, off: usize) -> u8 { self.mem[off] }
    #[inline] pub fn i8(&self, off: usize) -> i8 { self.mem[off] as i8 }
    #[inline] pub fn u16(&self, off: usize) -> u16 { u16::from_le_bytes([self.mem[off], self.mem[off + 1]]) }
    #[inline] pub fn i16(&self, off: usize) -> i16 { self.u16(off) as i16 }
    #[inline] pub fn u32(&self, off: usize) -> u32 { u32::from_le_bytes([self.mem[off], self.mem[off+1], self.mem[off+2], self.mem[off+3]]) }
    #[inline] pub fn set_u8(&mut self, off: usize, v: u8) { self.mem[off] = v; }
    #[inline] pub fn set_i8(&mut self, off: usize, v: i8) { self.mem[off] = v as u8; }
    #[inline] pub fn set_u16(&mut self, off: usize, v: u16) { let b = v.to_le_bytes(); self.mem[off] = b[0]; self.mem[off + 1] = b[1]; }
    #[inline] pub fn set_i16(&mut self, off: usize, v: i16) { self.set_u16(off, v as u16); }
    #[inline] pub fn set_u32(&mut self, off: usize, v: u32) { self.mem[off..off+4].copy_from_slice(&v.to_le_bytes()); }
    #[inline] pub fn bytes(&self, off: usize, len: usize) -> &[u8] { &self.mem[off..off + len] }
    #[inline] pub fn bytes_mut(&mut self, off: usize, len: usize) -> &mut [u8] { &mut self.mem[off..off + len] }
    /// NUL-terminated string starting at `off`.
    pub fn cstr(&self, off: usize) -> &[u8] {
        let end = self.mem[off..].iter().position(|&b| b == 0).map(|p| off + p).unwrap_or(self.mem.len());
        &self.mem[off..end]
    }
    pub fn cstr_string(&self, off: usize) -> String {
        self.cstr(off).iter().map(|&b| b as char).collect()
    }
    pub fn strlen(&self, off: usize) -> usize { self.cstr(off).len() }
}
