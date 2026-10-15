//! SHOPPES.OVL (translated)
//!
//! Shop conversations part 1: the SHOPPE.DAT text expander, greeting/farewell helpers, the
//! guild (keys/gems/torches), reagent sellers, stables (horses), the arms seller (buy menu and
//! the scrolling sell browser) and the healer.  SHOPPES2/SHOPPES3 hold the inn, tavern, ship
//! and remaining shop types.  Entry points are called from TALK `talk_to_shopkeeper`.
//!
//! Unnamed DGROUP globals used throughout (set up by TALK): 0xb114 shop slot (index of
//! g_location within the shop-type town list), 0xb116 shop type, 0xaafc shop name pointer ('#'),
//! 0xaafe shopkeeper name pointer ('$'), 0xab00 item name pointer ('&'), 0xac62 location name
//! pointer ('*'), 0xb118 price ('%'), 0xb11a quantity ('^'), 0xb7fa expansion buffer,
//! 0xb21e (g_ool_buffer) SHOPPE.DAT load area (0x5dc bytes).
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

const SHOP_SLOT: u16 = 0xb114; // i16: town slot within the shop type's list (TALK)
const SHOP_TYPE: u16 = 0xb116; // i16: shop type 0..7 (TALK)
const SHOP_PRICE: u16 = 0xb118; // i16: current price, expanded by '%'
const SHOP_QTY: u16 = 0xb11a; // i16: current quantity, expanded by '^'
const SHOP_NAME_PTR: u16 = 0xaafc; // '#'
const SHOPKEEPER_NAME_PTR: u16 = 0xaafe; // '$'
const SHOP_ITEM_NAME_PTR: u16 = 0xab00; // '&'
const SHOP_LOCATION_NAME_PTR: u16 = 0xac62; // '*'
const SHOP_TEXT_BUF: u16 = 0xb7fa; // expansion buffer of print_shop_text
const SHOPPE_BUF: u16 = 0xb21e; // SHOPPE.DAT load area (g_ool_buffer), 0x5dc bytes

/// MS C `itoa(value, buf, 10)`: writes the signed decimal digits of `value` NUL-terminated at
/// `buf` (a DGROUP scratch buffer).
fn itoa_dec(gm: &mut Game, value: i16, buf: u16) {
    let s = format!("{}", value);
    let mut p = buf;
    for &b in s.as_bytes() {
        gm.set_g8(p, b);
        p = p.wrapping_add(1);
    }
    gm.set_g8(p, 0);
}

/// Price adjustment shared by the guild, reagent, horse and arms purchases: the price at
/// 0xb118 is raised by price * (100 - 3 * intelligence) / 100, i.e. a smarter buyer pays
/// less (negative adjustments go through an UNSIGNED 32-bit divide, so an intelligence above
/// 33 produces the original's garbage low word - kept bit-exact).  `price_hi` is the high
/// word the caller supplies for the price operand (sign-extension or 0).
fn apply_intelligence_adjustment(gm: &mut Game, member: i16, price_hi: i16) {
    let price = gm.gi16(SHOP_PRICE);
    let int3 = (gm.g8((member as u16).wrapping_mul(0x20).wrapping_add(0x55b6)) as u16).wrapping_mul(3);
    let d = int3.wrapping_sub(100) as i16;
    // 32-bit negation of the sign-extended (int*3 - 100)
    let hi = ((d != 0) as u16).wrapping_sub((int3 < 100) as u16).wrapping_neg();
    let prod = rest::__aFlmul(gm, d.wrapping_neg() as u16, hi as i16, price as u16, price_hi);
    let q = rest::__aFuldiv(gm, prod as u16, (prod >> 16) as u16, 100, 0);
    gm.set_gi16(SHOP_PRICE, price.wrapping_add(q as i16));
}

/// orig 1000:a290 (37 bytes). Ghidra order (src, dst). strcpy that returns a pointer to the
/// terminating NUL written in `dst` (so consecutive copies concatenate).
pub fn str_copy_ret_end(gm: &mut Game, param_1: u16, param_2: u16) -> u16 {
    let mut src = param_1;
    let mut dst = param_2;
    loop {
        let c = gm.g8(src);
        src = src.wrapping_add(1);
        gm.set_g8(dst, c);
        if c == 0 { break; }
        dst = dst.wrapping_add(1);
    }
    dst
}

/// orig 1000:a2b6 (333 bytes). Expands a shop text into 0xb7fa and prints it. Bytes >= 0x80
/// are compressed-word indices (pointer table 0x23ea + 2*byte, i.e. g_tlk_word_ptrs; emitted with
/// a leading space and a trailing space when a plain character follows); '#' shop name
/// (0xaafc), '$' shopkeeper name (0xaafe), '&' item name (0xab00), '*' location name
/// (0xac62), '%' price (0xb118, decimal), '^' quantity (0xb11a), '@' "morning" (hour < 12),
/// "afternoon" (< 18) or "evening". Plain text runs are copied verbatim (the source is
/// temporarily NUL-terminated in place). The expanded buffer goes through print_string.
pub fn print_shop_text(gm: &mut Game, param_1: u16) {
    gm.with_buf(8, |gm, num| {
        let mut lit = param_1; // start of the pending literal run
        let mut out = SHOP_TEXT_BUF;
        let mut p = param_1;
        loop {
            let c = gm.g8(p);
            if c == 0 { break; }
            let special = matches!(c, 0x23 | 0x24 | 0x40 | 0x25 | 0x5e | 0x26 | 0x2a) || 0x7f < c;
            if special {
                gm.set_g8(p, 0);
                out = str_copy_ret_end(gm, lit, out);
                gm.set_g8(p, c);
                lit = p.wrapping_add(1);
            }
            p = p.wrapping_add(1);
            if c < 0x80 {
                let src: u16 = match c {
                    0x25 => { let v = gm.gi16(SHOP_PRICE); itoa_dec(gm, v, num); num }
                    0x23 => gm.g16(SHOP_NAME_PTR),
                    0x24 => gm.g16(SHOPKEEPER_NAME_PTR),
                    0x26 => gm.g16(SHOP_ITEM_NAME_PTR),
                    0x2a => gm.g16(SHOP_LOCATION_NAME_PTR),
                    0x40 => {
                        let h = gm.g8(g::HOUR);
                        if h < 0xc { 0x7826 } // "morning"
                        else if h < 0x12 { 0x782e } // "afternoon"
                        else { 0x7838 } // "evening"
                    }
                    0x5e => { let v = gm.gi16(SHOP_QTY); itoa_dec(gm, v, num); num }
                    _ => continue,
                };
                out = str_copy_ret_end(gm, src, out);
                continue;
            }
            gm.set_g8(out, 0x20);
            let word = gm.g16((c as u16).wrapping_mul(2).wrapping_add(0x23ea));
            out = str_copy_ret_end(gm, word, out.wrapping_add(1));
            let n = gm.g8(p);
            if n != 0 && n < 0x80 {
                gm.set_g8(out, 0x20);
                out = out.wrapping_add(1);
            }
        }
        // copy the remaining literal run including its NUL
        let len = util::str_len(gm, gm.ds(lit)) as u16;
        let mut n = len.wrapping_add(1);
        let mut s = lit;
        while n != 0 {
            let b = gm.g8(s);
            gm.set_g8(out, b);
            s = s.wrapping_add(1);
            out = out.wrapping_add(1);
            n -= 1;
        }
    });
    text::print_string(gm, SHOP_TEXT_BUF);
}

/// orig 1000:a40a (32 bytes). Loads 0x5dc bytes of SHOPPE.DAT from file offset `offset` into
/// 0xb21e and prints the (NUL-terminated) text there with print_shop_text.
pub fn print_shoppe_dat(gm: &mut Game, param_1: u16) {
    files::load_file_region(gm, param_1, 0x5dc, SHOPPE_BUF, 0x7840); // "SHOPPE.DAT"
    print_shop_text(gm, SHOPPE_BUF);
}

