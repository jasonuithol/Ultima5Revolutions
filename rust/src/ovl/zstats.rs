//! ZSTATS.OVL (translated)
//!
//! Level-4 overlay implementing the Z-stats command (`zstats_command`, vector 1778:06b6, from
//! ROOT `dispatch_command` 'Z' and COMBAT) and the Ready command (`ready_command`, vector
//! 1778:06ce, 'R'). Z-stats shows, in text window 1 over the status area, one page per party
//! member (`show_stats_page`: name/gender/level/class, status, Str/HP/Int/HM/Dex/Ex and
//! Magic; `show_arms_page`: the six equipped slots), the party Equipment page
//! (`show_party_equipment_page`: food, gold, keys, gems, torches, grapple) and the four
//! inventory lists Reagents / Spells / Items / Armaments (`inventory_page`, a scrolling list
//! of owned entries drawn by `print_item_line` inside `draw_list_frame`). Pages are cycled with
//! the arrow keys and '0'..'6'. The Items list is built by `snapshot_items_page` into the
//! scratch buffer 0xb9ee. Ready uses the same list machinery (`ready_browser`) with a cursor
//! and `ready_item` to equip/unequip while checking slots, hands, ammunition and strength.
//! `is_equipped`, `prev_owned` and `next_owned` are also used by SHOPPES (vectors
//! 1778:0806/0812/081e with `print_item_line`).
//!
//! Ghidra split off three fragments: ecea and ed12 belong to the `zstats_command` page loop
//! (FONT vector 1778:054e lands on ecea in FONT.OVL) and ee32 is the `0xff` tail of
//! `hands_state` (COMSUBS vector 1778:094a lands there in COMSUBS.OVL).
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// Party record offset (32 bytes per member at 0x55a8).
#[inline] fn rec32(m: i16) -> u16 { (m as u16).wrapping_mul(0x20) }
/// DGROUP 0xb9ee: the 0x26-byte Items page built by `snapshot_items_page` (8 scrolls, 8
/// potions, carpets, skull keys, amulet, crown, sceptre, 8 moonstones, 3 shards, spyglasses,
/// HMS Cape plans, sextants, pocket watch, black badge, sandalwood box).
const ITEMS_PAGE: u16 = 0xb9ee;
/// DGROUP 0x1a7e: item class byte per item id (2 ring, 4 amulet, 0x20 one-handed, 0x30
/// two-handed, 0x40 armour, 0x80 helm).
const ITEM_CLASS: u16 = 0x1a7e;
/// DGROUP 0x1aae: weight per item id (summed against the member's strength).
const ITEM_WEIGHT: u16 = 0x1aae;

/// orig 1000:e1e0 (129 bytes). Ghidra order (allow_zero). Member selection for Z-stats /
/// Ready. Outside combat, or when the active combatant is not a party member, prints
/// "Player: " (0x96b4) and runs ROOT `select_party_member(allow_zero)`; a valid choice echoes
/// the member's name and ends the line when the cursor is not at column 0. In combat with a
/// party-member active combatant that member is taken without asking. 0xffff (none) prints
/// "None!\n" (0x96be), 0xfffe ('0' = all) prints a newline. Returns the selection.
pub fn select_stats_member(gm: &mut Game, param_1: u16) -> u16 {
    let sel: u16;
    let ac = gm.g8(g::ACTIVE_COMBATANT) as u16;
    if gm.g8(g::LOCATION) < 0x81 || (gm.g8(g::ROSTER_FLAGS.wrapping_add(ac.wrapping_mul(8))) & 0x80) == 0 {
        text::print_string(gm, 0x96b4); // "Player: "
        sel = rest::select_party_member(gm, param_1 as i16) as u16;
        if -1 < sel as i16 {
            text::print_string(gm, sel.wrapping_mul(0x20).wrapping_add(0x55a8));
            if text::get_cursor_col(gm) != 0 {
                text::put_char(gm, 10);
            }
        }
    } else {
        sel = gm.g8(g::ROSTER_MEMBER.wrapping_add(ac.wrapping_mul(8))) as u16;
    }
    if sel == 0xffff {
        text::print_string(gm, 0x96be); // "None!\n"
    } else if sel == 0xfffe {
        text::put_char(gm, 10);
    }
    sel
}

