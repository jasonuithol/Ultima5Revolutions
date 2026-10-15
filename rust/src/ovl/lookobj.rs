//! LOOKOBJ.OVL (translated)
//!
//! The Look command for the surface and settlements (level-2 overlay, 1000:a290-1000:b45b) and the
//! gem-map ("View a gem" / "Strange vision") renderer. `cmd_look` (vector 1778:07a6, ROOT
//! dispatch_command 'L') asks a direction, then: a mirror (0x29) gives a death/strange vision;
//! an object from the monster table is described through LOOK2.DAT entry 0x100+tile
//! (`print_object_description`); sign tiles 0x89/0x8a/0xa0/0xa4/0xf8 are read from SIGNS.DAT
//! with the runic font (`look_sign` / `print_sign_text`); every other terrain tile goes through
//! `look_terrain` (LOOK2.DAT text plus specials: telescope 0x59 = sun/night sky with the eight
//! planets and the Shadowlords' marks, wishing well 0xa1, fountains 0xd8-0xdb, clocks 0xfa/0xfb,
//! the shrine/castle plaque 0xde, dungeon entrance signs 0xdf, big-object parts 0xe0-0xe2 chase
//! to their base tile). `view_gem_map` (vector 1778:078e, also CAST) draws the 32x32 map as 4x4
//! pixel cells on page 1 using the tile class table g_tile_class_1d1a (0x1d1a) and 16 cell
//! painters, copies it to page 0 and blinks the party cell until a key is pressed.
//!
//! Data: LOOK2.DAT (0x200 u16 offsets then NUL-terminated strings, 0x80 bytes read per look),
//! SIGNS.DAT (33 u16 offsets by location, then 4-byte header {location, z, x, y} + text + NUL;
//! read into g_ool_buffer 0xb21e, pre-filled with 0xff), DGROUP tables 0x3750 (planet start
//! orbit index), 0x3758 (planet sky row), 0x3760 (8 x 22 visibility bytes per orbit position),
//! 0x37be (pointers to the 9 sign box-drawing rows for codes 0x29..0x31), 0x3812/0x3822 (gem
//! cell pixel masks), 0x50ca glyph widths (FONT), g_view_tiles 0xab02 (stride 0x20).
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// orig 1000:a290 (66 bytes). Prints the LOOK2.DAT description of terrain tile `tile`: reads the
/// u16 at file offset tile*2, then up to 0x80 bytes of text at that offset and print_string()s
/// them (the text is NUL-terminated inside the file).
pub fn print_look_description(gm: &mut Game, param_1: i16) {
    let tile = param_1;
    gm.with_buf(2, |gm, off_buf| {
        gm.set_g16(off_buf, 0);
        files::load_file_region(gm, (tile << 1) as u16, 2, off_buf, 0x71f8); // "LOOK2.DAT"
        let off = gm.g16(off_buf);
        gm.with_buf(0x80, |gm, txt| {
            files::load_file_region(gm, off, 0x80, txt, 0x7202); // "LOOK2.DAT"
            text::print_string(gm, txt);
        });
    });
}

/// orig 1000:a2d2 (283 bytes). The wishing well (tile 0xa1): "a well.\n\nDrop a coin?" Y/N. N
/// prints "No"; Y prints "Yes" and, with gold, "Thy wish?" and reads up to 12 characters (one
/// gold is spent). An empty wish prints "Nothing"; a wish containing Corvette/Ferrari/
/// Lamborghini/Lotus/Porsche/Horse in Paws (0x16) or Empath Abbey (0x1f) prints "Poof!", plays
/// tones and spawns a horse (tile 0x10) at x+1 on the party's floor; elsewhere "No effect...".
/// Ghidra order: (z, y, x) = the party position.
pub fn look_well(gm: &mut Game, param_1: u8, param_2: u8, param_3: u8) {
    let (z, y, x) = (param_1, param_2, param_3);
    text::print_string(gm, 0x720c); // "a well.\n\nDrop a coin?"
    let mut c: i16;
    loop {
        c = input::read_command_key(gm);
        if c == b'Y' as i16 { break; }
        if c == b'N' as i16 { break; }
    }
    let msg: u16;
    if c == b'N' as i16 {
        msg = 0x7222; // "No\n"
    } else {
        text::print_string(gm, 0x7226); // "Yes\n"
        if gm.g16(g::GOLD) == 0 { return; }
        text::print_string(gm, 0x722c); // "\nThy wish?\n"
        let gold = gm.g16(g::GOLD).wrapping_sub(1);
        gm.set_g16(g::GOLD, gold);
        msg = gm.with_buf(14, |gm, wish| {
            rest::read_text_line(gm, 0xc, wish);
            if gm.g8(wish) == 0 {
                0x7238 // "Nothing\n"
            } else {
                let hay = wish as i16;
                let found = (rest::str_find_ci(gm, hay, 0x7242) < 0x8000) // "Corvette"
                    || (rest::str_find_ci(gm, hay, 0x724c) < 0x8000) // "Ferrari"
                    || (rest::str_find_ci(gm, hay, 0x7254) < 0x8000) // "Lamborghini"
                    || (rest::str_find_ci(gm, hay, 0x7260) < 0x8000) // "Lotus"
                    || (rest::str_find_ci(gm, hay, 0x7266) < 0x8000) // "Porsche"
                    || (-1 < rest::str_find_ci(gm, hay, 0x726e) as i16); // "Horse"
                if found {
                    let loc = gm.gi8(g::LOCATION);
                    if loc == 0x16 || loc == 0x1f {
                        text::print_string(gm, 0x7284); // "\nPoof!\n"
                        util::play_random_tones(gm, 2000, 3000, 10);
                        let slot = rest::find_spawn_slot(gm);
                        rest::set_monster_slot(gm, slot, 0, z, y, x.wrapping_add(1), 0x10, 0x10);
                        return 0;
                    }
                    0x7274 // "\nNo effect...\n"
                } else {
                    0x728c // "\nNo effect...\n"
                }
            }
        });
        if msg == 0 { return; }
    }
    text::print_string(gm, msg);
}