/// orig 1000:a42a (27 bytes). Called after every payment: when the Shadowlord present in town
/// (g_save_unk_3b2 = g_shadowlord_present) is 0 (Faulinei, Falsehood) the shopkeeper cheats
/// the party out of a further 1..64 gold (floored at 0).
pub fn shadowlord_cheat_gold(gm: &mut Game) {
    if gm.gi8(g::SAVE_UNK_3B2) == 0 {
        let v = util::rand_range(gm, 0x40, 1);
        rest::sub_i16_floor(gm, v, g::GOLD);
    }
}

/// orig 1000:a446 (75 bytes). Prints the opening quote and one of the four random greetings of
/// the current shop (SHOPPE.DAT offsets at 0x3b2a + type*8 + rand*2), then "\n\n:" when the
/// cursor is at column >= 12, "\n:" at column 0, or a single space otherwise.
pub fn print_greeting(gm: &mut Game) {
    text::put_char(gm, 0x22);
    let r = util::rand_range(gm, 3, 0);
    let off = gm.g16((gm.gi16(SHOP_TYPE) as u16).wrapping_mul(8).wrapping_add((r as u16).wrapping_mul(2)).wrapping_add(0x3b2a));
    print_shoppe_dat(gm, off);
    let s = if text::get_cursor_col(gm) < 0xc {
        if text::get_cursor_col(gm) != 0 {
            text::put_char(gm, 0x20);
            return;
        }
        0x7850 // "\n:"
    } else {
        0x784c // "\n\n:"
    };
    text::print_string(gm, s);
}

/// orig 1000:a492 (125 bytes). Farewell: `bought` 0 prints "\n\n\"" and a random "nothing
/// bought" line (SHOPPE.DAT offsets 0x3b6a + type*8 + rand*2), 1 prints "\n\n\"" and a random
/// "thanks for buying" line (0x3baa table); both then end with "says $.\n" (newline first when
/// the cursor is not at column 0). Any other value prints nothing.
pub fn print_farewell(gm: &mut Game, param_1: i16) {
    let bought = param_1;
    if bought == 0 {
        text::print_string(gm, 0x7854); // "\n\n\""
        let r = util::rand_range(gm, 3, 0);
        let off = gm.g16((gm.gi16(SHOP_TYPE) as u16).wrapping_mul(8).wrapping_add((r as u16).wrapping_mul(2)).wrapping_add(0x3b6a));
        print_shoppe_dat(gm, off);
    } else if bought == 1 {
        text::print_string(gm, 0x7858); // "\n\n\""
        let r = util::rand_range(gm, 3, 0);
        let off = gm.g16((gm.gi16(SHOP_TYPE) as u16).wrapping_mul(8).wrapping_add((r as u16).wrapping_mul(2)).wrapping_add(0x3baa));
        print_shoppe_dat(gm, off);
    }
    if bought == 0 || bought == 1 {
        if text::get_cursor_col(gm) != 0 {
            text::put_char(gm, 10);
        }
        print_shop_text(gm, 0x785c); // "says $.\n"
    }
}

/// orig 1000:a510 (58 bytes). Waits for 'Y' or 'N' (command keys), echoes "Yes"/"No" and
/// returns the key (0x59 / 0x4e). Used by SHOPPES3 (vector 1778:07e2).
pub fn ask_yes_no(gm: &mut Game) -> i16 {
    loop {
        let k = input::read_command_key(gm);
        if k == 0x4e {
            text::print_string(gm, 0x7866); // "No"
        } else if k == 0x59 {
            text::print_string(gm, 0x786a); // "Yes"
        }
        if k == 0x4e || k == 0x59 {
            return k;
        }
    }
}

/// orig 1000:a54a (311 bytes). Ghidra order (result, item, member). Guild purchase of item
/// 0 keys (3 for the price), 1 gems (4), 2 torches (5): price = 0x3bea[slot*4 + item] with
/// the intelligence adjustment, the item's pitch from SHOPPE.DAT (offset 0x3c0a[item]),
/// "Interested?"; 'N' -> "What else, then?" returns `result` unchanged; 'Y' with enough gold
/// pays (shadowlord cheat, status window), adds the goods (max 99), prints "Sold!" and
/// "What else, m'lady/m'lord?" (gender glyph 0x55b1 == 0xc is female) and returns 1; without
/// the gold prints SHOPPE.DAT 0x21e6 ("Death comes always to those who cheat the Guild!") and
/// returns -1.
pub fn guild_buy_item(gm: &mut Game, param_1: u16, param_2: i16, param_3: i16) -> u16 {
    let (mut result, item, member) = (param_1, param_2, param_3);
    let price = gm.gi16((gm.gi16(SHOP_SLOT) as u16).wrapping_mul(8).wrapping_add((item as u16).wrapping_mul(2)).wrapping_add(0x3bea));
    gm.set_gi16(SHOP_PRICE, price);
    apply_intelligence_adjustment(gm, member, price >> 15);
    text::print_string(gm, 0x786e); // "\n\n\""
    let off = gm.g16((item as u16).wrapping_mul(2).wrapping_add(0x3c0a));
    print_shoppe_dat(gm, off);
    text::print_string(gm, 0x7872); // "\n\nInterested?\" "
    loop {
        let c = input::read_command_key(gm) as u8;
        if c == b'N' {
            text::print_string(gm, 0x7882); // "No\n\n\"What else, then?\n\n"
        } else if c == b'Y' {
            text::print_string(gm, 0x789a); // "Yes\n"
            let price = gm.gi16(SHOP_PRICE);
            if price <= gm.gi16(g::GOLD) {
                let gold = gm.gi16(g::GOLD).wrapping_sub(price);
                gm.set_gi16(g::GOLD, gold);
                shadowlord_cheat_gold(gm);
                rest::draw_status_window(gm);
                if item == 0 {
                    rest::add_u8_capped(gm, 99, 3, g::KEYS);
                } else if item == 1 {
                    rest::add_u8_capped(gm, 99, 4, g::GEMS);
                } else if item == 2 {
                    rest::add_u8_capped(gm, 99, 5, g::TORCHES);
                }
                result = 1;
                print_shop_text(gm, 0x78a0); // "\n\"Sold!\"\nsays $.\n\n\"What else, \n"
                let s = if gm.gi8((member as u16).wrapping_mul(0x20).wrapping_add(0x55b1)) == 0xc { 0x78c0 } else { 0x78c8 }; // "m'lady" / "m'lord"
                text::print_string(gm, s);
                text::print_string(gm, 0x78d0); // "?\n\n"
            } else {
                print_shoppe_dat(gm, 0x21e6); // "Death comes always to those who cheat the Guild!" warns $.
                result = 0xffff;
            }
        }
        if c == b'N' || c == b'Y' {
            return result;
        }
    }
}

/// orig 1000:a686 (168 bytes). Guild menu loop: prints "a.........Keys / b.........Gems /
/// c......Torches / Thy concern?" and reads command keys: 'A'..'C' echo the lower-case letter
/// and run guild_buy_item (a -1 result ends the menu), 'D' prints SHOPPE.DAT 0x2215 (the
/// "limited time" offer), space/Esc end the menu; other keys are ignored. Returns the last
/// purchase result (0 nothing, 1 bought, -1 could not pay).
pub fn guild_menu(gm: &mut Game, param_1: u16) -> i16 {
    let member = param_1;
    let mut done = false;
    let mut result: i16 = 0;
    loop {
        if done {
            return result;
        }
        text::print_string(gm, 0x78d4); // "a.........Keys\n"
        text::print_string(gm, 0x78e4); // "b.........Gems\n"
        text::print_string(gm, 0x78f4); // "c......Torches\n\n"
        text::print_string(gm, 0x7906); // "Thy concern?\" "
        loop {
            let k = input::read_command_key(gm) as u8;
            let mut key = k;
            if k == 0x20 {
                done = true;
            } else if k < 0x21 {
                if k == 0x1b {
                    done = true;
                } else {
                    key = 0;
                }
            } else if 0x40 < k {
                if k < 0x44 {
                    text::put_char(gm, k | 0x20);
                    result = guild_buy_item(gm, result as u16, (k as i16).wrapping_sub(0x41), member as i16) as i16;
                    if result == -1 {
                        done = true;
                    }
                } else if k == 0x44 {
                    print_shoppe_dat(gm, 0x2215); // "That offer is for a limited time only."
                } else {
                    key = 0;
                }
            } else {
                key = 0;
            }
            if !(key == 0 && !done) { break; }
        }
    }
}