/// orig 1000:e262 (501 bytes). Ghidra order (member). Draws the member's statistics page:
/// status icon bar, the name as title, window 1 cleared (0xff), then in a one-column-wider
/// window: g_unk_1a58[class index] leading spaces, the gender glyph (record +9), " Lv-"
/// (0x96d6), the level (+0x16), a space and the class name (0x1a44[class index], class index =
/// position of the class letter in "AMBFDTPRS" 0x96c6); line 1 the status name (0x1a6a[index
/// of the status letter in "GPDSC" 0x96d0]) between 0xfc/0xfb; from line 3 "Str=" (0x96dc)
/// +0xc, "  HP:" (0x96e2) +0x10, "\nInt=" (0x96e8) +0xe, "  HM:" (0x96ee) max HP +0x12,
/// "\nDex=" (0x96f4) +0xd, "  Ex:" (0x96fa) +0x14, "\n\n    Magic:" (0x9700) +0xf.
pub fn show_stats_page(gm: &mut Game, param_1: i16) {
    rest::draw_status_icon_bar(gm);
    let rec = rec32(param_1);
    rest::draw_status_title(gm, rec.wrapping_add(0x55a8));
    text::define_text_window(gm, 9, 0x26, 1, 0x18, 1);
    text::put_char(gm, 0xff);
    text::define_text_window(gm, 9, 0x27, 1, 0x18, 1);
    let class_i = rest::str_index_of(gm, gm.g8(rec.wrapping_add(0x55b2)) as u16, 0x96c6u16 as i16); // "AMBFDTPRS"
    let status_i = rest::str_index_of(gm, gm.g8(g::PARTY_STATUS.wrapping_add(rec)) as u16, 0x96d0u16 as i16); // "GPDSC"
    let indent = gm.gi16((class_i as u16).wrapping_mul(2).wrapping_add(0x1a58)); // g_unk_1a58[class]
    if 0 < indent {
        let mut i: i16 = 0;
        loop {
            text::put_char(gm, 0x20);
            i += 1;
            if !(i < gm.gi16((class_i as u16).wrapping_mul(2).wrapping_add(0x1a58))) { break; }
        }
    }
    text::put_char(gm, gm.g8(rec.wrapping_add(0x55b1))); // gender glyph
    text::print_string(gm, 0x96d6); // " Lv-"
    text::print_number(gm, 0x20, 1, gm.g8(rec.wrapping_add(0x55be)) as i16);
    text::put_char(gm, 0x20);
    text::print_string(gm, gm.g16((class_i as u16).wrapping_mul(2).wrapping_add(0x1a44))); // class name
    text::define_text_window(gm, 9, 0x26, 1, 0x18, 1);
    text::set_cursor_pos(gm, 1, 0);
    text::put_char(gm, 0xfc);
    text::print_string(gm, gm.g16((status_i as u16).wrapping_mul(2).wrapping_add(0x1a6a))); // status name
    text::define_text_window(gm, 9, 0x27, 1, 0x18, 1);
    text::set_cursor_pos(gm, 3, 0);
    text::put_char(gm, 0xfb);
    text::print_string(gm, 0x96dc); // "Str="
    text::print_number(gm, 0x30, 2, gm.g8(rec.wrapping_add(0x55b4)) as i16);
    text::print_string(gm, 0x96e2); // "  HP:"
    text::print_number(gm, 0x20, 4, gm.gi16(rec.wrapping_add(0x55b8)));
    text::print_string(gm, 0x96e8); // "\nInt="
    text::print_number(gm, 0x30, 2, gm.g8(rec.wrapping_add(0x55b6)) as i16);
    text::print_string(gm, 0x96ee); // "  HM:"
    text::print_number(gm, 0x20, 4, gm.gi16(g::PARTY_MAX_HP.wrapping_add(rec)));
    text::print_string(gm, 0x96f4); // "\nDex="
    text::print_number(gm, 0x30, 2, gm.g8(rec.wrapping_add(0x55b5)) as i16);
    text::print_string(gm, 0x96fa); // "  Ex:"
    text::print_number(gm, 0x20, 4, gm.gi16(rec.wrapping_add(0x55bc)));
    text::print_string(gm, 0x9700); // "\n\n    Magic:"
    text::print_number(gm, 0x20, 2, gm.g8(rec.wrapping_add(0x55b7)) as i16);
}

/// orig 1000:e458 (47 bytes). Ghidra order (item). Prints one equipped slot on the Arms page:
/// for an item other than 0xff a space, the short item name (g_short_name_ptrs 0x1962) and a
/// newline. Returns true when the slot held an item.
pub fn print_equipped_slot(gm: &mut Game, param_1: i16) -> bool {
    if param_1 != 0xff {
        text::put_char(gm, 0x20);
        text::print_string(gm, gm.g16((param_1 as u16).wrapping_mul(2).wrapping_add(0x1962)));
        text::put_char(gm, 10);
    }
    param_1 != 0xff
}

/// orig 1000:e488 (243 bytes). Ghidra order (member). Draws the member's Arms page: status
/// icon bar, name title, window cleared, "Arms\n\n" (0x970e) centred (0xfc/0xfe ... 0xfb/0xfe),
/// then `print_equipped_slot` for helmet (+0x19), armour (+0x1a), left hand (+0x1b), right
/// hand (+0x1c), ring (+0x1d) and amulet (+0x1e); when none is equipped "(None ready)"
/// (0x9716) is centred on line 4.
pub fn show_arms_page(gm: &mut Game, param_1: i16) {
    rest::draw_status_icon_bar(gm);
    let rec = rec32(param_1);
    rest::draw_status_title(gm, rec.wrapping_add(0x55a8));
    text::define_text_window(gm, 9, 0x26, 1, 0x18, 1);
    text::put_char(gm, 0xff);
    text::put_char(gm, 0xfc);
    text::put_char(gm, 0xfe);
    text::print_string(gm, 0x970e); // "Arms\n\n"
    text::put_char(gm, 0xfb);
    text::put_char(gm, 0xfe);
    let a = print_equipped_slot(gm, gm.g8(rec.wrapping_add(0x55c1)) as i16) as i16;
    let b = print_equipped_slot(gm, gm.g8(rec.wrapping_add(0x55c2)) as i16) as i16;
    let c = print_equipped_slot(gm, gm.g8(rec.wrapping_add(0x55c3)) as i16) as i16;
    let d = print_equipped_slot(gm, gm.g8(rec.wrapping_add(0x55c4)) as i16) as i16;
    let e = print_equipped_slot(gm, gm.g8(g::PARTY_RING.wrapping_add(rec)) as i16) as i16;
    let f = print_equipped_slot(gm, gm.g8(rec.wrapping_add(0x55c6)) as i16) as i16;
    if a + b + c + d + e + f == 0 {
        text::set_cursor_pos(gm, 4, 0);
        text::put_char(gm, 0xfc);
        text::print_string(gm, 0x9716); // "(None ready)"
        text::put_char(gm, 0xfb);
    }
    text::define_text_window(gm, 9, 0x27, 1, 0x18, 1);
}

/// orig 1000:e57c (194 bytes). The party Equipment page: status icon bar, "Equipment"
/// (0x9724) title, window cleared, then "\n Food: " (0x972e) g_food, "\n Gold: " (0x9738)
/// g_gold, "\n\n Keys......." (0x9742) g_keys, "\n Gems......." (0x9752) g_gems,
/// "\n Torches...." (0x9760) g_torches and "\n Grapple" (0x976e) when g_grapple is owned.
pub fn show_party_equipment_page(gm: &mut Game) {
    rest::draw_status_icon_bar(gm);
    rest::draw_status_title(gm, 0x9724); // "Equipment"
    text::define_text_window(gm, 9, 0x26, 1, 0x18, 1);
    text::put_char(gm, 0xff);
    text::define_text_window(gm, 9, 0x27, 1, 0x18, 1);
    text::print_string(gm, 0x972e); // "\n Food: "
    text::print_number(gm, 0x20, 4, gm.gi16(g::FOOD));
    text::print_string(gm, 0x9738); // "\n Gold: "
    text::print_number(gm, 0x20, 4, gm.gi16(g::GOLD));
    text::print_string(gm, 0x9742); // "\n\n Keys......."
    text::print_number(gm, 0x20, 2, gm.g8(g::KEYS) as i16);
    text::print_string(gm, 0x9752); // "\n Gems......."
    text::print_number(gm, 0x20, 2, gm.g8(g::GEMS) as i16);
    text::print_string(gm, 0x9760); // "\n Torches...."
    text::print_number(gm, 0x20, 2, gm.g8(g::TORCHES) as i16);
    if gm.gi8(g::GRAPPLE) != 0 {
        text::print_string(gm, 0x976e); // "\n Grapple"
    }
}

