//! TOWN.OVL (translated)
//!
//! The settlement (town / castle / keep / dwelling / lighthouse / hut) game loop. ROOT main calls
//! `enter_town` (vector 1778:02c6) then `town_main_loop` (vector 1778:02d2) whenever g_location is
//! 1..0x20. `load_town_map` reads one 32x32 floor of TOWNE/DWELLING/CASTLE/KEEP.DAT into g_map
//! (0x6608) using the per-location floor index table at DGROUP 0x1e19 and the file-name pointer
//! table 0x2652; `init_town_npcs` places the 32 NPCs from the save-game schedule block
//! (g_npc_schedules 0x5d5e, 16 bytes each) into the monster table, keeping a 16-byte NPC state
//! record per NPC at 0x5f5e (+0 active, +2 x, +4 y, +6 z, +0xa mode 0xfe/0xfd, +0xc monster slot,
//! +0xe schedule index). Per turn the loop reads a command (`read_town_command`, with drunken
//! stumbling), moves the party (`move_party_town`, leaving the map asks "Dost thou wish to
//! leave?" and sets g_location = 0 so the loop returns to ROOT main), passes time, applies tile
//! hazards (trapdoors, swamp poison, fire), closes timed doors, wanders horses, runs the NPC
//! overlay's movement (NPC b044) and resolves NPC contact (attacks, guard arrests via
//! `guard_arrest`, Blackthorn's dungeon via BLCKTHRN a89e). Shadowlord support: when a Shadowlord
//! is in the settlement (g_shadowlord_loc[i] == g_location) it is spawned as NPC tile 0xfc, the
//! map is blighted and townsfolk are made hostile (Hatred) or fearful (Cowardice).
//!
//! Ghidra split the body of `move_party_town` (87d0) into five extra "functions" (88bc, 88c8,
//! 88ea, 890c, 892e) because jump tables of the direction switch land in the middle of it; they
//! are translated as documented no-ops and their code lives in `move_party_town`.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// NPC state record offset (16 bytes per NPC at 0x5f5e).
#[inline] fn npc16(npc: i16) -> u16 { (npc as u16).wrapping_mul(0x10) }
/// Monster-table slot offset (8 bytes per slot at 0x5c5a).
#[inline] fn slot8(slot: i16) -> u16 { (slot as u16).wrapping_mul(8) }
/// 32-bit mask `1 << (n as u8)` computed the way the compiler did (two 16-bit words shifted
/// with carry, `(char)n` iterations). Returns (low word, high word).
#[inline] fn bit_mask32(n: i16) -> (u16, u16) {
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

/// orig 1000:81d0 (81 bytes). Returns 1 when NPC `npc` of the current settlement has been
/// killed: its type must be 0x0e or >= 0x40 (people, not animals/objects) and the NPC's bit in
/// the 32-bit "NPC killed" word of g_location (save 0x5b0 + 4*location, DGROUP 0x5b56) must be
/// set. Vector 1778:037a (CMDS, SJOG, TALK, CAST use it).
pub fn is_npc_killed(gm: &mut Game, param_1: i16) -> u16 {
    let npc = param_1;
    let t = gm.g8(g::NPC_TYPES.wrapping_add(npc as u16));
    if t == 0x0e || 0x3f < t {
        let (lo, hi) = bit_mask32(npc);
        let base = (gm.g8(g::LOCATION) as u16).wrapping_mul(4);
        if (gm.g16(base.wrapping_add(0x5b58)) & hi) != 0 || (gm.g16(base.wrapping_add(0x5b56)) & lo) != 0 {
            return 1;
        }
    }
    0
}

/// orig 1000:8222 (93 bytes). Marks NPC `npc` (0..31) as killed in the current settlement's
/// "NPC killed" bitmap (DGROUP 0x5b56 + 4*location) unless its type group (&0xfc) is 0x70
/// (guards) or >= 0x80 (monsters/objects, except 0xb4 which is flagged). Vector 1778:0386.
pub fn set_npc_killed(gm: &mut Game, param_1: i16) {
    let npc = param_1;
    if -1 < npc && npc < 0x20 {
        let t = gm.g8(g::NPC_TYPES.wrapping_add(npc as u16)) & 0xfc;
        if (t != 0x70 && t < 0x80) || t == 0xb4 {
            let (lo, hi) = bit_mask32(npc);
            let base = (gm.g8(g::LOCATION) as u16).wrapping_mul(4);
            let v = gm.g16(base.wrapping_add(0x5b56)) | lo;
            gm.set_g16(base.wrapping_add(0x5b56), v);
            let v = gm.g16(base.wrapping_add(0x5b58)) | hi;
            gm.set_g16(base.wrapping_add(0x5b58), v);
        }
    }
}

/// orig 1000:8280 (109 bytes). Removes NPC `npc` from the map: clears its state record (mode,
/// z, y, x, active), clears the first five bytes plus value2/value3 of its monster-table slot
/// (value1 at +5 is left alone), zeroes its three schedule AI types and its type byte, and
/// marks the view for an object rebuild (g_view_dirty |= 2). Vector 1778:0392.
pub fn remove_npc(gm: &mut Game, param_1: i16) {
    let npc = param_1;
    let n16 = npc16(npc);
    let slot = gm.gi16(0x5f6au16.wrapping_add(n16)); // g_npc_slot[npc]
    let s8 = slot8(slot);
    gm.set_g16(0x5f68u16.wrapping_add(n16), 0); // g_npc_mode
    gm.set_g16(0x5f64u16.wrapping_add(n16), 0); // g_npc_z
    gm.set_g16(0x5f62u16.wrapping_add(n16), 0); // g_npc_y
    gm.set_g16(0x5f60u16.wrapping_add(n16), 0); // g_npc_x
    gm.set_g16(g::NPC_STATES.wrapping_add(n16), 0);
    gm.set_g8(0x5c61u16.wrapping_add(s8), 0); // slot +7 value3
    gm.set_g8(0x5c60u16.wrapping_add(s8), 0); // slot +6 value2 (anim state)
    gm.set_g8(0x5c5eu16.wrapping_add(s8), 0); // slot +4 z
    gm.set_g8(0x5c5du16.wrapping_add(s8), 0); // slot +3 y
    gm.set_g8(0x5c5cu16.wrapping_add(s8), 0); // slot +2 x
    gm.set_g8(0x5c5bu16.wrapping_add(s8), 0); // slot +1 anim tile
    gm.set_g8(0x5c5au16.wrapping_add(s8), 0); // slot +0 tile
    gm.set_g8(0x5d60u16.wrapping_add(n16), 0); // schedule AI type 3
    gm.set_g8(0x5d5fu16.wrapping_add(n16), 0); // schedule AI type 2
    gm.set_g8(0x5d5eu16.wrapping_add(n16), 0); // schedule AI type 1
    gm.set_g8(g::NPC_TYPES.wrapping_add(npc as u16), 0);
    let v = gm.g8(g::VIEW_DIRTY) | 2;
    gm.set_g8(g::VIEW_DIRTY, v);
}

/// orig 1000:82ee (79 bytes). Finds the NPC (0..31) whose state record points at monster-table
/// slot `slot`, has a non-zero type and is active; returns its index or -1. Vector 1778:039e.
pub fn find_npc_by_slot(gm: &mut Game, param_1: i16) -> i16 {
    let slot = param_1;
    let mut i: i16 = 0;
    loop {
        let n16 = npc16(i);
        if gm.gi16(0x5f6au16.wrapping_add(n16)) == slot
            && gm.g8(g::NPC_TYPES.wrapping_add(i as u16)) != 0
            && gm.gi16(g::NPC_STATES.wrapping_add(n16)) != 0
        {
            return i;
        }
        i += 1;
        if 0x1f < i { return -1; }
    }
}

/// orig 1000:8340 (161 bytes). Day/night switch of the loaded floor: for every tile 0x87 the
/// tile below it (y+1) is XORed with 0xdd (toggles the lit/unlit variant), then, unless the party
/// stands on an annotation tile (0x48/0x49), every annotation cell recorded by `load_town_map` is
/// restored to its original tile at 05:00 or replaced by tile 3 otherwise (20:00). Marks the
/// view for a rebuild. Vector 1778:031a (CMDS, SHOPPES3); called at the 20:00/05:00 transitions.
pub fn toggle_night_tiles(gm: &mut Game) {
    let mut x: i16 = 0;
    loop {
        let mut y: i16 = 0;
        loop {
            let p = rest::get_map_tile(gm, y as u16, x as u16);
            if gm.gi8(p) == -0x79 {
                let q = rest::get_map_tile(gm, (y + 1) as u16, x as u16);
                let v = gm.g8(q) ^ 0xdd;
                gm.set_g8(q, v);
            }
            y += 1;
            if !(y < 0x20) { break; }
        }
        x += 1;
        if !(x < 0x20) { break; }
    }
    let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
    if (gm.g8(p) & 0xfe) != 0x48 {
        let mut i: u16 = 0;
        while i < gm.g8(g::ANNOTATION_COUNT) as u16 {
            let ay = gm.g8(g::ANNOTATION_Y.wrapping_add(i)) as u16;
            let ax = gm.g8(g::ANNOTATION_X.wrapping_add(i)) as u16;
            if gm.gi8(g::HOUR) == 5 {
                let q = rest::get_map_tile(gm, ay, ax);
                let t = gm.g8(g::ANNOTATION_TILE.wrapping_add(i));
                gm.set_g8(q, t);
            } else {
                let q = rest::get_map_tile(gm, ay, ax);
                gm.set_g8(q, 3);
            }
            i = i.wrapping_add(1);
        }
    }
    let v = gm.g8(g::VIEW_DIRTY) | 2;
    gm.set_g8(g::VIEW_DIRTY, v);
}

