//! SHOPPES2.OVL (translated)
//!
//! Shop conversations, part 2: the TAVERN (shop type 1, vector 1778:09fe -> e84c) and the
//! SHIPWRIGHT (shop type 3, vector 1778:0a0a -> ec9c), both dispatched from TALK's
//! `talk_to_shopkeeper`.  The text machinery (print_shop_text / print_shoppe_dat, greeting and
//! farewell, the Shadowlord surcharge) lives in SHOPPES and is called through vectors.
//!
//! TALK-provided context (DGROUP): 0xb114 shop slot (index of the town in the shop type's
//! town list), 0xaafe shopkeeper name pointer ('$'), 0xb118 price ('%'), 0xab00 item/person
//! name ('&'), 0xac62 location name ('*'), 0xbcf8 the line-input buffer.
//! Tavern state (DGROUP 0xbd16..0xbd20): style byte, "bought something" flag, number of living
//! members for the last round/meal, the count printed as a word, the meal tile last placed and
//! the running count of drinks served (never reset by this overlay).
//! Shipwright state: 0xbd22 "kicked out" flag, 0xbd24 "bought" flag.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;
use crate::ovl::shoppes::{print_shop_text, print_shoppe_dat, shadowlord_cheat_gold, print_greeting, print_farewell};

const SHOP_SLOT: u16 = 0xb114; // i16: town slot within the shop type's list (TALK)
const SHOP_PRICE: u16 = 0xb118; // i16: current price, expanded by '%'
const SHOPKEEPER_NAME_PTR: u16 = 0xaafe; // '$'
const SHOP_ITEM_NAME_PTR: u16 = 0xab00; // '&'
const SHOP_LOCATION_NAME_PTR: u16 = 0xac62; // '*'
const INPUT: u16 = 0xbcf8; // TALK's line-input buffer
const TAVERN_STYLE: u16 = 0xbd16; // u8: menu style 0..3 of this tavern (g_tavern_style_table[slot])
const TAVERN_BOUGHT: u16 = 0xbd18; // i16: 1 once something was paid for (enables rumours, farewell)
const TAVERN_LIVE_COUNT: u16 = 0xbd1a; // i16: living members fed by the last meal (0 for a round of drinks)
const TAVERN_COUNT_WORD: u16 = 0xbd1c; // i16: living members, printed as "two".."six"
const TAVERN_MEAL_TILE: u16 = 0xbd1e; // i16: tile placed on the table by the last meal (0x9a/0x9b)
const TAVERN_ROUNDS: u16 = 0xbd20; // i16: rounds of drink served so far
const SHIP_KICKED_OUT: u16 = 0xbd22; // i16: 1 after "What? Cheat me, would ye? OUT!"
const SHIP_BOUGHT: u16 = 0xbd24; // i16: 1 after a vessel was paid for

/// Shared intelligence price adjustment (same arithmetic as SHOPPES): 0xb118 += price *
/// (100 - 3 * intelligence) / 100 with a signed 32-bit multiply and an UNSIGNED 32-bit divide
/// (an intelligence above 33 yields the original's garbage low word - kept bit-exact).
fn adjust_price_for_intelligence(gm: &mut Game, intelligence: u8) {
    let price = gm.gi16(SHOP_PRICE);
    let int3 = (intelligence as u16).wrapping_mul(3);
    let d = int3.wrapping_sub(100);
    // 32-bit negation of the sign-extended (int*3 - 100)
    let hi = ((d != 0) as u16).wrapping_sub((int3 < 100) as u16).wrapping_neg();
    let prod = rest::__aFlmul(gm, d.wrapping_neg(), hi as i16, price as u16, price >> 15);
    let q = rest::__aFuldiv(gm, prod as u16, (prod >> 16) as u16, 100, 0);
    gm.set_gi16(SHOP_PRICE, price.wrapping_add(q as i16));
}

/// orig 1000:e1e0 (106 bytes). Counts the party members that are not dead ('D' status): the
/// count goes to 0xbd1a and 0xbd1c and the price 0xb118 becomes count * `per_head`. Then prints
/// two newlines. (The address is shared with FONT/CAST2/COMSUBS functions - vectors 1778:0566/
/// 0986/099e belong to those overlays, not to this one.)
pub fn tally_living_members(gm: &mut Game, per_head: i16) {
    gm.set_gi16(SHOP_PRICE, 0);
    gm.set_gi16(TAVERN_COUNT_WORD, 0);
    gm.set_gi16(TAVERN_LIVE_COUNT, 0);
    if gm.g8(g::PARTY_COUNT) != 0 {
        let mut p = g::PARTY_STATUS;
        gm.set_gi16(TAVERN_LIVE_COUNT, 0);
        gm.set_gi16(TAVERN_COUNT_WORD, 0);
        let mut i: u16 = 0;
        loop {
            if gm.g8(p) != 0x44 {
                gm.set_gi16(TAVERN_COUNT_WORD, gm.gi16(TAVERN_COUNT_WORD).wrapping_add(1));
                gm.set_gi16(TAVERN_LIVE_COUNT, gm.gi16(TAVERN_LIVE_COUNT).wrapping_add(1));
                gm.set_gi16(SHOP_PRICE, gm.gi16(SHOP_PRICE).wrapping_add(per_head));
            }
            p = p.wrapping_add(0x20);
            i = i.wrapping_add(1);
            if i == gm.g8(g::PARTY_COUNT) as u16 { break; }
        }
    }
    text::put_char(gm, 10);
    text::put_char(gm, 10);
}

