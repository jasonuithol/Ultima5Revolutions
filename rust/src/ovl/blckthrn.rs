//! BLCKTHRN.OVL (translated)
//!
//! Two cut-scenes. `blackthorn_dungeon_scene` (vector 1778:033e, from TOWN `guard_arrest` in
//! Castle Blackthorn 0x12): the party is subdued, dragged to the dungeon, Blackthorn demands the
//! mantra of the first virtue not yet betrayed (g_shrine_destroyed[0..7] == 0); the player is
//! questioned up to four times (`interrogation`), a companion is threatened and, with two or
//! more living members, killed (`execute_prisoner`: the character is moved to party record 15
//! and marked 0x7f); giving the mantra betrays the shrine (flag 0xff, 5 karma) and releases the
//! party. Both outcomes leave the party at (10,7) in the castle basement. `resurrection_scene`
//! (vector 1778:02de, from the TOWN/MAINOUT loops when nobody is alive): darkness, Lord
//! British's "FORTIS FORTUNA AVENTARI", KARMA.DAT text chosen by karma/20, every member healed
//! and resurrected (CAST2 e7c0), and the party wakes in Castle British (0x11) at 06:00.
//! Animated figures are driven by byte-code move scripts in DGROUP (0x369e, 0x36da, 0x3702,
//! 0x370e, 0x3716) interpreted by `run_move_script`, which moves monster-table slots 0/1/6/7/8.
//!
//! Ghidra labelled 1000:a290 NPC_a290 and 1000:a558 OUTSUBS_a558 because other level-2
//! overlays have functions at the same addresses; a558 is a fragment of
//! `print_interrogation_question` (translated as a no-op).
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// orig 1000:a290 (46 bytes). Plays `count` footsteps, each followed by two animation frames.
pub fn footsteps(gm: &mut Game, param_1: i16) {
    let mut n = param_1;
    if 0 < n {
        loop {
            rest::play_footstep_sound(gm);
            util::animate_wait(gm, 2);
            n -= 1;
            if n == 0 { break; }
        }
    }
}

/// orig 1000:a2be (138 bytes). Decodes a move-script byte: the high bits select the monster
/// slot (0x10 -> 6, 0x14 -> 7, 0x18 -> 8, 0x1c -> 0, 0x20 -> 1; any other value leaves the
/// result undefined - the port returns 0) and the low two bits set g_dx/g_dy (0 N, 1 E, 2 S, 3 W).
pub fn decode_move_code(gm: &mut Game, param_1: u16) -> u16 {
    let code = param_1;
    let hi = code & 0xfc;
    let mut slot: u16 = 0; // TODO(verify): uninitialised in the original for other codes
    if hi == 0x10 {
        slot = 6;
    } else if hi == 0x14 {
        slot = 7;
    } else if hi == 0x18 {
        slot = 8;
    } else if hi == 0x1c {
        slot = 0;
    } else if hi == 0x20 {
        slot = 1;
    }
    gm.set_g16(g::DY, 0);
    gm.set_g16(g::DX, 0);
    let d = code & 3;
    if d == 0 {
        gm.set_g16(g::DY, 0xffff);
    } else if d == 1 {
        gm.set_g16(g::DX, 1);
    } else if d == 2 {
        gm.set_g16(g::DY, 1);
    } else if d == 3 {
        gm.set_g16(g::DX, 0xffff);
    }
    slot
}

