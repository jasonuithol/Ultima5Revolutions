//! ROOT keyboard layer: read_key / wait_key / read_line / read_command_key.
use crate::game::Game;
use crate::g;
use crate::engine::text::put_char;

/// read_key(): non-blocking; returns the game key code (0 = none).
pub fn read_key(gm: &mut Game) -> u16 {
    gm.set_g16(g::KEY_FROM_KEYPAD, 0);
    gm.pf.pump();
    let k = match gm.pf.keys.take() { Some(k) => k, None => return 0 };
    let (scan, asc) = ((k >> 8) as u8, (k & 0xff) as u8);
    let mut result: u16;
    if asc != 0 {
        if asc > 0x30 && asc < 0x3a {
            let keypad = (0x47..=0x53).contains(&scan) || gm.pf.keys.shift_flags() & 0x23 != 0;
            if keypad { let v = gm.g16(g::KEY_FROM_KEYPAD) + 1; gm.set_g16(g::KEY_FROM_KEYPAD, v); }
        }
        result = asc as u16;
    } else {
        result = 0;
        if scan > 0x3a {
            if scan < 0x45 { result = scan as u16 + 0x8e; }
            else {
                for i in 0..8u16 {
                    if scan == gm.g8(g::SCAN_TABLE + i) {
                        result = gm.g8(g::SCAN_XLAT + i) as u16;
                        let v = gm.g16(g::KEY_FROM_KEYPAD) + 1; gm.set_g16(g::KEY_FROM_KEYPAD, v);
                        break;
                    }
                }
            }
        }
        if result == 0 { return 0; }
    }
    if gm.g16(g::FLUSH_AFTER_KEY) != 0 { gm.pf.keys.flush(); }
    if std::env::var("U5_TRACE_KEYS").is_ok() { eprintln!("[key] raw={k:04x} -> {result:#x} keypad={}", gm.g16(g::KEY_FROM_KEYPAD)); }
    result
}
/// Extended-key path for arrows when the window reports them with ASCII 0 but our queue carries a scan code only.
pub fn flush_keyboard(gm: &mut Game) { gm.pf.keys.flush(); }

/// poll_key_with_cursor(): one iteration of the animated input cursor.
pub fn poll_key_with_cursor(gm: &mut Game) -> i16 {
    let saved = gm.g16(g::CURSOR_ADVANCE);
    gm.set_g16(g::CURSOR_ADVANCE, 0);
    let glyph = gm.gi16(g::CURSOR_GLYPH_BASE) + gm.gi16(g::CURSOR_FRAME);
    let f = gm.gi16(g::CURSOR_FRAME) + 1; gm.set_gi16(g::CURSOR_FRAME, f);
    put_char(gm, glyph as u8);
    let k = read_key(gm) as i16;
    if gm.gi16(g::CURSOR_FRAMES) <= gm.gi16(g::CURSOR_FRAME) { gm.set_gi16(g::CURSOR_FRAME, 0); }
    if k == 0 { crate::engine::util::wait_ticks(gm, 1); } else { put_char(gm, b' '); }
    gm.set_g16(g::CURSOR_ADVANCE, saved);
    k
}
/// wait_key(mode): 0 = block, 1 = single poll, N>1 = up to N polls.
pub fn wait_key(gm: &mut Game, mode: i16) -> i16 {
    let mut n: i16 = if mode != 0 { 0 } else { -1 };
    let saved = gm.g16(g::CPU_SPEED);
    if mode > 1 { gm.set_g16(g::CPU_SPEED, 500); }
    let mut k;
    loop {
        k = poll_key_with_cursor(gm);
        if mode != 0 { n += 1; }
        if !(n < mode && k == 0) { break; }
        if gm.pf.quit_requested { k = 0x1b; break; }
    }
    if mode > 1 { gm.set_g16(g::CPU_SPEED, saved); }
    k
}
/// read_line(buf,maxlen) — Ghidra order (maxlen,buf): line editor into emulated memory at `buf`.
pub fn read_line(gm: &mut Game, maxlen: i16, buf: u16) {
    let buf = gm.ds(buf);
    let mut n: i16 = 0;
    loop {
        let k = wait_key(gm, 0);
        if (k == 8 || k == 1) && n != 0 { crate::engine::text::backspace_chars(gm, 1); n -= 1; }
        else if k == 0x1b && n != 0 { crate::engine::text::backspace_chars(gm, n); n = 0; }
        else if k > 0x1f && k < 0x80 && n < maxlen { gm.m.set_u8(buf + n as u32, k as u8); n += 1; put_char(gm, k as u8); }
        if k == 0x0d || gm.pf.quit_requested { break; }
    }
    gm.m.set_u8(buf + n as u32, 0);
}
/// read_command_key(): blocking key read with map animation and keypad/arrow translation.
pub fn read_command_key(gm: &mut Game) -> i16 {
    if gm.gi16(g::ADAPTER_TYPE) == 2 { gm.set_g8(g::ANIMATE_FLAG, 0xff); }
    let mut key: i16;
    loop {
        key = crate::engine::util::to_upper(poll_key_with_cursor(gm));
        if key != 0 || gm.pf.quit_requested { break; }
        let loc = gm.g8(g::LOCATION);
        if loc < 0x21 || loc >= 0x80 { crate::engine::view::refresh_map_view(gm); }
    }
    if gm.g16(g::KEY_FROM_KEYPAD) == 0 {
        if key < 5 { key += 0xfa; }
    } else {
        key = match key as u8 { b'1' => 0xd4, b'2' => 4, b'3' => 0xd6, b'4' => 1, b'6' => 2, b'7' => 0xd3, b'8' => 3, b'9' => 0xd5, _ => key as u8 } as i16;
    }
    key
}
