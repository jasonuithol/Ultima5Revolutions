//! ROOT text output: 4 text windows (8x8 cells), put_char, print_string, print_number, fonts.
//! Parameter order = Ghidra decompiled order (reverse of source order).
use crate::game::Game;
use crate::g;
use crate::mem::Mem;

pub const WIN_X0: u16 = 0; pub const WIN_Y0: u16 = 1; pub const WIN_X1: u16 = 2; pub const WIN_Y1: u16 = 3;
pub const WIN_CURX: u16 = 4; pub const WIN_CURY: u16 = 5; pub const WIN_ATTR: u16 = 6; pub const WIN_FLAGS: u16 = 7;

#[inline] pub fn cur_win(gm: &Game) -> u16 { gm.g16(g::CUR_WINDOW_PTR) }
#[inline] pub fn win_u8(gm: &Game, field: u16) -> u8 { let w = cur_win(gm); gm.g8(w + field) }
#[inline] pub fn set_win_u8(gm: &mut Game, field: u16, v: u8) { let w = cur_win(gm); gm.set_g8(w + field, v); }

pub fn init_text_windows(gm: &mut Game) {
    gm.set_g16(g::CUR_WINDOW_PTR, g::TEXT_WINDOWS);
    for n in 0..4u16 {
        let b = g::TEXT_WINDOWS + n * 8;
        for (i, v) in [0u8, 0, 0x27, 0x18, 0, 0, 0x0f, 0].iter().enumerate() { gm.set_g8(b + i as u16, *v); }
    }
    gm.set_gi16(g::CUR_WINDOW, 0);
}
pub fn reset_text_attrs(gm: &mut Game) {
    gm.set_g16(g::TEXT_UNDERLINE, 0); gm.set_g16(g::TEXT_CENTRED, 0); gm.set_g16(g::TEXT_INVERSE, 0);
}
/// Inclusive pixel rect of the current window.
pub fn window_pixel_rect(gm: &Game) -> (i16, i16, i16, i16) {
    let w = cur_win(gm);
    (gm.g8(w) as i16 * 8, gm.g8(w + 1) as i16 * 8, gm.g8(w + 2) as i16 * 8 + 7, gm.g8(w + 3) as i16 * 8 + 7)
}
/// Font glyph (8 bytes) for character `ch` from the current font segment, with underline/inverse applied.
fn prepare_glyph(gm: &Game, ch: u8) -> [u8; 8] {
    let font = (gm.g16(g::FONT_SEG) as u32) << 4;
    let mut gl = [0u8; 8];
    for i in 0..8 { gl[i] = gm.m.u8(font + ch as u32 * 8 + i as u32); }
    if gm.g16(g::TEXT_UNDERLINE) != 0 { gl[7] = 0xff; }
    if gm.g16(g::TEXT_INVERSE) != 0 { for b in gl.iter_mut() { *b = !*b; } }
    gl
}
/// put_char(ch): the basic text primitive (control codes 0xfb..0xff, CR/LF, wrap, scroll).
pub fn put_char(gm: &mut Game, ch: u8) {
    let mut c = ch;
    if c > 0x7f {
        match c {
            0xff => {
                set_win_u8(gm, WIN_CURX, 0); set_win_u8(gm, WIN_CURY, 0);
                let bg = win_u8(gm, WIN_ATTR) >> 4;
                gm.pf.video.set_color(bg);
                let (x1, y1, x2, y2) = window_pixel_rect(gm);
                gm.pf.video.fill_rect(x1, y1, x2, y2, false);
                let cc = gm.gi16(g::CUR_COLOR) as u8;
                gm.pf.video.set_color(cc);
                return;
            }
            0xfe => { let v = gm.g16(g::TEXT_UNDERLINE) ^ 1; gm.set_g16(g::TEXT_UNDERLINE, v); let f = win_u8(gm, WIN_FLAGS) ^ 1; set_win_u8(gm, WIN_FLAGS, f); return; }
            0xfd => { let v = gm.g16(g::TEXT_INVERSE) ^ 1; gm.set_g16(g::TEXT_INVERSE, v); let f = win_u8(gm, WIN_FLAGS) ^ 4; set_win_u8(gm, WIN_FLAGS, f); return; }
            0xfc => { gm.set_g16(g::TEXT_CENTRED, 1); let f = win_u8(gm, WIN_FLAGS) | 2; set_win_u8(gm, WIN_FLAGS, f); return; }
            0xfb => { gm.set_g16(g::TEXT_CENTRED, 0); let f = win_u8(gm, WIN_FLAGS) & 0xfd; set_win_u8(gm, WIN_FLAGS, f); return; }
            _ => { c &= 0x7f; }
        }
    }
    let mut newline = false;
    if c == 10 {
        newline = true;
    } else if c != 13 {
        let gl = prepare_glyph(gm, c);
        let x = win_u8(gm, WIN_X0).wrapping_add(win_u8(gm, WIN_CURX));
        let y = win_u8(gm, WIN_Y0).wrapping_add(win_u8(gm, WIN_CURY));
        let (fg, bg) = (gm.g8(g::TEXT_FG), gm.g8(g::TEXT_BG));
        gm.pf.video.draw_glyph(x, y, fg, bg, &gl);
        if gm.g16(g::CURSOR_ADVANCE) == 0 { return; }
        let cx = win_u8(gm, WIN_CURX).wrapping_add(1);
        set_win_u8(gm, WIN_CURX, cx);
        if (cx.wrapping_add(win_u8(gm, WIN_X0)) as i8) <= win_u8(gm, WIN_X1) as i8 { return; }
        newline = true;
    }
    if newline { let cy = win_u8(gm, WIN_CURY).wrapping_add(1); set_win_u8(gm, WIN_CURY, cy); }
    // carriage return (also reached after a line feed / wrap)
    set_win_u8(gm, WIN_CURX, 0);
    if (win_u8(gm, WIN_Y1) as i8) < (win_u8(gm, WIN_CURY).wrapping_add(win_u8(gm, WIN_Y0))) as i8 {
        let (x1, y1, x2, y2) = window_pixel_rect(gm);
        let cy = win_u8(gm, WIN_CURY).wrapping_sub(1); set_win_u8(gm, WIN_CURY, cy);
        gm.pf.video.scroll_rect(x1, y1, x2, y2, -8);
    }
}
pub fn get_cursor_col(gm: &Game) -> i16 { win_u8(gm, WIN_CURX) as i16 }
pub fn get_cursor_row(gm: &Game) -> i16 { win_u8(gm, WIN_CURY) as i16 }
/// set_cursor_pos(col,row) — Ghidra order (row,col)
pub fn set_cursor_pos(gm: &mut Game, row: i16, col: i16) {
    let w = cur_win(gm);
    if (col as u8).wrapping_add(gm.g8(w)) < 0x28 && (row as u8).wrapping_add(gm.g8(w + 1)) < 0x19 {
        gm.set_g8(w + WIN_CURX, col as u8); gm.set_g8(w + WIN_CURY, row as u8);
    }
}
/// print_string(s): word-wrapped output of the NUL-terminated string at linear address `s`.
pub fn print_string(gm: &mut Game, s: u16) {
    let text: Vec<u8> = gm.m.cstr(gm.ds(s)).to_vec();
    print_bytes(gm, &text);
}
pub fn print_bytes(gm: &mut Game, p: &[u8]) {
    let at = |i: usize| -> u8 { if i < p.len() { p[i] } else { 0 } };
    if at(0) == 0 { return; }
    let wi = gm.gi16(g::CUR_WINDOW) as u16 * 8;
    let win = g::TEXT_WINDOWS + wi;
    let centred = gm.g8(win + 7) & 2 != 0;
    let width = gm.g8(win + 2) as i16 - gm.g8(win) as i16;
    let mut pos: usize = 0;          // local_8
    let mut done = false;            // local_a
    let mut pending_nl = false;      // local_4
    let mut buf = [0u8; 40];
    loop {
        if pending_nl { put_char(gm, 10); }
        let mut start = 0usize;      // local_14
        let mut skip_spaces = false; // local_6
        pending_nl = false;
        let col = get_cursor_col(gm);
        let avail = width - col;     // local_10
        let mut n = 0usize;          // iVar4
        let mut i = pos;
        while at(i) != b'\n' && at(i) != b'\r' && at(i) != 0 && (n as i16) <= width - col {
            if n < 40 { buf[n] = at(i); }
            n += 1; i += 1;
        }
        pos = i;
        let mut len = n as i16;      // local_c
        if n == 0 {
            if at(i) == 0 { done = true; } else { pos = i + 1; put_char(gm, at(i)); }
        } else {
            if avail < n as i16 {
                // back up to the last space
                while at(pos) != b' ' && len != 0 && at(pos) != b'\n' && at(pos) != 0 && at(pos) != b'\r' { pos -= 1; len -= 1; }
                if len == 0 {
                    len = n as i16; pos = i;
                    if gm.g8(win + 4) != 0 { put_char(gm, 10); skip_spaces = true; }
                } else {
                    // trim trailing spaces / CR / LF
                    loop {
                        loop { len -= 1; if !(len >= 0 && buf[len as usize] == b' ') { break; } }
                        if !(len >= 0 && (buf[len as usize] == b'\r' || buf[len as usize] == b'\n')) { break; }
                    }
                    if at(pos) != 0 { pos += 1; }
                    if len < avail { pending_nl = true; }
                }
                if avail < len { len -= 1; }
            } else { len -= 1; }
            if centred {
                if width < len { put_char(gm, 10); }
                let c = (avail - len) / 2;
                let r = get_cursor_row(gm);
                set_cursor_pos(gm, r, c);
            }
            if skip_spaces { while start < 40 && buf[start] == b' ' { start += 1; } }
            let mut k = start as i16;
            while k <= len { put_char(gm, buf[k as usize]); k += 1; }
        }
        if done { break; }
    }
}
/// print_number(value,width,pad) — Ghidra order (pad,width,value)
pub fn print_number(gm: &mut Game, pad: u8, mut width: i16, mut value: i16) {
    if width > 0x27 { width = 0x27; }
    let neg = value < 0;
    if neg { value = value.wrapping_neg(); }
    let pow10 = [1i16, 10, 100, 1000, 10000];
    let mut digits = 1i16;
    for k in 1..5 { if pow10[k] <= value { digits += 1; } }
    if neg { digits += 1; }
    let mut out: Vec<u8> = Vec::new();
    let padn = (width - digits).max(0);
    for _ in 0..padn { out.push(pad); }
    if neg { out.push(b'-'); digits -= 1; }
    let mut v = value as i32;
    while digits > 0 {
        let p = pow10[(digits - 1) as usize] as i32;
        let d = v / p; out.push(b'0' + d as u8); v -= p * d; digits -= 1;
    }
    print_bytes(gm, &out);
}
pub fn select_text_window(gm: &mut Game, n: u16) {
    if n < 4 {
        gm.set_gi16(g::CUR_WINDOW, n as i16);
        let w = g::TEXT_WINDOWS + n * 8;
        gm.set_g16(g::CUR_WINDOW_PTR, w);
        let attr = gm.g8(w + 6);
        gm.set_g8(g::TEXT_FG, attr & 0xf); gm.set_g8(g::TEXT_BG, attr >> 4);
        let f = gm.g8(w + 7);
        gm.set_g16(g::TEXT_UNDERLINE, (f & 1) as u16); gm.set_g16(g::TEXT_CENTRED, ((f & 2) >> 1) as u16); gm.set_g16(g::TEXT_INVERSE, ((f & 4) >> 2) as u16);
    }
}
/// define_text_window(n,x0,y0,x1,y1) — Ghidra order (y1,x1,y0,x0,n)
pub fn define_text_window(gm: &mut Game, y1: i16, x1: i16, y0: i16, x0: i16, n: u16) {
    if n < 4 {
        let cl = |v: i16, max: i16| v.clamp(0, max) as u8;
        let (mut a, mut b, mut c, mut d) = (cl(x0, 39), cl(y0, 24), cl(x1, 39), cl(y1, 24));
        if a > c { std::mem::swap(&mut a, &mut c); }
        if b > d { std::mem::swap(&mut b, &mut d); }
        let w = g::TEXT_WINDOWS + n * 8;
        gm.set_g8(w, a); gm.set_g8(w + 1, b); gm.set_g8(w + 2, c); gm.set_g8(w + 3, d);
    }
}
pub fn select_font(gm: &mut Game, n: u16) {
    if n < 4 {
        let seg = gm.g16(g::FONT_SEGS + n * 2);
        if seg != 0 { gm.set_g16(g::FONT_SEG, seg); gm.set_gi16(g::CUR_FONT, n as i16); }
    }
}
pub fn set_text_color(gm: &mut Game, fg: u8) {
    gm.set_g8(g::TEXT_FG, fg & 0xf);
    let a = win_u8(gm, WIN_ATTR) & 0xf0 | (fg & 0xf);
    set_win_u8(gm, WIN_ATTR, a);
}
pub fn set_text_bg(gm: &mut Game, bg: u8) {
    gm.set_g8(g::TEXT_BG, bg & 0xf);
    let a = win_u8(gm, WIN_ATTR) & 0x0f | (bg << 4);
    set_win_u8(gm, WIN_ATTR, a);
}
/// load_font(slot,name): loads a .CH font into slot 0..3; returns size, 0 on error, -2 bad slot.
pub fn load_font(gm: &mut Game, slot: u16, name: u16) -> i16 {
    if slot > 3 { return -2; }
    let nm = gm.gstr(name);
    match gm.read_file(&nm) {
        Some(data) => {
            let seg = match gm.alloc_seg(data.len() as u32) { Some(s) => s, None => return -1 };
            gm.m.write((seg as u32) << 4, &data);
            gm.set_g16(g::FONT_SEGS + slot * 2, seg);
            data.len() as i16
        }
        None => { gm.set_g16(g::LAST_DOS_ERROR, 2); crate::engine::files::call_error_hook(gm); 0 }
    }
}
/// backspace_chars(count): erase `count` characters backwards for line editing.
pub fn backspace_chars(gm: &mut Game, count: i16) {
    if get_cursor_col(gm) == 0 && get_cursor_row(gm) == 0 { return; }
    if count == 0 { return; }
    let saved = gm.g16(g::CURSOR_ADVANCE);
    gm.set_g16(g::CURSOR_ADVANCE, 0);
    if count > 0 {
        let wi = gm.gi16(g::CUR_WINDOW) as u16 * 8;
        let width = gm.g8(g::TEXT_WINDOWS + wi + 2) as i16 - gm.g8(g::TEXT_WINDOWS + wi) as i16;
        for _ in 0..count {
            let col = get_cursor_col(gm);
            put_char(gm, b' ');
            if col == 0 { let r = get_cursor_row(gm) - 1; set_cursor_pos(gm, r, width); }
            else { let r = get_cursor_row(gm); set_cursor_pos(gm, r, col - 1); }
        }
    }
    gm.set_g16(g::CURSOR_ADVANCE, saved);
}
/// Convenience for hand-written code: print a Rust string literal (stored in scratch memory).
pub fn print_str(gm: &mut Game, s: &str) { print_bytes(gm, s.as_bytes()); }
#[allow(dead_code)]
pub fn ds(off: u16) -> u32 { Mem::ds(off) }