/// orig 1000:a34e (415 bytes). Interprets a move script at DGROUP offset `script`. Opcodes
/// < 0x10: 0 end; 1 pair mode (the next move code is followed by a second code for another
/// slot); 2 n: repeat count; 3/4 footsteps on/off; 5 n: animate_wait(n); 6 v x y:
/// g_light_map[y][x] = v; 7 refresh_map_view; 8 footsteps(count), count = 1; 9 s: clear the
/// tile and anim tile of slot s. A byte >= 0x10 is a move code (`decode_move_code`): the slot
/// (and in pair mode the second slot) is moved by (g_dx, g_dy) `count` times, with a footstep
/// per step when enabled. Count and pair mode reset after every move; the footstep flag persists.
pub fn run_move_script(gm: &mut Game, param_1: u16) {
    let mut p = param_1;
    let mut steps_on = true;
    loop {
        let mut count: u16 = 1;
        let mut pair = false;
        let code: u8;
        loop {
            let b = gm.g8(p);
            let next = p.wrapping_add(1);
            if b == 0 { return; }
            if 0xf < b {
                code = b;
                p = next;
                break;
            }
            let mut after = next;
            match b {
                1 => { pair = true; }
                2 => { count = gm.g8(next) as u16; after = p.wrapping_add(2); }
                3 => { steps_on = true; }
                4 => { steps_on = false; }
                5 => { util::animate_wait(gm, gm.g8(next) as i16); after = p.wrapping_add(2); }
                6 => {
                    let v = gm.g8(next);
                    let x = gm.g8(p.wrapping_add(2)) as u16;
                    let y = gm.g8(p.wrapping_add(3)) as u16;
                    gm.set_g8(g::LIGHT_MAP.wrapping_add(x).wrapping_add(y.wrapping_mul(0x20)), v);
                    after = p.wrapping_add(4);
                }
                7 => { rest::refresh_map_view(gm); }
                8 => { footsteps(gm, count as i16); count = 1; }
                9 => {
                    let s = gm.g8(next) as u16;
                    gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(s.wrapping_mul(8)), 0); // slot anim tile
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(s.wrapping_mul(8)), 0);
                    after = p.wrapping_add(2);
                }
                _ => {}
            }
            p = after;
        }
        let slot = decode_move_code(gm, code as u16);
        let dy = gm.g16(g::DY);
        let dx = gm.g16(g::DX);
        let mut slot2: u16 = 0;
        let mut dx2: u16 = 0;
        let mut dy2: u16 = 0;
        if pair {
            slot2 = decode_move_code(gm, gm.g8(p) as u16);
            dx2 = gm.g16(g::DX);
            dy2 = gm.g16(g::DY);
            p = p.wrapping_add(1);
        }
        if count != 0 {
            loop {
                let a = g::MONSTER_X.wrapping_add(slot.wrapping_mul(8));
                let v = gm.g8(a).wrapping_add(dx as u8);
                gm.set_g8(a, v);
                let a = g::MONSTER_Y.wrapping_add(slot.wrapping_mul(8));
                let v = gm.g8(a).wrapping_add(dy as u8);
                gm.set_g8(a, v);
                if pair {
                    let a = g::MONSTER_X.wrapping_add(slot2.wrapping_mul(8));
                    let v = gm.g8(a).wrapping_add(dx2 as u8);
                    gm.set_g8(a, v);
                    let a = g::MONSTER_Y.wrapping_add(slot2.wrapping_mul(8));
                    let v = gm.g8(a).wrapping_add(dy2 as u8);
                    gm.set_g8(a, v);
                }
                if steps_on {
                    footsteps(gm, 1);
                }
                count = count.wrapping_sub(1);
                if count == 0 { break; }
            }
        }
    }
}

/// orig 1000:a508 (84 bytes). Prints Blackthorn's question of round `round` (0..3) about
/// virtue `virtue`: rounds 0..2 print MISCMSG text 0xb21e / 0xb24b / 0xb270 followed by the
/// virtue name (g_virtue_mantra_ptrs[virtue]) and '?"'; round 3 prints 0xb2ca. Ghidra order:
/// (virtue, round).
pub fn print_interrogation_question(gm: &mut Game, param_1: i16, param_2: i16) {
    let (virtue, round) = (param_1, param_2);
    let s: u16;
    if round == 0 {
        text::print_string(gm, 0xb21e);
        text::print_string(gm, gm.g16((virtue as u16).wrapping_mul(2).wrapping_add(g::VIRTUE_MANTRA_PTRS)));
        s = 0x6f6e; // "?\""
    } else if round == 1 {
        text::print_string(gm, 0xb24b);
        text::print_string(gm, gm.g16((virtue as u16).wrapping_mul(2).wrapping_add(g::VIRTUE_MANTRA_PTRS)));
        s = 0x6f72; // "?\""
    } else if round == 2 {
        text::print_string(gm, 0xb270);
        text::print_string(gm, gm.g16((virtue as u16).wrapping_mul(2).wrapping_add(g::VIRTUE_MANTRA_PTRS)));
        s = 0x6f76; // "?\""
    } else {
        if round != 3 {
            nop_after_question(gm);
            return;
        }
        s = 0xb2ca;
    }
    text::print_string(gm, s);
    nop_after_question(gm);
}