/// orig 1000:a3f2 (72 bytes). A fountain (tiles 0xd8-0xdb): "a gurgling fountain!\n\n" then
/// "Who will drink?" with select_party_member_no_zero; -1 prints "None!", a dead ('D') or
/// sleeping ('S') member "Incapacitated!", otherwise "Refreshing..." (no game effect).
pub fn look_fountain(gm: &mut Game) {
    text::print_string(gm, 0x729c); // "a gurgling fountain!\n\n"
    text::print_string(gm, 0x72b4); // "Who will drink?\n"
    let m = rest::select_party_member_no_zero(gm);
    let msg: u16 = if m == -1 {
        0x72c6 // "None!\n"
    } else {
        let st = gm.g8(g::PARTY_STATUS.wrapping_add((m as u16).wrapping_mul(0x20)));
        if st == b'D' || st == b'S' {
            0x72ce // "Incapacitated!\n\n"
        } else {
            0x72e0 // "Refreshing...\n"
        }
    };
    text::print_string(gm, msg);
}

/// orig 1000:a43c (159 bytes). Draws a planet in the telescope's night sky in g_ui_color1 at
/// view cell (row, col): a 3x3 block at pixels (col*8+7.., row*8+7..) plus one pixel above
/// (row*8+6, col*8+8) and one below (row*8+10) when they fall inside the view (x < 0xb1,
/// row*8 < 0xae). Ghidra order: (row, col).
pub fn draw_sky_planet(gm: &mut Game, param_1: i16, param_2: i16) {
    let (row, col) = (param_1, param_2);
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    let x0 = col.wrapping_mul(8);
    gfx::plot_pixel(gm, row.wrapping_mul(8).wrapping_add(8), x0.wrapping_add(6));
    let mut dy: i16 = 0;
    loop {
        let mut dx: i16 = 0;
        loop {
            if x0.wrapping_add(dx) < 0xb1 {
                gfx::plot_pixel(gm, row.wrapping_mul(8).wrapping_add(dy).wrapping_add(7), x0.wrapping_add(dx).wrapping_add(7));
            }
            dx += 1;
            if !(dx < 3) { break; }
        }
        dy += 1;
        if !(dy < 3) { break; }
    }
    if col.wrapping_mul(8) < 0xae {
        gfx::plot_pixel(gm, row.wrapping_mul(8).wrapping_add(8), col.wrapping_mul(8).wrapping_add(10));
    }
}

/// orig 1000:a4dc (282 bytes). Draws the Shadowlord crescent in g_ui_color0 to the right of a
/// planet at view cell (row, col): eight short vertical lines at x = col*8+5 .. col*8+12 whose
/// y extents (row*8+5..12) form an arc; each line is skipped when its x would leave the view
/// (x*8 < 0xb0 - n). Ghidra order: (row, col).
pub fn draw_sky_crescent(gm: &mut Game, param_1: i16, param_2: i16) {
    let x = param_2.wrapping_mul(8);
    let y = param_1 << 3;
    let c0 = gm.gi16(g::UI_COLOR0);
    gfx::set_color(gm, c0);
    if 2 < x {
        gfx::draw_vline(gm, y.wrapping_add(0xc), y.wrapping_add(10), x.wrapping_add(5));
        gfx::draw_vline(gm, y.wrapping_add(0xc), y.wrapping_add(10), x.wrapping_add(6));
        gfx::draw_vline(gm, y.wrapping_add(0xc), y.wrapping_add(8), x.wrapping_add(7));
    }
    if x < 0xb0 { gfx::draw_vline(gm, y.wrapping_add(0xc), y.wrapping_add(8), x.wrapping_add(8)); }
    if x < 0xaf { gfx::draw_vline(gm, y.wrapping_add(10), y.wrapping_add(6), x.wrapping_add(9)); }
    if x < 0xae { gfx::draw_vline(gm, y.wrapping_add(10), y.wrapping_add(6), x.wrapping_add(10)); }
    if x < 0xad { gfx::draw_vline(gm, y.wrapping_add(8), y.wrapping_add(5), x.wrapping_add(0xb)); }
    if x < 0xac { gfx::draw_vline(gm, y.wrapping_add(7), y.wrapping_add(5), x.wrapping_add(0xc)); }
}

