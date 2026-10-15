//! Reimplementation of the EGA.DRV drawing primitives on two 320x200 16-colour pages.
//! Semantics follow analysis/driver_api.md. Image/tile/glyph data is read from the emulated
//! memory in the ORIGINAL packed on-disk formats (the port never converts to planar form).
use crate::mem::Mem;

pub const SCREEN_W: usize = 320;
pub const SCREEN_H: usize = 200;

/// Standard EGA 16-colour palette (RGB).
pub const EGA_PALETTE: [u32; 16] = [
    0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xAA5500, 0xAAAAAA,
    0x555555, 0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF,
];

pub struct Video {
    /// pages[0] = visible screen, pages[1] = off-screen page. One colour index per pixel.
    pub pages: [Vec<u8>; 2],
    pub draw_page: usize,
    pub color: u8,
    /// Linear address of the decompressed TILES.16 data (512 tiles x 128 bytes, packed 4bpp). 0 = none.
    pub tileset: u32,
    /// Clip settings mirrored from the config block (entry 25 only).
    pub clip_enable: bool,
    pub clip_ymin: i16,
    pub clip_ymax: i16,
    /// CPU speed unit set by entry 37.
    pub speed: u16,
    /// 'dissolve makes sound' flag: cleared by the first glyph draw.
    pub dissolve_sound: bool,
    /// LFSR state for tile_dissolve_step.
    pub tile_lfsr: u8,
    pub dirty: bool,
    pub anim_seed: u16,
    /// Noise-routine RNG state (driver CS:0x27a5).
    pub noise_seed: u16,
    pub fire_seed: u16,
    pub patch_save: [[u8; 8]; 2],
    pub fire_frame: u8,
}

#[inline]
fn tile_px(mem: &Mem, tileset: u32, tile: u16, x: usize, y: usize) -> u8 {
    let b = mem.u8(tileset + (tile as u32) * 128 + (y as u32) * 8 + (x as u32) / 2);
    if x & 1 == 0 { b >> 4 } else { b & 15 }
}

