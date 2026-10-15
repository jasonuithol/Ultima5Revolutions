//! CAST2.OVL (translated)
//!
//! Level-3 overlay holding the second half of spell casting plus two unrelated vector
//! targets. Spell helpers (called from CAST through vectors 1778:0776..09b6): the per-circle
//! casting flash/sound `casting_flash` (e1e0), the "On who:" member picker (e27e), the
//! syllable spell-name reader `read_spell_name` (e2be, DATA.OVL rune table 0x1bfc and spell
//! code table g_spell_code_ptrs), the "Direction-" prompt (e4e6), Mani healing (e5a2), Rel Hur
//! wind change (e5ea), Wis An Ylem x-ray view (e64c), Kal Xen Corp daemon summoning (e6a2),
//! In Mani Corp resurrection (e7c0, also used by BLCKTHRN), In Wis position display (e8cc),
//! In Ex Por door unlock (e948, also the skull key), An Grav field dispel (e99c), light-spell
//! and timed-effect setters (eaca/ead8). Vector 1778:02ea `visit_shrine` (f056) is the shrine
//! cut-scene for the eight virtue shrines and the Shrine of the Codex (MISCMAPS.DAT map,
//! MISCMSG.DAT text at file 0x3ab), entered from MAINOUT/TOWN. Vector 1778:0a2e
//! `save_game_prompt` (f2de) is the Quit & Save command (ROOT dispatch_command 'Q').
//! e63e/e648 and ec1a are not functions: they are the tails of e5ea and eb46 that Ghidra split
//! because ZSTATS.OVL has vector targets at those addresses.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// Party record offset (32 bytes per member at 0x55a8).
#[inline] fn mem32(m: i16) -> u16 { (m as u16).wrapping_mul(0x20) }
/// Monster-table / roster slot offset (8 bytes per entry).
#[inline] fn slot8(i: i16) -> u16 { (i as u16).wrapping_mul(8) }

/// DGROUP 0xbd08: 13-byte line buffer used by the shrine dialogue (virtue / mantra answers).
const SHRINE_ANSWER_BUF: u16 = 0xbd08;
/// DGROUP 0xbd15: saved g_location while the shrine cut-scene runs with g_location = 0xff.
const SAVED_LOCATION_BD15: u16 = 0xbd15;
/// DGROUP 0xb21e: g_ool_buffer, reused as the MISCMSG.DAT text buffer (2000 bytes from file
/// offset 0x3ab); the shrine text tables 0x4b5e/0x4b6e hold offsets into it (printed as
/// ptr - 0x4de2 = ptr + 0xb21e).
const MISCMSG_BUF: u16 = 0xb21e;

/// orig 1000:e1e0 (156 bytes). Casting flash and sound for spell `circle` (1..8; 0 for some
/// items). For circle < 9: `play_random_tones(700, circle*1600+8000, 800)`, then the 8x8 view
/// cell at (0xb7,0xb7) is XOR-filled with g_ui_color1, two sweep noises are played from the
/// DATA.OVL per-circle tables (step 0x4b2c[c], thresholds 0x4b08[c] rising and 0x4b1a[c]
/// falling, count 10000+4000*c, phase 0x4af6[c]) and the cell is XOR-filled back. Circle >= 9
/// only plays `play_sweep_noise(1, 300, 65000, 1, 0x1180)` (used by the scroll/potion items).
pub fn casting_flash(gm: &mut Game, circle: i16) {
    if circle < 9 {
        util::play_random_tones(gm, 700, circle.wrapping_mul(0x640).wrapping_add(8000), 800);
        let c = gm.gi16(g::UI_COLOR1);
        gfx::set_color(gm, c);
        gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
        let i = (circle as u16).wrapping_mul(2);
        let step = gm.g16(0x4b2cu16.wrapping_add(i)); // per-circle threshold step
        // s_Missed_ + 1 + circle*4000: the pointer arithmetic is really the number 10000+4000*c.
        let count = 0x2710u16.wrapping_add((circle as u16).wrapping_mul(4000));
        let phase = gm.g16(0x4af6u16.wrapping_add(i));
        let thr_up = gm.g16(0x4b08u16.wrapping_add(i));
        util::play_sweep_noise(gm, step, thr_up, count, 1, phase);
        let thr_down = gm.g16(0x4b1au16.wrapping_add(i));
        util::play_sweep_noise(gm, (step as i16).wrapping_neg() as u16, thr_down, count, 1, phase);
        gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
    } else {
        util::play_sweep_noise(gm, 1, 300, 65000, 1, 0x1180);
    }
}

/// orig 1000:e27e (63 bytes). Prints "On who: " (0x94f4) and runs ROOT
/// `select_party_member_no_zero`; prints the chosen member's name (record 0x55a8+m*0x20) or
/// "None!" (0x94fe) when the result is negative, then a newline if the cursor is not in
/// column 0. Returns the member index (negative = cancelled).
pub fn pick_target_member(gm: &mut Game) -> i16 {
    text::print_string(gm, 0x94f4); // "On who: "
    let m = rest::select_party_member_no_zero(gm);
    let s = if m < 0 { 0x94feu16 /* "None!" */ } else { mem32(m).wrapping_add(0x55a8) };
    text::print_string(gm, s);
    if text::get_cursor_col(gm) != 0 {
        text::put_char(gm, 10);
    }
    m
}