/// orig 1000:a5f6 (410 bytes). Vector 1778:079a (also CAST). The telescope (tile 0x59). By day
/// (hour 6..17): "the sun!", the active member (or the first conscious one) takes 1 damage and
/// the status window is redrawn. By night: every view cell is blanked (0xff, stride 0x20), the
/// centre cell 0xac47 shows the telescope (0x59), draw_map_view, 80 random stars in
/// g_ui_color2+8 (y 9..0xac, x 9..0xb6), then the eight planets: each starts at orbit index
/// 0x3750[p] and is stepped back one valid position (0x3760[p][i] != 0, 22 positions) for every
/// day between the current date and 5 April (year%100 == 0x27), planet p drawn with
/// `draw_sky_planet(row 0x3758[p], index+1)`; a Shadowlord whose town number (g_shadowlord_loc)
/// is p+1 gets `draw_sky_crescent`. "the night sky! " then waits for a key.
pub fn look_telescope(gm: &mut Game) {
    let hour = gm.g8(g::HOUR);
    if hour < 6 || 0x11 < hour {
        gm.set_g8(g::VIEW_DIRTY, 1);
        let mut row: u16 = 0;
        loop {
            let mut p = g::VIEW_TILES.wrapping_add(row);
            for _ in 0..5 { gm.set_g16(p, 0xffff); p = p.wrapping_add(2); }
            gm.set_g8(p, 0xff);
            row = row.wrapping_add(0x20);
            if !(row < 0x160) { break; }
        }
        gm.set_g8(0xac47, 0x59); // g_view_tiles[5][5]: the telescope at the centre
        rest::draw_map_view(gm);
        let c = gm.gi16(g::UI_COLOR2).wrapping_add(8);
        gfx::set_color(gm, c);
        let mut n: i16 = 0x50;
        loop {
            let sx = util::rand_range(gm, 0xb6, 9);
            let sy = util::rand_range(gm, 0xac, 9);
            gfx::plot_pixel(gm, sy, sx);
            n -= 1;
            if n == 0 { break; }
        }
        let mut planet: i16 = 0;
        let mut vis_row: u16 = 0;
        loop {
            let mut yr: i16 = (gm.gi16(g::YEAR)) % 100;
            let mut mon: u16 = gm.g8(g::MONTH) as u16;
            let mut day: u16 = gm.g8(g::DAY) as u16;
            let mut pos: u16 = gm.g8(0x3750u16.wrapping_add(planet as u16)) as u16; // planet start orbit index
            while (0x27 < yr) || (yr == 0x27 && 4 < mon as i16) || (yr == 0x27 && mon == 4 && 5 < day as i16) {
                day = day.wrapping_sub(1);
                if day == 0 {
                    day = 0x1c;
                    mon = mon.wrapping_sub(1);
                }
                if mon == 0 {
                    mon = 0xd;
                    yr = yr.wrapping_sub(1);
                }
                loop {
                    pos = pos.wrapping_sub(1);
                    if (pos as i16) < 0 { pos = 0x15; }
                    if gm.gi8(0x3760u16.wrapping_add(vis_row).wrapping_add(pos)) != 0 { break; } // orbit visibility table
                }
            }
            let row = gm.g8(0x3758u16.wrapping_add(planet as u16)) as i16; // planet sky row
            draw_sky_planet(gm, row, (pos as i16).wrapping_add(1));
            let mut k: u16 = 0;
            loop {
                if gm.g8(g::SHADOWLORD_LOC.wrapping_add(k)) as u16 == (planet as u16).wrapping_add(1) {
                    draw_sky_crescent(gm, row, pos as i16);
                }
                k += 1;
                if !(k < 3) { break; }
            }
            vis_row = vis_row.wrapping_add(0x16);
            planet += 1;
            if !(planet < 8) { break; }
        }
        text::print_string(gm, 0x72fa); // "the night sky! "
        loop {
            let k = input::poll_key_with_cursor(gm);
            if k as i8 != 0 { break; }
        }
    } else {
        text::print_string(gm, 0x72f0); // "the sun!\n"
        if gm.gi8(g::ACTIVE_MEMBER) == -1 && rest::find_first_conscious_member(gm) == 0 {
            let v = gm.g16(g::DX) as u8;
            gm.set_g8(g::ACTIVE_MEMBER, v);
        }
        let m = gm.gi8(g::ACTIVE_MEMBER) as i16 as u16;
        rest::damage_party_member(gm, 1, m);
        rest::draw_status_window(gm);
    }
}

/// orig 1000:a792 (389 bytes, continues through the a878..a934 fragments Ghidra split off).
/// Looks at terrain tile `tile` at map (y, x). Big-object parts 0xe0/0xe1/0xe2 move the lookup
/// one cell up / right / left until a base tile is reached. Then: 0x59 `look_telescope`; 0xa1
/// `look_well(party z, y, x)`; 0xd8-0xdb `look_fountain`; otherwise the LOOK2.DAT text and a
/// suffix: clocks 0xfa/0xfb print "h:mm AM./PM." (12-hour, hour 0 -> 12); 0xde prints
/// "Truth"/"Love"/"Courage" in the Lycaeum (0x1e)/Empath Abbey (0x1f)/Serpent's Hold (0x20);
/// 0xdf names the dungeon from the x coordinate (0x7e Wrong, 0x3a Shame, 0x48 Destard, 0x5b
/// Despise, 0x80 Doom, 0x9c Covetous, 0xef Hythloth, 0xf0 Deceit); any other tile ends the line
/// with "\n" when the cursor is not at column 0. Ghidra order: (y, x, tile).
pub fn look_terrain(gm: &mut Game, param_1: i16, param_2: i16, param_3: u16) {
    let (mut y, mut x, mut tile) = (param_1, param_2, param_3);
    while tile == 0xe0 || tile == 0xe1 || tile == 0xe2 {
        if tile == 0xe0 {
            y = y.wrapping_sub(1);
        } else if tile == 0xe1 {
            x = x.wrapping_add(1);
        } else if tile == 0xe2 {
            x = x.wrapping_sub(1);
        }
        let p = rest::get_map_tile(gm, y as u16, x as u16);
        tile = gm.g8(p) as u16;
    }
    if tile == 0x59 {
        look_telescope(gm);
    } else if tile == 0xa1 {
        let (z, py, px) = (gm.g8(g::PARTY_Z), gm.g8(g::PARTY_Y), gm.g8(g::PARTY_X));
        look_well(gm, z, py, px);
    } else if (tile as u8 & 0xfc) == 0xd8 {
        look_fountain(gm);
    } else {
        print_look_description(gm, tile as i16);
        let msg: u16;
        if (tile as u8 & 0xfe) == 0xfa {
            let mut h = gm.g8(g::HOUR) % 0xc;
            if h == 0 { h = 0xc; }
            text::print_number(gm, 0x20, 1, h as i16);
            text::put_char(gm, 0x3a); // ':'
            let m = gm.g8(g::MINUTE) as i16;
            text::print_number(gm, 0x30, 2, m);
            msg = if gm.g8(g::HOUR) < 0xc { 0x7310 } else { 0x730a }; // " AM.\n" / " PM.\n"
        } else if tile == 0xde {
            let loc = gm.gi8(g::LOCATION);
            if loc == 0x1e {
                msg = 0x7316; // "Truth\n"
            } else if loc == 0x1f {
                msg = 0x731e; // "Love\n"
            } else {
                if loc != 0x20 { return; }
                msg = 0x7324; // "Courage\n"
            }
        } else {
            if tile != 0xdf {
                if text::get_cursor_col(gm) == 0 { return; }
                text::put_char(gm, 10);
                return;
            }
            if x == 0x7e {
                msg = 0x734a; // "Wrong\n"
            } else if x < 0x7f {
                if x == 0x3a {
                    msg = 0x735c; // "Shame\n"
                } else if x == 0x48 {
                    msg = 0x7340; // "Destard\n"
                } else {
                    if x != 0x5b { return; }
                    msg = 0x7336; // "Despise\n"
                }
            } else if x == 0x80 {
                msg = 0x736e; // "Doom\n"
            } else if x == 0x9c {
                msg = 0x7352; // "Covetous\n"
            } else if x == 0xef {
                msg = 0x7364; // "Hythloth\n"
            } else {
                if x != 0xf0 { return; }
                msg = 0x732e; // "Deceit\n"
            }
        }
        text::print_string(gm, msg);
    }
}