/// orig 1000:83e2 (154 bytes). Shadowlord blight: when a Shadowlord is present in this
/// settlement (g_shadowlord_present != -1) the random generator is seeded with the day number and
/// 7 of 8 tiles 0x2d ('-') become 0x2c and 7 of 8 tiles 0x2e ('.') become 0x2b over the whole
/// 32x32 map (so the blight pattern is stable within a day), the view is marked for a rebuild and
/// the generator is reseeded from the time hash.
pub fn blight_map_for_shadowlord(gm: &mut Game) {
    if gm.gi8(g::SAVE_UNK_3B2) != -1 { // g_shadowlord_present
        util::set_random_seed(gm, gm.g8(g::DAY) as u16);
        let mut row: i16 = 0;
        loop {
            let mut x: i16 = 0;
            loop {
                let a = g::MAP.wrapping_add((row + x) as u16);
                if gm.gi8(a) == 0x2d {
                    if util::rand_range(gm, 7, 0) != 0 {
                        gm.set_g8(a, 0x2c);
                    }
                } else if gm.gi8(a) == 0x2e && util::rand_range(gm, 7, 0) != 0 {
                    gm.set_g8(a, 0x2b);
                }
                x += 1;
                if !(x < 0x20) { break; }
            }
            row += 0x20;
            if !(row < 0x400) { break; }
        }
        let v = gm.g8(g::VIEW_DIRTY) | 2;
        gm.set_g8(g::VIEW_DIRTY, v);
        let h = util::get_time_hash(gm);
        util::set_random_seed(gm, h);
    }
}

/// orig 1000:847e (344 bytes). Decides whether a Shadowlord is in this settlement and spawns it.
/// g_shadowlord_present (save 0x3b2) is set to the index 0..2 whose g_shadowlord_loc equals
/// g_location (skipped when the party y is 4, i.e. just dumped in Castle British's cell by the
/// guards), else -1. If present: the map is blighted; if a monster slot already holds tile 0xfc
/// nothing more happens; otherwise a spawn slot is taken, the highest NPC index with type 0 (or
/// 31) becomes the Shadowlord: state active, slot linked, x = 15, y = DGROUP table 0x13a5[location],
/// z = 0, tile/anim 0xfc, schedule = three entries of AI type 6 at (15, y, floor 0) with zero
/// times, and NPC type 0xfc.
pub fn spawn_shadowlord(gm: &mut Game) {
    gm.set_gi8(g::SAVE_UNK_3B2, -1);
    if gm.gi8(g::PARTY_Y) != 4 {
        let mut i: i16 = 0;
        gm.set_gi8(g::SAVE_UNK_3B2, -1);
        loop {
            if gm.g8(g::SHADOWLORD_LOC.wrapping_add(i as u16)) == gm.g8(g::LOCATION) {
                gm.set_gi8(g::SAVE_UNK_3B2, i as i8);
                break;
            }
            i += 1;
            if !(i < 3) { break; }
        }
    }
    if gm.gi8(g::SAVE_UNK_3B2) != -1 {
        blight_map_for_shadowlord(gm);
        let mut p = g::MONSTER_TABLE;
        let mut i: i16 = 0;
        loop {
            if gm.gi8(p) == -4 { return; }
            p = p.wrapping_add(8);
            i += 1;
            if !(i < 0x20) { break; }
        }
        let slot = rest::find_spawn_slot(gm);
        let s8 = slot8(slot);
        // highest NPC index whose type is 0, or 31 when none is free
        let mut npc: i16;
        let mut j: i16 = 0x1f;
        loop {
            npc = j;
            if gm.g8(g::NPC_TYPES.wrapping_add(npc as u16)) == 0 { break; }
            j = npc - 1;
            npc = 0x1f;
            if !(-1 < j) { break; }
        }
        let n16 = npc16(npc);
        gm.set_g16(g::NPC_STATES.wrapping_add(n16), 1);
        gm.set_g16(0x5f6au16.wrapping_add(n16), slot as u16); // g_npc_slot
        gm.set_g8(0x5c5cu16.wrapping_add(s8), 0xf); // monster x
        gm.set_g16(0x5f60u16.wrapping_add(n16), 0xf); // g_npc_x
        let y = gm.g8(0x13a5u16.wrapping_add(gm.g8(g::LOCATION) as u16)); // shadowlord spawn y per location
        gm.set_g8(0x5c5du16.wrapping_add(s8), y); // monster y
        gm.set_g16(0x5f62u16.wrapping_add(n16), y as u16); // g_npc_y
        gm.set_g8(0x5c61u16.wrapping_add(s8), 0); // value3
        gm.set_g8(0x5c5eu16.wrapping_add(s8), 0); // z
        gm.set_g8(0x5c5fu16.wrapping_add(s8), 0); // value1
        gm.set_g8(0x5c60u16.wrapping_add(s8), 0); // value2
        gm.set_g16(0x5f64u16.wrapping_add(n16), 0); // g_npc_z
        gm.set_g8(0x5c5bu16.wrapping_add(s8), 0xfc); // anim tile
        gm.set_g8(0x5c5au16.wrapping_add(s8), 0xfc); // tile
        gm.set_g16(0x5d6au16.wrapping_add(n16), 0); // schedule times 1,2
        gm.set_g16(0x5d6cu16.wrapping_add(n16), 0); // schedule time 3, pad
        let mut k: u16 = 0;
        loop {
            let a = n16.wrapping_add(k);
            gm.set_g8(0x5d5eu16.wrapping_add(a), 6); // AI type
            gm.set_g8(0x5d61u16.wrapping_add(a), 0xf); // x
            let my = gm.g8(0x5c5du16.wrapping_add(s8));
            gm.set_g8(0x5d64u16.wrapping_add(a), my); // y
            gm.set_g8(0x5d67u16.wrapping_add(a), 0); // z
            k += 1;
            if !(k < 3) { break; }
        }
        gm.set_g8(g::NPC_TYPES.wrapping_add(npc as u16), 0xfc);
    }
}

/// orig 1000:85d8 (293 bytes). Loads the current floor of the settlement into g_map and
/// prepares it: sky strip frame/strip and wind line, no open door, chunk origin 0; the floor
/// index = DGROUP 0x1e19[location] + g_party_z (minus 0x100 for basements, z >= 0x80) selects the
/// 1 KB block of the map file named by g_map_dat_name_ptrs[(location-1)/8]. Then it scans the
/// map: tiles 0x48/0x49 are recorded in the annotation table (x, y, tile), the first '*' (0x2a)
/// becomes light beam 1 and any later '*' light beam 2 (-1 = none). Night tiles are applied when
/// the hour is < 5 or > 19, the light map is rebuilt, pass_time(0), the Shadowlord blight is
/// applied and, when `init_npcs` != 0, `init_town_npcs` places the NPCs. g_view_dirty = 1.
pub fn load_town_map(gm: &mut Game, param_1: i16) {
    let init_npcs = param_1;
    rest::draw_sky_strip_frame(gm);
    rest::draw_sky_strip(gm);
    rest::set_wind_and_draw(gm, -1);
    gm.set_g8(g::DOOR_TILE, 0);
    gm.set_g8(g::CHUNK_Y, 0);
    gm.set_g8(g::CHUNK_X, 0);
    let loc = gm.g8(g::LOCATION) as u16;
    let mut floor: i16 = (gm.g8(0x1e19u16.wrapping_add(loc)) as u16).wrapping_add(gm.g8(g::PARTY_Z) as u16) as i16;
    if 0x7f < gm.g8(g::PARTY_Z) {
        floor = floor.wrapping_sub(0x100);
    }
    let name = gm.g16((((loc as i16).wrapping_sub(1) >> 3) as u16).wrapping_mul(2).wrapping_add(0x2652));
    files::load_file_region(gm, (floor << 10) as u16, 0x400, g::MAP, name);
    gm.set_g8(g::ANNOTATION_COUNT, 0);
    gm.set_gi16(g::BEAM2_Y, -1);
    gm.set_gi16(g::BEAM2_X, -1);
    gm.set_gi16(g::BEAM1_Y, -1);
    gm.set_gi16(g::BEAM1_X, -1);
    let mut x: i16 = 0;
    loop {
        let mut y: i16 = 0;
        loop {
            let p = rest::get_map_tile(gm, y as u16, x as u16);
            if (gm.g8(p) & 0xfe) == 0x48 {
                let n = gm.g8(g::ANNOTATION_COUNT) as u16;
                gm.set_g8(g::ANNOTATION_X.wrapping_add(n), x as u8);
                gm.set_g8(g::ANNOTATION_Y.wrapping_add(n), y as u8);
                let q = rest::get_map_tile(gm, y as u16, x as u16);
                let t = gm.g8(q);
                gm.set_g8(g::ANNOTATION_TILE.wrapping_add(n), t);
                let c = gm.g8(g::ANNOTATION_COUNT).wrapping_add(1);
                gm.set_g8(g::ANNOTATION_COUNT, c);
            }
            let p = rest::get_map_tile(gm, y as u16, x as u16);
            if gm.gi8(p) == 0x2a {
                if gm.gi16(g::BEAM1_X) == -1 {
                    gm.set_gi16(g::BEAM1_X, x);
                    gm.set_gi16(g::BEAM1_Y, y);
                } else {
                    gm.set_gi16(g::BEAM2_X, x);
                    gm.set_gi16(g::BEAM2_Y, y);
                }
            }
            y += 1;
            if !(y < 0x20) { break; }
        }
        x += 1;
        if !(x < 0x20) { break; }
    }
    if gm.g8(g::HOUR) < 5 || 0x13 < gm.g8(g::HOUR) {
        toggle_night_tiles(gm);
    }
    rest::rebuild_light_map(gm);
    rest::pass_time(gm, 0);
    blight_map_for_shadowlord(gm);
    if init_npcs != 0 {
        init_town_npcs(gm);
    }
    gm.set_g8(g::VIEW_DIRTY, 1);
}