/// orig 1000:e2be (547 bytes). Reads a spell name as rune letters. Each letter key A..Z
/// (except J and O, which have no rune) while fewer than 4 syllables are typed prints the
/// syllable word from the DATA.OVL table 0x1b7a[letter] (= g_rune_syllable_ptrs at 0x1bfc for
/// 'A'), wrapping the line first if column + word length > 13, and remembers the column it
/// started at. Backspace (8) erases the last syllable (repositions the cursor at its column,
/// on the previous row if the cursor sits in column 0, overwrites it with spaces and returns
/// there). Enter (0xd) or Space (0x20) finish; Esc (0x1b) discards all syllables and finishes;
/// other keys are ignored. The letters are then bubble-sorted and compared, with repeated
/// letters collapsed, against the 48 alphabetised spell codes g_spell_code_ptrs[0..0x2f]
/// (exact match required). Returns the spell index, -1 when nothing was typed, else -2 when no
/// spell matched (a newline is printed whenever at least one syllable was entered).
pub fn read_spell_name(gm: &mut Game) -> i16 {
    let mut letters = [0u8; 4];
    let mut cols = [0u8; 4];
    let mut n: i16 = 0;
    let mut result: i16 = -1;
    let mut done = false;
    loop {
        let k = input::read_command_key(gm);
        let c = util::to_upper(k) as u8;
        if c != 0x4a && c != 0x4f {
            if c < 0x41 || 0x5a < c || 3 < n {
                if c == 8 {
                    if n != 0 {
                        let col = cols[(n - 1) as usize] as i16;
                        let mut row = text::get_cursor_row(gm);
                        if text::get_cursor_col(gm) == 0 {
                            row -= 1;
                        }
                        text::set_cursor_pos(gm, row, col);
                        let word = gm.g16(0x1b7au16.wrapping_add((letters[(n - 1) as usize] as u16).wrapping_mul(2)));
                        let mut len = util::str_len(gm, gm.ds(word));
                        if 0 < len {
                            loop {
                                text::put_char(gm, 0x20);
                                len -= 1;
                                if len == 0 { break; }
                            }
                        }
                        let row = text::get_cursor_row(gm);
                        text::set_cursor_pos(gm, row, col);
                        n -= 1;
                    }
                } else {
                    if c != 0xd {
                        if c == 0x1b {
                            n = 0;
                        } else if c != 0x20 {
                            continue; // LAB_1000_e3af: ignored key
                        }
                    }
                    done = true;
                }
            } else {
                letters[n as usize] = c;
                let word = gm.g16(0x1b7au16.wrapping_add((c as u16).wrapping_mul(2)));
                let len = util::str_len(gm, gm.ds(word));
                if 0xd < text::get_cursor_col(gm) + len {
                    text::put_char(gm, 10);
                }
                cols[n as usize] = text::get_cursor_col(gm) as u8;
                text::print_string(gm, word);
                text::put_char(gm, 0x20);
                n += 1;
            }
        }
        if done { break; }
    }
    let count = n;
    // Bubble sort the letters.
    loop {
        let mut sorted = true;
        if 1 < count {
            let mut i: i16 = 0;
            loop {
                if letters[(i + 1) as usize] < letters[i as usize] {
                    letters.swap(i as usize, (i + 1) as usize);
                    sorted = false;
                }
                i += 1;
                if i == n - 1 { break; }
            }
        }
        if sorted { break; }
    }
    // Match against the alphabetised spell codes.
    let mut idx: i16 = 0;
    loop {
        let code = gm.g16(g::SPELL_CODE_PTRS.wrapping_add((idx as u16).wrapping_mul(2)));
        let mut ok = true;
        let mut ci: i16 = 0;
        if 0 < count {
            let mut li: i16 = 0;
            loop {
                if gm.g8(code.wrapping_add(ci as u16)) != letters[li as usize] {
                    ok = false;
                    break;
                }
                while li < count - 1 && letters[(li + 1) as usize] == letters[li as usize] {
                    li += 1;
                }
                li += 1;
                ci += 1;
                if !(li < count) { break; }
            }
        }
        if ok && gm.gi8(code.wrapping_add(ci as u16)) != 0 {
            ok = false;
        }
        if ok {
            result = idx;
            break; // LAB_1000_e481
        }
        idx += 1;
        if 0x2f < idx { break; }
    }
    // LAB_1000_e481
    if count == 0 {
        result = -1;
    } else {
        text::put_char(gm, 10);
        if result < 0 {
            result = -2;
        }
    }
    result
}

/// orig 1000:e4e6 (181 bytes). "Direction-" prompt. Seeds g_dx/g_dy with the party position
/// (or the active combatant's roster x/y in combat, g_location >= 0x80), prints "Direction-"
/// (0x9504) and reads command keys until a direction: 1 West (g_dx-1), 2 East (+1), 3 North
/// (g_dy-1), 4 South (+1) print their name and return the direction code; Space prints
/// "Pass" and returns 0; anything else is ignored.
pub fn ask_direction(gm: &mut Game) -> i16 {
    let mut y = gm.g8(g::PARTY_Y);
    let mut x = gm.g8(g::PARTY_X);
    if 0x7f < gm.g8(g::LOCATION) {
        let ac = gm.g8(g::ACTIVE_COMBATANT) as u16;
        y = gm.g8(g::ROSTER_Y.wrapping_add(ac.wrapping_mul(8)));
        x = gm.g8(g::ROSTER_X.wrapping_add(ac.wrapping_mul(8)));
    }
    gm.set_g16(g::DX, x as u16);
    gm.set_g16(g::DY, y as u16);
    text::print_string(gm, 0x9504); // "Direction-"
    loop {
        let mut k = input::read_command_key(gm);
        let mut got = true;
        if k == 1 {
            text::print_string(gm, 0x9526); // "West\n"
            let v = gm.g16(g::DX); gm.set_g16(g::DX, v.wrapping_sub(1));
        } else if k == 2 {
            text::print_string(gm, 0x9518); // "East\n"
            let v = gm.g16(g::DX); gm.set_g16(g::DX, v.wrapping_add(1));
        } else if k == 3 {
            text::print_string(gm, 0x9510); // "North\n"
            let v = gm.g16(g::DY); gm.set_g16(g::DY, v.wrapping_sub(1));
        } else if k == 4 {
            text::print_string(gm, 0x951e); // "South\n"
            let v = gm.g16(g::DY); gm.set_g16(g::DY, v.wrapping_add(1));
        } else if k == 0x20 {
            text::print_string(gm, 0x952c); // "Pass\n"
            k = 0;
        } else {
            got = false;
        }
        if got { return k; }
    }
}

/// orig 1000:e5a2 (72 bytes). Mani (heal) on member `m`: returns false if the member is dead
/// ('D'); otherwise adds `random_damage_1_30()` hit points capped at the member's max HP
/// (record +0x10 / +0x12) and sets g_status_dirty. Returns true.
pub fn heal_member(gm: &mut Game, m: i16) -> bool {
    let alive = gm.g8(g::PARTY_STATUS.wrapping_add(mem32(m))) != b'D';
    if alive {
        let hp = mem32(m).wrapping_add(0x55b8);
        let amount = rest::random_damage_1_30(gm);
        let max = gm.gi16(g::PARTY_MAX_HP.wrapping_add(mem32(m)));
        rest::add_i16_capped(gm, max, amount, hp);
        gm.set_g8(g::STATUS_DIRTY, 1);
    }
    alive
}