/// orig 1000:a8d8 (13 bytes). Not a function: the "Deceit" arm (x == 0xf0) of `look_terrain`'s
/// dungeon-sign switch, split off by Ghidra (the ENDGAME vector 1778:04ca that the catalog lists
/// targets the same address inside ENDGAME.OVL). Its code lives in `look_terrain`.
pub fn fn_1000_a8d8(gm: &mut Game) {}

/// orig 1000:a934 (83 bytes). Prints the LOOK2.DAT description of object tile `tile` (0x100 +
/// tile: u16 offset at file offset tile*2 + 0x200, built with the CONCAT11(hi+2, lo) trick),
/// reading 0x80 bytes of text, then ends the line with "\n" if the cursor is not at column 0.
pub fn print_object_description(gm: &mut Game, param_1: i16) {
    let tile = param_1;
    let t2 = (tile << 1) as u16;
    let off_in_file = (((t2 >> 8) as u8).wrapping_add(2) as u16) << 8 | (t2 & 0xff);
    gm.with_buf(2, |gm, off_buf| {
        gm.set_g16(off_buf, 0);
        files::load_file_region(gm, off_in_file, 2, off_buf, 0x7374); // "LOOK2.DAT"
        let off = gm.g16(off_buf);
        gm.with_buf(0x80, |gm, txt| {
            files::load_file_region(gm, off, 0x80, txt, 0x737e); // "LOOK2.DAT"
            text::print_string(gm, txt);
        });
    });
    if text::get_cursor_col(gm) != 0 {
        text::put_char(gm, 10);
    }
}

/// orig 1000:a988 (235 bytes). Prints one sign. `offset` == -1 prints the default "LIVE BY THE
/// EIGHT LAWS" box (strings 0x742a/0x745e) in font 1 (runes). Otherwise `offset` indexes the
/// sign header inside g_ool_buffer: the text starts at +4, a sign consisting of just "\n" is
/// skipped (+6 = its NUL and the next header). Each byte selects font 1 when bit 7 is clear,
/// font 0 when set; 0x26/0x27 print glyph 0x6c; 0x29..0x31 print the box-drawing row string
/// 0x37be[code] in font 1; 0x0d switches to font 0 and waits for a key (page break); anything
/// else prints the byte & 0x7f. Stops at the byte before a NUL, restores font 0 and ends the
/// line if the cursor is not at column 0.
pub fn print_sign_text(gm: &mut Game, param_1: i16) {
    let mut p = param_1;
    if p == -1 {
        text::select_font(gm, 1);
        text::print_string(gm, 0x742a); // "abbbbbbbbbbbbbbc\ng              g\ng  LIVE@BY@[E  g\n"
        text::print_string(gm, 0x745e); // "g  EIGHT@LAWS  g\ng              g\ndeeeeeeeeeeeeeef\n"
        text::select_font(gm, 0);
    } else {
        p = p.wrapping_add(4);
        while gm.g8(g::OOL_BUFFER.wrapping_add(p as u16)) == b'\n' {
            p = p.wrapping_add(6);
        }
        loop {
            let mut b = gm.g8(g::OOL_BUFFER.wrapping_add(p as u16));
            text::select_font(gm, if (b & 0x80) == 0 { 1 } else { 0 });
            if b == 0x26 || b == 0x27 {
                b = 0x6c;
                text::put_char(gm, b);
            } else if b < 0x29 || 0x31 < b {
                if b != 0xd {
                    b &= 0x7f;
                    text::put_char(gm, b);
                } else {
                    text::select_font(gm, 0);
                    input::read_command_key(gm);
                }
            } else {
                text::select_font(gm, 1);
                let mut s = gm.g16(0x37beu16.wrapping_add((b as u16).wrapping_mul(2))); // box-drawing row strings
                while gm.g8(s) != 0 {
                    let c = gm.g8(s);
                    text::put_char(gm, c);
                    s = s.wrapping_add(1);
                }
            }
            let next = gm.g8(0xb21fu16.wrapping_add(p as u16)); // g_ool_buffer[p + 1]
            p = p.wrapping_add(1);
            if next == 0 { break; }
        }
        text::select_font(gm, 0);
        if text::get_cursor_col(gm) != 0 {
            text::put_char(gm, 10);
        }
    }
}

