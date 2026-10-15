//! MAINOUT.OVL (translated)
//!
//! The overworld (Britannia / Underworld) game loop: entering the outdoors (`run_overworld`,
//! vector 1778:02ba, called by ROOT main when g_location == 0), the per-turn loop
//! (`overworld_main_loop`), party movement by foot/horse/carpet/ship/skiff with wind-driven
//! sailing, the Attack and Enter commands outdoors (vectors 1778:042e / 03e6 from
//! dispatch_command), random encounters and overworld monster AI (`update_overworld_monsters`,
//! vector 1778:03f2, also run by hole_up_and_camp), ship cannon duels, whirlpools, waterfalls,
//! bridge trolls and the Shrine of the Codex gate. Map data lives in the four cached 16x16
//! chunks at g_map (0x6608) managed by OUTSUBS (`load_map_chunks` / `scroll_map_chunks`).
//!
//! Ghidra split three code fragments off as separate "functions" because overlay vectors of
//! other overlays point into the middle of MAINOUT code (1778:036e -> 8bb6, 1778:02c6 -> 93c0)
//! and because of a shared epilogue (8be4); those are translated as documented no-ops and
//! their code is part of `cmd_enter_outdoors` / `enemy_ship_fires`.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// Monster-table slot helpers (8-byte records at g_monster_table 0x5c5a: +0 tile, +1 anim tile,
/// +2 x, +3 y, +4 z, +5 value1 (hull / counter), +6 anim state, +7 value3).
#[inline] fn slot_off(slot: i16) -> u16 { (slot as u16).wrapping_mul(8) }

/// orig 1000:81d0 (122 bytes). Prepares the outdoor view when the party arrives on the
/// overworld/underworld: sky strip frame, pass_time(0), view dirty, prompt-at-turn-end and
/// map updates on, sailing off; computes the top-left cached chunk g_chunk_x/y so that the
/// party is in the inner 16x16 area (x & 0xf0, minus 0x10 if the low nibble < 8), loads all
/// four chunks (load_map_chunks(1,0) then (-1,0)), rebuilds the light map, draws the wind
/// line and sky strip and places the Underworld shards/amulet objects. Ghidra named it
/// TOWN_81d0 (TOWN shares the load address).
pub fn init_overworld(gm: &mut Game) {
    rest::draw_sky_strip_frame(gm);
    rest::pass_time(gm, 0);
    gm.set_g8(g::VIEW_DIRTY, 1);
    gm.set_g8(g::PROMPT_AT_TURN_END, 1);
    gm.set_g8(g::MAP_UPDATE_ENABLED, 1);
    gm.set_g8(g::SAIL_DIR, 0);
    let px = gm.g8(g::PARTY_X);
    gm.set_g8(g::CHUNK_X, px & 0xf0);
    if (px & 0xf) < 8 {
        let v = gm.g8(g::CHUNK_X).wrapping_sub(0x10);
        gm.set_g8(g::CHUNK_X, v);
    }
    let py = gm.g8(g::PARTY_Y);
    gm.set_g8(g::CHUNK_Y, py & 0xf0);
    if (py & 0xf) < 8 {
        let v = gm.g8(g::CHUNK_Y).wrapping_sub(0x10);
        gm.set_g8(g::CHUNK_Y, v);
    }
    ovl::outsubs::load_map_chunks(gm, 1, 0);
    ovl::outsubs::load_map_chunks(gm, -1, 0);
    rest::pass_time(gm, 0);
    rest::rebuild_light_map(gm);
    rest::set_wind_and_draw(gm, -1);
    rest::draw_sky_strip(gm);
    ovl::outsubs::place_shards_and_amulet(gm);
}

/// orig 1000:824a (96 bytes). If monster slot 1 holds an object on the party's level within
/// 5 tiles in both x and y, runs one animation frame (animate_wait(1)) so that the nearby
/// object keeps moving while the party is delayed (slow terrain, ship repairs).
pub fn animate_if_slot1_near(gm: &mut Game) {
    let px = gm.g8(g::PARTY_X) as u16;
    let py = gm.g8(g::PARTY_Y) as u16;
    let sx = gm.g8(0x5c64) as u16; // g_monster_table[1].x
    let sy = gm.g8(0x5c65) as u16; // g_monster_table[1].y
    let ddx = px.wrapping_sub(sx) as i16;
    let ddy = py.wrapping_sub(sy) as i16;
    let adx = (ddx ^ (ddx >> 15)).wrapping_sub(ddx >> 15);
    let ady = (ddy ^ (ddy >> 15)).wrapping_sub(ddy >> 15);
    if gm.gi8(g::MONSTER_SLOT1) != 0
        && gm.gi8(0x5c66) == gm.gi8(g::PARTY_Z) // g_monster_table[1].z
        && adx < 6
        && ady < 6
    {
        util::animate_wait(gm, 1);
    }
}

/// orig 1000:82aa (284 bytes). Prints the vehicle verb and turns the vehicle sprite to face
/// `dir` (0=N 1=E 2=S 3=W as a tile offset). Horse (0x10..0x13): "Ride ", facing E -> 0x12,
/// W -> 0x13; carpet (0x14..): "Fly ", E -> 0x14, W -> 0x15; skiff (0x28..): "Row ", tile =
/// 0x28 + dir; frigate sailing/anchored (0x20../0x24..): if already facing that way returns 1
/// only when sailing (tile < 0x24) with no wind (the ship cannot move), otherwise turns the
/// ship, prints "Head <North/East/South/West>", sets g_prompt_at_turn_end, warns "Hull weak!"
/// when the hull (g_ship_hull) < 50 and returns 1 (the turn is spent turning). Returns 0 for
/// every other case (the move proceeds).
pub fn turn_vehicle(gm: &mut Game, param_1: i16) -> u16 {
    let dir = param_1;
    let base = gm.g8(g::TRANSPORT_TILE) & 0xfc;
    if base == 0x10 {
        text::print_string(gm, 0x2946); // "Ride "
        if dir == 1 { gm.set_g8(g::TRANSPORT_TILE, 0x12); return 0; }
        if dir != 3 { return 0; }
        gm.set_g8(g::TRANSPORT_TILE, 0x13);
        return 0;
    }
    if base == 0x14 {
        text::print_string(gm, 0x294c); // "Fly "
        if dir == 1 { gm.set_g8(g::TRANSPORT_TILE, 0x14); return 0; }
        if dir != 3 { return 0; }
        gm.set_g8(g::TRANSPORT_TILE, 0x15);
        return 0;
    }
    if base != 0x20 && base != 0x24 {
        if base != 0x28 { return 0; }
        text::print_string(gm, 0x2951); // "Row "
        let v = (dir as u8).wrapping_add(gm.g8(g::TRANSPORT_TILE) & 0xfc);
        gm.set_g8(g::TRANSPORT_TILE, v);
        return 0;
    }
    let new_tile = (dir as u8).wrapping_add(gm.g8(g::TRANSPORT_TILE) & 0xfc);
    if gm.g8(g::TRANSPORT_TILE) == new_tile {
        if 0x23 < new_tile { gm.set_g8(g::TRANSPORT_TILE, new_tile); return 0; }
        if gm.gi8(g::WIND_DIR) != 0 { gm.set_g8(g::TRANSPORT_TILE, new_tile); return 0; }
        gm.set_g8(g::TRANSPORT_TILE, new_tile);
        return 1;
    }
    gm.set_g8(g::TRANSPORT_TILE, new_tile);
    text::print_string(gm, 0x2956); // "Head "
    let s: Option<u16> = match dir {
        0 => Some(0x295c), // "North\n"
        1 => Some(0x296a), // "East\n"
        2 => Some(0x2963), // "South\n"
        3 => Some(0x2970), // "West\n"
        _ => None,
    };
    if let Some(s) = s { text::print_string(gm, s); }
    // LAB_1000_837d
    gm.set_g8(g::PROMPT_AT_TURN_END, 1);
    if gm.g8(g::SHIP_HULL) < 0x32 {
        text::print_string(gm, 0x2976); // "Hull weak!\n"
    }
    1
}

/// orig 1000:83ce (340 bytes). Decides whether the party may step by (dy,dx). Prints
/// "Rowing!" in a skiff. An object at the destination blocks unless it can be boarded: from a
/// skiff (>= 0x28) an anchored frigate (0x24..0x27); on foot/horse/carpet any ship or skiff
/// (0x24..0x2b), the lighthouse object 0x1b or a horse (0x10/0x11). The destination's view tile
/// (g_view_tiles[5+dy][5+dx]) must also pass can_enter_tile for the current transport. Returns
/// 1 when the move is allowed. When blocked and not auto-sailing: silently returns 0 when a
/// vessel meets a whirlpool object (0xec..0xef), else prints "Blocked!" plus "OUCH!" and
/// damage_whole_party on tile 0x2f (cactus) or a beep, flushes the keyboard, returns 0. When
/// blocked while sailing: "BREAKING UP!" on tile 3, "COLLISION!" on anything but 'G' (0x47,
/// a dock): a dock prints "Docked!" and anchors the ship (tile + 4), otherwise random tones
/// and vehicle_damage; sailing stops (g_sail_dir = 0, prompt at turn end) and 0 is returned.
/// Ghidra order: (dy, dx).
pub fn can_party_move(gm: &mut Game, param_1: i16, param_2: i16) -> i16 {
    let (dy, dx) = (param_1, param_2);
    if (gm.g8(g::TRANSPORT_TILE) & 0xfc) == 0x24 {
        text::print_string(gm, 0x2982); // "Rowing!\n"
    }
    let mut boardable = true;
    let z = gm.g8(g::PARTY_Z) as u16;
    let ty = (dy as u16).wrapping_add(gm.g8(g::PARTY_Y) as u16);
    let tx = (dx as u16).wrapping_add(gm.g8(g::PARTY_X) as u16);
    let obj = rest::find_object_at(gm, z, ty, tx) as i16;
    let obj_b = obj as u8;
    if obj != 0 {
        boardable = false;
        let tt = gm.g8(g::TRANSPORT_TILE);
        if tt < 0x30 && 0x1f < tt {
            if 0x27 < tt && (0x23 < obj && obj < 0x28) {
                boardable = true;
            }
        } else if (0x23 < obj && obj < 0x2c) || obj == 0x1b || (obj_b & 0xfe) == 0x10 {
            boardable = true;
        }
    }
    // view tile at the destination: g_view_tiles[5+dy][5+dx] (0xaba7 = 0xab02 + 5*0x20 + 5)
    let vt = gm.gi8((dx as u16).wrapping_add((dy as u16).wrapping_mul(0x20)).wrapping_add(0xaba7));
    let ok: i16 = if boardable && rest::can_enter_tile(gm, vt as i16, gm.g8(g::TRANSPORT_TILE) as i16) != 0 { 1 } else { 0 };
    if ok != 0 { return ok; }
    if gm.g8(g::SAIL_DIR) == 0 {
        if 0x1f < gm.g8(g::TRANSPORT_TILE) && (obj_b & 0xfc) == 0xec {
            return 0;
        }
        text::print_string(gm, 0x29ae); // "Blocked!\n"
        if vt == 0x2f {
            text::print_string(gm, 0x29b8); // "OUCH!\n"
            rest::damage_whole_party(gm);
        } else {
            util::beep(gm, 200, 0xa5);
        }
        input::flush_keyboard(gm);
        return 0;
    }
    if vt == 3 {
        text::print_string(gm, 0x298b); // "BREAKING UP!\n"
    } else if vt != 0x47 {
        text::print_string(gm, 0x2999); // "COLLISION!\n"
    }
    // LAB_1000_84af
    if vt == 0x47 {
        text::print_string(gm, 0x29a5); // "Docked!\n"
        let v = gm.g8(g::TRANSPORT_TILE).wrapping_add(4);
        gm.set_g8(g::TRANSPORT_TILE, v);
    } else {
        util::play_random_tones(gm, 300, 2000, 100);
        vehicle_damage(gm);
    }
    gm.set_g8(g::PROMPT_AT_TURN_END, 1);
    gm.set_g8(g::SAIL_DIR, 0);
    0
}