/// orig 1000:a558 (24 bytes). Not a function: the round == 2 arm of
/// `print_interrogation_question`. Vector 1778:0452 targets OUTSUBS's a558, not this.
pub fn fn_1000_a558(gm: &mut Game) {}

/// orig 1000:a576 (4 bytes). Empty function (RET) called after each question.
pub fn nop_after_question(gm: &mut Game) {}

/// orig 1000:a57a (191 bytes). "Your response?" - reads up to 14 characters and returns 1 when
/// the mantra of `virtue` (g_virtue_mantra_ptrs[8 + virtue], DGROUP 0x1f5e) occurs anywhere in
/// the answer, compared case-insensitively (to_upper) at every start offset; 0 otherwise.
pub fn response_contains_mantra(gm: &mut Game, param_1: i16) -> i16 {
    let virtue = param_1;
    text::print_string(gm, 0x6f7a); // "\n\nYour response?\n:"
    gm.with_buf(18, |gm, buf| {
        rest::read_text_line(gm, 0xe, buf);
        text::print_string(gm, 0x6f8e); // "\n\n"
        let mantra = gm.g16((virtue as u16).wrapping_mul(2).wrapping_add(0x1f5e)); // mantra pointers
        let mlen = util::str_len(gm, gm.ds(mantra));
        let mut found: i16 = 0;
        let mut start: i16 = 0;
        let rlen = util::str_len(gm, gm.ds(buf));
        if mlen <= rlen {
            loop {
                found = 1;
                let mut j: i16 = 0;
                if 0 < mlen {
                    loop {
                        let a = util::to_upper(gm.g8(mantra.wrapping_add(j as u16)) as i16);
                        let b = util::to_upper(gm.g8(buf.wrapping_add((start + j) as u16)) as i16);
                        if b != a {
                            found = 0;
                            break;
                        }
                        j += 1;
                        if !(j < mlen) { break; }
                    }
                }
                if found != 0 {
                    return found;
                }
                start += 1;
                let rlen = util::str_len(gm, gm.ds(buf));
                if !(start <= rlen - mlen) { break; }
            }
        }
        found
    })
}

/// orig 1000:a63e (351 bytes). Executes a prisoner. Prints MISCMSG 0xb34b (`sliced` == 0) or
/// 0xb30f, animates, plays a rising then falling sweep, flashes the tile of monster slot 1
/// and clears its tiles, sets g_light_map[7][5] = 0x80. The victim is the second living member
/// (index of the second status != 'D'; the party count if there is none): its 32-byte record is
/// saved, records after it are shifted down by one, the copy is stored as record 15 with the
/// party byte (+0x1f) = 0x7f (permanently dead) and g_party_count is decremented; status redraw.
/// With `sliced` != 0: "<name> is sliced in half!", a key, then 0xb3be. Newline.
pub fn execute_prisoner(gm: &mut Game, param_1: i16) {
    let sliced = param_1;
    let s: u16 = if sliced == 0 { 0xb34b } else { 0xb30f };
    text::print_string(gm, s);
    util::animate_wait(gm, 10);
    let mut f: i16 = 2000;
    loop {
        util::play_sweep_noise(gm, 0, f as u16, 200, 1, 0xa50);
        f += 0x32;
        if !(f < 25000) { break; }
    }
    let mut f: i16 = 25000;
    loop {
        util::play_sweep_noise(gm, 0, f as u16, 200, 1, 0xa50);
        f -= 0x32;
        if !(2000 < f) { break; }
    }
    rest::flash_tile_at(gm, gm.g8(0x5c65) as i16, gm.g8(0x5c64) as i16); // slot 1 y, x
    gm.set_g8(0x5c63, 0); // slot 1 anim tile
    gm.set_g8(g::MONSTER_SLOT1, 0);
    gm.set_g8(0xadf9, 0x80); // g_light_map[7][5]
    let mut victim: u16 = 0;
    if gm.g8(g::PARTY_COUNT) != 0 {
        let mut p = g::PARTY_STATUS;
        victim = 0;
        let mut living: i16 = 0;
        loop {
            if gm.gi8(p) != 0x44 {
                living += 1;
            }
            if living == 2 { break; }
            p = p.wrapping_add(0x20);
            victim += 1;
            if !(victim < gm.g8(g::PARTY_COUNT) as u16) { break; }
        }
    }
    let rec = g::PARTY.wrapping_add(victim.wrapping_mul(0x20));
    let saved: Vec<u8> = gm.m.slice(gm.ds(rec), 0x20).to_vec();
    if (victim as i16) < 0xf {
        let mut dst = rec;
        let mut src = rec.wrapping_add(0x20);
        loop {
            for k in 0..0x20u16 {
                let v = gm.g8(src.wrapping_add(k));
                gm.set_g8(dst.wrapping_add(k), v);
            }
            dst = dst.wrapping_add(0x20);
            src = src.wrapping_add(0x20);
            if src == g::FOOD { break; }
        }
    }
    for k in 0..0x20u16 {
        gm.set_g8(0x5788u16.wrapping_add(k), saved[k as usize]); // party record 15
    }
    gm.set_g8(0x57a7, 0x7f); // record 15 +0x1f: permanently dead
    let c = gm.g8(g::PARTY_COUNT).wrapping_sub(1);
    gm.set_g8(g::PARTY_COUNT, c);
    rest::draw_status_window(gm);
    if sliced != 0 {
        text::print_string(gm, 0x6f92); // "\n\n"
        text::print_string(gm, 0x5788); // the victim's name (record 15)
        text::print_string(gm, 0x6f96); // " is sliced in half! "
        input::read_command_key(gm);
        text::print_string(gm, 0xb3be);
    }
    text::put_char(gm, 10);
}

