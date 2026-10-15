//! FONT.OVL (translated)
//!
//! Level-3 overlay (load segment 1e1e = 1000:e1e0, shared with CAST2/ZSTATS/COMSUBS/SHOPPES2/3).
//! Three services for the intro: the proportional-font text renderer (`draw_proportional_text`,
//! vector 1778:0566, PROPORT.PCS glyphs drawn with draw_image_masked on page 1, word-wrapped and
//! justified inside the layout block 0x5146..0x5158), "the View" (`run_the_view`, vector
//! 1778:055a: the scripted demo animation of MISCMAPS.DAT bytes 0x2c0.. that plays after the
//! title and on 'R'eturn to the View), and "Create New Character" (`create_new_character`,
//! vector 1778:054e: name, sex, the gypsy's seven card questions from QUESTION.DAT, then
//! INIT.GAM/INIT.OOL are written as SAVED.GAM/SAVED.OOL on disk 3). `view_idle_tick` (vector
//! 1778:0a3a) is the View's per-frame routine, also used by ROOT `tile_transition_effect` while
//! g_location == 'B'.
//!
//! Layout block (all u16): 0x5146/0x5148 left x of band 0/1, 0x514c/0x514e right x of band 0/1,
//! 0x5150 < y < 0x5152 selects band 1, 0x5154 g_font_space_width, 0x5156 cursor x, 0x5158
//! cursor y (lines are 9 pixels; nothing is drawn at y >= 0xc0). Glyph widths: byte table at
//! 0x50ca indexed by the character (0x50f7 = width of '-'); '_' is a soft hyphen (glyph 0xd),
//! '{' a 15-pixel gap, bytes < 0x21 are spaces. The View's RAM: 0xbd26/0xbd27 first/last
//! visible column, 0xbd28 widen toggle, 0xbd29 scene index, 0x515a beep phase; the 4-row scene
//! map lives at g_map 0x6608 (current) / 0x6708 (ground) with stride 0x20, objects in
//! g_monster_table. Character creation: 0xbd2a[8] cards shown this round, 0xbd32[8] cards
//! eliminated, 0xbd3c/0xbd3d/0xbd3e int/dex/str accumulators, 0x5164/0x516c/0x5174 per-card
//! int/dex/str bonuses, 0x517c 8x8 u16 QUESTION.DAT offsets, 0x51fc/0x5204 card image x/y.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// Glyph width table 0x50ca (the tail of the string at 0x50bd + 0xd), indexed by character.
#[inline] fn glyph_w(gm: &Game, c: u8) -> i16 { gm.g8(0x50cau16.wrapping_add(c as u16)) as i16 }
#[inline] fn txt(gm: &Game, p: i16, i: i16) -> u8 { gm.g8((p as u16).wrapping_add(i as u16)) }