/// orig 1000:8524 (140 bytes). Moves the party by (dy,dx) (8-bit wrapping coordinates) and
/// marks the view dirty. If the party leaves the inner area of the cached 2x2 chunk window
/// (its offset from g_chunk_x/y, masked to 0..31, is < 5 or > 26 in either axis) the window
/// scrolls: scroll_map_chunks(dy,dx) shifts the kept chunks, g_chunk_x/y advance by 16 in the
/// direction moved (masked to 0xf0), load_map_chunks(dy,dx) fetches the new row/column and the
/// light map is rebuilt. Also reached through vector 1778:0446 (OUTSUBS go_over_falls, CMDS).
/// Ghidra order: (dy, dx).
pub fn move_party_by(gm: &mut Game, param_1: u16, param_2: u16) {
    let (dy, dx) = (param_1, param_2);
    let v = gm.g8(g::PARTY_X).wrapping_add(dx as u8);
    gm.set_g8(g::PARTY_X, v);
    let v = gm.g8(g::PARTY_Y).wrapping_add(dy as u8);
    gm.set_g8(g::PARTY_Y, v);
    gm.set_g8(g::VIEW_DIRTY, 1);
    let rx = gm.g8(g::PARTY_X).wrapping_sub(gm.g8(g::CHUNK_X)) & 0x1f;
    let ry = gm.g8(g::PARTY_Y).wrapping_sub(gm.g8(g::CHUNK_Y)) & 0x1f;
    if rx < 5 || 0x1a < rx || ry < 5 || 0x1a < ry {
        ovl::outsubs::scroll_map_chunks(gm, dy as i16, dx as i16);
        let v = (dx as u8).wrapping_mul(0x10).wrapping_add(gm.g8(g::CHUNK_X)) & 0xf0;
        gm.set_g8(g::CHUNK_X, v);
        let v = (dy as u8).wrapping_mul(0x10).wrapping_add(gm.g8(g::CHUNK_Y)) & 0xf0;
        gm.set_g8(g::CHUNK_Y, v);
        ovl::outsubs::load_map_chunks(gm, dy as i16, dx as i16);
        rest::rebuild_light_map(gm);
    }
}

/// orig 1000:85b0 (173 bytes). Terrain speed penalty after a step. The tile under the party
/// classifies as: 0 (no delay) for tile 5, tiles < 4 or > 0xf; 1 (slow) for 4..8 and 0x1e/0x1f;
/// 2 (very slow) for 9..0xf. Slow: one extra monster update (update_overworld_monsters +
/// animate_if_slot1_near), "Slow progress!" if no monster acted, pass_time(2). Very slow: two
/// extra updates, "Very slow!" if neither acted, pass_time(4).
pub fn apply_terrain_delay(gm: &mut Game) {
    let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
    let t = gm.g8(p);
    let class: i16;
    if t == 5 {
        class = 0;
    } else if t != 0x1e && t != 0x1f {
        if t < 4 || 0xf < t {
            class = 0;
        } else if 8 < t {
            class = 2;
        } else {
            class = 1;
        }
    } else {
        class = 1;
    }
    // LAB_1000_85ef
    let minutes: i16;
    if class == 1 {
        let r = update_overworld_monsters(gm);
        animate_if_slot1_near(gm);
        if r == 0 {
            text::print_string(gm, 0x29bf); // "Slow progress!\n"
        }
        minutes = 2;
    } else {
        if class != 2 { return; }
        let r1 = update_overworld_monsters(gm);
        animate_if_slot1_near(gm);
        let r2 = update_overworld_monsters(gm);
        animate_if_slot1_near(gm);
        if r1.wrapping_add(r2) == 0 {
            text::print_string(gm, 0x29cf); // "Very slow!\n"
        }
        minutes = 4;
    }
    rest::pass_time(gm, minutes);
}

/// orig 1000:8660 (261 bytes). Handles a direction key (1=W 2=E 3=N 4=S) outdoors. On a
/// sailing frigate (0x20..0x23) a new direction becomes g_sail_dir (wind timer reset) and the
/// turn-end prompt is suppressed. Footsteps sound on foot (0x1c) or a moving horse (0x12/0x13).
/// Calls turn_vehicle with the facing (W->3, E->1, N->0, S->2); if that consumed the turn its
/// result is returned. Prints the direction name unless auto-sailing, then can_party_move(dy,dx);
/// if allowed and `dark` (the darkness flag from the main loop) is 0 the party actually moves
/// (move_party_by + apply_terrain_delay, with a footstep sound on a horse). Returns the
/// can_party_move result (1 = the turn is spent). Ghidra order: (dark, dir).
pub fn handle_move_key(gm: &mut Game, param_1: i16, param_2: u16) -> i16 {
    let (dark, dir) = (param_1, param_2);
    if (gm.g8(g::TRANSPORT_TILE) & 0xfc) == 0x20 {
        if dir != gm.g8(g::SAIL_DIR) as u16 {
            gm.set_g8(g::SAIL_DIR, dir as u8);
            gm.set_g8(g::WIND_TIMER, 0);
        }
        gm.set_g8(g::PROMPT_AT_TURN_END, 0);
    }
    let tt = gm.g8(g::TRANSPORT_TILE);
    if tt == 0x1c || (tt & 0xfe) == 0x12 {
        rest::play_footstep_sound(gm);
    }
    let mut dy: u16 = 0;
    let mut dx: u16 = 0;
    let mut name: Option<u16> = None;
    match dir {
        1 => {
            dx = 0xffff;
            let r = turn_vehicle(gm, 3);
            if r != 0 { return r as i16; }
            if gm.g8(g::SAIL_DIR) == 0 { name = Some(0x29ef); } // "West\n"
        }
        2 => {
            dx = 1;
            let r = turn_vehicle(gm, 1);
            if r != 0 { return r as i16; }
            if gm.g8(g::SAIL_DIR) == 0 { name = Some(0x29e9); } // "East\n"
        }
        3 => {
            dy = 0xffff;
            let r = turn_vehicle(gm, 0);
            if r != 0 { return r as i16; }
            if gm.g8(g::SAIL_DIR) == 0 { name = Some(0x29db); } // "North\n"
        }
        4 => {
            dy = 1;
            let r = turn_vehicle(gm, 2);
            if r != 0 { return r as i16; }
            if gm.g8(g::SAIL_DIR) == 0 { name = Some(0x29e2); } // "South\n"
        }
        _ => {}
    }
    if let Some(s) = name { text::print_string(gm, s); }
    // LAB_1000_86de
    let r = can_party_move(gm, dy as i16, dx as i16);
    if dark == 0 && r != 0 {
        if (gm.g8(g::TRANSPORT_TILE) & 0xfe) == 0x12 {
            rest::play_footstep_sound(gm);
        }
        move_party_by(gm, dy, dx);
        apply_terrain_delay(gm);
    }
    r
}

