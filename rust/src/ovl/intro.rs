//! INTRO.OVL (translated)
//!
//! Start-up, title sequence, main menu (Journey / Create / Transfer / Introduction /
//! Acknowledgements / Return to the View), the Ultima V story slideshow and the Ultima IV
//! character transfer. `run_intro` (1000:8b46) is the only vector target called by ROOT main;
//! `draw_bottom_banner` (85fe) and `title_fire_tick` (a250) are also reached from FONT through
//! vectors 1778:0542 / 1778:0572.
//!
//! RAM used (DATA.OVL bytes used as variables): 0xbb18 path cursor of the BRITISH.PTH trace,
//! 0xbb1a segment of ULTIMA.16, 0x5146..0x5158 proportional-text layout block consumed by
//! FONT_e1e0, 0x3304 "transferred character is an Avatar" flag, 0xbc88.. first Ultima IV
//! party.sav record, 0xbb1c.. party.sav bytes 0x140..0x1f5 (gold/karma block).
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// orig 1000:81d0 (63 bytes). Draws the four 288x49 fire-frame bands of ULTIMA.16 (images 1..4)
/// on page 1 at x=16, y=0/50/100/150 after clearing the page; driver entry 35 later blits them
/// into the title (rows 65..113 of page 0).
pub fn draw_title_fire_frames(gm: &mut Game) {
    gfx::set_draw_page(gm, 1);
    text::put_char(gm, 0xff);
    let mut i: i16 = 0;
    let mut y: i16 = 0;
    loop {
        let seg = gm.g16(0xbb1a); // DAT_1f64_bb1a: g_title_pic_seg (ULTIMA.16 segment)
        gfx::draw_image(gm, 0, y, 0x10, i + 1, seg);
        y += 0x32;
        i += 1;
        if i >= 4 { break; }
    }
}

/// orig 1000:8210 (252 bytes). Traces one segment of the BRITISH.PTH route (loaded over the
/// save-game area at 0x55a6) as a dotted trail on the Britannia map: starting at pixel (x,y) and
/// path byte g_intro_path_pos, each byte gives dy (bits 0-2, sign bit 3) and dx (bits 4-6, sign
/// bit 7); a magnitude > 2 means "move without drawing" (pen up for one step). Stops at a 0 byte
/// (segment end) and returns the next path index (stored in 0xbb18); a key press aborts and
/// returns 0. g_cpu_speed is forced to 0x113 while tracing; every 32 steps waits one tick.
/// Ghidra order: (y, x).
pub fn trace_path_segment(gm: &mut Game, param_1: i16, param_2: i16) -> i16 {
    let mut y = param_1;
    let mut x = param_2;
    let mut pen: i16 = gm.gi16(g::UI_COLOR1);
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    let saved_speed = gm.g16(g::CPU_SPEED);
    gm.set_g16(g::CPU_SPEED, 0x113);
    let mut i: u16 = gm.g16(g::INTRO_FLAG_BB18); // g_intro_path_pos (u16)
    while (i as i16) < 0x1060 {
        if pen != 0 { gfx::plot_pixel(gm, y, x); }
        pen = gm.gi16(g::UI_COLOR1);
        let b = gm.g8(0x55a6u16.wrapping_add(i));
        let mut dy: u16 = (b & 7) as u16;
        if 2 < dy { pen = 0; }
        if b & 8 != 0 { dy = dy.wrapping_neg(); }
        let mut dx: u16 = ((b & 0x70) >> 4) as u16;
        if 2 < dx { pen = 0; }
        if b & 0x80 != 0 { dx = dx.wrapping_neg(); }
        x = x.wrapping_add(dx as i16);
        y = y.wrapping_add(dy as i16);
        let k = input::read_key(gm);
        if k != 0 {
            gm.set_g16(g::CPU_SPEED, saved_speed);
            return 0;
        }
        if i & 0x1f == 0 { util::wait_ticks(gm, 1); }
        if gm.gi8(0x55a6u16.wrapping_add(i)) == 0 { break; }
        i = i.wrapping_add(1);
    }
    gm.set_g16(g::INTRO_FLAG_BB18, i.wrapping_add(1));
    gm.set_g16(g::CPU_SPEED, saved_speed);
    i.wrapping_add(1) as i16
}

/// orig 1000:830e (748 bytes). "Ultima V Introduction": the 21-screen story slideshow. Loads
/// PROPORT.PCS (proportional font), TEXT.16 and STORYn.16; for each screen draws the picture
/// (g_story_image_index at g_story_pic_x/y, plus an optional second image for hook > 3),
/// fills the FONT_e1e0 layout block 0x5146.. from the g_story_* tables, loads the screen's
/// paragraph (2000 bytes at g_story_text_offsets[n] of STORY.DAT into g_ool_buffer) or, for
/// hook 3 (the blue door), prints the two fixed strings, waits for a key (not on screen 0),
/// copies page 1 to page 0; hook 2 adds a dissolve of image 2 at (40,86). Hook 1 on screens
/// 0/7/14 draws TEXT.16 headings first. Ends with a key wait, g_display_mode = 0, select_disk(0).
pub fn play_story_sequence(gm: &mut Game) {
    if gm.g8(g::LOCATION) != 0x40 { gfx::free_picture(gm); }
    gm.set_g8(g::LOCATION, 0x40);
    let mut font_seg: u16;
    loop {
        let nm = gm.g16(g::RAW_FILE_NAMES); // g_raw_file_names[0] = "PROPORT.PCS"
        font_seg = gfx::load_file_to_mem(gm, nm);
        if font_seg != 0 { break; }
    }
    ovl::font::draw_proportional_text(gm, 0x2f30u16 as i16, font_seg); // "" (empty string)
    gfx::set_draw_page(gm, 1);
    text::put_char(gm, 0xff);
    gfx::set_draw_page(gm, 0);
    text::put_char(gm, 0xff);
    text::set_cursor_pos(gm, 10, 0);
    let mut text_seg: i16;
    loop {
        let nm = gm.g16(0x260c); // g_raw_file_names[17] = "TEXT.16"
        text_seg = gfx::load_image_file(gm, nm);
        if text_seg != 0 { break; }
    }
    let mut story_seg: i16;
    loop {
        let nm = gm.g16(0x260e); // g_raw_file_names[18] = "STORY1.16"
        story_seg = gfx::load_image_file(gm, nm);
        if story_seg != 0 { break; }
    }
    let mut cur_story: u16 = 0;
    let mut screen: i16 = 0;
    loop {
        if 0x14 < screen {
            gfx::free_far_block(gm, story_seg as u16);
            gfx::free_far_block(gm, text_seg as u16);
            gfx::free_far_block2(gm, font_seg);
            input::flush_keyboard(gm);
            loop { if input::read_key(gm) != 0 || gm.pf.quit_requested { break; } }
            gfx::set_draw_page(gm, 1);
            text::put_char(gm, 0xff);
            gfx::set_draw_page(gm, 0);
            gm.set_g8(g::DISPLAY_MODE, 0);
            files::select_disk(gm, 0);
            return;
        }
        let s = screen as u16;
        if gm.g8(0x30ae + s) as u16 != cur_story { // g_story_number
            gfx::free_far_block(gm, story_seg as u16);
            cur_story = gm.g8(0x30ae + s) as u16;
            loop {
                let nm = gm.g16(0x260e + cur_story * 2); // STORY{n}.16
                story_seg = gfx::load_image_file(gm, nm);
                if story_seg != 0 { break; }
            }
        }
        gm.set_g8(g::DISPLAY_MODE, 2);
        gfx::set_draw_page(gm, 1);
        text::put_char(gm, 0xff);
        if gm.gi8(0x30f0 + s) == 1 { // g_story_hook
            let heading: Option<(i16, i16, i16)> = if screen == 0 {
                gfx::draw_image(gm, 0, 0x1e, 0xe0, 0, text_seg as u16);
                Some((1, 0xa8, 0x3a))
            } else if screen == 7 {
                gfx::draw_image(gm, 0, 0x1a, 0xe8, 0, text_seg as u16);
                Some((2, 200, 0x36))
            } else if screen == 0xe {
                gfx::draw_image(gm, 0, 0, 0xb8, 0, text_seg as u16);
                Some((3, 0xf8, 0))
            } else { None };
            if let Some((idx, x, y)) = heading {
                gfx::draw_image(gm, 0, y, x, idx, text_seg as u16);
            }
        }
        // LAB_1000_83b9
        let py = gm.g8(0x30da + s) as i16; // g_story_pic_y
        let px = gm.g8(0x30c4 + s) as i16; // g_story_pic_x
        let img = gm.g8(0x3098 + s) as i16; // g_story_image_index
        gfx::draw_image(gm, 0, py, px, img, story_seg as u16);
        if 3 < gm.g8(0x30f0 + s) {
            let idx = (gm.g8(0x30f0 + s) as i16) * 2 - 5;
            gfx::draw_image(gm, 0, py + 0x37, px, idx, story_seg as u16);
        }
        // proportional-text layout block read by FONT_e1e0
        let v = gm.g8(0x2f98 + s * 2) as u16; gm.set_g16(0x5146, v); // g_story_left_x[2n]
        let v = gm.g8(0x2f99 + s * 2) as u16; gm.set_g16(0x5148, v); // g_story_left_x[2n+1]
        let v = gm.g16(0x2fc2 + s * 4); gm.set_g16(0x514c, v);       // g_story_right_x[2n]
        let v = gm.g16(0x2fc4 + s * 4); gm.set_g16(0x514e, v);       // g_story_right_x[2n+1]
        let v = gm.g8(0x3040 + s) as u16; gm.set_g16(0x5150, v);     // g_story_para1_y
        let v = gm.g8(0x3056 + s) as u16; gm.set_g16(0x5152, v);     // g_story_para2_y
        let x0 = gm.g8(0x306c + s) as u16;                           // g_story_first_line_x
        let v = gm.g8(0x3082 + s) as u16; gm.set_g16(0x5158, v);     // g_story_text_y
        gm.set_g16(0x5156, x0);
        let txt: u16;
        if gm.gi8(0x30f0 + s) == 3 {
            gfx::draw_image(gm, 0, 0x27, 0x60, 3, story_seg as u16);
            ovl::font::draw_proportional_text(gm, 0x2f31u16 as i16, font_seg); // "Instantly, a shimmering blue door springs up!"
            gm.set_g16(0x5158, 0xb4);
            txt = 0x2f5f; // "With heart beating rapidly, you step into it."
            gm.set_g16(0x5156, x0);
        } else {
            let off = gm.g16(0x3016 + s * 2); // g_story_text_offsets
            files::load_file_region(gm, off, 2000, g::OOL_BUFFER, 0x2f8d); // "STORY.DAT"
            txt = g::OOL_BUFFER;
        }
        ovl::font::draw_proportional_text(gm, txt as i16, font_seg);
        if screen != 0 {
            input::flush_keyboard(gm);
            loop { if input::read_key(gm) != 0 || gm.pf.quit_requested { break; } }
        }
        gfx::copy_page(gm, 0, 1);
        if gm.gi8(0x30f0 + s) == 2 {
            gfx::set_draw_page(gm, 1);
            gfx::draw_image(gm, 0, 0x56, 0x28, 2, story_seg as u16);
            gfx::rect_effect(gm, 0x78, 0x4b, 0x56, 0x28);
        }
        screen += 1;
    }
}

