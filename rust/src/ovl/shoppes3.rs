//! SHOPPES3.OVL (translated)
//!
//! Shop conversations, part 3: the INN (shop type 7, vector 1778:0a22 -> ea94), dispatched
//! from TALK's `talk_to_shopkeeper`. Rest for the night, leave a companion at the inn (the
//! member's record moves to the end of the 16-record table with the inn's location in byte
//! 0x1f and the months-at-inn counter 0x17 cleared) or pick one up again (paying the monthly
//! rate per month spent). Text machinery (print_shop_text / print_shoppe_dat, greeting,
//! farewell, ask_yes_no, the Shadowlord surcharge) is SHOPPES', called through vectors.
//!
//! TALK-provided context (DGROUP): 0xb114 shop slot (index of the inn in the inn town list),
//! 0xaafe shopkeeper name pointer ('$'), 0xb118 price ('%').
//! Inn tables (DGROUP): 0x4d7e nightly rate per slot, 0x4dc4 rooms per slot, 0x4e6e SHOPPE.DAT
//! offsets of the room descriptions, 0x4e7a/0x4e80 bed coordinates.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;
use crate::ovl::shoppes::{print_shop_text, print_shoppe_dat, shadowlord_cheat_gold, print_greeting, print_farewell, ask_yes_no};

const SHOP_SLOT: u16 = 0xb114; // i16: inn index within the inn town list (TALK)
const SHOP_PRICE: u16 = 0xb118; // i16: current price, expanded by '%'
const PARTY_RECORDS: u16 = 0x55a8; // 16 records x 32 bytes (save 0x02..)
const PARTY_INN_LOCATION: u16 = 0x55c7; // record byte 0x1f: settlement of the inn the member stays at (0 = with the party)
const PARTY_INTELLIGENCE: u16 = 0x55b6; // record byte 0x0e
const PARTY_MP: u16 = 0x55b7; // record byte 0x0f
const PARTY_GENDER: u16 = 0x55b1; // record byte 0x09: 0x0b male, 0x0c female

/// Inn price adjustment: 0xb118 += price * (100 - 3 * intelligence) / 100 with a signed
/// 32-bit multiply and a SIGNED 32-bit divide (unlike SHOPPES/SHOPPES2's unsigned one).
/// `int3` is 3 * intelligence as the 16-bit value the caller computed, `price_hi` the high
/// word the caller supplies for the price operand (sign extension or 0).
fn adjust_price_for_intelligence(gm: &mut Game, int3: u16, price_hi: i16) {
    let price = gm.gi16(SHOP_PRICE);
    let d = int3.wrapping_sub(100);
    // 32-bit negation of the sign-extended (int*3 - 100)
    let hi = (((int3 as i16) >> 15) as u16)
        .wrapping_sub((int3 < 100) as u16)
        .wrapping_add((d != 0) as u16)
        .wrapping_neg();
    let prod = rest::__aFlmul(gm, d.wrapping_neg(), hi as i16, price as u16, price_hi);
    let q = rest::__aFldiv(gm, prod as u16, (prod >> 16) as u16, 100, 0);
    gm.set_gi16(SHOP_PRICE, price.wrapping_add(q as i16));
}

/// Copies one 32-byte party record (16 words) from DGROUP `src` to DGROUP `dst`.
fn copy_record(gm: &mut Game, dst: u16, src: u16) {
    for i in 0..16u16 {
        let v = gm.g16(src.wrapping_add(i * 2));
        gm.set_g16(dst.wrapping_add(i * 2), v);
    }
}

/// orig 1000:e1e0 (43 bytes). Counts the party records (all 16, including the active party)
/// whose inn-location byte (0x55c7 + 32*i) equals g_location, i.e. the companions staying at
/// this inn. (Address shared with FONT/CAST2/COMSUBS functions; vectors 1778:0566/0986/099e
/// belong to those overlays.)
pub fn count_guests_here(gm: &mut Game) -> i16 {
    let mut n: i16 = 0;
    let mut p = PARTY_INN_LOCATION;
    let loc = gm.g8(g::LOCATION);
    let mut left = 0x10;
    loop {
        if gm.g8(p) == loc {
            n = n.wrapping_add(1);
        }
        p = p.wrapping_add(0x20);
        left -= 1;
        if left == 0 { break; }
    }
    n
}