/// orig 1000:e5ea (81 bytes). Ghidra order (kind, dir). Rel Hur / wind change: nothing when
/// both are 0. Casting flash circle 2 when `kind` == 0 (the spell), else circle 1 (the
/// "ask_direction" result 0 = Pass with kind 1 still flashes). Direction codes map to wind
/// values: 0 (Pass) -> 0 calm, 1 W -> 4, 2 E -> 3, 3 N -> 1, 4 S -> 2 and `set_wind_and_draw`
/// is called; any other code just returns (tail e63e/e648).
pub fn rel_hur_change_wind(gm: &mut Game, kind: i16, dir: i16) {
    if dir == 0 && kind == 0 {
        return;
    }
    casting_flash(gm, if kind == 0 { 2 } else { 1 });
    let wind: i16 = if dir == 0 {
        0
    } else if dir == 1 {
        4
    } else if dir == 2 {
        3
    } else if dir == 3 {
        1
    } else if dir == 4 {
        2
    } else {
        rel_hur_epilogue_fragment(gm);
        return;
    };
    rest::set_wind_and_draw(gm, wind);
    rel_hur_epilogue_fragment(gm);
}

/// orig 1000:e63e (5 bytes). Not a function: the `set_wind_and_draw(4)` (West) arm of
/// `rel_hur_change_wind`, split off by Ghidra because ZSTATS.OVL has vector 1778:09da at this
/// address. Kept only so the CAST2 address range is complete.
pub fn rel_hur_west_fragment(gm: &mut Game) {
    rest::set_wind_and_draw(gm, 4);
    rel_hur_epilogue_fragment(gm);
}

/// orig 1000:e648 (4 bytes). Not a function: the `POP BP; RET` epilogue of
/// `rel_hur_change_wind` (Ghidra lists it as a callee). No effect.
pub fn rel_hur_epilogue_fragment(gm: &mut Game) {}

/// orig 1000:e64c (86 bytes). Wis An Ylem (x-ray): rebuilds the lit view with full light
/// (`build_lit_view(0x20, y-chunk_y, x-chunk_x, -1)`) and shows it for 20 frames: each frame
/// animates monsters unless g_status_icon is 'T' (time stop), places objects, draws the map
/// view and waits one tick; finally `refresh_map_view` restores the normal view.
pub fn wis_an_ylem_xray_view(gm: &mut Game) {
    let py = gm.g8(g::PARTY_Y) as u16;
    let cy = gm.g8(g::CHUNK_Y) as u16;
    let px = gm.g8(g::PARTY_X) as u16;
    let cx = gm.g8(g::CHUNK_X) as u16;
    rest::build_lit_view(gm, 0x20, py.wrapping_sub(cy) as i16, px.wrapping_sub(cx) as i16, -1);
    let mut i: i16 = 0x14;
    loop {
        if gm.g8(g::STATUS_ICON) != b'T' {
            rest::animate_monsters(gm);
        }
        rest::place_objects_in_view(gm);
        rest::draw_map_view(gm);
        util::wait_ticks(gm, 1);
        i -= 1;
        if i == 0 { break; }
    }
    rest::refresh_map_view(gm);
}

/// orig 1000:e6a2 (282 bytes). Kal Xen Corp (summon daemon): casting flash circle 8 for the
/// spell (`from_item` == 0) or 5 for the item. Up to 8 attempts to find a random combat cell
/// (COMBAT `random_combat_cell`) that is free for a tile-0xd8 mover (COMBAT
/// `combat_cell_free`) and whose map tile is not 0xff; failing that returns 0. Then
/// `add_combatant(z, dy, dx, 0, 0x26)` (monster type 0x26 = daemon); on success a sweep noise
/// is played, the new slot's tile fields are set to 0x16 while `tile_transition_effect` plays
/// tile 0x1d8 at the cell, then set to 0xd8 (daemon). For the spell only, the caster's
/// intelligence (`get_combat_stat(-1, active)`) must exceed a 1..30 roll or "Oops..." is
/// printed and 0xffff returned (the daemon stays hostile). Otherwise the roster flags bit 0
/// (charmed/ally) is set and 1 returned. 0 if the roster is full.
pub fn kal_xen_corp_summon_daemon(gm: &mut Game, from_item: i16) -> u16 {
    casting_flash(gm, if from_item == 0 { 8 } else { 5 });
    let mut tries: i16 = 0;
    loop {
        let mut ok = ovl::combat::random_combat_cell(gm) != 0;
        if ok {
            let (dy, dx) = (gm.g16(g::DY), gm.g16(g::DX));
            ok = ovl::combat::combat_cell_free(gm, dy, dx, 0xd8) != 0;
        }
        if ok {
            let (dy, dx) = (gm.g16(g::DY), gm.g16(g::DX));
            let p = rest::get_map_tile(gm, dy, dx);
            ok = gm.gi8(p) != -1;
        }
        if ok { break; }
        tries += 1;
        if 7 < tries {
            return 0;
        }
    }
    let z = gm.g8(g::PARTY_Z);
    let (dy, dx) = (gm.g16(g::DY), gm.g16(g::DX));
    let idx = rest::add_combatant(gm, z, dy as u8, dx as u8, 0, 0x26) as i16;
    if -1 < idx {
        util::play_sweep_noise(gm, 5, 500, 12000, 1, 0xac8);
        let s = (gm.g8(g::ROSTER_SLOT.wrapping_add(slot8(idx))) as u16).wrapping_mul(8);
        gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(s), 0x16);
        gm.set_g8(g::MONSTER_TABLE.wrapping_add(s), 0x16);
        gfx::tile_transition_effect(gm, dy as i16, dx as i16, 0x1d8);
        gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(s), 0xd8);
        gm.set_g8(g::MONSTER_TABLE.wrapping_add(s), 0xd8);
        if from_item == 0 {
            let ac = gm.g8(g::ACTIVE_COMBATANT) as i16;
            let int = ovl::combat::get_combat_stat(gm, -1, ac) as i16;
            let roll = rest::random_damage_1_30(gm);
            if int <= roll {
                text::print_string(gm, 0x9532); // "Oops...\n"
                return 0xffff;
            }
        }
        let f = g::ROSTER_FLAGS.wrapping_add(slot8(idx));
        let v = gm.g8(f);
        gm.set_g8(f, v | 1);
        return 1;
    }
    0
}