/// orig 1000:e63e (186 bytes). Ghidra order (rows). Draws the list frame used by the
/// inventory and Ready pages: window 1 is set to rows 1..rows and cleared (0xff), then set to
/// the 9-row/column-0x27 window and a box drawn with the frame glyphs 0x10, 13 x 0x11, 0x13 on
/// top, 0x17 at columns 0 and 14 of rows 1..rows-1, and 0x14, 13 x 0x15, 0x16 at the bottom.
/// Vector 1778:09da.
pub fn draw_list_frame(gm: &mut Game, param_1: i16) {
    text::define_text_window(gm, param_1.wrapping_add(1), 0x26, 1, 0x18, 1);
    text::put_char(gm, 0xff);
    text::define_text_window(gm, 9, 0x27, 1, 0x18, 1);
    text::put_char(gm, 0x10);
    let mut i: i16 = 0xd;
    loop {
        text::put_char(gm, 0x11);
        i -= 1;
        if i == 0 { break; }
    }
    text::put_char(gm, 0x13);
    let mut r: i16 = 1;
    while r != param_1 {
        text::set_cursor_pos(gm, r, 0);
        text::put_char(gm, 0x17);
        text::set_cursor_pos(gm, r, 0xe);
        text::put_char(gm, 0x17);
        r = r.wrapping_add(1);
    }
    text::put_char(gm, 10);
    text::put_char(gm, 0x14);
    let mut i: i16 = 0xd;
    loop {
        text::put_char(gm, 0x15);
        i -= 1;
        if i == 0 { break; }
    }
    text::put_char(gm, 0x16);
}

/// orig 1000:e6f8 (84 bytes). Ghidra order (item, member). Returns 1 when `item` is in any of
/// the member's six equipment slots (+0x19 helmet .. +0x1e amulet), else 0.
pub fn is_equipped(gm: &mut Game, param_1: u16, param_2: i16) -> u16 {
    let rec = rec32(param_2);
    let item = param_1;
    if gm.g8(rec.wrapping_add(0x55c1)) as u16 == item
        || gm.g8(rec.wrapping_add(0x55c2)) as u16 == item
        || gm.g8(rec.wrapping_add(0x55c3)) as u16 == item
        || gm.g8(rec.wrapping_add(0x55c4)) as u16 == item
        || gm.g8(g::PARTY_RING.wrapping_add(rec)) as u16 == item
        || gm.g8(rec.wrapping_add(0x55c6)) as u16 == item
    {
        1
    } else {
        0
    }
}

/// orig 1000:e74c (56 bytes). Ghidra order (member, counts, n, start). Returns the largest
/// index below `start` whose count byte (counts[i]) is non-zero or, when `member` is not 0xff,
/// which that member has equipped (`is_equipped`); -1 when none. `n` is unused. Vector
/// 1778:0806 (SHOPPES af10/b34c/b436).
pub fn prev_owned(gm: &mut Game, param_1: i16, param_2: i16, param_3: u16, param_4: i16) -> i16 {
    let (member, counts) = (param_1, param_2 as u16);
    let mut i = param_4;
    loop {
        i = i.wrapping_sub(1);
        if i < 0 {
            return -1;
        }
        if gm.gi8(counts.wrapping_add(i as u16)) != 0 {
            break;
        }
        if member != 0xff && is_equipped(gm, i as u16, member) != 0 {
            return i;
        }
    }
    i
}

/// orig 1000:e784 (61 bytes). Ghidra order (member, counts, n, start). Returns the smallest
/// index above `start` and below `n` whose count byte is non-zero or, when `member` is not
/// 0xff, which that member has equipped; -1 when none. Vector 1778:0812 (SHOPPES).
pub fn next_owned(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16, param_4: i16) -> i16 {
    let (member, counts, n) = (param_1, param_2 as u16, param_3);
    let mut i = param_4;
    loop {
        i = i.wrapping_add(1);
        if n <= i {
            return -1;
        }
        if gm.gi8(counts.wrapping_add(i as u16)) != 0 {
            break;
        }
        if member != 0xff && is_equipped(gm, i as u16, member) != 0 {
            return i;
        }
    }
    i
}

/// orig 1000:e7c2 (260 bytes). Ghidra order (marker, names, counts, idx). Prints one list
/// line for entry `idx`: when counts[idx] is not -1 the count ("--" 0x9778 for 0, else 2 wide)
/// followed by `marker` (printed in font 1 when it is a control glyph < 0x20); then the name
/// from names[idx]: a leading '*' prints "\x1c + " (0x977c, font 1) and the rest, '!' prints
/// "\x1d + " (0x9782, font 1) and the name from the table at 0x19b2 + idx*2 instead, '('
/// prints "Moonstone " (0x9788) and the next character in font 1, otherwise the name as is.
/// The line is padded with spaces to column 14 and ended with a newline. Vector 1778:081e
/// (SHOPPES af10).
pub fn print_item_line(gm: &mut Game, param_1: u16, param_2: i16, param_3: i16, param_4: i16) {
    let (marker, names, counts, idx) = (param_1, param_2 as u16, param_3 as u16, param_4 as u16);
    let c = gm.gi8(counts.wrapping_add(idx));
    if c != -1 {
        if c == 0 {
            text::print_string(gm, 0x9778); // "--"
        } else {
            text::print_number(gm, 0x20, 2, c as i16);
        }
        if marker < 0x20 {
            text::select_font(gm, 1);
        }
        text::put_char(gm, marker as u8);
        text::select_font(gm, 0);
    }
    let name = gm.g16(idx.wrapping_mul(2).wrapping_add(names));
    let first = gm.g8(name);
    if first == b'*' {
        text::select_font(gm, 1);
        text::print_string(gm, 0x977c); // "\x1c + "
        text::print_string(gm, name.wrapping_add(1));
        text::select_font(gm, 0);
    } else if first == b'!' {
        text::select_font(gm, 1);
        text::print_string(gm, 0x9782); // "\x1d + "
        text::select_font(gm, 0);
        let s = gm.g16(idx.wrapping_mul(2).wrapping_add(0x19b2)); // alternate name table (potion colours)
        text::print_string(gm, s);
    } else if first == b'(' {
        text::print_string(gm, 0x9788); // "Moonstone "
        text::select_font(gm, 1);
        text::put_char(gm, gm.g8(name.wrapping_add(1)));
        text::select_font(gm, 0);
    } else {
        text::print_string(gm, name);
    }
    let col = text::get_cursor_col(gm);
    if col < 0xe {
        let mut n = 0xe - col;
        loop {
            text::put_char(gm, 0x20);
            n -= 1;
            if n == 0 { break; }
        }
    }
    text::put_char(gm, 10);
}