/// orig 1000:85fe (161 bytes). Vector 1778:0542 (also used by FONT). Draws a centred caption bar
/// on the bottom text row (24): two g_ui_color2 filled blocks at y 193..199 left and right of the
/// text (x 8..(20-len/2)*8 and (20+len/2+2)*8..311) with a g_ui_color1 line at y=192 above each,
/// then the text at column 20-len/2 between the bar end-caps.
pub fn draw_bottom_banner(gm: &mut Game, param_1: u16) {
    let a = gm.ds(param_1);
    let len = util::str_len(gm, a);
    let half = len / 2 + -0x12;
    let col = -half;
    let len2 = util::str_len(gm, a);
    let c2 = gm.gi16(g::UI_COLOR2);
    gfx::set_color(gm, c2);
    let x_left = half * -8;
    gfx::fill_rect(gm, 199, x_left, 0xc1, 8);
    let x_right = (len2 + col + 2) * 8;
    gfx::fill_rect(gm, 199, 0x137, 0xc1, x_right);
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    gfx::draw_line(gm, 0xc0, x_left, 0xc0, 8);
    gfx::draw_line(gm, 0xc0, 0x137, 0xc0, x_right);
    text::set_cursor_pos(gm, 0x18, col);
    rest::draw_bar_left_cap(gm);
    text::print_string(gm, param_1);
    rest::draw_bar_right_cap(gm);
}

/// orig 1000:86a0 (207 bytes). Draws the main-menu box: a text-glyph frame (0x7b corner, 0x7f
/// edge, 0x7c/0x7d/0x7e corners) in g_ui_color2 from row 15 spanning 40 columns and 8 inner rows,
/// then a g_ui_color1 rectangle (7,127)-(312,192) around it.
pub fn draw_menu_frame(gm: &mut Game) {
    let c2 = gm.g16(g::UI_COLOR2);
    text::set_text_color(gm, c2 as u8);
    text::set_cursor_pos(gm, 0xf, 0);
    text::put_char(gm, 0x7b);
    let mut n: i16 = 0x26;
    loop { text::put_char(gm, 0x7f); n -= 1; if n == 0 { break; } }
    text::put_char(gm, 0x7c);
    n = 8;
    loop {
        text::put_char(gm, 0x7f);
        let row = text::get_cursor_row(gm);
        text::set_cursor_pos(gm, row, 0x27);
        text::put_char(gm, 0x7f);
        n -= 1;
        if n == 0 { break; }
    }
    text::put_char(gm, 0x7d);
    n = 0x26;
    loop { text::put_char(gm, 0x7f); n -= 1; if n == 0 { break; } }
    gm.set_g16(g::CURSOR_ADVANCE, 0);
    text::put_char(gm, 0x7e);
    gm.set_g16(g::CURSOR_ADVANCE, 1);
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    gfx::draw_line(gm, 0x7f, 0x138, 0x7f, 7);
    gfx::draw_line_to(gm, 0xc0, 0x138);
    gfx::draw_line_to(gm, 0xc0, 7);
    gfx::draw_line_to(gm, 0x7f, 7);
    let c1 = gm.g16(g::UI_COLOR1);
    text::set_text_color(gm, c1 as u8);
}

/// orig 1000:8770 (198 bytes). Shows the burning "ULTIMA V" title: loads ULTIMA.16 (segment kept
/// in 0xbb1a), clears page 0, draws image 0 on page 1; if `animate` the title is revealed with
/// rect_effect (dissolve of rows 0..100) and, unless a key was pressed, burnt in with the WD.BIT
/// mask (driver entry 35 CF=1); otherwise rows 0..100 are copied straight. Then the fire frames
/// are drawn on page 1, one fire tick is shown, g_display_mode = 0 and the menu frame is drawn.
pub fn show_title_screen(gm: &mut Game, param_1: u16) {
    gm.set_g8(g::DISPLAY_MODE, 2);
    if gm.g8(g::LOCATION) != 0x40 {
        gfx::free_picture(gm);
        gm.set_g8(g::LOCATION, 0x40);
    }
    loop {
        let nm = gm.g16(0x261e); // g_raw_file_names[26] = "ULTIMA.16"
        let seg = gfx::load_image_file(gm, nm);
        gm.set_g16(0xbb1a, seg as u16); // g_title_pic_seg
        if seg != 0 { break; }
    }
    gfx::set_draw_page(gm, 0);
    text::put_char(gm, 0xff);
    gfx::set_draw_page(gm, 1);
    let seg = gm.g16(0xbb1a);
    gfx::draw_image(gm, 0, 0, 0, 0, seg);
    let mut animate = param_1;
    if animate != 0 {
        gfx::rect_effect(gm, 100, 0x13f, 0, 0);
        let k = input::read_key(gm);
        animate = (k == 0) as u16;
    }
    if animate == 0 {
        gfx::copy_rect_between_pages(gm, 100, 0x13f, 0, 0, 0, 1);
    }
    draw_title_fire_frames(gm);
    let seg = gm.g16(0xbb1a);
    gfx::free_far_block(gm, seg);
    if animate != 0 {
        let mut wd: u16;
        loop {
            wd = gfx::load_file_to_mem(gm, 0x3105); // "WD.BIT"
            if wd != 0 { break; }
        }
        title_fire_burn(gm, wd);
        gfx::free_far_block2(gm, wd);
    }
    gfx::set_draw_page(gm, 0);
    title_fire_tick(gm);
    gm.set_g8(g::DISPLAY_MODE, 0);
    draw_menu_frame(gm);
}

/// orig 1000:8836 (70 bytes). Prints one main-menu entry at row 17+index, column `col`, as
/// " text " — in inverse video (0xfd toggles) when index == selected. Ghidra order:
/// (text, col, index, selected).
pub fn draw_menu_item(gm: &mut Game, param_1: u16, param_2: u16, param_3: i16, param_4: i16) {
    if param_3 == param_4 { text::put_char(gm, 0xfd); }
    text::set_cursor_pos(gm, param_3 + 0x11, param_2 as i16);
    text::put_char(gm, 0x20);
    text::print_string(gm, param_1);
    text::put_char(gm, 0x20);
    if param_3 == param_4 { text::put_char(gm, 0xfd); }
}

