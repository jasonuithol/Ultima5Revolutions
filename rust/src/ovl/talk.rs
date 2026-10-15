//! TALK.OVL (translated)
//!
//! Conversations with settlement NPCs (level-3 overlay, 1000:bf80-1000:d28e). Entry points:
//! `cmd_talk` (vector target c39c, ROOT dispatch_command 'T' in settlements) and `talk_to_npc`
//! (c29e, also TOWN `handle_npc_contact` when a guard/NPC walks into the party). The NPC's
//! dialogue index (NPC state +0xa) selects: 0 "No response", < 0x80 a .TLK entry
//! (`run_conversation`), 0x81..0x88 a shop (`talk_to_shopkeeper` -> SHOPPES overlays, only when
//! the NPC's current schedule entry is 1 = working hours), 0xfd a frightened townsperson,
//! 0xfe a Shadowlord (TOWN `begone_vermin`), 0xff a guard (`guard_challenge`: password at
//! Blackthorn's palace, charity in Minoc, tribute elsewhere; 1 = arrest).
//!
//! .TLK file (towne/dwelling/castle/keep.tlk, names at DGROUP 0x4aa0): header u16 count then
//! {u16 npc index, u16 offset}; the 1 KB entry is loaded into g_ool_buffer (0xb21e) and
//! interpreted by `interpret_byte` through the script pointer 0xbcde. Sections are
//! NUL-terminated strings: 0 name, 1 description, 2 greeting, 3 job, 4 bye, then keyword/
//! answer pairs; a 0x90 byte ends the keyword list and introduces labels (0x90 <label 0x91..>
//! question NUL default-answer NUL {keyword NUL answer NUL}*). Text bytes have bit 7 set
//! (0xa0..0xff = ASCII | 0x80); bytes < 0x81 index the compressed-word table g_tlk_word_ptrs
//! (0x24e8); control bytes: 0x81 Avatar's name, 0x82 end, 0x83 pause, 0x84 join party,
//! 0x85 <3 digit bytes> pay gold, 0x86 <item> give item, 0x87 run the string after the next
//! one, 0x88 ask the Avatar's name, 0x89/0x8a karma +1/-1, 0x8b raise the alarm,
//! 0x8c <label|0xff> if the NPC knows the name, 0x8d newline, 0x8e toggle the rune font,
//! 0x8f wait for a key, 0x91..0x9f go to label (ask a question), 0xfe <karma> <label> karma
//! gate, 0xff "Your interest?" prompt. Output is word-buffered (0xbce4, 16 bytes, 0x4af1
//! length) for word wrap at column 17.
//!
//! Return value convention of the interpreter functions: 1 = the conversation has ended,
//! 0 = continue (2 from `handle_builtin_keyword` = not handled while answering a question).
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// Index (i16) of the NPC being talked to.
const TALK_NPC: u16 = 0xbcdc;
/// Near pointer into the loaded .TLK entry (the script read position).
const SCRIPT_PTR: u16 = 0xbcde;
/// Argument bytes collected for a multi-byte opcode (0x85 price digits, 0xfe karma/label).
const OP_ARGS: u16 = 0xbce0;
/// Pending output word buffer (16 bytes).
const PENDING_BUF: u16 = 0xbce4;
/// Label byte being looked for (0x91..0x9f).
const LABEL: u16 = 0xbcf4;
/// Index (u16) of the NPC keyword that matched.
const KEYWORD_IDX: u16 = 0xbcf6;
/// The player's typed response (NUL-terminated, 15 chars max).
const INPUT: u16 = 0xbcf8;
/// Opcode waiting for argument bytes (0 none).
const PENDING_OP: u16 = 0x4aee;
/// Number of argument bytes collected (byte; the code also touches it as a word with 0x4af0).
const PENDING_ARGC: u16 = 0x4aef;
/// Mask ANDed into every text byte: 0xff normal, 0x7f rune font (DATA.OVL initial 0xff).
const FONT_MASK: u16 = 0x4af0;
/// Length of the pending word buffer.
const PENDING_LEN: u16 = 0x4af1;
/// 0xff while the player answers an NPC's question (NAME/JOB/BYE are then not handled).
const ASKING: u16 = 0x4af2;
/// Cursor column recorded at the last printed character (newline suppression).
const LAST_COL: u16 = 0x4af3;
/// Last text byte emitted (double quotes are collapsed).
const LAST_CHAR: u16 = 0x4af4;
/// Set after a compressed word: the next literal character gets a leading space.
const NEED_SPACE: u16 = 0x4af5;
/// Table of 34 pointers to the built-in keywords (NAME, JOB, BYE, ... swear words).
const BUILTIN_KEYWORDS: u16 = 0x4aa8;

/// 32-bit mask `1 << n` as the compiler's long_shl_inplace produced it: (low word, high word).
#[inline] fn bit_mask32(n: u16) -> (u16, u16) {
    let mut lo: u16 = 1;
    let mut hi: u16 = 0;
    let mut c = n as u8;
    while c != 0 {
        let carry = (lo as i16) < 0;
        lo = lo << 1;
        hi = (hi << 1) | carry as u16;
        c = c.wrapping_sub(1);
    }
    (lo, hi)
}

/// Keyword match test used everywhere: str_find_ci found `needle` in the input at `m`, at the
/// start or right after a space.
#[inline] fn matched_at_word(gm: &Game, m: u16) -> bool {
    m != 0xffff && (m == 0 || gm.gi8(0xbcf7u16.wrapping_add(m)) == 0x20)
}

/// orig 1000:bf80 (82 bytes). Case-insensitive 7-bit comparison of two NUL-terminated DGROUP
/// strings: each pair of characters (masked with 0x7f, upper-cased) must match; stops when
/// both strings have ended after the compared pair. Returns 1 equal, 0 different. (Named
/// CMDS_bf80 by Ghidra: the same address exists in CMDS.) Ghidra order: (s1, s2).
pub fn str_equal_ci(gm: &mut Game, param_1: u16, param_2: u16) -> u16 {
    let mut p1 = param_1;
    let mut p2 = param_2;
    loop {
        let n1 = p1.wrapping_add(1);
        let c1 = util::to_upper((gm.g8(p1) & 0x7f) as i16);
        let n2 = p2.wrapping_add(1);
        let c2 = util::to_upper((gm.g8(p2) & 0x7f) as i16);
        if c2 != c1 {
            return 0;
        }
        p1 = n1;
        p2 = n2;
        if !(gm.g8(n2) != 0 || gm.g8(n1) != 0) { break; }
    }
    1
}

/// orig 1000:bfd4 (85 bytes). Whether the map tile at (y, x) is one the party talks across
/// (shop counters, tables etc.): 0x29, 0x94..0x9c, 0xa5, 0xae, 0xba..0xbb, 0xbe, 0xca..0xcb.
/// Returns 1/0. Ghidra order: (y, x).
pub fn is_talk_through_tile(gm: &mut Game, param_1: u16, param_2: u16) -> u16 {
    let p = rest::get_map_tile(gm, param_1, param_2);
    let t = gm.g8(p);
    if t != 0xa5 {
        let (lt, eq);
        if t < 0xa6 {
            if t == 0x29 { return 1; }
            if t < 0x94 { return 0; }
            lt = t < 0x9c;
            eq = t == 0x9c;
        } else {
            if t < 0xbc {
                if 0xb9 < t { return 1; }
                if t != 0xae { return 0; }
                return 1;
            }
            if t == 0xbe { return 1; }
            if t < 0xca { return 0; }
            lt = t < 0xcb;
            eq = t == 0xcb;
        }
        if !lt && !eq { return 0; }
    }
    1
}