/// orig 1000:e1e0 (671 bytes). Vector 1778:0566 (INTRO story screens, Create). Draws the
/// NUL-terminated string at `text` with the proportional font `seg` (PROPORT.PCS, glyph index =
/// char - 0x20) on page 1, word-wrapping inside the layout block: the band (0 or 1, giving the
/// left margin 0x5146[band] and right edge 0x514c[band]) is 1 while 0x5150 < y < 0x5152. For
/// each line the width is measured forward (space = g_font_space_width, '{' = 15, '_' = 0,
/// else glyph width + 1) until the right edge or '\n'; then it backs up to the last space (which
/// is dropped) or a '_' soft hyphen that still fits with a '-'; the remaining slack is spread
/// over the line's spaces (full justification, except on the last line of a paragraph). Glyphs
/// are only drawn while y < 0xc0; a hyphen glyph (0xd) is drawn at a '_' break. Each line
/// advances y by 9 and resets x to the band's left margin. Ends on NUL with page 0 selected.
/// Ghidra order: (text, seg).
pub fn draw_proportional_text(gm: &mut Game, param_1: i16, param_2: u16) {
    let (text, seg) = (param_1, param_2);
    gfx::set_draw_page(gm, 1);
    let y = gm.gi16(0x5158);
    let band: u16 = if gm.gi16(0x5150) < y && y < gm.gi16(0x5152) { 1 } else { 0 };
    let mut width: u16 = gm.g16(0x514cu16.wrapping_add(band * 2)).wrapping_sub(gm.g16(0x5146u16.wrapping_add(band * 2)));
    let mut pos: i16 = 0;
    let mut cur: i16 = gm.gi16(0x5156).wrapping_sub(gm.gi16(0x5146u16.wrapping_add(band * 2)));
    loop {
        if txt(gm, text, pos) == 0 {
            gfx::set_draw_page(gm, 0);
            return;
        }
        let mut spaces: i16 = 0;
        let mut i: i16 = pos;
        while txt(gm, text, i) != 0 && cur < width as i16 && txt(gm, text, i) != b'\n' {
            let c = txt(gm, text, i);
            if c < 0x21 {
                cur = cur.wrapping_add(gm.gi16(g::FONT_SPACE_WIDTH));
                spaces = spaces.wrapping_add(1);
            } else if c == b'{' {
                cur = cur.wrapping_add(0xf);
            } else if c != b'_' {
                cur = cur.wrapping_add(glyph_w(gm, c).wrapping_add(1));
            }
            i = i.wrapping_add(1);
        }
        // back up to a break point
        let mut hyphen = false;
        loop {
            let c = txt(gm, text, i);
            if (c == 0 && cur < width as i16) || c == b'\n' {
                break;
            }
            i = i.wrapping_sub(1);
            if i <= pos { break; }
            let c = txt(gm, text, i);
            if c == b' ' {
                cur = cur.wrapping_sub(gm.gi16(g::FONT_SPACE_WIDTH));
                spaces = spaces.wrapping_sub(1);
                break;
            }
            if c == b'_' && (gm.g8(0x50f7) as u16).wrapping_add(cur as u16).wrapping_add(1) < width {
                hyphen = true;
                break;
            }
            if c != b'_' && c != b'{' {
                cur = cur.wrapping_sub(glyph_w(gm, c).wrapping_add(1));
            }
        }
        if hyphen {
            cur = cur.wrapping_add(gm.g8(0x50f7) as i16).wrapping_add(1);
        }
        // LAB_1000_e2d0: draw the line pos..i
        let mut slack: i16 = (width as i16).wrapping_sub(cur);
        if pos < i {
            let brk = txt(gm, text, i);
            loop {
                let c = txt(gm, text, pos);
                if c < 0x21 {
                    let x = gm.gi16(0x5156).wrapping_add(gm.gi16(g::FONT_SPACE_WIDTH));
                    gm.set_gi16(0x5156, x);
                    if spaces != 0 && brk != 0 && brk != b'\n' {
                        let x = gm.gi16(0x5156).wrapping_add(slack / spaces);
                        gm.set_gi16(0x5156, x);
                        slack = slack.wrapping_sub(slack / spaces);
                        spaces = spaces.wrapping_sub(1);
                    }
                } else if c == b'{' {
                    let x = gm.gi16(0x5156).wrapping_add(0xf);
                    gm.set_gi16(0x5156, x);
                } else if c != b'_' {
                    if gm.gi16(0x5158) < 0xc0 {
                        let (y, x) = (gm.gi16(0x5158), gm.gi16(0x5156));
                        gfx::draw_image_masked(gm, y, x, (c as i16).wrapping_sub(0x20), seg);
                    }
                    let x = gm.gi16(0x5156).wrapping_add(glyph_w(gm, c).wrapping_add(1));
                    gm.set_gi16(0x5156, x);
                }
                pos = pos.wrapping_add(1);
                if i == pos { break; }
            }
        }
        pos = i;
        if txt(gm, text, i) != 0 {
            if txt(gm, text, i) == b'_' && gm.gi16(0x5158) < 0xc0 {
                let (y, x) = (gm.gi16(0x5158), gm.gi16(0x5156));
                gfx::draw_image_masked(gm, y, x, 0xd, seg);
            }
            cur = 0;
            let y = gm.gi16(0x5158).wrapping_add(9);
            gm.set_gi16(0x5158, y);
            let band: u16 = if gm.gi16(0x5150) < y && y < gm.gi16(0x5152) { 1 } else { 0 };
            let left = gm.g16(0x5146u16.wrapping_add(band * 2));
            gm.set_g16(0x5156, left);
            width = gm.g16(0x514cu16.wrapping_add(band * 2)).wrapping_sub(left);
            pos = i.wrapping_add(1);
        }
    }
}

/// orig 1000:e482 (89 bytes). Draws one cell of the View's 4-row scene at view row `row`
/// (0..3, screen row row+7) and column `col`: the terrain byte g_map[row*0x20+col] (0x6608) is
/// drawn through g_tile_anim_map; a 0 terrain byte means an object: the byte at 0x6688 (the
/// object layer, 4 rows below) is drawn as tile 0x100+obj unless it is 0x16 (nothing).
/// Ghidra order: (row, col).
pub fn view_draw_cell(gm: &mut Game, param_1: i16, param_2: i16) {
    let (row, col) = (param_1, param_2);
    let off = (col as u16).wrapping_add((row as u16).wrapping_mul(0x20));
    let terrain = gm.g8(g::MAP.wrapping_add(off));
    let tile: u16;
    if terrain == 0 {
        let obj = gm.g8(0x6688u16.wrapping_add(off)); // object layer of the View scene
        if obj == 0x16 { return; }
        tile = 0x100 | obj as u16;
    } else {
        tile = gm.g8(g::TILE_ANIM_MAP.wrapping_add(terrain as u16)) as u16;
    }
    gfx::draw_tile(gm, row.wrapping_add(7), col, tile as i16);
}