/// orig 1000:887c (114 bytes). Draws the six main-menu lines (Journey Onward / Create New
/// Character / Transfer from Ultima IV / Ultima V Introduction / Acknowledgements / Return to
/// the View) with entry `selected` highlighted.
pub fn draw_main_menu(gm: &mut Game, param_1: u16) {
    let sel = param_1 as i16;
    draw_menu_item(gm, 0x310c, 0xc, 0, sel); // "Journey Onward"
    draw_menu_item(gm, 0x311b, 9, 1, sel);   // "Create New Character"
    draw_menu_item(gm, 0x3130, 8, 2, sel);   // "Transfer from Ultima IV"
    draw_menu_item(gm, 0x3148, 9, 3, sel);   // "Ultima V Introduction"
    draw_menu_item(gm, 0x315e, 0xb, 4, sel); // "Acknowledgements"
    draw_menu_item(gm, 0x316f, 10, 5, sel);  // "Return to the View"
}

/// orig 1000:88ee (544 bytes). "Acknowledgements": loads STARTSC.16 (three 137-row strips:
/// 16-wide left curtain, 288-wide credits, 16-wide right curtain); draws the credits on page 1
/// at (16,63), stacks the curtains column by column on page 0, then slides them apart 8 px per
/// tick to reveal the credits; redraws the menu box with the ULTIMA.16 title image 1 at (16,65)
/// and the menu with entry 4 highlighted; waits for a key; slides the curtains shut, frees the
/// picture, "closes" the curtain rows upward, redraws the fire frames and flushes the keyboard.
pub fn show_acknowledgements(gm: &mut Game) {
    let mut pic: i16;
    loop {
        let nm = gm.g16(0x2620); // g_raw_file_names[27] = "STARTSC.16"
        pic = gfx::load_image_file(gm, nm);
        if pic != 0 { break; }
    }
    gfx::set_draw_page(gm, 1);
    gfx::draw_image(gm, 0, 0x3f, 0x10, 1, pic as u16);
    gfx::set_draw_page(gm, 0);
    let mut y: i16 = 199;
    loop {
        gfx::draw_image(gm, 0, y, 0x90, 0, pic as u16);
        gfx::draw_image(gm, 0, y, 0xa0, 2, pic as u16);
        y -= 1;
        if !(0x3e < y) { break; }
    }
    let mut d: i16 = 0;
    loop {
        gfx::draw_image(gm, 0, 0x3f, 0x88 - d, 0, pic as u16);
        gfx::copy_rect_between_pages(gm, 199, 0x9f - d, 0x3f, 0x98 - d, 0, 1);
        gfx::draw_image(gm, 0, 0x3f, d + 0xa8, 2, pic as u16);
        gfx::copy_rect_between_pages(gm, 199, d + 0xa7, 0x3f, d + 0xa0, 0, 1);
        util::wait_ticks(gm, 1);
        d += 8;
        if !(d < 0x90) { break; }
    }
    loop {
        let nm = gm.g16(0x261e); // "ULTIMA.16"
        let seg = gfx::load_image_file(gm, nm);
        gm.set_g16(0xbb1a, seg as u16);
        if seg != 0 { break; }
    }
    gfx::set_draw_page(gm, 1);
    text::put_char(gm, 0xff);
    let seg = gm.g16(0xbb1a);
    gfx::draw_image(gm, 0, 0x41, 0x10, 1, seg);
    gfx::free_far_block(gm, seg);
    draw_menu_frame(gm);
    draw_main_menu(gm, 4);
    loop { if input::read_key(gm) != 0 || gm.pf.quit_requested { break; } }
    gfx::set_draw_page(gm, 0);
    d = 0x88;
    loop {
        gfx::draw_image(gm, 0, 0x3f, 0x90 - d, 0, pic as u16);
        gfx::copy_rect_between_pages(gm, 199, 0x8f - d, 0x3f, 0x88 - d, 0, 1);
        gfx::draw_image(gm, 0, 0x3f, d + 0xa0, 2, pic as u16);
        gfx::copy_rect_between_pages(gm, 199, d + 0xb7, 0x3f, d + 0xb0, 0, 1);
        util::wait_ticks(gm, 1);
        d -= 8;
        if !(-1 < d) { break; }
    }
    gfx::free_far_block(gm, pic as u16);
    // NOTE: the original keeps drawing from the freed block (DOS free does not clear memory).
    y = 0x3f;
    loop {
        gfx::draw_image(gm, 0, y + 1, 0x90, 0, pic as u16);
        gfx::draw_image(gm, 0, y + 1, 0xa0, 2, pic as u16);
        gfx::copy_rect_between_pages(gm, y, 0xaf, y, 0x90, 0, 1);
        y += 1;
        if !(y < 199) { break; }
    }
    gfx::copy_rect_between_pages(gm, 199, 0xaf, 199, 0x90, 0, 1);
    draw_title_fire_frames(gm);
    gfx::set_draw_page(gm, 0);
    input::flush_keyboard(gm);
}

/// orig 1000:8b0e (56 bytes). Polls the keyboard once per tick for up to `ticks` ticks; returns
/// the key code or 0 when none arrived.
pub fn wait_key_ticks(gm: &mut Game, param_1: i16) -> i16 {
    let mut n: i16 = 0;
    loop {
        if param_1 <= n { return 0; }
        let k = input::read_key(gm) as i16;
        if k != 0 { return k; }
        if gm.pf.quit_requested { return 0x1b; }
        util::wait_ticks(gm, 1);
        n += 1;
    }
}