/// orig 1000:8768 (333 bytes). Reads the next overworld command. Refreshes the view and the
/// status window if dirty. If the view tile directly south of the party (g_view_tiles[6][5],
/// 0xabc7) is a waterfall (0xd4..0xd7) the party goes over the falls (OUTSUBS go_over_falls)
/// and 0 is returned. Otherwise prints the "\n>" prompt when g_prompt_at_turn_end is set
/// (always re-armed). While auto-sailing (g_sail_dir != 0): polls the keyboard; any key other
/// than the current sail direction ends sailing and is returned; else the ship advances
/// automatically: with wind, the turns between steps depend on how the sail direction matches
/// the wind delta tables (0x29f5 dx / 0x29f9 dy by wind direction; 1 = with the wind, 2/3 =
/// across/against, taken mod 3 and compared with g_wind_timer) - when the timer expires the
/// sail direction itself is returned as the command; each waiting turn passes 2 minutes (1 with
/// HMS Cape plans >= 0x80, using the word at g_bss_start as an alternate-turn toggle so the
/// monster update runs every other step), runs update_overworld_monsters, animate_wait(1) and
/// bumps g_wind_timer. When not sailing the command comes from read_command_key.
pub fn read_overworld_command(gm: &mut Game) -> u16 {
    view::refresh_map_view(gm);
    if gm.g8(g::STATUS_DIRTY) != 0 {
        rest::draw_status_window(gm);
        gm.set_g8(g::STATUS_DIRTY, 0);
    }
    let mut key: u16;
    if (gm.g8(0xabc7) & 0xfc) == 0xd4 { // g_view_tiles[6][5]: tile south of the party
        ovl::outsubs::go_over_falls(gm);
        key = 0;
    } else {
        if gm.g8(g::PROMPT_AT_TURN_END) != 0 {
            text::put_char(gm, 10);
            rest::draw_bar_left_cap(gm);
        }
        gm.set_g8(g::PROMPT_AT_TURN_END, 1);
        key = 0;
        while gm.g8(g::SAIL_DIR) != 0 {
            let k = input::poll_key_with_cursor(gm);
            key = util::to_upper(k) as u16;
            if key != 0 && key != gm.g8(g::SAIL_DIR) as u16 { break; }
            key = gm.g8(g::SAIL_DIR) as u16;
            if gm.g8(g::WIND_DIR) != 0 {
                let mut cdy: i8 = 0;
                let mut cdx: i8 = 0;
                if key == 1 { cdx = -1; }
                else if key == 2 { cdx = 1; }
                else if key == 3 { cdy = -1; }
                else if key == 4 { cdy = 1; }
                let wd = gm.g8(g::WIND_DIR) as u16;
                let mut n: u16 = 1;
                if gm.gi8(0x29f5u16.wrapping_add(wd)) != cdx { n = 2; } // g_unk_29f5: wind dx table
                if gm.gi8(0x29f9u16.wrapping_add(wd)) != cdy { n = n.wrapping_add(1); } // wind dy table
                if n % 3 <= gm.g8(g::WIND_TIMER) as u16 {
                    gm.set_g8(g::WIND_TIMER, 0);
                    break;
                }
            }
            let minutes: i16;
            if gm.g8(g::HMS_CAPE_PLANS) < 0x80 {
                minutes = 2;
            } else {
                let t = gm.g16(g::BSS_START);
                gm.set_g16(g::BSS_START, (t == 0) as u16); // alternate-turn toggle (BSS word)
                minutes = 1;
            }
            rest::pass_time(gm, minutes);
            if gm.g16(g::BSS_START) == 0 || gm.g8(g::HMS_CAPE_PLANS) < 0x80 {
                update_overworld_monsters(gm);
            }
            util::animate_wait(gm, 1);
            let v = gm.g8(g::WIND_TIMER).wrapping_add(1);
            gm.set_g8(g::WIND_TIMER, v);
            if gm.pf.quit_requested { break; }
        }
        if gm.g8(g::SAIL_DIR) == 0 {
            key = input::read_command_key(gm) as u16;
        }
    }
    key
}

/// orig 1000:88bc (163 bytes). The Attack command outdoors (vector 1778:042e from
/// dispatch_command when g_location == 0). Prints "Attack-"; over water (tile < 4) in a skiff
/// (0x28..) or on a carpet (0x14/0x15) answers "On foot!". Otherwise asks a direction; the
/// object at the adjacent cell (find_object_at, which leaves its slot in g_dx) is attacked
/// with fight_monster when its group (tile & 0xfc) is a ship 0x2c or any tile >= 0x40 except
/// 0xb4 (shard) and 0xe8; else "Nothing to attack!". Always returns 0 (no time passes).
pub fn cmd_attack_outdoors(gm: &mut Game) -> u16 {
    text::print_string(gm, 0x29fe); // "Attack-"
    let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
    let tt = gm.g8(g::TRANSPORT_TILE);
    if gm.g8(p) < 4 && ((tt & 0xfc) == 0x28 || (tt & 0xfe) == 0x14) {
        text::print_string(gm, 0x2a06); // "On foot!\n"
    } else {
        let d = rest::get_direction(gm);
        if d != 0 {
            let y = (gm.g8(g::PARTY_Y) as u16).wrapping_add(gm.g16(g::DY));
            let x = (gm.g8(g::PARTY_X) as u16).wrapping_add(gm.g16(g::DX));
            let obj = rest::find_object_at(gm, gm.g8(g::PARTY_Z) as u16, y, x) as u16 & 0xfc;
            if obj == 0x2c || (obj != 0xb4 && obj != 0xe8 && 0x3f < obj) {
                let slot = gm.gi16(g::DX);
                rest::fight_monster(gm, slot);
            } else {
                text::print_string(gm, 0x2a10); // "Nothing to attack!\n"
            }
        }
    }
    0
}

/// orig 1000:8960 (330 bytes). Enters a dungeon/cave/mine: prints the type name `name`, looks
/// the party position up in g_location_x/y entries 0x20..0x27 (locations 0x21..0x28). On foot
/// only ("On foot!" otherwise). Entry 0x27 (Doom): if no Shadowlord has been destroyed yet
/// (all three g_shadowlord_loc bytes < 0x80) prints "Attacked at entrance!", spawns a tile 0xfc
/// monster in a free slot and fights it instead. Other dungeons print "\n\n" + the inverse-video
/// location name. Then: disk 1 (BRIT.DAT present), the monster table is saved to BRIT.OOL/
/// UNDER.OOL, disk 2, the 512-byte dungeon map (DUNGEON.DAT offset index*0x200 - 0x4000) is
/// loaded to g_dungeon_map, g_location = index+1 and the start position is set: from the
/// surface (or for Doom) level 0 at (1,1) facing 1 (g_dungeon_dir_copy 5); from the Underworld
/// level 7 at (7,7) facing 3 (dir copy 4). "What dungeon?" if not on an entrance. Returns 1.
pub fn enter_dungeon(gm: &mut Game, param_1: u16) -> u16 {
    text::print_string(gm, param_1);
    let mut i: u16 = 0x20;
    loop {
        if gm.g8(g::LOCATION_X.wrapping_add(i)) == gm.g8(g::PARTY_X)
            && gm.g8(g::LOCATION_Y.wrapping_add(i)) == gm.g8(g::PARTY_Y) { break; }
        i = i.wrapping_add(1);
        if !(i < 0x28) { break; }
    }
    if i < 0x28 {
        if gm.g8(g::TRANSPORT_TILE) != 0x1c {
            text::print_string(gm, 0x2a24); // "\nOn foot!\n"
            return 1; // TODO(verify): the original returns print_string's AX (unspecified)
        }
        if i == 0x27 {
            if (gm.g8(g::SHADOWLORD_LOC) & gm.g8(0x58c9) & gm.g8(0x58ca)) < 0x80 { // g_shadowlord_loc[0..3]
                text::print_string(gm, 0x2a2f); // "\nAttacked at entrance!\n"
                let slot = rest::find_spawn_slot(gm);
                gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot_off(slot)), 0xfc);
                rest::fight_monster(gm, slot);
                return 1; // TODO(verify): the original returns fight_monster's AX (unspecified)
            }
        } else {
            text::print_string(gm, 0x2a47); // "\n\n"
            text::put_char(gm, 0xfc);
            let nm = gm.g16(g::LOCATION_NAME_PTRS.wrapping_add(i.wrapping_mul(2)));
            text::print_string(gm, nm);
            text::put_char(gm, 0xfb);
        }
        text::put_char(gm, 10);
        if gm.g8(g::CUR_DISK) != 1 {
            files::select_disk(gm, 1);
            loop {
                if files::file_exists(gm, g::STR_BRIT_DAT3) != 0 { break; } // "BRIT.DAT"
            }
        }
        let name = ovl::outsubs::ool_file_name(gm);
        files::save_file_region(gm, 0x100, g::MONSTER_TABLE, name);
        files::select_disk(gm, 2);
        files::load_file_region(gm, i.wrapping_mul(0x200).wrapping_sub(0x4000), 0x200, g::DUNGEON_MAP, g::STR_DUNGEON_DAT);
        gm.set_g8(g::LOCATION, (i as u8).wrapping_add(1));
        if gm.g8(g::PARTY_Z) == 0 || gm.g8(g::LOCATION) == 0x28 {
            gm.set_g8(g::PARTY_Z, 0);
            gm.set_g8(g::PARTY_Y, 1);
            gm.set_g8(g::PARTY_X, 1);
            gm.set_g8(g::DUNGEON_FACING, 1);
            gm.set_g8(g::DUNGEON_DIR_COPY, 5);
        } else {
            gm.set_g8(g::PARTY_Z, 7);
            gm.set_g8(g::DUNGEON_FACING, 3);
            gm.set_g8(g::DUNGEON_DIR_COPY, 4);
            gm.set_g8(g::PARTY_Y, 7);
            gm.set_g8(g::PARTY_X, 7);
        }
    } else {
        text::print_string(gm, 0x2a5f); // "\nWhat dungeon?\n"
    }
    1
}

/// orig 1000:8aae (260 bytes; the real function runs to the epilogue at 8be8, Ghidra split
/// the tail off as FUN_8bb6/FUN_8be4). The Enter command outdoors (vector 1778:03e6 from
/// dispatch_command). Prints "Enter " and dispatches on the map tile under the party:
/// 0x10 hut, 0x12 keep, 0x13 village, 0x14 towne, 0x15 castle, 0x1b lighthouse, 0x39 "the
/// palace of Blackthorn!", 0x3e "the Castle of Lord British!" -> OUTSUBS enter_settlement(name)
/// (its result, 0 = entered, is returned); 0x16 cave, 0x17 mine, 0x18 dungeon -> enter_dungeon
/// (result returned); 0x11 "the Shrine of the Codex!" -> CAST2_f056; 0x19 "the shrine of" +
/// the mantra-table entry of the shrine whose g_shrine_x/y matches (index 8 if none) ->
/// CAST2_f056; 0x1a prints "ruins"; anything else "What?" and returns 0. Returns 1 otherwise.
pub fn cmd_enter_outdoors(gm: &mut Game) -> u16 {
    let mut result: u16 = 1;
    text::print_string(gm, 0x2a6f); // "Enter "
    let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
    let tile = gm.g8(p) as u16;
    let mut town_name: Option<u16> = None;
    let mut dungeon_name: Option<u16> = None;
    let mut what = false;
    match tile {
        0x16 => dungeon_name = Some(0x2abd), // "cave"
        0x10 => town_name = Some(0x2a85),    // "hut"
        0x11 => {
            text::print_string(gm, 0x2a89); // "the Shrine of the Codex!\n"
            ovl::cast2::visit_shrine(gm);
        }
        0x12 => town_name = Some(0x2aa3), // "keep"
        0x13 => town_name = Some(0x2aa8), // "village"
        0x14 => town_name = Some(0x2ab0), // "towne"
        0x15 => town_name = Some(0x2ab6), // "castle"
        0x19 => {
            text::print_string(gm, 0x2a76); // "the shrine of\n"
            let mut i: u16 = 0;
            loop {
                if gm.g8(g::SHRINE_X.wrapping_add(i)) == gm.g8(g::PARTY_X)
                    && gm.g8(g::SHRINE_Y.wrapping_add(i)) == gm.g8(g::PARTY_Y) { break; }
                i = i.wrapping_add(1);
                if !(i < 8) { break; }
            }
            let s = gm.g16(g::VIRTUE_MANTRA_PTRS.wrapping_add(i.wrapping_mul(2)));
            text::print_string(gm, s);
            text::put_char(gm, 10);
            ovl::cast2::visit_shrine(gm);
        }
        0x17 => dungeon_name = Some(0x2ac2), // "mine"
        0x18 => dungeon_name = Some(0x2ac7), // "dungeon"
        0x1a => text::print_string(gm, 0x2acf), // "ruins"
        0x1b => town_name = Some(0x2ad5), // "lighthouse"
        0x39 => town_name = Some(0x2ae0), // "the palace of Blackthorn!"
        0x3e => town_name = Some(0x2afa), // "the Castle of Lord British!"
        _ => what = true,
    }
    if what {
        text::print_string(gm, 0x2b16); // "What?\n"
        result = 0;
    } else if let Some(nm) = town_name {
        result = ovl::outsubs::enter_settlement(gm, nm);
    } else if let Some(nm) = dungeon_name {
        result = enter_dungeon(gm, nm);
    }
    result
}