/// orig 1000:e7c0 (262 bytes). Ghidra order (mode, member). In Mani Corp (resurrect).
/// member < 0 -> 0xffff (cancelled). A member who is not 'D' returns 0, printing "Not dead!"
/// when `mode` != 0. Otherwise: status 'G', HP = 1; MP (record +0xf) = intelligence (+0xe) for
/// classes 'A' and 'M', intelligence/2 for 'B', unchanged otherwise; if g_karma < 98 the
/// experience (+0x14, 16-bit) is scaled to exp*karma/100 (32-bit arithmetic); level (+0x16) =
/// 1 + number of halvings of exp/100 down to 0; max HP = level*30. Casting flash circle 8 for
/// mode 0 (spell), 6 for mode 1 (item), none for any other mode (BLCKTHRN passes 0xff for the
/// Lord British resurrection). Sets g_status_dirty and returns 1.
pub fn in_mani_corp_resurrect(gm: &mut Game, mode: i16, member: i16) -> u16 {
    if member < 0 {
        return 0xffff;
    }
    let rec = mem32(member);
    if gm.g8(g::PARTY_STATUS.wrapping_add(rec)) != b'D' {
        if mode != 0 {
            text::print_string(gm, 0x953c); // "Not dead!\n"
        }
        return 0;
    }
    gm.set_g8(g::PARTY_STATUS.wrapping_add(rec), 0x47); // 'G'
    gm.set_g16(rec.wrapping_add(0x55b8), 1); // hp
    let class = gm.g8(rec.wrapping_add(0x55b2));
    let int = gm.g8(rec.wrapping_add(0x55b6)); // intelligence
    if class == b'A' || class == b'M' {
        gm.set_g8(rec.wrapping_add(0x55b7), int); // mp
    } else if class == b'B' {
        gm.set_g8(rec.wrapping_add(0x55b7), int >> 1);
    }
    // LAB_1000_e82e
    let karma = gm.g8(g::KARMA);
    if karma < 0x62 {
        let exp = gm.gi16(rec.wrapping_add(0x55bc)) as i32;
        let scaled = exp.wrapping_mul(karma as i32) / 100; // __aFlmul / __aFldiv
        gm.set_g16(rec.wrapping_add(0x55bc), scaled as u16);
    }
    let mut level: i16 = 1;
    let mut e = gm.gi16(rec.wrapping_add(0x55bc)) / 100;
    while 0 < e {
        level += 1;
        e >>= 1;
    }
    gm.set_g8(rec.wrapping_add(0x55be), level as u8);
    gm.set_gi16(g::PARTY_MAX_HP.wrapping_add(rec), level.wrapping_mul(0x1e));
    let circle: i16 = if mode == 1 {
        6
    } else if mode != 0 {
        gm.set_g8(g::STATUS_DIRTY, 1);
        return 1;
    } else {
        8
    };
    casting_flash(gm, circle);
    gm.set_g8(g::STATUS_DIRTY, 1);
    1
}

/// orig 1000:e8cc (124 bytes). In Wis (locate): in font 1 prints a newline then the party
/// position as rune letters: `'A'+(y>>4)` `'` `'A'+(y&15)` `", ` `'A'+(x>>4)` `'` `'A'+(x&15)`
/// `"`, returns to font 0 and prints a newline.
pub fn in_wis_print_position(gm: &mut Game) {
    text::select_font(gm, 1);
    text::put_char(gm, 10);
    let y = gm.g8(g::PARTY_Y);
    text::put_char(gm, (y >> 4).wrapping_add(0x41));
    text::put_char(gm, 0x27);
    text::put_char(gm, (y & 0xf).wrapping_add(0x41));
    text::print_string(gm, 0x9548); // "\", "
    let x = gm.g8(g::PARTY_X);
    text::put_char(gm, (x >> 4).wrapping_add(0x41));
    text::put_char(gm, 0x27);
    text::put_char(gm, (x & 0xf).wrapping_add(0x41));
    text::put_char(gm, 0x22);
    text::select_font(gm, 0);
    text::put_char(gm, 10);
}

/// orig 1000:e948 (82 bytes). In Ex Por (magic unlock), also the skull key: asks a direction
/// (0 = Pass -> 0xffff). The map tile at (g_dy,g_dx): magic-locked door 0x97 becomes 0xb8,
/// 0x98 becomes 0xba (unlocked doors), g_view_dirty |= 2 and 1 is returned; any other tile
/// returns 0 ("Failed").
pub fn in_ex_por_unlock_door(gm: &mut Game) -> u16 {
    if ask_direction(gm) == 0 {
        return 0xffff;
    }
    let (dy, dx) = (gm.g16(g::DY), gm.g16(g::DX));
    let p = rest::get_map_tile(gm, dy, dx);
    let t = gm.g8(p);
    if t == 0x97 {
        gm.set_g8(p, 0xb8);
    } else if t == 0x98 {
        gm.set_g8(p, 0xba);
    } else {
        return 0;
    }
    let vd = gm.g8(g::VIEW_DIRTY);
    gm.set_g8(g::VIEW_DIRTY, vd | 2);
    1
}

/// orig 1000:e99c (297 bytes). An Grav (dispel field). Outside combat (g_location < 0x80,
/// i.e. in a dungeon): casting flash circle 4 when `flash` != 0; the dungeon cell at the
/// party's position (g_dungeon_map[z][y][x]) or, if that is not a field (upper nibble 8),
/// the cell one step ahead in g_dungeon_facing (dy table 0x24de, dx table 0x24d6, wrapped
/// & 7); a field cell is reduced to (cell & 8), "Field destroyed!" (0x954c) is printed and
/// 0xffff returned, else 0. In combat: asks a direction (Pass -> 0xffff), flashes circle 4 if
/// requested, then clears (`set_monster_slot(i, 0,0,0,0,0,0)`) the first monster-table slot
/// whose tile & 0xfc == 0xe8 (field tiles 0xe8..0xeb) at (g_dx,g_dy) and returns 1; 0 if none.
pub fn an_grav_dispel_field(gm: &mut Game, flash: i16) -> u16 {
    if gm.g8(g::LOCATION) < 0x80 {
        if flash != 0 {
            casting_flash(gm, 4);
        }
        let z = gm.g8(g::PARTY_Z) as u16;
        let y = gm.g8(g::PARTY_Y) as u16;
        let x = gm.g8(g::PARTY_X) as u16;
        let mut cell = z.wrapping_mul(0x40).wrapping_add(y.wrapping_mul(8)).wrapping_add(x).wrapping_add(0x595a);
        if gm.g8(cell) & 0xf0 != 0x80 {
            let f = gm.g8(g::DUNGEON_FACING) as u16;
            let dy = gm.g16(0x24deu16.wrapping_add(f.wrapping_mul(2))); // facing -> dy (N -1, E 0, S 1, W 0)
            let dx = gm.g16(0x24d6u16.wrapping_add(f.wrapping_mul(2))); // facing -> dx (0, 1, 0, -1)
            cell = (dy.wrapping_add(y) & 7).wrapping_mul(8)
                .wrapping_add(dx.wrapping_add(x) & 7)
                .wrapping_add(z.wrapping_mul(0x40))
                .wrapping_add(0x595a);
        }
        if gm.g8(cell) & 0xf0 == 0x80 {
            let v = gm.g8(cell);
            gm.set_g8(cell, v & 8);
            text::print_string(gm, 0x954c); // "Field destroyed!\n"
            0xffff
        } else {
            0
        }
    } else {
        if ask_direction(gm) == 0 {
            return 0xffff;
        }
        if flash != 0 {
            casting_flash(gm, 4);
        }
        let mut i: i16 = 0;
        loop {
            let s = slot8(i);
            if gm.g8(g::MONSTER_TABLE.wrapping_add(s)) & 0xfc == 0xe8
                && gm.g8(g::MONSTER_X.wrapping_add(s)) as u16 == gm.g16(g::DX)
                && gm.g8(g::MONSTER_Y.wrapping_add(s)) as u16 == gm.g16(g::DY)
            {
                rest::set_monster_slot(gm, i, 0, 0, 0, 0, 0, 0);
                return 1;
            }
            i += 1;
            if !(i < 0x20) { break; }
        }
        0
    }
}

