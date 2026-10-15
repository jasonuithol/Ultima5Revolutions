//! ENDGAME.OVL (translated)
//!
//! The end-game cut-scene, vector 1778:04ca `endgame_sequence` (a8d8), run by SJOG `end_combat`
//! and DUNGEON when combat exits with g_combat_exit 'M' (Lord British freed in the Underworld
//! dungeon room). The 11x11 scene map comes from MISCMAPS.DAT (offset 0x210) and the dialogue
//! from ENDMSG.DAT (loaded at 0xb21e); Lord British (tile 0x7c, monster slot 0x1f) walks to the
//! party, dead members are revived, and he asks whether the party will return with him. With
//! the sandalwood box the box (tile 0xe, slot 6) is placed, the moongate (g_moongate_height)
//! rises, everyone leaves through it and `end_story_screens` (a290) shows the six END.DAT story
//! screens (END1.16/END2.16 pictures, TEXT.16 titles, PROPORT.PCS text through FONT
//! `draw_proportional_text`, ENDSC.16 final picture), followed by `print_quest_report` (a5b6):
//! the "Be it known that on the ... Day of the ... Month of the Year ..." certificate with
//! number words from the DATA.OVL tables 0x3e0a..0x3e5a, then the game hangs. Without the box
//! Lord British says "I see..." and the figures wander forever. a366/a454/a476 are not
//! functions but JMP targets inside a290 (split by Ghidra because other overlays have vector
//! targets at a366).
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// Monster-table slot offset (8 bytes per slot at 0x5c5a).
#[inline] fn slot8(i: u16) -> u16 { i.wrapping_mul(8) }

/// DGROUP 0xbcb2: 40-byte line accumulator of the quest report (`append_wrapped_word`).
const REPORT_LINE_BUF: u16 = 0xbcb2;
/// DGROUP 0xbcda: current length of the accumulator (u8).
const REPORT_LINE_LEN: u16 = 0xbcda;
/// DGROUP 0x260c: pointer to "TEXT.16"; 0x261a: pointers to END1.16, END2.16, ...; 0x2622: "ENDSC.16".
const TEXT16_NAME_PTR: u16 = 0x260c;
const END_PIC_NAME_PTRS: u16 = 0x261a;
const ENDSC16_NAME_PTR: u16 = 0x2622;
/// DGROUP 0x3e0a: number words ""(0), One .. Twenty (index 0..20), 0x3e34: Thirty .. Ninety
/// (index n/10 at 0x3e2e), 0x3e42: ordinals First .. Twelfth (0x3e40[1..12]).
const NUMBER_WORDS: u16 = 0x3e0a;
const TENS_WORDS: u16 = 0x3e2e;
const ORDINAL_WORDS: u16 = 0x3e40;
/// DGROUP 0x3e5a / 0x3e60: target x / y of each party member in the end scene.
const END_MEMBER_X: u16 = 0x3e5a;
const END_MEMBER_Y: u16 = 0x3e60;
/// DGROUP 0x1ade: party figure tile per class index in "AMBFDTPRS".
const CLASS_TILES: u16 = 0x1ade;
/// FONT layout block (see font.rs): left x per band, right x per band, band-1 y range, cursor.
const LAYOUT_LEFT0: u16 = 0x5146;
const LAYOUT_LEFT1: u16 = 0x5148;
const LAYOUT_RIGHT0: u16 = 0x514c;
const LAYOUT_RIGHT1: u16 = 0x514e;
const LAYOUT_BAND_Y0: u16 = 0x5150;
const LAYOUT_BAND_Y1: u16 = 0x5152;
const LAYOUT_CUR_X: u16 = 0x5156;
const LAYOUT_CUR_Y: u16 = 0x5158;