/// orig 1000:86fe (75 bytes). Ladder/stair handling after a step: if `tile` is one of the four
/// directional stair tiles 0xc4..0xc7 the view is refreshed and, when (tile - 0xc4) equals the
/// movement direction `dir` (0=N 1=E 2=S 3=W) the party goes up one floor ("Up!"), when it equals
/// dir^2 (the opposite direction) down one floor ("Down!"); any other combination does nothing.
/// The new floor is loaded with `load_town_map(1)`. Ghidra order: (tile, dir).
pub fn use_stairs(gm: &mut Game, param_1: i16, param_2: u16) {
    let (tile, dir) = (param_1, param_2);
    if (tile as u8 & 0xfc) == 0xc4 {
        rest::refresh_map_view(gm);
        let t = (tile as u16).wrapping_sub(0xc4);
        let s: u16;
        if t == dir {
            let z = gm.g8(g::PARTY_Z).wrapping_add(1);
            gm.set_g8(g::PARTY_Z, z);
            s = 0x265a; // "Up!\n"
        } else {
            if t != (dir ^ 2) { return; }
            let z = gm.g8(g::PARTY_Z).wrapping_sub(1);
            gm.set_g8(g::PARTY_Z, z);
            s = 0x265f; // "Down!\n"
        }
        text::print_string(gm, s);
        load_town_map(gm, 1);
    }
}

/// orig 1000:874c (130 bytes). Prints the vehicle verb and turns the vehicle sprite in a
/// settlement (dir 0=N 1=E 2=S 3=W): horse "Ride " (E -> 0x12, W -> 0x13), carpet "Fly "
/// (E -> 0x14, W -> 0x15); frigate (0x20/0x24) or skiff (0x28, prints "Row ") get base + dir.
pub fn turn_vehicle_town(gm: &mut Game, param_1: i16) {
    let dir = param_1;
    let base = gm.g8(g::TRANSPORT_TILE) & 0xfc;
    if base == 0x10 {
        text::print_string(gm, 0x2666); // "Ride "
        if dir == 1 {
            gm.set_g8(g::TRANSPORT_TILE, 0x12);
        } else if dir == 3 {
            gm.set_g8(g::TRANSPORT_TILE, 0x13);
        }
    } else if base == 0x14 {
        text::print_string(gm, 0x266c); // "Fly "
        if dir == 1 {
            gm.set_g8(g::TRANSPORT_TILE, 0x14);
        } else if dir == 3 {
            gm.set_g8(g::TRANSPORT_TILE, 0x15);
        }
    } else {
        if base != 0x20 && base != 0x24 {
            if base != 0x28 { return; }
            text::print_string(gm, 0x2671); // "Row "
        }
        let v = (dir as u8).wrapping_add(gm.g8(g::TRANSPORT_TILE) & 0xfc);
        gm.set_g8(g::TRANSPORT_TILE, v);
    }
}

/// orig 1000:87d0 (235 bytes + the fragments 88bc..8a2c). Moves the party one step in a
/// settlement; `dir` is the key code 1=W 2=E 3=N 4=S. Plays a footstep on foot/horse, turns the
/// vehicle and prints the direction name. Leaving the 32x32 map (x or y would leave 0..31) sets
/// the `leaving` flag. An object at the destination blocks unless it is boardable: from a frigate
/// or skiff (transport 0x20..0x2f) only an anchored frigate 0x24..0x27 from a skiff (>= 0x28);
/// otherwise ships/skiffs 0x24..0x2b, the lighthouse 0x1b, horses 0x10/0x11 and objects 0x1e/0x1f.
/// The destination view tile (g_view_tiles[5+dy][5+dx]) must pass can_enter_tile. If allowed and
/// not leaving: the party moves, the view is dirty, a footstep plays on horseback and
/// `use_stairs` checks for a directional stair. If leaving: "Dost thou wish to leave?" Y/N/ESC;
/// Y prints "Exit to Underworld!" (location 0x19 Ararat, z = 0xff) or "Exit to Britannia!" (z = 0),
/// clears the open door, puts the party at the location's overworld coordinates (tables 0x1e89 /
/// 0x1eb1 indexed by location) and sets g_location = 0; returns 1. Blocked: "Blocked!", beep,
/// keyboard flush. Returns 0 in every other case (the move or refusal used the turn).
pub fn move_party_town(gm: &mut Game, param_1: i16) -> u16 {
    let mut dir = param_1;
    if gm.g8(g::TRANSPORT_TILE) == 0x1c || (gm.g8(g::TRANSPORT_TILE) & 0xfe) == 0x12 {
        rest::play_footstep_sound(gm);
    }
    let mut leaving: u16 = 0;
    let mut dy: i16 = 0;
    let mut dx: i16 = 0;
    match dir {
        1 => {
            dx -= 1;
            if gm.g8(g::PARTY_X) < 1 { leaving = 1; }
            dir = 3;
            turn_vehicle_town(gm, 3);
            text::print_string(gm, 0x268a); // "West\n"
        }
        2 => {
            dx += 1;
            if 0x1e < gm.g8(g::PARTY_X) { leaving = 1; }
            dir = 1;
            turn_vehicle_town(gm, 1);
            text::print_string(gm, 0x2684); // "East\n"
        }
        3 => {
            dy -= 1;
            if gm.g8(g::PARTY_Y) < 1 { leaving = 1; }
            dir = 0;
            turn_vehicle_town(gm, 0);
            text::print_string(gm, 0x2676); // "North\n"
        }
        4 => {
            dy += 1;
            if 0x1e < gm.g8(g::PARTY_Y) { leaving = 1; }
            dir = 2;
            turn_vehicle_town(gm, 2);
            text::print_string(gm, 0x267d); // "South\n"
        }
        _ => {}
    }
    // LAB_1000_8839
    let mut passable = true;
    // g_view_tiles[5+dy][5+dx] (0xaba7 = 0xab02 + 5*0x20 + 5)
    let vt = gm.g8((dy as u16).wrapping_mul(0x20).wrapping_add(dx as u16).wrapping_add(0xaba7)) as u16;
    let obj = rest::find_object_at(
        gm,
        gm.g8(g::PARTY_Z) as u16,
        (dy as u16).wrapping_add(gm.g8(g::PARTY_Y) as u16),
        (dx as u16).wrapping_add(gm.g8(g::PARTY_X) as u16),
    ) as i16;
    if obj != 0 {
        passable = false;
        let tt = gm.g8(g::TRANSPORT_TILE);
        if tt < 0x30 && 0x1f < tt {
            // FUN_1000_892e fragment
            if 0x27 < tt && 0x23 < obj && obj < 0x28 {
                passable = true;
            }
        } else if (0x23 < obj && obj < 0x2c) || obj == 0x1b || (obj as u8 & 0xfe) == 0x10 || obj == 0x1e || obj == 0x1f {
            passable = true;
        }
    }
    // LAB_1000_8946
    if passable && rest::can_enter_tile(gm, vt as i16, gm.g8(g::TRANSPORT_TILE) as i16) != 0 {
        if leaving == 0 {
            // LAB_1000_89e0
            let v = gm.g8(g::PARTY_X).wrapping_add(dx as u8);
            gm.set_g8(g::PARTY_X, v);
            let v = gm.g8(g::PARTY_Y).wrapping_add(dy as u8);
            gm.set_g8(g::PARTY_Y, v);
            gm.set_g8(g::VIEW_DIRTY, 1);
            if (gm.g8(g::TRANSPORT_TILE) & 0xfe) == 0x12 {
                rest::play_footstep_sound(gm);
            }
            use_stairs(gm, vt as i16, dir as u16);
            return leaving;
        }
        text::print_string(gm, 0x2690); // "\nDost thou wish to leave? "
        let mut k: i16;
        loop {
            k = input::read_command_key(gm);
            if k == 0x59 || k == 0x4e { break; }
            if k == 0x1b { break; }
        }
        if k == 0x59 {
            text::print_string(gm, 0x26ab); // "Yes\n\nExit to\n"
            if gm.g8(g::LOCATION) == 0x19 {
                text::print_string(gm, 0x26b9); // "Underworld!\n"
                gm.set_g8(g::PARTY_Z, 0xff);
            } else {
                text::print_string(gm, 0x26c6); // "Britannia!\n"
                gm.set_g8(g::PARTY_Z, 0);
            }
            gm.set_g8(g::DOOR_TILE, 0);
            let loc = gm.g8(g::LOCATION) as u16;
            let x = gm.g8(0x1e89u16.wrapping_add(loc)); // overworld x of each location
            gm.set_g8(g::PARTY_X, x);
            let y = gm.g8(0x1eb1u16.wrapping_add(loc)); // overworld y of each location
            gm.set_g8(g::PARTY_Y, y);
            gm.set_g8(g::LOCATION, 0);
            return leaving;
        }
        text::print_string(gm, 0x26d2); // "No\n"
    } else {
        // LAB_1000_8a0a
        text::print_string(gm, 0x26d6); // "Blocked!\n"
        util::beep(gm, 200, 0xa5);
        input::flush_keyboard(gm);
    }
    0
}