/// orig 1000:a732 (94 bytes). Guild shop (type 5) entry: greeting, then 'Y' prints "Yes\n\n\"We
/// sell:\n\n" and runs guild_menu, space/'N' prints "No"; ends with print_farewell(result).
pub fn guild_shop(gm: &mut Game, param_1: u16) {
    let member = param_1;
    let mut result: i16 = 0;
    print_greeting(gm);
    let mut done = false;
    loop {
        let c = input::read_command_key(gm) as u8;
        if c == b' ' || c == b'N' {
            text::print_string(gm, 0x7928); // "No"
            done = true;
        } else if c == b'Y' {
            text::print_string(gm, 0x7916); // "Yes\n\n\"We sell:\n\n"
            result = guild_menu(gm, member);
            done = true;
        }
        if done {
            print_farewell(gm, result);
            return;
        }
    }
}

/// orig 1000:a792 (354 bytes). Ghidra order (result, choice, member). Reagent purchase: finds
/// the `choice`-th reagent (0-based) stocked by this shop (price byte 0x3a32[slot*8 + i] != 0);
/// none -> returns -2 (invalid letter). Holding 99 already -> "Thou canst not carry any more!"
/// and a key wait. Otherwise the price (with the intelligence adjustment) and the quantity
/// (0x3a5a[slot*8 + i]) are set, the reagent name (0x97b7 + 0x3c10[i]) is printed with "Is
/// this thy need?"; 'N' -> "What else?"; 'Y' pays (quantity added, max 99, "I thank thee!",
/// "Anything else?") and returns 1, or prints the "profaneth my shoppe with an empty purse"
/// text at 0xb6e2 (inside the loaded SHOPPE.DAT block) and returns -1.
pub fn reagent_buy_item(gm: &mut Game, param_1: u16, param_2: i16, param_3: i16) -> u16 {
    let (mut result, choice, member) = (param_1, param_2, param_3);
    let slot = gm.gi16(SHOP_SLOT) as u16;
    let mut n: i16 = 0;
    let mut i: i16 = 0;
    loop {
        if gm.gi8(slot.wrapping_mul(8).wrapping_add(i as u16).wrapping_add(0x3a32)) != 0 {
            if n == choice { break; }
            n += 1;
        }
        i += 1;
        if !(i < 8) { break; }
    }
    if i == 8 {
        result = 0xfffe;
    } else if gm.gi8((i as u16).wrapping_add(g::REAGENTS)) == 99 {
        text::print_string(gm, 0x792c); // "\n\n\"Thou canst not carry any more!\"\n\n"
        input::read_command_key(gm);
    } else {
        let idx = slot.wrapping_mul(8).wrapping_add(i as u16);
        let price = gm.g8(idx.wrapping_add(0x3a32)) as u16;
        gm.set_g16(SHOP_PRICE, price);
        apply_intelligence_adjustment(gm, member, 0);
        let qty = gm.g8(idx.wrapping_add(0x3a5a)) as u16;
        gm.set_g16(SHOP_QTY, qty);
        text::print_string(gm, 0x7952); // "\n\n\""
        let name = gm.g16((i as u16).wrapping_mul(2).wrapping_add(0x3c10)).wrapping_add(0x97b7); // "Spells" + 1 + offset
        print_shop_text(gm, name);
        text::print_string(gm, 0x7956); // " Is this thy need?\" "
        if 0xc < text::get_cursor_col(gm) {
            text::print_string(gm, 0x796c); // "\n:"
        }
        loop {
            let c = input::read_command_key(gm) as u8;
            if c == b'N' {
                text::print_string(gm, 0x7970); // "No\n\n\"What else?\n\n"
            } else if c == b'Y' {
                text::print_string(gm, 0x7982); // "Yes\n"
                let price = gm.gi16(SHOP_PRICE);
                if price <= gm.gi16(g::GOLD) {
                    let gold = gm.gi16(g::GOLD).wrapping_sub(price);
                    gm.set_gi16(g::GOLD, gold);
                    shadowlord_cheat_gold(gm);
                    rest::draw_status_window(gm);
                    let qty = gm.gi16(SHOP_QTY);
                    rest::add_u8_capped(gm, 99, qty, (i as u16).wrapping_add(g::REAGENTS));
                    result = 1;
                    print_shop_text(gm, 0x7988); // "\n\"I thank thee!\"\nsays $.\n"
                    text::print_string(gm, 0x79a2); // "\"Anything else?\n\n"
                } else {
                    print_shop_text(gm, 0xb6e2); // loaded block + 0x4c4: "Thou profaneth my shoppe with thine empty purse! OUT!"
                    result = 0xffff;
                }
            }
            if !(c != b'N' && c != b'Y') { break; }
        }
    }
    result
}

/// orig 1000:a8f6 (246 bytes). Reagent menu loop: loads the reagent-shop text block
/// (SHOPPE.DAT 0x1a67) and repeatedly lists the stocked reagents as "A...name" (names
/// 0x3c20[i]) followed by "Thy interest?"; 'A'..'E' run reagent_buy_item (-1 ends the menu
/// with result -1, -2 is ignored), Enter/space/Esc end it. Returns the last result.
pub fn reagent_menu(gm: &mut Game, param_1: u16) -> i16 {
    let member = param_1;
    let mut done = false;
    let mut result: i16 = 0;
    files::load_file_region(gm, 0x1a67, 0x5dc, SHOPPE_BUF, 0x7a0a); // "SHOPPE.DAT"
    loop {
        if done {
            return result;
        }
        let mut letter: u8 = b'A';
        let mut i: i16 = 0;
        loop {
            if gm.gi8((gm.gi16(SHOP_SLOT) as u16).wrapping_mul(8).wrapping_add(i as u16).wrapping_add(0x3a32)) != 0 {
                text::put_char(gm, letter);
                text::print_string(gm, 0x7a16); // "..."
                let name = gm.g16((i as u16).wrapping_mul(2).wrapping_add(0x3c20));
                text::print_string(gm, name);
                text::put_char(gm, 10);
                letter = letter.wrapping_add(1);
            }
            i += 1;
            if !(i < 8) { break; }
        }
        text::print_string(gm, 0x7a1a); // "\nThy interest?\" "
        loop {
            let k = input::read_command_key(gm) as u8;
            let mut key = k;
            let mut r = result;
            if k == 0x1b {
                done = true;
            } else if k < 0x1c {
                if k != 0xd {
                    key = 0;
                } else {
                    done = true;
                }
            } else if k == 0x20 {
                done = true;
            } else if k < 0x41 || 0x45 < k {
                key = 0;
            } else {
                r = reagent_buy_item(gm, result as u16, (k as i16).wrapping_sub(0x41), member as i16) as i16;
                if r == -1 {
                    result = -1;
                    done = true;
                    r = result;
                } else if r == -2 {
                    key = 0;
                    r = result;
                }
            }
            result = r;
            if !(key == 0 && !done) { break; }
        }
    }
}

/// orig 1000:a9ee (94 bytes). Reagent shop (type 4) entry: greeting, 'Y' prints "Yes\n\n\"Fine!
/// We sell:\n\n" and runs reagent_menu, space/'N' prints "No"; ends with print_farewell.
pub fn reagent_shop(gm: &mut Game, param_1: u16) {
    let member = param_1;
    let mut result: i16 = 0;
    print_greeting(gm);
    let mut done = false;
    loop {
        let c = input::read_command_key(gm) as u8;
        if c == b' ' || c == b'N' {
            text::print_string(gm, 0x7a44); // "No"
            done = true;
        } else if c == b'Y' {
            text::print_string(gm, 0x7a2c); // "Yes\n\n\"Fine! We sell:\n\n"
            result = reagent_menu(gm, member);
            done = true;
        }
        if done {
            print_farewell(gm, result);
            return;
        }
    }
}