/// orig 1000:eaca (13 bytes). Sets g_light_spell_turns (In Lor 100, Vas Lor 0xff, scroll
/// 0xf0; BLCKTHRN re-applies the saved value).
pub fn set_light_spell_turns(gm: &mut Game, turns: u8) {
    gm.set_g8(g::LIGHT_SPELL_TURNS, turns);
}

/// orig 1000:ead8 (28 bytes). Ghidra order (circle, turns, icon). Starts a timed status
/// effect: g_status_icon = `icon` ('P' protection, 'Q' quickness, 'C' confusion, 'N' negate
/// magic, ...), g_protection_turns = `turns`, casting flash for `circle`, redraw the status
/// window.
pub fn start_timed_effect(gm: &mut Game, circle: u16, turns: u8, icon: u8) {
    gm.set_g8(g::STATUS_ICON, icon);
    gm.set_g8(g::PROTECTION_TURNS, turns);
    casting_flash(gm, circle as i16);
    rest::draw_status_window(gm);
}

/// orig 1000:eaf4 (82 bytes). Places `count` urns (tile 3, anim 3) in monster-table slots
/// 1..count for the Shrine of the Codex scene: the DATA.OVL table 0x4b95[count] gives the
/// first index into the position tables 0x4bc2 (y) / 0x4b9e (x), consecutive entries follow.
/// z = g_party_z.
pub fn place_codex_urns(gm: &mut Game, count: i16) {
    let mut k = gm.g8(0x4b95u16.wrapping_add(count as u16)) as u16;
    if 0 < count {
        let mut i: i16 = 0;
        while i != count {
            let z = gm.g8(g::PARTY_Z);
            let y = gm.g8(0x4bc2u16.wrapping_add(k));
            let x = gm.g8(0x4b9eu16.wrapping_add(k));
            rest::set_monster_slot(gm, i + 1, 0, z, y, x, 3, 3);
            k = k.wrapping_add(1);
            i += 1;
        }
    }
}