/// orig 1000:8bb6 (23 bytes). Not a function: Ghidra split this off because vector 1778:036e
/// (vec_TOWN_8bb6) points here when TOWN is loaded. In MAINOUT the bytes are the tail of the
/// tile switch of `cmd_enter_outdoors` (0x17/0x18 -> enter_dungeon, else "What?"). No-op.
pub fn enter_outdoors_fragment_8bb6(gm: &mut Game) {}

/// orig 1000:8be4 (5 bytes). The shared epilogue of `cmd_enter_outdoors` (POP SI; MOV SP,BP;
/// POP BP; RET), split off by Ghidra. No-op.
pub fn enter_outdoors_epilogue_8be4(gm: &mut Game) {}

/// orig 1000:8bea (69 bytes). Darkness check run every turn: if the map tile under the party is
/// 0xff (the "unending darkness" of the Underworld void) and the status icon is not 0x0e, the
/// light level is forced to 0 and, on the first such turn (`flag` == 0), the view is redrawn
/// and 1 is returned (the flag stays 1 while in darkness: moves are then not executed by
/// handle_move_key). Otherwise returns 0 after pass_time(0).
pub fn check_darkness_tile(gm: &mut Game, param_1: i16) -> i16 {
    let mut flag = param_1;
    let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
    if gm.gi8(p) == -1 && gm.g8(g::STATUS_ICON) != 0x0e {
        gm.set_g8(g::LIGHT_LEVEL, 0);
        if flag == 0 {
            view::refresh_map_view(gm);
            flag = 1;
        }
    } else {
        flag = 0;
        rest::pass_time(gm, 0);
    }
    flag
}

/// orig 1000:8c30 (36 bytes). In the Underworld (g_party_z != 0) a 1/256 chance per turn
/// (rand_range(0,255) == 0x69) of "EARTHQUAKE!": screen shake and damage_whole_party.
pub fn underworld_earthquake_check(gm: &mut Game) {
    if gm.g8(g::PARTY_Z) != 0 {
        let r = util::rand_range(gm, 0xff, 0);
        if r == 0x69 {
            text::print_string(gm, 0x2b1d); // "EARTHQUAKE!\n"
            rest::earthquake_shake(gm);
            rest::damage_whole_party(gm);
        }
    }
}

/// orig 1000:8c54 (611 bytes). The overworld turn loop. Each iteration: check_darkness_tile;
/// find_first_conscious_member: 1 (everyone asleep) prints "Zzzzzz..." and lets the turn pass,
/// -1 (everyone dead) saves the monster table (disk 1) and runs the BLCKTHRN death/resurrection
/// scene (vector 1778:02de) then returns. enter_moongate (moongate under the party) runs
/// CAST2_f056 and the loop returns if g_location changed. Then read_overworld_command: keys
/// >= 0x20 go to dispatch_command (digits '0'..'9' to set_active_player); 1..4 move
/// (handle_move_key with the darkness flag); 5 asks "Exit to DOS?" (Y: restore_video_mode,
/// exit(0); else prints "N"); 0xb (^K) prints the karma value; 0x13 (^S) toggles sound with
/// "Sound On/Off"; 0x16 (^V) prints the version "1.16"; 0 does nothing; others "What?" (and
/// the turn still passes). After the command: g_sail_dir is cleared unless on a sailing ship;
/// if still outdoors and the command consumed time: pass_time(2), then the tile under the
/// party triggers bridge trolls (0x6a/0x6b), swamp poison on foot (tile 4 + OUTSUBS
/// poison_in_swamp + animate_wait), lava burning (0x8f + OUTSUBS burn_on_lava) or, at
/// (0xe9,0xeb) on the surface, the Shrine of the Codex gate ("Pass, Seeker!" when
/// g_shrine_quests != 0, else "Thou art not upon a Sacred Quest! Passage denied!" and the party
/// is pushed back south by one). Then underworld_earthquake_check, end_of_turn_party_update,
/// "Rough seas!" (deep water tile 1 in a skiff or on a carpet: flash + vehicle_damage), a
/// waterfall under the party (0xd4..0xd7 -> OUTSUBS go_over_falls) and update_overworld_monsters.
/// Returns when g_location becomes nonzero (town/dungeon entered, moongate) or after death.
pub fn overworld_main_loop(gm: &mut Game) {
    let mut dark: i16 = 0;
    loop {
        let mut done = false;
        let mut time_passes: i16 = 1;
        dark = check_darkness_tile(gm, dark);
        let r = rest::find_first_conscious_member(gm) as i16;
        let mut msg: Option<u16> = None;
        if r == 1 {
            text::put_char(gm, 10);
            rest::draw_bar_left_cap(gm);
            msg = Some(0x2b2a); // "Zzzzzz...\n"
        } else if r == -1 {
            if gm.g8(g::CUR_DISK) != 1 {
                files::select_disk(gm, 1);
                loop {
                    if files::file_exists(gm, 0x2b35) != 0 { break; } // "BRIT.DAT"
                }
            }
            let name = ovl::outsubs::ool_file_name(gm);
            files::save_file_region(gm, 0x100, g::MONSTER_TABLE, name);
            ovl::blckthrn::resurrection_scene(gm);
            done = true;
            time_passes = 0;
        } else {
            if rest::enter_moongate(gm) != 0 {
                ovl::cast2::visit_shrine(gm);
            }
            if gm.g8(g::LOCATION) != 0 { return; }
            let key = read_overworld_command(gm) as u8;
            if 0x1f < key {
                if key < 0x30 || 0x39 < key {
                    time_passes = rest::dispatch_command(gm, key as i16) as i16;
                } else {
                    time_passes = rest::set_active_player(gm, key as i16) as i16;
                }
            } else {
                match key {
                    0 => { time_passes = 0; }
                    1 | 2 | 3 | 4 => { time_passes = handle_move_key(gm, dark, key as u16); }
                    5 => {
                        text::print_string(gm, 0x2b3e); // "Exit to DOS? "
                        let c = input::read_command_key(gm) as u8;
                        if c != b'Y' {
                            msg = Some(0x2b4c); // "N\n"
                        } else {
                            gfx::restore_video_mode(gm);
                            rest::exit(gm, 0, 0);
                        }
                    }
                    0xb => {
                        let k = gm.g8(g::KARMA) as i16;
                        text::print_number(gm, 0x20, 1, k);
                        text::put_char(gm, 10);
                        time_passes = 0;
                    }
                    0x13 => {
                        text::print_string(gm, 0x2b54); // "Sound "
                        let s = if gm.g8(g::SOUND_ENABLED) == 0 { 0x2b60 } else { 0x2b5b }; // "On\n" / "Off\n"
                        text::print_string(gm, s);
                        let v = (gm.g8(g::SOUND_ENABLED) == 0) as u8;
                        gm.set_g8(g::SOUND_ENABLED, v);
                    }
                    0x16 => {
                        text::print_string(gm, 0x2b4f); // "1.16"
                        text::put_char(gm, 10);
                        time_passes = 0;
                    }
                    _ => { msg = Some(0x2b64); } // "What?\n"
                }
            }
        }
        if let Some(s) = msg { text::print_string(gm, s); }
        // LAB_1000_8de2
        if (gm.g8(g::TRANSPORT_TILE) & 0xfc) != 0x20 {
            gm.set_g8(g::SAIL_DIR, 0);
        }
        if gm.g8(g::LOCATION) == 0 {
            if time_passes != 0 {
                rest::pass_time(gm, 2);
                let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
                let tile = gm.g8(p) as u16;
                if (gm.g8(p) & 0xfe) == 0x6a {
                    bridge_troll_ambush(gm);
                } else if tile == 4 && gm.g8(g::TRANSPORT_TILE) == 0x1c {
                    ovl::outsubs::poison_in_swamp(gm);
                    util::animate_wait(gm, 1);
                } else if tile == 0x8f {
                    ovl::outsubs::burn_on_lava(gm);
                } else if gm.g8(g::PARTY_X) == 0xe9 && gm.g8(g::PARTY_Y) == 0xeb
                    && gm.g8(g::PARTY_Z) == 0 && gm.g8(g::LOCATION) == 0
                {
                    text::print_string(gm, 0x2b6b); // "\n\""
                    if gm.g8(g::SHRINE_QUESTS) == 0 {
                        text::print_string(gm, 0x2b7e); // "Thou art not upon a Sacred Quest!\n"
                        text::print_string(gm, 0x2ba1); // "Passage denied!\"\n"
                        let v = gm.g8(g::PARTY_Y).wrapping_add(1);
                        gm.set_g8(g::PARTY_Y, v);
                    } else {
                        text::print_string(gm, 0x2b6e); // "Pass, Seeker!\"\n"
                    }
                }
                underworld_earthquake_check(gm);
                rest::end_of_turn_party_update(gm);
                let tt = gm.g8(g::TRANSPORT_TILE);
                if tile == 1 && ((tt & 0xfc) == 0x28 || (tt & 0xfe) == 0x14) {
                    text::print_string(gm, 0x2bb3); // "Rough seas!\n"
                    rest::flash_tile_at(gm, gm.g8(g::PARTY_Y) as i16, gm.g8(g::PARTY_X) as i16);
                    vehicle_damage(gm);
                }
                if ((tile as u8) & 0xfc) == 0xd4 {
                    ovl::outsubs::go_over_falls(gm);
                }
                update_overworld_monsters(gm);
            }
        } else {
            done = true;
        }
        if done || gm.pf.quit_requested { return; }
    }
}