/// orig 1000:aa4e (487 bytes). Stables (shop type 2). Finds the first free g_monster_table
/// slot and the first of the four neighbouring cells (offsets 0x3c38/0x3c40: N, S, E, W) that
/// holds no object and is tile 'D' (0x44), 'E' (0x45) or 5; no slot or no cell -> "The stables
/// are closed." Otherwise price = 0x3c30[slot] with the intelligence adjustment, greeting;
/// space/'N' -> "No"; 'Y' prints the sales pitch (SHOPPE.DAT 0x1643) and "Deal?": 'N' -> "No",
/// 'Y' -> "Yes!" then either "Thou couldst not afford to feed it!" (result -1) or the gold is
/// paid and a horse object (tile 0x10, frame 0x10, value1/value2/value3 = 0) is placed in the
/// slot at the free cell on the party's level; the map and status window are redrawn (result
/// 1). Ends with print_farewell(result).
pub fn horse_shop(gm: &mut Game, param_1: i16) {
    let member = param_1;
    let mut n: i16 = 0;
    let mut p: u16 = g::MONSTER_TABLE;
    loop {
        if gm.g8(p) == 0 { break; }
        p = p.wrapping_add(8);
        n += 1;
        if !(p < 0x5d5a) { break; }
    }
    let mut k: i16 = 0;
    let mut dxp: u16 = 0x3c38;
    let mut dyp: u16 = 0x3c40;
    let mut x: i16 = 0;
    let mut y: i16 = 0;
    loop {
        x = gm.gi16(dxp).wrapping_add(gm.g8(g::PARTY_X) as i16);
        y = gm.gi16(dyp).wrapping_add(gm.g8(g::PARTY_Y) as i16);
        let tp = rest::get_map_tile(gm, y as u16, x as u16);
        let tile = gm.gi8(tp);
        let obj = rest::find_object_at(gm, gm.g8(g::PARTY_Z) as u16, y as u16, x as u16);
        if obj == 0 && (tile == 0x44 || tile == 0x45 || tile == 5) { break; }
        dxp = dxp.wrapping_add(2);
        dyp = dyp.wrapping_add(2);
        k += 1;
        if !(dyp < 0x3c48) { break; }
    }
    if k == 4 || n == 0x20 {
        text::print_string(gm, 0x7a48); // "The stables are closed.\n"
        return;
    }
    let mut done = false;
    let mut result: i16 = 0;
    let price = gm.gi16((gm.gi16(SHOP_SLOT) as u16).wrapping_mul(2).wrapping_add(0x3c30));
    gm.set_gi16(SHOP_PRICE, price);
    apply_intelligence_adjustment(gm, member, price >> 15);
    print_greeting(gm);
    let base = (n as u16).wrapping_mul(8);
    loop {
        let mut c = input::read_command_key(gm) as u8;
        if c == b' ' || c == b'N' {
            text::print_string(gm, 0x7ab2); // "No"
            done = true;
        } else if c == b'Y' {
            text::print_string(gm, 0x7a62); // "Yes\n\n\""
            print_shoppe_dat(gm, 0x1643); // "Thou canst hardly find better horses at lower prices. % gp is truly unworthy of these magnificent creatures!"
            text::print_string(gm, 0x7a6a); // "\n\nDeal?\" "
            loop {
                c = input::read_command_key(gm) as u8;
                if c == b'Y' { break; }
                if c == b'N' { break; }
            }
            if c == b'N' {
                text::print_string(gm, 0x7a74); // "No"
            } else {
                text::print_string(gm, 0x7a78); // "Yes!"
                let price = gm.gi16(SHOP_PRICE);
                if gm.gi16(g::GOLD) < price {
                    text::print_string(gm, 0x7a7e); // "\n\n\"Thou couldst not afford to "
                    print_shop_text(gm, 0x7a9e); // "feed it!\"\nyells $.\n"
                    result = -1;
                } else {
                    let gold = gm.gi16(g::GOLD).wrapping_sub(price);
                    gm.set_gi16(g::GOLD, gold);
                    shadowlord_cheat_gold(gm);
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(base).wrapping_add(5), 0); // value1
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(base).wrapping_add(7), 0); // value3
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(base).wrapping_add(6), 0); // value2 (animation state)
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(base).wrapping_add(1), 0x10); // frame tile: horse
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(base), 0x10); // base tile: horse
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(base).wrapping_add(2), x as u8);
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(base).wrapping_add(3), y as u8);
                    let z = gm.g8(g::PARTY_Z);
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(base).wrapping_add(4), z);
                    rest::refresh_map_view(gm);
                    rest::draw_status_window(gm);
                    result = 1;
                }
            }
            done = true;
        }
        if done { break; }
    }
    print_farewell(gm, result);
}

/// orig 1000:ac3c (386 bytes). Ghidra order (result, item, member). Arms purchase of
/// inventory item `item`: price = 0x3a82[item] with the intelligence adjustment, the item's
/// pitch from SHOPPE.DAT (offset 0x3c48[item]), a random "Wouldst thou buy one?" line
/// (0x3ca6) and the closing quote. 'N' -> "No"; 'Y' -> "Yes": holding 99 -> "Thou canst not
/// carry any more!" says $. and a key wait; not enough gold -> a random insult (0x3cae) "yells
/// $." and result -1; otherwise the gold is paid (shadowlord cheat, status window), arrows
/// (0x1b) and quarrels (0x1d) are set to 99, anything else +1 (cap 99), "Sold!" and result 1.
/// Unless the result is -1, "Anything else," + "then?" (nothing bought yet) / "milady?" /
/// "sir?" is printed. Returns the result.
pub fn arms_buy_item(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16) -> i16 {
    let (mut result, item, member) = (param_1, param_2, param_3);
    let price = gm.gi16((item as u16).wrapping_mul(2).wrapping_add(0x3a82));
    gm.set_gi16(SHOP_PRICE, price);
    apply_intelligence_adjustment(gm, member, price >> 15);
    text::print_string(gm, 0x7b5e); // "\n\n\""
    let off = gm.g16((item as u16).wrapping_mul(2).wrapping_add(0x3c48));
    print_shoppe_dat(gm, off);
    text::print_string(gm, 0x7b62); // "\n\n"
    let r = util::rand_range(gm, 3, 0);
    let s = gm.g16((r as u16).wrapping_mul(2).wrapping_add(0x3ca6)); // "Wouldst thou buy one?" ...
    text::print_string(gm, s);
    text::print_string(gm, 0x7b66); // "\" "
    loop {
        let c = input::read_command_key(gm) as u8;
        if c == b'N' {
            text::print_string(gm, 0x7b6a); // "No\n\n"
        } else if c == b'Y' {
            text::print_string(gm, 0x7b70); // "Yes\n"
            if gm.gi8(g::INVENTORY.wrapping_add(item as u16)) == 99 {
                text::print_string(gm, 0x7b76); // "\n\"Thou canst not carry any more!\"\n"
                print_shop_text(gm, 0x7b9a); // "says $.\n\n"
                input::read_command_key(gm);
            } else if gm.gi16(g::GOLD) < gm.gi16(SHOP_PRICE) {
                text::print_string(gm, 0x7ba4); // "\n\""
                let r = util::rand_range(gm, 3, 0);
                let s = gm.g16((r as u16).wrapping_mul(2).wrapping_add(0x3cae)); // "Can't pay?! Out with ye, orc-face!" ...
                text::print_string(gm, s);
                print_shop_text(gm, 0x7ba8); // "\"\nyells $.\n"
                result = -1;
            } else {
                let gold = gm.gi16(g::GOLD).wrapping_sub(gm.gi16(SHOP_PRICE));
                gm.set_gi16(g::GOLD, gold);
                shadowlord_cheat_gold(gm);
                rest::draw_status_window(gm);
                if item == 0x1b || item == 0x1d {
                    gm.set_g8(g::INVENTORY.wrapping_add(item as u16), 99);
                } else {
                    rest::add_u8_capped(gm, 99, 1, g::INVENTORY.wrapping_add(item as u16));
                }
                print_shop_text(gm, 0x7bb4); // "\nSold!\n"
                result = 1;
            }
        }
        if !(c != b'N' && c != b'Y') { break; }
    }
    if result != -1 {
        text::print_string(gm, 0x7bbc); // "\"Anything else,\n"
        let s = if result == 0 {
            0x7bdc // "then?"
        } else if gm.gi8((member as u16).wrapping_mul(0x20).wrapping_add(0x55b1)) == 0xc {
            0x7bce // "milady?"
        } else {
            0x7bd6 // "sir?"
        };
        text::print_string(gm, s);
    }
    result
}

