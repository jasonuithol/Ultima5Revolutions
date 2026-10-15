//! ROOT graphics wrappers (1000:0878–1140) over the driver reimplementation.
//! Parameter order = Ghidra decompiled order (reverse of source order), see game.rs.
use crate::game::Game;
use crate::g;

#[inline] fn clip_enabled(gm: &Game) -> bool { gm.g16(g::CLIP_ENABLED) != 0 }

/// Cohen–Sutherland outcode against the software clip window (0x52d0..0x52d6).
fn outcode(gm: &Game, x: i16, y: i16) -> u8 {
    let mut c = 0;
    if x < gm.gi16(g::CLIP_XMIN) { c |= 1; }
    if x > gm.gi16(g::CLIP_XMAX) { c |= 2; }
    if y < gm.gi16(g::CLIP_YMIN) { c |= 4; }
    if y > gm.gi16(g::CLIP_YMAX) { c |= 8; }
    c
}
/// Returns the clipped line or None if rejected.
fn clip_line(gm: &Game, mut x1: i16, mut y1: i16, mut x2: i16, mut y2: i16) -> Option<(i16, i16, i16, i16)> {
    let (xmin, xmax, ymin, ymax) = (gm.gi16(g::CLIP_XMIN) as i32, gm.gi16(g::CLIP_XMAX) as i32, gm.gi16(g::CLIP_YMIN) as i32, gm.gi16(g::CLIP_YMAX) as i32);
    loop {
        let (c1, c2) = (outcode(gm, x1, y1), outcode(gm, x2, y2));
        if c1 == 0 && c2 == 0 { return Some((x1, y1, x2, y2)); }
        if c1 & c2 != 0 { return None; }
        let c = if c1 != 0 { c1 } else { c2 };
        let (ax, ay, bx, by) = (x1 as i32, y1 as i32, x2 as i32, y2 as i32);
        let (nx, ny) = if c & 8 != 0 { (ax + (bx - ax) * (ymax - ay) / (by - ay).max(1).min(by - ay).max(if by - ay == 0 { 1 } else { by - ay }), ymax) }
            else if c & 4 != 0 { (ax + (bx - ax) * (ymin - ay) / (if by - ay == 0 { 1 } else { by - ay }), ymin) }
            else if c & 2 != 0 { (xmax, ay + (by - ay) * (xmax - ax) / (if bx - ax == 0 { 1 } else { bx - ax })) }
            else { (xmin, ay + (by - ay) * (xmin - ax) / (if bx - ax == 0 { 1 } else { bx - ax })) };
        if c == c1 { x1 = nx as i16; y1 = ny as i16; } else { x2 = nx as i16; y2 = ny as i16; }
    }
}

pub fn restore_video_mode(_gm: &mut Game) {}

/// init_graphics_driver(set_mode)
pub fn init_graphics_driver(gm: &mut Game, set_mode: i16) {
    if set_mode != 0 && gm.gi16(g::SAVED_VIDEO_MODE) == -1 { gm.set_gi16(g::SAVED_VIDEO_MODE, 3); }
    if set_mode != 0 { gm.pf.video.init(); }
    gm.set_g16(g::DRV_SEG, 0x3000); // "driver loaded"
}
pub fn driver_init_screen(gm: &mut Game) {
    gm.pf.video.pages[1].iter_mut().for_each(|p| *p = 0);
    gm.set_g16(g::SCRATCH_SEG, 0xa800);
}
pub fn load_video_driver(gm: &mut Game) -> i16 { gm.set_gi16(g::ADAPTER_TYPE, 1); gm.set_g16(g::DRV_SEG, 0x3000); 1 }
pub fn driver_set_speed(gm: &mut Game) { let s = gm.g16(g::CPU_SPEED); gm.pf.video.speed = s; }
pub fn detect_video_adapter(gm: &mut Game) { gm.set_gi16(g::ADAPTER_TYPE, 1); gm.set_gi16(g::MACHINE_CLASS, 2); }