/// orig 1000:e20c (70 bytes). Prints two newlines and returns 1 when fewer guests stay here
/// than the inn has rooms (g_inn_room_counts[slot], 0x4dc4: 3,4,3,2,2,2). Otherwise prints
/// '"I am sorry, milady/sir, but we have no room available."' (gender 0x0c = milady) and
/// returns 0.
pub fn check_room_available(gm: &mut Game, gender: i16) -> u16 {
    text::print_string(gm, 0x4d84); // "\n\n"
    let n = count_guests_here(gm) as u16;
    let slot = gm.gi16(SHOP_SLOT) as u16;
    if n < gm.g8(slot.wrapping_add(g::UNK_4DC4)) as u16 {
        1
    } else {
        text::print_string(gm, 0x4d87); // "\"I am sorry,\n"
        let s: u16 = if gender == 0xc { 0x4d95 } else { 0x4d9c }; // "milady" / "sir"
        text::print_string(gm, s);
        text::print_string(gm, 0x4da0); // ", but we\nhave no room\navailable.\"\n\n"
        0
    }
}

/// orig 1000:e252 (566 bytes). Ghidra order (result, gender, intelligence). Rest for the
/// night ('R'). No room -> returns 0xfffe. Price = g_inn_rates[slot] (0x4d7e: 2,3,2,3,2,3) *
/// party count, intelligence-adjusted; the room description (SHOPPE.DAT g_inn_room_text_offsets
/// 0x4e6e[slot]) and '"Wilt thou take it?"' (ask_yes_no). 'N' returns `result` unchanged.
/// Without the gold: '"Highwaymen! Cheap, at that! OUT!" screams $.' and `result`. Otherwise
/// pays (surcharge), '"Have a pleasant night, sir!" says $.', closes the open door
/// (set_town_map_tile(g_door_y, g_door_x, g_door_tile)), moves the party to the inn's bed
/// (g_inn_bed_x/y 0x4e7a/0x4e80[slot]), puts every 'G' member to 'S'leep, "Zzzzzz....", twelve
/// pass_time(5), then pass_time(9) per animation frame (with regeneration-ring ticks and the
/// day/night tile toggle at hours 20 and 5) until 6 o'clock. "Morning!", NPCs re-placed
/// (TOWN init_town_npcs); every living member: HP = max HP, MP = INT (Avatar/Mage) or INT/2
/// (Bard); a 'P'oisoned member dies ('<name> has passed away.', HP 0), sleepers wake. Party x
/// += 1 (steps out of the bed), active member restored, returns 0xffff.
pub fn inn_rest(gm: &mut Game, result: u16, gender: i16, intelligence: i16) -> u16 {
    let mut result = result;
    text::put_char(gm, 0x52);
    if check_room_available(gm, gender) == 0 {
        return 0xfffe;
    }
    text::put_char(gm, 0x22);
    let slot = gm.gi16(SHOP_SLOT) as u16;
    let price = (gm.g8(slot.wrapping_add(g::UNK_4D7E)) as u16).wrapping_mul(gm.g8(g::PARTY_COUNT) as u16);
    gm.set_g16(SHOP_PRICE, price);
    let int3 = (intelligence as u16).wrapping_mul(3);
    adjust_price_for_intelligence(gm, int3, (price as i16) >> 15);
    let off = gm.g16(slot.wrapping_mul(2).wrapping_add(g::INN_ROOM_TEXT_OFFSETS));
    print_shoppe_dat(gm, off);
    text::print_string(gm, 0x4dca); // "\nWilt thou take\nit?\" "
    let c = ask_yes_no(gm) as u8;
    text::print_string(gm, 0x4de0); // "\n\n"
    if c == b'Y' {
        if gm.gi16(g::GOLD) < gm.gi16(SHOP_PRICE) {
            text::print_string(gm, 0x4de3); // "\"Highwaymen!\nCheap, at that!\nOUT!\" "
            print_shop_text(gm, 0x4e07); // "screams\n$.\n"
        } else {
            let v = gm.gi16(g::GOLD).wrapping_sub(gm.gi16(SHOP_PRICE));
            gm.set_gi16(g::GOLD, v);
            shadowlord_cheat_gold(gm);
            rest::draw_status_window(gm);
            text::print_string(gm, 0x4e13); // "\"Have a pleasant\nnight, "
            let s: u16 = if gender == 0xc { 0x4e2c } else { 0x4e33 }; // "milady" / "sir"
            text::print_string(gm, s);
            print_shop_text(gm, 0x4e37); // "!\"\nsays $.\n\n"
            util::animate_wait(gm, 10);
            let (dy, dx, dt) = (gm.g8(g::DOOR_Y), gm.g8(g::DOOR_X), gm.gi8(g::DOOR_TILE));
            rest::set_town_map_tile(gm, dy, dx, dt);
            let saved_active = gm.g8(g::ACTIVE_MEMBER);
            let bx = gm.g8(slot.wrapping_add(g::INN_BED_X));
            gm.set_g8(g::PARTY_X, bx);
            let by = gm.g8(slot.wrapping_add(g::INN_BED_Y));
            gm.set_g8(g::PARTY_Y, by);
            gm.set_g8(g::VIEW_DIRTY, 1);
            let n = gm.g8(g::PARTY_COUNT) as u16;
            if n != 0 {
                let mut p = g::PARTY_STATUS;
                let mut i: u16 = 0;
                loop {
                    if gm.g8(p) == b'G' {
                        gm.set_g8(p, b'S');
                    }
                    p = p.wrapping_add(0x20);
                    i = i.wrapping_add(1);
                    if !(i < n) { break; }
                }
            }
            rest::draw_status_window(gm);
            view::refresh_map_view(gm);
            text::print_string(gm, 0x4e44); // "Zzzzzz....\n\n"
            let mut k = 0xc;
            loop {
                rest::pass_time(gm, 5);
                k -= 1;
                if k == 0 { break; }
            }
            while gm.gi8(g::HOUR) != 6 {
                util::animate_wait(gm, 1);
                rest::regeneration_ring_tick(gm);
                rest::draw_status_window(gm);
                rest::pass_time(gm, 9);
                if gm.gi8(g::HOUR) == 0x14 || gm.gi8(g::HOUR) == 5 {
                    ovl::town::toggle_night_tiles(gm);
                }
            }
            text::print_string(gm, 0x4e51); // "Morning!\n"
            ovl::town::init_town_npcs(gm);
            let mut i: u16 = 0;
            if gm.g8(g::PARTY_COUNT) != 0 {
                let mut rec = PARTY_RECORDS;
                loop {
                    if gm.g8(rec.wrapping_add(0xb)) != b'D' {
                        let mhp = gm.g16(rec.wrapping_add(0x12));
                        gm.set_g16(rec.wrapping_add(0x10), mhp);
                        let class = gm.g8(rec.wrapping_add(10));
                        if class == b'A' || class == b'M' {
                            let v = gm.g8(rec.wrapping_add(0xe));
                            gm.set_g8(rec.wrapping_add(0xf), v);
                        } else if class == b'B' {
                            let v = gm.g8(rec.wrapping_add(0xe)) >> 1;
                            gm.set_g8(rec.wrapping_add(0xf), v);
                        }
                        if gm.g8(rec.wrapping_add(0xb)) == b'P' {
                            gm.set_g8(rec.wrapping_add(0xb), 0x44);
                            gm.set_g16(rec.wrapping_add(0x10), 0);
                            text::put_char(gm, 10);
                            text::print_string(gm, rec); // member name
                            text::print_string(gm, 0x4e5b); // " has\npassed away.\n"
                        } else if gm.g8(rec.wrapping_add(0xb)) == b'S' {
                            gm.set_g8(rec.wrapping_add(0xb), 0x47);
                        }
                    }
                    rec = rec.wrapping_add(0x20);
                    i = i.wrapping_add(1);
                    if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
                }
            }
            let x = gm.g8(g::PARTY_X).wrapping_add(1);
            gm.set_g8(g::PARTY_X, x);
            gm.set_g8(g::VIEW_DIRTY, 1);
            gm.set_g8(g::ACTIVE_MEMBER, saved_active);
            rest::draw_status_window(gm);
            result = 0xffff;
        }
    }
    result
}