/// orig 1000:adc0 (291 bytes). Arms "Buy" menu loop: lists the shop's stock (0x3ae2[slot*8..],
/// 0xff-terminated, up to 8) as "a...Name" using the long name (0x17f6) when shorter than 13
/// characters, else the short name (0x1962), each on its own line, then a blank line, a random
/// "What may I show thee?" line (0x3cb6) and the closing quote. Letters within the stock echo
/// in lower case and run arms_buy_item (-1 ends the loop); Esc/space end it; other keys are
/// ignored; after a purchase the list is reprinted. Returns the last result.
pub fn arms_buy_menu(gm: &mut Game, param_1: u16) -> i16 {
    let member = param_1;
    let mut done = false;
    let mut result: i16 = 0;
    loop {
        text::print_string(gm, 0x7c44); // "\n\n"
        let mut n: i16 = 0;
        loop {
            let id = gm.g8((gm.gi16(SHOP_SLOT) as u16).wrapping_mul(8).wrapping_add(n as u16).wrapping_add(0x3ae2)) as u16;
            if id == 0xff { break; }
            text::put_char(gm, (n as u8).wrapping_add(0x61));
            text::print_string(gm, 0x7c48); // "..."
            let long = gm.g16(id.wrapping_mul(2).wrapping_add(0x17f6));
            let len = util::str_len(gm, gm.ds(long));
            let s = if len < 0xd { long } else { gm.g16(id.wrapping_mul(2).wrapping_add(0x1962)) };
            text::print_string(gm, s);
            if text::get_cursor_col(gm) != 0 {
                text::put_char(gm, 10);
            }
            n += 1;
            if !(n < 8) { break; }
        }
        text::put_char(gm, 10);
        let r = util::rand_range(gm, 3, 0);
        let s = gm.g16((r as u16).wrapping_mul(2).wrapping_add(0x3cb6)); // "What may I show thee?" ...
        text::print_string(gm, s);
        text::print_string(gm, 0x7c4c); // "\" "
        loop {
            let k = input::read_command_key(gm) as u8;
            let mut key = k;
            let i = (k as i16).wrapping_sub(0x41);
            if i < n && -1 < i {
                text::put_char(gm, k.wrapping_add(0x20));
                let id = gm.g8((gm.gi16(SHOP_SLOT) as u16).wrapping_mul(8).wrapping_add(i as u16).wrapping_add(0x3ae2));
                result = arms_buy_item(gm, result, id as i16, member as i16);
                if result == -1 {
                    done = true;
                }
            } else if k == 0x1b || k == 0x20 {
                done = true;
            } else {
                key = 0;
            }
            if !(key == 0 && !done) { break; }
        }
        if done {
            return result;
        }
    }
}

/// orig 1000:aee8 (40 bytes). Returns 1 when any of the 0x30 arms/armour inventory counts
/// (g_inventory[0..0x2f]) is non-zero, else 0.
pub fn has_sellable_arms(gm: &mut Game) -> u16 {
    let mut i: u16 = 0;
    loop {
        if gm.g8(g::INVENTORY.wrapping_add(i)) != 0 {
            return 1;
        }
        i += 1;
        if !(i < 0x30) { break; }
    }
    0
}

/// orig 1000:af10 (498 bytes). Ghidra order (&cursor, &top); the port takes the two locals by
/// reference. Redraws the four-row sell list in text window 1. First the cursor is repaired
/// when its item is no longer owned: if cursor == top it moves to the previous owned item (or
/// the first owned when none) and top follows; otherwise to the next owned item, or, at the
/// end, to the last owned item while top moves back one when possible. Rows 1..4 then show
/// the owned items from `top` (print_item_line with marker '-', short names 0x1962), the
/// cursor line wrapped in 0xfd highlight codes; remaining rows are blanked (0x7c50). The
/// "more" indicator at (6,6): 0x18 more above, 0x19 more below, 0x12 both, or the plain
/// separator bar. Returns the window row of the cursor line (uninitialised in the original
/// when no line matched; 0 here). Window 2 is reselected.
pub fn draw_sell_list(gm: &mut Game, cursor: &mut i16, top: &mut i16) -> u16 {
    let mut c = *cursor;
    if gm.g8(g::INVENTORY.wrapping_add(c as u16)) == 0 {
        if *top == c {
            c = ovl::zstats::prev_owned(gm, 0xff, g::INVENTORY as i16, 0x30, c);
            *cursor = c;
            if c == -1 {
                c = ovl::zstats::next_owned(gm, 0xff, g::INVENTORY as i16, 0x30, -1);
                *cursor = c;
            }
            *top = *cursor;
        } else {
            c = ovl::zstats::next_owned(gm, 0xff, g::INVENTORY as i16, 0x30, *cursor);
            *cursor = c;
            if c == -1 {
                *cursor = ovl::zstats::prev_owned(gm, 0xff, g::INVENTORY as i16, 0x30, 0x30);
                let t = *top;
                let p = ovl::zstats::prev_owned(gm, 0xff, g::INVENTORY as i16, 0x30, t);
                if -1 < p {
                    *top = ovl::zstats::prev_owned(gm, 0xff, g::INVENTORY as i16, 0x30, t);
                }
            }
        }
    }
    text::select_text_window(gm, 1);
    text::set_cursor_pos(gm, 1, 1);
    let mut row: u16 = 0; // TODO(verify): uninitialised in the original when no row matches
    let mut item = *top;
    if item != -1 {
        let cur = *cursor;
        loop {
            if cur == item {
                text::put_char(gm, 0xfd);
                row = text::get_cursor_row(gm) as u16;
            }
            ovl::zstats::print_item_line(gm, 0x2d, 0x1962, g::INVENTORY as i16, item);
            if cur == item {
                text::put_char(gm, 0xfd);
            }
            let r = text::get_cursor_row(gm);
            text::set_cursor_pos(gm, r, 1);
            if text::get_cursor_row(gm) == 5 { break; }
            item = ovl::zstats::next_owned(gm, 0xff, g::INVENTORY as i16, 0x30, item);
            if item == -1 { break; }
        }
    }
    let mut r = text::get_cursor_row(gm);
    while r != 5 {
        text::set_cursor_pos(gm, r, 1);
        text::print_string(gm, 0x7c50); // 13 spaces
        r = r.wrapping_add(1);
    }
    let mut more: i16 = 0;
    if ovl::zstats::prev_owned(gm, 0xff, g::INVENTORY as i16, 0x30, *top) != -1 {
        more = 2;
    }
    if item != -1 && ovl::zstats::next_owned(gm, 0xff, g::INVENTORY as i16, 0x30, item) != -1 {
        more += 1;
    }
    if more == 0 {
        rest::draw_status_separator(gm);
    } else {
        text::set_cursor_pos(gm, 6, 6);
        rest::draw_bar_left_cap(gm);
        let glyph = match more {
            1 => Some(0x19),
            2 => Some(0x18),
            3 => Some(0x12),
            _ => None,
        };
        if let Some(gl) = glyph {
            text::put_char(gm, gl);
            rest::draw_bar_right_cap(gm);
        }
    }
    text::select_text_window(gm, 2);
    row
}