/// orig 1000:a290 (213 bytes, body continues through a366, a454 and a476). The six end-story
/// screens. `select_disk(2)`; loads PROPORT.PCS (g_raw_file_names[0]) with `load_file_to_mem`,
/// TEXT.16 (0x260c) and the first story picture file (0x261a[0] = END1.16) with
/// `load_image_file`, retrying each until it succeeds; dissolves the screen (`rect_effect`)
/// and selects text window 0. For screens 0..5: if g_end_story_number[s] differs from the
/// loaded one the picture file is swapped (END1.16 / END2.16); draw page 1, g_display_mode =
/// 2, put_char(0xff) (clear); hook g_end_hook[s] == 1 draws the TEXT.16 titles (screen 0:
/// images 0 at (0,0xd8) and 4 at (0x1c,0x98); screen 3: images 5 at (0,0xe0) and 0 at
/// (0,0xb0)); then the story picture g_end_image_index[s] at (g_end_pic_y, g_end_pic_x); the
/// FONT layout block is filled from the g_end_* tables (left x per band 0x3da6[2s..],
/// right x 0x3db2[2s..], band y range 0x3dd6/0x3ddc, first-line x 0x3de2, text y 0x3de8);
/// END.DAT text at g_end_text_offsets[s] (2000 bytes to 0xb21e) is drawn with FONT
/// `draw_proportional_text`; every screen but the first waits for a key; page 1 is copied to
/// page 0. Finally the pictures and font are freed and (tail a476) after a key press page 1
/// is cleared, ENDSC.16 is drawn at (0,0x28), copied to page 0, draw page 0 and
/// g_display_mode = 0.
pub fn end_story_screens(gm: &mut Game) {
    files::select_disk(gm, 2);
    let font: u16;
    loop {
        let name = gm.g16(g::RAW_FILE_NAMES); // "PROPORT.PCS"
        let f = gfx::load_file_to_mem(gm, name);
        if f != 0 { font = f; break; }
    }
    let text16: u16;
    loop {
        let name = gm.g16(TEXT16_NAME_PTR);
        let s = gfx::load_image_file(gm, name);
        if s != 0 { text16 = s as u16; break; }
    }
    let mut pic: u16;
    loop {
        let name = gm.g16(END_PIC_NAME_PTRS);
        let s = gfx::load_image_file(gm, name);
        if s != 0 { pic = s as u16; break; }
    }
    gfx::rect_effect(gm, 199, 0x13f, 0, 0);
    text::select_text_window(gm, 0);
    let mut story: u16 = 0x31;
    let mut screen: u16 = 0;
    loop {
        if gm.g8(g::END_STORY_NUMBER.wrapping_add(screen)) as u16 != story {
            gfx::free_far_block(gm, pic);
            story = gm.g8(g::END_STORY_NUMBER.wrapping_add(screen)) as u16;
            loop {
                let name = gm.g16(END_PIC_NAME_PTRS.wrapping_add(story.wrapping_mul(2)));
                let s = gfx::load_image_file(gm, name);
                if s != 0 { pic = s as u16; break; }
            }
        }
        gfx::set_draw_page(gm, 1);
        gm.set_g8(g::DISPLAY_MODE, 2);
        text::put_char(gm, 0xff);
        if gm.g8(g::END_HOOK.wrapping_add(screen)) == 1 {
            if screen == 0 {
                // a366 arm
                gfx::draw_image(gm, 0, 0, 0xd8, 0, text16);
                gfx::draw_image(gm, 0, 0x1c, 0x98, 4, text16);
            } else if screen == 3 {
                // a454 arm
                gfx::draw_image(gm, 0, 0, 0xe0, 5, text16);
                gfx::draw_image(gm, 0, 0, 0xb0, 0, text16);
            }
        }
        // LAB_1000_a38c
        let py = gm.g8(g::END_PIC_Y.wrapping_add(screen)) as i16;
        let px = gm.g8(g::END_PIC_X.wrapping_add(screen)) as i16;
        let idx = gm.g8(g::END_IMAGE_INDEX.wrapping_add(screen)) as i16;
        gfx::draw_image(gm, 0, py, px, idx, pic);
        let s2 = screen.wrapping_mul(2);
        let v = gm.g8(g::END_LEFT_X.wrapping_add(s2)) as u16; gm.set_g16(LAYOUT_LEFT0, v);
        let v = gm.g8(g::END_LEFT_X.wrapping_add(s2).wrapping_add(1)) as u16; gm.set_g16(LAYOUT_LEFT1, v);
        let v = gm.g16(g::END_RIGHT_X.wrapping_add(s2.wrapping_mul(2))); gm.set_g16(LAYOUT_RIGHT0, v);
        let v = gm.g16(g::END_RIGHT_X.wrapping_add(s2.wrapping_mul(2)).wrapping_add(2)); gm.set_g16(LAYOUT_RIGHT1, v);
        let v = gm.g8(g::END_PARA1_Y.wrapping_add(screen)) as u16; gm.set_g16(LAYOUT_BAND_Y0, v);
        let v = gm.g8(g::END_PARA2_Y.wrapping_add(screen)) as u16; gm.set_g16(LAYOUT_BAND_Y1, v);
        let v = gm.g8(g::END_FIRST_LINE_X.wrapping_add(screen)) as u16; gm.set_g16(LAYOUT_CUR_X, v);
        let v = gm.g8(g::END_TEXT_Y.wrapping_add(screen)) as u16; gm.set_g16(LAYOUT_CUR_Y, v);
        let off = gm.g16(g::END_TEXT_OFFSETS.wrapping_add(s2));
        files::load_file_region(gm, off, 2000, 0xb21e, 0x81fe); // "END.DAT"
        ovl::font::draw_proportional_text(gm, 0xb21e_u16 as i16, font);
        if screen != 0 {
            loop {
                if input::read_key(gm) != 0 { break; }
            }
        }
        gfx::copy_page(gm, 0, 1);
        screen = screen.wrapping_add(1);
        if !(screen < 6) { break; }
    }
    gfx::free_far_block(gm, pic);
    gfx::free_far_block(gm, text16);
    gfx::free_far_block2(gm, font);
    // a476 tail
    loop {
        if input::read_key(gm) != 0 { break; }
    }
    gfx::set_draw_page(gm, 1);
    text::put_char(gm, 0xff);
    let endsc: u16;
    loop {
        let name = gm.g16(ENDSC16_NAME_PTR);
        let s = gfx::load_image_file(gm, name);
        if s != 0 { endsc = s as u16; break; }
    }
    gfx::draw_image(gm, 0, 0, 0x28, 0, endsc);
    gfx::free_far_block(gm, endsc);
    gfx::copy_page(gm, 0, 1);
    gfx::set_draw_page(gm, 0);
    gm.set_g8(g::DISPLAY_MODE, 0);
}