/// orig 1000:e24a (54 bytes). Prints the living-member count 0xbd1c as a word: "two" (0x9aa8),
/// "three" (0x9aac), "four" (0x9ab2), "five" (0x9ab8), "six" (0x9abe); prints nothing for
/// any other count (a lone Avatar never reaches the "of ye" phrase with a number).
pub fn print_count_word(gm: &mut Game) {
    let s: u16 = match gm.gi16(TAVERN_COUNT_WORD) {
        2 => 0x9aa8, // "two"
        3 => 0x9aac, // "three"
        4 => 0x9ab2, // "four"
        5 => 0x9ab8, // "five"
        6 => 0x9abe, // "six"
        _ => return,
    };
    text::print_string(gm, s);
}

/// orig 1000:e27e (6 bytes). Prints "five" (0x9ab8). Not called from this overlay; the only
/// reference is vector 1778:09aa, which belongs to CAST2's code at the same address.
pub fn print_word_five(gm: &mut Game) {
    text::print_string(gm, 0x9ab8); // "five"
}

/// orig 1000:e28c (46 bytes). Clears the active member (0xff, so the status window shows the
/// whole party) and prints "sir" (0x9ac2) when the Avatar's gender byte (record 0, 0x55b1) is
/// 0x0b, else "milady" (0x9ac6).
pub fn print_sir_or_milady(gm: &mut Game) {
    gm.set_g8(g::ACTIVE_MEMBER, 0xff);
    let s: u16 = if gm.gi8(0x55b1) == 0x0b { 0x9ac2 } else { 0x9ac6 }; // "sir" / "milady"
    text::print_string(gm, s);
}

/// orig 1000:e2bc (244 bytes). Charges for a round or a meal: '"That will be % gold for the
/// <count> of ye, sir/milady.' Without the gold: '"CAN'T PAY? Beat it!" yells $.' and returns 1
/// (the customer is thrown out). Otherwise the gold is taken (Shadowlord surcharge); for a
/// round of drinks (0xbd1a == 0) the rounds counter 0xbd20 is incremented, for a meal the
/// food is raised by the number of living members (capped at 9999) and the table tile 0x95
/// directly above the party becomes 0x9b (or the one below becomes 0x9a) with the map view
/// refreshed and the tile remembered in 0xbd1e. Ends with "Enjoy!" and returns 0.
pub fn serve_round_or_meal(gm: &mut Game) -> u16 {
    text::put_char(gm, 0x22);
    text::print_string(gm, 0x9ace); // "That will be "
    let price = gm.gi16(SHOP_PRICE);
    text::print_number(gm, 0x20, 1, price);
    text::print_string(gm, 0x9adc); // " gold for the "
    print_count_word(gm);
    text::print_string(gm, 0x9aec); // " of ye,\n"
    print_sir_or_milady(gm);
    text::put_char(gm, 0x2e);
    if gm.gi16(g::GOLD) < gm.gi16(SHOP_PRICE) {
        text::print_string(gm, 0x9af6); // "\n\n\"CAN'T PAY?\nBeat it!\"\nyells "
        let s = gm.g16(SHOPKEEPER_NAME_PTR);
        text::print_string(gm, s);
        text::put_char(gm, 0x2e);
        text::put_char(gm, 10);
        return 1;
    }
    let v = gm.gi16(g::GOLD).wrapping_sub(gm.gi16(SHOP_PRICE));
    gm.set_gi16(g::GOLD, v);
    shadowlord_cheat_gold(gm);
    if gm.gi16(TAVERN_LIVE_COUNT) == 0 {
        gm.set_gi16(TAVERN_ROUNDS, gm.gi16(TAVERN_ROUNDS).wrapping_add(1));
    } else {
        let n = gm.gi16(TAVERN_LIVE_COUNT);
        rest::add_i16_capped(gm, 0x270f, n, g::FOOD);
        let px = gm.g8(g::PARTY_X) as u16;
        let py = gm.g8(g::PARTY_Y) as u16;
        let above = rest::get_map_tile(gm, py.wrapping_sub(1), px);
        if gm.g8(above) == 0x95 {
            let p = rest::get_map_tile(gm, py.wrapping_sub(1), px);
            gm.set_g8(p, 0x9b);
            gm.set_gi16(TAVERN_MEAL_TILE, 0x9b);
            view::refresh_map_view(gm);
        } else {
            let below = rest::get_map_tile(gm, py.wrapping_add(1), px);
            if gm.g8(below) == 0x95 {
                let p = rest::get_map_tile(gm, py.wrapping_add(1), px);
                gm.set_g8(p, 0x9a);
                gm.set_gi16(TAVERN_MEAL_TILE, 0x9a);
                view::refresh_map_view(gm);
            }
        }
    }
    // LAB_1000_e3a8
    text::print_string(gm, 0x9b16); // "\nEnjoy!\"\n\n"
    0
}