/// orig 1000:c02c (58 bytes). "Dost thou pay? " - waits for Y ("Yes") or N ("No"); returns 0
/// for yes, 1 for no.
pub fn ask_dost_thou_pay(gm: &mut Game) -> u16 {
    text::print_string(gm, 0x9052); // "Dost thou pay? "
    loop {
        let k = input::read_command_key(gm);
        let c = util::to_upper(k) as i8;
        if c == 0x59 {
            text::print_string(gm, 0x9066); // "Yes\n"
            return 0;
        }
        if c == 0x4e { break; }
    }
    text::print_string(gm, 0x906c); // "No\n"
    1
}

/// orig 1000:c066 (234 bytes). Talks to a shopkeeper with dialogue index `dialogue`
/// (0x81..0x88): on horseback (transport 0x12/0x13) anywhere but shop type 2 the merchant
/// shouts "GET THAT HORSE OUT OF HERE!". Otherwise the acting member is g_active_member or the
/// first conscious one (g_dx), the shop type (dialogue - 0x81) goes to 0xb116, the slot of
/// g_location in g_shop_towns[type] to 0xb114, the shop and shopkeeper name pointers
/// (g_shop_name_ptrs / g_shopkeeper_name_ptrs [type][slot]) to 0xaafc/0xaafe, and the shop
/// overlay function for the type is called: 0 SHOPPES b542, 1 SHOPPES2 e84c, 2 SHOPPES aa4e,
/// 3 SHOPPES2 ec9c, 4 SHOPPES a9ee, 5 SHOPPES a732, 6 SHOPPES b788, 7 SHOPPES3 ea94.
pub fn talk_to_shopkeeper(gm: &mut Game, param_1: i16) {
    let dialogue = param_1;
    if (gm.g8(g::TRANSPORT_TILE) & 0xfe) == 0x12 && dialogue != 0x83 {
        text::print_string(gm, 0x9072); // "A merchant says: \"GET THAT HORSE OUT OF HERE!\"\n"
        return;
    }
    let member: u16 = if gm.g8(g::ACTIVE_MEMBER) == 0xff {
        rest::find_first_conscious_member(gm);
        gm.g16(g::DX)
    } else {
        gm.g8(g::ACTIVE_MEMBER) as u16
    };
    let ty = dialogue.wrapping_sub(0x81);
    gm.set_gi16(0xb116, ty); // g_shop_type
    gm.set_gi16(0xb114, 0); // g_shop_slot
    let loc = gm.g8(g::LOCATION);
    if gm.g8((ty as u16).wrapping_mul(0x10).wrapping_add(0x23ca)) != loc {
        gm.set_gi16(0xb114, 0);
        loop {
            if 0xf < gm.gi16(0xb114) { break; }
            let s = gm.gi16(0xb114).wrapping_add(1);
            gm.set_gi16(0xb114, s);
            if gm.g8((ty as u16).wrapping_mul(0x10).wrapping_add(s as u16).wrapping_add(0x23ca)) == loc { break; }
        }
    }
    let i = (ty as u16).wrapping_mul(0x20).wrapping_add((gm.gi16(0xb114) as u16).wrapping_mul(2));
    let v = gm.g16(i.wrapping_add(0x21ca));
    gm.set_g16(0xaafc, v); // g_shop_name_ptr
    let v = gm.g16(i.wrapping_add(0x22ca));
    gm.set_g16(0xaafe, v); // g_shopkeeper_name_ptr
    match ty {
        0 => ovl::shoppes::arms_shop(gm, member),
        1 => ovl::shoppes2::tavern_shop(gm, member),
        2 => ovl::shoppes::horse_shop(gm, member as i16),
        3 => ovl::shoppes2::shipwright_shop(gm, member),
        4 => ovl::shoppes::reagent_shop(gm, member),
        5 => ovl::shoppes::guild_shop(gm, member),
        6 => ovl::shoppes::healer_shop(gm, member),
        7 => ovl::shoppes3::inn_shop(gm, member as i16),
        _ => {}
    }
}

/// orig 1000:c162 (314 bytes). A guard (dialogue 0xff) challenges the party; returns 1 = arrest.
/// Location 0x12 (Blackthorn's palace): with status icon 0x1d ("bearer of the badge") the
/// password is asked (14 chars read, truncated to 4) and compared with DGROUP 0x4a9a
/// ("IMPE"): match -> "Pass, friend." returns 0; otherwise 1. Location 5 (Minoc): "Thou wilt
/// give half thy gold to charity?" - refusing returns 1, else gold /= 2. Elsewhere: a tribute of
/// 10 gp per living party member is demanded; refusing or lacking the gold returns 1, else it is
/// paid. The status window is redrawn and 0 returned.
pub fn guard_challenge(gm: &mut Game) -> u16 {
    if gm.gi8(g::LOCATION) == 0x12 {
        if gm.gi8(g::STATUS_ICON) == 0x1d {
            text::put_char(gm, 0x22);
            text::print_string(gm, 0x90fc); // "Give now the password, bearer of the badge!"
            text::put_char(gm, 0x22);
            text::print_string(gm, 0x9128); // "\nYour response: "
            let ok = gm.with_buf(16, |gm, buf| {
                rest::read_text_line(gm, 0xe, buf);
                text::put_char(gm, 10);
                gm.set_g8(buf.wrapping_add(4), 0);
                str_equal_ci(gm, buf, 0x4a9a) // "IMPE"
            });
            if ok != 0 {
                text::put_char(gm, 10);
                text::put_char(gm, 0x22);
                text::print_string(gm, 0x913a); // "Pass, friend."
                text::put_char(gm, 0x22);
                text::put_char(gm, 10);
                return 0;
            }
        }
        return 1;
    }
    if gm.gi8(g::LOCATION) == 5 {
        text::put_char(gm, 0x22);
        text::print_string(gm, 0x90a2); // "Thou wilt give half thy gold to charity?"
        text::put_char(gm, 0x22);
        if ask_dost_thou_pay(gm) != 0 {
            return 1;
        }
        let v = gm.gi16(g::GOLD) / 2;
        gm.set_gi16(g::GOLD, v);
    } else {
        let mut tribute: i16 = 0;
        if gm.g8(g::PARTY_COUNT) != 0 {
            let mut p = g::PARTY_STATUS;
            let end = g::PARTY_STATUS.wrapping_add((gm.g8(g::PARTY_COUNT) as u16).wrapping_mul(0x20));
            loop {
                if gm.g8(p) != 0x44 {
                    tribute += 10;
                }
                p = p.wrapping_add(0x20);
                if p == end { break; }
            }
        }
        text::print_string(gm, 0x90cc); // "A guard demands a "
        text::print_number(gm, 0x20, 2, tribute);
        text::print_string(gm, 0x90e0); // "gp tribute to Blackthorn!\n"
        if ask_dost_thou_pay(gm) != 0 {
            return 1;
        }
        if gm.gi16(g::GOLD) < tribute {
            return 1;
        }
        let v = gm.gi16(g::GOLD).wrapping_sub(tribute);
        gm.set_gi16(g::GOLD, v);
    }
    rest::draw_status_window(gm);
    0
}