/// orig 1000:e8c8 (681 bytes). Ghidra order (names, counts, n, title). Scrolling inventory
/// list page (Reagents / Spells / Items / Armaments) of the Z-stats screen. Draws the title and
/// an 8-row `draw_list_frame`; the first owned entry is `next_owned(0xff, counts, n, -1)` -
/// none prints "(None owned!)" (0x9794) on row 4 and waits for one command key. Otherwise the
/// loop: from row 1 column 1 prints owned entries with `print_item_line('-', ..)` until the
/// cursor reaches row 8; the status icon shows which directions have more entries (0x19 more
/// below, 0x18 more above, 0x12 both, plain bar none); then a command key: space, Esc, keys 1
/// and 2 (left/right page change) and '0'..'6' (page selection) end the page and are returned
/// to the caller; 3 scrolls up one entry, 4 down one, 0xd5 / 0xd6 (PgUp / PgDn) by seven,
/// 0xd3 (Home) jumps to the first entry and 0xd4 (End) to the last page (last entry then six
/// back). Returns the terminating key.
pub fn inventory_page(gm: &mut Game, param_1: u16, param_2: u16, param_3: u16, param_4: u16) -> i16 {
    let (names, counts, n, title) = (param_1 as i16, param_2 as i16, param_3 as i16, param_4);
    rest::draw_status_title(gm, title);
    draw_list_frame(gm, 8);
    let mut done = false;
    let mut key: i16;
    let mut first = next_owned(gm, 0xff, counts, n, -1); // local_6
    if first == -1 {
        draw_list_frame(gm, 8);
        text::set_cursor_pos(gm, 4, 1);
        text::print_string(gm, 0x9794); // "(None owned!)"
        rest::draw_status_icon_bar(gm);
        text::select_text_window(gm, 2);
        key = input::read_command_key(gm);
    } else {
        key = 0;
        while !done {
            text::select_text_window(gm, 1);
            text::set_cursor_pos(gm, 1, 1);
            let mut cur = first; // local_c
            while cur != -1 {
                print_item_line(gm, 0x2d, names, counts, cur);
                let row = text::get_cursor_row(gm);
                text::set_cursor_pos(gm, row, 1);
                if text::get_cursor_row(gm) == 8 {
                    break;
                }
                cur = next_owned(gm, 0xff, counts, n, cur);
            }
            let mut more: i16 = 0; // local_10
            if prev_owned(gm, 0xff, counts, n as u16, first) != -1 {
                more = 2;
            }
            if cur != -1 && next_owned(gm, 0xff, counts, n, cur) != -1 {
                more += 1;
            }
            if more == 0 {
                rest::draw_status_icon_bar(gm);
            } else if more == 1 {
                rest::draw_status_icon(gm, 0x19);
            } else if more == 2 {
                rest::draw_status_icon(gm, 0x18);
            } else if more == 3 {
                rest::draw_status_icon(gm, 0x12);
            }
            // LAB_1000_e9b3
            text::select_text_window(gm, 2);
            key = input::read_command_key(gm);
            if key == 0x20 {
                done = true;
            } else if key < 0x21 {
                if key == 3 {
                    // scroll up one
                    for _ in 0..1 {
                        if prev_owned(gm, 0xff, counts, n as u16, first) != -1 {
                            first = prev_owned(gm, 0xff, counts, n as u16, first);
                        }
                    }
                } else if key < 4 {
                    if 0 < key && key <= 2 {
                        done = true;
                    }
                } else if key == 4 {
                    // scroll down one
                    for _ in 0..1 {
                        if cur != -1 && next_owned(gm, 0xff, counts, n, cur) != -1 {
                            first = next_owned(gm, 0xff, counts, n, first);
                            cur = next_owned(gm, 0xff, counts, n, cur);
                        }
                    }
                } else if key == 0x1b {
                    done = true;
                }
            } else if key == 0xd3 {
                first = next_owned(gm, 0xff, counts, n, -1);
            } else if key < 0xd4 {
                if 0x2f < key && key <= 0x36 {
                    done = true;
                }
            } else if key == 0xd4 {
                first = prev_owned(gm, 0xff, counts, n as u16, n);
                let mut k: i16 = 0;
                while k < 6 {
                    let p = prev_owned(gm, 0xff, counts, n as u16, first);
                    if p == -1 { break; }
                    k += 1;
                    first = p;
                }
            } else if key == 0xd5 {
                for _ in 0..7 {
                    if prev_owned(gm, 0xff, counts, n as u16, first) != -1 {
                        first = prev_owned(gm, 0xff, counts, n as u16, first);
                    }
                }
            } else if key == 0xd6 {
                for _ in 0..7 {
                    if cur != -1 && next_owned(gm, 0xff, counts, n, cur) != -1 {
                        first = next_owned(gm, 0xff, counts, n, first);
                        cur = next_owned(gm, 0xff, counts, n, cur);
                    }
                }
            }
        }
    }
    key
}