/// orig 1000:b106 (236 bytes). Ghidra order (item, member). Sells one unit of inventory item
/// `item`: arrows (0x1b) / quarrels (0x1d) -> "We don't deal in used ammunition!" growls $.,
/// returns 1 (ends the sell session). Otherwise "\n\n\"": a zero base price (0x3a82[item])
/// -> "That, I cannot buy from thee." says $.; else the offer = intelligence * price * 3 / 100
/// + 1 (16-bit product, unsigned 32-bit divide), the item name for '&' = 0x3cce[item] or the
/// long name 0x17f6[item], a random offer phrase from the loaded SHOPPE.DAT block (0x3cbe[rand
/// 0..7] - 0x59a2 = buffer address) and "Deal?": 'N' -> "No"; 'Y' -> "Yes \"Done!\" says $.",
/// gold += offer (cap 9999), count -= 1 (floor 0). The gold line is redrawn. Returns 0.
pub fn arms_sell_item(gm: &mut Game, param_1: i16, param_2: i16) -> u16 {
    let (item, member) = (param_1, param_2);
    if item == 0x1b || item == 0x1d {
        print_shop_text(gm, 0x7d32); // "\n\n\"We don't deal in used ammunition!\"\ngrowls $.\n"
        return 1;
    }
    text::print_string(gm, 0x7d64); // "\n\n\""
    let i2 = (item as u16).wrapping_mul(2);
    let base = gm.gi16(i2.wrapping_add(0x3a82));
    if base == 0 {
        print_shop_text(gm, 0x7d8c); // "That, I cannot buy from thee.\"\nsays $."
    } else {
        let u = (gm.g8((member as u16).wrapping_mul(0x20).wrapping_add(0x55b6)) as u16).wrapping_mul(base as u16);
        let prod = (u as u32).wrapping_mul(3);
        let q = rest::__aFuldiv(gm, prod as u16, (prod >> 16) as u16, 100, 0);
        gm.set_gi16(SHOP_PRICE, (q as i16).wrapping_add(1));
        let mut name = gm.g16(i2.wrapping_add(0x3cce));
        if name == 0 {
            name = gm.g16(i2.wrapping_add(0x17f6));
        }
        gm.set_g16(SHOP_ITEM_NAME_PTR, name);
        let r = util::rand_range(gm, 7, 0);
        let off = gm.g16((r as u16).wrapping_mul(2).wrapping_add(0x3cbe)).wrapping_sub(0x59a2); // SHOPPE.DAT offset -> 0xb21e buffer
        print_shop_text(gm, off);
        text::print_string(gm, 0x7d68); // "\n\nDeal?\" "
        let mut c;
        loop {
            c = input::read_command_key(gm) as u8;
            if c == b'N' { break; }
            if c == b'Y' { break; }
        }
        if c == b'N' {
            text::print_string(gm, 0x7d72); // "No"
        } else {
            print_shop_text(gm, 0x7d76); // "Yes\n\n\"Done!\"\nsays $."
            let price = gm.gi16(SHOP_PRICE);
            rest::add_i16_capped(gm, 0x270f, price, g::GOLD);
            rest::sub_u8_floor(gm, 1, g::INVENTORY.wrapping_add(item as u16));
        }
        rest::print_gold_line(gm);
    }
    0
}

/// orig 1000:b1f4 (343 bytes). Takes the member (RET 2; the stub lacked it). Arms "Sell"
/// browser. Nothing sellable -> print_nothing_to_sell. Otherwise a random "What dost thou
/// wish to sell?" line (0x3d2e), the "Arms" title in the status area, a 4-row list frame in
/// window 1 (frame glyphs 0x10-0x17), cursor = top = first owned item, draw_sell_list, and the
/// sell-offer text block is loaded (SHOPPE.DAT 0xbc0). Key loop (Ghidra split it into the
/// fragments b34c/b3b4/b41a/b436/b47a/b4c6/b4ce/b4f6): 1/3 move up one, 2/4 down one, PgUp
/// (0xd5) up four, PgDn (0xd6) down four, Home (0xd3) first item, End (0xd4) last item with
/// top three back, Enter/space sell the cursor item (arms_sell_item; afterwards, while arms
/// remain, the list is redrawn and a random "What else can ye offer me?" (0x3d3e) is printed,
/// otherwise the session ends), Esc ends. Moving down past row 4 scrolls top; moving up from
/// top scrolls top. finish_sell_browser(refused) closes the session.
pub fn arms_sell_browser(gm: &mut Game, param_1: u16) {
    let member = param_1;
    if has_sellable_arms(gm) == 0 {
        print_nothing_to_sell(gm);
        return;
    }
    let r = util::rand_range(gm, 3, 0);
    let s = gm.g16((r as u16).wrapping_mul(2).wrapping_add(0x3d2e)); // "Which item wouldst thou like to sell?" ...
    text::print_string(gm, s);
    text::print_string(gm, 0x7ef0); // "\" "
    rest::draw_status_title(gm, 0x7ef4); // "Arms"
    text::select_text_window(gm, 1);
    text::define_text_window(gm, 6, 0x26, 1, 0x18, 1);
    text::put_char(gm, 0xff);
    text::define_text_window(gm, 9, 0x27, 1, 0x18, 1);
    text::put_char(gm, 0x10);
    let mut i = 0xd;
    loop {
        text::put_char(gm, 0x11);
        i -= 1;
        if i == 0 { break; }
    }
    text::put_char(gm, 0x13);
    let mut i: i16 = 1;
    loop {
        text::set_cursor_pos(gm, i, 0);
        text::put_char(gm, 0x17);
        text::set_cursor_pos(gm, i, 0xe);
        text::put_char(gm, 0x17);
        i += 1;
        if !(i < 5) { break; }
    }
    text::put_char(gm, 10);
    text::put_char(gm, 0x14);
    let mut i = 0xd;
    loop {
        text::put_char(gm, 0x15);
        i -= 1;
        if i == 0 { break; }
    }
    text::put_char(gm, 0x16);
    let mut cursor = ovl::zstats::next_owned(gm, 0xff, g::INVENTORY as i16, 0x30, -1);
    let mut top = cursor;
    let mut row = draw_sell_list(gm, &mut cursor, &mut top) as i16;
    let mut refused: u16 = 0;
    let mut done: i16 = 0;
    files::load_file_region(gm, 0xbc0, 0x5dc, SHOPPE_BUF, 0x7efa); // "SHOPPE.DAT"
    loop {
        if done != 0 {
            finish_sell_browser(gm, refused);
            return;
        }
        let key = input::read_command_key(gm) as u8;
        enum Act { Up, Down, Sell, Quit, Home, End, None }
        let act = match key {
            0xd | 0x20 => Act::Sell,
            0x1b => Act::Quit,
            0xd3 => Act::Home,
            0xd4 => Act::End,
            0xd5 | 1 | 3 => Act::Up,
            0xd6 | 2 | 4 => Act::Down,
            _ => Act::None,
        };
        match act {
            Act::None => continue,
            Act::Quit => {
                done = 1; // b4c6
                continue;
            }
            Act::Sell => {
                // b47a
                refused = arms_sell_item(gm, cursor, member as i16);
                if refused == 0 && has_sellable_arms(gm) != 0 {
                    row = draw_sell_list(gm, &mut cursor, &mut top) as i16;
                    text::print_string(gm, 0x7f06); // "\n\n\""
                    let r = util::rand_range(gm, 3, 0);
                    let s = gm.g16((r as u16).wrapping_mul(2).wrapping_add(0x3d3e)); // "What else can ye offer me?" ...
                    text::print_string(gm, s);
                    text::print_string(gm, 0x7f0a); // "\" "
                } else {
                    done = 1;
                }
                continue;
            }
            Act::Home => {
                // b41a
                cursor = ovl::zstats::next_owned(gm, 0xff, g::INVENTORY as i16, 0x30, -1);
                top = cursor;
            }
            Act::End => {
                // b436
                top = ovl::zstats::prev_owned(gm, 0xff, g::INVENTORY as i16, 0x30, 0x30);
                cursor = top;
                let mut n: i16 = 1;
                loop {
                    let p = ovl::zstats::prev_owned(gm, 0xff, g::INVENTORY as i16, 0x30, top);
                    if p == -1 { break; }
                    top = p;
                    n += 1;
                    if 4 <= n { break; }
                }
            }
            Act::Up => {
                // b34c
                let step: i16 = if key == 0xd5 { 4 } else { 1 };
                if 0 < step {
                    let mut counter = step;
                    let mut cur = cursor;
                    loop {
                        let p = ovl::zstats::prev_owned(gm, 0xff, g::INVENTORY as i16, 0x30, cur);
                        if cur == top {
                            if -1 < p {
                                top = p;
                                cur = p;
                            }
                        } else {
                            cur = p;
                        }
                        counter -= 1;
                        if counter == 0 { break; }
                    }
                    cursor = cur;
                }
            }
            Act::Down => {
                // b3b4
                let step: i16 = if key == 0xd6 { 4 } else { 1 };
                if 0 < step {
                    let mut counter = step;
                    loop {
                        let p = ovl::zstats::next_owned(gm, 0xff, g::INVENTORY as i16, 0x30, cursor);
                        if -1 < p {
                            cursor = p;
                            row += 1;
                            if 4 < row {
                                top = ovl::zstats::next_owned(gm, 0xff, g::INVENTORY as i16, 0x30, top);
                            }
                        }
                        counter -= 1;
                        if counter == 0 { break; }
                    }
                }
            }
        }
        // b3a2: redraw
        row = draw_sell_list(gm, &mut cursor, &mut top) as i16;
    }
}