/// orig 1000:8b46 (1663 bytes). Vector 1778:02ae, called once by ROOT main. The whole start-up
/// and title/menu state machine: sound on; identity g_tile_anim_map; detect_video_adapter;
/// init_system; Tandy with < 368 KB falls back to CGA; load_video_driver (retry);
/// init_graphics_driver(1); CGA/Hercules: rename every "*.16" in g_raw_file_names to "*.4" in
/// place, otherwise set the EGA palette words 0x13ae..0x13ba (4,15,1,1,5,14,7);
/// driver_init_screen (failure → restore_video_mode, exit(1)); fonts IBM.CH/RUNES.CH (or the
/// .HCS Hercules fonts) into slots 0/1; window 0 = full screen. If a 'J' is already pending the
/// title is skipped (TILES.16 registered, g_location = 'A'); otherwise the title sequence:
/// TITLE.BIT + BRITISH.BIT, BRITISH.PTH over 0x55a6, driver entry 37 zoom (draw_stacked_pictures),
/// TITLE.BIT image 7 at (108,140), 18 ticks + key, image 8 at (152,0), the four-segment path trace
/// on the Britannia map, BRITISH.BIT at (24,66) and image 9 at (104,160); each stage is skipped
/// once a key was seen; then show_title_screen(not aborted) and FONT_e684 (the View).
/// Then the menu loop: windows 0/1/2, "Copyright 1988 Lord British" banner, the JCTUAR menu with
/// arrow (1/3 up, 2/4 down) and Enter/Space selection, 200 idle polls (fire ticks) → 'R'.
/// Actions: A = acknowledgements; C = FONT_ecea (create); T = transfer_from_ultima4; U = story;
/// R = FONT_e684 (the View); J = draw_game_frame, select_disk(3), SAVED.GAM (0x1060 bytes to
/// 0x55a6); with a party: SAVED.OOL → g_ool_buffer, select_disk(1), BRIT.OOL/UNDER.OOL written
/// from 0xb21e/0xb31e (plus UNDER.OOL on disk 5 when outdoors in the Underworld), wind redraw,
/// g_cell_h = 8 and RETURN to main; without: "No active game" message and back to the menu.
pub fn run_intro(gm: &mut Game) {
    // INTRO.OVL has no code-space data tables but load it anyway so the image is resident.
    let dir = gm.dir.clone();
    let _ = crate::loader::load_overlay(&mut gm.m, &dir, "INTRO");
    gm.set_g8(g::SOUND_ENABLED, 1);
    let mut i: i16 = 0;
    loop {
        gm.set_g8(g::TILE_ANIM_MAP.wrapping_add(i as u16), i as u8);
        i += 1;
        if !(i < 0x100) { break; }
    }
    gfx::detect_video_adapter(gm);
    util::init_system(gm);
    let saved_speed = gm.g16(g::CPU_SPEED);
    if gm.gi16(g::ADAPTER_TYPE) == 2 && gm.g16(g::MEM_KB) < 0x170 {
        gm.set_g16(g::FLAG_TANDY, 0);
        gm.set_gi16(g::FORCE_CGA, 1);
        gm.set_gi16(g::ADAPTER_TYPE, 0);
    }
    loop { if gfx::load_video_driver(gm) != 0 { break; } }
    gfx::init_graphics_driver(gm, 1);
    if gm.gi16(g::ADAPTER_TYPE) == 0 || gm.gi16(g::ADAPTER_TYPE) == 3 {
        // CGA / Hercules: rename "*.16" to "*.4" in the file-name table (in place).
        let mut n: u16 = 0;
        loop {
            let p = gm.g16(g::RAW_FILE_NAMES + n * 2);
            if gm.g8(p) == 0 { break; }
            let mut q = p;
            loop {
                let c = gm.g8(q);
                q = q.wrapping_add(1);
                if c == b'.' { break; }
            }
            // q = char after '.', q-1 = '.'
            n += 1;
            if gm.g8(q) == b'1' {
                gm.set_g8(q, b'4');
                gm.set_g8(q.wrapping_sub(1).wrapping_add(2), 0);
            }
        }
    } else {
        gm.set_g16(g::UI_COLOR2, 1);
        gm.set_g16(g::UI_COLOR0, 4);
        gm.set_g16(g::UI_COLOR1, 0xf);
        gm.set_g16(g::UI_COLOR4, 5);
        gm.set_g16(g::UI_COLOR3, 2);
        gm.set_gi16(g::UI_COLOR_SUN, 0xe);
        gm.set_gi16(g::UI_COLOR_MOON, 7);
    }
    // driver_init_screen returns 0 only when the driver could not allocate its scratch page.
    let ok: i16 = { gfx::driver_init_screen(gm); 1 };
    if ok == 0 {
        gfx::restore_video_mode(gm);
        std::process::exit(1);
    }
    if gm.gi16(g::ADAPTER_TYPE) == 3 {
        loop { if text::load_font(gm, 0, 0x3182) != 0 { break; } } // "ibm.hcs"
        loop { if text::load_font(gm, 1, 0x318a) != 0 { break; } } // "runes.hcs"
    } else {
        loop { if text::load_font(gm, 0, 0x3194) != 0 { break; } } // "ibm.ch"
        loop { if text::load_font(gm, 1, 0x319b) != 0 { break; } } // "runes.ch"
    }
    text::select_font(gm, 0);
    text::define_text_window(gm, 0x18, 0x27, 0, 0, 0);
    text::select_text_window(gm, 0);
    let k = input::read_key(gm);
    let mut state: u16; // local_a: 0x4a = go straight to Journey Onward; else 1/0
    if util::to_upper(k as i16) == 0x4a {
        text::put_char(gm, 0xff);
        text::put_char(gm, 0xfc);
        text::set_cursor_pos(gm, 0xb, 0);
        text::print_string(gm, 0x31a4); // "Journey Onward\n\n"
        text::put_char(gm, 0xfb);
        loop {
            let nm = gm.g16(0x25f0); // "TILES.16"
            if gfx::load_picture_file(gm, nm) != 0 { break; }
        }
        gm.set_g8(g::LOCATION, 0x41);
        state = 0x4a;
    } else {
        let mut title_seg: u16;
        loop {
            let nm = gm.g16(0x25ee); // "TITLE.BIT"
            title_seg = gfx::load_file_to_mem(gm, nm);
            if title_seg != 0 { break; }
        }
        let mut map_seg: u16;
        loop {
            let nm = gm.g16(0x25ec); // "BRITISH.BIT"
            map_seg = gfx::load_file_to_mem(gm, nm);
            if map_seg != 0 { break; }
        }
        files::load_file_region(gm, 0, 0x1060, 0x55a6, 0x31b5); // "BRITISH.PTH" over the save-game area
        let aborted = gfx::draw_stacked_pictures(gm, title_seg);
        state = (aborted == 0) as u16;
        gfx::set_draw_page(gm, 1);
        gfx::fill_rect(gm, 199, 0x13f, 0x8c, 0);
        gfx::draw_image_masked(gm, 0x8c, 0x6c, 7, title_seg);
        gfx::copy_rect_between_pages(gm, 199, 0x13f, 0x8c, 0, 0, 1);
        if state != 0 {
            util::wait_ticks(gm, 0x12);
            let k = wait_key_ticks(gm, 0x14);
            state = (k == 0) as u16;
        }
        if state != 0 {
            text::put_char(gm, 0xff);
            gfx::draw_image_masked(gm, 0, 0x98, 8, title_seg);
            gfx::copy_page(gm, 0, 1);
            let k = wait_key_ticks(gm, 0x14);
            state = (k == 0) as u16;
            if state != 0 {
                gfx::set_draw_page(gm, 0);
                gm.set_g16(g::INTRO_FLAG_BB18, 0);
                state = 0;
                let mut r = trace_path_segment(gm, 0x44, 0x2c);
                if r != 0 {
                    r = trace_path_segment(gm, 0x5e, 0x40);
                    if r != 0 {
                        r = trace_path_segment(gm, 0x4e, 0x8f);
                        if r != 0 {
                            state = trace_path_segment(gm, 0x69, 0xa7) as u16;
                        }
                    }
                }
                gfx::set_draw_page(gm, 1);
            }
            gfx::draw_image_masked(gm, 0x42, 0x18, 0, map_seg);
            gfx::copy_page(gm, 0, 1);
            gfx::draw_image_masked(gm, 0xa0, 0x68, 9, title_seg);
            gfx::copy_page(gm, 0, 1);
            if state != 0 {
                let k = wait_key_ticks(gm, 0x14);
                state = (k == 0) as u16;
            }
        }
        gfx::free_far_block2(gm, map_seg);
        gfx::free_far_block2(gm, title_seg);
        gm.set_g8(g::LOCATION, 0x40);
        text::put_char(gm, 0xff);
        gfx::set_draw_page(gm, 0);
        show_title_screen(gm, state);
        if state != 0 {
            ovl::font::run_the_view(gm); // "the View"
        }
        state = 1;
    }
    let seed = util::get_time_hash(gm);
    util::set_random_seed(gm, seed);
    // LAB_1000_8e90: the menu loop
    loop {
        text::define_text_window(gm, 0x17, 0x26, 0x10, 1, 0);
        text::select_text_window(gm, 0);
        text::put_char(gm, 0xff);
        text::define_text_window(gm, 0x18, 0x27, 0, 0, 0);
        text::define_text_window(gm, 9, 0x27, 1, 0x18, 1);
        text::define_text_window(gm, 0x17, 0x27, 0xb, 0x18, 2);
        let mut key: u8; // local_10
        if state == 0x4a {
            key = 0x4a;
        } else {
            let mut sel: i16 = 0;
            draw_bottom_banner(gm, 0x31c1); // "Copyright 1988 Lord British"
            draw_main_menu(gm, 0);
            loop {
                text::set_cursor_pos(gm, 0xf, 0xf);
                rest::draw_bar_left_cap(gm);
                text::print_string(gm, 0x31dd); // "Select: "
                rest::draw_bar_right_cap(gm);
                text::set_cursor_pos(gm, 0xf, 0x17);
                gm.set_g16(g::CPU_SPEED, 0x113);
                let mut polls: i16 = 0;
                loop {
                    let k = input::poll_key_with_cursor(gm);
                    key = util::to_upper(k) as u8;
                    if key == 0 { title_fire_tick(gm); }
                    polls += 1;
                    if gm.pf.quit_requested { key = 0x52; }
                    if !(polls < 200 && key == 0) { break; }
                }
                gm.set_g16(g::CPU_SPEED, saved_speed);
                let mut new_sel: i16 = -1; // -1 = no menu redraw
                if key == 0xd {
                    key = gm.g8(0x3270 + sel as u16); // "JCTUAR"[sel]
                } else if key < 0xe {
                    if key == 0 {
                        key = 0x52; // idle timeout → "Return to the View"
                    } else if key == 1 || key == 3 {
                        sel -= 1;
                        new_sel = sel;
                        if sel < 0 { sel = 5; new_sel = sel; }
                    } else if key == 2 || key == 4 {
                        sel += 1;
                        new_sel = sel;
                        if 5 < sel { sel = 0; new_sel = sel; }
                    } else {
                        key = 0;
                    }
                } else if key == 0x43 {
                    new_sel = 1;
                } else if key < 0x44 {
                    if key == 0x20 {
                        key = gm.g8(0x3270 + sel as u16);
                    } else if key == 0x41 {
                        new_sel = 4;
                    } else {
                        key = 0;
                    }
                } else if key == 0x4a {
                    new_sel = 0;
                } else if key == 0x52 {
                    new_sel = 5;
                } else if key == 0x54 {
                    new_sel = 2;
                } else if key == 0x55 {
                    new_sel = 3;
                } else {
                    key = 0;
                }
                if new_sel >= 0 { draw_main_menu(gm, new_sel as u16); }
                // LAB_1000_8f8e
                if !(key < 0x21) { break; }
            }
        }
        if key == 0x41 {
            if gm.g8(g::LOCATION) != 0x40 {
                gfx::free_picture(gm);
                gm.set_g8(g::LOCATION, 0x40);
            }
            show_acknowledgements(gm);
            continue;
        }
        if key == 0x43 || key == 0x54 {
            if key == 0x43 {
                ovl::font::create_new_character(gm); // vector 1778:054e: "Create New Character"
            } else {
                text::put_char(gm, 0xff);
                transfer_from_ultima4(gm);
            }
            // LAB_1000_916b
            text::define_text_window(gm, 0x18, 0x27, 0, 0, 0);
            text::select_text_window(gm, 0);
            text::put_char(gm, 0xff);
            gfx::set_draw_page(gm, 1);
            text::put_char(gm, 0xff);
            show_title_screen(gm, 0);
            draw_title_fire_frames(gm);
            gfx::set_draw_page(gm, 0);
        } else if key == 0x4a {
            rest::draw_game_frame(gm);
            rest::set_wind_and_draw(gm, 0);
            text::select_text_window(gm, 2);
            text::set_cursor_pos(gm, 0xc, 0);
            if gm.g8(g::LOCATION) == 0x40 {
                loop {
                    let nm = gm.g16(0x25f0); // "TILES.16"
                    if gfx::load_picture_file(gm, nm) != 0 { break; }
                }
                gm.set_g8(g::LOCATION, 0x41);
            }
            files::select_disk(gm, 3);
            files::load_file_region(gm, 0, 0x1060, 0x55a6, 0x31e6); // "SAVED.GAM"
            if gm.g8(g::PARTY) != 0 {
                files::load_file_region(gm, 0, 0x200, g::OOL_BUFFER, 0x323f); // "SAVED.OOL"
                files::select_disk(gm, 1);
                files::save_file_region(gm, 0x100, g::OOL_BUFFER, 0x3249); // "BRIT.OOL"
                files::save_file_region(gm, 0x100, 0xb31e, 0x3252);        // "UNDER.OOL"
                if gm.g8(g::LOCATION) == 0 && gm.g8(g::PARTY_Z) != 0 {
                    files::select_disk(gm, 5);
                    loop { if files::file_exists(gm, 0x325c) != 0 { break; } } // "UNDER.DAT"
                    files::save_file_region(gm, 0x100, 0xb31e, 0x3266); // "UNDER.OOL"
                }
                rest::set_wind_and_draw(gm, -1);
                gm.set_gi16(g::CELL_H, 8);
                return;
            }
            text::print_string(gm, 0x31f0); // "\n\nNo active game. "
            text::print_string(gm, 0x3203); // "Please create a character "
            text::print_string(gm, 0x321e); // "or transfer one from Ultima IV. "
            input::wait_key(gm, 0);
            gm.set_g8(g::DISK3_DRIVE_LETTER, 0xff);
            files::select_disk(gm, 0);
            text::select_text_window(gm, 0);
            text::put_char(gm, 0xff);
            gfx::set_draw_page(gm, 1);
            text::put_char(gm, 0xff);
            gfx::set_draw_page(gm, 0);
            show_title_screen(gm, 0);
            state = 0;
        } else if key == 0x52 {
            ovl::font::run_the_view(gm); // "Return to the View"
        } else if key == 0x55 {
            play_story_sequence(gm);
            show_title_screen(gm, 0);
        }
        if gm.pf.quit_requested { std::process::exit(0); }
    }
}