/// orig 1000:eb7a (157 bytes). Builds the Items page buffer at 0xb9ee (0x26 bytes): [0..8)
/// g_scrolls, [8..16) g_potions, 0xb9fe g_magic_carpets, 0xb9ff g_skull_keys, 0xba00
/// g_amulet_lb, 0xba01 g_crown_lb, 0xba02 g_sceptre, [0xba03..0xba0b) 0xff for each moonstone
/// whose g_moonstone_loc is 0xff (carried) else 0, 0xba0b..0xba0d the three shards (copied as
/// one word plus one byte from 0x57b6), 0xba0e g_spyglasses, 0xba0f 0xff when g_hms_cape_plans
/// else 0, 0xba10 g_sextants, 0xba11 g_pocket_watch, 0xba12 g_black_badge, 0xba13
/// g_sandalwood_box. Vector 1778:09e6.
pub fn snapshot_items_page(gm: &mut Game) {
    let mut i: u16 = 0;
    loop {
        gm.set_g8(ITEMS_PAGE.wrapping_add(i), gm.g8(g::SCROLLS.wrapping_add(i)));
        gm.set_g8(0xb9f6u16.wrapping_add(i), gm.g8(g::POTIONS.wrapping_add(i)));
        let v = if gm.gi8(g::MOONSTONE_LOC.wrapping_add(i)) == -1 { 0xff } else { 0 };
        gm.set_g8(0xba03u16.wrapping_add(i), v);
        i += 1;
        if !(i < 8) { break; }
    }
    gm.set_g8(0xb9fe, gm.g8(g::MAGIC_CARPETS));
    gm.set_g8(0xb9ff, gm.g8(g::SKULL_KEYS));
    gm.set_g8(0xba00, gm.g8(g::AMULET_LB));
    gm.set_g8(0xba01, gm.g8(g::CROWN_LB));
    gm.set_g8(0xba02, gm.g8(g::SCEPTRE));
    // three shards: one word + one byte copied from g_shard_falsehood
    gm.set_g16(0xba0b, gm.g16(g::SHARD_FALSEHOOD));
    gm.set_g8(0xba0d, gm.g8(g::SHARD_FALSEHOOD.wrapping_add(2)));
    gm.set_g8(0xba0e, gm.g8(g::SPYGLASSES));
    if gm.gi8(g::HMS_CAPE_PLANS) == 0 {
        gm.set_g8(0xba0f, 0);
    } else {
        gm.set_g8(0xba0f, 0xff);
    }
    gm.set_g8(0xba10, gm.g8(g::SEXTANTS));
    gm.set_g8(0xba11, gm.g8(g::POCKET_WATCH));
    gm.set_g8(0xba12, gm.g8(g::BLACK_BADGE));
    gm.set_g8(0xba13, gm.g8(g::SANDALWOOD_BOX));
}

/// orig 1000:ec1a (203 bytes, plus the ecea and ed12 fragments). The Z-stats command.
/// `select_stats_member(1)`: 0xfffe ('0') starts on page 6 (the party Equipment page), a
/// negative result returns at once. Prints "\nStatus: " (0x97a2), builds the Items page,
/// clears the status separator and enters the page loop with page = member*2 and key 0.
/// Pages: 0..11 are member pages (even = stats, odd = arms, member = page/2), 12 = party
/// Equipment, 13 Reagents (0x19d2 names, g_reagents 0x5850, 8), 14 Spells (0x19e2,
/// g_mixed_spells 0x57f0, 0x30), 15 Items (0x1916, 0xb9ee, 0x26), 16 Armaments (0x1962,
/// g_inventory 0x57c0, 0x30); the list pages run `inventory_page` and use the key it returns.
/// Keys: space / Esc finish (`finish_stats_screen`); 1 and 3 go to the previous page (from
/// 12 to the last member's arms page party_count*2-1, from 0 to 16); 2 and 4 to the next
/// (from party_count*2-1 to 12, from 16 to 0); '0' jumps to page 12; '1'..'6' (when below
/// g_party_count) to that member's stats page; other keys redraw. Member and Equipment
/// pages read their key with `read_command_key` in window 2. Vector 1778:06b6.
pub fn zstats_command(gm: &mut Game) {
    let mut sel = select_stats_member(gm, 1) as i16; // local_8
    if sel == -2 {
        sel = 6;
    } else if sel < 0 {
        return;
    }
    text::print_string(gm, 0x97a2); // "\nStatus: "
    snapshot_items_page(gm);
    rest::clear_status_separator(gm);
    let mut page: i16 = sel << 1; // local_4
    let mut key: u8 = 0; // local_6
    loop {
        // LAB_1000_ec58
        if key == 0x20 || key == 0x1b {
            finish_stats_screen(gm);
            return;
        }
        let k = key as u16;
        if k == 3 || k == 1 {
            // LAB_1000_ec86: previous page
            if page == 0xc {
                page = (gm.g8(g::PARTY_COUNT) as i16).wrapping_mul(2).wrapping_sub(1);
            } else if page < 1 {
                page = 0x10;
            } else {
                page -= 1;
            }
        } else if k == 2 || k == 4 {
            // LAB_1000_ecca: next page
            if (gm.g8(g::PARTY_COUNT) as i16).wrapping_mul(2).wrapping_sub(1) == page {
                page = 0xc;
            } else if page < 0x10 {
                page += 1;
            } else {
                page = 0; // ecea
            }
        } else if k > 3 {
            // FUN_1000_ed12 fragment: digit keys
            if k == 0x30 {
                page = 0xc;
            } else if k >= 0x31 && k <= 0x36 {
                if k.wrapping_sub(0x31) < gm.g8(g::PARTY_COUNT) as u16 {
                    page = (k as i16).wrapping_mul(2).wrapping_sub(0x62);
                }
            }
        }
        // LAB_1000_ecab
        text::select_text_window(gm, 1);
        if page < 0xc {
            if (page & 1) == 0 {
                show_stats_page(gm, page >> 1);
            } else {
                show_arms_page(gm, page >> 1);
            }
        } else if page == 0xc {
            show_party_equipment_page(gm);
        } else {
            let (names, counts, n, title): (u16, u16, u16, u16) = match page {
                0xd => (0x19d2, 0x5850, 8, 0x97ac),    // "Reagents", g_reagents
                0xe => (0x19e2, 0x57f0, 0x30, 0x97b6), // "Spells", g_mixed_spells
                0xf => (0x1916, ITEMS_PAGE, 0x26, 0x97be), // "Items"
                0x10 => (0x1962, 0x57c0, 0x30, 0x97c4), // "Armaments", g_inventory
                _ => continue, // TODO(verify): unreachable page; the original loops on the same key
            };
            key = inventory_page(gm, names, counts, n, title) as u8;
            continue;
        }
        // LAB_1000_ed3f
        text::select_text_window(gm, 2);
        key = input::read_command_key(gm) as u8;
    }
}

/// orig 1000:ecea (37 bytes). Not a function: the "wrap to page 0" step of the
/// `zstats_command` page loop that Ghidra split off because the FONT vector 1778:054e lands on
/// this address in FONT.OVL. Translated inside `zstats_command`.
pub fn fn_1000_ecea_fragment(gm: &mut Game) {}