/// orig 1000:8ef2 (105 bytes). Entry point from ROOT main (vector 1778:02ba) when
/// g_location == 0. init_overworld, then if a vessel was bought in a town (g_bought_vessel >
/// 0x3f) a free monster slot receives it at (g_bought_ship_x, g_bought_ship_y) on level 0:
/// value3 = g_bought_vessel & 0x3f, tile/anim 0x29 (skiff) when < 0x80 else 0x25 (anchored
/// frigate), hull (value1) 99; g_bought_vessel is cleared. Then overworld_main_loop.
pub fn run_overworld(gm: &mut Game) {
    init_overworld(gm);
    if 0x3f < gm.g8(g::BOUGHT_VESSEL) {
        let slot = rest::find_spawn_slot(gm);
        let so = slot_off(slot);
        let v = gm.g8(g::BOUGHT_VESSEL) & 0x3f;
        gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(so).wrapping_add(1), v); // value3 (+7)
        let v = gm.g8(g::BOUGHT_SHIP_X);
        gm.set_g8(g::MONSTER_X.wrapping_add(so), v);
        let v = gm.g8(g::BOUGHT_SHIP_Y);
        gm.set_g8(g::MONSTER_Y.wrapping_add(so), v);
        gm.set_g8(g::MONSTER_Z.wrapping_add(so), 0);
        let tile: u8 = if gm.g8(g::BOUGHT_VESSEL) < 0x80 { 0x29 } else { 0x25 };
        gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(so), tile);
        gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), tile);
        gm.set_g8(g::SHIP_HULL.wrapping_add(so), 99);
        gm.set_g8(g::BOUGHT_VESSEL, 0);
    }
    overworld_main_loop(gm);
}

/// orig 1000:8f5c (116 bytes). Random-encounter threshold for this turn (compared with
/// rand_range(1,30): a spawn happens when the roll is below it). On the surface: 0 on tiles
/// 0x20..0x26 (roads/paths), 2 on swamp (4) or forest/hills (9..0xf), 1 elsewhere, plus 3 at
/// night (hour > 0x1f or < 5). In the Underworld always 3.
pub fn spawn_rate_for_terrain(gm: &mut Game) -> i16 {
    let rate: i16;
    if gm.g8(g::PARTY_Z) < 0x80 {
        let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
        let t = gm.g8(p);
        let mut r: i16;
        if t < 0x20 || 0x26 < t {
            if t == 4 || (8 < t && t < 0x10) { r = 2; } else { r = 1; }
        } else {
            r = 0;
        }
        let h = gm.g8(g::HOUR);
        if 0x1f < h || h < 5 { r += 3; }
        rate = r;
    } else {
        rate = 3;
    }
    rate
}

/// orig 1000:8fd4 (73 bytes). Weighted random pick: rolls 0..255 and walks the byte weight
/// table at `table` subtracting each weight until the remaining roll is below the next weight;
/// returns the index reached (the weights of each table sum to 256).
pub fn weighted_random_index(gm: &mut Game, param_1: i16) -> i16 {
    let table = param_1 as u16;
    let mut r = util::rand_range(gm, 0xff, 0) as u16;
    let mut i: i16 = 0;
    while gm.g8((i as u16).wrapping_add(table)) as u16 <= r {
        let w = gm.g8((i as u16).wrapping_add(table)) as u16;
        i = i.wrapping_add(1);
        r = r.wrapping_sub(w);
    }
    i
}

/// orig 1000:901e (252 bytes). Chooses the monster tile to spawn on terrain tile `terrain`, or 0
/// for none. Water (tiles < 4, 0x60..0x6f, 0xd4..0xd7, 0xe4..0xe7): 1/4 chance
/// (rand_range(0,64) < 16); Underworld -> tile table 0x2bda weighted by 0x2bf6; surface deep
/// water (1) has a 1/8 chance of a whirlpool (0xec), else table 0x2bd4 weighted by 0x2bf0 (sea
/// serpents, pirate ships ...). Tile 7: 1/4 chance of 0xe0. Swamp (4) in the Underworld
/// (g_party_z == 0xff): 0xf8. Other land tiles except 0xc/0xd and only < 0x10 or 0x30..0x33:
/// surface table 0x2bc0 weighted by 0x2bdc, Underworld table 0x2bcc weighted by 0x2be8.
/// (These tables are the first part of what symbols.json calls g_dungeon_draw_tables.)
pub fn choose_spawn_monster(gm: &mut Game, param_1: i16) -> u8 {
    let t = param_1;
    if t < 4 || (0x5f < t && t < 0x70) || (0xd3 < t && t < 0xd8) || (0xe3 < t && t < 0xe8) {
        let r = util::rand_range(gm, 0x40, 0);
        if r < 0x10 {
            if 0x7f < gm.g8(g::PARTY_Z) {
                let i = weighted_random_index(gm, 0x2bf6); // Underworld water weights
                return gm.g8((i as u16).wrapping_add(0x2bda)); // Underworld water monsters
            }
            if t == 1 && util::rand_range(gm, 7, 0) == 7 {
                return 0xec;
            }
            let i = weighted_random_index(gm, 0x2bf0); // sea weights
            return gm.g8((i as u16).wrapping_add(0x2bd4)); // sea monsters
        }
    } else if t == 7 {
        let r = util::rand_range(gm, 3, 0);
        if r == 0 { return 0xe0; }
    } else {
        if t == 4 && gm.g8(g::PARTY_Z) == 0xff {
            return 0xf8;
        }
        if t != 0xc && t != 0xd && (t < 0x10 || ((t as u8) & 0xfc) == 0x30) {
            if gm.g8(g::PARTY_Z) < 0x80 {
                let i = weighted_random_index(gm, 0x2bdc); // surface land weights
                return gm.g8((i as u16).wrapping_add(0x2bc0)); // surface land monsters
            }
            let i = weighted_random_index(gm, 0x2be8); // Underworld land weights
            return gm.g8((i as u16).wrapping_add(0x2bcc)); // Underworld land monsters
        }
    }
    0
}

/// orig 1000:911e (117 bytes). Picks a random spawn cell inside the cached 32x32 chunk window
/// into g_dx/g_dy (g_chunk_x/y + 0..31, 8-bit wrapped) at least 7 cells away from the party in
/// both axes and not wrapped around (|delta| <= 0xf9).
pub fn random_spawn_position(gm: &mut Game) {
    loop {
        loop {
            let r = util::rand_range(gm, 0x1f, 0) as u8;
            gm.set_g16(g::DX, r.wrapping_add(gm.g8(g::CHUNK_X)) as u16);
            let r = util::rand_range(gm, 0x1f, 0) as u8;
            gm.set_g16(g::DY, r.wrapping_add(gm.g8(g::CHUNK_Y)) as u16);
            let d = gm.g16(g::DX).wrapping_sub(gm.g8(g::PARTY_X) as u16) as i16;
            let a = (d ^ (d >> 15)).wrapping_sub(d >> 15);
            if !(a < 7) { break; }
        }
        let d = gm.g16(g::DY).wrapping_sub(gm.g8(g::PARTY_Y) as u16) as i16;
        let ay = (d ^ (d >> 15)).wrapping_sub(d >> 15);
        let d = gm.g16(g::DX).wrapping_sub(gm.g8(g::PARTY_X) as u16) as i16;
        let ax = (d ^ (d >> 15)).wrapping_sub(d >> 15);
        if !(ay < 7 || 0xf9 < ax || 0xf9 < ay) { break; }
    }
}

/// orig 1000:9194 (150 bytes). Random encounter spawn: up to 128 tries of random_spawn_position
/// + choose_spawn_monster for the terrain there; a pirate ship (0x2c) is rejected on tiles
/// 0x60..0x6f. On success a free slot is filled (set_monster_slot: value1 0, party level, tile
/// used for both tile and anim tile) and a ship gets hull (value1) 100.
pub fn spawn_random_monster(gm: &mut Game) {
    let mut tries: i16 = 0;
    let mut y: u16 = 0;
    let mut x: u16 = 0;
    let mut m: i8 = 0;
    loop {
        random_spawn_position(gm);
        y = gm.g16(g::DY);
        x = gm.g16(g::DX);
        let p = rest::get_map_tile(gm, y, x);
        let t = gm.g8(p);
        m = choose_spawn_monster(gm, t as i16) as i8;
        if m != 0 {
            if m != 0x2c { break; }
            let p2 = rest::get_map_tile(gm, y, x);
            if (gm.g8(p2) & 0xf0) != 0x60 { break; }
        }
        tries += 1;
        if !(tries < 0x80) { break; }
    }
    if tries != 0x80 {
        let slot = rest::find_spawn_slot(gm);
        rest::set_monster_slot(gm, slot, 0, gm.g8(g::PARTY_Z), y as u8, x as u8, m as u8, m as u8);
        if m == 0x2c {
            gm.set_g8(g::SHIP_HULL.wrapping_add(slot_off(slot)), 100);
        }
    }
}

/// orig 1000:922c (65 bytes). Returns 1 when `tile` is a moving overworld object: an enemy ship
/// (0x2c..0x2f) or any tile >= 0x80 except the shards 0xb4..0xb7 and 0xe8..0xeb. Also vector
/// 1778:045e (CMDS).
pub fn is_active_monster_tile(gm: &mut Game, param_1: i16) -> u16 {
    let t = param_1;
    if (t < 0x2c || 0x2f < t) && ((t < 0x80 || (0xb3 < t && t < 0xb8)) || (0xe7 < t && t < 0xec)) {
        0
    } else {
        1
    }
}