/// orig 1000:c29e (246 bytes). Talks to NPC `npc` (also TOWN's guard/NPC contact handler).
/// Prints a newline and stores the NPC in 0xbcdc. If the AI type of its current schedule entry
/// is 4 (guard post) it becomes 1 (wander near post); otherwise a guard-tile NPC (slot tile
/// 0x70) whose schedule entry is even or whose dialogue index is 0 says "The guard offers no
/// response." Dialogue 0: "No response"; < 0x80: `run_conversation`; 0xfd: "Don't hurt me!
/// Please go away!"; 0xfe: TOWN `begone_vermin`; 0xff: returns `guard_challenge` (1 = arrest);
/// shopkeepers (0x81..) are served through `talk_to_shopkeeper` only when the stored entry is odd
/// and the hour's entry is odd (working hours), else "A merchant says: Come see me at ... when
/// it's open!". Returns 0 in all other cases.
pub fn talk_to_npc(gm: &mut Game, param_1: i16) -> u16 {
    let npc = param_1;
    text::put_char(gm, 10);
    gm.set_gi16(TALK_NPC, npc);
    let n16 = (npc as u16).wrapping_mul(0x10);
    let ai = gm.g16(n16.wrapping_add(0x5f6c)).wrapping_add(n16).wrapping_add(g::NPC_SCHEDULES);
    let s: u16;
    'resp: {
        if gm.gi8(ai) == 4 {
            gm.set_gi8(ai, 1);
        } else if gm.gi8(g::MONSTER_TABLE.wrapping_add((gm.gi16(n16.wrapping_add(0x5f6a)) as u16).wrapping_mul(8))) == 0x70
            && ((gm.g8(n16.wrapping_add(0x5f6c)) & 1) == 0 || gm.gi16(n16.wrapping_add(0x5f68)) == 0)
        {
            s = 0x9148; // "The guard offers no response.\n"
            break 'resp;
        }
        let d = gm.gi16(n16.wrapping_add(0x5f68)); // dialogue index
        if d == 0 {
            s = 0x9168; // "No response!\n"
        } else {
            if d < 0x80 {
                run_conversation(gm, d);
                return 0;
            }
            if d == 0xfd {
                text::put_char(gm, 0x22);
                text::print_string(gm, 0x9176); // "Don't hurt me! Please go away!"
                text::put_char(gm, 0x22);
                text::put_char(gm, 10);
                return 0;
            }
            if d == 0xfe {
                // TODO(verify): the original pushes no argument; TOWN 92aa reads the caller's
                // saved SI as `npc`. The NPC being talked to is passed here.
                ovl::town::begone_vermin(gm, npc as u16);
                return 0;
            }
            if d == 0xff {
                return guard_challenge(gm);
            }
            if (gm.g8(n16.wrapping_add(0x5f6c)) & 1) != 0
                && (ovl::npc::schedule_index_for_hour(gm, gm.g8(g::HOUR) as i8, gm.gi16(TALK_NPC)) & 1) != 0
            {
                talk_to_shopkeeper(gm, d);
                return 0;
            }
            text::print_string(gm, 0x9196); // "A merchant says: \"Come see me at ..."
            s = 0x91c4; // "when it's open!\"\n"
        }
    }
    text::print_string(gm, s);
    0
}

/// orig 1000:c39c (181 bytes). The Talk command in a settlement (ROOT dispatch_command 'T').
/// Reads a direction (0 -> returns 0); the target cell is one step away, or two when the first
/// cell holds no object and is a talk-through tile (counter). No object: "Nobody's here!".
/// Otherwise the NPC owning the object's slot (find_npc_by_slot(g_dx)) is found; a mirror
/// (0x9d) under it gives "No response", a bed/sleeping tile 0xab the "Zzzz" string 0x91e8,
/// anything else returns `talk_to_npc` (non-zero = arrest, ROOT returns 2).
pub fn cmd_talk(gm: &mut Game) -> u16 {
    let d = rest::get_direction(gm);
    let dy = gm.gi16(g::DY);
    let dx = gm.gi16(g::DX);
    if d != 0 {
        let mut x = gm.gi16(g::DX).wrapping_add(gm.g8(g::PARTY_X) as i16);
        let mut y = gm.gi16(g::DY).wrapping_add(gm.g8(g::PARTY_Y) as i16);
        let obj = rest::find_object_at(gm, gm.g8(g::PARTY_Z) as u16, y as u16, x as u16);
        if obj == 0 && is_talk_through_tile(gm, y as u16, x as u16) != 0 {
            x = x.wrapping_add(dx);
            y = y.wrapping_add(dy);
        }
        let obj = rest::find_object_at(gm, gm.g8(g::PARTY_Z) as u16, y as u16, x as u16);
        let s: u16;
        if obj == 0 {
            s = 0x91d6; // "Nobody's here!\n"
        } else {
            let npc = ovl::town::find_npc_by_slot(gm, gm.gi16(g::DX));
            let p = rest::get_map_tile(gm, y as u16, x as u16);
            if gm.gi8(p) == -99 {
                s = 0x91f6; // "No response!\n"
            } else {
                if gm.gi8(p) != -0x55 {
                    return talk_to_npc(gm, npc);
                }
                s = 0x91e8; // "Zzzz..." (sleeping)
            }
        }
        text::print_string(gm, s);
    }
    0
}

/// orig 1000:c452 (8 bytes). Emits a newline through the interpreter (byte 0x8d).
pub fn emit_newline(gm: &mut Game) {
    interpret_byte(gm, 0x8d);
}

/// orig 1000:c45a (8 bytes). Emits a double quote through the interpreter (byte 0xa2).
pub fn emit_quote(gm: &mut Game) {
    interpret_byte(gm, 0xa2);
}

/// orig 1000:c462 (143 bytes). Flushes the pending word buffer: each byte selects font 1 when
/// its bit 7 is clear (runes) else font 0, is masked to 7 bits and printed, except a space at
/// column 0 and a newline while the recorded column (0x4af3) is >= 15 (the line already
/// wrapped; the record is reset instead). Non-newline characters record the cursor column.
/// Clears the buffer and selects font 0.
pub fn flush_pending_text(gm: &mut Game) {
    if gm.g8(PENDING_LEN) != 0 {
        let mut i: u16 = 0;
        while i < gm.g8(PENDING_LEN) as u16 {
            let b = gm.g8(PENDING_BUF.wrapping_add(i));
            text::select_font(gm, ((b & 0x80) == 0) as u16);
            let c = b & 0x7f;
            i += 1;
            let col = text::get_cursor_col(gm);
            if col != 0 || c != 0x20 {
                if gm.g8(LAST_COL) < 0xf || c != 10 {
                    if c != 10 {
                        let col = text::get_cursor_col(gm);
                        gm.set_g8(LAST_COL, col as u8);
                    }
                    text::put_char(gm, c);
                } else {
                    gm.set_g8(LAST_COL, 0);
                }
            }
        }
        gm.set_g8(PENDING_LEN, 0);
        text::select_font(gm, 0);
    }
}

/// orig 1000:c4f4 (65 bytes). Appends `ch` to the pending word buffer (unless it holds 16
/// bytes already, which flushes first). A newline (0x8a) or space (0xa0) ends a word: if the
/// cursor column plus the pending length exceeds 17 a newline is printed first (word wrap),
/// then the buffer is flushed.
pub fn queue_char(gm: &mut Game, param_1: u8) {
    let ch = param_1;
    if gm.g8(PENDING_LEN) != 0x10 {
        let n = gm.g16(PENDING_LEN) & 0xff;
        let v = gm.g16(PENDING_LEN).wrapping_add(1);
        gm.set_g16(PENDING_LEN, v);
        gm.set_g8(PENDING_BUF.wrapping_add(n), ch);
        if ch != 0x8a && ch != 0xa0 {
            return;
        }
        let col = text::get_cursor_col(gm);
        if 0x11 < col.wrapping_add((gm.g16(PENDING_LEN) & 0xff) as i16) {
            text::put_char(gm, 10);
        }
    }
    flush_pending_text(gm);
}