/// orig 1000:a366 (238 bytes as listed). Not a function: the screen-0 title arm of
/// `end_story_screens` (JZ target at a359; other overlays have vector 1778:0326 here). The
/// decompilation re-lists the rest of the loop with unassigned BP locals. No effect.
pub fn end_story_screen0_fragment(gm: &mut Game) {}

/// orig 1000:a454 (33 bytes). Not a function: the screen-3 title arm of `end_story_screens`
/// (JMP target at a360, jumps back to a385). No effect.
pub fn end_story_screen3_fragment(gm: &mut Game) {}

/// orig 1000:a476 (83 bytes). Not a function: the tail of `end_story_screens` (JMP target at
/// a452): key wait, ENDSC.16 on a cleared page 1, copy to page 0, text mode. No effect here.
pub fn end_story_finale_fragment(gm: &mut Game) {}

/// orig 1000:a4ca (82 bytes). Appends the NUL-terminated string at `s` to the 40-byte report
/// line buffer 0xbcb2 (length byte 0xbcda, at most 39 characters are kept), NUL-terminates
/// it, and when the last stored character is '\n' prints the buffer with `print_string` and
/// resets the length to 0. The certificate text is built word by word this way so that
/// `print_string`'s own wrapping sees whole lines.
pub fn append_wrapped_word(gm: &mut Game, s: u16) {
    let mut p = s;
    loop {
        let c = gm.g8(p);
        let len = gm.g8(REPORT_LINE_LEN);
        if c == 0 || !(len < 0x27) { break; }
        gm.set_g8(REPORT_LINE_BUF.wrapping_add(len as u16), c);
        p = p.wrapping_add(1);
        gm.set_g8(REPORT_LINE_LEN, len.wrapping_add(1));
    }
    let len = gm.g8(REPORT_LINE_LEN) as u16;
    gm.set_g8(REPORT_LINE_BUF.wrapping_add(len), 0);
    if gm.g8(len.wrapping_add(0xbcb1)) == b'\n' {
        text::print_string(gm, REPORT_LINE_BUF);
        gm.set_g8(REPORT_LINE_LEN, 0);
    }
}

/// orig 1000:a51c (73 bytes). Appends the English number word for `n` (0..99): above 20 the
/// tens word 0x3e2e[n/10] ("Twenty".."Ninety") is appended, and if n%10 is 0 that is all
/// (returns n/10); otherwise "-" (0x82c6) and then the units word; n <= 20 appends
/// 0x3e0a[n] ("", "One" .. "Twenty"). Return value is unused by callers.
pub fn append_number_word(gm: &mut Game, n: i16) -> i16 {
    let mut n = n;
    if 0x14 < n {
        let w = gm.g16(TENS_WORDS.wrapping_add(((n / 10) as u16).wrapping_mul(2)));
        append_wrapped_word(gm, w);
        let tens = n / 10;
        n %= 10;
        if n == 0 {
            return tens;
        }
        append_wrapped_word(gm, 0x82c6); // "-"
    }
    let w = gm.g16(NUMBER_WORDS.wrapping_add((n as u16).wrapping_mul(2)));
    append_wrapped_word(gm, w);
    0 // a4ca returns nothing; Ghidra's iVar1 is undefined here
}