/// orig 1000:926e (198 bytes). Damage to the party's vehicle. In a frigate (0x20..0x27): a
/// 1..30 roll below the hull (g_ship_hull, slot 0 value1) is subtracted from it (status window
/// redrawn); otherwise "Ship sunk!": with no skiff aboard (slot 0 value3 == 0) and no magic
/// carpet the party is on foot in the water, "DROWNING!!!" and damage_whole_party repeats until
/// everyone is dead; else "Abandon ship!" into a carpet (one g_magic_carpets used, tile 0x14 or
/// 0x15) or the skiff (0x28 + heading). Not in a frigate: damage_whole_party.
pub fn vehicle_damage(gm: &mut Game) {
    if (gm.g8(g::TRANSPORT_TILE) & 0xf8) == 0x20 {
        let r = util::rand_range(gm, 0x1e, 1) as u16;
        if r < gm.g8(g::SHIP_HULL) as u16 {
            let v = gm.g8(g::SHIP_HULL).wrapping_sub(r as u8);
            gm.set_g8(g::SHIP_HULL, v);
            rest::draw_status_window(gm);
        } else {
            text::print_string(gm, 0x6ada); // "Ship sunk!\n"
            let skiffs = gm.gi8(0x5c61); // g_monster_table[0] value3: skiffs aboard
            if skiffs == 0 && gm.g8(g::MAGIC_CARPETS) == 0 {
                gm.set_g8(g::TRANSPORT_TILE, 0);
                rest::draw_status_window(gm);
                view::refresh_map_view(gm);
                rest::play_tone_sweep(gm, 0x1e78, 0x28, 0x96, 0x294);
                text::print_string(gm, 0x6af6); // "DROWNING!!!\n"
                loop {
                    let r = rest::find_first_conscious_member(gm) as i16;
                    if r == -1 || gm.pf.quit_requested { break; }
                    rest::flash_tile_at(gm, gm.g8(g::PARTY_Y) as i16, gm.g8(g::PARTY_X) as i16);
                    rest::damage_whole_party(gm);
                }
            } else {
                text::print_string(gm, 0x6ae6); // "Abandon ship!\n"
                if skiffs == 0 {
                    let v = gm.g8(g::MAGIC_CARPETS).wrapping_sub(1);
                    gm.set_g8(g::MAGIC_CARPETS, v);
                    let r = util::rand_range(gm, 1, 0) as u8;
                    gm.set_g8(g::TRANSPORT_TILE, r.wrapping_add(0x14));
                } else {
                    let v = (gm.g8(g::TRANSPORT_TILE) & 3).wrapping_add(0x28);
                    gm.set_g8(g::TRANSPORT_TILE, v);
                }
                gm.set_g8(g::STATUS_DIRTY, 1);
            }
        }
    } else {
        rest::damage_whole_party(gm);
    }
}

/// orig 1000:9338 (135 bytes; runs to the RET at 9414, Ghidra split the tail off as
/// FUN_93c0). An enemy ship in slot `slot` fires at the party: when the x distance is 0 a ship
/// facing N/S (anim tile 0x2c/0x2e) turns broadside (0x2d or 0x2f at random); when the y
/// distance is 0 one facing E/W (0x2d/0x2f) turns to 0x2c or 0x2e. Redraws, plays the cannon
/// sweep and fires missile type 1 from view cell (5,5) to the ship's view cell (dy+5, dx+5)
/// via COMSUBS_f4be; a hit redraws, flashes the party tile and applies vehicle_damage.
/// Ghidra order: (dy_dist, dx_dist, slot).
pub fn enemy_ship_fires(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16) {
    let (dy_dist, dx_dist, slot) = (param_1, param_2, param_3);
    let so = slot_off(slot);
    if dx_dist == 0 {
        let t = gm.g8(g::PARTY_VEHICLE_TILE.wrapping_add(so));
        if t == 0x2c || t == 0x2e {
            let r = util::rand_range(gm, 3, 0) as u8;
            gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(so), (r & 2).wrapping_add(0x2d));
        }
    }
    if dy_dist == 0 {
        let t = gm.g8(g::PARTY_VEHICLE_TILE.wrapping_add(so));
        if t == 0x2d || t == 0x2f {
            let r = util::rand_range(gm, 3, 0) as u8;
            gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(so), (r & 2).wrapping_add(0x2c));
        }
    }
    view::refresh_map_view(gm);
    rest::play_tone_sweep(gm, 100, 5, 300, 0x514);
    let vy = gm.g8(g::MONSTER_Y.wrapping_add(so)).wrapping_sub(gm.g8(g::PARTY_Y)).wrapping_add(5) as i16;
    let vx = gm.g8(g::MONSTER_X.wrapping_add(so)).wrapping_sub(gm.g8(g::PARTY_X)).wrapping_add(5) as i16;
    let hit = ovl::comsubs::animate_projectile(gm, 1, 5, 5, vy, vx);
    if hit != 0 {
        view::refresh_map_view(gm);
        rest::flash_tile_at(gm, gm.g8(g::PARTY_Y) as i16, gm.g8(g::PARTY_X) as i16);
        vehicle_damage(gm);
    }
}

/// orig 1000:93c0 (87 bytes). Not a function: vector 1778:02c6 (vec_TOWN_93c0) points here when
/// TOWN is loaded; in MAINOUT these bytes are the second half of `enemy_ship_fires` (the
/// missile call and hit handling). No-op.
pub fn enemy_ship_fires_tail_93c0(gm: &mut Game) {}

/// orig 1000:9418 (209 bytes). A monster in slot `slot` has reached the party (adjacent).
/// Whirlpool (0xec..0xef) while not on foot: the whirlpool object is removed, "WHIRLPOOL!",
/// the party tile briefly becomes 0xec for the redraw and sound, vehicle_damage, then the
/// party is dropped into the Underworld at (0x22,0x12) level 0xff and init_overworld. Tiles
/// 0xe0..0xe3 only cause vehicle_damage. Any other monster: redraw, "Attacked!", and unless
/// the party is on water (tile < 4) in a skiff/carpet, fight_monster(slot); on water in a
/// skiff/carpet only vehicle_damage.
pub fn monster_contact(gm: &mut Game, param_1: i16) {
    let slot = param_1;
    let so = slot_off(slot);
    let tile = gm.g8(g::MONSTER_TABLE.wrapping_add(so));
    if (tile & 0xfc) == 0xec {
        if gm.g8(g::TRANSPORT_TILE) != 0x1c {
            gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(so), 0);
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), 0);
            text::print_string(gm, 0x6b04); // "\nWHIRLPOOL!\n"
            let saved = gm.g8(g::TRANSPORT_TILE);
            gm.set_g8(g::TRANSPORT_TILE, 0xec);
            view::refresh_map_view(gm);
            rest::play_tone_sweep(gm, 0x1e78, 0x28, 0x96, 0x294);
            gm.set_g8(g::TRANSPORT_TILE, saved);
            vehicle_damage(gm);
            gm.set_g8(g::PARTY_Z, 0xff);
            gm.set_g8(g::PARTY_X, 0x22);
            gm.set_g8(g::PARTY_Y, 0x12);
            init_overworld(gm);
            return;
        }
    } else if (tile & 0xfc) != 0xe0 {
        view::refresh_map_view(gm);
        text::print_string(gm, 0x6b12); // "\nAttacked!\n"
        let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
        let tt = gm.g8(g::TRANSPORT_TILE);
        if 3 < gm.g8(p) || ((tt & 0xfe) != 0x14 && (tt & 0xfc) != 0x28) {
            rest::fight_monster(gm, slot);
            return;
        }
    }
    vehicle_damage(gm);
}

/// Absolute 8-bit-wrapped distance between a monster coordinate and a party coordinate as the
/// original computes it: |m - p| with values > 0x7f folded to 0x100 - d.
#[inline] fn wrapped_dist(m: u8, p: u8) -> i16 {
    let d = (m as u16).wrapping_sub(p as u16) as i16;
    let a = (d ^ (d >> 15)).wrapping_sub(d >> 15);
    if 0x7f < a { 0x100 - a } else { a }
}

/// orig 1000:94ea (358 bytes). Lets the monster in `slot` act when near the party; returns 1 if
/// it did. Orthogonally adjacent: monster_contact. Tiles 0x88 and 0xdc (ranged attackers)
/// within 3 cells in both axes have a 1/8 chance to fire missile type 3 at the party
/// (COMSUBS_f4be from (5,5) to the monster's view cell; a hit flashes the party and calls
/// vehicle_damage) - counts as acting even on a miss. An enemy ship (0x2c..0x2f) in line
/// (same row or column) within 3 cells prints "* BOOOM! *" and enemy_ship_fires.
pub fn monster_act_near_party(gm: &mut Game, param_1: i16) -> u16 {
    let slot = param_1;
    let so = slot_off(slot);
    let tile = gm.g8(g::MONSTER_TABLE.wrapping_add(so));
    let ddx = wrapped_dist(gm.g8(g::MONSTER_X.wrapping_add(so)), gm.g8(g::PARTY_X));
    let ddy = wrapped_dist(gm.g8(g::MONSTER_Y.wrapping_add(so)), gm.g8(g::PARTY_Y));
    if (ddx == 1 && ddy == 0) || (ddx == 0 && ddy == 1) {
        monster_contact(gm, slot);
        return 1;
    }
    if tile == 0x88 || tile == 0xdc {
        if ddx < 4 && ddy < 4 && util::rand_range(gm, 7, 0) == 0 {
            view::refresh_map_view(gm);
            rest::play_tone_sweep(gm, 100, 5, 300, 0x514);
            let vy = gm.g8(g::MONSTER_Y.wrapping_add(so)).wrapping_sub(gm.g8(g::PARTY_Y)).wrapping_add(5) as i16;
            let vx = gm.g8(g::MONSTER_X.wrapping_add(so)).wrapping_sub(gm.g8(g::PARTY_X)).wrapping_add(5) as i16;
            let hit = ovl::comsubs::animate_projectile(gm, 3, 5, 5, vy, vx);
            if hit != 0 {
                rest::flash_tile_at(gm, gm.g8(g::PARTY_Y) as i16, gm.g8(g::PARTY_X) as i16);
                vehicle_damage(gm);
            }
            return 1;
        }
    } else if (tile & 0xfc) == 0x2c && ((ddx == 0 && ddy < 4) || (ddy == 0 && ddx < 4)) {
        text::print_string(gm, 0x6b1e); // "* BOOOM! *\n\n"
        enemy_ship_fires(gm, ddy, ddx, slot);
        return 1;
    }
    0
}