/// orig 1000:a7a0 (11 bytes). Waits for a key and runs the "release the prisoners" move
/// script at 0x369e.
pub fn script_release(gm: &mut Game) {
    input::read_command_key(gm);
    run_move_script(gm, 0x369e);
}

/// orig 1000:a7ac (46 bytes). Blackthorn's threat: prints MISCMSG 0xb447, runs the move
/// script 0x36da (a companion is brought forward), prints 0xb47d, the name of party member 1
/// (0x55c8) and ' die!" ', waits for a key and prints a blank line.
pub fn script_threaten_companion(gm: &mut Game) {
    text::print_string(gm, 0xb447);
    run_move_script(gm, 0x36da);
    text::print_string(gm, 0xb47d);
    text::print_string(gm, 0x55c8); // party record 1 name
    text::print_string(gm, 0x6fac); // " die!\" "
    input::read_command_key(gm);
    text::print_string(gm, 0x6fb4); // "\n\n"
}

/// orig 1000:a7da (188 bytes). The interrogation about `virtue` with `living` living members.
/// Up to four rounds: the question is printed and the answer checked. A correct mantra sets
/// g_shrine_destroyed[virtue] = 0xff, costs 5 karma, with more than one living member runs
/// `execute_prisoner(0)` (the companion is killed anyway) else prints 0xb4ba, then
/// `script_release` and returns. A wrong answer with a single living member prints 0xb4fa and
/// releases. Otherwise the first refusal triggers `script_threaten_companion`; later refusals
/// pass 2 minutes, redraw the status and mutilate the companion: g_light_map[9][5] = 0xea
/// (round 0), 0xeb (round 1), 0xe8 (round 2), and `execute_prisoner(1)` at round 3. After four
/// rounds the function returns without release. Ghidra order: (living, virtue).
pub fn interrogation(gm: &mut Game, param_1: i16, param_2: i16) {
    let (living, virtue) = (param_1, param_2);
    let mut threatened = false;
    text::print_string(gm, 0x6fb8); // "\n\n"
    let mut round: i16 = 0;
    loop {
        print_interrogation_question(gm, virtue, round);
        if response_contains_mantra(gm, virtue) != 0 {
            gm.set_g8(g::SHRINE_DESTROYED.wrapping_add(virtue as u16), 0xff);
            rest::sub_u8_floor(gm, 5, g::KARMA);
            if 1 < living {
                execute_prisoner(gm, 0);
            } else {
                text::print_string(gm, 0xb4ba);
            }
            script_release(gm);
            return;
        }
        if living < 2 {
            text::print_string(gm, 0xb4fa);
            script_release(gm);
            return;
        }
        if threatened {
            rest::pass_time(gm, 2);
            rest::draw_status_window(gm);
            if round == 0 {
                gm.set_g8(0xae39, 0xea); // g_light_map[9][5]
            } else if round == 1 {
                gm.set_g8(0xae39, 0xeb);
            } else if round == 2 {
                gm.set_g8(0xae39, 0xe8);
            } else if round == 3 {
                execute_prisoner(gm, 1);
            }
        } else {
            threatened = true;
            script_threaten_companion(gm);
        }
        round += 1;
        if 3 < round {
            return;
        }
    }
}