/// orig 1000:88bc (11 bytes). Not a function: the tail of the object-boardable test inside
/// `move_party_town` (obj == 0x1f). Vector 1778:042e targets MAINOUT's 88bc, not TOWN's.
pub fn fn_1000_88bc(gm: &mut Game) -> u16 { 0 }

/// orig 1000:88c8 (33 bytes). Not a function: the `dir == 4` (South) arm of `move_party_town`.
pub fn fn_1000_88c8(gm: &mut Game) -> u16 { 0 }

/// orig 1000:88ea (33 bytes). Not a function: the `dir == 2` (East) arm of `move_party_town`.
pub fn fn_1000_88ea(gm: &mut Game) -> u16 { 0 }

/// orig 1000:890c (33 bytes). Not a function: the `dir == 1` (West) arm of `move_party_town`.
pub fn fn_1000_890c(gm: &mut Game) -> u16 { 0 }

/// orig 1000:892e (254 bytes). Not a function: the on-a-vessel boardable test and the shared
/// tail (can_enter_tile, move / leave prompt / Blocked!) of `move_party_town`.
pub fn fn_1000_892e(gm: &mut Game) -> u16 { 0 }

/// orig 1000:8a2e (118 bytes). Makes NPC `npc` hostile: its schedule times are zeroed and all
/// three AI types become 6 (NPC types < 0x2f, animals/monsters) or 7 (people).
pub fn make_npc_attack(gm: &mut Game, param_1: i16) {
    let npc = param_1;
    let ai: u8 = if gm.g8(g::NPC_TYPES.wrapping_add(npc as u16)) < 0x2f { 6 } else { 7 };
    let n16 = npc16(npc);
    gm.set_g16(0x5d6au16.wrapping_add(n16), 0); // schedule times
    gm.set_g16(0x5d6cu16.wrapping_add(n16), 0);
    let w = (ai as u16).wrapping_mul(0x100).wrapping_add(ai as u16);
    gm.set_g16(0x5d5eu16.wrapping_add(n16), w); // AI types 1,2
    gm.set_g8(0x5d60u16.wrapping_add(n16), w as u8); // AI type 3
}

/// orig 1000:8aa4 (132 bytes). Makes a townsperson flee: if NPC `npc` has a type in 0x40..0x73
/// and either its mode (state +0xa) is 0xfe or any of its four schedule time bytes is non-zero,
/// its mode becomes 0xfd and all three AI types become 3.
pub fn make_npc_flee(gm: &mut Game, param_1: i16) {
    let npc = param_1;
    let n16 = npc16(npc);
    let mut has_times = false;
    let mut i: u16 = 0;
    loop {
        if gm.gi8(0x5d6au16.wrapping_add(n16).wrapping_add(i)) != 0 {
            has_times = true;
        }
        i += 1;
        if !(i < 4) { break; }
    }
    let t = gm.g8(g::NPC_TYPES.wrapping_add(npc as u16));
    if t < 0x74 && 0x3f < t && (gm.g16(0x5f68u16.wrapping_add(n16)) == 0xfe || has_times) {
        gm.set_g16(0x5f68u16.wrapping_add(n16), 0xfd); // g_npc_mode
        gm.set_g16(0x5d5eu16.wrapping_add(n16), 0x303); // AI types 1,2
        gm.set_g8(0x5d60u16.wrapping_add(n16), 3); // AI type 3
    }
}

/// orig 1000:8b28 (98 bytes). Raises the alarm: every active NPC that is a Shadowlord (0xfc),
/// type 0xd8 or a guard (0x70) is made hostile; every other active NPC has a 50% chance of being
/// made to flee. Vector 1778:0356 (CMDS, SHOPPES3, TALK).
pub fn raise_alarm(gm: &mut Game) {
    let mut i: i16 = 0;
    let mut p = g::NPC_STATES;
    loop {
        if gm.gi16(p) != 0 {
            let t = gm.gi8(g::NPC_TYPES.wrapping_add(i as u16));
            if t == -4 || t == -0x28 || t == 0x70 {
                make_npc_attack(gm, i);
            } else if util::rand_range(gm, 0xff, 0) < 0x80 {
                make_npc_flee(gm, i);
            }
        }
        p = p.wrapping_add(0x10);
        i += 1;
        if !(i < 0x20) { break; }
    }
}

/// orig 1000:8b8c (42 bytes). Fights NPC `npc`: flags it killed, runs the combat for its monster
/// slot (ROOT fight_monster), removes the NPC, reloads the floor without re-placing NPCs and
/// re-evaluates the Shadowlord spawn.
pub fn fight_npc(gm: &mut Game, param_1: i16) {
    let npc = param_1;
    set_npc_killed(gm, npc);
    let slot = gm.gi16(0x5f6au16.wrapping_add(npc16(npc)));
    rest::fight_monster(gm, slot);
    remove_npc(gm, npc);
    load_town_map(gm, 0);
    spawn_shadowlord(gm);
}

/// orig 1000:8bb6 (407 bytes). The Attack command in a settlement (ROOT dispatch_command 'A').
/// "Attack-"; on a water tile (< 4) while not on foot: "On foot!" (returns 0, no turn). A
/// direction is read (0 -> returns 1). A mirror (0x9d) at the target is broken (tile 0x9f,
/// "Broken!", rising random tones). The object there (find_object_at, slot in g_dx ->
/// find_npc_by_slot) is attackable when its tile is >= 0x40, not a whirlpool-group 0xe8..0xef and
/// not group 0xb4; otherwise "Nothing to attack!". Attacking a person (< 0x80) costs 5 karma and
/// raises the alarm; group 0xd8 raises the alarm without the karma loss. If the target stands
/// on a bed/sleep tile (0x84, 0x85, 0x9f or 0xab) it is "Murdered!" (another 5 karma, tile flash,
/// NPC flagged killed and removed) unless it is tile 0x78 ("Missed!"); otherwise combat starts
/// via `fight_npc`. Returns 1 (a turn passes).
pub fn cmd_attack_town(gm: &mut Game) -> u16 {
    text::print_string(gm, 0x26e0); // "Attack-"
    let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
    if gm.g8(p) < 4 && gm.g8(g::TRANSPORT_TILE) != 0x1c {
        text::print_string(gm, 0x26e8); // "On foot!\n"
        return 0;
    }
    if rest::get_direction(gm) == 0 {
        return 1;
    }
    let x = (gm.g8(g::PARTY_X) as u16).wrapping_add(gm.g16(g::DX)) as i16;
    let y = (gm.g8(g::PARTY_Y) as u16).wrapping_add(gm.g16(g::DY)) as i16;
    let p = rest::get_map_tile(gm, y as u16, x as u16);
    if gm.gi8(p) == -99 {
        let q = rest::get_map_tile(gm, y as u16, x as u16);
        gm.set_g8(q, 0x9f);
        text::print_string(gm, 0x26f2); // "Broken!\n"
        let mut f: i16 = 2000;
        loop {
            util::play_random_tones(gm, f, 0x78, 0x28);
            f += 1000;
            if !(f < 20000) { break; }
        }
        let v = gm.g8(g::VIEW_DIRTY) | 2;
        gm.set_g8(g::VIEW_DIRTY, v);
        return 1;
    }
    let obj = rest::find_object_at(gm, gm.g8(g::PARTY_Z) as u16, y as u16, x as u16) as i16;
    let mut nothing = true;
    let mut npc: i16 = 0; // only meaningful when obj != 0 (uninitialised in the original)
    if obj != 0 {
        npc = find_npc_by_slot(gm, gm.gi16(g::DX));
        if 0x3f < obj && (obj < 0xe8 || 0xef < obj) && (obj as u8 & 0xfc) != 0xb4 {
            nothing = false;
        }
    }
    if nothing {
        text::print_string(gm, 0x26fb); // "Nothing to attack!\n"
        return 1;
    }
    if obj < 0x80 {
        rest::sub_u8_floor(gm, 5, g::KARMA);
        raise_alarm(gm);
    } else if (obj as u8 & 0xfc) == 0xd8 {
        raise_alarm(gm);
    }
    let p = rest::get_map_tile(gm, y as u16, x as u16);
    let t = gm.g8(p);
    if t < 0x84 || (0x85 < t && t != 0x9f && t != 0xab) {
        if npc < 0 { return 1; }
        set_npc_killed(gm, npc);
        fight_npc(gm, npc);
        return 1;
    }
    if obj != 0x78 {
        text::print_string(gm, 0x2718); // "Murdered!\n"
        rest::sub_u8_floor(gm, 5, g::KARMA);
        rest::flash_tile_at(gm, y, x);
        if npc < 0 { return 1; }
        set_npc_killed(gm, npc);
        remove_npc(gm, npc);
        return 1;
    }
    text::print_string(gm, 0x270f); // "Missed!\n"
    1
}