/// orig 1000:aa74 (434 bytes). Reads the sign at map (y, x) on floor z. The wanted poster in
/// Yew (location 4, z 0, x 0x11, y 0x15) is composed on the fly: a runic box with "Wanted:",
/// the names of the first three party members (g_party_count permitting) centred at column
/// 7-len/2, and "Dead or Alive". Otherwise the 33 u16 offsets of signs.dat are read, the
/// location's sign group (offset 0 = none -> the Eight Laws default) is loaded over g_ool_buffer
/// (pre-filled with 2000 bytes of 0xff) and scanned header by header ({loc, z, x, y} then NUL-
/// terminated text) for a matching z/x/y; a match is printed with `print_sign_text`, reaching a
/// 0xff header byte prints the default instead. Ghidra order: (y, x, z).
pub fn look_sign(gm: &mut Game, param_1: i8, param_2: i8, param_3: i8) {
    let (y, x, z) = (param_1, param_2, param_3);
    if gm.gi8(g::LOCATION) == 4 && z == 0 && x == 0x11 && y == 0x15 {
        text::select_font(gm, 1);
        text::print_string(gm, 0x7492); // "abbbbbbbbbbbbbc\ng   "
        text::select_font(gm, 0);
        text::print_string(gm, 0x74a8); // "Wanted:   "
        text::select_font(gm, 1);
        text::print_string(gm, 0x74b4); // "g\ng"
        let r = text::get_cursor_row(gm);
        text::set_cursor_pos(gm, r, 0xe);
        text::print_string(gm, 0x74b8); // "g\ng"
        let mut i: u16 = 0;
        let mut rec: u16 = 0x55a8; // g_party[0].name
        loop {
            if i < gm.g8(g::PARTY_COUNT) as u16 {
                text::select_font(gm, 0);
                let len = util::str_len(gm, gm.ds(rec));
                let col = -((len / 2) + -7);
                let r = text::get_cursor_row(gm);
                text::set_cursor_pos(gm, r, col);
                text::print_string(gm, rec);
                text::select_font(gm, 1);
            }
            let r = text::get_cursor_row(gm);
            text::set_cursor_pos(gm, r, 0xe);
            text::print_string(gm, 0x74bc); // "g\ng"
            rec = rec.wrapping_add(0x20);
            i += 1;
            if !((i as i16) < 3) { break; }
        }
        let r = text::get_cursor_row(gm);
        text::set_cursor_pos(gm, r, 0xe);
        text::print_string(gm, 0x74c0); // "g\ng"
        text::select_font(gm, 0);
        text::print_string(gm, 0x74c4); // "Dead or Alive"
        text::select_font(gm, 1);
        text::print_string(gm, 0x74d2); // "g\ndeeeeeeeeeeeeef\n"
        text::select_font(gm, 0);
    } else {
        let group_off = gm.with_buf(0x42, |gm, offs| {
            files::load_file_region(gm, 0, 0x42, offs, 0x74e6); // "signs.dat"
            gm.g16(offs.wrapping_add((gm.g8(g::LOCATION) as u16).wrapping_mul(2)))
        });
        gm.m.fill(gm.ds(g::OOL_BUFFER), 2000, 0xff);
        if group_off == 0 {
            print_sign_text(gm, -1);
        } else {
            files::load_file_region(gm, group_off, 2000, g::OOL_BUFFER, 0x74f0); // "signs.dat"
            let mut p: u16 = 0;
            loop {
                let done: u16;
                if gm.gi8(0xb21fu16.wrapping_add(p)) == z
                    && gm.gi8(0xb220u16.wrapping_add(p)) == x
                    && gm.gi8(0xb221u16.wrapping_add(p)) == y
                {
                    print_sign_text(gm, p as i16);
                    done = 1;
                } else {
                    p = p.wrapping_add(4);
                    let mut q;
                    loop {
                        q = p;
                        p = q.wrapping_add(1);
                        if gm.g8(g::OOL_BUFFER.wrapping_add(q)) == 0 { break; }
                    }
                    done = (gm.g8(0xb21fu16.wrapping_add(q)) == 0xff) as u16;
                    if done != 0 {
                        print_sign_text(gm, -1);
                    }
                }
                if done != 0 { break; }
            }
        }
    }
}