/// orig 1000:91d6 (584 bytes). Reads the first Ultima IV character record (party.sav bytes
/// 8..0x2f → 0xbc88: +0 hp, +2 max hp, +4 exp, +6 str, +8 dex, +0xa int, +0xc mp (all u16),
/// +0x14 name[16], +0x24 sex (0x0b male / 0x0c female), +0x25 class 0..7) from disk 4 and
/// validates it (str/dex/int ≤ 70, hp/max hp/exp ≤ 9999, class ≤ 7, no control characters in
/// the name). Invalid → error message, wait_key, returns 1. Otherwise builds g_party[0]: name
/// (≤ 8 chars), gender 0x0b/0x0c, class letter from the U4 class index (M B F D T P R S),
/// status 'G', str/dex/int/mp, hp/max hp, exp, level = max hp / 100; then loads party.sav
/// 0x140..0x1f5 to 0xbb1c and sets the Avatar flag 0x3304 when the eight words at 0xbb22..
/// 0xbb30 (karma, 0 = elevated) are all zero. Returns 0.
pub fn load_ultima4_character(gm: &mut Game) -> u16 {
    let mut bad = false;
    gm.set_g8(g::DISPLAY_MODE, 1);
    gm.set_g8(g::CUR_DISK, 4);
    let d = files::get_current_drive_letter(gm);
    gm.set_g8(0xa9cc, d); // g_disk_drive_letters[4]
    files::load_file_region(gm, 8, 0x28, 0xbc88, 0x3278); // "party.sav"
    if 0x46 < gm.g16(0xbc8e) || 0x46 < gm.g16(0xbc90) || 0x46 < gm.g16(0xbc92)
        || 0x270f < gm.g16(0xbc8c) || 0x270f < gm.g16(0xbc88) || 0x270f < gm.g16(0xbc8a)
        || 7 < gm.g8(0xbcad)
    {
        bad = true;
    }
    let mut i: u16 = 0;
    loop {
        let c = gm.g8(0xbc9c + i);
        if c != 0 && c < 0x20 { bad = true; break; }
        i += 1;
        if !(i < 8) { break; }
    }
    if bad {
        text::put_char(gm, 0xff);
        text::put_char(gm, 0xfc);
        text::set_cursor_pos(gm, 5, 0);
        text::print_string(gm, 0x3282); // "Error:  Your Ultima IV game\n\ncontains bad data.\n\n"
        text::print_string(gm, 0x32b4); // "Unable to continue transfer.\n\n\n"
        text::print_string(gm, 0x32d4); // "Press any key to return to the menu."
        text::put_char(gm, 0xfb);
        input::wait_key(gm, 0);
        return 1;
    }
    let mut i: u16 = 0;
    loop {
        let c = gm.g8(0xbc9c + i);
        if c == 0 { break; }
        gm.set_g8(g::PARTY + i, c);
        i += 1;
        if !(i < 8) { break; }
    }
    gm.set_g8(g::PARTY + i, 0);
    if gm.g8(0xbcac) == 0xb { gm.set_g8(0x55b1, 0xb); } else { gm.set_g8(0x55b1, 0xc); } // gender
    let cls: u8 = match gm.g8(0xbcad) {
        0 => 0x4d, 1 => 0x42, 2 => 0x46, 3 => 0x44, 4 => 0x54, 5 => 0x50, 6 => 0x52, 7 => 0x53,
        _ => gm.g8(g::PARTY_CLASS),
    };
    gm.set_g8(g::PARTY_CLASS, cls);
    gm.set_g8(g::PARTY_STATUS, 0x47);
    let v = gm.g16(0xbc8e); gm.set_g8(0x55b4, v as u8);         // str
    let v = gm.g16(0xbc90); gm.set_g8(g::PARTY_DEX, v as u8);   // dex
    let v = gm.g16(0xbc92); gm.set_g8(0x55b6, v as u8);         // int
    let v = gm.g16(0xbc94); gm.set_g8(0x55b7, v as u8);         // mp
    let v = gm.g16(0xbc88); gm.set_g16(g::PARTY_HP, v);
    let v = gm.g16(0xbc8a); gm.set_g16(g::PARTY_MAX_HP, v);
    let v = gm.g16(0xbc8c); gm.set_g16(0x55bc, v);              // exp
    let v = (gm.g16(0xbc8a) as u32 / 100) as u8; gm.set_g8(0x55be, v); // level
    files::load_file_region(gm, 0x140, 0xb6, 0xbb1c, 0x32f9); // "party.sav"
    if gm.g16(0xbb22) == 0 && gm.g16(0xbb24) == 0 && gm.g16(0xbb26) == 0 && gm.g16(0xbb28) == 0
        && gm.g16(0xbb2a) == 0 && gm.g16(0xbb2c) == 0 && gm.g16(0xbb2e) == 0 && gm.g16(0xbb30) == 0
    {
        gm.set_g16(0x3304, 1); // g_transfer_is_avatar
    }
    0
}

/// orig 1000:9438 (66 bytes). Prints the class name ("Mage\n", "Bard\n", "Fighter\n", "Druid\n",
/// "Tinker\n", "Paladin\n", "Ranger\n", "Shepherd\n") for g_party_class; nothing for other letters.
pub fn print_class_name(gm: &mut Game) {
    let s: u16 = match gm.g8(g::PARTY_CLASS) {
        0x42 => 0x330c, 0x44 => 0x331b, 0x46 => 0x3312, 0x4d => 0x3306,
        0x50 => 0x332a, 0x52 => 0x3333, 0x53 => 0x333b, 0x54 => 0x3322,
        _ => return,
    };
    text::print_string(gm, s);
}