impl Video {
    pub fn new() -> Video {
        Video {
            pages: [vec![0; SCREEN_W * SCREEN_H], vec![0; SCREEN_W * SCREEN_H]],
            draw_page: 0, color: 0, tileset: 0,
            clip_enable: false, clip_ymin: 0, clip_ymax: 199,
            speed: 100, dissolve_sound: true, tile_lfsr: 1, dirty: true, anim_seed: 0x7664, noise_seed: 0x7664, fire_seed: 0x7664, patch_save: [[0; 8]; 2], fire_frame: 0,
        }
    }
    pub fn init(&mut self) {
        for p in self.pages.iter_mut() { for x in p.iter_mut() { *x = 0; } }
        self.draw_page = 0; self.dirty = true;
    }
    #[inline]
    fn put_on(&mut self, page: usize, x: i32, y: i32, c: u8, xor: bool) {
        if x < 0 || y < 0 || x >= SCREEN_W as i32 || y >= SCREEN_H as i32 { return; }
        let i = y as usize * SCREEN_W + x as usize;
        let p = &mut self.pages[page][i];
        *p = if xor { *p ^ c } else { c } & 15;
        if page == 0 { self.dirty = true; }
    }
    #[inline]
    pub fn get_on(&self, page: usize, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= SCREEN_W as i32 || y >= SCREEN_H as i32 { return 0; }
        self.pages[page][y as usize * SCREEN_W + x as usize]
    }
    // ---- entries 15, 16, 17, 19, 20, 21 ----
    pub fn set_color(&mut self, c: u8) { self.color = c & 15; }
    /// Entry 16. Returns true if the point was outside the screen.
    pub fn plot_pixel(&mut self, x: i16, y: i16, xor: bool) -> bool {
        if x < 0 || y < 0 || x >= 320 || y >= 200 { return true; }
        let c = self.color; let pg = self.draw_page;
        self.put_on(pg, x as i32, y as i32, c, xor);
        false
    }
    /// Entry 12: colour on page 0.
    pub fn get_pixel(&self, x: i16, y: i16) -> u8 { self.get_on(0, x as i32, y as i32) }
    /// Entry 17: Bresenham line, inclusive endpoints, ALWAYS page 0.
    pub fn draw_line(&mut self, x1: i16, y1: i16, x2: i16, y2: i16, xor: bool) {
        let c = self.color;
        let (mut x, mut y) = (x1 as i32, y1 as i32);
        let dx = (x2 as i32 - x1 as i32).abs();
        let dy = (y2 as i32 - y1 as i32).abs();
        let sx = if x2 >= x1 { 1 } else { -1 };
        let sy = if y2 >= y1 { 1 } else { -1 };
        if dx >= dy {
            let mut err = 2 * dy - dx;
            for _ in 0..=dx {
                self.put_on(0, x, y, c, xor);
                if err < 0 { err += 2 * dy; } else { y += sy; err += 2 * dy - 2 * dx; }
                x += sx;
            }
        } else {
            let mut err = 2 * dx - dy;
            for _ in 0..=dy {
                self.put_on(0, x, y, c, xor);
                if err < 0 { err += 2 * dx; } else { x += sx; err += 2 * dx - 2 * dy; }
                y += sy;
            }
        }
    }
    /// Entry 19.
    pub fn draw_hline(&mut self, x1: i16, y: i16, x2: i16, xor: bool) {
        let (a, b) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
        let c = self.color; let pg = self.draw_page;
        for x in a..=b { self.put_on(pg, x as i32, y as i32, c, xor); }
    }
    /// Entry 20.
    pub fn draw_vline(&mut self, x: i16, y1: i16, y2: i16, xor: bool) {
        let c = self.color; let pg = self.draw_page;
        for y in y1..=y2 { self.put_on(pg, x as i32, y as i32, c, xor); }
    }
    /// Entry 21: inclusive rectangle fill. XOR on page 1 degenerates to an opaque fill (hardware quirk).
    pub fn fill_rect(&mut self, x1: i16, y1: i16, x2: i16, y2: i16, xor: bool) {
        let (ax, bx) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
        let (ay, by) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };
        let c = self.color; let pg = self.draw_page;
        let xor = xor && pg == 0;
        for y in ay..=by { for x in ax..=bx { self.put_on(pg, x as i32, y as i32, c, xor); } }
    }
    /// Entry 13: scroll the rect's contents by `rows` (<0 up, >0 down); vacated rows := colour 0.
    /// Fast path when x1 == 192 (message window): region x 192..319, rows from 96, up 8 rows,
    /// pixels mapped to 15 where the intensity bit (8) is set, else 0; vacated rows not cleared.
    pub fn scroll_rect(&mut self, x1: i16, y1: i16, x2: i16, y2: i16, rows: i16) {
        let pg = self.draw_page;
        if x1 == 192 {
            for r in 96..SCREEN_H as i32 {
                for x in 192..320 {
                    let src = if r + 8 < SCREEN_H as i32 { self.get_on(pg, x, r + 8) } else { 0 };
                    let v = if x < 200 { src } else if src & 8 != 0 { 15 } else { 0 };
                    self.put_on(pg, x, r, v, false);
                }
            }
            return;
        }
        let n = rows.abs() as i32;
        if n == 0 { return; }
        let (ay, by) = (y1 as i32, y2 as i32);
        let (ax, bx) = (x1 as i32, x2 as i32);
        if rows < 0 {
            for r in (ay + n)..=by {
                for x in ax..=bx { let v = self.get_on(pg, x, r); self.put_on(pg, x, r - n, v, false); }
            }
            for r in (by - n + 1).max(ay)..=by { for x in ax..=bx { self.put_on(pg, x, r, 0, false); } }
        } else {
            for r in (ay..=(by - n)).rev() {
                for x in ax..=bx { let v = self.get_on(pg, x, r); self.put_on(pg, x, r + n, v, false); }
            }
            for r in ay..=(ay + n - 1).min(by) { for x in ax..=bx { self.put_on(pg, x, r, 0, false); } }
        }
    }
    // ---- entries 8, 9, 10 ----
    /// Entry 8 with CF=1: page 1 -> page 0 rect copy. (CF=0 direction is a no-op in EGA.DRV.)
    pub fn copy_rect_back_to_front(&mut self, x1: i16, y1: i16, x2: i16, y2: i16) {
        let (ax, bx) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
        let (ay, by) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };
        for y in ay..=by { for x in ax..=bx { let v = self.get_on(1, x as i32, y as i32); self.put_on(0, x as i32, y as i32, v, false); } }
    }
    /// Entry 9.
    pub fn copy_page(&mut self, src: usize, dst: usize) {
        if src == dst { return; }
        let s = self.pages[src].clone();
        self.pages[dst].copy_from_slice(&s);
        if dst == 0 { self.dirty = true; }
    }
    /// Entry 10.
    pub fn swap_pages(&mut self) { self.pages.swap(0, 1); self.dirty = true; }
    // ---- entry 31: glyph ----
    pub fn draw_glyph(&mut self, cell_x: u8, cell_y: u8, fg: u8, bg: u8, glyph: &[u8]) {
        self.dissolve_sound = false;
        let pg = self.draw_page;
        let x0 = cell_x as i32 * 8; let y0 = cell_y as i32 * 8;
        for r in 0..8 {
            let b = glyph[r];
            for c in 0..8 {
                let v = if b & (0x80 >> c) != 0 { fg } else { bg };
                self.put_on(pg, x0 + c as i32, y0 + r as i32, v, false);
            }
        }
    }
    // ---- entry 27: tile ----
    pub fn draw_tile(&mut self, mem: &Mem, col: u8, row: u8, tile: u16, cell_w: u8, cell_h: u8) {
        if self.tileset == 0 { return; }
        let pg = self.draw_page;
        let x0 = (cell_w as i32 & !7) + 16 * col as i32;
        let y0 = cell_h as i32 + 16 * row as i32;
        let t = tile & 511;
        for y in 0..16 { for x in 0..16 {
            let v = tile_px(mem, self.tileset, t, x, y);
            self.put_on(pg, x0 + x as i32, y0 + y as i32, v, false);
        } }
    }
    /// Pixel of a tile in the registered tile set (used by effects and by game code that peeks tiles).
    pub fn tile_pixel(&self, mem: &Mem, tile: u16, x: usize, y: usize) -> u8 { tile_px(mem, self.tileset, tile, x, y) }
    pub fn set_tile_pixel(&self, mem: &mut Mem, tile: u16, x: usize, y: usize, v: u8) {
        let a = self.tileset + (tile as u32) * 128 + (y as u32) * 8 + (x as u32) / 2;
        let b = mem.u8(a);
        mem.set_u8(a, if x & 1 == 0 { (b & 0x0f) | (v << 4) } else { (b & 0xf0) | (v & 15) });
    }
    // ---- entries 25/33: multi-image .16 blit (packed rows: 2 px/byte, bpr = ((w+7)/8)*4) ----
    /// `flags`: bit0 = vertical flip, bit1 = horizontal mirror. `file` = linear address of the
    /// decompressed image file (u16 count, then (u16 image_off, u16 mask_off) pairs).
    pub fn draw_image(&mut self, mem: &Mem, file: u32, index: u16, x: i16, y: i16, flags: u16) {
        let count = mem.u16(file);
        if index >= count { return; }
        let ent = file + 2 + 4 * index as u32;
        let img = file + mem.u16(ent) as u32;
        let mask_off = mem.u16(ent + 2);
        let w = mem.u16(img) as i32; let h = mem.u16(img + 2) as i32;
        if w == 0 || h == 0 { return; }
        let bpr = ((w + 7) / 8 * 4) as u32;
        let pg = self.draw_page;
        let use_mask = mask_off != 0 && pg == 1;
        let mask = if use_mask { Some(file + mask_off as u32) } else { None };
        let mbpr = ((w + 7) / 8) as u32;
        let x0 = (x as i32) & !7;
        let (ymin, ymax) = if self.clip_enable { (self.clip_ymin as i32, self.clip_ymax as i32) } else { (0, 199) };
        let vflip = pg == 1 && flags & 1 != 0;
        let mirror = pg == 1 && flags & 2 != 0;
        // visible width in pixels: page 1 keeps pixels beyond w; page 0 copies whole bytes (bpr*2 px)
        let vis_w = if pg == 0 { bpr as i32 * 2 } else { w };
        for r in 0..h {
            let dy = if vflip { y as i32 + h - r } else { y as i32 + r };
            if dy < ymin || dy > ymax { continue; }
            let rowp = img + 4 + r as u32 * bpr;
            let mrow = mask.map(|m| m + 4 + r as u32 * mbpr);
            for px in 0..vis_w {
                let b = mem.u8(rowp + (px / 2) as u32);
                let v = if px & 1 == 0 { b >> 4 } else { b & 15 };
                if let Some(mr) = mrow {
                    if px < w {
                        let mb = mem.u8(mr + (px / 8) as u32);
                        if mb & (0x80 >> (px & 7)) != 0 { continue; }
                    }
                }
                let dx = if mirror { x0 + 8 * (bpr as i32 / 4) - 2 - px } else { x0 + px };
                if mirror {
                    let old = self.get_on(pg, dx, dy);
                    self.put_on(pg, dx, dy, old | v, false);
                } else {
                    self.put_on(pg, dx, dy, v, false);
                }
            }
        }
    }
    // ---- entry 26: 1-bpp sprite, set bits -> colour 15, ALWAYS page 1 ----
    pub fn draw_bitmap_white(&mut self, mem: &Mem, file: u32, index: u16, x: i16, y: i16) {
        let count = mem.u16(file);
        if index >= count { return; }
        let img = file + mem.u16(file + 2 + 2 * index as u32) as u32;
        let w = mem.u16(img) as i32; let h = mem.u16(img + 2) as i32;
        let bpr = ((w + 7) >> 3) as u32; // PROPORT.PCS glyphs are < 8 wide but still one byte per row
        for r in 0..h {
            let rowp = img + 4 + r as u32 * bpr;
            for px in 0..(bpr as i32 * 8) {
                let b = mem.u8(rowp + (px / 8) as u32);
                if b & (0x80 >> (px & 7)) != 0 { self.put_on(1, x as i32 + px, y as i32 + r, 15, false); }
            }
        }
    }
    /// Entry 34 CF=1: one pixel of a tile at a map cell (page 0), in LFSR order.
    pub fn tile_dissolve_step(&mut self, mem: &Mem, col: u8, row: u8, tile: u16, step: u8, cell_w: u8, cell_h: u8) {
        let x0 = (cell_w as i32 & !7) + 16 * col as i32;
        let y0 = cell_h as i32 + 16 * row as i32;
        let (px, py) = if step == 0 { (0usize, 0usize) } else {
            if step == 1 { self.tile_lfsr = 1; }
            let v = self.tile_lfsr;
            let p = ((v >> 4) as usize, (v & 15) as usize);
            self.tile_lfsr = (v >> 1) ^ (if v & 1 != 0 { 0xB8 } else { 0 });
            p
        };
        let c = tile_px(mem, self.tileset, tile & 511, px, py);
        self.put_on(0, x0 + px as i32, y0 + py as i32, c, false);
    }
    /// Entry 32 CF=1: tile 5 (or 0x44 when dungeon) with its last `frame` rows replaced by the first
    /// `frame` rows of tile 0xDC, drawn at the map cell on the draw page.
    pub fn draw_effect_frame(&mut self, mem: &Mem, frame: u8, col: u8, row: u8, dungeon: bool, cell_w: u8, cell_h: u8) {
        let base: u16 = if dungeon { 0x44 } else { 5 };
        let pg = self.draw_page;
        let x0 = (cell_w as i32 & !7) + 16 * col as i32;
        let y0 = cell_h as i32 + 16 * row as i32;
        let f = (frame as usize).min(16);
        for y in 0..16 { for x in 0..16 {
            let v = if y >= 16 - f { tile_px(mem, self.tileset, 0xDC, x, y - (16 - f)) } else { tile_px(mem, self.tileset, base, x, y) };
            self.put_on(pg, x0 + x as i32, y0 + y as i32, v, false);
        } }
    }
    /// LFSR taps used by dissolve_rect / title effects.
    pub fn lfsr_taps(bits: u32) -> u16 {
        match bits { 2 => 0x0003, 3 => 0x0006, 4 => 0x000C, 5 => 0x0014, 6 => 0x0030, 7 => 0x0060, 8 => 0x00B8,
            9 => 0x0110, 10 => 0x0240, 11 => 0x0500, 12 => 0x0CA0, 13 => 0x1B00, 14 => 0x3500, 15 => 0x6000, _ => 0xB400 }
    }
    /// Entry 34 CF=0 (without the sound/key-abort part): copy rect page1 -> page0 in LFSR pixel order.
    /// Calls `step` after every pixel; if it returns false the dissolve aborts.
    pub fn dissolve_rect<F: FnMut(&mut Video, u32) -> bool>(&mut self, x1: i16, y1: i16, x2: i16, y2: i16, mut step: F) {
        let (ax, bx) = if x1 <= x2 { (x1 as i32, x2 as i32) } else { (x2 as i32, x1 as i32) };
        let (ay, by) = if y1 <= y2 { (y1 as i32, y2 as i32) } else { (y2 as i32, y1 as i32) };
        let w = bx - ax + 1; let h = by - ay + 1;
        let n = (w * h) as u32;
        if n <= 1 { let v = self.get_on(1, ax, ay); self.put_on(0, ax, ay, v, false); return; }
        let bits = 32 - (n - 1).leading_zeros();
        let taps = Video::lfsr_taps(bits) as u32;
        let mut v: u32 = 1; let mut count = 0u32;
        loop {
            let x = (v % w as u32) as i32; let y = (v / w as u32) as i32;
            if y < h { let c = self.get_on(1, ax + x, ay + y); self.put_on(0, ax + x, ay + y, c, false); }
            count += 1;
            if !step(self, count) { return; }
            let carry = v & 1; v >>= 1; if carry != 0 { v ^= taps; }
            if v == 1 { break; }
        }
        let c = self.get_on(1, ax, ay); self.put_on(0, ax, ay, c, false);
    }
    pub fn render_rgb(&self, out: &mut Vec<u32>, scale: usize) {
        let w = SCREEN_W * scale;
        out.resize(w * SCREEN_H * scale, 0);
        for y in 0..SCREEN_H {
            for x in 0..SCREEN_W {
                let c = EGA_PALETTE[(self.pages[0][y * SCREEN_W + x] & 15) as usize];
                for sy in 0..scale { let row = (y * scale + sy) * w; for sx in 0..scale { out[row + x * scale + sx] = c; } }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Tile-set mutation entries (32 CF=0 animate_tiles, 36 tile_patch), expressed on packed nibbles.
// "M(t)" in the driver = plane-3 (intensity bit) of tile t used as a per-pixel mask.
// ---------------------------------------------------------------------------------------------
fn lcg(s: &mut u16) -> u16 { *s = (s.wrapping_add(0x9248).rotate_right(3) ^ 0x9248).wrapping_add(0x11); *s }

impl Video {
    #[inline] fn tp(&self, mem: &Mem, t: u16, x: usize, y: usize) -> u8 { tile_px(mem, self.tileset, t, x, y) }
    #[inline] fn stp(&self, mem: &mut Mem, t: u16, x: usize, y: usize, v: u8) { self.set_tile_pixel(mem, t, x, y, v); }
    /// Write one plane byte (8 pixels) of group g (0..31: row g/2, half g%2) of tile t.
    fn set_plane_byte(&self, mem: &mut Mem, t: u16, g: usize, plane: u8, b: u8) {
        let (y, x0) = (g / 2, (g % 2) * 8);
        for px in 0..8 {
            let bit = (b >> (7 - px)) & 1;
            let c = self.tp(mem, t, x0 + px, y);
            self.stp(mem, t, x0 + px, y, (c & !(1 << plane)) | (bit << plane));
        }
    }
    fn swap_rows(&self, mem: &mut Mem, t: u16, r1: usize, r2: usize, x0: usize, x1: usize) {
        for x in x0..x1 { let a = self.tp(mem, t, x, r1); let b = self.tp(mem, t, x, r2); self.stp(mem, t, x, r1, b); self.stp(mem, t, x, r2, a); }
    }
    /// Entry 32 CF=0: per-frame mutation of the tile bitmaps (water, shoreline, fire, flags).
    pub fn animate_tiles(&mut self, mem: &mut Mem) {
        if self.tileset == 0 { return; }
        let mut seed = self.anim_seed;
        // (1) noise sprites
        for g in 0..32 {
            let r = lcg(&mut seed) as u8;
            self.set_plane_byte(mem, 0x1e8, g, 1, r); self.set_plane_byte(mem, 0x1e8, g, 3, r);
            self.set_plane_byte(mem, 0x1e9, g, 0, r); self.set_plane_byte(mem, 0x1e9, g, 2, r); self.set_plane_byte(mem, 0x1e9, g, 3, r);
            self.set_plane_byte(mem, 0x1ea, g, 2, r); self.set_plane_byte(mem, 0x1ea, g, 3, r);
            self.set_plane_byte(mem, 0x1eb, g, 0, r); self.set_plane_byte(mem, 0x1eb, g, 3, r);
        }
        // (2) water tiles: rows rotate down by one
        for &t in &[1u16, 2, 3, 0x8f] {
            let base = self.tileset + t as u32 * 128;
            let last: Vec<u8> = (0..8).map(|i| mem.u8(base + 15 * 8 + i)).collect();
            for r in (1..16).rev() { for i in 0..8u32 { let v = mem.u8(base + (r - 1) * 8 + i); mem.set_u8(base + r * 8 + i, v); } }
            for i in 0..8u32 { mem.set_u8(base + i, last[i as usize]); }
        }
        // (3) shoreline through masks of 0x70..0x7f
        for i in 0..16u16 { self.masked_copy(mem, 0x60 + i, 0x70 + i, 3, false); }
        // (4)
        self.masked_copy(mem, 0x108, 0x100, 0x1eb, false);
        self.masked_copy(mem, 0x1b4, 0x100, 0x1e9, false);
        // (5)
        for i in 0..4u16 { self.masked_copy(mem, 0x34 + i, 0xd0 + i, 3, false); self.masked_copy(mem, 0xe4 + i, 0xd0 + i, 3, true); }
        // (6) flames: a ^= (b & c) pixel-wise
        for i in 0..4u16 { self.xor_and(mem, 0xb0 + i, 0x1ea, 0xc0 + i); self.xor_and(mem, 0xbc + i, 0x1ea, 0xcc + i); }
        self.xor_and(mem, 0xde, 0x1eb, 0xc2);
        // (7) row swaps by seed bits
        let s = seed;
        if s & 1 != 0 { self.swap_rows(mem, 0x12, 0, 2, 0, 16); }
        if s & 2 != 0 { self.swap_rows(mem, 0x14, 2, 4, 0, 8); }
        if s & 4 != 0 { self.swap_rows(mem, 0x15, 0, 2, 0, 16); }
        if s & 8 != 0 { self.swap_rows(mem, 0x3e, 0, 2, 0, 16); }
        if s & 16 != 0 { self.swap_rows(mem, 0x121, 1, 3, 8, 16); }
        if s & 32 != 0 { self.swap_rows(mem, 0x123, 1, 3, 0, 16); }
        if s & 64 != 0 { self.swap_rows(mem, 0x12d, 1, 3, 8, 16); }
        if s & 1 != 0 { self.swap_rows(mem, 0x12f, 1, 3, 0, 16); }
        self.anim_seed = seed;
    }
    /// dst := (dst & ~M(mask)) | (src & M(mask)); `invert` flips the mask sense.
    fn masked_copy(&self, mem: &mut Mem, dst: u16, mask: u16, src: u16, invert: bool) {
        for y in 0..16 { for x in 0..16 {
            let m = (self.tp(mem, mask, x, y) & 8 != 0) ^ invert;
            if m { let v = self.tp(mem, src, x, y); self.stp(mem, dst, x, y, v); }
        } }
    }
    fn xor_and(&self, mem: &mut Mem, a: u16, b: u16, c: u16) {
        for y in 0..16 { for x in 0..16 {
            let v = self.tp(mem, a, x, y) ^ (self.tp(mem, b, x, y) & self.tp(mem, c, x, y));
            self.stp(mem, a, x, y, v);
        } }
    }
    /// Entry 36: tile patching commands (0/1 save+replace/restore row 14 of 0xc8 and 0x117; 2 clock hands;
    /// 3 swap colour bits 1<->2 in terrain tiles; 4 ENDGAME recolour).
    pub fn tile_patch(&mut self, mem: &mut Mem, cmd: u8, hour: u8, minute: u8) {
        if self.tileset == 0 { return; }
        match cmd {
            0 => {
                for (i, &t) in [0xc8u16, 0x117].iter().enumerate() {
                    let base = self.tileset + t as u32 * 128;
                    for k in 0..8u32 { self.patch_save[i][k as usize] = mem.u8(base + 14 * 8 + k); let v = mem.u8(base + k); mem.set_u8(base + 14 * 8 + k, v); }
                }
            }
            1 => {
                for (i, &t) in [0xc8u16, 0x117].iter().enumerate() {
                    let base = self.tileset + t as u32 * 128;
                    for k in 0..8u32 { mem.set_u8(base + 14 * 8 + k, self.patch_save[i][k as usize]); }
                }
            }
            2 => {
                let ring = [(8usize, 2usize), (9, 2), (9, 2), (9, 3), (9, 4), (9, 4), (8, 4), (7, 4), (7, 4), (7, 3), (7, 2), (7, 2)];
                for &t in &[0xfau16, 0xfb] {
                    for &(x, y) in &[(7usize, 2usize), (8, 2), (9, 2), (7, 3), (9, 3), (7, 4), (8, 4), (9, 4)] {
                        let c = self.tp(mem, t, x, y) & 0b0100; // keep plane 2 only
                        self.stp(mem, t, x, y, c);
                    }
                    let (hx, hy) = ring[(hour % 12) as usize]; self.stp(mem, t, hx, hy, 15);
                    let (mx, my) = ring[((minute / 5) % 12) as usize]; self.stp(mem, t, mx, my, 15);
                }
            }
            3 => {
                let mut tiles: Vec<u16> = vec![5, 0x1e, 0x1f, 0x4c, 0xca];
                tiles.extend(0x20..=0x26); tiles.extend(0x30..=0x37); tiles.extend(0x60..=0x6f);
                for t in tiles { for y in 0..16 { for x in 0..16 {
                    let c = self.tp(mem, t, x, y);
                    let n = (c & 0b1001) | ((c & 2) << 1) | ((c & 4) >> 1);
                    self.stp(mem, t, x, y, n);
                } } }
            }
            4 => {
                let map = [0u8, 5, 4, 4, 2, 1, 2, 7, 8, 12, 12, 12, 10, 9, 14, 15];
                for &t in &[0x44u16, 0x5c, 0x5d, 0x90, 0x92, 0x94, 0x96, 0x9b, 0xab, 0xac, 0xaf, 0xb0, 0xb1, 0xbf, 0xdc, 0x108, 0x10e, 0x11a, 0x138, 0x139, 0x13a, 0x13b] {
                    for y in 0..16 { for x in 0..16 { let c = self.tp(mem, t, x, y); self.stp(mem, t, x, y, map[c as usize]); } }
                }
            }
            _ => {}
        }
    }
}