/// orig 1000:a566 (79 bytes). Appends the ordinal word for `n`: n < 13 -> 0x3e40[n]
/// ("First" .. "Twelfth"); n < 20 -> number word + "th" (0x831e); n == 20 -> "Twent" +
/// "ieth"; else "Twent" + "y-" (0x832e) + 0x3e18[n] (= ordinal n-20, "First" ..).
pub fn append_ordinal_word(gm: &mut Game, n: i16) {
    let w: u16;
    if n < 0xd {
        w = gm.g16(ORDINAL_WORDS.wrapping_add((n as u16).wrapping_mul(2)));
    } else if n < 0x14 {
        append_number_word(gm, n);
        w = 0x831e; // "th"
    } else {
        append_wrapped_word(gm, 0x8322); // "Twent"
        if n == 0x14 {
            w = 0x8328; // "ieth"
        } else {
            append_wrapped_word(gm, 0x832e); // "y-"
            w = gm.g16(0x3e18u16.wrapping_add((n as u16).wrapping_mul(2)));
        }
    }
    append_wrapped_word(gm, w);
}

/// MS C `itoa(value, buf, 10)` for the quest report: writes the decimal digits (with '-' for
/// negatives) NUL-terminated at DGROUP `buf` and returns `buf`.
fn itoa_decimal(gm: &mut Game, value: i16, buf: u16) -> u16 {
    let s = format!("{}", value);
    let mut p = buf;
    for &b in s.as_bytes() {
        gm.set_g8(p, b);
        p = p.wrapping_add(1);
    }
    gm.set_g8(p, 0);
    buf
}