/// orig 1000:8d52 (195 bytes). The Klimb command in a settlement (ROOT dispatch_command 'K',
/// vector 1778:034a). "Klimb-"; on horseback "-On foot!" (returns 0). Standing on a ladder tile
/// the party changes floor through `use_stairs`: 0x86 and 0xc9 go down, 0xc8 goes up (returns 1).
/// Otherwise a direction is read; the target must be tile 'L' (0x4c), 0xca or 0xcb else "What?"
/// (returns 0); the party steps onto it, the view is dirty and refreshed. Returns 1.
pub fn cmd_klimb_town(gm: &mut Game) -> i16 {
    let mut result: i16 = 0;
    text::print_string(gm, 0x2723); // "Klimb-"
    if (gm.g8(g::TRANSPORT_TILE) & 0xfe) == 0x12 {
        text::print_string(gm, 0x272a); // "-On foot!\n"
    } else {
        let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
        let t = gm.gi8(p);
        if t == -0x7a {
            use_stairs(gm, 0xc4, 2);
            result = 1;
        } else if t == -0x38 {
            use_stairs(gm, 0xc4, 0);
            result = 1;
        } else if t == -0x37 {
            use_stairs(gm, 0xc4, 2);
            result = 1;
        }
        if result != 0 {
            return result;
        }
        if rest::get_direction(gm) != 0 {
            let p = rest::get_map_tile(
                gm,
                (gm.g8(g::PARTY_Y) as u16).wrapping_add(gm.g16(g::DY)),
                (gm.g8(g::PARTY_X) as u16).wrapping_add(gm.g16(g::DX)),
            );
            let t = gm.gi8(p);
            if t != 0x4c && t != -0x36 && t != -0x35 {
                text::print_string(gm, 0x2735); // "What?\n"
                return result;
            }
            let v = gm.g8(g::PARTY_X).wrapping_add(gm.g16(g::DX) as u8);
            gm.set_g8(g::PARTY_X, v);
            let v = gm.g8(g::PARTY_Y).wrapping_add(gm.g16(g::DY) as u8);
            gm.set_g8(g::PARTY_Y, v);
            gm.set_g8(g::VIEW_DIRTY, 1);
            rest::refresh_map_view(gm);
        }
        result = 1;
    }
    result
}

/// orig 1000:8e1a (46 bytes). Returns 1 when the map tile at (y, x) is 0xa2 or 'C' (0x43), the
/// tiles a horse stays hitched next to. Ghidra order: (y, x).
pub fn is_hitching_tile(gm: &mut Game, param_1: u16, param_2: u16) -> u16 {
    let (y, x) = (param_1, param_2);
    let p = rest::get_map_tile(gm, y, x);
    if gm.gi8(p) != -0x5e {
        let p = rest::get_map_tile(gm, y, x);
        if gm.gi8(p) != 0x43 {
            return 0;
        }
    }
    1
}

/// orig 1000:8e48 (329 bytes). Lets loose horses wander: every monster slot holding a horse
/// (tile 0x10/0x11) on the party's floor has a 50% chance per turn to move, provided none of the
/// four neighbouring map tiles is a hitching tile. A random axis and sign is chosen (a horizontal
/// move also sets the facing tile: 0x11 when moving west, 0x10 east); the target must be inside
/// 0..31, enterable by a horse (can_enter_tile with mover 0x10) and free of objects
/// (find_object_at_rev); then the slot's tiles and position are updated and the view marked.
pub fn wander_horses(gm: &mut Game) {
    let mut p = g::MONSTER_TABLE;
    let mut i: i16 = 0;
    loop {
        let mut t = gm.g8(p);
        if (t & 0xfe) == 0x10 && gm.g8(p.wrapping_add(4)) == gm.g8(g::PARTY_Z) && util::rand_range(gm, 1, 0) == 0 {
            let mut x = gm.g8(p.wrapping_add(2)) as u16;
            let mut y = gm.g8(p.wrapping_add(3)) as u16;
            if is_hitching_tile(gm, y.wrapping_add(1), x) == 0
                && is_hitching_tile(gm, y, x.wrapping_add(1)) == 0
                && is_hitching_tile(gm, y.wrapping_sub(1), x) == 0
                && is_hitching_tile(gm, y, x.wrapping_sub(1)) == 0
            {
                if util::rand_range(gm, 1, 0) == 0 {
                    let r = util::rand_range(gm, 1, 0);
                    y = y.wrapping_add((r * 2 - 1) as u16);
                } else {
                    let r = util::rand_range(gm, 1, 0) * 2 - 1;
                    x = x.wrapping_add(r as u16);
                    t = if r < 1 { 0x11 } else { 0x10 };
                }
                if (x as i16) < 0x20 && (y as i16) < 0x20 && -1 < (x as i16) && -1 < (y as i16) {
                    let q = rest::get_map_tile(gm, y, x);
                    let mt = gm.g8(q) as i16;
                    if rest::can_enter_tile(gm, mt, 0x10) != 0
                        && rest::find_object_at_rev(gm, gm.g8(g::PARTY_Z) as u16, y, x) == 0
                    {
                        gm.set_g8(p.wrapping_add(1), t);
                        gm.set_g8(p, t);
                        gm.set_g8(p.wrapping_add(2), x as u8);
                        gm.set_g8(p.wrapping_add(3), y as u8);
                        let v = gm.g8(g::VIEW_DIRTY) | 2;
                        gm.set_g8(g::VIEW_DIRTY, v);
                    }
                }
            }
        }
        p = p.wrapping_add(8);
        i += 1;
        if !(i < 0x20) { break; }
    }
}

/// orig 1000:8f94 (111 bytes). Reads the next command in a settlement. When `redraw` != 0 the
/// map view is refreshed, the status window redrawn if dirty, a newline and the prompt cap are
/// printed. Then read_command_key; while drunk (g_drunk_moves != 0) there is a 50% chance that
/// the alarm is raised, the drunk counter decremented, "Hic!" printed and the key replaced by a
/// random direction key from the 4-byte table at 0x2742 (3,4,2,1).
pub fn read_town_command(gm: &mut Game, param_1: i16) -> u16 {
    let redraw = param_1;
    if redraw != 0 {
        rest::refresh_map_view(gm);
        if gm.g8(g::STATUS_DIRTY) != 0 {
            rest::draw_status_window(gm);
            gm.set_g8(g::STATUS_DIRTY, 0);
        }
        text::put_char(gm, 10);
        rest::draw_bar_left_cap(gm);
    }
    let mut key = input::read_command_key(gm) as u16;
    if gm.g8(g::DRUNK_MOVES) != 0 {
        if util::rand_range(gm, 1, 0) != 0 {
            raise_alarm(gm);
            let v = gm.g8(g::DRUNK_MOVES).wrapping_sub(1);
            gm.set_g8(g::DRUNK_MOVES, v);
            text::print_string(gm, 0x273c); // "Hic!\n"
            let r = util::rand_range(gm, 3, 0);
            key = gm.g8((r as u16).wrapping_add(0x2742)) as u16;
        }
    }
    key
}

/// orig 1000:9004 (203 bytes). A digit key '0'..'9' in a settlement. If the view tile directly
/// south of the party (g_view_tiles[6][5], 0xabc7) is the harpsichord (0x8d) the digit plays a
/// note (frequency word table 0x2746[n], sound permitting) and advances the 13-note melody
/// matcher at 0x2767 against the note sequence 0x275a..0x2766 (6 7 8 9 8 7 8 7 6 7 6 5 3): a
/// correct note increments the position; completing all 13 resets it and, in Castle British
/// (0x11) on floor 2, toggles map byte 0x67b9 (g_map[13][17]) with XOR 0xb, shakes the screen and
/// dirties the view (the secret passage). Wrong notes fall back: position 10 with '8' -> 3,
/// position 11 with '7' -> 2, the first melody note -> 1, anything else -> 0. Returns 3 (no turn,
/// re-prompt). Otherwise the digit selects the active player (ROOT set_active_player) and its
/// result is returned.
pub fn handle_number_key(gm: &mut Game, param_1: i16) -> u16 {
    let key = param_1;
    let mut result: u16 = 3;
    if gm.gi8(0xabc7) == -0x73 { // g_view_tiles[6][5]: tile south of the party
        let n = (key as u16).wrapping_sub(0x30);
        if gm.g8(g::SOUND_ENABLED) != 0 {
            let f = gm.g16(n.wrapping_mul(2).wrapping_add(0x2746)); // note frequency table
            util::play_sweep_noise(gm, 0xfffc, 0x4e20, 4000, 1, f);
        }
        let pos = gm.g8(0x2767) as u16; // melody position
        if gm.g8(0x275au16.wrapping_add(pos)) as u16 == n {
            let np = gm.g8(0x2767).wrapping_add(1);
            gm.set_g8(0x2767, np);
            if np == 0xd {
                gm.set_g8(0x2767, 0);
                if gm.g8(g::LOCATION) == 0x11 && gm.g8(g::PARTY_Z) == 2 {
                    let v = gm.g8(0x67b9) ^ 0xb; // g_map[13][17]
                    gm.set_g8(0x67b9, v);
                    rest::earthquake_shake(gm);
                    gm.set_g8(g::VIEW_DIRTY, 1);
                }
            }
        } else if pos == 10 && n == 8 {
            gm.set_g8(0x2767, 3);
        } else if pos == 0xb && n == 7 {
            gm.set_g8(0x2767, 2);
        } else if n == gm.g8(0x275a) as u16 {
            gm.set_g8(0x2767, 1);
        } else {
            gm.set_g8(0x2767, 0);
        }
    } else {
        result = rest::set_active_player(gm, key);
    }
    result
}