/// orig 1000:c536 (204 bytes). Pays the price given by the three collected digit bytes
/// (0xbce0..2, ASCII | 0x80): not enough gold -> the pending text is dropped, "Thou hast not
/// enough gold." is printed, the pending opcode cleared and the "Your interest?" loop entered
/// (its result returned). Otherwise the gold is deducted and the status window redrawn; paying
/// an NPC whose tile group is 0x6c (beggar) at most once per 100 turns (g_turn_counter reset)
/// gives +1 karma, +2 more when it took the last coin. Returns 0.
pub fn pay_gold_price(gm: &mut Game) -> u16 {
    let price: i16 = ((gm.g8(OP_ARGS) & 0x7f) as i16)
        .wrapping_mul(100)
        .wrapping_add(((gm.g8(OP_ARGS.wrapping_add(1)) & 0x7f) as i16).wrapping_mul(10))
        .wrapping_add((gm.g8(OP_ARGS.wrapping_add(2)) & 0x7f) as i16)
        .wrapping_sub(0x14d0);
    if gm.gi16(g::GOLD) < price {
        gm.set_g8(PENDING_LEN, 0);
        text::put_char(gm, 0x22);
        text::print_string(gm, 0x9328); // "Thou hast not enough gold."
        text::put_char(gm, 0x22);
        text::print_string(gm, 0x9344);
        gm.set_g8(PENDING_OP, 0);
        gm.set_g8(PENDING_ARGC, 0);
        return interest_loop(gm);
    }
    let v = gm.gi16(g::GOLD).wrapping_sub(price);
    gm.set_gi16(g::GOLD, v);
    rest::draw_status_window(gm);
    let slot8 = (gm.gi16((gm.gi16(TALK_NPC) as u16).wrapping_mul(0x10).wrapping_add(0x5f6a)) as u16).wrapping_mul(8);
    if (gm.g8(g::MONSTER_TABLE.wrapping_add(slot8)) & 0xfc) == 0x6c && 99 < gm.g8(g::TURN_COUNTER) {
        gm.set_g8(g::TURN_COUNTER, 0);
        rest::add_u8_capped(gm, 99, 1, g::KARMA);
        if gm.gi16(g::GOLD) == 0 {
            rest::add_u8_capped(gm, 99, 2, g::KARMA);
        }
    }
    0
}

/// orig 1000:c602 (133 bytes). Gives the party item `id`: < 0x40 one unit of g_inventory[id]
/// (cap 99); 0x41 food +1 / 0x42 gold +1 (cap 9999, status redrawn); 0x43 keys, 0x44 gems,
/// 0x45 torches, 0x46 grapple, 0x47 magic carpets, 0x4b skull keys +1 (cap 99); 0x48 sextant,
/// 0x49 spyglass, 0x4a black badge set to 0xff.
pub fn give_item(gm: &mut Game, param_1: u8) {
    let id = param_1;
    if id < 0x40 {
        rest::add_u8_capped(gm, 99, 1, (id as u16).wrapping_add(0x57c0));
        return;
    }
    let p: u16;
    match id {
        0x41 | 0x42 => {
            let q = if id == 0x41 { g::FOOD } else { g::GOLD };
            rest::add_i16_capped(gm, 0x270f, 1, q);
            rest::draw_status_window(gm);
            return;
        }
        0x43 => p = g::KEYS,
        0x44 => p = g::GEMS,
        0x45 => p = g::TORCHES,
        0x46 => p = 0x57af, // g_grapple
        0x47 => p = 0x57b0, // g_magic_carpets
        0x48 => { gm.set_g8(g::SEXTANTS, 0xff); return; }
        0x49 => { gm.set_g8(g::SPYGLASSES, 0xff); return; }
        0x4a => { gm.set_g8(g::BLACK_BADGE, 0xff); return; }
        0x4b => p = 0x57b1, // g_skull_keys
        _ => return,
    }
    rest::add_u8_capped(gm, 99, 1, p);
}

/// orig 1000:c6a8 (49 bytes). Advances the script pointer past bytes until one equals `end`
/// (returns 1) or `stop` (returns 0); the matching byte is consumed. Ghidra order:
/// (stop, end).
pub fn skip_until(gm: &mut Game, param_1: i8, param_2: i8) -> u16 {
    loop {
        let p = gm.g16(SCRIPT_PTR);
        gm.set_g16(SCRIPT_PTR, p.wrapping_add(1));
        let c = gm.gi8(p);
        if c == param_2 {
            return 1;
        }
        if c == param_1 { break; }
    }
    0
}

/// orig 1000:c6da (46 bytes). Rewinds the script pointer to the start of the entry
/// (g_ool_buffer) and skips `n` NUL-terminated sections (a 0x90 byte also ends a skip).
pub fn seek_section(gm: &mut Game, param_1: i16) {
    gm.set_g16(SCRIPT_PTR, g::OOL_BUFFER);
    let mut n = param_1;
    while n != 0 {
        skip_until(gm, -0x70, 0); // 0x90, NUL
        n -= 1;
    }
}

/// orig 1000:c708 (33 bytes). Interprets script bytes from the script pointer until a NUL
/// (returns 0) or until `interpret_byte` reports the end of the conversation (returns 1).
pub fn run_script(gm: &mut Game) -> u16 {
    loop {
        let p = gm.g16(SCRIPT_PTR);
        if gm.g8(p) == 0 {
            return 0;
        }
        gm.set_g16(SCRIPT_PTR, p.wrapping_add(1));
        let b = gm.g8(p);
        if interpret_byte(gm, b) != 0 { break; }
    }
    1
}

/// orig 1000:c72a (19 bytes). Seeks section `n` of the entry and runs it; returns `run_script`'s
/// result (1 = conversation over).
pub fn run_section(gm: &mut Game, param_1: u8) -> u16 {
    seek_section(gm, param_1 as i16);
    run_script(gm)
}

/// orig 1000:c73e (37 bytes). Opcode 0x87: skips the byte after the opcode (and any NULs), then
/// the following NUL-terminated string, and runs the string after that; returns `run_script`'s
/// result.
pub fn skip_string_and_run(gm: &mut Game) -> u16 {
    let mut p = gm.g16(SCRIPT_PTR);
    loop {
        p = p.wrapping_add(1);
        if gm.g8(p) != 0 { break; }
    }
    loop {
        gm.set_g16(SCRIPT_PTR, p.wrapping_add(1));
        let c = gm.g8(p);
        p = p.wrapping_add(1);
        if c == 0 { break; }
    }
    run_script(gm)
}

/// orig 1000:c764 (37 bytes). Emits the Avatar's name (party record 0, NUL-terminated) as text
/// bytes (| 0x80) through `interpret_byte`.
pub fn emit_avatar_name(gm: &mut Game) {
    let mut p = g::PARTY;
    while gm.g8(p) != 0 {
        let b = gm.g8(p) | 0x80;
        interpret_byte(gm, b);
        p = p.wrapping_add(1);
    }
}