/// orig 1000:e3b2 (33 bytes). Meal option: echoes the tavern's meal key letter
/// (g_tavern_meal_keys[style], 0x4c1e), tallies the living members at the tavern's meal price
/// (g_tavern_meal_prices[slot], 0x4c36) and serves. Returns `serve_round_or_meal`'s result
/// (1 = thrown out), which the original leaves in AX.
pub fn order_meal(gm: &mut Game) -> u16 {
    let style = gm.g8(TAVERN_STYLE) as u16;
    let k = gm.g8(style.wrapping_add(0x4c1e));
    text::put_char(gm, k);
    let slot = gm.gi16(SHOP_SLOT) as u16;
    let price = gm.gi16(slot.wrapping_mul(2).wrapping_add(0x4c36));
    tally_living_members(gm, price);
    serve_round_or_meal(gm)
}

/// orig 1000:e3d4 (394 bytes). Drink option: echoes the drink key letter (0x4c24[style]).
/// After three rounds (0xbd20 == 3) the keeper asks '"But haven't ye had enough to drink?"':
/// 'Y' returns 0 without serving, 'N' makes the party drunk (g_drunk_moves = 25, karma - 1)
/// and serves anyway. Tallies living members at 1 gold each. Taverns whose drink key is 'W'
/// show the wine list (a-f, prices g_wine_prices 0x4c48: 18/192/79/30/275/98): a letter
/// prints '"Ah, a fine choice, sir."', charges (CAN'T PAY -> return 1), counts a round and
/// prints "Enjoy!" (return 0); space quits the list with SHOPPE.DAT 0x1413 ("Our meagre stock
/// is not good enough for thee!") and returns 2. Other taverns serve a plain round through
/// `serve_round_or_meal` with 0xbd1a cleared (price = living count * 1).
pub fn order_drink(gm: &mut Game) -> u16 {
    let style = gm.g8(TAVERN_STYLE) as u16;
    let k = gm.g8(style.wrapping_add(0x4c24));
    text::put_char(gm, k);
    if gm.gi16(TAVERN_ROUNDS) == 3 {
        text::print_string(gm, 0x9b22); // "\n\n\"I beg thy\npardon, "
        print_sir_or_milady(gm);
        text::print_string(gm, 0x9b38); // ",\"\nsays "
        let s = gm.g16(SHOPKEEPER_NAME_PTR);
        text::print_string(gm, s);
        text::print_string(gm, 0x9b42); // ".\n\"But haven't\nye had enough\nto drink?\" "
        loop {
            let c = input::read_command_key(gm) as u8;
            if c == b'Y' {
                text::print_string(gm, 0x9b6c); // "Yes\n\n"
                return 0; // LAB_1000_e421
            }
            if c == b'N' {
                text::print_string(gm, 0x9b72); // "No!"
                gm.set_g8(g::DRUNK_MOVES, 0x19);
                rest::sub_u8_floor(gm, 1, g::KARMA);
            }
            if c == b'Y' || c == b'N' { break; }
        }
    }
    tally_living_members(gm, 1);
    if gm.g8(style.wrapping_add(0x4c24)) == b'W' {
        text::print_string(gm, 0x9b76); // "\"Our wine list,\n"
        print_sir_or_milady(gm);
        text::print_string(gm, 0x9b88); // ".\n\n"
        text::print_string(gm, 0x9b8c); // "a) Rose.......18\n"
        text::print_string(gm, 0x9b9e); // "b) Claret....192\n"
        text::print_string(gm, 0x9bb0); // "c) Sauterne...79\n"
        text::print_string(gm, 0x9bc2); // "d) Muscatel...30\n"
        text::print_string(gm, 0x9bd4); // "e) Moselle...275\n"
        text::print_string(gm, 0x9be6); // "f) Chablis....98\n\n"
        text::print_string(gm, 0x9bfa); // "Thy choice?\" "
        let mut key: u8;
        loop {
            key = input::read_command_key(gm) as u8;
            if key == 0x20 { break; }
            if !(key < 0x41 || 0x46 < key) { break; }
        }
        if key != 0x20 {
            text::put_char(gm, key);
            text::print_string(gm, 0x9c08); // "\n\n\"Ah, a fine\nchoice, "
            print_sir_or_milady(gm);
            text::put_char(gm, 0x2e);
            key = key.wrapping_add(0xbf); // letter - 'A'
            let price = gm.gi16((key as u16).wrapping_mul(2).wrapping_add(0x4c48));
            if gm.gi16(g::GOLD) < price {
                text::print_string(gm, 0x9c20); // "\"\n\n\"CAN'T PAY?\nBeat it!\"\nyells "
                let s = gm.g16(SHOPKEEPER_NAME_PTR);
                text::print_string(gm, s);
                text::put_char(gm, 0x2e);
                text::put_char(gm, 10);
                return 1;
            }
            let v = gm.gi16(g::GOLD).wrapping_sub(price);
            gm.set_gi16(g::GOLD, v);
            shadowlord_cheat_gold(gm);
            gm.set_gi16(TAVERN_ROUNDS, gm.gi16(TAVERN_ROUNDS).wrapping_add(1));
            text::print_string(gm, 0x9c40); // "\nEnjoy!\""
        }
        // LAB_1000_e4b2
        text::put_char(gm, 10);
        text::put_char(gm, 10);
        if key == 0x20 {
            print_shoppe_dat(gm, 0x1413); // "Well! Our meagre stock is not good enough for thee!"
            2
        } else {
            0
        }
    } else {
        gm.set_gi16(TAVERN_LIVE_COUNT, 0);
        serve_round_or_meal(gm)
    }
}