/// orig 1000:90d2 (468 bytes). End-of-turn tile effects in a settlement. Each sleeping member
/// ('S') wakes with chance 1/16. Then, repeating while the party keeps falling: a trapdoor
/// (0x8c, not on a carpet) prints "A TRAPDOOR!", damages the whole party as if on foot and either
/// (Stonegate, 0x1d) blanks the map window, plays a falling tone 1000..251, fills the map with
/// tile 0x8f, clears the monster table and kills every member (HP 0, status 'D', tones, status
/// redraw) or drops the party one floor (`load_town_map(1)`) and re-checks. Swamp (tile 4) on foot
/// poisons each living, unpoisoned member whose dexterity is below rand(0..29) ("Poisoned!").
/// Fire/lava (0xbc, 0x8f): "Burning!" and whole-party damage. Finally ROOT
/// end_of_turn_party_update.
pub fn apply_tile_hazards(gm: &mut Game) {
    if gm.g8(g::PARTY_COUNT) != 0 {
        let mut p = g::PARTY_STATUS;
        let mut i: u16 = 0;
        loop {
            if gm.gi8(p) == 0x53 && util::rand_range(gm, 0xf, 0) == 0xf {
                gm.set_g8(p, 0x47);
            }
            p = p.wrapping_add(0x20);
            i += 1;
            if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
        }
    }
    loop {
        let mut again = false;
        let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
        let t = gm.gi8(p);
        if t == -0x74 && (gm.g8(g::TRANSPORT_TILE) & 0xfe) != 0x14 {
            text::print_string(gm, 0x2768); // "A TRAPDOOR!\n"
            let saved = gm.g8(g::TRANSPORT_TILE);
            gm.set_g8(g::TRANSPORT_TILE, 0);
            rest::refresh_map_view(gm);
            rest::damage_whole_party(gm);
            gm.set_g8(g::TRANSPORT_TILE, saved);
            if gm.g8(g::LOCATION) == 0x1d {
                gfx::set_color(gm, 0);
                gfx::fill_rect(gm, 0xb7, 0xb7, 8, 8);
                let mut f: u16 = 1000;
                loop {
                    util::start_tone(gm, f);
                    util::delay_loop(gm, 0x28, 1);
                    f = f.wrapping_sub(1);
                    if !(0xfa < f as i16) { break; }
                }
                util::stop_tone(gm);
                let mut a = g::MAP;
                for _ in 0..0x400 {
                    gm.set_g8(a, 0x8f);
                    a = a.wrapping_add(1);
                }
                gm.set_g8(g::VIEW_DIRTY, 1);
                let mut a = g::MONSTER_TABLE;
                for _ in 0..0x100 {
                    gm.set_g8(a, 0);
                    a = a.wrapping_add(1);
                }
                let mut i: u16 = 0;
                if gm.g8(g::PARTY_COUNT) != 0 {
                    let mut hp = g::PARTY_HP;
                    let mut st = g::PARTY_STATUS;
                    loop {
                        gm.set_g16(hp, 0);
                        gm.set_g8(st, 0x44);
                        util::play_random_tones(gm, 500, 3000, 0x28);
                        rest::draw_status_window(gm);
                        hp = hp.wrapping_add(0x20);
                        st = st.wrapping_add(0x20);
                        i += 1;
                        if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
                    }
                }
            } else {
                let z = gm.g8(g::PARTY_Z).wrapping_sub(1);
                gm.set_g8(g::PARTY_Z, z);
                load_town_map(gm, 1);
                again = true;
            }
        } else if t == 4 && gm.g8(g::TRANSPORT_TILE) == 0x1c {
            let mut i: u16 = 0;
            while i < gm.g8(g::PARTY_COUNT) as u16 {
                let r = i.wrapping_mul(0x20);
                let st = gm.g8(g::PARTY_STATUS.wrapping_add(r));
                if st != 0x44 && st != 0x50 {
                    let rr = util::rand_range(gm, 0x1d, 0) as u16;
                    if (gm.g8(0x55b5u16.wrapping_add(r)) as u16) < rr { // g_party_dex
                        text::print_string(gm, 0x2775); // "Poisoned!\n"
                        gm.set_g8(g::PARTY_STATUS.wrapping_add(r), 0x50);
                        rest::draw_status_window(gm);
                    }
                }
                i += 1;
            }
        } else if t == -0x44 || t == -0x71 {
            rest::refresh_map_view(gm);
            text::print_string(gm, 0x2780); // "Burning!\n"
            rest::damage_whole_party(gm);
        }
        if !again { break; }
    }
    rest::end_of_turn_party_update(gm);
}

/// orig 1000:92aa (23 bytes). A Shadowlord's "Begone, vermin!": prints it, damages the whole
/// party and makes NPC `npc` (the Shadowlord's own record) flee-mode. Also called by TALK.
pub fn begone_vermin(gm: &mut Game, param_1: u16) {
    let npc = param_1;
    text::print_string(gm, 0x278a); // "\"Begone,\nvermin!\"\n"
    rest::damage_whole_party(gm);
    make_npc_flee(gm, npc as i16);
}

/// orig 1000:92c2 (100 bytes). Whether NPC `npc` is affected by a Shadowlord's influence: 1 when
/// any of its four schedule time bytes is non-zero, cancelled when the NPC type is outside
/// 0x40..0x73 and with a further 50% chance. NOTE: the original indexes g_npc_types with the
/// loop counter (always 4) instead of `npc` - a compiler/source bug kept for fidelity.
pub fn npc_affected_by_shadowlord(gm: &mut Game, param_1: i16) -> u16 {
    let npc = param_1;
    let n16 = npc16(npc);
    let mut result: u16 = 0;
    let mut i: u16 = 0;
    loop {
        if gm.gi8(0x5d6au16.wrapping_add(n16).wrapping_add(i)) != 0 {
            result = 1;
        }
        i += 1;
        if !(i < 4) { break; }
    }
    // TODO(verify): original reads g_npc_types[4] (BX = loop counter), not g_npc_types[npc]
    let t = gm.g8(g::NPC_TYPES.wrapping_add(i));
    if t < 0x40 || 0x73 < t {
        result = 0;
    }
    if util::rand_range(gm, 1, 0) != 0 {
        result = 0;
    }
    result
}

/// orig 1000:9326 (96 bytes). Applies the present Shadowlord's influence on entering: Hatred
/// (index 1) makes every affected NPC hostile with mode 0xfe; Cowardice (index 2) makes them
/// flee with mode 0xfd. Falsehood (0) has no effect here.
pub fn shadowlord_influence_npcs(gm: &mut Game) {
    if gm.gi8(g::SAVE_UNK_3B2) == 1 {
        let mut i: i16 = 0;
        let mut p: u16 = 0x5f68; // g_npc_mode
        loop {
            if npc_affected_by_shadowlord(gm, i) != 0 {
                make_npc_attack(gm, i);
                gm.set_g16(p, 0xfe);
            }
            p = p.wrapping_add(0x10);
            i += 1;
            if !(i < 0x20) { break; }
        }
    } else if gm.gi8(g::SAVE_UNK_3B2) == 2 {
        let mut i: i16 = 0;
        let mut p: u16 = 0x5f68;
        loop {
            if npc_affected_by_shadowlord(gm, i) != 0 {
                make_npc_flee(gm, i);
                gm.set_g16(p, 0xfd);
            }
            p = p.wrapping_add(0x10);
            i += 1;
            if !(i < 0x20) { break; }
        }
    }
}

/// orig 1000:9388 (56 bytes). Prints "An air of <falsehood|hatred|cowardice> doth surround
/// thee..." (name pointer table g_shadowlord_name_ptrs[idx]) with an ominous sweep sound.
pub fn print_air_of(gm: &mut Game, param_1: i16) {
    let idx = param_1;
    text::print_string(gm, 0x27b8); // "\nAn air of\n"
    let s = gm.g16((idx as u16).wrapping_mul(2).wrapping_add(g::SHADOWLORD_NAME_PTRS));
    text::print_string(gm, s);
    text::print_string(gm, 0x27c4); // " doth surround thee...\n"
    util::play_sweep_noise(gm, 1, 2000, 60000, 1, 0x19c8);
}