/// orig 1000:e48e (481 bytes). Ghidra order (result, gender, intelligence). Leave a companion
/// ('L'). No room -> `result`. A lone Avatar gets SHOPPE.DAT 0x26e8 ('"Lord British is
/// missing, and thou dost plan to do is SLEEP for a month or so? Not in my inn!"') and 0xffff.
/// Otherwise asks '$ asks, "Who will stay?"' (select_party_member_no_zero): none -> "Nobody",
/// result 0xfffe; the Avatar (0) -> "Thy friend(s) will not leave thee!" and ask again; a dead
/// member -> SHOPPE.DAT 0x2723 ('Whaddya think this is, a morgue?'), result 0xffff. Else the
/// monthly rate = g_inn_rates[slot] * 10, intelligence-adjusted (price high word 0), '"The rate
/// for our most comfortable room will be % gold per month, due at check-out. Wilt thou take
/// it?"' (ask_yes_no). 'Y': result 1; the active member index is fixed up (cleared if it is the
/// one leaving, decremented if above); the member's record gets inn location = g_location and
/// months-at-inn = 0, is copied out, the records after it shift down one slot and the copy is
/// stored in slot 15; party count - 1, status window, '"I thank thee." says $.'
pub fn inn_leave_companion(gm: &mut Game, result: u16, gender: i16, intelligence: i16) -> u16 {
    let mut result = result;
    text::put_char(gm, 0x4c);
    if check_room_available(gm, gender) == 0 {
        return result;
    }
    if gm.g8(g::PARTY_COUNT) == 1 {
        print_shoppe_dat(gm, 0x26e8); // "Lord British is missing, and all thou dost plan to do is SLEEP ...? Not in my inn!"
        return 0xffff;
    }
    let int3 = (intelligence as u16).wrapping_mul(3);
    loop {
        let mut finished = true;
        print_shop_text(gm, 0x4e86); // "$ asks,\n\"Who will\nstay?\" "
        let m = rest::select_party_member_no_zero(gm) as u16;
        if m == 0xffff {
            text::print_string(gm, 0x4ea0); // "Nobody\n\n"
            result = 0xfffe;
        } else {
            text::print_string(gm, 0x4ea9); // "\n\n"
            if m == 0 {
                text::print_string(gm, 0x4eac); // "Thy friend"
                if 2 < gm.g8(g::PARTY_COUNT) {
                    text::put_char(gm, 0x73);
                }
                text::print_string(gm, 0x4eb7); // " will not leave thee!\n\n"
                finished = false;
            } else if gm.g8(g::PARTY_STATUS.wrapping_add(m.wrapping_mul(0x20))) == b'D' {
                print_shoppe_dat(gm, 0x2723); // "Whaddya think this is, a morgue? Get that corpse out of here!" yells $.
                result = 0xffff;
            } else {
                let slot = gm.gi16(SHOP_SLOT) as u16;
                let price = (gm.g8(slot.wrapping_add(g::UNK_4D7E)) as u16).wrapping_mul(10);
                gm.set_g16(SHOP_PRICE, price);
                adjust_price_for_intelligence(gm, int3, 0);
                text::print_string(gm, 0x4ecf); // "\"The rate for\nour most comfortable room will be "
                print_shop_text(gm, 0x4f00); // "% gold per month, due at check-out."
                text::print_string(gm, 0x4f24); // "\nWilt thou take\nit?\" "
                let c = ask_yes_no(gm) as u8;
                text::print_string(gm, 0x4f3a); // "\n\n"
                if c == b'Y' {
                    result = 1;
                    let active = gm.g8(g::ACTIVE_MEMBER);
                    if m == active as u16 {
                        gm.set_g8(g::ACTIVE_MEMBER, 0xff);
                    } else if m < active as u16 && active != 0xff {
                        gm.set_g8(g::ACTIVE_MEMBER, active.wrapping_sub(1));
                    }
                    let rec = m.wrapping_mul(0x20);
                    let loc = gm.g8(g::LOCATION);
                    gm.set_g8(PARTY_INN_LOCATION.wrapping_add(rec), loc);
                    gm.set_g8(g::PARTY_MONTHS_AT_INN.wrapping_add(rec), 0);
                    // local_22: copy of the leaving member's record
                    let mut saved = [0u16; 16];
                    for i in 0..16u16 {
                        saved[i as usize] = gm.g16(PARTY_RECORDS.wrapping_add(rec).wrapping_add(i * 2));
                    }
                    if (m as i16) < 0xf {
                        let mut dst = m.wrapping_mul(0x20).wrapping_add(PARTY_RECORDS);
                        let mut src = m.wrapping_mul(0x20).wrapping_add(0x55c8);
                        let mut left = (0xf as i16).wrapping_sub(m as i16);
                        loop {
                            copy_record(gm, dst, src);
                            dst = dst.wrapping_add(0x20);
                            src = src.wrapping_add(0x20);
                            left -= 1;
                            if left == 0 { break; }
                        }
                    }
                    for i in 0..16u16 {
                        gm.set_g16(0x5788u16.wrapping_add(i * 2), saved[i as usize]); // record 15
                    }
                    let n = gm.g8(g::PARTY_COUNT).wrapping_sub(1);
                    gm.set_g8(g::PARTY_COUNT, n);
                    rest::draw_status_window(gm);
                    print_shop_text(gm, 0x4f3d); // "\"I thank thee.\"\nsays $.\n\n"
                }
            }
        }
        if finished { break; }
    }
    result
}