/// orig 1000:9652 (70 bytes). Returns 1 if the monster in `slot` may move onto map cell (y,x):
/// can_enter_tile(map tile, monster tile) and no object there (find_object_at_rev on the
/// party's level). Ghidra order: (y, x, slot).
pub fn monster_can_move_to(gm: &mut Game, param_1: u16, param_2: u16, param_3: i16) -> u16 {
    let (y, x, slot) = (param_1, param_2, param_3);
    let mtile = gm.g8(g::MONSTER_TABLE.wrapping_add(slot_off(slot))) as u16;
    let p = rest::get_map_tile(gm, y, x);
    let t = gm.g8(p) as i16;
    if rest::can_enter_tile(gm, t, mtile as i16) != 0
        && rest::find_object_at_rev(gm, gm.g8(g::PARTY_Z) as u16, y, x) == 0
    {
        return 1;
    }
    0
}

/// orig 1000:9698 (32 bytes). Returns 0 when (y,x) is the cell the last moved monster came from
/// (g_prev_monster_x/y at 0xa526/0xa527, BSS), else 1 - prevents a monster oscillating.
/// Ghidra order: (y, x).
pub fn is_not_prev_monster_pos(gm: &mut Game, param_1: u16, param_2: u16) -> u16 {
    let (y, x) = (param_1, param_2);
    if x == gm.g8(0xa526) as u16 && y == gm.g8(0xa527) as u16 { 0 } else { 1 }
}

/// orig 1000:96ba (140 bytes). For the monster in `slot`, if it is within 5 cells of the party
/// in both axes returns the byte of the 6x6 (row stride 11) 0/1 table at 0x2c18 indexed by
/// [dy][dx] (which relative positions "see" the party), else 0.
pub fn monster_sees_party(gm: &mut Game, param_1: i16) -> u8 {
    let so = slot_off(param_1);
    let ddx = wrapped_dist(gm.g8(g::MONSTER_X.wrapping_add(so)), gm.g8(g::PARTY_X));
    let ddy = wrapped_dist(gm.g8(g::MONSTER_Y.wrapping_add(so)), gm.g8(g::PARTY_Y));
    if ddx < 6 && ddy < 6 {
        gm.g8((ddx as u16).wrapping_add((ddy as u16).wrapping_mul(0xb)).wrapping_add(0x2c18))
    } else {
        0
    }
}

/// orig 1000:9748 (324 bytes). Moves the monster in `slot` by (dy,dx). An enemy ship
/// (0x2c..0x2f) turns to face the direction (N 0, E 1, S 2, W 3 -> tile 0x2c + facing). Other
/// monsters except 0xdc, 0x94, 0xd8 and 0xf0 are slowed by the destination terrain: on tiles
/// 4,6,7,8,0x1e,0x1f a 1/2 chance to stay put, on 9..0xf a 2/3 chance. The old position is
/// remembered in g_prev_monster_x/y (0xa526/0xa527), the position is updated (8-bit), the view
/// is flagged (g_view_dirty |= 2) and a monster that steps onto map tile 0xdc is removed
/// (tile and anim tile cleared). Ghidra order: (dy, dx, slot).
pub fn move_monster(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16) {
    let (dy, dx, slot) = (param_1, param_2, param_3);
    let so = slot_off(slot);
    let tile = gm.g8(g::MONSTER_TABLE.wrapping_add(so));
    let nx = (gm.g8(g::MONSTER_X.wrapping_add(so)) as u16).wrapping_add(dx as u16);
    let ny = (gm.g8(g::MONSTER_Y.wrapping_add(so)) as u16).wrapping_add(dy as u16);
    let p = rest::get_map_tile(gm, ny, nx);
    if (tile & 0xfc) == 0x2c {
        let mut facing: i8 = 0; // TODO(verify): uninitialised in the original for a diagonal step (never happens)
        if dx == 0 && dy == -1 { facing = 0; }
        else if dx == 1 && dy == 0 { facing = 1; }
        else if dx == 0 && dy == 1 { facing = 2; }
        else if dx == -1 && dy == 0 { facing = 3; }
        let t = (facing as u8).wrapping_add(0x2c);
        gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(so), t);
        gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), t);
    } else if tile != 0xdc && tile != 0x94 && tile != 0xd8 && tile != 0xf0 {
        match gm.g8(p) {
            4 | 6 | 7 | 8 | 0x1e | 0x1f => {
                if util::rand_range(gm, 1, 0) == 0 { return; }
            }
            9 | 10 | 0xb | 0xc | 0xd | 0xe | 0xf => {
                if util::rand_range(gm, 2, 0) != 2 { return; }
            }
            _ => {}
        }
    }
    let v = gm.g8(g::MONSTER_X.wrapping_add(so));
    gm.set_g8(0xa526, v); // g_prev_monster_x
    let v = gm.g8(g::MONSTER_Y.wrapping_add(so));
    gm.set_g8(0xa527, v); // g_prev_monster_y
    gm.set_g8(g::MONSTER_X.wrapping_add(so), nx as u8);
    gm.set_g8(g::MONSTER_Y.wrapping_add(so), ny as u8);
    let v = gm.g8(g::VIEW_DIRTY) | 2;
    gm.set_g8(g::VIEW_DIRTY, v);
    let p2 = rest::get_map_tile(gm, ny, nx);
    if gm.gi8(p2) == -0x24 {
        gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(so), 0);
        gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), 0);
    }
}

/// orig 1000:98cc (213 bytes). Random step for the monster in `slot`: picks a direction
/// (rand 0..3: N, E, S, W) and moves that way if monster_can_move_to allows it, otherwise
/// gives up for this turn (the retry counter for out-of-range rolls, max 3, is unreachable).
pub fn monster_random_step(gm: &mut Game, param_1: i16) {
    let slot = param_1;
    let so = slot_off(slot);
    let x = gm.g8(g::MONSTER_X.wrapping_add(so)) as u16;
    let y = gm.g8(g::MONSTER_Y.wrapping_add(so)) as u16;
    let mut n: i16 = 0;
    loop {
        let r = util::rand_range(gm, 3, 0);
        if r == 0 {
            if monster_can_move_to(gm, y.wrapping_sub(1), x, slot) == 0 { return; }
            move_monster(gm, -1, 0, slot);
            return;
        }
        if r == 1 {
            if monster_can_move_to(gm, y, x.wrapping_add(1), slot) == 0 { return; }
            move_monster(gm, 0, 1, slot);
            return;
        }
        if r == 2 {
            if monster_can_move_to(gm, y.wrapping_add(1), x, slot) == 0 { return; }
            move_monster(gm, 1, 0, slot);
            return;
        }
        if r == 3 {
            if monster_can_move_to(gm, y, x.wrapping_sub(1), slot) == 0 { return; }
            move_monster(gm, 0, -1, slot);
            return;
        }
        n += 1;
        if 2 < n { return; }
    }
}

/// orig 1000:99a4 (431 bytes). Moves the monster in `slot` one step toward the party. The
/// signed 8-bit deltas give a step (sx, sy) of +-1 toward the party (0 when aligned). A coin
/// toss chooses whether the x step or the y step is tried first; a step is taken when
/// monster_can_move_to allows it and the cell is not the one just left (is_not_prev_monster_pos),
/// else the other axis is tried; if neither works monster_random_step. `mode` (Ghidra param_1)
/// is passed by monster_turn (0 for the 0xfc pursuer, 1 otherwise) but is not used.
/// Ghidra order: (mode, slot).
pub fn monster_chase_party(gm: &mut Game, param_1: u16, param_2: i16) {
    let slot = param_2;
    let so = slot_off(slot);
    let x = gm.g8(g::MONSTER_X.wrapping_add(so)) as u16;
    let y = gm.g8(g::MONSTER_Y.wrapping_add(so)) as u16;
    let bx = gm.g8(g::MONSTER_X.wrapping_add(so)).wrapping_sub(gm.g8(g::PARTY_X));
    let mut ddx = bx as u16;
    if 0x7f < bx { ddx = ddx.wrapping_sub(0x100); }
    let by = gm.g8(g::MONSTER_Y.wrapping_add(so)).wrapping_sub(gm.g8(g::PARTY_Y));
    let mut ddy = by as u16;
    if 0x7f < by { ddy = ddy.wrapping_sub(0x100); }
    let mut sx: i16 = 0;
    if ddx == 0 { sx = 0; } else if (ddx as i16) < 1 { if (ddx as i16) < 0 { sx = 1; } } else { sx = -1; }
    let mut sy: i16 = 0;
    if ddy == 0 { sy = 0; } else if (ddy as i16) < 1 { if (ddy as i16) < 0 { sy = 1; } } else { sy = -1; }
    let r = util::rand_range(gm, 1, 0);
    let (mdy, mdx): (i16, i16);
    if r == 1 {
        if sx != 0
            && monster_can_move_to(gm, y, x.wrapping_add(sx as u16), slot) != 0
            && is_not_prev_monster_pos(gm, y, x.wrapping_add(sx as u16)) != 0
        {
            mdy = 0; mdx = sx; // LAB_1000_9ab2
        } else if sy == 0 {
            monster_random_step(gm, slot);
            return;
        } else if monster_can_move_to(gm, y.wrapping_add(sy as u16), x, slot) == 0
            || is_not_prev_monster_pos(gm, y.wrapping_add(sy as u16), x) == 0
        {
            monster_random_step(gm, slot);
            return;
        } else {
            mdy = sy; mdx = 0;
        }
    } else {
        let try_x: bool;
        if sy == 0 {
            try_x = true;
        } else if monster_can_move_to(gm, y.wrapping_add(sy as u16), x, slot) == 0
            || is_not_prev_monster_pos(gm, y.wrapping_add(sy as u16), x) == 0
        {
            try_x = true;
        } else {
            try_x = false;
        }
        if try_x {
            // LAB_1000_9b25
            if sx == 0 {
                monster_random_step(gm, slot);
                return;
            }
            if monster_can_move_to(gm, y, x.wrapping_add(sx as u16), slot) == 0
                || is_not_prev_monster_pos(gm, y, x.wrapping_add(sx as u16)) == 0
            {
                monster_random_step(gm, slot);
                return;
            }
            mdy = 0; mdx = sx;
        } else {
            mdy = sy; mdx = 0;
        }
    }
    // LAB_1000_9afa
    move_monster(gm, mdy, mdx, slot);
}