/// orig 1000:93c0 (188 bytes). Enters a settlement (vector 1778:02c6; ROOT main, moongate
/// arrival, guard arrest, BLCKTHRN resurrection). Sets g_view_dirty and the "just entered" flag
/// 0xa9bc (skips the moongate check on the first loop pass). When `arrived` != 0 (coming from
/// outside rather than resuming a saved game inside): monster slots 1..31 get tile 0, drunkenness
/// cleared, map updates enabled, NPC overlay reset (NPC a290) and NPC schedules evaluated for the
/// current hour (NPC a366). Then g_shadowlord_present = -1, `load_town_map(arrived)`,
/// `spawn_shadowlord`, every killed NPC is removed, in Stonegate (0x1d) NPC 9 is removed once the
/// sceptre is owned, the status window is drawn and, if anyone is conscious, the view refreshed
/// and the Shadowlord presence announced: in Stonegate one line per Shadowlord still alive
/// (g_shadowlord_loc < 0x80), elsewhere the present one plus its influence on the NPCs.
pub fn enter_town(gm: &mut Game, param_1: i16) {
    let arrived = param_1;
    gm.set_g8(g::VIEW_DIRTY, 1);
    gm.set_g8(0xa9bc, 1); // g_just_entered
    if arrived != 0 {
        let mut p = g::MONSTER_SLOT1;
        for _ in 0..0x1f {
            gm.set_g8(p, 0);
            p = p.wrapping_add(8);
        }
        gm.set_g8(g::DRUNK_MOVES, 0);
        gm.set_g8(g::MAP_UPDATE_ENABLED, 1);
        ovl::npc::load_npc_schedules(gm);
        ovl::npc::init_npc_states(gm, gm.g8(g::HOUR) as u16);
    }
    gm.set_gi8(g::SAVE_UNK_3B2, -1);
    load_town_map(gm, arrived);
    spawn_shadowlord(gm);
    let mut i: i16 = 0;
    loop {
        if is_npc_killed(gm, i) != 0 {
            remove_npc(gm, i);
        }
        i += 1;
        if !(i < 0x20) { break; }
    }
    if gm.g8(g::LOCATION) == 0x1d && gm.g8(g::SCEPTRE) != 0 {
        remove_npc(gm, 9);
    }
    rest::draw_status_window(gm);
    let m = rest::find_first_conscious_member(gm) as i16;
    if -1 < m {
        rest::refresh_map_view(gm);
        if gm.g8(g::LOCATION) == 0x1d {
            let mut i: i16 = 2;
            loop {
                if gm.g8(g::SHADOWLORD_LOC.wrapping_add(i as u16)) < 0x80 {
                    print_air_of(gm, i);
                }
                i -= 1;
                if !(-1 < i) { break; }
            }
        } else if gm.gi8(g::SAVE_UNK_3B2) != -1 {
            print_air_of(gm, gm.gi8(g::SAVE_UNK_3B2) as i16);
            shadowlord_influence_npcs(gm);
        }
    }
}

/// orig 1000:947e (162 bytes). A guard arrests the party. In Castle Blackthorn (0x12): if nobody
/// is conscious returns 0, else the BLCKTHRN dungeon scene (vector 1778:033e) runs. Elsewhere:
/// "Thou art under arrest!" "Wilt thou come quietly?" Y/N; N: "Then defend thyself, rogue!",
/// the alarm is raised and 1 is returned (fight); Y: "The guard strikes thee unconscious!",
/// blank colour, "Thou dost awaken to...", the party is put in the Yew jail (location 4,
/// x 25, y 4), time passes in 20-minute steps until 08:00, keys and floor are cleared. Both paths
/// end with `enter_town(1)` and return 0.
pub fn guard_arrest(gm: &mut Game) -> u16 {
    if gm.g8(g::LOCATION) == 0x12 {
        let m = rest::find_first_conscious_member(gm) as i16;
        if m < 0 {
            return 0;
        }
        ovl::blckthrn::blackthorn_dungeon_scene(gm);
    } else {
        text::print_string(gm, 0x27e1 + 1); // "\n\"Thou art under arrest!\"\n\n"
        text::print_string(gm, 0x27fe); // "\"Wilt thou come quietly?\"\n\n:"
        let mut c: i8;
        loop {
            c = input::read_command_key(gm) as i8;
            if c == 0x4e { break; }
            if c == 0x59 { break; }
        }
        if c != 0x59 {
            text::print_string(gm, 0x285e); // "No\n\n\"Then defend thyself, rogue!\"\n"
            raise_alarm(gm);
            return 1;
        }
        text::print_string(gm, 0x281b); // "Yes\n\nThe guard strikes thee unconscious!\n"
        gfx::set_color(gm, 0);
        text::print_string(gm, 0x2845); // "\nThou dost awaken to...\n"
        gm.set_g8(g::LOCATION, 4);
        gm.set_g8(g::PARTY_X, 0x19);
        gm.set_g8(g::PARTY_Y, 4);
        gm.set_g8(g::VIEW_DIRTY, 1);
        while gm.g8(g::HOUR) != 8 {
            rest::pass_time(gm, 0x14);
        }
        gm.set_g8(g::KEYS, 0);
        gm.set_g8(g::PARTY_Z, 0);
    }
    enter_town(gm, 1);
    0
}

/// orig 1000:9522 (202 bytes). Resolves contact with the NPC the NPC overlay reported in
/// 0x65bf (g_npc_contact_idx) with kind 0x65be (g_npc_contact_kind): kind 'a' (the NPC wants to
/// attack): a Shadowlord-influenced NPC (mode 0xfe) shouts "Begone, vermin!"; a non-guard
/// (monster tile != 0x70) attacks; a guard arrests (`guard_arrest`). Otherwise, when `cmd_minus1`
/// == 0 (a normal turn): nothing unless the NPC has a mode and TALK c29e (the guard/NPC reaction
/// dialogue) returns non-zero, then arrest; when `cmd_minus1` != 0 (the command itself demanded
/// it, result 2) arrest directly. If an attack results: an NPC with monster tile < 0x40 is simply
/// removed, else "Attacked!" and `fight_npc`.
pub fn handle_npc_contact(gm: &mut Game, param_1: i16) {
    let cmd_minus1 = param_1;
    let mut attack: i16 = 0;
    let npc = gm.g8(0x65bf) as u16; // g_npc_contact_idx
    let n16 = npc.wrapping_mul(0x10);
    let slot = gm.gi16(0x5f6au16.wrapping_add(n16));
    rest::refresh_map_view(gm);
    let mut arrest = true;
    if gm.gi8(g::SAVE_UNK_1018) == 0x61 { // g_npc_contact_kind 'a'
        if gm.g16(0x5f68u16.wrapping_add(n16)) == 0xfe {
            text::put_char(gm, 10);
            begone_vermin(gm, npc);
            arrest = false;
        } else if gm.gi8(g::MONSTER_TABLE.wrapping_add(slot8(slot))) != 0x70 {
            attack = 1;
            arrest = false;
        }
    } else if cmd_minus1 == 0 {
        if gm.g16(0x5f68u16.wrapping_add(n16)) == 0 {
            arrest = false;
        } else if ovl::talk::talk_to_npc(gm, npc as i16) == 0 {
            arrest = false;
        }
    }
    if arrest {
        attack = guard_arrest(gm) as i16;
    }
    // LAB_1000_95ac
    if attack != 0 {
        let slot = gm.gi16(0x5f6au16.wrapping_add(n16));
        if gm.g8(g::MONSTER_TABLE.wrapping_add(slot8(slot))) < 0x40 {
            remove_npc(gm, npc as i16);
        } else {
            text::print_string(gm, 0x2881); // "\nAttacked!\n"
            fight_npc(gm, npc as i16);
        }
    }
}