/// orig 1000:a5b6 (472 bytes). The certificate after the story screens. Cursor to (1,0),
/// put_char(0xfd) and (0xfc), "Be it known that on" (0x8332); then through the word
/// accumulator: "the " + ordinal(g_day) + " Day of\n" + "the " + ordinal(g_month) +
/// " Month\n"; "of the Year\n"; number(g_year/100) + " Hundred\n" + number(g_year%100) +
/// "\n\n" + the Avatar's name (0x55a8) + " the Avatar\n\n"; then the fixed lines 0x8390,
/// 0x83a0, 0x83b2, 0x83ca, 0x83de, in font 1 0x83ee/0x8404 ("THE QUEST OF THE AVATAR IS
/// FOREVER"), font 0, put_char(0xfd), "Report now, thy Quest compleat in" (0x8414) followed
/// by the elapsed time since the start date (year 139, month 4, day 5: days = day-5 (or
/// day+23 and one month less), months = month-4 (or +13 and one year less), years =
/// year-139/140) as "N year(s), N month(s), N day(s)" (itoa) and "to Lord British at Origin
/// Systems!" (0x845c). Then loops forever (the original never returns).
pub fn print_quest_report(gm: &mut Game) {
    text::set_cursor_pos(gm, 1, 0);
    text::put_char(gm, 0xfd);
    text::put_char(gm, 0xfc);
    text::print_string(gm, 0x8332); // "Be it known that on\n"
    gm.set_g8(REPORT_LINE_LEN, 0);
    append_wrapped_word(gm, 0x8348); // "the "
    let day = gm.g8(g::DAY) as i16;
    append_ordinal_word(gm, day);
    append_wrapped_word(gm, 0x834e); // " Day of\n"
    append_wrapped_word(gm, 0x8358); // "the "
    let month = gm.g8(g::MONTH) as i16;
    append_ordinal_word(gm, month);
    append_wrapped_word(gm, 0x835e); // " Month\n"
    text::print_string(gm, 0x8366); // "of the Year\n"
    let year = gm.gi16(g::YEAR);
    append_number_word(gm, year / 100);
    append_wrapped_word(gm, 0x8374); // " Hundred\n"
    append_number_word(gm, year % 100);
    append_wrapped_word(gm, 0x837e); // "\n\n"
    append_wrapped_word(gm, 0x55a8); // the Avatar's name
    append_wrapped_word(gm, 0x8382); // " the Avatar\n\n"
    text::print_string(gm, 0x8390); // "saved the life\n"
    text::print_string(gm, 0x83a0); // "of our sovereign\n"
    text::print_string(gm, 0x83b2); // "Lord British, thereby\n"
    text::print_string(gm, 0x83ca); // "saving our people\n"
    text::print_string(gm, 0x83de); // "and our land.\n\n"
    text::select_font(gm, 1);
    text::print_string(gm, 0x83ee); // "THE QUEST OF THE AVATAR\n"
    text::print_string(gm, 0x8404); // "IS FOREVER\n\n\n\n"
    text::select_font(gm, 0);
    text::put_char(gm, 0xfd);
    text::print_string(gm, 0x8414); // "Report now, thy Quest compleat in\n"
    let mut years: i16 = year.wrapping_sub(0x8b);
    let mut months: i16 = month - 4;
    let mut days: i16 = day - 5;
    if days < 0 {
        days = day + 0x17;
        months = month - 5;
    }
    if months < 0 {
        months += 0xd;
        years = year.wrapping_sub(0x8c);
    }
    gm.with_buf(8, |gm, buf| {
        if years != 0 {
            let s = itoa_decimal(gm, years, buf);
            append_wrapped_word(gm, s);
            append_wrapped_word(gm, 0x8438); // " year"
            if 1 < years {
                append_wrapped_word(gm, 0x843e); // "s"
            }
            if months != 0 || days != 0 {
                append_wrapped_word(gm, 0x8440); // ", "
            }
        }
        if months != 0 {
            let s = itoa_decimal(gm, months, buf);
            append_wrapped_word(gm, s);
            append_wrapped_word(gm, 0x8444); // " month"
            if 1 < months {
                append_wrapped_word(gm, 0x844c); // "s"
            }
            if days != 0 {
                append_wrapped_word(gm, 0x844e); // ", "
            }
        }
        if days != 0 {
            let s = itoa_decimal(gm, days, buf);
            append_wrapped_word(gm, s);
            append_wrapped_word(gm, 0x8452); // " day"
            if 1 < days {
                append_wrapped_word(gm, 0x8458); // "s"
            }
        }
    });
    append_wrapped_word(gm, 0x845a); // "\n"
    text::print_string(gm, 0x845c); // "to Lord British at Origin Systems!"
    // The original spins forever here; keep pumping the window so it can be closed.
    loop {
        util::wait_ticks(gm, 1);
    }
}

/// orig 1000:a78e (18 bytes). One walking step of the end scene: animate_wait(2), footstep
/// sound, animate_wait(3).
pub fn end_step_pause(gm: &mut Game) {
    util::animate_wait(gm, 2);
    rest::play_footstep_sound(gm);
    util::animate_wait(gm, 3);
}

/// orig 1000:a7a0 (142 bytes). Ghidra order (target_y, target_x, slot). Moves monster-table
/// slot `slot` one cell toward (target_x, target_y): returns 0 when the slot's tile is 0 or it
/// is already there; otherwise steps in y when |dx| < |dy| (toward the target), else in x,
/// plays `end_step_pause` and returns 1.
pub fn move_slot_toward(gm: &mut Game, target_y: u16, target_x: u16, slot: i16) -> u16 {
    let s = slot8(slot as u16);
    let tile = gm.g8(g::MONSTER_TABLE.wrapping_add(s));
    let mx = gm.g8(g::MONSTER_X.wrapping_add(s));
    let my = gm.g8(g::MONSTER_Y.wrapping_add(s));
    if tile == 0 || (mx as u16 == target_x && my as u16 == target_y) {
        return 0;
    }
    let dx = (mx as u16).wrapping_sub(target_x) as i16;
    let dy = (my as u16).wrapping_sub(target_y) as i16;
    let adx = (dx ^ (dx >> 15)).wrapping_sub(dx >> 15);
    let ady = (dy ^ (dy >> 15)).wrapping_sub(dy >> 15);
    if adx < ady {
        if target_y < my as u16 {
            gm.set_g8(g::MONSTER_Y.wrapping_add(s), my.wrapping_sub(1));
        } else {
            gm.set_g8(g::MONSTER_Y.wrapping_add(s), my.wrapping_add(1));
        }
    } else if target_x < mx as u16 {
        gm.set_g8(g::MONSTER_X.wrapping_add(s), mx.wrapping_sub(1));
    } else {
        gm.set_g8(g::MONSTER_X.wrapping_add(s), mx.wrapping_add(1));
    }
    end_step_pause(gm);
    1
}