/// orig 1000:c78a (301 bytes). Opcode 0x84: the NPC joins the party. With 6 members already a
/// key is read and "Thou hast no room for me in thy party! Seek me again if one of thy members
/// leaves" is printed (returns 0). Otherwise the first three letters of the NPC's name (section
/// 0) are compared case-insensitively with the first three letters of party records 15 down to
/// 1; the matching record gets byte +0x1f = 0 (inn/party flag), is swapped with record
/// g_party_count, the count is incremented, the NPC is flagged killed and removed from the map
/// (TOWN set_npc_killed / remove_npc), the status window redrawn and pending text flushed
/// (returns 1). No match: a quote and newline are queued and "System Error - No Match!" printed
/// (returns 1). The script pointer is restored.
pub fn join_party(gm: &mut Game) -> u16 {
    let saved = gm.g16(SCRIPT_PTR);
    if gm.g8(g::PARTY_COUNT) == 6 {
        input::read_command_key(gm);
        text::print_string(gm, 0x9348); // "\"Thou hast no room for me in thy party!"
        text::print_string(gm, 0x9372); // "Seek me again if one of thy members leaves.\"\n"
        return 0;
    }
    seek_section(gm, 0);
    let p = gm.g16(SCRIPT_PTR);
    let name = [gm.g8(p), gm.g8(p.wrapping_add(1)), gm.g8(p.wrapping_add(2)), 0u8];
    gm.set_g16(SCRIPT_PTR, p.wrapping_add(3));
    let mut i: i16 = 0xf;
    let mut off: u16 = 0xf << 5;
    loop {
        let rec = g::PARTY.wrapping_add(off);
        let member = [gm.g8(rec), gm.g8(rec.wrapping_add(1)), gm.g8(rec.wrapping_add(2)), 0u8];
        let eq = gm.with_buf(4, |gm, a| {
            for k in 0..4u16 { gm.set_g8(a.wrapping_add(k), member[k as usize]); }
            gm.with_buf(4, |gm, b| {
                for k in 0..4u16 { gm.set_g8(b.wrapping_add(k), name[k as usize]); }
                str_equal_ci(gm, a, b)
            })
        });
        if eq != 0 {
            let rec = g::PARTY.wrapping_add((i as u16).wrapping_mul(0x20));
            gm.set_g8(rec.wrapping_add(0x1f), 0); // party record +0x1f (inn / party flag)
            let mut tmp = [0u8; 32];
            for k in 0..32u16 { tmp[k as usize] = gm.g8(rec.wrapping_add(k)); }
            let last = g::PARTY.wrapping_add((gm.g8(g::PARTY_COUNT) as u16).wrapping_mul(0x20));
            for k in 0..32u16 {
                let v = gm.g8(last.wrapping_add(k));
                gm.set_g8(rec.wrapping_add(k), v);
            }
            for k in 0..32u16 { gm.set_g8(last.wrapping_add(k), tmp[k as usize]); }
            let c = gm.g8(g::PARTY_COUNT).wrapping_add(1);
            gm.set_g8(g::PARTY_COUNT, c);
            ovl::town::set_npc_killed(gm, gm.gi16(TALK_NPC));
            ovl::town::remove_npc(gm, gm.gi16(TALK_NPC));
            rest::draw_status_window(gm);
            flush_pending_text(gm);
            gm.set_g16(SCRIPT_PTR, saved);
            return 1;
        }
        off = off.wrapping_sub(0x20);
        i -= 1;
        if off == 0 { break; }
    }
    queue_char(gm, 0x22);
    queue_char(gm, 10);
    text::print_string(gm, 0x93a8); // "System Error - No Match!"
    gm.set_g16(SCRIPT_PTR, saved);
    1
}

/// orig 1000:c8ba (38 bytes). Finds the label byte 0xbcf4 in the entry (scanning sections
/// delimited by 0x90/0x9f from the start) and runs the label's text (the question); returns
/// `run_script`'s result.
pub fn run_label_text(gm: &mut Game) -> u16 {
    gm.set_g16(SCRIPT_PTR, g::OOL_BUFFER);
    while gm.g8(gm.g16(SCRIPT_PTR)) != gm.g8(LABEL) {
        skip_until(gm, -0x61, -0x70); // 0x9f, 0x90
    }
    let p = gm.g16(SCRIPT_PTR).wrapping_add(1);
    gm.set_g16(SCRIPT_PTR, p);
    run_script(gm)
}

/// orig 1000:c8e0 (14 bytes). Skips to the end of the current string (the matched keyword) and
/// runs the next one (its answer); returns `run_script`'s result.
pub fn run_next_string(gm: &mut Game) -> u16 {
    skip_until(gm, -0x70, 0); // 0x90, NUL
    run_script(gm)
}

/// orig 1000:c8ee (44 bytes). Finds the label 0xbcf4, skips its question text and runs the
/// default answer that follows; returns `run_script`'s result.
pub fn run_label_default_answer(gm: &mut Game) -> u16 {
    gm.set_g16(SCRIPT_PTR, g::OOL_BUFFER);
    while gm.g8(gm.g16(SCRIPT_PTR)) != gm.g8(LABEL) {
        skip_until(gm, -0x61, -0x70); // 0x9f, 0x90
    }
    skip_until(gm, -0x61, 0); // 0x9f, NUL
    run_script(gm)
}

/// orig 1000:c91a (62 bytes). Positions the script pointer at NPC keyword `k`: from the entry
/// start skips 2k+5 NUL-terminated sections; returns 0 when a 0x90 byte (end of the keyword
/// list) is met first, else 1.
pub fn seek_keyword(gm: &mut Game, param_1: i16) -> u16 {
    gm.set_g16(SCRIPT_PTR, g::OOL_BUFFER);
    let mut n = param_1.wrapping_mul(2).wrapping_add(5);
    loop {
        if n == 0 {
            return 1;
        }
        if skip_until(gm, -0x70, 0) == 0 { break; }
        n -= 1;
    }
    0
}

/// orig 1000:c958 (83 bytes). Searches the NPC's keywords for one found in the typed response
/// (str_find_ci, at the start or after a space): sets 0xbcf6 to the keyword index and leaves
/// the script pointer on the keyword, returns 1; 0 when the keyword list is exhausted.
pub fn find_npc_keyword(gm: &mut Game) -> u16 {
    gm.set_g16(KEYWORD_IDX, 0);
    loop {
        if seek_keyword(gm, gm.gi16(KEYWORD_IDX)) == 0 {
            return 0;
        }
        let m = rest::str_find_ci(gm, INPUT as i16, gm.g16(SCRIPT_PTR));
        if matched_at_word(gm, m) { break; }
        let p = gm.g16(SCRIPT_PTR).wrapping_add(1);
        gm.set_g16(SCRIPT_PTR, p);
        let k = gm.g16(KEYWORD_IDX).wrapping_add(1);
        gm.set_g16(KEYWORD_IDX, k);
    }
    1
}

/// orig 1000:c9ac (15 bytes). Flushes pending text and reads the player's response (15 chars)
/// into 0xbcf8.
pub fn read_response(gm: &mut Game) {
    flush_pending_text(gm);
    rest::read_text_line(gm, 0xf, INPUT);
}

/// orig 1000:c9bc (24 bytes). Says goodbye: a quote, then section 4 (bye); if that did not end
/// the conversation a quote and newline are emitted. Returns 1.
pub fn say_bye(gm: &mut Game) -> u16 {
    emit_quote(gm);
    if run_section(gm, 4) == 0 {
        emit_quote(gm);
        emit_newline(gm);
    }
    1
}