/// orig 1000:ac2c (253 bytes). Vector 1778:07a6: the Look command (ROOT dispatch_command 'L'
/// outside dungeons). Asks a direction (0 = cancelled); the target cell is the party position
/// plus g_dx/g_dy. A mirror (tile 0x29): choose a member; if rand(1..30) >= the member's
/// intelligence "Death vision!" (1 damage, status redraw) else "Strange vision!" and the gem map
/// of the party position. Otherwise "\nThou dost see\n": an object in the monster table is
/// described by `print_object_description`; sign tiles 0x89, 0x8a, 0xa0, 0xa4 and 0xf8 print
/// "\n" and `look_sign`; everything else goes to `look_terrain`.
pub fn cmd_look(gm: &mut Game) {
    if rest::get_direction(gm) == 0 { return; }
    let x = (gm.g8(g::PARTY_X) as u16).wrapping_add(gm.g16(g::DX)) as i16;
    let y = (gm.g8(g::PARTY_Y) as u16).wrapping_add(gm.g16(g::DY)) as i16;
    let p = rest::get_map_tile(gm, y as u16, x as u16);
    let tile = gm.g8(p);
    let z = gm.g8(g::PARTY_Z) as u16;
    let obj = rest::find_object_at(gm, z, y as u16, x as u16) as i16;
    if tile == 0x29 {
        let m = rest::select_target_member(gm);
        if m == 0xffff { return; }
        let r = util::rand_range(gm, 0x1e, 1) as u16;
        if (gm.g8(m.wrapping_mul(0x20).wrapping_add(0x55b6)) as u16) <= r { // g_party[m].intelligence
            text::print_string(gm, 0x74fa); // "Death vision!\n"
            rest::damage_party_member(gm, 1, m);
            rest::draw_status_window(gm);
            return;
        }
        text::print_string(gm, 0x750a); // "Strange vision!\n"
        let (py, px) = (gm.gi8(g::PARTY_Y), gm.gi8(g::PARTY_X));
        view_gem_map(gm, py, px);
        return;
    }
    text::print_string(gm, 0x751c); // "\nThou dost see\n"
    if obj != 0 {
        print_object_description(gm, obj);
        return;
    }
    let is_sign = if tile != 0xa0 {
        if tile < 0xa1 {
            !(tile < 0x89 || 0x8a < tile)
        } else {
            !(tile != 0xa4 && tile != 0xf8)
        }
    } else {
        true
    };
    if !is_sign {
        look_terrain(gm, y, x, tile as u16);
        return;
    }
    text::print_string(gm, 0x752c); // "\n"
    let z = gm.gi8(g::PARTY_Z);
    look_sign(gm, y as i8, x as i8, z);
}

/// orig 1000:ad2c (33 bytes). Sets g_dx/g_dy to the top-left pixel of gem-map cell (row, col):
/// x = col*4 + 0x20, y = row*4 + 0x20. Ghidra order: (row, col).
pub fn set_gem_cell_origin(gm: &mut Game, param_1: i16, param_2: i16) {
    gm.set_gi16(g::DX, param_2.wrapping_mul(4).wrapping_add(0x20));
    gm.set_gi16(g::DY, param_1.wrapping_mul(4).wrapping_add(0x20));
}

#[inline] fn gx(gm: &Game) -> i16 { gm.gi16(g::DX) }
#[inline] fn gy(gm: &Game) -> i16 { gm.gi16(g::DY) }

/// orig 1000:ad4e (70 bytes). Gem cell class 1: four pixels in g_ui_color3+8 at (+0,+1),
/// (+2,+1), (+1,+3), (+3,+3) (y, x offsets from g_dy/g_dx).
pub fn gem_draw_speckles(gm: &mut Game) {
    let c = gm.gi16(g::UI_COLOR3).wrapping_add(8);
    gfx::set_color(gm, c);
    let (y, x) = (gy(gm), gx(gm));
    gfx::plot_pixel(gm, y, x + 1);
    gfx::plot_pixel(gm, y + 2, x + 1);
    gfx::plot_pixel(gm, y + 1, x + 3);
    gfx::plot_pixel(gm, y + 3, x + 3);
}

/// orig 1000:ad94 (36 bytes; the bytes adb8..adef after it are an unreferenced fragment). Gem
/// cell class 2: the whole 4x4 cell filled with g_ui_color3+8.
pub fn gem_fill_color3(gm: &mut Game) {
    let c = gm.gi16(g::UI_COLOR3).wrapping_add(8);
    gfx::set_color(gm, c);
    let (y, x) = (gy(gm), gx(gm));
    gfx::fill_rect(gm, y + 3, x + 3, y, x);
}

/// orig 1000:adf0 (55 bytes). Gem cell class 4: two horizontal g_ui_color1 lines on rows +0
/// and +3.
pub fn gem_draw_top_bottom(gm: &mut Game) {
    let c = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c);
    let (y, x) = (gy(gm), gx(gm));
    gfx::draw_line(gm, y, x + 3, y, x);
    gfx::draw_line(gm, y + 3, x + 3, y + 3, x);
}

/// orig 1000:ae28 (56 bytes). Gem cell class 5: the inner 2x2 block in g_ui_color1 (rows +1
/// and +2, columns +1..+2).
pub fn gem_draw_centre(gm: &mut Game) {
    let c = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c);
    let (y, x) = (gy(gm), gx(gm));
    gfx::draw_line(gm, y + 1, x + 2, y + 1, x + 1);
    gfx::draw_line(gm, y + 2, x + 2, y + 2, x + 1);
}

/// orig 1000:ae60 (102 bytes). Gem cell class 6: a g_ui_color1 frame around the cell (rows +0
/// and +3 full, columns +0 and +3 on rows +1..+2).
pub fn gem_draw_frame(gm: &mut Game) {
    let c = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c);
    let (y, x) = (gy(gm), gx(gm));
    gfx::draw_line(gm, y, x + 3, y, x);
    gfx::draw_line(gm, y + 3, x + 3, y + 3, x);
    gfx::draw_line(gm, y + 2, x, y + 1, x);
    gfx::draw_line(gm, y + 2, x + 3, y + 1, x + 3);
}

/// orig 1000:aec6 (101 bytes). Gem cell class 8: two 2x2 blocks in g_ui_color_sun, top-left
/// (rows +0..1, columns +0..1) and bottom-right (rows +2..3, columns +2..3).
pub fn gem_draw_sun_checker(gm: &mut Game) {
    let c = gm.gi16(g::UI_COLOR_SUN);
    gfx::set_color(gm, c);
    let (y, x) = (gy(gm), gx(gm));
    gfx::draw_line(gm, y, x + 1, y, x);
    gfx::draw_line(gm, y + 1, x + 1, y + 1, x);
    gfx::draw_line(gm, y + 2, x + 3, y + 2, x + 2);
    gfx::draw_line(gm, y + 3, x + 3, y + 3, x + 2);
}