/// orig 1000:e674 (33 bytes). Previous guest in the register: scans the records below `idx`
/// (idx-1 down to 1) for one whose inn location is g_location; returns its index or 0 when
/// none (record 0, the Avatar, is never a guest).
pub fn prev_guest(gm: &mut Game, idx: i16) -> i16 {
    let mut i = idx;
    let loc = gm.g8(g::LOCATION);
    loop {
        i = i.wrapping_sub(1);
        if i == 0 {
            return 0;
        }
        if gm.g8(PARTY_INN_LOCATION.wrapping_add((i as u16).wrapping_mul(0x20))) == loc {
            break;
        }
    }
    i
}

/// orig 1000:e696 (46 bytes). Next guest in the register: scans the records above `idx`
/// (idx+1 up to 15) for one whose inn location is g_location; returns its index or 0 when
/// none. `next_guest(0)` finds the first guest.
pub fn next_guest(gm: &mut Game, idx: i16) -> i16 {
    let mut i = idx;
    let loc = gm.g8(g::LOCATION);
    loop {
        i = i.wrapping_add(1);
        if 0xf < i {
            break;
        }
        if gm.g8(PARTY_INN_LOCATION.wrapping_add((i as u16).wrapping_mul(0x20))) == loc {
            break;
        }
    }
    if i == 0x10 {
        i = 0;
    }
    i
}