/// orig 1000:94aa (63 bytes). Compresses an Ultima IV statistic (max 50) towards the Ultima V
/// scale (max 30): v ≤ 9 unchanged; 10..29 → (v-9)/2+10; ≥ 30 → (v-30)/4+20.
pub fn convert_u4_stat(gm: &mut Game, param_1: i16) -> i16 {
    let mut v = param_1;
    if 9 < v {
        if v < 0x1e {
            v = (v + -9) / 2 + 10;
        } else {
            // signed division by 4 (rounding toward zero) written out with a sign mask
            let m: u16 = (v.wrapping_sub(0x1e) as u16) >> 0xf;
            let m = if m != 0 { 0xffffu16 } else { 0 };
            let t = ((v.wrapping_sub(0x1e) as u16 ^ m).wrapping_sub(m)) as i16;
            v = ((((t >> 2) as u16 ^ m).wrapping_sub(m)) as i16).wrapping_add(0x14);
        }
    }
    v
}

/// orig 1000:94ea (2722 bytes). "Transfer from Ultima IV". Normalises g_location ('B' → 'A'),
/// sets the disk 3/4 drive letters to disk 1's, loads INIT.GAM (0x1060 bytes → 0x55a6) and
/// INIT.OOL (0x100 → 0xb31e), full-screen window; set_default_drive(letter) loop (a key gives a
/// new letter, Esc returns); error hook = 1000:2322; load_ultima4_character (error → return).
/// Avatar flag → class 'A'. Shows "Found:" name, level, sex, class, STR/DEX/INT and the
/// Avatar line; then the two-column before/after conversion panels (setup_transfer_windows,
/// print_stat_label 0..7): keep/enter name (read_line 8 into g_party), keep/change sex,
/// class (Avatar keeps letter 'A'), exp /= 10, level = 1 + floor(log2(exp/100)) (loop), hp =
/// max hp = level*30, str = convert (min 20), dex = convert, int = convert (mp = int), each with
/// a "was (50), now (30)" message; finally window 2 = (2,21)-(37,22), select_disk(3), zeroes the
/// first 0x100 bytes of g_ool_buffer, writes SAVED.OOL (0x200 from 0xb21e) and SAVED.GAM
/// (0x1060 from 0x55a6), select_disk(0), g_display_mode = 0, restores g_location.
pub fn transfer_from_ultima4(gm: &mut Game) {
    let mut loc = gm.g8(g::LOCATION);
    loop {
        gm.set_g8(g::LOCATION, loc);
        if loc != 0x42 { break; }
        loc = 0x41;
    }
    let saved_loc = loc;
    let d1 = gm.g8(g::DISK_DRIVE_LETTERS);
    gm.set_g8(g::DISK3_DRIVE_LETTER, d1);
    gm.set_g8(0xa9cc, d1); // g_disk_drive_letters[4]
    files::load_file_region(gm, 0, 0x1060, 0x55a6, 0x3345); // "INIT.GAM"
    files::load_file_region(gm, 0, 0x100, 0xb31e, 0x334e);  // "INIT.OOL"
    text::define_text_window(gm, 0x18, 0x27, 0, 0, 0);
    text::select_text_window(gm, 0);
    text::put_char(gm, 0xff);
    let mut letter: u8 = gm.g8(g::DISK_DRIVE_LETTERS);
    loop {
        if files::set_default_drive(gm, letter) != 0 { break; }
        let k = input::wait_key(gm, 0);
        letter = util::to_upper(k) as u8;
        if letter == 0x1b {
            // TODO(verify): the original stores an uninitialised register (SI) here; the intent is
            // clearly to restore the saved location.
            gm.set_g8(g::LOCATION, saved_loc);
            return;
        }
    }
    gm.set_g16(g::ERROR_HOOK, 0x2322);
    gm.set_g16(g::ERROR_HOOK_SEG, 0x1000);
    if load_ultima4_character(gm) != 0 {
        gm.set_g8(g::LOCATION, saved_loc);
        return;
    }
    if gm.g16(0x3304) != 0 { gm.set_g8(g::PARTY_CLASS, 0x41); } // Avatar
    text::put_char(gm, 0xff);
    text::set_cursor_pos(gm, 0xb, 0);
    text::put_char(gm, 0xfc);
    text::print_string(gm, 0x33d9); // "Found:\n"
    text::set_cursor_pos(gm, 0xc, 0);
    text::print_string(gm, g::PARTY);
    text::put_char(gm, 0xfb);
    text::set_cursor_pos(gm, 0xd, 0xc);
    text::print_string(gm, 0x33e1); // "a level "
    let lv = gm.g8(0x55be) as i16;
    text::print_number(gm, 0x20, 1, lv);
    let s = if gm.g8(0x55b1) == 0xb { 0x33ea } else { 0x33f1 }; // " Male " / " Female "
    text::print_string(gm, s);
    print_class_name(gm);
    text::set_cursor_pos(gm, 0xf, 0x11);
    text::print_string(gm, 0x33fa); // "STR:  "
    let v = gm.g8(0x55b4) as i16; text::print_number(gm, 0x20, 1, v);
    text::set_cursor_pos(gm, 0x10, 0x11);
    text::print_string(gm, 0x3401); // "DEX:  "
    let v = gm.g8(g::PARTY_DEX) as i16; text::print_number(gm, 0x20, 1, v);
    text::set_cursor_pos(gm, 0x11, 0x11);
    text::print_string(gm, 0x3408); // "INT:  "
    let v = gm.g8(0x55b6) as i16; text::print_number(gm, 0x20, 1, v);
    text::set_cursor_pos(gm, 0x14, 10);
    text::print_string(gm, g::PARTY);
    text::print_string(gm, 0x340f); // " is "
    let s = if gm.g16(0x3304) == 0 { 0x341f } else { 0x3414 }; // "not an Avatar" / "an Avatar."
    text::print_string(gm, s);
    input::wait_key(gm, 0);
    text::put_char(gm, 0xff);
    setup_transfer_windows(gm);
    let mut i: i16 = 0;
    loop { print_stat_label(gm, i); i += 1; if !(i < 8) { break; } }
    text::select_text_window(gm, 0);
    text::set_cursor_pos(gm, 3, 0);
    text::put_char(gm, 0xfc);
    text::print_string(gm, g::PARTY);
    text::put_char(gm, 0xfb);
    text::set_cursor_pos(gm, 5, 10);
    let s = if gm.g8(0x55b1) == 0xb { 0x342d } else { 0x3432 }; // "Male" / "Female"
    text::print_string(gm, s);
    text::set_cursor_pos(gm, 6, 10);
    if gm.g16(0x3304) == 0 { print_class_name(gm); } else { text::print_string(gm, 0x3439); } // "Avatar"
    text::set_cursor_pos(gm, 8, 10);
    let v = gm.gi16(0x55bc); text::print_number(gm, 0x20, 1, v);
    text::set_cursor_pos(gm, 9, 10);
    let v = gm.g8(0x55be) as i16; text::print_number(gm, 0x20, 1, v);
    text::set_cursor_pos(gm, 0xb, 10);
    let v = gm.g8(0x55b4) as i16; text::print_number(gm, 0x20, 1, v);
    text::set_cursor_pos(gm, 0xc, 10);
    let v = gm.g8(g::PARTY_DEX) as i16; text::print_number(gm, 0x20, 1, v);
    text::set_cursor_pos(gm, 0xd, 10);
    let v = gm.g8(0x55b6) as i16; text::print_number(gm, 0x20, 1, v);
    text::set_cursor_pos(gm, 0xf, 0);
    text::put_char(gm, 0xfc);
    let s = if gm.g16(0x3304) == 0 { 0x3447 } else { 0x3440 }; // "Non-Avatar" / "Avatar"
    text::print_string(gm, s);
    text::put_char(gm, 0xfb);
    text::set_cursor_pos(gm, 2, 7);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x3452); // "Name:"
    text::select_text_window(gm, 1);
    text::set_cursor_pos(gm, 2, 7);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x3458); // "Name:"
    text::put_char(gm, 0xfd);
    text::select_text_window(gm, 2);
    text::put_char(gm, 0xff);
    text::set_cursor_pos(gm, 0, 10);
    text::print_string(gm, 0x345e); // "Keep this name?"
    let mut yn: u8;
    loop {
        let k = input::wait_key(gm, 0);
        yn = util::to_upper(k) as u8;
        if yn == b'Y' { break; }
        if yn == b'N' { break; }
        if gm.pf.quit_requested { yn = b'Y'; break; }
    }
    text::put_char(gm, 0xff);
    if yn == b'N' {
        loop {
            text::set_cursor_pos(gm, 0, 1);
            text::print_string(gm, 0x346e); // "Enter new name: "
            input::read_line(gm, 8, g::PARTY);
            if gm.g8(g::PARTY) != 0 || gm.pf.quit_requested { break; }
        }
    }
    text::select_text_window(gm, 1);
    text::set_cursor_pos(gm, 3, 0);
    text::put_char(gm, 0xfc);
    text::print_string(gm, g::PARTY);
    text::put_char(gm, 0xfb);
    text::set_cursor_pos(gm, 2, 7);
    text::print_string(gm, 0x347f); // "Name:"
    text::set_cursor_pos(gm, 5, 5);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x3485); // "Sex:"
    text::put_char(gm, 0xfd);
    text::select_text_window(gm, 0);
    text::set_cursor_pos(gm, 2, 7);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x348a); // "Name:"
    text::set_cursor_pos(gm, 5, 5);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x3490); // "Sex:"
    text::put_char(gm, 0xfd);
    text::select_text_window(gm, 2);
    text::put_char(gm, 0xff);
    text::put_char(gm, 0xfc);
    text::print_string(gm, 0x3495); // "Keep same sex?"
    text::put_char(gm, 0xfb);
    loop {
        let k = input::wait_key(gm, 0);
        yn = util::to_upper(k) as u8;
        if yn == b'Y' { break; }
        if yn == b'N' { break; }
        if gm.pf.quit_requested { yn = b'Y'; break; }
    }
    text::select_text_window(gm, 1);
    text::set_cursor_pos(gm, 5, 10);
    if (yn == b'Y' && gm.g8(0x55b1) == 0xb) || (yn == b'N' && gm.g8(0x55b1) == 0xc) {
        text::print_string(gm, 0x34a4); // "Male"
        gm.set_g8(0x55b1, 0xb);
    } else {
        text::print_string(gm, 0x34a9); // "Female"
        gm.set_g8(0x55b1, 0xc);
    }
    text::set_cursor_pos(gm, 5, 5);
    text::print_string(gm, 0x34b0); // "Sex:"
    text::set_cursor_pos(gm, 6, 3);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x34b5); // "Class:"
    text::put_char(gm, 0xfd);
    text::set_cursor_pos(gm, 6, 10);
    if gm.g16(0x3304) == 0 { print_class_name(gm); } else { text::print_string(gm, 0x34bc); } // "Avatar"
    text::select_text_window(gm, 0);
    text::set_cursor_pos(gm, 5, 5);
    text::print_string(gm, 0x34c3); // "Sex:"
    text::set_cursor_pos(gm, 6, 3);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x34c8); // "Class:"
    text::put_char(gm, 0xfd);
    text::select_text_window(gm, 2);
    text::put_char(gm, 0xff);
    text::set_cursor_pos(gm, 0, 2);
    let s = if gm.g16(0x3304) == 0 { 0x34e7 } else { 0x34cf }; // "Class remains intact" / "Thou art now an Avatar:"
    text::print_string(gm, s);
    input::wait_key(gm, 0);
    text::select_text_window(gm, 0);
    text::set_cursor_pos(gm, 6, 3);
    text::print_string(gm, 0x34fc); // "Class:"
    text::set_cursor_pos(gm, 8, 5);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x3503); // "Exp:"
    text::put_char(gm, 0xfd);
    text::select_text_window(gm, 1);
    text::set_cursor_pos(gm, 6, 3);
    text::print_string(gm, 0x3508); // "Class:"
    text::set_cursor_pos(gm, 8, 5);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x350f); // "Exp:"
    text::put_char(gm, 0xfd);
    text::set_cursor_pos(gm, 8, 10);
    let exp = gm.gi16(0x55bc) / 10;
    gm.set_gi16(0x55bc, exp);
    text::print_number(gm, 0x20, 1, exp);
    text::select_text_window(gm, 2);
    text::put_char(gm, 0xff);
    text::put_char(gm, 0xfc);
    text::print_string(gm, 0x3514); // "Experience has been converted"
    text::put_char(gm, 0xfb);
    input::wait_key(gm, 0);
    text::select_text_window(gm, 0);
    text::set_cursor_pos(gm, 8, 5);
    text::print_string(gm, 0x3532); // "Exp:"
    text::set_cursor_pos(gm, 9, 3);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x3537); // "Level:"
    text::put_char(gm, 0xfd);
    text::select_text_window(gm, 1);
    text::set_cursor_pos(gm, 8, 5);
    text::print_string(gm, 0x353e); // "Exp:"
    text::set_cursor_pos(gm, 9, 3);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x3543); // "Level:"
    text::put_char(gm, 0xfd);
    text::set_cursor_pos(gm, 9, 10);
    gm.set_g8(0x55be, 1);
    let mut e = gm.gi16(0x55bc) / 100;
    while 0 < e {
        let l = gm.g8(0x55be).wrapping_add(1);
        gm.set_g8(0x55be, l);
        e >>= 1;
    }
    let hp = (gm.g8(0x55be) as u16).wrapping_mul(0x1e);
    gm.set_g16(g::PARTY_HP, hp);
    gm.set_g16(g::PARTY_MAX_HP, hp);
    let lv = gm.g8(0x55be) as i16;
    text::print_number(gm, 0x20, 1, lv);
    text::select_text_window(gm, 2);
    text::put_char(gm, 0xff);
    text::put_char(gm, 0xfc);
    text::print_string(gm, 0x354a); // "Level has been converted"
    text::put_char(gm, 0xfb);
    input::wait_key(gm, 0);
    text::select_text_window(gm, 0);
    text::set_cursor_pos(gm, 9, 3);
    text::print_string(gm, 0x3563); // "Level:"
    text::set_cursor_pos(gm, 0xb, 5);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x356a); // "STR:"
    text::put_char(gm, 0xfd);
    text::select_text_window(gm, 1);
    text::set_cursor_pos(gm, 9, 3);
    text::print_string(gm, 0x356f); // "Level:"
    text::set_cursor_pos(gm, 0xb, 5);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x3576); // "STR:"
    text::put_char(gm, 0xfd);
    text::set_cursor_pos(gm, 0xb, 10);
    let old = gm.g8(0x55b4);
    let v = convert_u4_stat(gm, old as i16) as u8;
    gm.set_g8(0x55b4, v);
    if gm.g8(0x55b4) < 0x14 { gm.set_g8(0x55b4, 0x14); }
    let v = gm.g8(0x55b4) as i16;
    text::print_number(gm, 0x20, 1, v);
    text::select_text_window(gm, 2);
    text::put_char(gm, 0xff);
    text::set_cursor_pos(gm, 0, 1);
    text::print_string(gm, 0x357b); // "Strength: was "
    text::print_number(gm, 0x20, 1, old as i16);
    text::print_string(gm, 0x358a); // "(50), now "
    let v = gm.g8(0x55b4) as i16;
    text::print_number(gm, 0x20, 1, v);
    text::print_string(gm, 0x3595); // "(30)"
    input::wait_key(gm, 0);
    text::select_text_window(gm, 0);
    text::set_cursor_pos(gm, 0xb, 5);
    text::print_string(gm, 0x359a); // "STR:"
    text::set_cursor_pos(gm, 0xc, 5);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x359f); // "DEX:"
    text::put_char(gm, 0xfd);
    text::select_text_window(gm, 1);
    text::set_cursor_pos(gm, 0xb, 5);
    text::print_string(gm, 0x35a4); // "STR:"
    text::set_cursor_pos(gm, 0xc, 5);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x35a9); // "DEX:"
    text::put_char(gm, 0xfd);
    text::set_cursor_pos(gm, 0xc, 10);
    let old = gm.g8(g::PARTY_DEX);
    let v = convert_u4_stat(gm, old as i16) as u8;
    gm.set_g8(g::PARTY_DEX, v);
    let v = gm.g8(g::PARTY_DEX) as i16;
    text::print_number(gm, 0x20, 1, v);
    text::select_text_window(gm, 2);
    text::put_char(gm, 0xff);
    text::set_cursor_pos(gm, 0, 1);
    text::print_string(gm, 0x35ae); // "Dexterity: was "
    text::print_number(gm, 0x20, 1, old as i16);
    text::print_string(gm, 0x35be); // "(50), now "
    let v = gm.g8(g::PARTY_DEX) as i16;
    text::print_number(gm, 0x20, 1, v);
    text::print_string(gm, 0x35c9); // "(30)"
    input::wait_key(gm, 0);
    text::select_text_window(gm, 0);
    text::set_cursor_pos(gm, 0xc, 5);
    text::print_string(gm, 0x35ce); // "DEX:"
    text::set_cursor_pos(gm, 0xd, 5);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x35d3); // "INT:"
    text::put_char(gm, 0xfd);
    text::select_text_window(gm, 1);
    text::set_cursor_pos(gm, 0xc, 5);
    text::print_string(gm, 0x35d8); // "DEX:"
    text::set_cursor_pos(gm, 0xd, 5);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x35dd); // "INT:"
    text::put_char(gm, 0xfd);
    text::set_cursor_pos(gm, 0xd, 10);
    let old = gm.g8(0x55b6);
    let v = convert_u4_stat(gm, old as i16) as u8;
    gm.set_g8(0x55b6, v);
    gm.set_g8(0x55b7, v); // mp = int
    let v = gm.g8(0x55b6) as i16;
    text::print_number(gm, 0x20, 1, v);
    text::select_text_window(gm, 2);
    text::put_char(gm, 0xff);
    text::set_cursor_pos(gm, 0, 1);
    text::print_string(gm, 0x35e2); // "Intellect: was "
    text::print_number(gm, 0x20, 1, old as i16);
    text::print_string(gm, 0x35f2); // "(50), now "
    let v = gm.g8(0x55b6) as i16;
    text::print_number(gm, 0x20, 1, v);
    text::print_string(gm, 0x35fd); // "(30)"
    input::wait_key(gm, 0);
    text::select_text_window(gm, 0);
    text::set_cursor_pos(gm, 0xd, 5);
    text::print_string(gm, 0x3602); // "INT:"
    text::select_text_window(gm, 1);
    text::set_cursor_pos(gm, 0xd, 5);
    text::print_string(gm, 0x3607); // "INT:"
    text::set_cursor_pos(gm, 0xf, 0);
    text::put_char(gm, 0xfc);
    let s = if gm.g16(0x3304) == 0 { 0x3613 } else { 0x360c }; // "Non-Avatar" / "Avatar"
    text::print_string(gm, s);
    text::put_char(gm, 0xfb);
    text::select_text_window(gm, 2);
    text::define_text_window(gm, 0x16, 0x25, 0x15, 2, 2);
    text::put_char(gm, 0xff);
    files::select_disk(gm, 3);
    text::print_string(gm, 0x361e); // "\n\n Conversion complete, saving...\n"
    let mut p: u16 = g::OOL_BUFFER;
    let mut n: i16 = 0x100;
    while n != 0 {
        gm.set_g8(p, 0);
        p = p.wrapping_add(1);
        n -= 1;
    }
    files::save_file_region(gm, 0x200, g::OOL_BUFFER, 0x3641); // "SAVED.OOL"
    files::save_file_region(gm, 0x1060, 0x55a6, 0x364b);       // "SAVED.GAM"
    files::select_disk(gm, 0);
    gm.set_g8(g::DISPLAY_MODE, 0);
    gm.set_g8(g::LOCATION, saved_loc);
}