/// orig 1000:e560 (389 bytes). Rations option for `member`: echoes the rations key
/// (0x4c2a[style]: 'R' Rations / 'P' Provisions; styles 1 and 2 sell none), prices a 25-serving
/// pack at g_rations_prices[slot] (0x4c54) with the intelligence adjustment, prints one of
/// seven random pitches (SHOPPE.DAT offsets at 0x4c66) and '"How many wouldst thou like?"'.
/// 0 -> '"Hrumph."' and return 2. Each pack: without the gold prints two newlines and, if it is
/// the first pack, either hands out "table scraps" (food < 3: food += 1..2, SHOPPE.DAT 0x143f)
/// or '"Thou hast neither gold nor need! Out!" yells $.' - both return 1 (thrown out); for a
/// later pack '"Thou canst afford only N!"' and return 0. Otherwise gold -= price (floored at 0),
/// food += 25 (capped 9999, the loop stops when food reaches 9999). Afterwards two newlines,
/// the Shadowlord surcharge, return 0.
pub fn buy_rations(gm: &mut Game, member: i16) -> u16 {
    let style = gm.g8(TAVERN_STYLE) as u16;
    let k = gm.g8(style.wrapping_add(0x4c2a));
    text::put_char(gm, k);
    text::put_char(gm, 10);
    text::put_char(gm, 10);
    let slot = gm.gi16(SHOP_SLOT) as u16;
    let price = gm.gi16(slot.wrapping_mul(2).wrapping_add(0x4c54));
    gm.set_gi16(SHOP_PRICE, price);
    let intel = gm.g8((member as u16).wrapping_mul(0x20).wrapping_add(0x55b6));
    adjust_price_for_intelligence(gm, intel);
    text::put_char(gm, 0x22);
    let r = util::rand_range(gm, 6, 0);
    let off = gm.g16((r as u16).wrapping_mul(2).wrapping_add(0x4c66));
    print_shoppe_dat(gm, off);
    text::print_string(gm, 0x9c4a); // "\n\nHow many wouldst\nthou like?\" "
    let n = rest::read_number(gm, 2);
    if n == 0 {
        text::print_string(gm, 0x9c6a); // "\n\n\"Hrumph.\""
        return 2;
    }
    let mut left = n;
    while left != 0 {
        if gm.gi16(g::GOLD) < gm.gi16(SHOP_PRICE) {
            text::print_string(gm, 0x9c76); // "\n\n"
            if n.wrapping_sub(left) == 0 {
                if gm.gi16(g::FOOD) < 3 {
                    let r = util::rand_range(gm, 1, 0);
                    rest::add_i16_capped(gm, 0x270f, r.wrapping_add(1), g::FOOD);
                    print_shoppe_dat(gm, 0x143f); // "I'll give thee some table scraps! Now go!" orders $.
                } else {
                    text::print_string(gm, 0x9c7a); // "\"Thou hast\nneither gold nor\nneed! Out!\"\n"
                    text::print_string(gm, 0x9ca4); // "yells "
                    let s = gm.g16(SHOPKEEPER_NAME_PTR);
                    text::print_string(gm, s);
                    text::print_string(gm, 0x9cac); // ".\n"
                }
                return 1;
            }
            text::print_string(gm, 0x9cb0); // "\"Thou canst\nafford only "
            text::print_number(gm, 0x20, 1, n.wrapping_sub(left));
            text::print_string(gm, 0x9cca); // "!\"\n\n"
            return 0;
        }
        let p = gm.gi16(SHOP_PRICE);
        rest::sub_i16_floor(gm, p, g::GOLD);
        rest::add_i16_capped(gm, 0x270f, 0x19, g::FOOD);
        if gm.gi16(g::FOOD) == 0x270f { break; }
        left = left.wrapping_sub(1);
    }
    text::print_string(gm, 0x9cd0); // "\n\n"
    shadowlord_cheat_gold(gm);
    0
}