/// orig 1000:e4dc (282 bytes). Vector 1778:0a3a; the View's frame routine, also called by ROOT
/// `tile_transition_effect` while g_location == 'B'. Runs `count` frames: animate_monsters,
/// INTRO title_fire_tick, every monster-table object (tile != 0) is stamped into the scene
/// (terrain 0, object layer 0x6688 = its animation tile); columns 0xbd26..=0xbd27 of the 4 rows
/// are drawn with `view_draw_cell` (cells holding 0xfe are skipped); while 0xbd26 != 0 the
/// visible range widens by one column on every other frame (0xbd28 toggles). Any key returns 1.
/// `count` != 0 waits one tick per frame; scene 2 plays play_random_tones(10000, 60, 20) each
/// frame, scene 3 beeps (0x515a cycles 0..7: 3000 Hz at 0, 2000 Hz at 4). Returns 0 after
/// `count` frames (count 0 runs until a key).
pub fn view_idle_tick(gm: &mut Game, param_1: i16) -> u16 {
    let mut count = param_1;
    loop {
        rest::animate_monsters(gm);
        ovl::intro::title_fire_tick(gm);
        let mut p: u16 = g::MONSTER_TABLE;
        loop {
            if gm.g8(p) != 0 {
                let off = (gm.g8(p.wrapping_add(3)) as u16).wrapping_mul(0x20).wrapping_add(gm.g8(p.wrapping_add(2)) as u16);
                gm.set_g8(g::MAP.wrapping_add(off), 0);
                let anim = gm.g8(p.wrapping_add(1));
                gm.set_g8(0x6688u16.wrapping_add(off), anim);
            }
            p = p.wrapping_add(8);
            if !(p < 0x5d5a) { break; }
        }
        let mut col: u16 = gm.g8(0xbd26) as u16;
        while col <= gm.g8(0xbd27) as u16 {
            let mut row: i16 = 0;
            let mut q = col.wrapping_add(g::MAP);
            loop {
                if gm.gi8(q) != -2 {
                    view_draw_cell(gm, row, col as i16);
                }
                q = q.wrapping_add(0x20);
                row += 1;
                if !(row < 4) { break; }
            }
            col = col.wrapping_add(1);
        }
        if gm.g8(0xbd26) != 0 && gm.g8(0xbd28) != 0 {
            let v = gm.g8(0xbd26).wrapping_sub(1);
            gm.set_g8(0xbd26, v);
            let v = gm.g8(0xbd27).wrapping_add(1);
            gm.set_g8(0xbd27, v);
        }
        let t = (gm.g8(0xbd28) == 0) as u8;
        gm.set_g8(0xbd28, t);
        gm.pf.maybe_present();
        if input::read_key(gm) != 0 {
            return 1;
        }
        if count != 0 {
            util::wait_ticks(gm, 1);
        }
        let scene = gm.g8(0xbd29);
        if scene == 2 {
            util::play_random_tones(gm, 0x2710, 0x3c, 0x14); // 10000 = s_Missed_ + 1 in Ghidra
        } else if scene == 3 {
            let mut ph = gm.gi16(0x515a).wrapping_add(1);
            if 7 < ph { ph = 0; }
            gm.set_gi16(0x515a, ph);
            let freq: u16;
            if ph == 0 {
                freq = 3000;
                util::beep(gm, 3, freq);
            } else if ph == 4 {
                freq = 2000;
                util::beep(gm, 3, freq);
            }
        }
        count = count.wrapping_sub(1);
        if count == 0 {
            return 0;
        }
    }
}

/// orig 1000:e5f8 (140 bytes). Loads View scene `index` (0..3): INTRO `draw_bottom_banner`
/// with the caption pointer 0x515c[index], then the 19-column x 4-row block at g_ool_buffer +
/// index*0x80 (stride 0x20) is copied into both g_map 0x6608 (current) and 0x6708 (ground).
/// Visible range 0xbd26 = 0xbd27 = 9, toggle 0xbd28 = 0, scene number 0xbd29 = index.
pub fn view_load_scene(gm: &mut Game, param_1: i16) {
    let idx = param_1;
    let caption = gm.g16(0x515cu16.wrapping_add((idx as u16).wrapping_mul(2)));
    ovl::intro::draw_bottom_banner(gm, caption);
    let mut col: u16 = 0;
    loop {
        let mut dst_ground = col.wrapping_add(0x6708);
        let mut dst_cur = col.wrapping_add(g::MAP);
        let base = (idx as u16).wrapping_mul(0x80).wrapping_add(col);
        let mut src = g::OOL_BUFFER.wrapping_add(base);
        let end = base.wrapping_add(0xb29e); // g_ool_buffer + base + 0x80 (4 rows)
        loop {
            let v = gm.g8(src);
            gm.set_g8(dst_cur, v);
            gm.set_g8(dst_ground, v);
            dst_ground = dst_ground.wrapping_add(0x20);
            dst_cur = dst_cur.wrapping_add(0x20);
            src = src.wrapping_add(0x20);
            if !(src < end) { break; }
        }
        col = col.wrapping_add(1);
        if !((col as i16) < 0x13) { break; }
    }
    gm.set_g8(0xbd27, 9);
    gm.set_g8(0xbd26, 9);
    gm.set_g8(0xbd28, 0);
    gm.set_g8(0xbd29, idx as u8);
}