/// orig 1000:c9d4 (174 bytes). Handles built-in keyword `idx` (0 NAME, 1-2 JOB/WORK, 3-4 BYE,
/// others insults). While answering a question (0x4af2) the first five return 2 (not handled).
/// NAME: "My name is " + section 0; JOB: a quote + section 3; both return 1 if the script ended
/// the conversation, else emit quote, newline, newline and return 0. BYE: returns `say_bye`.
/// Insults: "With language like that, how did thy mother ever get along with thee?", quote,
/// newlines, a 28-tick pause (or a key) and the keyboard flushed; returns 0.
pub fn handle_builtin_keyword(gm: &mut Game, param_1: u8) -> u16 {
    let idx = param_1;
    let r: u16;
    if idx == 0 {
        if gm.g8(ASKING) != 0 {
            return 2;
        }
        text::print_string(gm, 0x93c2); // "\"My name is "
        r = run_section(gm, 0);
    } else if idx < 3 {
        if gm.g8(ASKING) != 0 {
            return 2;
        }
        emit_quote(gm);
        r = run_section(gm, 3);
    } else if 2 < idx && idx < 5 {
        if gm.g8(ASKING) != 0 {
            return 2;
        }
        return say_bye(gm);
    } else {
        text::print_string(gm, 0x93d0); // "\"With language like that, how did thy mother..."
        emit_quote(gm);
        emit_newline(gm);
        emit_newline(gm);
        let mut i: i16 = 0;
        loop {
            rest::refresh_map_view(gm);
            if input::read_key(gm) != 0 { break; }
            util::wait_ticks(gm, 1);
            i += 1;
            if !(i < 0x1c) { break; }
        }
        input::flush_keyboard(gm);
        return 0;
    }
    if r != 0 {
        return 1;
    }
    emit_quote(gm);
    emit_newline(gm);
    emit_newline(gm);
    0
}

/// orig 1000:ca84 (204 bytes). The "Your interest? " loop. An empty response says goodbye
/// (returns `say_bye`). The 34 built-in keywords are tried first (`handle_builtin_keyword`:
/// 0 = handled, loop again; 1 = conversation over, returns 1; 2 = not handled). Then the NPC's
/// own keywords (`find_npc_keyword`): found -> a quote and the answer section 2k+6 (returns 1
/// if it ended the conversation), else "I cannot help thee with that."; a quote and two
/// newlines close the answer and the loop repeats.
pub fn interest_loop(gm: &mut Game) -> u16 {
    loop {
        gm.set_g8(ASKING, 0);
        text::print_string(gm, 0x9408); // "Your interest? "
        read_response(gm);
        if gm.g8(INPUT) == 0 {
            text::print_string(gm, 0x941a);
            return say_bye(gm);
        }
        emit_newline(gm);
        emit_newline(gm);
        let mut idx: u8 = 0;
        let mut r: i16 = -1;
        let mut p = BUILTIN_KEYWORDS;
        loop {
            let m = rest::str_find_ci(gm, INPUT as i16, gm.g16(p));
            if matched_at_word(gm, m) {
                r = handle_builtin_keyword(gm, idx) as i16;
                if r == 0 { break; }
                if r == 1 {
                    return 1;
                }
            }
            p = p.wrapping_add(2);
            idx += 1;
            if !(idx < 0x22) { break; }
        }
        if r != 0 {
            r = find_npc_keyword(gm) as i16;
            if r == 0 {
                text::print_string(gm, 0x9420); // "\"I cannot help thee with that.\""
            } else {
                emit_quote(gm);
                r = run_section(gm, gm.g16(KEYWORD_IDX).wrapping_mul(2).wrapping_add(6) as u8) as i16;
                if r != 0 {
                    return 1;
                }
            }
            emit_quote(gm);
            emit_newline(gm);
            emit_newline(gm);
        }
    }
}

/// orig 1000:cb54 (133 bytes). Searches the keyword/answer pairs of label 0xbcf4 (after its
/// question and default answer) for a keyword in the response; returns 1 with the script
/// pointer on the keyword, 0 when the label's list ends (0x90).
pub fn find_label_keyword(gm: &mut Game) -> u16 {
    gm.set_g16(SCRIPT_PTR, g::OOL_BUFFER);
    while gm.g8(gm.g16(SCRIPT_PTR)) != gm.g8(LABEL) {
        skip_until(gm, -0x61, -0x70); // 0x9f, 0x90
    }
    skip_until(gm, -0x61, 0); // 0x9f, NUL
    skip_until(gm, -0x61, 0);
    loop {
        let m = rest::str_find_ci(gm, INPUT as i16, gm.g16(SCRIPT_PTR));
        if matched_at_word(gm, m) {
            return 1;
        }
        if skip_until(gm, -0x70, 0) == 0 { break; }
        if skip_until(gm, -0x70, 0) == 0 { break; }
    }
    0
}

/// orig 1000:cbdc (226 bytes). Asks the question at label 0xbcf4: a quote and the label's text
/// (1 if it ended the conversation); then, with 0x4af2 = 0xff, "You respond: " is asked until
/// non-empty ("What didst thou say?" otherwise). Built-in keywords are tried: handled (0)
/// repeats the question, 1 ends the conversation. Otherwise the label's own keywords
/// (`find_label_keyword`) select the answer after the keyword or the default answer; if that
/// does not end the conversation a quote and two newlines are emitted and the "Your interest?"
/// loop resumes (its result returned); else 1.
pub fn ask_question_at_label(gm: &mut Game) -> u16 {
    let mut r: i16;
    loop {
        emit_quote(gm);
        r = run_label_text(gm) as i16;
        if r != 0 {
            return 1;
        }
        emit_quote(gm);
        loop {
            emit_newline(gm);
            emit_newline(gm);
            gm.set_g8(ASKING, 0xff);
            text::print_string(gm, 0x9440); // "You respond: "
            read_response(gm);
            if gm.g8(INPUT) == 0 {
                text::print_string(gm, 0x9450); // "\"What didst thou say?\"\n"
            }
            if gm.g8(INPUT) != 0 { break; }
        }
        emit_newline(gm);
        emit_newline(gm);
        let mut idx: u8 = 0;
        r = -1;
        let mut p = BUILTIN_KEYWORDS;
        loop {
            let m = rest::str_find_ci(gm, INPUT as i16, gm.g16(p));
            if matched_at_word(gm, m) {
                r = handle_builtin_keyword(gm, idx) as i16;
                if r == 0 { break; }
                if r == 1 {
                    return 1;
                }
            }
            p = p.wrapping_add(2);
            idx += 1;
            if !(idx < 0x22) { break; }
        }
        if r != 0 { break; }
    }
    let r = if find_label_keyword(gm) == 0 {
        emit_quote(gm);
        run_label_default_answer(gm)
    } else {
        emit_quote(gm);
        run_next_string(gm)
    };
    if r == 0 {
        emit_quote(gm);
        emit_newline(gm);
        emit_newline(gm);
        return interest_loop(gm);
    }
    1
}

/// orig 1000:ccc2 (56 bytes). Records that NPC `npc` of the current settlement has been told
/// the Avatar's name: sets bit npc of the 32-bit word at 0x5bd6 + 4*g_location (g_npc_met_flags).
pub fn set_npc_met(gm: &mut Game, param_1: u16) {
    let (lo, hi) = bit_mask32(param_1);
    let base = (gm.g8(g::LOCATION) as u16).wrapping_mul(4);
    let v = gm.g16(base.wrapping_add(0x5bd6)) | lo;
    gm.set_g16(base.wrapping_add(0x5bd6), v);
    let v = gm.g16(base.wrapping_add(0x5bd8)) | hi;
    gm.set_g16(base.wrapping_add(0x5bd8), v);
}

/// orig 1000:ccfa (67 bytes). Whether NPC `npc` of the current settlement knows the Avatar's
/// name (bit npc of the word at 0x5bd6 + 4*g_location).
pub fn is_npc_met(gm: &mut Game, param_1: u16) -> bool {
    let (lo, hi) = bit_mask32(param_1);
    let base = (gm.g8(g::LOCATION) as u16).wrapping_mul(4);
    (gm.g16(base.wrapping_add(0x5bd8)) & hi) != 0 || (gm.g16(base.wrapping_add(0x5bd6)) & lo) != 0
}