/// orig 1000:ed12 (156 bytes). Not a function: the digit-key dispatch (4, '0', '1'..'6') of
/// the `zstats_command` page loop. Translated inside `zstats_command`.
pub fn fn_1000_ed12_fragment(gm: &mut Game) {}

/// orig 1000:edb6 (23 bytes). Restores the status area after Z-stats: status icon bar, top
/// bar, separator, `draw_status_window`, then prints "Done\n" (0x97ce).
pub fn finish_stats_screen(gm: &mut Game) {
    rest::draw_status_icon_bar(gm);
    rest::draw_status_top_bar(gm);
    rest::draw_status_separator(gm);
    rest::draw_status_window(gm);
    text::print_string(gm, 0x97ce); // "Done\n"
}

/// orig 1000:edce (27 bytes). Ghidra order (msg). Prints a Ready refusal: "\n\n" (0x97d4),
/// the message, then "\n\nItem: " (0x97d8).
pub fn print_ready_refusal(gm: &mut Game, param_1: u16) {
    text::print_string(gm, 0x97d4); // "\n\n"
    text::print_string(gm, param_1);
    text::print_string(gm, 0x97d8); // "\n\nItem: "
}

/// orig 1000:edea (71 bytes, plus the ee32 tail). Ghidra order (member). Hand availability:
/// both hands (+0x1b, +0x1c) empty -> 2; left hand empty -> 0; right hand empty and the left
/// hand's item class (0x1a7e) is not '0' (two-handed) -> 1; otherwise 0xff (no free hand).
pub fn hands_state(gm: &mut Game, param_1: i16) -> u16 {
    let rec = rec32(param_1);
    let left = gm.gi8(rec.wrapping_add(0x55c3));
    let right = gm.gi8(rec.wrapping_add(0x55c4));
    if left == -1 && right == -1 {
        2
    } else if left == -1 {
        0
    } else if right == -1 && gm.gi8((left as u8 as u16).wrapping_add(ITEM_CLASS)) != b'0' as i8 {
        1
    } else {
        0xff // fn_1000_ee32
    }
}

/// orig 1000:ee32 (9 bytes). Not a function: the `return 0xff` tail of `hands_state` (edea);
/// Ghidra names it COMSUBS_ee32 because the COMSUBS vector 1778:094a lands on this address in
/// COMSUBS.OVL. Translated inside `hands_state`.
pub fn fn_1000_ee32_fragment(gm: &mut Game) -> u16 { 0xff }

/// orig 1000:ee3c (711 bytes). Ghidra order (item, member). Ready/unready `item` for
/// `member`; returns 1 only when a ring vanished (which ends the Ready browser), else 0.
/// Items 0x1b and 0x1d (ammunition counters) do nothing. Armour 9..0xf cannot be changed in
/// combat (location > 0x7f) while enemies remain ("Thou canst not change armour in heated
/// battle!" 0x97e2). An already equipped item is unequipped (`remove_equipped_item`, count at
/// 0x57c0+item incremented below 99); in combat unreadying 0x2a also restores the member's
/// combat tile from 0x1ade[class index] into the active combatant's monster slot. Weapons
/// 0x1a / 0x24 need g_inventory[0x1b] and 0x1c needs g_inventory[0x1d] ("Thou hast no
/// ammunition for that weapon!" 0x981c). The weights (0x1aae) of the six equipped slots are
/// summed; by item class (0x1a7e): 2 ring (slot +0x1d must be free: 0x9916), 4 amulet
/// (+0x1e: 0x98f0), 0x20 one-handed (`hands_state`: 0xff -> 0x9892, 2 -> left hand, else the
/// free hand +0x1b/+0x1c), '0' two-handed (both hands free: 0x98ba, left hand), '@' armour
/// (+0x1a: 0x9866), 0x80 helm (+0x19: 0x9846); an unknown class writes to record +0x1f (the
/// original's stale pointer). If item weight + carried weight <= strength (+0xc) the slot
/// takes the item and the count drops; rings 0x2a / 0x2c then vanish on a 1-in-16 roll
/// ("\n\nRing vanishes!\n" 0x995e, ring slot 0xff, tone, return 1); readying 0x2a in combat
/// sets the combatant's slot tile/anim tile to 0x1d. Too heavy: "Thou art not strong enough!"
/// (0x9942). Refusals go through `print_ready_refusal`.
pub fn ready_item(gm: &mut Game, param_1: i16, param_2: i16) -> u16 {
    let (item, member) = (param_1, param_2);
    let rec = rec32(member);
    let mut slot_p: u16 = 0; // local_6: only valid in combat
    if 0x7f < gm.g8(g::LOCATION) {
        let s = gm.g8(g::ROSTER_SLOT.wrapping_add((gm.g8(g::ACTIVE_COMBATANT) as u16).wrapping_mul(8))) as u16;
        slot_p = g::MONSTER_TABLE.wrapping_add(s.wrapping_mul(8));
    }
    if item == 0x1b {
        return 0;
    }
    if item == 0x1d {
        return 0;
    }
    let msg: u16;
    'refuse: {
        if 8 < item && item < 0x10 && 0x7f < gm.g8(g::LOCATION) && gm.gi8(g::NO_MORE_ENEMIES) == 0 {
            msg = 0x97e2; // "Thou canst not change armour in heated battle!"
            break 'refuse;
        }
        let inv_p = (item as u16).wrapping_add(0x57c0);
        if is_equipped(gm, item as u16, member) != 0 {
            rest::remove_equipped_item(gm, item as u16, member);
            if gm.g8(inv_p) < 99 {
                let v = gm.gi8(inv_p).wrapping_add(1);
                gm.set_gi8(inv_p, v);
            }
            if gm.g8(g::LOCATION) < 0x80 {
                return 0;
            }
            if item != 0x2a {
                return 0;
            }
            let ci = rest::str_index_of(gm, gm.g8(rec.wrapping_add(0x55b2)) as u16, 0x9812u16 as i16); // "AMBFDTPRS"
            let tile = gm.g8((ci as u16).wrapping_add(0x1ade)); // class combat tile
            gm.set_g8(slot_p.wrapping_add(1), tile);
            gm.set_g8(slot_p, tile);
            return 0;
        }
        if ((item == 0x1a || item == 0x24) && gm.gi8(0x57db) == 0) || (item == 0x1c && gm.gi8(0x57dd) == 0) {
            msg = 0x981c; // "Thou hast no ammunition for that weapon!"
            break 'refuse;
        }
        let mut carried: i16 = 0;
        let mut p = rec.wrapping_add(0x55c1); // local_10
        let mut k: i16 = 6;
        loop {
            let it = gm.g8(p);
            if it != 0xff {
                carried = carried.wrapping_add(gm.g8((it as u16).wrapping_add(ITEM_WEIGHT)) as i16);
            }
            p = p.wrapping_add(1);
            k -= 1;
            if k == 0 { break; }
        }
        let mut target = p; // TODO(verify): unknown item classes fall through with the stale loop pointer (record +0x1f)
        let w = gm.g8((item as u16).wrapping_add(ITEM_WEIGHT)) as u16;
        let strength = gm.g8(rec.wrapping_add(0x55b4)) as u16;
        let cls = gm.gi8((item as u16).wrapping_add(ITEM_CLASS));
        if cls == 2 {
            if gm.gi8(g::PARTY_RING.wrapping_add(rec)) != -1 {
                msg = 0x9916; // "Only one magic ring may be worn at a time!"
                break 'refuse;
            }
            target = g::PARTY_RING.wrapping_add(rec);
        } else if cls == 4 {
            if gm.gi8(rec.wrapping_add(0x55c6)) != -1 {
                msg = 0x98f0; // "Thou must remove thine other amulet!"
                break 'refuse;
            }
            target = rec.wrapping_add(0x55c6);
        } else if cls == b' ' as i8 {
            let mut h = hands_state(gm, member) as i16; // local_c
            if h == 0xff {
                msg = 0x9892; // "Thou must free one of thy hands first!"
                break 'refuse;
            }
            if h == 2 {
                h = 0;
            }
            target = rec.wrapping_add(h as u16).wrapping_add(0x55c3);
        } else if cls == b'0' as i8 {
            if hands_state(gm, member) != 2 {
                msg = 0x98ba; // "Both hands must be free before thou canst wield that!"
                break 'refuse;
            }
            target = rec.wrapping_add(0x55c3);
        } else if cls == b'@' as i8 {
            if gm.gi8(rec.wrapping_add(0x55c2)) != -1 {
                msg = 0x9866; // "Thou must first remove thine other armour!"
                break 'refuse;
            }
            target = rec.wrapping_add(0x55c2);
        } else if cls == -0x80 {
            if gm.gi8(rec.wrapping_add(0x55c1)) != -1 {
                msg = 0x9846; // "Remove first thy present helm!"
                break 'refuse;
            }
            target = rec.wrapping_add(0x55c1);
        }
        // LAB_1000_efc9
        if w.wrapping_add(carried as u16) <= strength {
            gm.set_g8(target, item as u8);
            let v = gm.gi8(inv_p).wrapping_sub(1);
            gm.set_gi8(inv_p, v);
            if (item == 0x2a || item == 0x2c) && util::rand_range(gm, 0xf, 0) == 0 {
                text::print_string(gm, 0x995e); // "\n\nRing vanishes!\n"
                gm.set_g8(g::PARTY_RING.wrapping_add(rec), 0xff);
                rest::play_tone_sweep(gm, 0x28, 1, 2000, 0x4b0);
                return 1;
            }
            if item != 0x2a {
                return 0;
            }
            if gm.g8(g::LOCATION) < 0x80 {
                return 0;
            }
            gm.set_g8(slot_p.wrapping_add(1), 0x1d);
            gm.set_g8(slot_p, 0x1d);
            return 0;
        }
        msg = 0x9942; // "Thou art not strong enough!"
    }
    // LAB_1000_ef0f
    print_ready_refusal(gm, msg);
    0
}