/// orig 1000:b34c (202 bytes). Fragment of arms_sell_browser (keys 1/3/PgUp: move the cursor
/// up, scrolling `top` when the cursor is on it); inlined there.
pub fn fn_1000_b34c_fragment(gm: &mut Game) {}

/// orig 1000:b41a (28 bytes). Fragment of arms_sell_browser (Home: cursor = top = first owned
/// item); inlined there.
pub fn fn_1000_b41a_fragment(gm: &mut Game) {}

/// orig 1000:b436 (67 bytes). Fragment of arms_sell_browser (End: cursor = last owned item, top
/// up to three entries before it); inlined there.
pub fn fn_1000_b436_fragment(gm: &mut Game) {}

/// orig 1000:b47a (75 bytes). Fragment of arms_sell_browser (Enter/space: sell the cursor item,
/// redraw and ask for more while arms remain); inlined there.
pub fn fn_1000_b47a_fragment(gm: &mut Game) {}

/// orig 1000:b4c6 (8 bytes). Fragment of arms_sell_browser (Esc: set the done flag); inlined
/// there. (Catalog name COMBAT_b4c6: vector 1778:0602 belongs to COMBAT's copy of the range.)
pub fn fn_1000_b4c6_fragment(gm: &mut Game) {}

/// orig 1000:b4ce (40 bytes). Fragment of arms_sell_browser (dispatch of keys > 0xd: Esc,
/// space, Home, End, PgUp, PgDn); inlined there.
pub fn fn_1000_b4ce_fragment(gm: &mut Game) {}

/// orig 1000:b4f6 (59 bytes). Tail of arms_sell_browser, given its `refused` local: redraws the
/// status window and top bar; unless the session ended with the ammunition refusal prints
/// "\n\n\"" + a random farewell (0x3d36: "Good-bye...", ...) + "\"\n" and, while arms remain,
/// "says $.\n".
pub fn finish_sell_browser(gm: &mut Game, refused: u16) {
    rest::draw_status_window(gm);
    rest::draw_status_top_bar(gm);
    if refused == 0 {
        text::print_string(gm, 0x7f0e); // "\n\n\""
        let r = util::rand_range(gm, 3, 0);
        let s = gm.g16((r as u16).wrapping_mul(2).wrapping_add(0x3d36));
        text::print_string(gm, s);
        text::print_string(gm, 0x7f12); // "\"\n"
        if has_sellable_arms(gm) != 0 {
            print_shop_text(gm, 0x7f16); // "says $.\n"
        }
    }
}

/// orig 1000:b532 (15 bytes). Tail of arms_sell_browser: "Thou hast nothing to sell!" growls $.
pub fn print_nothing_to_sell(gm: &mut Game) {
    print_shop_text(gm, 0x7f20); // "Thou hast nothing to sell!\"\ngrowls $.\n"
}

/// orig 1000:b542 (198 bytes). Arms seller (shop type 0) entry from TALK: "\"Good @, and
/// welcome to #!\"", a key wait, "$ says, \"" + one of two greetings (0x3d46) + "\" "; then
/// 'B' -> "Buy", a random "Very good!" (0x3d4a) and "We have:" (0x3d52) line and arms_buy_menu;
/// 'S' -> "Sell", arms_sell_browser and result -1 (no farewell); space -> "No". Ends with
/// print_farewell(result).
pub fn arms_shop(gm: &mut Game, param_1: u16) {
    let member = param_1;
    let mut result: u16 = 0;
    print_shop_text(gm, 0x8018); // "\"Good @, and welcome to #!\"\n"
    input::read_command_key(gm);
    print_shop_text(gm, 0x8036); // "\n$ says,\n\""
    let r = util::rand_range(gm, 1, 0);
    let s = gm.g16((r as u16).wrapping_mul(2).wrapping_add(0x3d46)); // "Hail, friend! Wouldst thou Buy or Sell?" ...
    text::print_string(gm, s);
    text::print_string(gm, 0x8042); // "\" "
    loop {
        let c = input::read_command_key(gm) as u8;
        if c == b' ' {
            text::print_string(gm, 0x8056); // "No"
        } else if c == b'B' {
            text::print_string(gm, 0x8046); // "Buy\n\n\""
            let r = util::rand_range(gm, 3, 0);
            let s = gm.g16((r as u16).wrapping_mul(2).wrapping_add(0x3d4a)); // "Very good!\n" ...
            text::print_string(gm, s);
            let r = util::rand_range(gm, 3, 0);
            let s = gm.g16((r as u16).wrapping_mul(2).wrapping_add(0x3d52)); // "We have:" ...
            text::print_string(gm, s);
            result = arms_buy_menu(gm, member) as u16;
        } else if c == b'S' {
            text::print_string(gm, 0x804e); // "Sell\n\n\""
            arms_sell_browser(gm, member);
            result = 0xffff;
        }
        if !(c != b'B' && c != b'S' && c != b' ') { break; }
    }
    print_farewell(gm, result as i16);
}

/// orig 1000:b60c (52 bytes). Healer patient selection: with a single party member returns
/// 0 without asking; otherwise "\n\n\"Who needs my aid?\" " and select_party_member_no_zero
/// (-1 = cancelled, printing "No one"). Returns the member index.
pub fn select_patient(gm: &mut Game) -> i16 {
    if gm.gi8(g::PARTY_COUNT) == 1 {
        0
    } else {
        text::print_string(gm, 0x805a); // "\n\n\"Who needs my aid?\" "
        let m = rest::select_party_member_no_zero(gm);
        if m == -1 {
            text::print_string(gm, 0x8072); // "No one"
        }
        m
    }
}

/// orig 1000:b640 (186 bytes). The healer's "light" effect: XOR-fills the 8x8 cell at
/// (0xb7,0xb7) in g_ui_color0 with a rising/falling sweep noise, again in g_ui_color1 with a
/// second sweep, and a final XOR fill with a third sweep (play_sweep_noise argument sets
/// (1,5000,0x57e4,1,0x100e)/(-1,27500,..), (1,1,40000,1,0x11b2)/(-1,40000,..),
/// (2,1,18000,1,0x8fc)/(-2,36000,..)).
pub fn healing_light_effect(gm: &mut Game) {
    let c0 = gm.gi16(g::UI_COLOR0);
    gfx::set_color(gm, c0);
    gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
    util::play_sweep_noise(gm, 1, 0x1388, 0x57e4, 1, 0x100e);
    util::play_sweep_noise(gm, 0xffff, 0x6b6c, 0x57e4, 1, 0x100e);
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
    util::play_sweep_noise(gm, 1, 1, 40000, 1, 0x11b2);
    util::play_sweep_noise(gm, 0xffff, 40000, 40000, 1, 0x11b2);
    gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
    util::play_sweep_noise(gm, 2, 1, 0x4650, 1, 0x8fc);
    util::play_sweep_noise(gm, 0xfffe, 36000, 0x4650, 1, 0x8fc);
}