/// orig 1000:9fe2 (63 bytes). Prints stat label `i` (0..7: Name/Sex/Class/Exp/Level/STR/DEX/INT
/// from g_str_stat_label_ptrs 0x3666) at column 3 of row g_stat_label_rows[i] (0x3656:
/// 2,5,6,8,9,11,12,13) in both transfer windows 0 and 1.
pub fn print_stat_label(gm: &mut Game, param_1: i16) {
    text::select_text_window(gm, 0);
    let i = (param_1 * 2) as u16;
    let row = gm.gi16(0x3656 + i);
    let s = gm.g16(0x3666 + i);
    text::set_cursor_pos(gm, row, 3);
    text::print_string(gm, s);
    text::select_text_window(gm, 1);
    let row = gm.gi16(0x3656 + i);
    let s = gm.g16(0x3666 + i);
    text::set_cursor_pos(gm, row, 3);
    text::print_string(gm, s);
}

/// orig 1000:a022 (195 bytes). Draws the transfer message box: glyph frame in g_ui_color2 from
/// row 19 (40 columns, 3 inner rows) and a g_ui_color1 rectangle (7,159)-(312,184).
pub fn draw_transfer_message_frame(gm: &mut Game) {
    let c2 = gm.g16(g::UI_COLOR2);
    text::set_text_color(gm, c2 as u8);
    text::set_cursor_pos(gm, 0x13, 0);
    text::put_char(gm, 0x7b);
    let mut n: i16 = 0x26;
    loop { text::put_char(gm, 0x7f); n -= 1; if n == 0 { break; } }
    text::put_char(gm, 0x7c);
    n = 3;
    loop {
        text::put_char(gm, 0x7f);
        let row = text::get_cursor_row(gm);
        text::set_cursor_pos(gm, row, 0x27);
        text::put_char(gm, 0x7f);
        n -= 1;
        if n == 0 { break; }
    }
    text::put_char(gm, 0x7d);
    n = 0x26;
    loop { text::put_char(gm, 0x7f); n -= 1; if n == 0 { break; } }
    text::put_char(gm, 0x7e);
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    gfx::draw_line(gm, 0x9f, 0x138, 0x9f, 7);
    gfx::draw_line_to(gm, 0xb8, 0x138);
    gfx::draw_line_to(gm, 0xb8, 7);
    gfx::draw_line_to(gm, 0x9f, 7);
    let c1 = gm.g16(g::UI_COLOR1);
    text::set_text_color(gm, c1 as u8);
}