/// orig 1000:a832 (161 bytes). Random wander of monster-table slot `slot` in the end scene:
/// if its tile is non-zero and rand_range(1,0) is 1, up to 8 attempts pick rand_range(3,0):
/// 0 x+1, 1 x-1, 2 y+1, 3 y-1; the first cell whose g_view_tiles entry (stride 0x20) is 0x44
/// ('D', the floor tile) becomes the new position. Always ends with animate_wait(1).
pub fn wander_slot(gm: &mut Game, slot: i16) {
    let s = slot8(slot as u16);
    if gm.g8(g::MONSTER_TABLE.wrapping_add(s)) != 0 && util::rand_range(gm, 1, 0) != 0 {
        let mut tries: i16 = 0;
        loop {
            let mut x = gm.g8(g::MONSTER_X.wrapping_add(s)) as u16;
            let mut y = gm.g8(g::MONSTER_Y.wrapping_add(s)) as u16;
            let d = util::rand_range(gm, 3, 0);
            if d == 0 {
                x = x.wrapping_add(1);
            } else if d == 1 {
                x = x.wrapping_sub(1);
            } else if d == 2 {
                y = y.wrapping_add(1);
            } else if d == 3 {
                y = y.wrapping_sub(1);
            }
            if gm.g8(g::VIEW_TILES.wrapping_add(y.wrapping_mul(0x20)).wrapping_add(x)) == 0x44 {
                gm.set_g8(g::MONSTER_X.wrapping_add(s), x as u8);
                gm.set_g8(g::MONSTER_Y.wrapping_add(s), y as u8);
                break;
            }
            tries += 1;
            if !(tries < 8) { break; }
        }
    }
    util::animate_wait(gm, 1);
}

