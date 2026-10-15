//! The game context: emulated memory, heap, platform, and DGROUP accessors.
//! Every translated function is `fn name(gm: &mut Game, ...)`.
//!
//! PARAMETER ORDER CONVENTION: Rust parameters follow the Ghidra decompiled order (param_1 first),
//! which for this Pascal-convention binary is the REVERSE of the original source order. Call sites in
//! the decompiled C therefore translate verbatim. Parameter names carry the true meaning.
use crate::mem::{Heap, Mem};
use crate::platform::Platform;
use std::path::{Path, PathBuf};

pub struct Game {
    pub m: Mem,
    pub heap: Heap,
    pub pf: Platform,
    pub dir: PathBuf,
    /// emulated stack scratch allocator (DGROUP 0xbd50..0xc540 region), grows downward
    pub sp: u16,
    /// set when exit() is called
    pub exit_code: Option<i16>,
}

impl Game {
    pub fn new(dir: PathBuf, headless: bool) -> Game {
        let mut m = Mem::new();
        crate::loader::load_images(&mut m, &dir).expect("load game images");
        Game { m, heap: Heap::new(), pf: Platform::new(headless), dir, sp: 0xc540, exit_code: None }
    }
    // ---- DGROUP accessors (offset relative to DS) ----
    #[inline] pub fn ds(&self, off: u16) -> u32 { Mem::ds(off) }
    #[inline] pub fn g8(&self, off: u16) -> u8 { self.m.u8(Mem::ds(off)) }
    #[inline] pub fn gi8(&self, off: u16) -> i8 { self.m.i8(Mem::ds(off)) }
    #[inline] pub fn g16(&self, off: u16) -> u16 { self.m.u16(Mem::ds(off)) }
    #[inline] pub fn gi16(&self, off: u16) -> i16 { self.m.i16(Mem::ds(off)) }
    #[inline] pub fn g32(&self, off: u16) -> u32 { self.m.u32(Mem::ds(off)) }
    #[inline] pub fn set_g8(&mut self, off: u16, v: u8) { self.m.set_u8(Mem::ds(off), v); }
    #[inline] pub fn set_gi8(&mut self, off: u16, v: i8) { self.m.set_i8(Mem::ds(off), v); }
    #[inline] pub fn set_g16(&mut self, off: u16, v: u16) { self.m.set_u16(Mem::ds(off), v); }
    #[inline] pub fn set_gi16(&mut self, off: u16, v: i16) { self.m.set_i16(Mem::ds(off), v); }
    #[inline] pub fn set_g32(&mut self, off: u16, v: u32) { self.m.set_u32(Mem::ds(off), v); }
    /// NUL-terminated string at a DGROUP offset.
    pub fn gstr(&self, off: u16) -> String { self.m.cstr_string(Mem::ds(off)) }
    /// Far pointer stored at DGROUP offset (off, seg) -> linear.
    pub fn gfar(&self, off: u16) -> u32 { self.m.farptr(Mem::ds(off)) }

    // ---- emulated stack frames for C locals whose address escapes ----
    /// Returns the DGROUP offset of the new scratch buffer.
    pub fn stack_alloc(&mut self, n: u16) -> u16 { self.sp -= (n + 1) & !1; self.sp }
    pub fn stack_free(&mut self, n: u16) { self.sp += (n + 1) & !1; }
    /// Run `f` with a zeroed scratch buffer of `n` bytes in emulated memory.
    pub fn with_buf<R>(&mut self, n: u16, f: impl FnOnce(&mut Game, u16) -> R) -> R {
        let a = self.stack_alloc(n);
        self.m.fill(Mem::ds(a), n as usize, 0);
        let r = f(self, a);
        self.stack_free(n);
        r
    }
    /// Copy a Rust byte string into scratch memory (NUL-terminated) for the duration of `f`.
    pub fn with_str<R>(&mut self, s: &[u8], f: impl FnOnce(&mut Game, u16) -> R) -> R {
        let n = s.len() as u16 + 1;
        let a = self.stack_alloc(n);
        self.m.write(Mem::ds(a), s);
        self.m.set_u8(Mem::ds(a) + s.len() as u32, 0);
        let r = f(self, a);
        self.stack_free(n);
        r
    }

    // ---- heap in segments (paragraph numbers), like DOS allocations ----
    pub fn alloc_seg(&mut self, bytes: u32) -> Option<u16> { self.heap.alloc(bytes).map(|a| (a >> 4) as u16) }
    pub fn free_seg(&mut self, seg: u16) { if seg != 0 { self.heap.free((seg as u32) << 4, 0); } }

    // ---- files (case-insensitive lookup in the game directory) ----
    pub fn resolve_path(&self, name: &str) -> Option<PathBuf> {
        let name = name.trim_end_matches('\0');
        let p = self.dir.join(name);
        if p.exists() { return Some(p); }
        let lower = name.to_ascii_lowercase();
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().to_ascii_lowercase() == lower { return Some(e.path()); }
            }
        }
        None
    }
    pub fn read_file(&self, name: &str) -> Option<Vec<u8>> { self.resolve_path(name).and_then(|p| std::fs::read(p).ok()) }
    pub fn write_file(&self, name: &str, data: &[u8]) -> bool {
        let p = self.resolve_path(name).unwrap_or_else(|| self.dir.join(name));
        std::fs::write(p, data).is_ok()
    }
    pub fn game_path(&self) -> &Path { &self.dir }
}