#[inline] fn mon_tile(slot: u16) -> u16 { g::MONSTER_TABLE.wrapping_add(slot.wrapping_mul(8)) }
#[inline] fn mon_x(slot: u16) -> u16 { g::MONSTER_X.wrapping_add(slot.wrapping_mul(8)) }
#[inline] fn mon_y(slot: u16) -> u16 { g::MONSTER_X.wrapping_add(slot.wrapping_mul(8)).wrapping_add(1) }

/// orig 1000:e684 (1225 bytes). Vector 1778:055a: "the View", the scripted demo after the
/// title / on 'R'. g_cell_h = 16, visible columns 9..9, a g_ui_color2 bar 0x78..0x7e with a
/// g_ui_color1 line at y 0x7f. Unless g_location is already 'B', 2000 bytes of MISCMAPS.DAT
/// from 0x2c0 are loaded over g_ool_buffer (scene maps at 0..0x1ff, script from 0x200) and, if
/// nothing was registered yet ('@'), the LZW scratch segment is saved into the FLAMES area,
/// TILES.16 registered and the scratch restored. g_location = 'B'. Script bytes from 0x200
/// (indexes into g_ool_buffer; m = g_monster_table slot): 0 slot,tile,x,y place object; 1 slot
/// remove (ground restored); 2 slot,dir step (0x24d6/0x24de deltas) restoring the ground;
/// 3 n = n idle frames; 4 x,y explosion frames 1..15 then tile 0xdc + 2 frames; 5 implosion
/// 15..1 at the same x,y then tile 5 + 2 frames; 6 n load scene n; 7 slot dissolve to the
/// object's own tile; 8 slot dissolve to the ground tile; 9 restart at 0x200; 10 tile,x,y set a
/// ground tile; 11 lightning: 5 double lines from (0x80+9k, 0x98+3k) with an idle frame each,
/// tile 0 at slot[+2]'s position, play_random_tones(4000,1200,1), 3 frames; 12 clear all
/// slots; 13 slot,dir step then 7 frames; 14 n loop start; 15 loop end (repeat n times). Any
/// key (read_key after each opcode, or inside the frame routine) returns; running past byte
/// 1999 returns too.
pub fn run_the_view(gm: &mut Game) {
    gm.set_gi16(g::CELL_H, 0x10);
    gm.set_g8(0xbd27, 9);
    gm.set_g8(0xbd26, 9);
    gm.set_g8(0xbd28, 0);
    let c2 = gm.gi16(g::UI_COLOR2);
    gfx::set_color(gm, c2);
    gfx::fill_rect(gm, 0x7e, 199, 0x78, 0x78);
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    gfx::draw_line(gm, 0x7f, 199, 0x7f, 0x78);
    if gm.g8(g::LOCATION) != b'B' {
        files::load_file_region(gm, 0x2c0, 2000, g::OOL_BUFFER, 0xa044); // "MISCMAPS.DAT"
        if gm.g8(g::LOCATION) == b'@' {
            save_scratch_to_overlay_area(gm);
            loop {
                let nm = gm.g16(0x25f0); // "TILES.16"
                if gfx::load_picture_file(gm, nm) != 0 { break; }
            }
            restore_scratch_from_overlay_area(gm);
        }
    }
    gm.set_g8(g::LOCATION, 0x42);
    let buf = |e: i16, k: u16| -> u16 { g::OOL_BUFFER.wrapping_add(e as u16).wrapping_add(k) };
    let mut e: i16 = 0x200;
    let (mut ex, mut ey): (u16, u16) = (0, 0); // local_6 / local_8 (persist across opcodes 4/5)
    let mut loop_start: i16 = 0;
    let mut loop_count: u16 = 0;
    loop {
        if 1999 < e { return; }
        let op = gm.g8(buf(e, 0));
        let mut wait_frames: Option<i16> = None;
        match op {
            0 => {
                let slot = gm.g8(buf(e, 1)) as u16;
                let tile = gm.g8(buf(e, 2));
                gm.set_g8(mon_tile(slot).wrapping_add(1), tile);
                gm.set_g8(mon_tile(slot), tile);
                let x = gm.g8(buf(e, 3));
                gm.set_g8(mon_x(slot), x);
                let y = gm.g8(buf(e, 4));
                gm.set_g8(mon_y(slot), y);
                gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(slot.wrapping_mul(8)), 0);
                e = e.wrapping_add(4);
            }
            1 => {
                let slot = gm.g8(buf(e, 1)) as u16;
                gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(slot.wrapping_mul(8)), 0);
                gm.set_g8(mon_tile(slot).wrapping_add(1), 0);
                gm.set_g8(mon_tile(slot), 0);
                let off = (gm.g8(mon_y(slot)) as u16).wrapping_mul(0x20).wrapping_add(gm.g8(mon_x(slot)) as u16);
                let v = gm.g8(0x6708u16.wrapping_add(off));
                e = e.wrapping_add(1);
                gm.set_g8(g::MAP.wrapping_add(off), v); // LAB_1000_e790
            }
            2 | 0xd => {
                let slot = gm.g8(buf(e, 1)) as u16;
                let dir = gm.g8(buf(e, 2)) as u16;
                let off = (gm.g8(mon_y(slot)) as u16).wrapping_mul(0x20).wrapping_add(gm.g8(mon_x(slot)) as u16);
                let v = gm.g8(0x6708u16.wrapping_add(off));
                gm.set_g8(g::MAP.wrapping_add(off), v);
                let d2 = dir.wrapping_mul(2);
                let nx = gm.gi8(mon_x(slot)).wrapping_add(gm.gi8(0x24d6u16.wrapping_add(d2))); // direction dx table
                gm.set_gi8(mon_x(slot), nx);
                let ny = gm.gi8(mon_y(slot)).wrapping_add(gm.gi8(0x24deu16.wrapping_add(d2))); // direction dy table
                gm.set_gi8(mon_y(slot), ny);
                e = e.wrapping_add(2);
                if op == 0xd { wait_frames = Some(7); }
            }
            3 => {
                let n = gm.g8(buf(e, 1)) as i16;
                e = e.wrapping_add(1);
                wait_frames = Some(n);
            }
            4 | 5 => {
                if op == 4 {
                    ex = gm.g8(buf(e, 1)) as u16;
                    ey = gm.g8(buf(e, 2)) as u16;
                }
                let off = ey.wrapping_mul(0x20).wrapping_add(ex);
                gm.set_g8(0x6708u16.wrapping_add(off), 0xfe);
                gm.set_g8(g::MAP.wrapping_add(off), 0xfe);
                if op == 4 {
                    let mut frame: i16 = 1;
                    loop {
                        gfx::draw_effect_frame(gm, (ey as i16).wrapping_add(7), ex as i16, frame);
                        if view_idle_tick(gm, 1) != 0 { return; }
                        frame += 1;
                        if !(frame < 0x10) { break; }
                    }
                } else {
                    let mut frame: i16 = 0xf;
                    loop {
                        gfx::draw_effect_frame(gm, (ey as i16).wrapping_add(7), ex as i16, frame);
                        if view_idle_tick(gm, 1) != 0 { return; }
                        frame -= 1;
                        if !(0 < frame) { break; }
                    }
                }
                let off = ey.wrapping_mul(0x20).wrapping_add(ex);
                let tile: u8 = if op == 4 { 0xdc } else { 5 };
                if op == 4 { e = e.wrapping_add(2); }
                // LAB_1000_e865
                gm.set_g8(0x6708u16.wrapping_add(off), tile);
                gm.set_g8(g::MAP.wrapping_add(off), tile);
                wait_frames = Some(2);
            }
            6 => {
                let n = gm.g8(buf(e, 1)) as i16;
                view_load_scene(gm, n);
                e = e.wrapping_add(1);
            }
            7 | 8 => {
                let slot = gm.g8(buf(e, 1)) as u16;
                let rec = mon_tile(slot);
                let saved = gm.g8(rec);
                let r: i16;
                if op == 7 {
                    gm.set_g8(rec.wrapping_add(1), 0x16);
                    gm.set_g8(rec, 0x16);
                    let (y, x) = (gm.g8(mon_y(slot)) as i16, gm.g8(mon_x(slot)) as i16);
                    r = gfx::tile_transition_effect(gm, y.wrapping_add(7), x, (0x100u16 | saved as u16) as i16);
                } else {
                    let bx = gm.g8(mon_x(slot)) as u16;
                    let uy = gm.g8(mon_y(slot)) as u16;
                    gm.set_g8(rec.wrapping_add(1), 0x16);
                    gm.set_g8(rec, 0x16);
                    let ground = gm.g8(bx.wrapping_add(uy.wrapping_mul(0x20)).wrapping_add(0x6708)) as i16;
                    r = gfx::tile_transition_effect(gm, (uy as i16).wrapping_add(7), bx as i16, ground);
                }
                if r != 0 { return; }
                // LAB_1000_e91a
                e = e.wrapping_add(1);
                gm.set_g8(rec.wrapping_add(1), saved);
                gm.set_g8(rec, saved);
            }
            9 => {
                e = 0x1ff;
            }
            10 => {
                let tile = gm.g8(buf(e, 1));
                let off = (gm.g8(buf(e, 3)) as u16).wrapping_mul(0x20).wrapping_add(gm.g8(buf(e, 2)) as u16);
                gm.set_g8(0x6708u16.wrapping_add(off), tile);
                e = e.wrapping_add(3);
                gm.set_g8(g::MAP.wrapping_add(off), tile); // LAB_1000_e790
            }
            0xb => {
                let mut yy: i16 = 0;
                let mut k: i16 = 0;
                let mut xx: i16 = 0;
                loop {
                    if view_idle_tick(gm, 1) != 0 { return; }
                    let c1 = gm.gi16(g::UI_COLOR1);
                    gfx::set_color(gm, c1);
                    gfx::draw_line(gm, xx + 0x9b, yy + 0x89, xx + 0x98, yy + 0x80);
                    gfx::draw_line(gm, xx + 0x9c, yy + 0x89, xx + 0x99, yy + 0x80);
                    yy += 9;
                    xx += 3;
                    k += 1;
                    if !(k < 5) { break; }
                }
                let slot = gm.g8(buf(e, 2)) as u16;
                let (y, x) = (gm.g8(mon_y(slot)) as i16, gm.g8(mon_x(slot)) as i16);
                gfx::draw_tile(gm, y.wrapping_add(7), x, 0);
                util::play_random_tones(gm, 4000, 0x4b0, 1);
                e = e.wrapping_add(2);
                wait_frames = Some(3);
            }
            0xc => {
                let mut p: u16 = 0;
                loop {
                    gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(p), 0);
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(p), 0);
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(p).wrapping_add(1), 0);
                    p = p.wrapping_add(8);
                    if !(g::MONSTER_ANIM_STATE.wrapping_add(p) < 0x5d60) { break; }
                }
            }
            0xe => {
                loop_start = e.wrapping_add(1);
                loop_count = gm.g8(buf(e, 1)) as u16;
                e = loop_start;
            }
            0xf => {
                loop_count = loop_count.wrapping_sub(1);
                if loop_count != 0 {
                    e = loop_start;
                }
            }
            _ => {}
        }
        if let Some(n) = wait_frames {
            // LAB_1000_eafc
            if view_idle_tick(gm, n) != 0 { return; }
        }
        gm.pf.maybe_present();
        if input::read_key(gm) != 0 { return; }
        e = e.wrapping_add(1);
    }
}