/// orig 1000:eb46 (212 bytes, continues through ec1a). Virtue-shrine dialogue. The shrine
/// index 0..7 is found from g_shrine_x/g_shrine_y (0x1f6e/0x1f76) against the party position
/// (6 = Spirituality when none matches). The party tile becomes 0x6c (kneeling), the shrine
/// text (MISCMSG 0xb58b, 0xb5b6 "what virtue?") is shown and a 12-char answer read into 0xbd08;
/// an empty answer leaves. The answer must contain the shrine's virtue stem (0x4b3e[i]) and
/// the mantra (0x1f5e[i]) must be given three times ("Mantra:" 0x958e; an empty line leaves).
/// On success: (a) quest not yet completed (g_shrine_quests_done bit clear): ordain the quest
/// (g_shrine_quests bit set), show the quest text (0xb5ff, 0xb62c, quest string
/// 0xb21e+0x4b5e[i], 0xb669) with a 7-note tune from tables 0x4be6/0x4bf4/0x4c02/0x4c10;
/// (b) quest completed but not ordained: ask for a gold donation of 0..9 hundred (0xb692;
/// "0" -> " gp" and leave; not enough gold -> 0xb6b9 and ask again), deduct it, add the
/// hundreds to karma (cap 99), print "ALAKAZAM" in font 1 and a rising/falling sweep;
/// (c) quest ordained and done: clear the g_shrine_quests bit, print 0xb6d7 with a rising/
/// falling sweep, `earthquake_shake`, karma +3 (cap 99), then +1 strength / dexterity /
/// intelligence (cap 30) per the flag tables 0x4b7e/0x4b86/0x4b8e, and shrine 7 (Humility)
/// gives another +3 karma. Wrong answers print 0xb5de.
pub fn shrine_quest_dialogue(gm: &mut Game) {
    let mut paid = false;
    let mut sh: i16 = 0;
    loop {
        if gm.g8(g::SHRINE_X.wrapping_add(sh as u16)) == gm.g8(g::PARTY_X)
            && gm.g8(0x1f76u16.wrapping_add(sh as u16)) == gm.g8(g::PARTY_Y)
        {
            break;
        }
        sh += 1;
        if !(sh < 8) { break; }
    }
    if sh == 8 {
        sh = 6;
    }
    gm.set_g8(g::PARTY_VEHICLE_TILE, 0x6c);
    gm.set_g8(g::MONSTER_TABLE, 0x6c);
    rest::refresh_map_view(gm);
    text::print_string(gm, 0xb58b); // MISCMSG: shrine greeting
    util::animate_wait(gm, 10);
    text::print_string(gm, 0xb5b6); // MISCMSG: "what virtue dost thou seek?"
    rest::read_text_line(gm, 0xc, SHRINE_ANSWER_BUF);
    if gm.g8(SHRINE_ANSWER_BUF) != 0 {
        let stem = gm.g16(0x4b3eu16.wrapping_add((sh as u16).wrapping_mul(2)));
        let mut ok = rest::str_find_ci(gm, SHRINE_ANSWER_BUF as i16, stem) != 0xffff;
        util::animate_wait(gm, 6);
        text::put_char(gm, 10);
        let mut k: i16 = 0;
        loop {
            text::print_string(gm, 0x958e); // "\nMantra:"
            rest::read_text_line(gm, 0xc, SHRINE_ANSWER_BUF);
            if gm.g8(SHRINE_ANSWER_BUF) == 0 {
                return;
            }
            let mantra = gm.g16(0x1f5eu16.wrapping_add((sh as u16).wrapping_mul(2)));
            if rest::str_find_ci(gm, SHRINE_ANSWER_BUF as i16, mantra) == 0xffff {
                ok = false;
            }
            util::animate_wait(gm, 0xc);
            k += 1;
            if !(k < 3) { break; }
        }
        if ok {
            let bit: u16 = 1u16 << ((sh as u8) & 0x1f);
            if gm.g8(g::SHRINE_QUESTS_DONE) as u16 & bit == 0 {
                let q = gm.g8(g::SHRINE_QUESTS);
                gm.set_g8(g::SHRINE_QUESTS, q | (bit as u8));
                text::print_string(gm, 0xb5ff);
                gm.set_g8(g::PARTY_VEHICLE_TILE, 0x1c);
                gm.set_g8(g::MONSTER_TABLE, 0x1c);
                input::read_command_key(gm);
                text::print_string(gm, 0xb62c);
                let quest = gm.g16(0x4b5eu16.wrapping_add((sh as u16).wrapping_mul(2))).wrapping_sub(0x4de2);
                text::print_string(gm, quest);
                text::print_string(gm, 0x9598); // "\"\n"
                input::read_command_key(gm);
                text::print_string(gm, 0xb669);
                let mut j: u16 = 0;
                loop {
                    let step = gm.g16(0x4c10u16.wrapping_add(j));
                    let thr = gm.g16(0x4c02u16.wrapping_add(j));
                    let count = gm.g16(0x4bf4u16.wrapping_add(j));
                    let phase = gm.g16(0x4be6u16.wrapping_add(j));
                    util::play_sweep_noise(gm, step, thr, count, 1, phase);
                    j = j.wrapping_add(2);
                    if !(0x4c10u16.wrapping_add(j) < 0x4c1e) { break; }
                }
            } else if gm.g8(g::SHRINE_QUESTS) as u16 & bit == 0 {
                let mut hundreds: i16 = 0;
                while !paid {
                    text::print_string(gm, 0xb692);
                    let mut key: u16;
                    loop {
                        loop {
                            key = input::read_command_key(gm) as u16;
                            if !(key < 0x30) { break; }
                        }
                        if !(0x39 < key) { break; }
                    }
                    text::put_char(gm, key as u8);
                    hundreds = key.wrapping_sub(0x30) as i16;
                    if hundreds == 0 {
                        text::print_string(gm, 0x959c); // " gp\n"
                        return;
                    }
                    text::print_string(gm, 0x95a2); // "00 gp\n\n"
                    let gold = gm.gi16(g::GOLD);
                    if hundreds.wrapping_mul(100).wrapping_sub(gold) == 0 || hundreds.wrapping_mul(100) < gold {
                        paid = true;
                    } else {
                        text::print_string(gm, 0xb6b9);
                    }
                }
                let gold = gm.gi16(g::GOLD);
                gm.set_gi16(g::GOLD, gold.wrapping_add(hundreds.wrapping_mul(-100)));
                rest::draw_status_window(gm);
                let k = gm.g8(g::KARMA).wrapping_add(hundreds as u8);
                gm.set_g8(g::KARMA, if 99 < k { 99 } else { k });
                text::select_font(gm, 1);
                text::print_string(gm, 0x95aa); // "ALAKAZAM"
                text::select_font(gm, 0);
                text::print_string(gm, 0x95b4); // "!\n"
                let c = gm.gi16(g::UI_COLOR1);
                gfx::set_color(gm, c);
                gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
                let mut f: i16 = 2000;
                loop {
                    util::play_sweep_noise(gm, 0, f as u16, 200, 1, 0xa8c);
                    f += 0x32;
                    if !(f < 25000) { break; }
                }
                f = 25000;
                loop {
                    util::play_sweep_noise(gm, 0, f as u16, 200, 1, 0xa8c);
                    f -= 0x32;
                    if !(2000 < f) { break; }
                }
            } else {
                let q = gm.g8(g::SHRINE_QUESTS) as u16;
                gm.set_g8(g::SHRINE_QUESTS, (q & ((bit & q) ^ 0xff)) as u8);
                text::print_string(gm, 0xb6d7);
                let c = gm.gi16(g::UI_COLOR1);
                gfx::set_color(gm, c);
                gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
                let mut f: i16 = 2000;
                loop {
                    util::play_sweep_noise(gm, 0, f as u16, 0x96, 1, 0xc1c);
                    f += 0x32;
                    if !(f < 25000) { break; }
                }
                f = 25000;
                loop {
                    util::play_sweep_noise(gm, 0, f as u16, 0x96, 1, 0xc1c);
                    f -= 0x32;
                    if !(2000 < f) { break; }
                }
                rest::earthquake_shake(gm);
                let k = gm.g8(g::KARMA).wrapping_add(3);
                gm.set_g8(g::KARMA, if 99 < k { 99 } else { k });
                if gm.gi8(0x4b7eu16.wrapping_add(sh as u16)) != 0 {
                    let v = gm.g8(0x55b4).wrapping_add(1); // strength (record +0xc)
                    gm.set_g8(0x55b4, if 0x1e < v { 0x1e } else { v });
                    text::print_string(gm, 0x95b8); // "Strength +1\n"
                }
                if gm.gi8(0x4b86u16.wrapping_add(sh as u16)) != 0 {
                    let v = gm.g8(g::PARTY_DEX).wrapping_add(1);
                    gm.set_g8(g::PARTY_DEX, if 0x1e < v { 0x1e } else { v });
                    text::print_string(gm, 0x95c6); // "Dexterity +1\n"
                }
                if gm.gi8(0x4b8eu16.wrapping_add(sh as u16)) != 0 {
                    let v = gm.g8(0x55b6).wrapping_add(1); // intelligence (record +0xe)
                    gm.set_g8(0x55b6, if 0x1e < v { 0x1e } else { v });
                    text::print_string(gm, 0x95d4); // "Intelligence +1\n"
                }
                if sh == 7 {
                    let k = gm.g8(g::KARMA).wrapping_add(3);
                    if 99 < k {
                        gm.set_g8(g::KARMA, 99);
                    } else {
                        gm.set_g8(g::KARMA, k);
                    }
                }
            }
            util::animate_wait(gm, 10);
        } else {
            text::print_string(gm, 0xb5de); // MISCMSG: wrong answer
        }
    }
}