/// orig 1000:af2c (87 bytes). Gem cell class 9: in g_ui_color3+8 full rows +0 and +2 plus
/// pixels (+1,+2) and (+3,+0).
pub fn gem_draw_stripes(gm: &mut Game) {
    let c = gm.gi16(g::UI_COLOR3).wrapping_add(8);
    gfx::set_color(gm, c);
    let (y, x) = (gy(gm), gx(gm));
    gfx::draw_line(gm, y, x + 3, y, x);
    gfx::draw_line(gm, y + 2, x + 3, y + 2, x);
    gfx::plot_pixel(gm, y + 1, x + 2);
    gfx::plot_pixel(gm, y + 3, x);
}

/// orig 1000:af84 (228 bytes). Gem cell class 10 (water): four pixels at (+0,+1), (+1,+3),
/// (+2,+1), (+3,+3), each coloured by its bit (8,4,2,1) of the nibble table 0x3822[tile & 0xf]:
/// for tiles 0x60-0x6f a clear bit gives g_ui_color3+8, otherwise g_ui_color4 on CGA/Hercules
/// (adapter 0/3) or g_ui_color2+8 on EGA/Tandy. Ghidra order: (tile).
pub fn gem_draw_water(gm: &mut Game, param_1: u16) {
    let tile = param_1;
    let adapter = gm.gi16(g::ADAPTER_TYPE);
    let c2 = gm.gi16(g::UI_COLOR2).wrapping_add(8);
    let c3 = gm.gi16(g::UI_COLOR3).wrapping_add(8);
    let c4 = gm.gi16(g::UI_COLOR4);
    let bits = gm.g8(0x3822u16.wrapping_add(tile & 0xf)); // gem water pixel nibbles
    let mut colors = [0i16; 4];
    let mut mask: u16 = 8;
    for slot in colors.iter_mut() {
        if (tile as u8 & 0xf0) == 0x60 && (bits as u16 & mask) == 0 {
            *slot = c3;
        } else if adapter == 0 || adapter == 3 {
            *slot = c4;
        } else {
            *slot = c2;
        }
        mask = ((mask as i16) >> 1) as u16;
    }
    let (y, x) = (gy(gm), gx(gm));
    gfx::set_color(gm, colors[0]);
    gfx::plot_pixel(gm, y, x + 1);
    gfx::set_color(gm, colors[1]);
    gfx::plot_pixel(gm, y + 1, x + 3);
    gfx::set_color(gm, colors[2]);
    gfx::plot_pixel(gm, y + 2, x + 1);
    gfx::set_color(gm, colors[3]);
    gfx::plot_pixel(gm, y + 3, x + 3);
}

/// Colour used by the gem painters for "deep" cells: g_ui_color4 on CGA/Hercules, else
/// g_ui_color2+8.
fn gem_deep_color(gm: &Game) -> i16 {
    let a = gm.gi16(g::ADAPTER_TYPE);
    if a != 0 && a != 3 { gm.gi16(g::UI_COLOR2).wrapping_add(8) } else { gm.gi16(g::UI_COLOR4) }
}

/// orig 1000:b06a (59 bytes). Gem cell class 11: pixels (+0,+0) and (+2,+2) in the deep colour
/// (g_ui_color4 on adapter 0/3, else g_ui_color2+8).
pub fn gem_draw_deep_dots(gm: &mut Game) {
    let c = gem_deep_color(gm);
    gfx::set_color(gm, c);
    let (y, x) = (gy(gm), gx(gm));
    gfx::plot_pixel(gm, y, x);
    gfx::plot_pixel(gm, y + 2, x + 2);
}

/// orig 1000:b0a6 (99 bytes). Gem cell class 13: pixels (+0,+1), (+1,+3) in g_ui_color3+8 and
/// (+2,+0), (+3,+2) in the deep colour.
pub fn gem_draw_mixed_dots(gm: &mut Game) {
    let c3 = gm.gi16(g::UI_COLOR3).wrapping_add(8);
    gfx::set_color(gm, c3);
    let (y, x) = (gy(gm), gx(gm));
    gfx::plot_pixel(gm, y, x + 1);
    gfx::plot_pixel(gm, y + 1, x + 3);
    let c = gem_deep_color(gm);
    gfx::set_color(gm, c);
    gfx::plot_pixel(gm, y + 2, x);
    gfx::plot_pixel(gm, y + 3, x + 2);
}

/// orig 1000:b10a (256 bytes). Gem cell class 16 (walls/buildings, tiles 0x22-0x28): speckles,
/// then the inner 2x2 in g_ui_color0 and, per bit 8/4/2/1 of 0x3812[tile], a 2-pixel edge on
/// the top / right / bottom / left side; tiles 0x22-0x25 finally get a black pixel at (+2,+1),
/// (+1,+1), (+1,+2), (+2,+2) respectively (the doorway). Ghidra order: (tile).
pub fn gem_draw_building(gm: &mut Game, param_1: i16) {
    let tile = param_1;
    gem_draw_speckles(gm);
    let c0 = gm.gi16(g::UI_COLOR0);
    gfx::set_color(gm, c0);
    let (y, x) = (gy(gm), gx(gm));
    gfx::fill_rect(gm, y + 2, x + 2, y + 1, x + 1);
    let bits = gm.g8(0x3812u16.wrapping_add(tile as u16)); // building edge bits
    if (bits & 8) != 0 { gfx::draw_line(gm, y, x + 2, y, x + 1); }
    if (bits & 4) != 0 { gfx::draw_line(gm, y + 2, x + 3, y + 1, x + 3); }
    if (bits & 2) != 0 { gfx::draw_line(gm, y + 3, x + 2, y + 3, x + 1); }
    if (bits & 1) != 0 { gfx::draw_line(gm, y + 2, x, y + 1, x); }
    gfx::set_color(gm, 0);
    let (py, px);
    if tile == 0x22 {
        px = x + 1;
        py = y + 2;
    } else if tile == 0x23 {
        px = x + 1;
        py = y + 1;
    } else if tile == 0x24 {
        px = x + 2;
        py = y + 1;
    } else if tile == 0x25 {
        px = x + 2;
        py = y + 2;
    } else {
        return;
    }
    gfx::plot_pixel(gm, py, px);
}