/// orig 1000:eb78 (48 bytes). Picks a random card 0..7 that is neither already shown this round
/// (0xbd2a[i]) nor eliminated (0xbd32[i]), marks it shown and returns it.
pub fn pick_unused_card(gm: &mut Game) -> i16 {
    let mut i: i16;
    loop {
        loop {
            i = util::rand_range(gm, 7, 0);
            if gm.gi8(0xbd2au16.wrapping_add(i as u16)) == 0 { break; }
        }
        if gm.gi8(0xbd32u16.wrapping_add(i as u16)) == 0 { break; }
    }
    gm.set_g8(0xbd2au16.wrapping_add(i as u16), 1);
    i
}

/// orig 1000:eba8 (321 bytes). One gypsy question: page 1 is cleared, CREATE.16 image 1 is
/// drawn at x 0x10 and x 200 (the two card frames), text cursor (0x5156, 0x5158) = (0, 0x98);
/// two unused cards a and b are picked; the lower-numbered one is drawn on the left (image
/// card+2 at (0x51fc[card], 0x5204[card])), the other on the right (+0xb8); the question text at
/// QUESTION.DAT offset 0x517c[a*8 + b] (2000 bytes into g_ool_buffer) is drawn with
/// `draw_proportional_text`; page 1 is copied to page 0 and A/B is awaited (the letter of the
/// first-picked card is 'A' when it is on the right... as the original computes it: 'B' when
/// b < a). The chosen card's int/dex/str bonuses (0x5164/0x516c/0x5174) are added to
/// 0xbd3c/0xbd3d/0xbd3e and the rejected card is eliminated (0xbd32). Ghidra order:
/// (create_seg, font_seg).
pub fn ask_gypsy_question(gm: &mut Game, param_1: u16, param_2: u16) {
    let (create_seg, font_seg) = (param_1, param_2);
    gfx::set_draw_page(gm, 1);
    text::put_char(gm, 0xff);
    gfx::draw_image(gm, 0, 0, 0x10, 1, create_seg);
    gfx::draw_image(gm, 0, 0, 200, 1, create_seg);
    gm.set_g16(0x5156, 0);
    gm.set_g16(0x5158, 0x98);
    let mut a = pick_unused_card(gm);
    let b = pick_unused_card(gm);
    let (letter, right, left): (i16, i16, i16);
    if b < a {
        letter = b'B' as i16;
        right = a;
        left = b;
    } else {
        letter = b'A' as i16;
        right = b;
        left = a;
    }
    let (ly, lx) = (gm.g8(g::CREATE_IMAGE_Y.wrapping_add(left as u16)) as i16, gm.g8(g::CREATE_IMAGE_X.wrapping_add(left as u16)) as i16);
    gfx::draw_image(gm, 0, ly, lx, left.wrapping_add(2), create_seg);
    let (ry, rx) = (gm.g8(g::CREATE_IMAGE_Y.wrapping_add(right as u16)) as i16, gm.g8(g::CREATE_IMAGE_X.wrapping_add(right as u16)) as i16);
    gfx::draw_image(gm, 0, ry, rx.wrapping_add(0xb8), right.wrapping_add(2), create_seg);
    let off = gm.g16(0x517cu16.wrapping_add((b as u16).wrapping_mul(2)).wrapping_add((a as u16).wrapping_mul(0x10))); // QUESTION.DAT offsets
    files::load_file_region(gm, off, 2000, g::OOL_BUFFER, 0xa052); // "QUESTION.DAT"
    draw_proportional_text(gm, g::OOL_BUFFER as i16, font_seg);
    gfx::copy_page(gm, 0, 1);
    let mut c: i16;
    loop {
        let k = input::read_key(gm) as i16;
        c = util::to_upper(k);
        if c == b'A' as i16 { break; }
        if c == b'B' as i16 { break; }
        gm.pf.maybe_present();
    }
    let mut rejected = b;
    if c != letter {
        rejected = a;
        a = b;
    }
    let v = gm.g8(0xbd3c).wrapping_add(gm.g8(0x5164u16.wrapping_add(a as u16))); // int bonus
    gm.set_g8(0xbd3c, v);
    let v = gm.g8(0xbd3d).wrapping_add(gm.g8(0x516cu16.wrapping_add(a as u16))); // dex bonus
    gm.set_g8(0xbd3d, v);
    let v = gm.g8(0xbd3e).wrapping_add(gm.g8(0x5174u16.wrapping_add(a as u16))); // str bonus
    gm.set_g8(0xbd3e, v);
    gm.set_g8(0xbd32u16.wrapping_add(rejected as u16), 1);
}