/// orig 1000:f10e (856 bytes). Ghidra order (mode, member, first). List browser with a cursor
/// used by the Ready command: mode 'R' (0x52) lists g_inventory (0x57c0, 0x30 entries, short
/// names 0x1962, marker = the equipped glyph 0x1ae8[item] when the member has it equipped,
/// else a space), any other mode the Items page (0xb9ee, 0x26, names 0x1916, marker space).
/// Each pass redraws from row 1 column 1 the entries from `first` (`next_owned(member, ..)`
/// chains, so equipped items with count 0 are listed too), printing 0xfd around the line at
/// the cursor row (`cursor_row`, initially 1) and remembering that entry as the selection,
/// until row 8; the status icon shows 0x19 / 0x18 / 0x12 for more entries below / above /
/// both. Keys: Enter and space select (mode 'R': `ready_item(selection, member)`, whose result
/// 1 ends the browser; other modes end at once); Esc prints "Done\n" (0x9970, mode 'R') or
/// "None!\n" (0x9976) and returns -1; 1 and 3 move up one, 0xd5 seven: at cursor rows 1 and 4
/// the list scrolls instead of the cursor (when it cannot, a row-4 cursor moves up), elsewhere
/// the cursor moves up; 2 and 4 move down one, 0xd6 seven: at rows 4 and 7 or past the shown
/// entries the list scrolls (when it cannot and more than 4 entries are shown, a row-4 cursor
/// moves down), elsewhere the cursor moves down; 0xd3 (Home) restarts at the first entry with
/// the cursor on row 1; 0xd4 (End) shows the last page with the cursor on its last row.
/// Returns the selected entry index. Vector 1778:09f2.
pub fn ready_browser(gm: &mut Game, param_1: i16, param_2: u16, param_3: i16) -> i16 {
    let (mode, member) = (param_1, param_2 as i16);
    let mut first = param_3;
    let mut done: i16 = 0; // local_8
    let mut cursor_row: i16 = 1; // local_10
    let counts: i16; // local_14
    let n: i16; // local_a
    if mode == 0x52 {
        counts = 0x57c0u16 as i16;
        n = 0x30;
    } else {
        counts = ITEMS_PAGE as i16;
        n = 0x26;
    }
    let mut sel: i16 = 0; // local_1a (uninitialised in the original until a row matches)
    loop {
        // LAB_1000_f462
        if done != 0 {
            return sel;
        }
        text::select_text_window(gm, 1);
        text::set_cursor_pos(gm, 1, 1);
        let mut shown: i16 = 0; // local_4
        let mut cur = first; // iVar2
        while cur != -1 {
            shown += 1;
            if text::get_cursor_row(gm) == cursor_row {
                text::put_char(gm, 0xfd);
                sel = cur;
            }
            let (marker, names): (u16, i16) = if mode == 0x52 {
                let m = if is_equipped(gm, cur as u16, member) == 0 {
                    0x20
                } else {
                    gm.g8((cur as u16).wrapping_add(0x1ae8)) as u16 // equipped-slot glyph
                };
                (m, 0x1962)
            } else {
                (0x20, 0x1916)
            };
            print_item_line(gm, marker, names, counts, cur);
            let row = text::get_cursor_row(gm);
            text::set_cursor_pos(gm, row, 1);
            if text::get_cursor_row(gm) - 1 == cursor_row {
                text::put_char(gm, 0xfd);
            }
            if text::get_cursor_row(gm) == 8 {
                break;
            }
            cur = next_owned(gm, member, counts, n, cur);
        }
        let mut more: i16 = 0; // local_18
        if prev_owned(gm, member, counts, n as u16, first) != -1 {
            more = 2;
        }
        if cur != -1 && next_owned(gm, member, counts, n, cur) != -1 {
            more += 1;
        }
        if more == 0 {
            rest::draw_status_icon_bar(gm);
        } else if more == 1 {
            rest::draw_status_icon(gm, 0x19);
        } else if more == 2 {
            rest::draw_status_icon(gm, 0x18);
        } else if more == 3 {
            rest::draw_status_icon(gm, 0x12);
        }
        // LAB_1000_f257
        text::select_text_window(gm, 2);
        let key = input::read_command_key(gm);
        let mut select = key == 0xd;
        if !select {
            if key < 0xe {
                if key == 1 || key == 3 {
                    // LAB_1000_f2a0: up
                    let steps: i16 = if key == 0xd5 { 7 } else { 1 };
                    for _ in 0..steps {
                        if cursor_row == 4 || cursor_row == 1 {
                            let p = prev_owned(gm, member, counts, n as u16, first);
                            if p == -1 {
                                if cursor_row == 4 {
                                    cursor_row -= 1;
                                }
                            } else {
                                first = prev_owned(gm, member, counts, n as u16, first);
                            }
                        } else {
                            cursor_row -= 1;
                        }
                    }
                    continue;
                }
                if key == 2 || key == 4 {
                    // down
                    let steps: i16 = if key == 0xd6 { 7 } else { 1 };
                    for _ in 0..steps {
                        if cursor_row == 4 || cursor_row == 7 || shown <= cursor_row {
                            if cur == -1 || next_owned(gm, member, counts, n, cur) == -1 {
                                if cursor_row == 4 && 4 < shown {
                                    cursor_row += 1;
                                }
                            } else {
                                first = next_owned(gm, member, counts, n, first);
                                cur = next_owned(gm, member, counts, n, cur);
                            }
                        } else {
                            cursor_row += 1;
                        }
                    }
                    continue;
                }
                continue;
            }
            if key == 0x1b {
                let s = if mode == 0x52 { 0x9970 } else { 0x9976 }; // "Done\n" / "None!\n"
                text::print_string(gm, s);
                done = 1;
                sel = -1;
                continue;
            }
            if key == 0x20 {
                select = true;
            } else if key == 0xd3 {
                first = next_owned(gm, member, counts, n, -1);
                cursor_row = 1;
                continue;
            } else if key == 0xd4 {
                first = prev_owned(gm, member, counts, n as u16, n);
                cursor_row = 1;
                loop {
                    sel = prev_owned(gm, member, counts, n as u16, first);
                    if sel == -1 { break; }
                    cursor_row += 1;
                    first = sel;
                    if !(cursor_row < 7) { break; }
                }
                continue;
            } else if key == 0xd5 {
                // up 7 (LAB_1000_f2a0 with key 0xd5)
                for _ in 0..7 {
                    if cursor_row == 4 || cursor_row == 1 {
                        let p = prev_owned(gm, member, counts, n as u16, first);
                        if p == -1 {
                            if cursor_row == 4 {
                                cursor_row -= 1;
                            }
                        } else {
                            first = prev_owned(gm, member, counts, n as u16, first);
                        }
                    } else {
                        cursor_row -= 1;
                    }
                }
                continue;
            } else if key == 0xd6 {
                // down 7
                for _ in 0..7 {
                    if cursor_row == 4 || cursor_row == 7 || shown <= cursor_row {
                        if cur == -1 || next_owned(gm, member, counts, n, cur) == -1 {
                            if cursor_row == 4 && 4 < shown {
                                cursor_row += 1;
                            }
                        } else {
                            first = next_owned(gm, member, counts, n, first);
                            cur = next_owned(gm, member, counts, n, cur);
                        }
                    } else {
                        cursor_row += 1;
                    }
                }
                continue;
            } else {
                continue;
            }
        }
        if select {
            // LAB_1000_f3fc
            if mode == 0x52 {
                done = ready_item(gm, sel, member) as i16;
            } else {
                done = 1;
            }
        }
    }
}