/// orig 1000:95ee (576 bytes). The settlement main loop (vector 1778:02d2, ROOT main). Each
/// pass: if only sleeping members remain, "Zzzzzz..."; if everybody is dead the BLCKTHRN
/// resurrection scene (vector 1778:02de) runs with no turn; otherwise the moongate check (skipped
/// once after entering, flag 0xa9bc; a moongate trip runs CAST2 f056) and, if g_location became 0,
/// return. Commands are read with `read_town_command` (first with redraw) until the result is
/// not 3: keys >= 0x20 go to ROOT dispatch_command, digits to `handle_number_key`; 1..4 move
/// (`move_party_town`, a 1 result means the party left); 5 (^E) asks "Exit to DOS?" (Y restores
/// the video mode and exits); 0xb (^K) prints the karma; 0x13 (^S) toggles sound; 0x16 (^V)
/// prints the version "1.16"; anything else "What?". After a turn-consuming command with a
/// conscious party: pass_time(1), night tiles at 20:00/05:00, `apply_tile_hazards`, the open
/// door closes when its timer runs out, the party slot copies x/y/z, and unless time is stopped
/// ('T') - and only every other turn on horseback (keys other than space) or with Quickness ('Q')
/// - horses wander, NPCs move (NPC b044, not when the command returned 2+) and NPC contact is
/// resolved (`handle_npc_contact(result-1)`) when one was reported or the result was 2. Leaves
/// when g_location is 0 (exit to the overworld, the cell transfer or an arrest elsewhere).
pub fn town_main_loop(gm: &mut Game) {
    let mut mount_toggle = false;
    let mut quick_toggle = false;
    loop {
        let mut leave: i16 = 0;
        let mut cmd: u16 = 1;
        let mut key: u16 = 0; // TODO(verify): uninitialised in the original when the party is asleep/dead
        let m = rest::find_first_conscious_member(gm) as i16;
        if m == 1 {
            text::put_char(gm, 10);
            rest::draw_bar_left_cap(gm);
            text::print_string(gm, 0x288d); // "Zzzzzz...\n"
        } else if m == -1 {
            ovl::blckthrn::resurrection_scene(gm);
            cmd = 0;
        } else {
            if gm.g8(0xa9bc) == 0 { // g_just_entered
                if rest::enter_moongate(gm) != 0 {
                    ovl::cast2::visit_shrine(gm);
                }
            } else {
                gm.set_g8(0xa9bc, 0);
            }
            if gm.g8(g::LOCATION) == 0 {
                return;
            }
            let mut redraw: i16 = 1;
            loop {
                key = read_town_command(gm, redraw);
                redraw = 0;
                if 0x1f < key {
                    if key < 0x30 || 0x39 < key {
                        cmd = rest::dispatch_command(gm, key as i16);
                    } else {
                        cmd = handle_number_key(gm, key as i16);
                    }
                } else {
                    match key {
                        1 | 2 | 3 | 4 => {
                            leave = move_party_town(gm, key as i16) as i16;
                            cmd = (leave == 0) as u16;
                        }
                        5 => {
                            text::print_string(gm, 0x2898); // "Exit to DOS? "
                            key = input::read_command_key(gm) as u16;
                            if key == 0x59 {
                                gfx::restore_video_mode(gm);
                                rest::exit(gm, 0, 0);
                            } else {
                                text::print_string(gm, 0x28a6); // "N\n"
                            }
                        }
                        0xb => {
                            text::print_number(gm, 0x20, 1, gm.g8(g::KARMA) as i16);
                            text::put_char(gm, 10);
                            cmd = 0;
                        }
                        0x13 => {
                            text::print_string(gm, 0x28ae); // "Sound "
                            let s: u16 = if gm.g8(g::SOUND_ENABLED) == 0 { 0x28ba } else { 0x28b5 }; // "On\n" / "Off\n"
                            text::print_string(gm, s);
                            let v = (gm.g8(g::SOUND_ENABLED) == 0) as u8;
                            gm.set_g8(g::SOUND_ENABLED, v);
                            cmd = 0;
                        }
                        0x16 => {
                            text::print_string(gm, 0x28a9); // "1.16"
                            text::put_char(gm, 10);
                            cmd = 0;
                        }
                        _ => {
                            text::print_string(gm, 0x28be); // "What?\n"
                            cmd = 0;
                        }
                    }
                }
                // LAB_1000_976d
                if cmd != 3 { break; }
            }
        }
        if gm.g8(g::LOCATION) == 0 {
            leave = 1;
        } else if cmd != 0 {
            let m2 = rest::find_first_conscious_member(gm) as i16;
            let hour_before = gm.g8(g::HOUR);
            if m2 != -1 {
                rest::pass_time(gm, 1);
                if hour_before != gm.g8(g::HOUR) && (gm.g8(g::HOUR) == 0x14 || gm.g8(g::HOUR) == 5) {
                    toggle_night_tiles(gm);
                }
                apply_tile_hazards(gm);
                if gm.g8(g::DOOR_TILE) != 0 {
                    let t = gm.g8(g::DOOR_TIMER).wrapping_sub(1);
                    gm.set_g8(g::DOOR_TIMER, t);
                    if t == 0 {
                        rest::set_town_map_tile(gm, gm.g8(g::DOOR_Y), gm.g8(g::DOOR_X), gm.gi8(g::DOOR_TILE));
                    }
                }
                let px = gm.g8(g::PARTY_X);
                gm.set_g8(g::MONSTER_X, px);
                let py = gm.g8(g::PARTY_Y);
                gm.set_g8(g::MONSTER_Y, py);
                let pz = gm.g8(g::PARTY_Z);
                gm.set_g8(g::MONSTER_Z, pz);
                let tt = gm.g8(g::TRANSPORT_TILE);
                let move_ok = (tt < 0x12 || 0x15 < tt || key == 0x20 || {
                    mount_toggle = !mount_toggle;
                    !mount_toggle
                }) && (gm.gi8(g::STATUS_ICON) != 0x54
                    && (gm.gi8(g::STATUS_ICON) != 0x51 || {
                        quick_toggle = !quick_toggle;
                        !quick_toggle
                    }));
                if move_ok {
                    wander_horses(gm);
                    if (cmd as i16) < 2 {
                        ovl::npc::move_npcs(gm, gm.g8(g::HOUR) as u16);
                    }
                    if gm.g8(0x65bf) != 0 || cmd == 2 { // g_npc_contact_idx
                        handle_npc_contact(gm, (cmd as i16).wrapping_sub(1));
                    }
                }
            }
        }
        if leave != 0 {
            return;
        }
    }
}

/// orig 1000:9864 (146 bytes). Places all NPCs on entering a settlement (vector 1778:030e;
/// CMDS, SHOPPES3). Monster slots 1..31 are cleared and the NPC-slot links (state +0xc) of NPCs
/// 1..31 zeroed. Then every NPC 1..31 with a non-zero type: the NPC overlay picks the schedule
/// entry for the current hour (NPC b570(hour, npc) -> 0..2), `place_npc` puts it at that entry's
/// (z, y, x), the state is marked active, the schedule index stored (state +0xe) and its movement
/// list pointer (0x655e + 2*npc, save 0xfb8) set to 0xffff (empty).
pub fn init_town_npcs(gm: &mut Game) {
    let mut slot: i16 = 1;
    let mut p: u16 = 0x5f7a; // g_npc_slot of NPC 1
    loop {
        rest::set_monster_slot(gm, slot, 0, 0, 0, 0, 0, 0);
        gm.set_g16(p, 0);
        p = p.wrapping_add(0x10);
        slot += 1;
        if !(slot < 0x20) { break; }
    }
    let mut npc: i16 = 1;
    let mut n16: u16 = 0x10;
    let mut mp: u16 = 0x6560; // movement list pointer of NPC 1
    loop {
        if gm.g8(g::NPC_TYPES.wrapping_add(npc as u16)) != 0 {
            let idx = ovl::npc::schedule_index_for_hour(gm, gm.g8(g::HOUR) as i8, npc) as i16;
            let a = n16.wrapping_add(idx as u16);
            let z = gm.g8(a.wrapping_add(0x5d67)) as u16;
            let y = gm.g8(a.wrapping_add(0x5d64)) as u16;
            let x = gm.g8(a.wrapping_add(0x5d61)) as u16;
            place_npc(gm, z, y, x, npc);
            gm.set_g16(g::NPC_STATES.wrapping_add(n16), 1);
            gm.set_g16(n16.wrapping_add(0x5f6c), idx as u16); // schedule index
            gm.set_g16(mp, 0xffff);
        }
        n16 = n16.wrapping_add(0x10);
        mp = mp.wrapping_add(2);
        npc += 1;
        if !(npc < 0x20) { break; }
    }
}

/// orig 1000:98f6 (314 bytes). Puts NPC `npc` at (z, y, x) (vector 1778:03aa, used by the NPC
/// overlay when schedules change). Nothing for type 0. If the NPC is on another floor than the
/// party and has a monster slot, the slot's tile is cleared and the link dropped. If it is on the
/// party's floor without a slot, a spawn slot is filled with the NPC's type as tile/anim tile and
/// value1 = 0x1e for type 1, else 0xff when the NPC's bit is set in the 32-bit table 0x28c2[location]
/// else 0; its animation state is reset and the link stored. If on the party's floor with a slot,
/// the slot's x/y/z follow. Finally the state record x/y/z are updated and the NPC marked active.
/// Ghidra order: (z, y, x, npc).
pub fn place_npc(gm: &mut Game, param_1: u16, param_2: u16, param_3: u16, param_4: i16) {
    let (z, y, x, npc) = (param_1, param_2, param_3, param_4);
    if gm.g8(g::NPC_TYPES.wrapping_add(npc as u16)) != 0 {
        let n16 = npc16(npc);
        let pz = gm.g8(g::PARTY_Z) as u16;
        if z != pz && gm.gi16(0x5f6au16.wrapping_add(n16)) != 0 {
            let s = gm.gi16(0x5f6au16.wrapping_add(n16));
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(s)), 0);
            gm.set_g16(0x5f6au16.wrapping_add(n16), 0);
        }
        if z == pz && gm.gi16(0x5f6au16.wrapping_add(n16)) == 0 {
            let s = rest::find_spawn_slot(gm);
            let t = gm.g8(g::NPC_TYPES.wrapping_add(npc as u16));
            let value1: u8;
            if t == 1 {
                value1 = 0x1e;
            } else {
                let (lo, hi) = bit_mask32(npc);
                let base = (gm.g8(g::LOCATION) as u16).wrapping_mul(4);
                if (gm.g16(base.wrapping_add(0x28c4)) & hi) == 0 && (gm.g16(base.wrapping_add(0x28c2)) & lo) == 0 {
                    value1 = 0;
                } else {
                    value1 = 0xff;
                }
            }
            rest::set_monster_slot(gm, s, value1, z as u8, y as u8, x as u8, t, t);
            gm.set_g8(0x5c60u16.wrapping_add(slot8(s)), 0); // anim state
            gm.set_g16(0x5f6au16.wrapping_add(n16), s as u16);
        }
        if z == pz && gm.gi16(0x5f6au16.wrapping_add(n16)) != 0 {
            let s = gm.gi16(0x5f6au16.wrapping_add(n16));
            let s8 = slot8(s);
            gm.set_g8(0x5c5cu16.wrapping_add(s8), x as u8);
            gm.set_g8(0x5c5du16.wrapping_add(s8), y as u8);
            gm.set_g8(0x5c5eu16.wrapping_add(s8), z as u8);
        }
        gm.set_g16(0x5f60u16.wrapping_add(n16), x);
        gm.set_g16(0x5f62u16.wrapping_add(n16), y);
        gm.set_g16(0x5f64u16.wrapping_add(n16), z);
        gm.set_g16(g::NPC_STATES.wrapping_add(n16), 1);
    }
}