/// orig 1000:b6fa (140 bytes). "for % gold. Wilt thou pay?" with the price at 0xb118; reads
/// 'Y'/'N' (echoed). Returns true when the service is NOT performed: 'N', or 'Y' without
/// enough gold unless the price is <= 100 and the town is location 7 (Skara Brae), where the
/// healer treats the poor for free ("I am truly sorry, but I cannot accept charity cases."
/// SHOPPE.DAT 0x23ab is printed otherwise). When paying is possible the gold is deducted
/// (plus the shadowlord cheat).
pub fn ask_healer_payment(gm: &mut Game) -> bool {
    print_shop_text(gm, 0x807a); // "for % gold.\n\nWilt thou\npay?\" "
    loop {
        let k = input::read_command_key(gm);
        if k == 0x59 {
            text::print_string(gm, 0x8098); // "Yes"
        } else if k == 0x4e {
            text::print_string(gm, 0x809c); // "No"
        }
        if k == 0x59 || k == 0x4e {
            let mut declined = k == 0x4e;
            let price = gm.g16(SHOP_PRICE);
            if !declined && (gm.g16(g::GOLD) < price) && (100 < price || gm.gi8(g::LOCATION) != 7) {
                print_shoppe_dat(gm, 0x23ab); // "I am truly sorry, but I cannot accept charity cases." says $.
                declined = true;
            }
            if !declined && price <= gm.g16(g::GOLD) {
                let gold = gm.g16(g::GOLD).wrapping_sub(price);
                gm.set_g16(g::GOLD, gold);
                shadowlord_cheat_gold(gm);
            }
            return declined;
        }
    }
}

/// orig 1000:b788 (556 bytes). Healer (shop type 6) entry from TALK; takes the member argument
/// (RET 2) but does not use it. Greeting, then 'N' leaves, 'Y' prints "We have powers to
/// Cure, Heal, or Resurrect." says $. "What is the nature of thy need?" and reads C/H/R
/// (Enter/space -> "Nothing", ends). Each service picks a patient (select_patient; -1 skips):
/// Cure needs status 'P' (else "Thou hast no need of this art!"), in location 5 (Minoc) it is
/// free ("Receive now the Light!"), elsewhere "I can cure thy poisoned body " + payment of
/// 0x3d8e[slot]; then the light effect and status 'G'. Heal is refused for the dead or at full
/// HP; free in Minoc, else "I can heal thee " + 0x3d86[slot]; light and HP = max HP.
/// Resurrect needs status 'D', costs 0x3d96[slot] (u16) and on payment runs the light,
/// CAST2 in_mani_corp_resurrect(0xff, member), HP = max HP and redraws the status window.
/// After each service (unless ending) the status window is redrawn and "Is there any other
/// way in which I may aid thee?" asked. Ends with print_farewell(1).
pub fn healer_shop(gm: &mut Game, param_1: u16) {
    let _member = param_1;
    print_greeting(gm);
    let mut done = false;
    while !done {
        let mut c;
        loop {
            c = input::read_command_key(gm) as u8;
            if c == b'N' {
                text::print_string(gm, 0x80a0); // "No"
            } else if c == b'Y' {
                text::print_string(gm, 0x80a4); // "Yes\n\n"
            }
            if !(c != b'N' && c != b'Y') { break; }
        }
        if c == b'N' { break; }
        text::print_string(gm, 0x80aa); // "\"We have powers to Cure, Heal, or Resurrect.\"\n"
        print_shop_text(gm, 0x80da); // "says $.\n\n\"What is the nature of thy need?\" "
        loop {
            c = input::read_command_key(gm) as u8;
            if c == b'C' || c == b'H' || c == b'R' { break; }
            if c == b' ' || c == b'\r' { break; }
        }
        if c == b'\r' || c == b' ' {
            text::print_string(gm, 0x81c0); // "Nothing"
            done = true;
        } else if c == b'C' {
            text::print_string(gm, 0x8106); // "Curing"
            let m = select_patient(gm);
            if m != -1 {
                let rec = (m as u16).wrapping_mul(0x20);
                if gm.g8(g::PARTY_STATUS.wrapping_add(rec)) == b'P' {
                    text::print_string(gm, 0x810e); // "\n\n\""
                    let mut skip = false;
                    if gm.gi8(g::LOCATION) == 5 {
                        text::print_string(gm, 0x8112); // "Receive now the Light!\""
                    } else {
                        text::print_string(gm, 0x812a); // "I can cure thy poisoned body "
                        let price = gm.g8((gm.gi16(SHOP_SLOT) as u16).wrapping_add(0x3d8e)) as u16;
                        gm.set_g16(SHOP_PRICE, price);
                        if ask_healer_payment(gm) {
                            skip = true;
                        }
                    }
                    if !skip {
                        healing_light_effect(gm);
                        gm.set_g8(g::PARTY_STATUS.wrapping_add(rec), 0x47); // 'G'
                    }
                } else {
                    print_shop_text(gm, 0x3d5a); // "\n\n\"Thou hast no need of this art!\"\nsays $."
                }
            }
        } else if c == b'H' {
            text::print_string(gm, 0x8148); // "Healing"
            let m = select_patient(gm);
            if m != -1 {
                let rec = (m as u16).wrapping_mul(0x20);
                if gm.g8(g::PARTY_STATUS.wrapping_add(rec)) == b'D'
                    || gm.gi16(rec.wrapping_add(0x55b8)) == gm.gi16(g::PARTY_MAX_HP.wrapping_add(rec))
                {
                    print_shop_text(gm, 0x3d5a); // "Thou hast no need of this art!"
                } else {
                    text::print_string(gm, 0x8150); // "\n\n\""
                    let mut skip = false;
                    if gm.gi8(g::LOCATION) == 5 {
                        text::print_string(gm, 0x8154); // "Receive now the Light!\""
                    } else {
                        text::print_string(gm, 0x816c); // "I can heal thee "
                        let price = gm.g8((gm.gi16(SHOP_SLOT) as u16).wrapping_add(0x3d86)) as u16;
                        gm.set_g16(SHOP_PRICE, price);
                        if ask_healer_payment(gm) {
                            skip = true;
                        }
                    }
                    if !skip {
                        healing_light_effect(gm);
                        let mx = gm.gi16(g::PARTY_MAX_HP.wrapping_add(rec));
                        gm.set_gi16(rec.wrapping_add(0x55b8), mx);
                    }
                }
            }
        } else if c == b'R' {
            text::print_string(gm, 0x817e); // "Resurrect"
            let m = select_patient(gm);
            if m != -1 {
                let rec = (m as u16).wrapping_mul(0x20);
                if gm.g8(g::PARTY_STATUS.wrapping_add(rec)) != b'D' {
                    print_shop_text(gm, 0x3d5a); // "Thou hast no need of this art!"
                } else {
                    text::print_string(gm, 0x8188); // "\n\n\""
                    text::print_string(gm, 0x818c); // "I can raise this unfortunate person from "
                    text::print_string(gm, 0x81b6); // "the dead "
                    let price = gm.g16((gm.gi16(SHOP_SLOT) as u16).wrapping_mul(2).wrapping_add(0x3d96));
                    gm.set_g16(SHOP_PRICE, price);
                    if !ask_healer_payment(gm) {
                        healing_light_effect(gm);
                        ovl::cast2::in_mani_corp_resurrect(gm, 0xff, m);
                        let mx = gm.gi16(g::PARTY_MAX_HP.wrapping_add(rec));
                        gm.set_gi16(rec.wrapping_add(0x55b8), mx);
                        rest::draw_status_window(gm);
                    }
                }
            }
        }
        if !done {
            rest::draw_status_window(gm);
            text::print_string(gm, 0x81c8); // "\n\n\"Is there any other way in which I may\n"
            text::print_string(gm, 0x81f2); // "aid thee?\" "
        }
    }
    print_farewell(gm, 1);
}