/// orig 1000:ecea (839 bytes). Vector 1778:054e: "Create New Character". Frees the registered
/// picture unless g_location == '@', forces disk 3's drive letter to be asked again (0xff),
/// loads PROPORT.PCS and CREATE.16, INIT.GAM over the save area 0x55a6, draws the text panel,
/// asks the name (read_line 8 into g_party[0]; empty = abort) and M/F (gender 0xb/0xc at
/// 0x55b1). Then the gypsy: layout block = full width (0..0x140, band 1 for y 0x59..200),
/// QUESTION.DAT intro text at offset 0 with CREATE.16 image 0 at y 0x60, key wait; the
/// accumulators start from INIT.GAM's int/dex/str; four questions, the shown-flags cleared, two
/// more, cleared, one last (7 in all); the closing text (QUESTION.DAT 0x322, image 10 at
/// (0xa8,100), space width 4 then 5), key wait; buffers freed, page 1 cleared, text mode 0;
/// int = MP = 0xbd3c, dex = 0xbd3d, str = max(0xbd3e, 20); INIT.OOL -> 0xb31e, the first 0x100
/// bytes of g_ool_buffer zeroed and SAVED.OOL (0x200) / SAVED.GAM (0x1060) written on disk 3.
/// Always ends with select_disk(0) and g_location = '@'.
pub fn create_new_character(gm: &mut Game) {
    if gm.g8(g::LOCATION) != b'@' {
        gfx::free_picture(gm);
    }
    gm.set_g8(g::DISK3_DRIVE_LETTER, 0xff);
    let mut font_seg: u16;
    loop {
        let nm = gm.g16(g::RAW_FILE_NAMES); // "PROPORT.PCS"
        font_seg = gfx::load_file_to_mem(gm, nm);
        if font_seg != 0 { break; }
    }
    let mut create_seg: i16;
    loop {
        let nm = gm.g16(0x260a); // "CREATE.16"
        create_seg = gfx::load_image_file(gm, nm);
        if create_seg != 0 { break; }
    }
    files::load_file_region(gm, 0, 0x1060, 0x55a6, 0xa060); // "INIT.GAM" -> save area
    let c2 = gm.gi16(g::UI_COLOR2);
    gfx::set_color(gm, c2);
    gfx::fill_rect(gm, 0x7e, 200, 0x78, 0x78);
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    gfx::draw_line(gm, 0x7f, 200, 0x7f, 0x78);
    gfx::set_color(gm, 0);
    gfx::fill_rect(gm, 0xbf, 0x137, 0x80, 8);
    text::set_cursor_pos(gm, 0x11, 3);
    text::print_string(gm, 0xa06a); // "By what name shalt thou be known?"
    text::set_cursor_pos(gm, 0x13, 0xe);
    text::put_char(gm, 0x3a);
    input::read_line(gm, 8, g::PARTY);
    if gm.g8(g::PARTY) != 0 {
        text::set_cursor_pos(gm, 0x15, 8);
        text::print_string(gm, 0xa08c); // "Art thou Male or Female? "
        let mut c: i16;
        loop {
            let k = input::wait_key(gm, 0);
            c = util::to_upper(k);
            if c == b'M' as i16 { break; }
            if c == b'F' as i16 { break; }
        }
        text::put_char(gm, c as u8);
        if c == b'M' as i16 {
            gm.set_g8(0x55b1, 0xb); // g_party[0].gender
        } else {
            gm.set_g8(0x55b1, 0xc);
        }
        gm.set_g16(0x5156, 0);
        gm.set_g16(0x5158, 9);
        gm.set_g16(0x5146, 0);
        gm.set_g16(0x514c, 0x140);
        gm.set_g16(0x5148, 0xaf);
        gm.set_g16(0x514e, 0x140);
        gm.set_g16(0x5150, 0x59);
        gm.set_g16(0x5152, 200);
        files::load_file_region(gm, 0, 2000, g::OOL_BUFFER, 0xa0a6); // "QUESTION.DAT"
        gfx::set_draw_page(gm, 1);
        gm.set_g8(g::DISPLAY_MODE, 2);
        text::put_char(gm, 0xff);
        gfx::draw_image(gm, 0, 0x60, 0, 0, create_seg as u16);
        draw_proportional_text(gm, g::OOL_BUFFER as i16, font_seg);
        gfx::copy_page(gm, 0, 1);
        gm.set_g16(0x5150, 200);
        loop {
            if input::read_key(gm) != 0 { break; }
            gm.pf.maybe_present();
        }
        let v = gm.g8(0x55b6); gm.set_g8(0xbd3c, v); // intelligence
        let v = gm.g8(g::PARTY_DEX); gm.set_g8(0xbd3d, v);
        let v = gm.g8(0x55b4); gm.set_g8(0xbd3e, v); // strength
        let mut i: u8 = 0;
        loop {
            gm.set_g8(0xbd32u16.wrapping_add(i as u16), 0);
            gm.set_g8(0xbd2au16.wrapping_add(i as u16), 0);
            i = i.wrapping_add(1);
            if !(i < 8) { break; }
        }
        for _ in 0..4 { ask_gypsy_question(gm, create_seg as u16, font_seg); }
        gm.m.fill(gm.ds(0xbd2a), 8, 0);
        for _ in 0..2 { ask_gypsy_question(gm, create_seg as u16, font_seg); }
        gm.m.fill(gm.ds(0xbd2a), 8, 0);
        ask_gypsy_question(gm, create_seg as u16, font_seg);
        gm.set_g16(0x5158, 0);
        gm.set_g16(0x5156, 0);
        gm.set_g16(0x5150, 0x5a);
        gm.set_g16(0x5148, 0);
        gm.set_g16(0x514e, 0xa6);
        gm.set_g16(g::FONT_SPACE_WIDTH, 4);
        files::load_file_region(gm, 0x322, 2000, g::OOL_BUFFER, 0xa0b4); // "QUESTION.DAT"
        gfx::set_draw_page(gm, 1);
        text::put_char(gm, 0xff);
        gfx::draw_image(gm, 0, 100, 0xa8, 10, create_seg as u16);
        draw_proportional_text(gm, g::OOL_BUFFER as i16, font_seg);
        gm.set_g16(g::FONT_SPACE_WIDTH, 5);
        gfx::copy_page(gm, 0, 1);
        loop {
            if input::read_key(gm) != 0 { break; }
            gm.pf.maybe_present();
        }
        gfx::free_far_block(gm, create_seg as u16);
        gfx::free_far_block2(gm, font_seg);
        gfx::set_draw_page(gm, 1);
        text::put_char(gm, 0xff);
        gfx::set_draw_page(gm, 0);
        gm.set_g8(g::DISPLAY_MODE, 0);
        let v = gm.g8(0xbd3c);
        gm.set_g8(0x55b6, v); // intelligence
        gm.set_g8(0x55b7, v); // current MP
        let v = gm.g8(0xbd3d);
        gm.set_g8(g::PARTY_DEX, v);
        let s = gm.g8(0xbd3e);
        let s = if s < 0x14 { 0x14 } else { s }; // SUB/SBB/NOT/AND/ADD: unsigned max(str, 20)
        gm.set_g8(0x55b4, s);
        files::load_file_region(gm, 0, 0x100, g::OOL_BUFFER_UNDER, 0xa0c2); // "INIT.OOL"
        text::put_char(gm, 0xff);
        text::set_cursor_pos(gm, 10, 0);
        files::select_disk(gm, 3);
        gm.m.fill(gm.ds(g::OOL_BUFFER), 0x100, 0);
        files::save_file_region(gm, 0x200, g::OOL_BUFFER, 0xa0cc); // "SAVED.OOL"
        files::save_file_region(gm, 0x1060, 0x55a6, 0xa0d6); // "SAVED.GAM"
    }
    files::select_disk(gm, 0);
    gm.set_g8(g::LOCATION, 0x40);
}