/// orig 1000:a89e (769 bytes). Blackthorn's dungeon (vector 1778:033e, from TOWN guard_arrest
/// in Castle Blackthorn). Counts living members; transport on foot; "Thou art subdued and
/// blindfolded!". The first virtue with g_shrine_destroyed == 0 is chosen; if all eight are
/// betrayed the scene is skipped. Otherwise: blank map window, z = 0xff, sky/wind, five
/// footsteps, "Strong guards drag thee away!", 18 (3 without sound) more footsteps, monster
/// table cleared, g_location = g_active_combatant = 0xff, MISCMAPS.DAT (0xb0 bytes, the cell,
/// 11 rows x 16) read into g_view_objects and copied into g_light_map rows (11 bytes each),
/// MISCMSG.DAT (1000 bytes) into 0xb21e. Each party record i < living is placed in slot i at
/// the seat (x table 0x1f42, y table 0x1f48, index from 0x1f0a[living*8 + i]) with the class
/// tile 0x1ade[index of the class letter in "AMBFDTPRS"]. "Thou hast been chained and
/// manacled!", "Footsteps!", guards (0x70) in slots 6/7 enter by script 0x3702, Blackthorn
/// appears in slot 8 (tile 0x16 -> transition 0x178 -> tile 0x78): "Ah, <name>! 'Tis indeed an
/// honour...", "GUARD! Release this good lady/man at once!" (avatar gender 0x55b1: 0xc / 0xb),
/// script 0x370e, 0xb54a, then `interrogation(living, virtue)`; if slot 8 still holds a tile,
/// script 0x3716 (Blackthorn leaves). Finally z = 0xff, sky/wind, party at (10,7), keys = 0,
/// g_location = 0x12.
pub fn blackthorn_dungeon_scene(gm: &mut Game) {
    let mut living: i16 = 0;
    if gm.g8(g::PARTY_COUNT) != 0 {
        let mut p = g::PARTY_STATUS;
        living = 0;
        let mut i: u16 = 0;
        loop {
            if gm.gi8(p) != 0x44 {
                living += 1;
            }
            p = p.wrapping_add(0x20);
            i += 1;
            if i == gm.g8(g::PARTY_COUNT) as u16 { break; }
        }
    }
    gm.set_g8(g::TRANSPORT_TILE, 0x1c);
    rest::refresh_map_view(gm);
    text::print_string(gm, 0x6fbc); // "\nThou art subdued and blindfolded!"
    let mut virtue: i16 = 0;
    loop {
        if gm.g8(g::SHRINE_DESTROYED.wrapping_add(virtue as u16)) == 0 { break; }
        virtue += 1;
        if !(virtue < 8) { break; }
    }
    if !(7 < virtue) {
        util::animate_wait(gm, 2);
        gfx::set_color(gm, 0);
        gfx::fill_rect(gm, 0xb7, 0xb7, 8, 8);
        gm.set_g8(g::PARTY_Z, 0xff);
        rest::draw_sky_strip(gm);
        rest::set_wind_and_draw(gm, -1);
        let mut n: i16 = 5;
        loop {
            util::wait_ticks(gm, 5);
            rest::play_footstep_sound(gm);
            n -= 1;
            if n == 0 { break; }
        }
        text::print_string(gm, 0x6fe0); // "\n\nStrong guards drag thee away!"
        let mut i: i16 = 0;
        loop {
            let limit: i16 = if gm.gi8(g::SOUND_ENABLED) == 0 { 3 } else { 0x12 };
            if limit <= i { break; }
            util::wait_ticks(gm, 5);
            rest::play_footstep_sound(gm);
            i += 1;
        }
        let mut s: i16 = 0;
        loop {
            rest::set_monster_slot(gm, s, 0, 0, 0, 0, 0, 0);
            s += 1;
            if !(s < 0x20) { break; }
        }
        gm.set_g8(g::LOCATION, 0xff);
        gm.set_g8(g::ACTIVE_COMBATANT, 0xff);
        files::load_file_region(gm, 0, 0xb0, g::VIEW_OBJECTS, 0x7000); // "MISCMAPS.DAT"
        files::load_file_region(gm, 0, 1000, 0xb21e, 0x700e); // "MISCMSG.DAT"
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
        if 0 < living {
            let mut slot = g::MONSTER_TABLE;
            let mut cls = g::PARTY_CLASS;
            let mut i: i16 = 0;
            loop {
                let seat = gm.g8(((living as u16).wrapping_mul(8)).wrapping_add(i as u16).wrapping_add(0x1f0a)) as u16; // seat table
                let x = gm.g8(seat.wrapping_add(0x1f42)); // seat x
                gm.set_g8(slot.wrapping_add(2), x);
                let y = gm.g8(seat.wrapping_add(0x1f48)); // seat y
                gm.set_g8(slot.wrapping_add(3), y);
                let ci = rest::str_index_of(gm, gm.g8(cls) as u16, 0x701a); // "AMBFDTPRS"
                let tile = gm.g8((ci as u16).wrapping_add(0x1ade)); // class tiles
                gm.set_g8(slot.wrapping_add(1), tile);
                gm.set_g8(slot, tile);
                slot = slot.wrapping_add(8);
                cls = cls.wrapping_add(0x20);
                i += 1;
                if i == living { break; }
            }
        }
        util::animate_wait(gm, 0x10);
        text::print_string(gm, 0x7024); // "\n\nThou hast been chained and manacled!"
        util::animate_wait(gm, 0x32);
        text::print_string(gm, 0x704c); // "\n\nFootsteps!"
        footsteps(gm, 8);
        rest::set_monster_slot(gm, 6, 0, 0, 10, 4, 0x70, 0x70);
        rest::set_monster_slot(gm, 7, 0, 0, 10, 6, 0x70, 0x70);
        run_move_script(gm, 0x3702);
        util::play_sweep_noise(gm, 5, 100, 0x32c8, 1, 0xaf0);
        rest::set_monster_slot(gm, 8, 0, 0, 5, 5, 0x16, 0x16);
        gfx::tile_transition_effect(gm, 5, 5, 0x178);
        rest::set_monster_slot(gm, 8, 0, 0, 5, 5, 0x78, 0x78);
        util::animate_wait(gm, 8);
        text::print_string(gm, 0x705a); // "\n\nBlackthorn says:\n\n\"Ah, "
        text::print_string(gm, g::PARTY); // the Avatar's name
        text::print_string(gm, 0x7074); // "!\n'Tis indeed an honour to meet thee at last! "
        input::read_command_key(gm);
        text::print_string(gm, 0x70a4); // "\n\nGUARD! Release this good"
        if gm.gi8(0x55b1) == 0xc { // avatar gender
            text::print_string(gm, 0x70c0); // " lady "
        } else if gm.gi8(0x55b1) == 0xb {
            text::print_string(gm, 0x70c8); // "man "
        }
        text::print_string(gm, 0x70ce); // "at once!\""
        run_move_script(gm, 0x370e);
        text::print_string(gm, 0xb54a);
        input::read_command_key(gm);
        interrogation(gm, living, virtue);
        if gm.g8(0x5c9a) != 0 { // monster slot 8 tile
            run_move_script(gm, 0x3716);
        }
    }
    // LAB_1000_ab77
    gm.set_g8(g::PARTY_Z, 0xff);
    rest::draw_sky_strip(gm);
    rest::set_wind_and_draw(gm, -1);
    gm.set_g8(g::PARTY_X, 10);
    gm.set_g8(g::PARTY_Y, 7);
    gm.set_g8(g::KEYS, 0);
    gm.set_g8(g::LOCATION, 0x12);
}