/// set_color(color)
pub fn set_color(gm: &mut Game, color: i16) { gm.set_gi16(g::CUR_COLOR, color); gm.pf.video.set_color(color as u8); }
/// fill_rect(x1,y1,x2,y2) — Ghidra order (y2,x2,y1,x1)
pub fn fill_rect(gm: &mut Game, y2: i16, x2: i16, y1: i16, x1: i16) { gm.pf.video.fill_rect(x1, y1, x2, y2, false); }
pub fn fill_rect_xor(gm: &mut Game, y2: i16, x2: i16, y1: i16, x1: i16) { gm.pf.video.fill_rect(x1, y1, x2, y2, true); }
/// copy_rect_between_pages(src,dst,x1,y1,x2,y2) — Ghidra order (y2,x2,y1,x1,dst,src). Only 1->0 does anything.
pub fn copy_rect_between_pages(gm: &mut Game, y2: i16, x2: i16, y1: i16, x1: i16, dst: i16, src: i16) -> i16 {
    if src == dst || src > 1 || dst > 1 || src < 0 || dst < 0 { return 0; }
    if src == 1 { gm.pf.video.copy_rect_back_to_front(x1, y1, x2, y2); }
    0
}
/// draw_line(x1,y1,x2,y2) — Ghidra order (y2,x2,y1,x1). Always page 0. Sets the pen to (x2,y2).
pub fn draw_line(gm: &mut Game, y2: i16, x2: i16, y1: i16, x1: i16) {
    gm.set_gi16(g::PEN_X, x2); gm.set_gi16(g::PEN_Y, y2);
    if clip_enabled(gm) {
        if let Some((a, b, c, d)) = clip_line(gm, x1, y1, x2, y2) { gm.pf.video.draw_line(a, b, c, d, false); }
    } else { gm.pf.video.draw_line(x1, y1, x2, y2, false); }
}
/// draw_line_to(x2,y2) — Ghidra order (y2,x2): from the pen position.
pub fn draw_line_to(gm: &mut Game, y2: i16, x2: i16) {
    let (x1, y1) = (gm.gi16(g::PEN_X), gm.gi16(g::PEN_Y));
    draw_line(gm, y2, x2, y1, x1);
}
/// plot_pixel(x,y) — Ghidra order (y,x)
pub fn plot_pixel(gm: &mut Game, y: i16, x: i16) {
    if clip_enabled(gm) && outcode(gm, x, y) != 0 { return; }
    gm.pf.video.plot_pixel(x, y, false);
}
/// draw_hline(x1,y,x2) — Ghidra order (x2,y,x1)
pub fn draw_hline(gm: &mut Game, x2: i16, y: i16, x1: i16) {
    let (mut a, mut b) = if x1 <= x2 { (x1, x2) } else { (x2, x1) };
    if clip_enabled(gm) {
        if y < gm.gi16(g::CLIP_YMIN) || y > gm.gi16(g::CLIP_YMAX) { return; }
        a = a.max(gm.gi16(g::CLIP_XMIN)); b = b.min(gm.gi16(g::CLIP_XMAX));
        if a > b { return; }
    }
    gm.pf.video.draw_hline(a, y, b, false);
}
/// draw_vline(x,y1,y2) — Ghidra order (y2,y1,x)
pub fn draw_vline(gm: &mut Game, y2: i16, y1: i16, x: i16) {
    let (mut a, mut b) = if y1 <= y2 { (y1, y2) } else { (y2, y1) };
    if clip_enabled(gm) {
        if x < gm.gi16(g::CLIP_XMIN) || x > gm.gi16(g::CLIP_XMAX) { return; }
        a = a.max(gm.gi16(g::CLIP_YMIN)); b = b.min(gm.gi16(g::CLIP_YMAX));
        if a > b { return; }
    }
    gm.pf.video.draw_vline(x, a, b, false);
}
pub fn set_draw_page(gm: &mut Game, page: i16) { gm.set_gi16(g::DRAW_PAGE, page); gm.pf.video.draw_page = (page & 1) as usize; }
/// set_clip_window(xmin,ymin,xmax,ymax) — Ghidra order (ymax,xmax,ymin,xmin)
pub fn set_clip_window(gm: &mut Game, ymax: i16, xmax: i16, ymin: i16, xmin: i16) {
    gm.set_gi16(g::CLIP_XMIN, xmin); gm.set_gi16(g::CLIP_YMIN, ymin); gm.set_gi16(g::CLIP_XMAX, xmax); gm.set_gi16(g::CLIP_YMAX, ymax);
    gm.pf.video.clip_ymin = ymin; gm.pf.video.clip_ymax = ymax;
}
/// copy_page(src,dst) — Ghidra order (dst,src)
pub fn copy_page(gm: &mut Game, dst: i16, src: i16) { gm.pf.video.copy_page((src & 1) as usize, (dst & 1) as usize); }
fn sync_clip(gm: &mut Game) {
    gm.pf.video.clip_enable = clip_enabled(gm);
    gm.pf.video.clip_ymin = gm.gi16(g::CLIP_YMIN); gm.pf.video.clip_ymax = gm.gi16(g::CLIP_YMAX);
}
/// draw_image(seg,index,x,y,mode) — Ghidra order (mode,y,x,index,seg)
pub fn draw_image(gm: &mut Game, mode: i16, y: i16, x: i16, index: i16, seg: u16) {
    sync_clip(gm);
    gm.pf.video.draw_image(&gm.m, (seg as u32) << 4, index as u16, x, y, mode as u16);
}
/// draw_image_alt: entry 33 = mirrored variant.
pub fn draw_image_alt(gm: &mut Game, mode: i16, y: i16, x: i16, index: i16, seg: u16) {
    sync_clip(gm);
    gm.pf.video.draw_image(&gm.m, (seg as u32) << 4, index as u16, x, y, mode as u16 | 2);
}
/// draw_image_masked(seg,index,x,y) — Ghidra order (y,x,index,seg): 1-bpp white sprite onto page 1.
pub fn draw_image_masked(gm: &mut Game, y: i16, x: i16, index: i16, seg: u16) {
    gm.pf.video.draw_bitmap_white(&gm.m, (seg as u32) << 4, index as u16, x, y);
}
/// draw_tile(tile,col,row) — Ghidra order (row,col,tile)
pub fn draw_tile(gm: &mut Game, row: i16, col: i16, tile: i16) {
    let (cw, ch) = (gm.g16(g::CELL_W) as u8, gm.g16(g::CELL_H) as u8);
    gm.pf.video.draw_tile(&gm.m, col as u8, row as u8, tile as u16, cw, ch);
}
/// draw_effect_frame(frame,col,row) — Ghidra order (row,col,frame)
pub fn draw_effect_frame(gm: &mut Game, row: i16, col: i16, frame: i16) {
    let (cw, ch) = (gm.g16(g::CELL_W) as u8, gm.g16(g::CELL_H) as u8);
    let dungeon = gm.g8(g::LOCATION) == 0xff;
    gm.pf.video.draw_effect_frame(&gm.m, frame as u8, col as u8, row as u8, dungeon, cw, ch);
}
/// tile_transition_effect(tile,col,row) — Ghidra order (row,col,tile). 256-step tile dissolve; polls keys every 8 steps.
pub fn tile_transition_effect(gm: &mut Game, row: i16, col: i16, tile: i16) -> i16 {
    let (cw, ch) = (gm.g16(g::CELL_W) as u8, gm.g16(g::CELL_H) as u8);
    for step in 0..256u16 {
        gm.pf.video.tile_dissolve_step(&gm.m, col as u8, row as u8, tile as u16, step as u8, cw, ch);
        if step & 7 == 7 {
            if gm.g8(g::LOCATION) == b'B' {
                let r = crate::ovl::font::view_idle_tick(gm, 1);
                if r != 0 { return r as i16; }
            } else {
                crate::engine::view::refresh_map_view(gm);
            }
            gm.pf.maybe_present();
        }
    }
    0
}
/// rect_effect(x1,y1,x2,y2) — Ghidra order (y2,x2,y1,x1): dissolve page 1 -> page 0 in LFSR order.
/// While the driver's "dissolve makes sound" flag is set (until the first glyph is ever drawn) every
/// second pixel plays one click of the driver noise routine: the speaker is gated on, one random
/// tone of (speed>>4) loop iterations in [100, max/2-100] Hz (max starts at 240 and grows by one
/// per click), then gated off; a pending key aborts. The clicks are synthesised here and the pixel
/// loop is paced to the same wall-clock duration.
pub fn rect_effect(gm: &mut Game, y2: i16, x2: i16, y1: i16, x1: i16) -> i16 {
    let sound = gm.pf.video.dissolve_sound;
    let (ax, bx) = if x1 <= x2 { (x1 as i32, x2 as i32) } else { (x2 as i32, x1 as i32) };
    let (ay, by) = if y1 <= y2 { (y1 as i32, y2 as i32) } else { (y2 as i32, y1 as i32) };
    let n = ((bx - ax + 1) * (by - ay + 1)).max(1) as u32;
    let iu = crate::engine::util::iter_us(gm);
    let pixel_us = 30.0 * iu;                                    // ~30 instructions per pixel
    let click_us = ((gm.g16(g::CPU_SPEED) >> 4) as f64 + 15.0) * iu;
    let mut total_us = n as f64 * pixel_us;
    if sound {
        let mut sy = crate::platform::sound::Synth::new();
        let mut freq_max: u32 = 240;
        for i in 0..n {
            if i % 2 == 1 {
                let seed = &mut gm.pf.video.noise_seed;
                *seed = (seed.wrapping_add(0x9248).rotate_right(3) ^ 0x9248).wrapping_add(0x11);
                let span = (freq_max / 2).saturating_sub(100) + 1;
                let f = 100 + (*seed as u32) % span;
                sy.tone(f as f64, click_us);
                sy.tone(0.0, pixel_us);
                freq_max += 1;
            } else { sy.tone(0.0, pixel_us); }
        }
        total_us = sy.samples.len() as f64 * 1_000_000.0 / crate::platform::sound::RATE as f64;
        if gm.g16(g::SOUND_ENABLED) != 0 { gm.pf.speaker.play_samples(&sy.samples); }
    }
    let start = std::time::Instant::now();
    let mut aborted = false;
    let mut v = std::mem::replace(&mut gm.pf.video, crate::platform::video::Video::new());
    v.dissolve_rect(x1, y1, x2, y2, |_v, count| {
        if count % 64 == 0 {
            let due = start + std::time::Duration::from_micros((total_us * count as f64 / n as f64) as u64);
            let now = std::time::Instant::now();
            if due > now { std::thread::sleep(due - now); }
            gm.pf.maybe_present();
            if sound && gm.pf.key_available().is_some() { aborted = true; return false; }
        }
        true
    });
    gm.pf.video = v;
    gm.pf.pump();
    aborted as i16
}
/// load_image_file(name) -> segment (0 on error). The image file stays in its packed format.
pub fn load_image_file(gm: &mut Game, name: u16) -> i16 {
    match crate::engine::files::load_file(gm, name) { Some((_, seg)) => seg as i16, None => 0 }
}
pub fn load_file_to_mem(gm: &mut Game, name: u16) -> u16 {
    match crate::engine::files::load_file(gm, name) { Some((_, seg)) => seg, None => 0 }
}
pub fn free_far_block(gm: &mut Game, seg: u16) { gm.free_seg(seg); }
pub fn free_far_block2(gm: &mut Game, seg: u16) { gm.free_seg(seg); }
/// load_picture_file(name): registers the tile set (TILES.16) or a 1-bpp picture file with the driver.
pub fn load_picture_file(gm: &mut Game, name: u16) -> i16 {
    match crate::engine::files::load_file(gm, name) {
        Some((_, seg)) => { gm.pf.video.tileset = (seg as u32) << 4; 1 }
        None => 0,
    }
}
pub fn free_picture(gm: &mut Game) {
    let t = gm.pf.video.tileset;
    if t != 0 { gm.heap.free(t, 0); gm.pf.video.tileset = 0; }
}
/// draw_stacked_pictures(pic_seg): the 7 title strips onto page 1, then title zoom.
pub fn draw_stacked_pictures(gm: &mut Game, pic_seg: u16) -> i16 {
    set_draw_page(gm, 1);
    set_color(gm, 0);
    fill_rect(gm, 199, 319, 0, 0);
    let mut y: i16 = 0;
    for i in 0..7u16 {
        let w = gm.gi16(g::PIC_WIDTHS + i * 2);
        let h = gm.gi16(g::PIC_HEIGHTS + i * 2);
        draw_image_masked(gm, y, (320 - w) / 2, i as i16, pic_seg);
        y += h;
    }
    set_draw_page(gm, 0);
    driver_set_speed(gm);
    // returns the driver's AX: 1 when a key aborted the zoom (INTRO tests it)
    crate::engine::effects::title_zoom(gm) as i16
}