/// orig 1000:f032 (41 bytes). Saves the 0x3000-byte LZW scratch segment (g_scratch_seg:0) into
/// the code segment at offset g_unk_520c, obtained on first use from the FLAMES overlay entry
/// (vector 1778:057e = 0xa299, the free level-2 area). Used around the TILES.16 registration in
/// `run_the_view` so the decompression dictionary survives it.
pub fn save_scratch_to_overlay_area(gm: &mut Game) {
    if gm.g16(g::UNK_520C) == 0 {
        let off = ovl::flames::flames_entry(gm);
        gm.set_g16(g::UNK_520C, off);
    }
    let dst = 0x10000u32 + gm.g16(g::UNK_520C) as u32; // CS:g_unk_520c
    let src = (gm.g16(g::SCRATCH_SEG) as u32) << 4;
    gm.m.copy(dst, src, 0x3000);
}

/// orig 1000:f05b (28 bytes). Copies the 0x3000 bytes saved by `save_scratch_to_overlay_area`
/// from CS:g_unk_520c back into the scratch segment (g_scratch_seg:0).
pub fn restore_scratch_from_overlay_area(gm: &mut Game) {
    let src = 0x10000u32 + gm.g16(g::UNK_520C) as u32; // CS:g_unk_520c
    let dst = (gm.g16(g::SCRATCH_SEG) as u32) << 4;
    gm.m.copy(dst, src, 0x3000);
}