/// orig 1000:a0e6 (253 bytes). Draws one transfer stat panel (20 columns wide) whose left pixel
/// edge is `x`: g_ui_color2 borders (left 0..7, right x+0x8f..x+0x97, bottom rows 137..143),
/// the "{" + " Ultima IV " (in g_ui_color1) + "}" caption on row 0 of the current window, the
/// 0x7c/0x7d/0x7e corner glyphs and a g_ui_color1 outline (x+7,7)-(x+0x8f,0x88) broken at the
/// caption. Ghidra order: (x).
pub fn draw_transfer_stat_box(gm: &mut Game, param_1: i16) {
    let c2 = gm.gi16(g::UI_COLOR2);
    gfx::set_color(gm, c2);
    gfx::fill_rect(gm, 0x8f, param_1 + 6, 0, param_1);
    let xr = param_1 + 0x8f;
    gfx::fill_rect(gm, 0x89, param_1 + 0x97, 0, xr);
    let xl = param_1 + 7;
    gfx::fill_rect(gm, 0x8f, param_1 + 0x96, 0x89, xl);
    let c2 = gm.g16(g::UI_COLOR2);
    text::set_text_color(gm, c2 as u8);
    text::set_cursor_pos(gm, 0, 0);
    text::print_string(gm, 0x3676); // "{\x7f\x7f"
    rest::draw_bar_left_cap(gm);
    let c1 = gm.g16(g::UI_COLOR1);
    text::set_text_color(gm, c1 as u8);
    text::print_string(gm, 0x6d3a); // " Ultima IV "
    let c2 = gm.g16(g::UI_COLOR2);
    text::set_text_color(gm, c2 as u8);
    rest::draw_bar_right_cap(gm);
    text::print_string(gm, 0x367a); // "\x7f\x7f"
    text::put_char(gm, 0x7c);
    text::set_cursor_pos(gm, 0x11, 0);
    text::put_char(gm, 0x7d);
    text::set_cursor_pos(gm, 0x11, 0x12);
    text::put_char(gm, 0x7e);
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    gfx::draw_line(gm, 7, xl, 7, param_1 + 0x18);
    gfx::draw_line_to(gm, 0x88, xl);
    gfx::draw_line_to(gm, 0x88, xr);
    gfx::draw_line_to(gm, 7, xr);
    gfx::draw_line_to(gm, 7, param_1 + 0x80);
    let c1 = gm.g16(g::UI_COLOR1);
    text::set_text_color(gm, c1 as u8);
}

/// orig 1000:a1e4 (108 bytes). Lays out the Ultima IV transfer screen: message frame, window 0 =
/// columns 0..19 rows 0..18 (before), window 1 = columns 21..39 (after), window 2 = (3,21)-(37,21)
/// message line; draws both stat boxes (x = 0 and 168) and blanks the caption cell (0,12) of
/// window 1.
pub fn setup_transfer_windows(gm: &mut Game) {
    draw_transfer_message_frame(gm);
    text::define_text_window(gm, 0x12, 0x13, 0, 0, 0);
    text::define_text_window(gm, 0x12, 0x27, 0, 0x15, 1);
    text::define_text_window(gm, 0x15, 0x25, 0x15, 3, 2);
    text::select_text_window(gm, 0);
    draw_transfer_stat_box(gm, 0);
    text::select_text_window(gm, 1);
    draw_transfer_stat_box(gm, 0xa8);
    text::set_cursor_pos(gm, 0, 0xc);
    text::put_char(gm, 0x20);
}

/// orig 1000:a250 (29 bytes). Vector 1778:0572 (also used by FONT and ROOT). One title-fire
/// tick: wait_ticks(1) then driver entry 35 with CF=0 (next 49-row fire frame band from page 1
/// into page 0 rows 65..113).
pub fn title_fire_tick(gm: &mut Game) {
    util::wait_ticks(gm, 1);
    effects::title_fire_frame(gm);
}

/// orig 1000:a26e (25 bytes). Driver entry 35 with CF=1 and AX = segment of the decompressed
/// WD.BIT: the burning-title dissolve of page 1 through the fire mask. Ghidra order: (mask_seg).
pub fn title_fire_burn(gm: &mut Game, param_1: u16) {
    let _ = effects::title_fire_dissolve(gm, param_1);
}