/// orig 1000:cd3e (183 bytes). Consumes argument byte `b` of the pending opcode 0x4aee.
/// 0x85 (pay): collects 3 digit bytes then `pay_gold_price` (1 ends the conversation). 0x86
/// (give): `give_item(b & 0x7f)`. 0x8c (if the NPC knows the name): cleared; when the name is
/// not known returns 0 (text continues); b == 0xff -> `interest_loop`, else label b ->
/// `ask_question_at_label`. 0xfe (karma gate): collects threshold and label; karma below the
/// threshold -> 0, else the label is asked. Any other state clears the opcode. Returns 0 unless
/// a called routine ended the conversation.
pub fn interpret_opcode_arg(gm: &mut Game, param_1: u8) -> u16 {
    let b = param_1;
    let op = gm.gi8(PENDING_OP);
    if op == -0x7b { // 0x85
        let n = gm.g16(PENDING_ARGC) & 0xff;
        let v = gm.g16(PENDING_ARGC).wrapping_add(1);
        gm.set_g16(PENDING_ARGC, v);
        gm.set_g8(OP_ARGS.wrapping_add(n), b);
        if gm.gi8(PENDING_ARGC) != 3 {
            return 0;
        }
        if pay_gold_price(gm) != 0 {
            return 1;
        }
    } else if op != -0x7a { // not 0x86
        if op == -0x74 { // 0x8c
            gm.set_g8(PENDING_OP, 0);
            let v = gm.g16(PENDING_ARGC) & 0xff00;
            gm.set_g16(PENDING_ARGC, v);
            if !is_npc_met(gm, gm.gi16(TALK_NPC) as u16) {
                return 0;
            }
            if b == 0xff {
                return interest_loop(gm);
            }
        } else {
            if op != -2 { // not 0xfe
                // LAB_1000_cd5a
                let v = gm.g16(PENDING_ARGC) & 0xff00;
                gm.set_g16(PENDING_ARGC, v);
                gm.set_g8(PENDING_OP, 0);
                return 0;
            }
            let n = gm.g16(PENDING_ARGC) & 0xff;
            let v = gm.g16(PENDING_ARGC).wrapping_add(1);
            gm.set_g16(PENDING_ARGC, v);
            gm.set_g8(OP_ARGS.wrapping_add(n), b);
            if gm.gi8(PENDING_ARGC) != 2 {
                return 0;
            }
            gm.set_g8(PENDING_OP, 0);
            let v = gm.g16(PENDING_ARGC) & 0xff00;
            gm.set_g16(PENDING_ARGC, v);
            let label = gm.g8(OP_ARGS.wrapping_add(1));
            if gm.g8(g::KARMA) < gm.g8(OP_ARGS) {
                gm.set_g8(PENDING_OP, 0);
                return 0;
            }
            gm.set_g8(LABEL, label);
            return ask_question_at_label(gm);
        }
        gm.set_g8(LABEL, b);
        return ask_question_at_label(gm);
    } else {
        give_item(gm, b & 0x7f);
    }
    // LAB_1000_cd5a
    let v = gm.g16(PENDING_ARGC) & 0xff00;
    gm.set_g16(PENDING_ARGC, v);
    gm.set_g8(PENDING_OP, 0);
    0
}

/// orig 1000:cdf8 (184 bytes). Opcode 0x88: "What is thy name?" / "You respond: ". An empty
/// answer gets "If you say so..."; otherwise the first four letters of each party member's name
/// are searched in the response (str_find_ci at a word start): a match marks the NPC as met and
/// prints "A pleasure!", none prints "If you say so...". Clears the pending opcode state.
pub fn ask_avatar_name(gm: &mut Game) {
    emit_quote(gm);
    flush_pending_text(gm);
    text::print_string(gm, 0x9468); // "What is thy name?\""
    text::print_string(gm, 0x947c); // "\nYou respond: "
    read_response(gm);
    gm.set_g8(PENDING_ARGC, 0);
    gm.set_g8(PENDING_OP, 0);
    let s: u16;
    if gm.g8(INPUT) == 0 {
        s = 0x948c; // "\"If you say so...\"\n"
    } else {
        if gm.g8(g::PARTY_COUNT) != 0 {
            let mut off: u16 = 0;
            let mut i: u16 = 0;
            loop {
                let rec = g::PARTY.wrapping_add(off);
                let found = gm.with_buf(6, |gm, buf| {
                    for k in 0..4u16 {
                        let v = gm.g8(rec.wrapping_add(k));
                        gm.set_g8(buf.wrapping_add(k), v);
                    }
                    gm.set_g8(buf.wrapping_add(4), 0);
                    let m = rest::str_find_ci(gm, INPUT as i16, buf);
                    matched_at_word(gm, m)
                });
                if found {
                    set_npc_met(gm, gm.gi16(TALK_NPC) as u16);
                    text::print_string(gm, 0x94a0); // "\"A pleasure!\"\n"
                    return;
                }
                off = off.wrapping_add(0x20);
                i += 1;
                if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
            }
        }
        s = 0x94b0; // "\"If you say so...\"\n"
    }
    text::print_string(gm, s);
}

/// orig 1000:ceb2 (463 bytes). The dialogue byte interpreter; returns 1 when the conversation
/// ends, else 0. A pending multi-byte opcode (0x4aee) routes the byte to
/// `interpret_opcode_arg`. A second consecutive quote (0xa2) is dropped. Control bytes: 0x89
/// karma +1, 0x8a karma -1, 0x8b raise the alarm (TOWN), 0x81 Avatar's name, 0x82 end (1),
/// 0x83 28-tick pause, 0x84 `join_party`, 0x85/0x86/0x8c/0xfe arm a multi-byte opcode, 0x87 run
/// the string after the next one (ends -> 1; else the script pointer is restored), 0x88
/// `ask_avatar_name`, 0x8e toggle the rune font mask, 0x8f wait for a command key,
/// 0x91..0x9f `ask_question_at_label`, 0xff flush and `interest_loop`. Text: bytes < 0x81 emit a
/// space and the compressed word g_tlk_word_ptrs[byte] (chars | 0x80; an empty word emits the
/// raw byte) and set the "needs space" flag; other bytes are emitted (0x8d -> 0x8a newline,
/// masked with the font mask) after a space when the flag is set.
pub fn interpret_byte(gm: &mut Game, param_1: u8) -> u16 {
    let b = param_1;
    if gm.g8(PENDING_OP) != 0 {
        return interpret_opcode_arg(gm, b);
    }
    gm.set_g8(PENDING_ARGC, 0);
    if b == 0xa2 && gm.g8(LAST_CHAR) == 0xa2 {
        gm.set_g8(PENDING_ARGC, 0);
        return 0;
    }
    if b == 0x89 {
        rest::add_u8_capped(gm, 99, 1, g::KARMA);
        return 0;
    }
    let mut text_byte = false;
    if b < 0x8a {
        match b {
            0x81 => emit_avatar_name(gm),
            0x82 => {
                gm.set_g8(PENDING_ARGC, 0);
                return 1;
            }
            0x83 => {
                let mut i: i16 = 0;
                loop {
                    rest::refresh_map_view(gm);
                    if input::read_key(gm) != 0 { break; }
                    util::wait_ticks(gm, 1);
                    i += 1;
                    if !(i < 0x1c) { break; }
                }
                input::flush_keyboard(gm);
            }
            0x84 => return join_party(gm),
            0x85 | 0x86 => gm.set_g8(PENDING_OP, b),
            0x87 => {
                let saved = gm.g16(SCRIPT_PTR);
                if skip_string_and_run(gm) != 0 {
                    return 1;
                }
                gm.set_g16(SCRIPT_PTR, saved);
            }
            0x88 => ask_avatar_name(gm),
            _ => text_byte = true,
        }
    } else {
        if b == 0x8e {
            gm.set_g8(PENDING_ARGC, 0);
            let v = gm.g8(FONT_MASK) ^ 0x80;
            gm.set_g8(FONT_MASK, v);
            return 0;
        }
        if b < 0x8f {
            if b == 0x8a {
                rest::sub_u8_floor(gm, 1, g::KARMA);
                return 0;
            }
            if b == 0x8b {
                ovl::town::raise_alarm(gm);
                return 0;
            }
            if b == 0x8c {
                gm.set_g8(PENDING_OP, b);
            } else {
                text_byte = true;
            }
        } else if b < 0xa0 {
            if 0x90 < b {
                gm.set_g8(LABEL, b);
                return ask_question_at_label(gm);
            }
            if b == 0x8f {
                input::read_command_key(gm);
                return 0;
            }
            text_byte = true;
        } else {
            if b == 0xfe {
                gm.set_g8(PENDING_OP, b);
            } else if b == 0xff {
                flush_pending_text(gm);
                return interest_loop(gm);
            } else {
                text_byte = true;
            }
        }
    }
    if text_byte {
        // switchD_1000_cf01_default
        if b < 0x81 {
            queue_char(gm, 0xa0);
            let word = gm.g16((b as u16).wrapping_mul(2).wrapping_add(g::TLK_WORD_PTRS));
            let mut p = word;
            while gm.g8(p) != 0 {
                let c = gm.g8(p) | 0x80;
                queue_char(gm, c);
                p = p.wrapping_add(1);
            }
            if gm.g8(word) == 0 {
                queue_char(gm, b);
            } else {
                gm.set_g8(NEED_SPACE, 1);
            }
        } else {
            let mut c = b | 0x80;
            if c == 0x8d {
                c = 0x8a;
            }
            c &= gm.g8(FONT_MASK);
            gm.set_g8(LAST_CHAR, c);
            if gm.g8(NEED_SPACE) != 0 {
                queue_char(gm, 0xa0);
            }
            queue_char(gm, c);
            gm.set_g8(NEED_SPACE, 0);
        }
    }
    0
}