/// orig 1000:a8d8 (1185 bytes). Vector 1778:04ca, the end-game scene (SJOG `end_combat` /
/// DUNGEON when g_combat_exit is 'M'). g_active_combatant = 0xff, status window redrawn,
/// driver tile-patch command 4 (`driver_tile_patch_special(1)`), the scene map from
/// MISCMAPS.DAT offset 0x210 (11 rows of 16) into g_view_objects and ENDMSG.DAT (1000 bytes)
/// into 0xb21e, rows copied into g_light_map (stride 32); every monster slot's tile and anim
/// tile cleared; Lord British in slot 0x1f (tile/anim 0x7c at x 5, y 8, +6 = 0); after
/// animate_wait(40) he walks to (5,3). Each party member: a dead one is announced
/// (name + " lives!"), revived ('G', hp = max hp) with a flash and sweep noise; its figure
/// (tile 0x1ade[class index in "AMBFDTPRS"]) is placed in slot m at (5,9) and walked to
/// (0x3e5a[m], 0x3e60[m]). Then ENDMSG text 0xb21e + name + '!"', key, 0xb23f and a Y/N
/// answer (Yes/No echoed); N asks again with 0xb267. With Y and the sandalwood box: LB walks
/// to (5,4) then (5,5), the box appears in slot 6 (tile 0xe at (5,4)), texts 0xb2c9, "He
/// says:", 0xb2f1, 0xb346, 0xb385, 0xb3e7, 0xb42f, 0xb469 each after a key, the box tile
/// becomes 8, a sweep noise, the box vanishes, g_light_map[4][5] = 0xdc and the moongate rises
/// (g_moongate_height 1..16), LB and then every member walk to (5,4) and vanish, the gate
/// sinks (15..0), tile 0x44 is drawn at (4,5), page 1 is cleared black (free_picture,
/// fill_rect), and `end_story_screens` + `print_quest_report` follow (never returns). Without
/// the box (or answering N): '"I see...', 0xb4f3, the party figure steps up once, slots 2, 0x1f
/// and 0 walk to (8,6), (4,1) and (8,4) and slots 1, 3, 4, 5 wander forever.
pub fn endgame_sequence(gm: &mut Game) {
    gm.set_g8(g::ACTIVE_COMBATANT, 0xff);
    rest::draw_status_window(gm);
    rest::driver_tile_patch_special(gm, 1);
    files::load_file_region(gm, 0x210, 0xb0, g::VIEW_OBJECTS, 0x8480); // "MISCMAPS.DAT"
    files::load_file_region(gm, 0, 1000, 0xb21e, 0x848e); // "ENDMSG.DAT"
    let mut src: u16 = 0;
    let mut dst: u16 = 0;
    loop {
        for k in 0..11u16 {
            let v = gm.g8(g::VIEW_OBJECTS.wrapping_add(src).wrapping_add(k));
            gm.set_g8(g::LIGHT_MAP.wrapping_add(dst).wrapping_add(k), v);
        }
        src = src.wrapping_add(0x10);
        dst = dst.wrapping_add(0x20);
        if !(dst < 0x160) { break; }
    }
    let mut s: u16 = 0;
    while g::PARTY_VEHICLE_TILE.wrapping_add(s) < 0x5d5b {
        gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(s), 0);
        gm.set_g8(g::MONSTER_TABLE.wrapping_add(s), 0);
        s = s.wrapping_add(8);
    }
    // Slot 0x1f (0x5d52): Lord British.
    gm.set_g8(0x5d53, 0x7c);
    gm.set_g8(0x5d52, 0x7c);
    gm.set_g8(0x5d54, 5);
    gm.set_g8(0x5d55, 8);
    gm.set_g8(0x5d58, 0);
    util::animate_wait(gm, 0x28);
    loop {
        if move_slot_toward(gm, 3, 5, 0x1f) == 0 { break; }
    }
    let mut m: u16 = 0;
    if gm.g8(g::PARTY_COUNT) != 0 {
        loop {
            let rec = m.wrapping_mul(0x20);
            let st = g::PARTY_STATUS.wrapping_add(rec);
            if gm.g8(st) == b'D' {
                text::put_char(gm, 10);
                text::print_string(gm, 0x55a8u16.wrapping_add(rec));
                text::print_string(gm, 0x849a); // " lives!\n"
                gm.set_g8(st, b'G');
                let maxhp = gm.g16(g::PARTY_MAX_HP.wrapping_add(rec));
                gm.set_g16(0x55b8u16.wrapping_add(rec), maxhp);
                let c = gm.gi16(g::UI_COLOR1);
                gfx::set_color(gm, c);
                gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
                // s_STARTSC_16 + 10 is the number 0x1388 (5000), not text.
                util::play_sweep_noise(gm, 1, 0x1388, 40000, 1, 0x2260);
                rest::draw_status_window(gm);
            }
            let class = gm.g8(0x55b2u16.wrapping_add(rec)) as u16;
            let ci = rest::str_index_of(gm, class, 0x84a4u16 as i16); // "AMBFDTPRS"
            let tile = gm.g8(CLASS_TILES.wrapping_add(ci as u16));
            let ms = slot8(m);
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(ms).wrapping_add(1), tile);
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(ms), tile);
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(ms).wrapping_add(2), 5);
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(ms).wrapping_add(3), 9);
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(ms).wrapping_add(6), 0);
            util::animate_wait(gm, 1);
            let ty = gm.g8(END_MEMBER_Y.wrapping_add(m)) as u16;
            let tx = gm.g8(END_MEMBER_X.wrapping_add(m)) as u16;
            if move_slot_toward(gm, ty, tx, m as i16) != 0 {
                loop {
                    if move_slot_toward(gm, ty, tx, m as i16) == 0 { break; }
                }
            }
            m = m.wrapping_add(1);
            if !(m < gm.g8(g::PARTY_COUNT) as u16) { break; }
        }
    }
    util::animate_wait(gm, 0x28);
    text::print_string(gm, 0xb21e); // ENDMSG: greeting
    text::print_string(gm, 0x55a8); // the Avatar's name
    text::print_string(gm, 0x84ae); // "!\"\n\n"
    input::read_command_key(gm);
    text::print_string(gm, 0xb23f); // ENDMSG: the question
    let mut answer: i16;
    loop {
        answer = input::read_command_key(gm);
        if answer == 0x4e {
            text::print_string(gm, 0x84ba); // "No\n\n"
        } else if answer == 0x59 {
            text::print_string(gm, 0x84b4); // "Yes\n\n"
        }
        if answer == 0x59 || answer == 0x4e { break; }
    }
    if answer == 0x4e {
        text::print_string(gm, 0xb267); // ENDMSG: asks again
        loop {
            answer = input::read_command_key(gm);
            if answer == 0x4e {
                text::print_string(gm, 0x84c6); // "No\n\n"
            } else if answer == 0x59 {
                text::print_string(gm, 0x84c0); // "Yes\n\n"
            }
            if answer == 0x59 || answer == 0x4e { break; }
        }
    }
    if answer == 0x59 && gm.g8(g::SANDALWOOD_BOX) != 0 {
        util::animate_wait(gm, 8);
        loop {
            if move_slot_toward(gm, 4, 5, 0) == 0 { break; }
        }
        loop {
            if move_slot_toward(gm, 5, 5, 0) == 0 { break; }
        }
        util::animate_wait(gm, 4);
        // Slot 6 (0x5c8a): the sandalwood box.
        gm.set_g8(0x5c8b, 0xe);
        gm.set_g8(0x5c8a, 0xe);
        gm.set_g8(0x5c8c, 5);
        gm.set_g8(0x5c8d, 4);
        gm.set_g8(0x5c90, 0);
        text::print_string(gm, 0xb2c9);
        util::animate_wait(gm, 0x28);
        text::print_string(gm, 0x84cc); // "\n\nHe says:\n\n"
        input::read_command_key(gm);
        text::print_string(gm, 0xb2f1);
        input::read_command_key(gm);
        text::print_string(gm, 0xb346);
        input::read_command_key(gm);
        text::print_string(gm, 0xb385);
        input::read_command_key(gm);
        text::print_string(gm, 0xb3e7);
        input::read_command_key(gm);
        text::print_string(gm, 0xb42f);
        input::read_command_key(gm);
        text::print_string(gm, 0xb469);
        gm.set_g8(0x5c8a, 8);
        gm.set_g8(0x5c8b, 8);
        input::read_command_key(gm);
        util::play_sweep_noise(gm, 1, 0x2710, 50000, 1, 0x1450); // s_Missed_+1 == 10000
        gm.set_g8(0x5c8a, 0);
        gm.set_g8(0x5c8b, 0);
        gm.set_g8(0xad99, 0xdc); // g_light_map[4][5]: moongate base tile
        gm.set_g8(g::MOONGATE_HEIGHT, 1);
        loop {
            util::animate_wait(gm, 1);
            let h = gm.g8(g::MOONGATE_HEIGHT);
            gm.set_g8(g::MOONGATE_HEIGHT, h.wrapping_add(1));
            if !(gm.g8(g::MOONGATE_HEIGHT) < 0x10) { break; }
        }
        util::animate_wait(gm, 4);
        loop {
            if move_slot_toward(gm, 4, 5, 0x1f) == 0 { break; }
        }
        gm.set_g8(0x5d53, 0);
        gm.set_g8(0x5d52, 0);
        util::animate_wait(gm, 1);
        if gm.g8(g::PARTY_COUNT) != 0 {
            let mut i: u16 = 0;
            loop {
                loop {
                    if move_slot_toward(gm, 4, 5, i as i16) == 0 { break; }
                }
                gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(slot8(i)), 0);
                gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(i)), 0);
                util::animate_wait(gm, 1);
                i = i.wrapping_add(1);
                if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
            }
        }
        gm.set_g8(g::MOONGATE_HEIGHT, 0xf);
        loop {
            util::animate_wait(gm, 1);
            let h = gm.g8(g::MOONGATE_HEIGHT);
            gm.set_g8(g::MOONGATE_HEIGHT, h.wrapping_sub(1));
            if gm.g8(g::MOONGATE_HEIGHT) == 0 { break; }
        }
        gfx::draw_tile(gm, 4, 5, 0x44);
        gfx::set_draw_page(gm, 1);
        gfx::set_color(gm, 0);
        gfx::free_picture(gm);
        gfx::fill_rect(gm, 199, 0x13f, 0, 0);
        gfx::set_draw_page(gm, 0);
        end_story_screens(gm);
        print_quest_report(gm);
        return;
    }
    text::print_string(gm, 0x84da); // "\"I see...\n"
    util::animate_wait(gm, 0x28);
    text::print_string(gm, 0xb4f3);
    let y = gm.g8(g::MONSTER_Y);
    gm.set_g8(g::MONSTER_Y, y.wrapping_sub(1));
    end_step_pause(gm);
    loop {
        let a = move_slot_toward(gm, 6, 8, 2);
        let b = move_slot_toward(gm, 1, 4, 0x1f);
        let c = move_slot_toward(gm, 4, 8, 0);
        if a.wrapping_add(b).wrapping_add(c) == 0 { break; }
    }
    loop {
        wander_slot(gm, 1);
        wander_slot(gm, 3);
        wander_slot(gm, 4);
        wander_slot(gm, 5);
    }
}