/// orig 1000:e6e8 (347 bytes). Rumour option: echoes the rumour key (0x4c30[style]: 'CTHA' =
/// Chat, Talk, Help, Advice) and asks '"Of what wouldst thou hear my lore, sir?" You respond:'.
/// The answer (up to 15 chars into 0xbcf8; empty -> return 0) is searched case-insensitively
/// for each of the 26 keywords g_rumour_keywords (0x4c74); a hit counts only at the start of
/// the input or right after a space. No hit: '"That, I cannot help thee with.' and ask again.
/// A hit sets the price from g_rumour_prices (0x4d10) and prints SHOPPE.DAT 0x134e ('Well
/// now, my memory on that subject is a bit hazy. Mayhap % gold crowns ...') + '"Fair 'nuff?"'.
/// 'N' -> "No" and return 0. 'Y' -> "Yes"; without the gold '"Sorry, sir,' + SHOPPE.DAT 0x146a
/// ('I only attend to PAYING customers!' says $) and return 1 (thrown out). Else pays (no
/// surcharge), sets the '&' name from g_rumour_person_names (0x4ca8) and the '*' location
/// from g_rumour_location_names (0x4cf6) via g_rumour_location_index (0x4cdc), prints one of
/// four random replies (0x4d44: '"Seek out & in *!"' etc.), 'says $.' and returns 0.
pub fn buy_rumour(gm: &mut Game) -> u16 {
    let style = gm.g8(TAVERN_STYLE) as u16;
    let k = gm.g8(style.wrapping_add(0x4c30));
    text::put_char(gm, k);
    text::print_string(gm, 0x9efc); // "\n\n\""
    let mut found: i16;
    loop {
        text::print_string(gm, 0x9f00); // "Of what wouldst\nthou hear my\nlore, "
        print_sir_or_milady(gm);
        text::print_string(gm, 0x9f24); // "?\"\n\nYou respond:\n"
        rest::read_text_line(gm, 0xf, INPUT);
        text::print_string(gm, 0x9f36); // "\n\n"
        if gm.g8(INPUT) == 0 {
            return 0;
        }
        let mut i: i16 = 0;
        found = -1;
        let mut kw: u16 = 0x4c74; // g_rumour_keywords
        loop {
            let needle = gm.g16(kw);
            let pos = rest::str_find_ci(gm, INPUT as i16, needle) as i16;
            // match at the start of the input, or preceded by a space (input[pos-1])
            if pos == 0 || (0 < pos && gm.gi8((pos as u16).wrapping_add(0xbcf7)) == 0x20) {
                found = i;
                break;
            }
            kw = kw.wrapping_add(2);
            i = i.wrapping_add(1);
            if !(i < 0x1a) { break; }
        }
        if found != -1 {
            let p = gm.gi16((found as u16).wrapping_mul(2).wrapping_add(0x4d10));
            gm.set_gi16(SHOP_PRICE, p);
            print_shoppe_dat(gm, 0x134e); // "Well now, my memory on that subject is a bit hazy. Mayhap % gold crowns ..."
            text::print_string(gm, 0x9f5c); // "\n\nFair 'nuff?\" "
            break;
        }
        text::print_string(gm, 0x9f3a); // "\"That, I cannot help thee with.\n\n"
    }
    let mut c: i16;
    loop {
        c = input::read_command_key(gm);
        if c == 0x59 { break; }
        if c == 0x4e { break; }
    }
    if c == 0x4e {
        text::print_string(gm, 0x9f6c); // "No\n\n"
        return 0;
    }
    text::print_string(gm, 0x9f72); // "Yes\n\n"
    if gm.gi16(g::GOLD) < gm.gi16(SHOP_PRICE) {
        text::print_string(gm, 0x9f78); // "\"Sorry, "
        print_sir_or_milady(gm);
        print_shoppe_dat(gm, 0x146a); // ",\nI only attend to PAYING customers!"\nsays $.
        return 1;
    }
    let v = gm.gi16(g::GOLD).wrapping_sub(gm.gi16(SHOP_PRICE));
    gm.set_gi16(g::GOLD, v);
    rest::draw_status_window(gm);
    let name = gm.g16((found as u16).wrapping_mul(2).wrapping_add(0x4ca8));
    gm.set_g16(SHOP_ITEM_NAME_PTR, name);
    let li = gm.g8((found as u16).wrapping_add(0x4cdc)) as u16;
    let loc = gm.g16(li.wrapping_mul(2).wrapping_add(0x4cf6));
    gm.set_g16(SHOP_LOCATION_NAME_PTR, loc);
    let r = util::rand_range(gm, 3, 0);
    let off = gm.g16((r as u16).wrapping_mul(2).wrapping_add(0x4d44));
    print_shoppe_dat(gm, off);
    text::print_string(gm, 0x9f82); // "\nsays "
    let s = gm.g16(SHOPKEEPER_NAME_PTR);
    text::print_string(gm, s);
    text::print_string(gm, 0x9f8a); // ".\n\n"
    0
}

/// orig 1000:e844 (8 bytes). Tavern farewell: SHOPPES `print_farewell` with the "bought
/// something" flag 0xbd18 (0 = "nothing bought" line, 1 = "thanks" line).
pub fn tavern_farewell(gm: &mut Game) {
    let b = gm.gi16(TAVERN_BOUGHT);
    print_farewell(gm, b);
}