/// orig 1000:d09c (98 bytes). Opens a conversation: "You see " + section 1 (description); if
/// that ended it returns 1. Then two newlines; an NPC that knows the Avatar emits a quote and
/// section 2 (greeting); otherwise, after reseeding the random generator, with 50% chance
/// "I am called " + section 0 (name), else nothing. A section that ends the conversation
/// returns 1; otherwise a quote and two newlines follow and 0 is returned.
pub fn greet_npc(gm: &mut Game) -> u16 {
    text::print_string(gm, 0x94c4); // "You see "
    if run_section(gm, 1) != 0 {
        return 1;
    }
    emit_newline(gm);
    emit_newline(gm);
    let section: u8;
    if !is_npc_met(gm, gm.gi16(TALK_NPC) as u16) {
        let h = util::get_time_hash(gm);
        util::set_random_seed(gm, h);
        if util::rand_range(gm, 1, 0) == 0 {
            return 0;
        }
        text::print_string(gm, 0x94ce); // "\"I am called "
        section = 0;
    } else {
        emit_quote(gm);
        section = 2;
    }
    if run_section(gm, section) != 0 {
        return 1;
    }
    emit_quote(gm);
    emit_newline(gm);
    emit_newline(gm);
    0
}

/// orig 1000:d100 (250 bytes). After a conversation while Shadowlord 0 is present in the
/// settlement (g_save_unk_3b2 == 0): "Something was stolen!" with a tone sweep and a reseeded
/// generator. Without keys, gems or torches the highest non-empty inventory slot 47..0, then
/// potions 7..0, then scrolls 7..0 loses one unit; if all are empty 1..15 gold is taken and
/// the status window redrawn. Otherwise a random one of keys / gems / torches that is non-zero
/// loses one unit.
pub fn shadowlord_steals_item(gm: &mut Game) {
    if gm.gi8(g::SAVE_UNK_3B2) == 0 {
        text::print_string(gm, 0x94dc); // "Something was stolen!\n"
        rest::play_tone_sweep(gm, 0x32, 1, 2000, 800);
        let h = util::get_time_hash(gm);
        util::set_random_seed(gm, h);
        if gm.gi8(g::KEYS) == 0 && gm.gi8(g::GEMS) == 0 && gm.gi8(g::TORCHES) == 0 {
            let mut i: i16 = 0x2f;
            loop {
                if gm.gi8((i as u16).wrapping_add(0x57c0)) != 0 {
                    rest::sub_u8_floor(gm, 1, (i as u16).wrapping_add(0x57c0));
                    return;
                }
                i -= 1;
                if !(-1 < i) { break; }
            }
            let mut i: i16 = 7;
            loop {
                if gm.gi8((i as u16).wrapping_add(0x5828)) != 0 { // g_potions
                    rest::sub_u8_floor(gm, 1, (i as u16).wrapping_add(0x5828));
                    return;
                }
                i -= 1;
                if !(-1 < i) { break; }
            }
            let mut i: i16 = 7;
            loop {
                if gm.gi8((i as u16).wrapping_add(0x5820)) != 0 { // g_scrolls
                    rest::sub_u8_floor(gm, 1, (i as u16).wrapping_add(0x5820));
                    return;
                }
                i -= 1;
                if !(-1 < i) { break; }
            }
            let amount = util::rand_range(gm, 0xf, 1);
            rest::sub_i16_floor(gm, amount, g::GOLD);
            rest::draw_status_window(gm);
        } else {
            let target: u16;
            loop {
                let r = util::rand_range(gm, 2, 0);
                if r == 0 {
                    if gm.gi8(g::KEYS) != 0 {
                        target = g::KEYS;
                        break;
                    }
                    continue;
                }
                if r == 1 {
                    if gm.gi8(g::GEMS) != 0 {
                        target = g::GEMS;
                        break;
                    }
                    continue;
                }
                if r != 2 {
                    return;
                }
                if gm.gi8(g::TORCHES) != 0 {
                    target = g::TORCHES;
                    break;
                }
            }
            rest::sub_u8_floor(gm, 1, target);
        }
    }
}

/// orig 1000:d1fe (145 bytes). Runs the .TLK conversation `talk_idx` of the current settlement
/// group ((g_location-1)/8 selects the file name at 0x4aa0): the 0x200-byte header is read
/// into g_ool_buffer (count at 0xb21e, {npc, offset} pairs from 0xb220), the matching entry's
/// 1 KB is loaded over it, then `greet_npc`, the "Your interest?" loop and, if neither ended
/// the conversation, `say_bye`; finally `shadowlord_steals_item`.
pub fn run_conversation(gm: &mut Game, param_1: i16) {
    let talk_idx = param_1;
    let grp = (gm.g8(g::LOCATION) as u16).wrapping_sub(1) >> 3;
    let name = gm.g16(grp.wrapping_mul(2).wrapping_add(0x4aa0)); // g_tlk_name_ptrs
    files::load_file_region(gm, 0, 0x200, g::OOL_BUFFER, name);
    let mut p: u16 = 0xb220;
    let mut i: i16 = 1;
    while gm.gi16(p) != talk_idx && i <= gm.gi16(0xb21e) {
        p = p.wrapping_add(4);
        i += 1;
    }
    files::load_file_region(gm, gm.g16(p.wrapping_add(2)), 0x400, g::OOL_BUFFER, name);
    if greet_npc(gm) == 0 && interest_loop(gm) == 0 {
        say_bye(gm);
    }
    shadowlord_steals_item(gm);
}