/// orig 1000:ec1a (739 bytes as listed). Not a function: the second half of
/// `shrine_quest_dialogue` (from the mantra loop onwards), split by Ghidra because ZSTATS.OVL
/// has vector 1778:06b6 at this address; its decompilation uses unassigned BP locals. The
/// whole logic is translated in `shrine_quest_dialogue`; this entry has no effect.
pub fn shrine_quest_dialogue_fragment(gm: &mut Game) {}

/// orig 1000:ef04 (319 bytes). Shrine of the Codex dialogue: three key waits with MISCMSG
/// texts 0xb703 and 0xb733, then the lowest ordained quest bit in g_shrine_quests is looked
/// up; none -> 0xb75c. Otherwise that bit is set in g_shrine_quests_done, the virtue's Codex
/// text (quoted, 0xb21e+0x4b6e[i], then 0x95e6) is shown; when all eight are done
/// (g_shrine_quests_done == 0xff) the view cell flashes three times with earthquakes and the
/// final Codex wisdom (0xb773, "Thou dost read:" 0x95ea, font-1 texts 0xb79d, 0xb7ef, 0xb82a,
/// 0xb89e) is shown page by page.
pub fn codex_shrine_dialogue(gm: &mut Game) {
    input::read_command_key(gm);
    text::print_string(gm, 0xb703);
    input::read_command_key(gm);
    text::print_string(gm, 0xb733);
    input::read_command_key(gm);
    let mut i: i16 = 0;
    loop {
        if gm.g8(g::SHRINE_QUESTS) as u16 & (1u16 << ((i as u8) & 0x1f)) != 0 {
            break;
        }
        i += 1;
        if !(i < 8) { break; }
    }
    if i == 8 {
        text::print_string(gm, 0xb75c);
    } else {
        let bit = (1u16 << ((i as u8) & 0x1f)) as u8;
        let d = gm.g8(g::SHRINE_QUESTS_DONE);
        gm.set_g8(g::SHRINE_QUESTS_DONE, d | bit);
        text::put_char(gm, 0x22);
        let s = gm.g16(0x4b6eu16.wrapping_add((i as u16).wrapping_mul(2))).wrapping_sub(0x4de2);
        text::print_string(gm, s);
        text::print_string(gm, 0x95e6); // "\"\n\n"
        input::read_command_key(gm);
        if gm.g8(g::SHRINE_QUESTS_DONE) == 0xff {
            let c0 = gm.gi16(g::UI_COLOR0);
            let c1 = gm.gi16(g::UI_COLOR1);
            gfx::set_color(gm, c0);
            gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
            rest::earthquake_shake(gm);
            gfx::set_color(gm, c1);
            gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
            rest::earthquake_shake(gm);
            gfx::set_color(gm, c0);
            gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
            rest::earthquake_shake(gm);
            text::print_string(gm, 0xb773);
            input::read_command_key(gm);
            text::print_string(gm, 0x95ea); // "Thou dost read:\n\n"
            for s in [0xb79du16, 0xb7ef, 0xb82a, 0xb89e] {
                text::select_font(gm, 1);
                text::print_string(gm, s);
                text::select_font(gm, 0);
                input::read_command_key(gm);
            }
        }
    }
}

/// orig 1000:f044 (18 bytes). One walking step of the shrine scene: animate_wait(1),
/// footstep sound, animate_wait(4).
pub fn shrine_step_pause(gm: &mut Game) {
    util::animate_wait(gm, 1);
    rest::play_footstep_sound(gm);
    util::animate_wait(gm, 4);
}