/// orig 1000:e84c (371 bytes). TAVERN shop entry (vector 1778:09fe, TALK shop type 1) for
/// `member` (the talker, whose intelligence prices rations). Looks up the tavern's menu style
/// (g_tavern_style_table 0x4d4c[slot]), clears the bought flag and meal tile, prints the
/// greeting and waits for Y/N ('N'/space -> "No" + farewell). 'Y' prints the style's menu text
/// (SHOPPE.DAT 0x4d56[style], e.g. "What'll it be... a leg of our tender roast Mutton, a
/// tankard of Ale, or Rations for thy travels?"), '" ' and a newline when the cursor is at
/// column >= 15. Key loop: space/Esc/Enter -> farewell and return; the style's meal key ->
/// `order_meal`; drink key -> `order_drink`; rations key -> `buy_rations(member)`; rumour key
/// (only after something was bought, else silently ignored) -> `buy_rumour`. A result of 1
/// (thrown out) returns at once without farewell; other keys are ignored. After a served
/// option the bought flag is set unless the result was 2 (wine list abandoned), the status
/// window is redrawn and '"Anything else for thee?"' is asked: 'N' -> "No" + farewell, 'Y' ->
/// the style's follow-up text (0x4d5e[style]) and back to the key loop.
pub fn tavern_shop(gm: &mut Game, member: u16) {
    let slot = gm.gi16(SHOP_SLOT) as u16;
    let st = gm.g8(slot.wrapping_add(0x4d4c));
    gm.set_g8(TAVERN_STYLE, st);
    gm.set_gi16(TAVERN_BOUGHT, 0);
    gm.set_gi16(TAVERN_MEAL_TILE, 0);
    print_greeting(gm);
    loop {
        let c = input::read_command_key(gm) as u8;
        if c == b'N' || c == b' ' {
            text::print_string(gm, 0x9f8e); // "No"
            tavern_farewell(gm);
            return;
        }
        if c == b'Y' { break; }
    }
    text::print_string(gm, 0x9f92); // "Yes\n\n\""
    let style = gm.g8(TAVERN_STYLE) as u16;
    let off = gm.g16(style.wrapping_mul(2).wrapping_add(0x4d56));
    print_shoppe_dat(gm, off);
    text::put_char(gm, 0x22);
    text::put_char(gm, 0x20);
    if !(text::get_cursor_col(gm) < 0xf) {
        text::put_char(gm, 10);
    }
    loop {
        // LAB_1000_e8bd
        let c = input::read_command_key(gm) as u8;
        let mut result: u16 = 0;
        if c == b' ' || c == 0x1b || c == 0x0d {
            tavern_farewell(gm);
            return;
        }
        let style = gm.g8(TAVERN_STYLE) as u16;
        if gm.g8(style.wrapping_add(0x4c1e)) == c {
            if order_meal(gm) != 0 {
                return;
            }
        }
        if gm.g8(style.wrapping_add(0x4c24)) == c {
            result = order_drink(gm);
            if result == 1 {
                return;
            }
        }
        if gm.g8(style.wrapping_add(0x4c2a)) == c {
            result = buy_rations(gm, member as i16);
            if result == 1 {
                return;
            }
        }
        if gm.g8(style.wrapping_add(0x4c30)) == c {
            if gm.gi16(TAVERN_BOUGHT) == 0 {
                continue;
            }
            if buy_rumour(gm) != 0 {
                return;
            }
        }
        if !(gm.g8(style.wrapping_add(0x4c1e)) == c
            || gm.g8(style.wrapping_add(0x4c24)) == c
            || gm.g8(style.wrapping_add(0x4c2a)) == c
            || gm.g8(style.wrapping_add(0x4c30)) == c)
        {
            continue;
        }
        if result != 2 {
            gm.set_gi16(TAVERN_BOUGHT, 1);
        }
        rest::draw_status_window(gm);
        text::print_string(gm, 0x9f9a); // "\"Anything else\nfor thee?\" "
        loop {
            let c = input::read_command_key(gm) as u8;
            if c == b'N' {
                text::print_string(gm, 0x9fb6); // "No"
                tavern_farewell(gm);
                return;
            }
            if c == b'Y' { break; }
        }
        text::print_string(gm, 0x9fba); // "Yes\n\n\""
        let off = gm.g16(style.wrapping_mul(2).wrapping_add(0x4d5e));
        print_shoppe_dat(gm, off);
        text::put_char(gm, 0x22);
        text::put_char(gm, 0x20); // LAB_1000_e8b9
    }
}

/// orig 1000:e9c2 (42 bytes). Shipwright payment check: returns true when `price` <= gold.
/// Otherwise prints SHOPPE.DAT 0x198e ('"What? Cheat me, would ye? OUT!"') and 'yells $.',
/// sets the kicked-out flag 0xbd22 (suppresses the farewell) and returns false.
pub fn shipwright_check_gold(gm: &mut Game, price: i16) -> bool {
    let ok = price <= gm.gi16(g::GOLD);
    if !ok {
        print_shoppe_dat(gm, 0x198e); // "What? Cheat me, would ye? OUT!"
        print_shop_text(gm, 0x9fc2); // "yells $.\n"
        gm.set_gi16(SHIP_KICKED_OUT, 1);
    }
    ok
}