/// orig 1000:aba0 (854 bytes). Resurrection by Lord British (vector 1778:02de, run by the
/// town/overworld loops when nobody is alive). Waits for disk 1 (BRIT.DAT present), ends the
/// light spell (CAST2 eaca), transport 0x1e, refresh if in a settlement, saves the old location
/// and sets g_location = g_active_combatant = 0xff. "An unending darkness engulfs thee..." with
/// the map window wiped to black through page 1 (rect_effect), g_light_map rows 0..10 set to
/// 0xff, all monster slots cleared. "Thou hast found refuge." / "No evil lives here..." / "But
/// thy slumber is disturbed!"; the party slot becomes a person (0x1c) at (5,5); six sweep
/// sounds from the word tables 0x3744/0x3738/0x372c/0x3720; "Someone shouts FORTIS FORTUNA
/// AVENTARI"; two sparkles (slot 1 tile 0x16 at (2,7) then (8,7), transition effects 0x5e/0x5f
/// written into g_light_map[7][2]/[7][8]); "There is a peal of thunder!", two quakes, Lord
/// British appears (slot 1: transition 0x174 at (5,2), tile 0x74). The KARMA.DAT paragraph
/// selected by g_karma/20 (offset table 0x1a74) is printed in quotes; "Strange words are
/// intoned."; for each member a sweep tone of 0x8e30/(i+7), CAST2 e7c0(0xff, i) (resurrect) and
/// HP = max HP. "Vertigo..."; the window is replaced through page 1 by tile 0x11c at (5,5);
/// karma is raised to at least 75; the party is put in Castle British (0x11) floor 1 at (10,10)
/// on foot, protection and status icon cleared, time passes in 9-minute steps until 06:00, light
/// and torch turns cleared, food at least 63, and if the old location was a settlement
/// `enter_town(1)` reloads it.
pub fn resurrection_scene(gm: &mut Game) {
    files::select_disk(gm, 1);
    loop {
        if files::file_exists(gm, 0x70d8) != 0 { break; } // "BRIT.DAT"
    }
    ovl::cast2::set_light_spell_turns(gm, gm.g8(g::LIGHT_SPELL_TURNS));
    gm.set_g8(g::TRANSPORT_TILE, 0x1e);
    if gm.g8(g::LOCATION) < 0x21 {
        rest::refresh_map_view(gm);
    }
    util::wait_ticks(gm, 10);
    let old_loc = gm.g8(g::LOCATION);
    gm.set_g8(g::LOCATION, 0xff);
    gm.set_g8(g::ACTIVE_COMBATANT, 0xff);
    text::print_string(gm, 0x70e2); // "\nAn unending darkness engulfs thee..."
    gfx::set_draw_page(gm, 1);
    gfx::set_color(gm, 0);
    gfx::fill_rect(gm, 0xb7, 0xb7, 8, 8);
    gfx::set_draw_page(gm, 0);
    gfx::rect_effect(gm, 0xb7, 0xb7, 8, 8);
    let mut row: u16 = 0;
    loop {
        for k in 0..11u16 {
            gm.set_g8(g::LIGHT_MAP.wrapping_add(row).wrapping_add(k), 0xff);
        }
        row = row.wrapping_add(0x20);
        if !(row < 0x160) { break; }
    }
    let mut s: u16 = 0;
    loop {
        rest::set_monster_slot(gm, s as i16, 0, 0, 0, 0, 0, 0);
        s += 1;
        if !(s < 0x20) { break; }
    }
    rest::refresh_map_view(gm);
    text::print_string(gm, 0x7108); // "\n\nThou hast found refuge."
    util::wait_ticks(gm, 0xe);
    text::print_string(gm, 0x7122); // "\n\nNo evil lives here, only peace and darkness."
    util::wait_ticks(gm, 0x1c);
    text::print_string(gm, 0x7152); // "\n\nBut thy slumber is disturbed!"
    gm.set_g8(g::COMBAT_MAP_CENTRE, 0);
    gm.set_g8(g::PARTY_VEHICLE_TILE, 0x1c);
    gm.set_g8(g::MONSTER_TABLE, 0x1c);
    gm.set_g8(g::MONSTER_Y, 5);
    gm.set_g8(g::MONSTER_X, 5);
    rest::refresh_map_view(gm);
    let mut i: u16 = 0;
    loop {
        let phase = gm.g16(0x3720u16.wrapping_add(i)); // sweep tables (6 entries each)
        let count = gm.g16(0x372cu16.wrapping_add(i));
        let threshold = gm.g16(0x3738u16.wrapping_add(i));
        let step = gm.g16(0x3744u16.wrapping_add(i));
        util::play_sweep_noise(gm, step, threshold, count, 1, phase);
        i += 2;
        if !(0x3744u16.wrapping_add(i) < 0x3750) { break; }
    }
    text::print_string(gm, 0x7172); // "\n\nSomeone shouts\n\n\"FORTIS FORTUNA\nAVENTARI\""
    util::wait_ticks(gm, 6);
    gm.set_g8(0x5c63, 0x16); // slot 1 anim tile
    gm.set_g8(g::MONSTER_SLOT1, 0x16);
    gm.set_g8(0x5c64, 2); // slot 1 x
    gm.set_g8(0x5c65, 7); // slot 1 y
    gm.set_g8(0xadf6, 0); // g_light_map[7][2]
    gfx::tile_transition_effect(gm, 7, 2, 0x5e);
    gm.set_g8(0xadf6, 0x5e);
    gm.set_g8(g::MONSTER_SLOT1, 0);
    rest::refresh_map_view(gm);
    util::wait_ticks(gm, 4);
    gm.set_g8(g::MONSTER_SLOT1, 0x16);
    gm.set_g8(0x5c64, 8);
    gm.set_g8(0xadfc, 0); // g_light_map[7][8]
    gfx::tile_transition_effect(gm, 7, 8, 0x5f);
    gm.set_g8(0xadfc, 0x5f);
    gm.set_g8(g::MONSTER_SLOT1, 0);
    rest::refresh_map_view(gm);
    util::wait_ticks(gm, 4);
    text::print_string(gm, 0x719e); // "\n\nThere is a peal of thunder!\n"
    rest::earthquake_shake(gm);
    rest::earthquake_shake(gm);
    gm.set_g8(0xad59, 0); // g_light_map[2][5]
    gm.set_g8(0x5c63, 0x16);
    gm.set_g8(g::MONSTER_SLOT1, 0x16);
    gm.set_g8(0x5c64, 5);
    gm.set_g8(0x5c65, 2);
    gfx::tile_transition_effect(gm, 2, 5, 0x174);
    gm.set_g8(0x5c63, 0x74);
    gm.set_g8(g::MONSTER_SLOT1, 0x74);
    rest::refresh_map_view(gm);
    let karma = gm.g8(g::KARMA) as u16;
    text::print_string(gm, 0x71be); // "\n\""
    let off = gm.g16((karma / 0x14).wrapping_mul(2).wrapping_add(0x1a74)); // KARMA.DAT paragraph offsets
    files::load_file_region(gm, off, 2000, 0xb21e, 0x71c2); // "KARMA.DAT"
    text::print_string(gm, 0xb21e);
    text::put_char(gm, 0x22);
    input::read_command_key(gm);
    text::print_string(gm, 0x71cc); // "\n\nStrange words are intoned."
    util::wait_ticks(gm, 4);
    if gm.g8(g::PARTY_COUNT) != 0 {
        let mut hp = g::PARTY_HP;
        let mut maxhp = g::PARTY_MAX_HP;
        let mut i: u16 = 0;
        loop {
            let f = rest::__aFuldiv(gm, 0x8e30, 0, i.wrapping_add(7), 0);
            util::play_sweep_noise(gm, 2, 2000, 0x7530, 1, f as u16);
            ovl::cast2::in_mani_corp_resurrect(gm, 0xff, i as i16);
            let v = gm.g16(maxhp);
            gm.set_g16(hp, v);
            rest::draw_status_window(gm);
            hp = hp.wrapping_add(0x20);
            maxhp = maxhp.wrapping_add(0x20);
            i += 1;
            if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
        }
    }
    text::print_string(gm, 0x71ea); // "\n\nVertigo...\n"
    util::wait_ticks(gm, 4);
    gfx::set_draw_page(gm, 1);
    gfx::set_color(gm, 0);
    gfx::fill_rect(gm, 0xb7, 0xb7, 8, 8);
    gfx::draw_tile(gm, 5, 5, 0x11c);
    gfx::set_draw_page(gm, 0);
    gfx::rect_effect(gm, 0xb7, 0xb7, 8, 8);
    if gm.g8(g::KARMA) < 0x4b {
        gm.set_g8(g::KARMA, 0x4b);
    }
    gm.set_g8(g::LOCATION, 0x11);
    gm.set_g8(g::PARTY_Z, 1);
    gm.set_g8(g::TRANSPORT_TILE, 0x1c);
    gm.set_g8(g::PARTY_Y, 10);
    gm.set_g8(g::PARTY_X, 10);
    gm.set_g8(g::PROTECTION_TURNS, 0);
    gm.set_g8(g::STATUS_ICON, 0);
    while gm.gi8(g::HOUR) != 6 {
        rest::pass_time(gm, 9);
    }
    gm.set_g8(g::LIGHT_SPELL_TURNS, 0);
    gm.set_g8(g::TORCH_TURNS, 0);
    if gm.g16(g::FOOD) == 0 {
        gm.set_g16(g::FOOD, 0x3f);
    }
    if old_loc != 0 && old_loc < 0x21 {
        ovl::town::enter_town(gm, 1);
    }
}