/// orig 1000:e6c6 (970 bytes). Ghidra order (result, intelligence). Pick up a companion
/// ('P'). A full party (6) -> "One must first be left behind!", no guest here -> '"No one here
/// is from thy party!" says $.' (both return `result`). One guest is chosen directly; with
/// several, '"Who will check out?"' opens the GUEST REGISTER in text window 1 (frame glyphs
/// 0x10-0x17 inside a 9x0x26/0x27 window, separator cleared) listing every guest's name
/// (records 1..15 at column 4) and runs a highlight browser in window 2: keys 1/3 move up
/// (`prev_guest`), 2/4 down (`next_guest`), the XOR bar (g_ui_color1, x 0xc6..0x131, 8 pixel
/// rows from y 0x28) follows the selection; Enter/space accept, Esc prints "No one" and
/// selects nobody; other keys are ignored. The separator and status window are redrawn.
/// Nobody -> `result`. Price = months at inn (min 1) * (g_inn_rates[slot]*10 adjusted for
/// intelligence, signed divide): '"That will be % gold, please."'; without the gold SHOPPE.DAT
/// 0x275a ('Unfortunately, thou dost not possess the necessary funds! GUARDS!') and TOWN
/// `raise_alarm`, returning 0xffff. Else pay (surcharge), status window, the guest's record is
/// moved into slot party_count (records party_count..sel-1 shift up by one), party_count += 1,
/// inn location cleared; a 'P'oisoned guest is now 'D'ead with 0 HP ("Thy friend has died, by
/// the way."), otherwise HP = max HP and MP = INT (Avatar/Mage) or INT/2 (Bard) with "I hope
/// thou hast found thy stay enjoyable,"; then 'says $.', status window, returns 1.
pub fn inn_pick_up_companion(gm: &mut Game, result: u16, intelligence: i16) -> u16 {
    let here = count_guests_here(gm);
    if gm.g8(g::PARTY_COUNT) == 6 {
        text::print_string(gm, 0x4f57); // "\n\nOne must first be left behind!\n\n"
        return result;
    }
    if here == 0 {
        print_shop_text(gm, 0x4f7a); // "\n\n\"No one here is from thy party!\"\nsays $.\n\n"
        return result;
    }
    let mut sel: i16;
    if here < 2 {
        sel = next_guest(gm, 0);
    } else {
        text::print_string(gm, 0x4fa7); // "\n\n\"Who will\ncheck out?\" "
        text::select_text_window(gm, 1);
        rest::clear_status_separator(gm);
        text::define_text_window(gm, 9, 0x26, 1, 0x18, 1);
        text::put_char(gm, 0xff);
        text::define_text_window(gm, 9, 0x27, 1, 0x18, 1);
        text::put_char(gm, 0x10);
        for _ in 0..0xd {
            text::put_char(gm, 0x11);
        }
        text::put_char(gm, 0x13);
        let mut row: i16 = 1;
        loop {
            text::set_cursor_pos(gm, row, 0);
            text::put_char(gm, 0x17);
            text::set_cursor_pos(gm, row, 0xe);
            text::put_char(gm, 0x17);
            row += 1;
            if !(row < 8) { break; }
        }
        text::put_char(gm, 10);
        text::put_char(gm, 0x14);
        for _ in 0..0xd {
            text::put_char(gm, 0x15);
        }
        text::put_char(gm, 0x16);
        text::set_cursor_pos(gm, 1, 1);
        text::print_string(gm, 0x4fc0); // "    GUEST"
        text::set_cursor_pos(gm, 2, 1);
        text::print_string(gm, 0x4fca); // "  REGISTER:\n\n"
        let loc = gm.g8(g::LOCATION);
        let mut p = 0x55e7u16; // inn location byte of record 1
        let mut rec = 0x55c8u16; // record 1
        let mut left = 0xf;
        loop {
            if gm.g8(p) == loc {
                let r = text::get_cursor_row(gm);
                text::set_cursor_pos(gm, r, 4);
                text::print_string(gm, rec);
                text::put_char(gm, 10);
            }
            p = p.wrapping_add(0x20);
            rec = rec.wrapping_add(0x20);
            left -= 1;
            if left == 0 { break; }
        }
        text::select_text_window(gm, 2);
        let mut chosen = false;
        sel = next_guest(gm, 0);
        let mut y: i16 = 0x28;
        let col = gm.gi16(g::UI_COLOR1);
        gfx::set_color(gm, col);
        loop {
            gfx::fill_rect_xor(gm, y + 7, 0x131, y, 0xc6);
            loop {
                let k = input::read_command_key(gm) as u8;
                let r: i16 = match k {
                    1 | 3 => {
                        // LAB_1000_e85c
                        let n = prev_guest(gm, sel);
                        if n != 0 {
                            gfx::fill_rect_xor(gm, y + 7, 0x131, y, 0xc6);
                            y -= 8;
                            sel = n;
                        }
                        n
                    }
                    2 | 4 => {
                        // LAB_1000_e884
                        let n = next_guest(gm, sel);
                        if n != 0 {
                            gfx::fill_rect_xor(gm, y + 7, 0x131, y, 0xc6);
                            y += 8;
                            sel = n;
                        }
                        n
                    }
                    0x0d | 0x20 => {
                        chosen = true;
                        1
                    }
                    0x1b => {
                        text::print_string(gm, 0x4fd8); // "No one\n\n"
                        sel = 0;
                        chosen = true;
                        1
                    }
                    _ => 0, // LAB_1000_e8d0
                };
                if r != 0 { break; }
            }
            if chosen { break; }
        }
        rest::draw_status_separator(gm);
        rest::draw_status_window(gm);
    }
    if sel == 0 {
        return result;
    }
    let mut months = gm.g8(g::PARTY_MONTHS_AT_INN.wrapping_add((sel as u16).wrapping_mul(0x20))) as u16;
    if months == 0 {
        months = 1;
    }
    let slot = gm.gi16(SHOP_SLOT) as u16;
    let price = (gm.g8(slot.wrapping_add(g::UNK_4D7E)) as u16).wrapping_mul(10);
    gm.set_g16(SHOP_PRICE, price);
    let int3 = (intelligence as u16).wrapping_mul(3);
    adjust_price_for_intelligence(gm, int3, 0);
    let total = months.wrapping_mul(gm.g16(SHOP_PRICE));
    gm.set_g16(SHOP_PRICE, total);
    print_shop_text(gm, 0x4fe1); // "\n\n\"That will be % gold, please.\"\n\n\""
    if gm.gi16(g::GOLD) < gm.gi16(SHOP_PRICE) {
        print_shoppe_dat(gm, 0x275a); // "Unfortunately, thou dost not possess the necessary funds!\nGUARDS!\"\nsays $.\n"
        ovl::town::raise_alarm(gm);
        return 0xffff;
    }
    let v = gm.gi16(g::GOLD).wrapping_sub(gm.gi16(SHOP_PRICE));
    gm.set_gi16(g::GOLD, v);
    shadowlord_cheat_gold(gm);
    rest::draw_status_window(gm);
    // local_24: copy of the guest's record
    let src = (sel as u16).wrapping_mul(0x20).wrapping_add(PARTY_RECORDS);
    let mut saved = [0u16; 16];
    for i in 0..16u16 {
        saved[i as usize] = gm.g16(src.wrapping_add(i * 2));
    }
    let count = gm.g8(g::PARTY_COUNT) as u16;
    if (count as i16) < sel {
        let mut dst = (sel as u16).wrapping_mul(0x20).wrapping_add(PARTY_RECORDS);
        let mut src = (sel as u16).wrapping_mul(0x20).wrapping_add(0x5588); // record sel-1
        let mut left = sel.wrapping_sub(count as i16);
        loop {
            copy_record(gm, dst, src);
            dst = dst.wrapping_sub(0x20);
            src = src.wrapping_sub(0x20);
            left -= 1;
            if left == 0 { break; }
        }
    }
    let dst = count.wrapping_mul(0x20).wrapping_add(PARTY_RECORDS);
    for i in 0..16u16 {
        gm.set_g16(dst.wrapping_add(i * 2), saved[i as usize]);
    }
    let rec = count.wrapping_mul(0x20);
    gm.set_g8(g::PARTY_COUNT, (count as u8).wrapping_add(1));
    gm.set_g8(PARTY_INN_LOCATION.wrapping_add(rec), 0);
    let msg: u16;
    if gm.g8(g::PARTY_STATUS.wrapping_add(rec)) == b'P' {
        gm.set_g8(g::PARTY_STATUS.wrapping_add(rec), 0x44);
        gm.set_g16(g::PARTY_HP.wrapping_add(rec), 0);
        msg = 0x5005; // "Thy friend has died, by the way.\"\n"
    } else {
        let mhp = gm.g16(g::PARTY_MAX_HP.wrapping_add(rec));
        gm.set_g16(g::PARTY_HP.wrapping_add(rec), mhp);
        let class = gm.g8(g::PARTY_CLASS.wrapping_add(rec));
        if class == b'A' || class == b'M' {
            let v = gm.g8(PARTY_INTELLIGENCE.wrapping_add(rec));
            gm.set_g8(PARTY_MP.wrapping_add(rec), v);
        } else if class == b'B' {
            let v = gm.g8(PARTY_INTELLIGENCE.wrapping_add(rec)) >> 1;
            gm.set_g8(PARTY_MP.wrapping_add(rec), v);
        }
        msg = 0x5028; // "I hope thou hast found thy stay enjoyable,\"\n"
    }
    // LAB_1000_ea6b
    text::print_string(gm, msg);
    print_shop_text(gm, 0x5055); // "says $.\n\n"
    rest::draw_status_window(gm);
    1
}