/// orig 1000:e9ee (153 bytes). Ghidra order (member, pay, announce). Completes a vessel sale:
/// `announce` prints SHOPPE.DAT 0x19ab ('"Sold! Thou may take delivery at the docks outside
/// the city!') and records the dock cell g_dock_x/y[slot] (0x4d76/0x4d7a) in g_bought_ship_x/y;
/// `pay` subtracts the price, sets the bought flag 0xbd24 and applies the surcharge. Then the
/// status window, '"Will there be anything else, ' + "milady" when the member's gender byte
/// equals 'F' (0x46 - never true for the 0x0b/0x0c encoding, so always "sir") + '?" ' and a
/// Y/N: 'Y' prints "Yes" and returns false (keep shopping), 'N' prints "No" and returns true.
pub fn shipwright_deliver(gm: &mut Game, member: i16, pay: i16, announce: i16) -> bool {
    if announce != 0 {
        print_shoppe_dat(gm, 0x19ab); // "Sold! Thou may take delivery at the docks outside the city!"
        let slot = gm.gi16(SHOP_SLOT) as u16;
        let x = gm.g8(slot.wrapping_add(g::DOCK_X));
        gm.set_g8(g::BOUGHT_SHIP_X, x);
        let y = gm.g8(slot.wrapping_add(g::DOCK_Y));
        gm.set_g8(g::BOUGHT_SHIP_Y, y);
    }
    if pay != 0 {
        let v = gm.gi16(g::GOLD).wrapping_sub(gm.gi16(SHOP_PRICE));
        gm.set_gi16(g::GOLD, v);
        gm.set_gi16(SHIP_BOUGHT, 1);
        shadowlord_cheat_gold(gm);
    }
    rest::draw_status_window(gm);
    print_shoppe_dat(gm, 0x19da); // "\n\nWill there be anything else, "
    // TODO(verify): original compares the gender byte (0x0b/0x0c) with 'F' - always "sir".
    let s: u16 = if gm.gi8((member as u16).wrapping_mul(0x20).wrapping_add(0x55b1)) == 0x46 { 0x9fcc } else { 0x9fd4 }; // "milady" / "sir"
    text::print_string(gm, s);
    text::print_string(gm, 0x9fd8); // "?\" "
    let mut c: u8;
    loop {
        c = input::read_command_key(gm) as u8;
        if c == b'Y' { break; }
        if c == b'N' { break; }
    }
    if c != b'Y' {
        text::print_string(gm, 0x9fe0); // "No"
    } else {
        text::print_string(gm, 0x9fdc); // "Yes"
    }
    c != b'Y'
}