/// orig 1000:f056 (645 bytes). Vector 1778:02ea. The shrine cut-scene (MAINOUT/TOWN call it
/// after `enter_moongate` succeeds on a shrine tile, and MAINOUT for the Shrine of the Codex,
/// map tile 0x11). Saves g_location to 0xbd15 and sets it to 0xff, g_active_combatant 0xff,
/// backs up the monster table (0x5c5a..0x5d5a) to 0xa9fc clearing every slot's tile. Loads
/// the 11x11 shrine map from MISCMAPS.DAT (offset 0x160 Codex, 0xb0 virtue shrine; 0xb0 bytes
/// as 11 rows of 16) into g_view_objects, MISCMSG.DAT text (file 0x3ab, 2000 bytes) into
/// 0xb21e, and copies the rows into g_light_map (stride 32). Counts the party records 1..15
/// whose byte +0x1f is 0x7f (members lost in the Underworld, shown as urns) and places urns.
/// Prints the arrival text (0xb8f9 Codex / 0xb8cc shrine), walks the party figure (slot 0,
/// tile 0x1c at x 5, y 10) up 7 (Codex) or 4 rows with footstep pauses, at the Codex listing
/// "Thou dost see an urn / urns marked:" with the lost members' names in upper case, runs the
/// Codex or virtue dialogue, walks back down to row 10, clears the figure, waits, restores
/// the monster table and g_location, passes 16 minutes and rebuilds the light map.
pub fn visit_shrine(gm: &mut Game) {
    let (py, px) = (gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
    let p = rest::get_map_tile(gm, py, px);
    let tile = gm.gi8(p);
    gm.set_g8(g::VIEW_DIRTY, 1);
    let loc = gm.g8(g::LOCATION);
    gm.set_g8(SAVED_LOCATION_BD15, loc);
    gm.set_g8(g::LOCATION, 0xff);
    gm.set_g8(g::ACTIVE_COMBATANT, 0xff);
    let mut s: u16 = 0;
    while g::MONSTER_TABLE.wrapping_add(s) < 0x5d5a {
        for k in 0..8u16 {
            let v = gm.g8(g::MONSTER_TABLE.wrapping_add(s).wrapping_add(k));
            gm.set_g8(g::MONSTER_TABLE_BACKUP_A9FC.wrapping_add(s).wrapping_add(k), v);
        }
        gm.set_g8(g::MONSTER_TABLE.wrapping_add(s), 0);
        s = s.wrapping_add(8);
    }
    let (name, off): (u16, u16) = if tile == 0x11 { (0x960a, 0x160) } else { (0x95fc, 0xb0) }; // "MISCMAPS.DAT"
    files::load_file_region(gm, off, 0xb0, g::VIEW_OBJECTS, name);
    files::load_file_region(gm, 0x3ab, 2000, MISCMSG_BUF, 0x9618); // "MISCMSG.DAT"
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
    rest::refresh_map_view(gm);
    let mut urns: i16 = 0;
    let mut q: u16 = 0x55e7; // record 1 + 0x1f
    loop {
        if gm.g8(q) == 0x7f {
            urns += 1;
        }
        q = q.wrapping_add(0x20);
        if !(q < 0x57c7) { break; }
    }
    if 0 < urns {
        place_codex_urns(gm, urns);
    }
    text::print_string(gm, if tile == 0x11 { 0xb8f9 } else { 0xb8cc });
    for _ in 0..4 {
        shrine_step_pause(gm);
    }
    gm.set_g8(g::PARTY_VEHICLE_TILE, 0x1c);
    gm.set_g8(g::MONSTER_TABLE, 0x1c);
    gm.set_g8(g::MONSTER_X, 5);
    gm.set_g8(g::MONSTER_Y, 10);
    let z = gm.g8(g::PARTY_Z);
    gm.set_g8(g::MONSTER_Z, z);
    shrine_step_pause(gm);
    if tile == 0x11 && 0 < urns {
        text::print_string(gm, 0x9624); // "\n\nThou dost see\n"
        text::print_string(gm, if urns == 1 { 0x9636 } else { 0x9648 }); // "an urn marked:" / "urns marked:"
        let mut rec: u16 = 0x20;
        loop {
            if gm.g8(rec.wrapping_add(0x55c7)) == 0x7f { // record +0x1f
                let mut i: u16 = 0;
                let mut ch = gm.g8(rec.wrapping_add(0x55a8));
                while ch != 0 {
                    let c = gm.g8(rec.wrapping_add(i).wrapping_add(0x55a8));
                    text::put_char(gm, util::to_upper(c as i16) as u8);
                    ch = gm.g8(rec.wrapping_add(i).wrapping_add(1).wrapping_add(0x55a8));
                    i = i.wrapping_add(1);
                }
                text::put_char(gm, 10);
            }
            rec = rec.wrapping_add(0x20);
            if !(rec < 0x200) { break; }
        }
    }
    let mut steps: i16 = if tile == 0x11 { 7 } else { 4 };
    while steps != 0 {
        let y = gm.g8(g::MONSTER_Y);
        gm.set_g8(g::MONSTER_Y, y.wrapping_sub(1));
        shrine_step_pause(gm);
        steps -= 1;
    }
    if tile == 0x11 {
        codex_shrine_dialogue(gm);
    } else {
        shrine_quest_dialogue(gm);
    }
    gm.set_g8(g::PARTY_VEHICLE_TILE, 0x1c);
    gm.set_g8(g::MONSTER_TABLE, 0x1c);
    util::animate_wait(gm, 1);
    if gm.g8(g::MONSTER_Y) < 10 {
        let mut k: i16 = 10 - gm.g8(g::MONSTER_Y) as i16;
        loop {
            let y = gm.g8(g::MONSTER_Y);
            gm.set_g8(g::MONSTER_Y, y.wrapping_add(1));
            shrine_step_pause(gm);
            k -= 1;
            if k == 0 { break; }
        }
    }
    gm.set_g8(g::MONSTER_Y, 0);
    gm.set_g8(g::MONSTER_X, 0);
    gm.set_g8(g::MONSTER_TABLE, 0);
    for _ in 0..4 {
        shrine_step_pause(gm);
    }
    util::animate_wait(gm, 10);
    let mut s: u16 = 0;
    while g::MONSTER_TABLE_BACKUP_A9FC.wrapping_add(s) < 0xaafc {
        for k in 0..8u16 {
            let v = gm.g8(g::MONSTER_TABLE_BACKUP_A9FC.wrapping_add(s).wrapping_add(k));
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(s).wrapping_add(k), v);
        }
        s = s.wrapping_add(8);
    }
    let loc = gm.g8(SAVED_LOCATION_BD15);
    gm.set_g8(g::LOCATION, loc);
    rest::pass_time(gm, 0x10);
    rest::rebuild_light_map(gm);
}

/// orig 1000:f2de (187 bytes). Vector 1778:0a2e, the Quit & Save command. Prints "\nSave
/// game? " and waits for Y or N. N prints "No". Y prints "Yes\nSaving...", then: if the
/// current disk (g_cur_disk) is not 5, `select_disk(1)`; loads UNDER.OOL (0x100 bytes) into
/// 0xb31e, `select_disk(1)`, loads BRIT.OOL (0x100 bytes) into 0xb21e; if the current disk
/// was not 1 writes UNDER.OOL back from 0xb31e; `select_disk(3)` and writes SAVED.GAM (0x1060
/// bytes from 0x55a6) and SAVED.OOL (0x200 bytes from 0xb21e); restores the disk and prints
/// "Done.".
pub fn save_game_prompt(gm: &mut Game) {
    text::print_string(gm, 0x9658); // "\nSave game? "
    let mut c: u8;
    loop {
        c = input::read_command_key(gm) as u8;
        if c == b'Y' { break; }
        if c == b'N' { break; }
    }
    let msg: u16;
    if c == b'N' {
        msg = 0x9666; // "No\n"
    } else {
        text::print_string(gm, 0x966a); // "Yes\nSaving...\n"
        let cur = gm.gi8(g::CUR_DISK);
        if cur != 5 {
            files::select_disk(gm, 1);
        }
        files::load_file_region(gm, 0, 0x100, 0xb31e, 0x967a); // "UNDER.OOL"
        files::select_disk(gm, 1);
        files::load_file_region(gm, 0, 0x100, 0xb21e, 0x9684); // "BRIT.OOL"
        if cur != 1 {
            files::save_file_region(gm, 0x100, 0xb31e, 0x968e); // "UNDER.OOL"
        }
        files::select_disk(gm, 3);
        files::save_file_region(gm, 0x1060, 0x55a6, 0x9698); // "SAVED.GAM"
        files::save_file_region(gm, 0x200, 0xb21e, 0x96a2); // "SAVED.OOL"
        files::select_disk(gm, cur as i16);
        msg = 0x96ac; // "Done.\n"
    }
    text::print_string(gm, msg);
}