/// orig 1000:ea94 (288 bytes). INN shop entry (vector 1778:0a22, TALK shop type 7) for
/// `member`, whose intelligence and gender byte are captured first. Clears the protection
/// spell (g_protection_turns) and the status icon, redraws the status window and prints the
/// greeting. Loop: Y/N (space counts as 'N'; "No" ends the visit, "Yes" continues) then
/// '$ asks, "Art thou here to Pick up or Leave a companion, or to Rest for the night?"' and a
/// key: space -> done; 'L' -> `inn_leave_companion` (its result replaces the running result
/// unless it is 0xfffe; 0xffff ends the visit); 'P' -> `inn_pick_up_companion` (result
/// replaced; 0xffff ends); 'R' -> `inn_rest` (any result but 0xfffe ends the visit with that
/// result); other keys are ignored. Unless done, '"Is there anything more I can do for
/// thee?"' precedes the next Y/N. Ends with SHOPPES `print_farewell(result)` (0 nothing
/// bought, 1 paid, -1 nothing printed).
pub fn inn_shop(gm: &mut Game, member: i16) {
    let mut result: i16 = 0;
    let mut done = false;
    let intel = gm.g8((member as u16).wrapping_mul(0x20).wrapping_add(PARTY_INTELLIGENCE)) as i16;
    let gender = gm.g8((member as u16).wrapping_mul(0x20).wrapping_add(PARTY_GENDER)) as i16;
    gm.set_g8(g::PROTECTION_TURNS, 0);
    gm.set_g8(g::STATUS_ICON, 0);
    rest::draw_status_window(gm);
    print_greeting(gm);
    while !done {
        let mut c: u8;
        loop {
            c = input::read_command_key(gm) as u8;
            if c == b' ' {
                c = b'N';
            }
            if c == b'N' {
                text::print_string(gm, 0x505f); // "No"
            } else if c == b'Y' {
                text::print_string(gm, 0x5062); // "Yes"
            }
            if c == b'N' || c == b'Y' { break; }
        }
        if c == b'N' { break; }
        print_shop_text(gm, 0x5066); // "\n\n$ asks,\n\"Art thou here\nto Pick up or\n"
        text::print_string(gm, 0x508e); // "Leave a\ncompanion, or\nto Rest for the\nnight?\" "
        loop {
            c = input::read_command_key(gm) as u8;
            if c == b' ' {
                // LAB_1000_eb3f
                done = true;
            } else if c == b'L' {
                let r = inn_leave_companion(gm, result as u16, gender, intel) as i16;
                if -2 < r {
                    result = r;
                }
                // LAB_1000_eb5f
                if result == -1 {
                    done = true;
                }
            } else if c == b'P' {
                let r = inn_pick_up_companion(gm, result as u16, intel) as i16;
                result = r;
                if r == -1 {
                    done = true;
                }
            } else if c == b'R' {
                let r = inn_rest(gm, result as u16, gender, intel) as i16;
                if -2 < r {
                    done = true;
                    result = r;
                }
            } else {
                c = 0;
            }
            if c != 0 { break; }
        }
        if !done {
            text::print_string(gm, 0x50bd); // "\"Is there\nanything more\nI can do for\nthee?\""
        }
    }
    print_farewell(gm, result);
}