/// orig 1000:b20e (359 bytes). Paints one gem-map cell for `tile` at (row, col): the class
/// g_tile_class_1d1a[tile] selects the painter: 1 speckles, 2 colour3 fill, 3 g_ui_color0 fill,
/// 4 top/bottom, 5 centre, 6 frame, 7 (and 0x5a) g_ui_color1 fill, 8 sun checker, 9 stripes,
/// 10 water(tile), 11 deep dots, 12 one deep pixel at (+2,+2), 13 mixed dots, 14 two vertical
/// g_ui_color1 lines at columns +1 and +2, 15 g_ui_color2+8 fill, 16 building(tile); other
/// classes (0) draw nothing. Ghidra order: (row, col, tile).
pub fn draw_gem_cell(gm: &mut Game, param_1: u16, param_2: u16, param_3: u16) {
    let tile = param_3;
    let class = gm.g8(g::TILE_CLASS_1D1A.wrapping_add(tile)) as u16;
    set_gem_cell_origin(gm, param_1 as i16, param_2 as i16);
    let (y, x) = (gy(gm), gx(gm));
    let fill_color: i16;
    match class {
        8 => { gem_draw_sun_checker(gm); return; }
        0xc => {
            let c = gem_deep_color(gm);
            gfx::set_color(gm, c);
            gfx::plot_pixel(gm, y + 2, x + 2);
            return;
        }
        9 => { gem_draw_stripes(gm); return; }
        10 => { gem_draw_water(gm, tile); return; }
        0xb => { gem_draw_deep_dots(gm); return; }
        0xd => { gem_draw_mixed_dots(gm); return; }
        0xe => {
            let c = gm.gi16(g::UI_COLOR1);
            gfx::set_color(gm, c);
            gfx::draw_line(gm, y + 3, x + 1, y, x + 1);
            gfx::draw_line(gm, y + 3, x + 2, y, x + 2);
            return;
        }
        0x10 => { gem_draw_building(gm, tile as i16); return; }
        0xf => { fill_color = gm.gi16(g::UI_COLOR2).wrapping_add(8); }
        1 => { gem_draw_speckles(gm); return; }
        2 => { gem_fill_color3(gm); return; }
        3 => { fill_color = gm.gi16(g::UI_COLOR0); }
        4 => { gem_draw_top_bottom(gm); return; }
        5 => { gem_draw_centre(gm); return; }
        6 => { gem_draw_frame(gm); return; }
        7 | 0x5a => { fill_color = gm.gi16(g::UI_COLOR1); }
        _ => { return; }
    }
    gfx::set_color(gm, fill_color);
    gfx::fill_rect(gm, y + 3, x + 3, y, x);
}

/// orig 1000:b38c (207 bytes). Vector 1778:078e (ROOT 'V'iew a gem outside dungeons, CAST
/// spells, the mirror's strange vision). Draws the 32x32 map window starting at
/// (g_chunk_y, g_chunk_x) as 4x4-pixel cells on page 1 (black 8..0xb7 square first), copies the
/// square to page 0, then blinks the party cell (y - g_chunk_y, x - g_chunk_x) with
/// fill_rect_xor in g_ui_color1 every 4 polls of poll_key_with_cursor until a key arrives, and
/// finally refresh_map_view. Ghidra order: (y, x).
pub fn view_gem_map(gm: &mut Game, param_1: i8, param_2: i8) {
    let x = param_2.wrapping_sub(gm.gi8(g::CHUNK_X));
    let y = param_1.wrapping_sub(gm.gi8(g::CHUNK_Y));
    gfx::set_draw_page(gm, 1);
    gfx::set_color(gm, 0);
    gfx::fill_rect(gm, 0xb7, 0xb7, 8, 8);
    let mut row: i16 = 0;
    loop {
        let mut col: i16 = 0;
        loop {
            let cy = (gm.g8(g::CHUNK_Y) as u16).wrapping_add(row as u16);
            let cx = (gm.g8(g::CHUNK_X) as u16).wrapping_add(col as u16);
            let p = rest::get_map_tile(gm, cy, cx);
            let tile = gm.g8(p) as u16;
            draw_gem_cell(gm, row as u16, col as u16, tile);
            col += 1;
            if !(col < 0x20) { break; }
        }
        row += 1;
        if !(row < 0x20) { break; }
    }
    gfx::copy_rect_between_pages(gm, 0xb7, 0xb7, 8, 8, 0, 1);
    gfx::set_draw_page(gm, 0);
    set_gem_cell_origin(gm, y as i16, x as i16);
    let mut n: i16 = 0;
    loop {
        let k = input::poll_key_with_cursor(gm);
        if k != 0 { break; }
        n += 1;
        if n == 4 {
            n = 0;
            let c = gm.gi16(g::UI_COLOR1);
            gfx::set_color(gm, c);
            let (py, px) = (gy(gm), gx(gm));
            gfx::fill_rect_xor(gm, py + 3, px + 3, py, px);
        }
        gm.pf.maybe_present();
    }
    view::refresh_map_view(gm);
}