/// orig 1000:f476 (121 bytes). The Ready command: `select_stats_member(0)`; for a valid
/// member, when `next_owned(member, g_inventory, 0x30, -1)` finds nothing prints "Thou art
/// empty-\nhanded!\n" (0x997e); otherwise prints "Item: " (0x9998), clears the status
/// separator, selects window 1, draws the member's name as title and an 8-row list frame,
/// runs `ready_browser('R', member, first)` and finally restores the status area (icon bar,
/// top bar, separator, `draw_status_window`). Vector 1778:06ce (ROOT dispatch_command 'R',
/// COMBAT party_member_turn).
pub fn ready_command(gm: &mut Game) {
    let m = select_stats_member(gm, 0) as i16;
    if -1 < m {
        let first = next_owned(gm, m, 0x57c0u16 as i16, 0x30, -1);
        if first == -1 {
            text::print_string(gm, 0x997e); // "Thou art empty-\nhanded!\n"
        } else {
            text::print_string(gm, 0x9998); // "Item: "
            rest::clear_status_separator(gm);
            text::select_text_window(gm, 1);
            rest::draw_status_title(gm, rec32(m).wrapping_add(0x55a8));
            draw_list_frame(gm, 8);
            ready_browser(gm, 0x52, m as u16, first);
            rest::draw_status_icon_bar(gm);
            rest::draw_status_top_bar(gm);
            rest::draw_status_separator(gm);
            rest::draw_status_window(gm);
        }
    }
}