/// orig 1000:9b5c (210 bytes). One AI turn for the monster in `slot`. Whirlpools (0xec..):
/// value1 toggles each turn and they only move on odd turns, half the time randomly. Tile
/// 0xfc (the dungeon-entrance pursuer): if it sees the party (monster_sees_party) its value1
/// counter is incremented and while it was < 20 it chases with mode 0; otherwise mode 1.
/// Enemy ships (0x2c..0x2f): with no wind they stay; the word table at 0x2bf8 [heading][wind
/// direction] gives a pace value - 4 means move every turn, otherwise the value3 counter (+7)
/// is incremented and when it exceeds the pace it is reset and the ship skips this turn.
/// Everything else chases the party (monster_chase_party mode 1).
pub fn monster_turn(gm: &mut Game, param_1: i16) {
    let slot = param_1;
    let so = slot_off(slot);
    let tile = gm.g8(g::MONSTER_TABLE.wrapping_add(so));
    let mut mode: u16 = 1;
    if (tile & 0xfc) == 0xec {
        let p = g::SHIP_HULL.wrapping_add(so); // value1
        let v = gm.g8(p) ^ 1;
        gm.set_g8(p, v);
        if v == 0 { return; }
        if util::rand_range(gm, 1, 0) == 0 {
            monster_random_step(gm, slot);
            return;
        }
    } else if tile == 0xfc {
        if monster_sees_party(gm, slot) != 0 {
            let p = g::SHIP_HULL.wrapping_add(so); // value1 counter
            let old = gm.g8(p);
            gm.set_g8(p, old.wrapping_add(1));
            if old < 0x14 {
                mode = 0;
            }
        }
    } else if (tile & 0xfc) == 0x2c {
        if gm.g8(g::WIND_DIR) == 0 { return; }
        let heading = (tile as u16).wrapping_sub(0x2c);
        let pace = gm.g16((gm.g8(g::WIND_DIR) as u16).wrapping_mul(2).wrapping_add(heading.wrapping_mul(8)).wrapping_add(0x2bf6)); // ship pace table 0x2bf8
        if pace != 4 {
            let p = g::MONSTER_ANIM_STATE.wrapping_add(so).wrapping_add(1); // value3 (+7)
            let v = gm.g8(p).wrapping_add(1);
            gm.set_g8(p, v);
            if pace < v as u16 {
                gm.set_g8(p, 0);
                return;
            }
        }
    }
    // LAB_1000_9c24
    monster_chase_party(gm, mode, slot);
}

/// orig 1000:9c30 (221 bytes). Per-turn overworld monster update (vector 1778:03f2; also from
/// hole_up_and_camp). Skipped entirely (returns 0) while time is stopped (status icon 'T'),
/// on alternate turns under Quickness ('Q', toggle byte 0x2c55) and on alternate turns when
/// riding a horse or carpet (0x12..0x15, toggle byte 0x2c57) - the toggles are DATA.OVL bytes
/// used as RAM. Otherwise: a random encounter is spawned when rand_range(1,30) <
/// spawn_rate_for_terrain; then slots 31 down to 1 holding an active monster tile act
/// (monster_act_near_party, its results summed; while nothing has acted yet the monster also
/// takes its AI turn via monster_turn); finally active monsters that drifted outside the
/// cached 32x32 window (offset from g_chunk_x/y > 31, 8-bit) are cleared. Returns the number
/// of monsters that acted against the party.
pub fn update_overworld_monsters(gm: &mut Game) -> i16 {
    let mut acted: i16 = 0;
    let icon = gm.g8(g::STATUS_ICON);
    let mut skip = icon == b'T';
    if !skip && icon == b'Q' {
        let v = gm.g8(0x2c55) ^ 1; // quickness alternate-turn toggle
        gm.set_g8(0x2c55, v);
        if v != 0 { skip = true; }
    }
    if !skip {
        let tt = gm.g8(g::TRANSPORT_TILE);
        if (tt & 0xfe) == 0x12 || (tt & 0xfe) == 0x14 {
            let v = gm.g8(0x2c57) ^ 1; // horse/carpet alternate-turn toggle
            gm.set_g8(0x2c57, v);
            if v != 0 { skip = true; }
        }
    }
    if skip {
        acted = 0;
    } else {
        let roll = util::rand_range(gm, 0x1e, 1);
        let rate = spawn_rate_for_terrain(gm);
        if roll < rate {
            spawn_random_monster(gm);
        }
        let mut slot: i16 = 0x1f;
        loop {
            let t = gm.g8(g::MONSTER_TABLE.wrapping_add(slot_off(slot)));
            if is_active_monster_tile(gm, t as i16) != 0 {
                let r = monster_act_near_party(gm, slot) as i16;
                acted = acted.wrapping_add(r);
                if acted == 0 {
                    monster_turn(gm, slot);
                }
            }
            slot -= 1;
            if !(0 < slot) { break; }
        }
        let mut slot: i16 = 0x1f;
        loop {
            let so = slot_off(slot);
            let t = gm.g8(g::MONSTER_TABLE.wrapping_add(so));
            if is_active_monster_tile(gm, t as i16) != 0
                && (0x1f < gm.g8(g::MONSTER_X.wrapping_add(so)).wrapping_sub(gm.g8(g::CHUNK_X))
                    || 0x1f < gm.g8(g::MONSTER_Y.wrapping_add(so)).wrapping_sub(gm.g8(g::CHUNK_Y)))
            {
                rest::set_monster_slot(gm, slot, 0, 0, 0, 0, 0, 0);
            }
            slot -= 1;
            if !(0 < slot) { break; }
        }
    }
    acted
}

/// orig 1000:9d0e (169 bytes). "Caught! The trolls demand a <N> gp toll! Dost thou pay?" where
/// N = 99 - 3 * strength of the first conscious member (find_first_conscious_member leaves
/// the index in g_dx). Y/N is read; paying subtracts the toll (status redrawn) and ends it if
/// the gold stays >= 0 (as a signed word), otherwise the gold is restored; refusing or being
/// unable to pay spawns a troll group (tile 0xe4) on the party's cell and fights it.
pub fn troll_bridge_toll(gm: &mut Game) {
    text::print_string(gm, 0x6b2c); // "Caught!\n\nThe trolls demand a "
    rest::find_first_conscious_member(gm);
    let idx = gm.g16(g::DX);
    let strength = gm.g8(idx.wrapping_mul(0x20).wrapping_add(0x55b4)) as i16; // party record +0xc strength
    let delta = strength.wrapping_mul(3).wrapping_sub(99);
    let toll = delta.wrapping_neg();
    text::print_number(gm, 0x20, 2, toll);
    text::print_string(gm, 0x6b4a); // " gp toll!\n\nDost thou pay?"
    let mut c: u8;
    loop {
        c = input::read_command_key(gm) as u8;
        if c == b'Y' { break; }
        if c == b'N' || gm.pf.quit_requested { break; }
    }
    text::put_char(gm, c);
    text::put_char(gm, 10);
    if c == b'Y' {
        let v = gm.gi16(g::GOLD).wrapping_add(delta);
        gm.set_gi16(g::GOLD, v);
        gm.set_g8(g::STATUS_DIRTY, 1);
        if -1 < gm.gi16(g::GOLD) {
            gm.set_g8(g::STATUS_DIRTY, 1);
            return;
        }
        let v = gm.gi16(g::GOLD).wrapping_add(toll);
        gm.set_gi16(g::GOLD, v);
    }
    let slot = rest::find_spawn_slot(gm);
    rest::set_monster_slot(gm, slot, 0, 0, gm.g8(g::PARTY_Y), gm.g8(g::PARTY_X), 0, 0xe4);
    rest::fight_monster(gm, slot);
}

/// orig 1000:9db8 (198 bytes). On a bridge tile (0x6a/0x6b) on foot there is a 1/8 chance of
/// "Thou spieth trolls under the bridge!": every member who is not dead or asleep "sneaks
/// across" (three dots with animate_wait(5) between them); a member whose dexterity is below
/// rand_range(1,30) is caught (troll_bridge_toll, which ends the check); otherwise
/// "Trolls evaded!".
pub fn bridge_troll_ambush(gm: &mut Game) {
    let r = util::rand_range(gm, 7, 0);
    if r == 0 && gm.g8(g::TRANSPORT_TILE) == 0x1c {
        view::refresh_map_view(gm);
        text::print_string(gm, 0x6b64); // "\nThou spieth trolls under the bridge!\n\n"
        util::animate_wait(gm, 10);
        if gm.g8(g::PARTY_COUNT) != 0 {
            let mut i: u16 = 0;
            loop {
                let rec = i.wrapping_mul(0x20);
                let st = gm.g8(g::PARTY_STATUS.wrapping_add(rec));
                if st != b'D' && st != b'S' {
                    text::print_string(gm, 0x55a8u16.wrapping_add(rec)); // member name
                    text::print_string(gm, 0x6b8c); // " sneaks across"
                    let mut n: i16 = 3;
                    loop {
                        util::animate_wait(gm, 5);
                        text::put_char(gm, 0x2e);
                        n -= 1;
                        if n == 0 { break; }
                    }
                    text::print_string(gm, 0x6b9c); // "\n\n"
                    let roll = util::rand_range(gm, 0x1e, 1) as u16;
                    if (gm.g8(g::PARTY_DEX.wrapping_add(rec)) as u16) < roll {
                        troll_bridge_toll(gm);
                        return;
                    }
                }
                i = i.wrapping_add(1);
                if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
            }
        }
        text::print_string(gm, 0x6ba0); // "Trolls evaded!\n"
    }
}