/// orig 1000:ea88 (525 bytes). Shipwright menu for `member`. Loops: SHOPPE.DAT 0x18eb ('"We
/// sell ocean-going Frigates and small, light Skiffs. Which would ye like to see?"') and a
/// key. Esc/space leave. 'F': if no vessel is owned yet (g_bought_vessel & 0xc0 == 0) the
/// frigate price g_frigate_prices[slot] (0x4d66: 600/753/650/700) is intelligence-adjusted,
/// the pitch 0x183e and 'Wilt thou take it?' printed; 'N' -> "No", return; 'Y' -> "Yes",
/// gold check (fail -> return), g_bought_vessel = 0x82 (frigate, 0 skiffs... see notes), then
/// `shipwright_deliver(member, pay, announce)`. With a vessel already owned: 0x19f2 ('dock
/// space is limited ... delivery price would be 87,000 gold. Acceptable?'); 'Y' -> "Yes" and a
/// gold check against 10000 (always fails: thrown out), 'N' -> "No" and the menu is re-entered
/// recursively; either way return afterwards. 'S': skiff price g_skiff_prices[slot] (0x4d6e:
/// 200/175/125/100) adjusted, pitch 0x188c, 'Wilt thou take it?'; 'N' -> "No", return; 'Y' ->
/// "Yes", gold check; no vessel -> g_bought_vessel = 0x40 and deliver(pay, announce); only a
/// skiff owned -> 0x195f ('More than one skiff ... only if ye had bought a ship!') and
/// deliver(no pay, no announce); frigate owned -> 0x193b ('It will be placed onboard thy
/// frigate!'), g_bought_vessel += 1 and deliver(pay, no announce). The menu repeats while
/// deliver answers "anything else" with 'Y'.
pub fn shipwright_menu(gm: &mut Game, member: i16) {
    let intel_p = (member as u16).wrapping_mul(0x20).wrapping_add(0x55b6);
    loop {
        print_shoppe_dat(gm, 0x18eb); // "We sell ocean-going Frigates and small, light Skiffs. Which would ye like to see?"
        let k = input::read_command_key(gm);
        let mut done: i16 = 0;
        if k == 0x1b || k == 0x20 {
            done = 1;
        } else if k == 0x46 {
            text::print_string(gm, 0x9fe4); // "F\n\n"
            done = 1;
            if gm.g8(g::BOUGHT_VESSEL) & 0xc0 == 0 {
                let slot = gm.gi16(SHOP_SLOT) as u16;
                let p = gm.gi16(slot.wrapping_mul(2).wrapping_add(0x4d66));
                gm.set_gi16(SHOP_PRICE, p);
                let intel = gm.g8(intel_p);
                adjust_price_for_intelligence(gm, intel);
                print_shoppe_dat(gm, 0x183e); // "These stout-hearted vessels can travel the roughest of seas! For % gold, thou may master the winds!"
                print_shoppe_dat(gm, 0x1a50); // "\n\nWilt thou\ntake it?\" "
                let mut c: u8;
                loop {
                    c = input::read_command_key(gm) as u8;
                    if c == b'Y' { break; }
                    if c == b'N' { break; }
                }
                if c != b'Y' {
                    text::print_string(gm, 0x9ff6); // "No"  (LAB_1000_ebad)
                    return;
                }
                text::print_string(gm, 0x9ff0); // "Yes\n\n"
                let p = gm.gi16(SHOP_PRICE);
                if !shipwright_check_gold(gm, p) {
                    return;
                }
                gm.set_g8(g::BOUGHT_VESSEL, 0x82);
                // LAB_1000_ec62
                done = shipwright_deliver(gm, member, 1, 1) as i16;
            } else {
                print_shoppe_dat(gm, 0x19f2); // "Unfortunately, our dock space is limited right now. The delivery price would be 87,000 gold. Acceptable?"
                let mut c: u8;
                loop {
                    c = input::read_command_key(gm) as u8;
                    if c == b'Y' { break; }
                    if c == b'N' { break; }
                }
                if c == b'Y' {
                    text::print_string(gm, 0x9fe8); // "Yes"
                    shipwright_check_gold(gm, 0x2710); // 10000: never affordable, result ignored
                } else {
                    text::print_string(gm, 0x9fec); // "No"
                    shipwright_menu(gm, member);
                }
            }
        } else if k == 0x53 {
            text::print_string(gm, 0x9ffa); // "S\n\n"
            let slot = gm.gi16(SHOP_SLOT) as u16;
            let p = gm.gi16(slot.wrapping_mul(2).wrapping_add(0x4d6e));
            gm.set_gi16(SHOP_PRICE, p);
            let intel = gm.g8(intel_p);
            adjust_price_for_intelligence(gm, intel);
            print_shoppe_dat(gm, 0x188c); // "Skiffs allow thee to leave thy ship and cross shallow waters ... Ours cost % gp each."
            print_shoppe_dat(gm, 0x1a50); // "\n\nWilt thou\ntake it?\" "
            let mut c: u8;
            loop {
                c = input::read_command_key(gm) as u8;
                if c == b'Y' { break; }
                if c == b'N' { break; }
            }
            done = 1;
            if c != b'Y' {
                text::print_string(gm, 0xa004); // "No"  (LAB_1000_ebad)
                return;
            }
            text::print_string(gm, 0x9ffe); // "Yes\n\n"
            let p = gm.gi16(SHOP_PRICE);
            if !shipwright_check_gold(gm, p) {
                return;
            }
            let vessel = gm.g8(g::BOUGHT_VESSEL);
            if vessel & 0xc0 == 0 {
                gm.set_g8(g::BOUGHT_VESSEL, 0x40);
                done = shipwright_deliver(gm, member, 1, 1) as i16;
            } else if vessel & 0x80 == 0 {
                if vessel & 0x40 != 0 {
                    print_shoppe_dat(gm, 0x195f); // "More than one skiff would be purchased only if ye had bought a ship!"
                    done = shipwright_deliver(gm, member, 0, 0) as i16;
                }
                // else LAB_1000_ec89 with done == 1 (unreachable: & 0xc0 != 0 and & 0x80 == 0 implies 0x40)
            } else {
                print_shoppe_dat(gm, 0x193b); // "It will be placed onboard thy frigate!"
                gm.set_g8(g::BOUGHT_VESSEL, vessel.wrapping_add(1));
                done = shipwright_deliver(gm, member, 1, 0) as i16;
            }
        }
        // LAB_1000_ec89
        if done != 0 {
            return;
        }
    }
}

/// orig 1000:ec9c (96 bytes). SHIPWRIGHT shop entry (vector 1778:0a0a, TALK shop type 3) for
/// `member`. Clears the kicked-out (0xbd22) and bought (0xbd24) flags, prints the greeting and
/// waits for a key: space or 'N' prints "No"; 'Y' prints "Yes" and runs `shipwright_menu`.
/// Unless the customer was thrown out, the farewell is printed with the bought flag.
pub fn shipwright_shop(gm: &mut Game, member: u16) {
    let mut leave = false;
    gm.set_gi16(SHIP_KICKED_OUT, 0);
    gm.set_gi16(SHIP_BOUGHT, 0);
    print_greeting(gm);
    loop {
        let k = input::read_command_key(gm);
        if k == 0x20 || k == 0x4e {
            text::print_string(gm, 0xa00c); // "No"
            leave = true;
        } else if k == 0x59 {
            text::print_string(gm, 0xa008); // "Yes"
            shipwright_menu(gm, member as i16);
            leave = true;
        }
        if leave {
            if gm.gi16(SHIP_KICKED_OUT) == 0 {
                let b = gm.gi16(SHIP_BOUGHT);
                print_farewell(gm, b);
            }
            return;
        }
    }
}
