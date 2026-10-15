//! OUTSUBS.OVL (translated)
//!
//! Level-2 helper overlay for the outdoors: the 2x2 cache of 16x16 map chunks at g_map
//! (0x6608) read from BRIT.DAT / UNDER.DAT (`load_map_chunks`, `load_map_chunk`,
//! `scroll_map_chunks`), the BRIT.OOL / UNDER.OOL file-name choice (`ool_file_name`), entering
//! a settlement (`enter_settlement`), going over a waterfall into the Underworld
//! (`go_over_falls`), placing the Underworld shard/amulet objects (`place_shards_and_amulet`),
//! lava and swamp effects, and the camp-time "apparition" of the old man that heals, raises
//! levels and reads the KARMA.DAT text (`apparition_level_up`). All entry points are reached
//! through vectors (see the module doc of each function).
//!
//! Ghidra split `apparition_level_up` into five pieces (a8e8, a966, aa4e, ab8c, ab9e) because
//! the vector 1778:087e (vec_SHOPPES_aa4e) points into its middle; the four fragments are
//! translated as documented no-ops.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// orig 1000:a290 (74 bytes). Vector 1778:02f6/057e/05f6/07b2 target. `chunk` is the chunk
/// code << 8 ((chunk_y/16) << 4 | chunk_x/16 in the high byte). If the high byte matches one
/// of the eight dungeon chunk codes at 0x3866 the result is "the dungeon is sealed" (its
/// g_dungeon_open byte is 0); for any other chunk returns true.
pub fn is_dungeon_sealed_chunk(gm: &mut Game, param_1: u16) -> bool {
    let chunk = param_1;
    let mut i: u16 = 0;
    loop {
        if gm.g8(0x3866u16.wrapping_add(i)) as u16 == chunk >> 8 { // g_dungeon_chunk_codes
            return gm.g8(g::DUNGEON_OPEN.wrapping_add(i)) == 0;
        }
        i = i.wrapping_add(1);
        if !(i < 8) { break; }
    }
    true
}

/// orig 1000:a2da (77 bytes). Like is_dungeon_sealed_chunk but for the eight shrine chunk codes
/// at 0x386e: returns true when the matching g_shrine_destroyed byte is >= 0x80 (the shrine
/// has been destroyed, its tile becomes ruins); false otherwise.
pub fn is_shrine_destroyed_chunk(gm: &mut Game, param_1: i16) -> bool {
    let chunk = param_1 as u16;
    let mut i: u16 = 0;
    loop {
        if gm.g8(0x386eu16.wrapping_add(i)) as u16 == chunk >> 8 { // g_shrine_chunk_codes
            return 0x7f < gm.g8(g::SHRINE_DESTROYED.wrapping_add(i));
        }
        i = i.wrapping_add(1);
        if !(i < 8) { break; }
    }
    false
}

/// orig 1000:a328 (281 bytes). Loads one 16x16 map chunk into cache slot `slot` (0..3, 256
/// bytes each at g_map). `chunk` is the chunk code << 8 (= the file offset in UNDER.DAT). For
/// BRIT.DAT (name starts with 'B') the code indexes g_brit_chunk_map (0x3876): 0xff means an
/// all-water chunk (filled with tile 1, no file access), else the mapped index << 8 is the
/// file offset. In the Underworld disk 5 is selected first. After loading, entrance tiles
/// 0x16..0x18 of a sealed dungeon are replaced by 0xdf and a destroyed shrine (0x19) by ruins
/// 0x1a, written through get_map_tile at the chunk's absolute map coordinates.
/// Ghidra order: (chunk, slot, filename).
pub fn load_map_chunk(gm: &mut Game, param_1: u16, param_2: i16, param_3: u16) {
    let (chunk, slot, name) = (param_1, param_2, param_3);
    let base = g::MAP.wrapping_add((slot as u16).wrapping_mul(0x100));
    let offset: u16;
    if gm.g8(name) == b'B' {
        let m = gm.g8(g::BRIT_CHUNK_MAP.wrapping_add(chunk >> 8));
        if m == 0xff {
            for i in 0..0x100u16 {
                gm.set_g8(base.wrapping_add(i), 1);
            }
            return;
        }
        offset = (m as u16) << 8;
    } else {
        offset = chunk;
    }
    if gm.g8(g::PARTY_Z) != 0 {
        files::select_disk(gm, 5);
    }
    files::load_file_region(gm, offset, 0x100, base, name);
    let cx = ((chunk as i16) >> 4) as u16 & 0xf0;
    let cy = ((chunk as i16) >> 8) as u16 & 0xf0;
    let mut row: u16 = 0;
    loop {
        let mut col: u16 = 0;
        while col < 0x10 {
            let t = gm.g8(base.wrapping_add(row.wrapping_mul(0x10)).wrapping_add(col));
            if 0x15 < t {
                if t < 0x19 {
                    if is_dungeon_sealed_chunk(gm, chunk) {
                        let p = rest::get_map_tile(gm, row.wrapping_add(cy), col.wrapping_add(cx));
                        gm.set_g8(p, 0xdf);
                    }
                } else if t == 0x19 && is_shrine_destroyed_chunk(gm, chunk as i16) {
                    let p = rest::get_map_tile(gm, row.wrapping_add(cy), col.wrapping_add(cx));
                    gm.set_g8(p, 0x1a);
                }
            }
            col = col.wrapping_add(1);
        }
        row = row.wrapping_add(1);
        if !(row < 0x10) { break; }
    }
}

/// orig 1000:a444 (274 bytes). Vector 1778:040a target. Fills the chunk cache after the party
/// moved by (dy,dx) (or (1,0)/(-1,0) from init_overworld to load everything). The top-left
/// chunk code is (g_chunk_y/16)<<4 | g_chunk_x/16 (as code << 8); the right column is +0x100
/// (or -0xf00 when g_chunk_x == 0xf0, wrapping to column 0) and the bottom row adds 0x10 to
/// the high byte (8-bit, wrapping). Slot 0 (top-left) is reloaded when dx == -1 or dy == -1,
/// slot 1 (top-right) when dx == 1 or dy == -1, slot 2 (bottom-left) when dx == -1 or dy == 1,
/// slot 3 (bottom-right) when dx == 1 or dy == 1; from BRIT.DAT (g_party_z < 0x80) or
/// UNDER.DAT. Finally the lighthouse tile 0x1b is searched in the 1024 cached bytes to set the
/// light-beam source g_beam1_x/y (offset & 0xf, (offset >> 4) & 0xf, +16 for the right/bottom
/// chunk), or -1/-1 if absent. Ghidra order: (dy, dx).
pub fn load_map_chunks(gm: &mut Game, param_1: i16, param_2: i16) {
    let (dy, dx) = (param_1, param_2);
    let code = (gm.g8(g::CHUNK_X) as u16).wrapping_mul(0x10).wrapping_add((gm.g8(g::CHUNK_Y) as u16).wrapping_mul(0x100));
    let right: u16 = if gm.g8(g::CHUNK_X) == 0xf0 { 0xf100 } else { 0x100 }; // -0xf00 or +0x100
    let name: u16 = if gm.g8(g::PARTY_Z) < 0x80 { 0x3980 } else { 0x3976 }; // "BRIT.DAT" / "UNDER.DAT"
    let down = |c: u16| -> u16 { (((c >> 8) as u8).wrapping_add(0x10) as u16) << 8 | (c & 0xff) };
    if dx == -1 || dy == -1 {
        load_map_chunk(gm, code, 0, name);
    }
    if dx == 1 || dy == -1 {
        load_map_chunk(gm, code.wrapping_add(right), 1, name);
    }
    if dx == -1 || dy == 1 {
        load_map_chunk(gm, down(code), 2, name);
    }
    if dx == 1 || dy == 1 {
        load_map_chunk(gm, down(code.wrapping_add(right)), 3, name);
    }
    let p = rest::memchr(gm, g::MAP, 0x1b, 0x400);
    if p == 0 {
        gm.set_gi16(g::BEAM1_Y, -1);
        gm.set_gi16(g::BEAM1_X, -1);
    } else {
        let off = p.wrapping_sub(g::MAP); // offset within the 4 cached chunks
        gm.set_g16(g::BEAM1_X, off & 0xf);
        gm.set_g16(g::BEAM1_Y, (off >> 4) & 0xf);
        if off & 0x100 != 0 {
            let v = gm.g16(g::BEAM1_X).wrapping_add(0x10);
            gm.set_g16(g::BEAM1_X, v);
        }
        if off & 0x200 != 0 {
            let v = gm.g16(g::BEAM1_Y).wrapping_add(0x10);
            gm.set_g16(g::BEAM1_Y, v);
        }
    }
}

/// orig 1000:a558 (158 bytes). Vector 1778:0452 target. Shifts the cached chunks when the
/// window scrolls by (dy,dx) so that the two chunks that stay visible move to their new slots
/// before load_map_chunks fetches the others: dst = 3 if dx < 0 or dy < 0 else 0; src = 2 if
/// dx < 0 or dy > 0 else 1; 256 bytes src -> dst, then (src + dy*2 + dx) -> (dst + dy*2 + dx).
/// E.g. east: 1 -> 0 and 3 -> 2; south: 2 -> 0 and 3 -> 1; west: 2 -> 3 and 0 -> 1; north:
/// 1 -> 3 and 0 -> 2. Ghidra order: (dy, dx).
pub fn scroll_map_chunks(gm: &mut Game, param_1: i16, param_2: i16) {
    let (dy, dx) = (param_1, param_2);
    let dst: i16 = if dx < 0 || dy < 0 { 3 } else { 0 };
    let src: i16 = if dx < 0 || 0 < dy { 2 } else { 1 };
    let copy = |gm: &mut Game, s: i16, d: i16| {
        let sa = gm.ds(g::MAP.wrapping_add((s as u16).wrapping_mul(0x100)));
        let da = gm.ds(g::MAP.wrapping_add((d as u16).wrapping_mul(0x100)));
        gm.m.copy(da, sa, 0x100);
    };
    copy(gm, src, dst);
    // original: (src + dy + dx*2) -> (dst + dy + dx*2)  [param_1 = dy, param_2 = dx]
    copy(gm, src.wrapping_add(dy).wrapping_add(dx.wrapping_mul(2)), dst.wrapping_add(dy).wrapping_add(dx.wrapping_mul(2)));
}

/// orig 1000:a5f8 (32 bytes). Vector 1778:02a2 target (ROOT main, moongate_travel, MAINOUT).
/// Returns the DGROUP offset of the monster/object list file name for the current level:
/// "BRIT.OOL" (0x3989) when g_party_z == 0, else "UNDER.OOL" (0x3992).
pub fn ool_file_name(gm: &mut Game) -> u16 {
    if gm.g8(g::PARTY_Z) == 0 { 0x3989 } else { 0x3992 }
}

/// orig 1000:a618 (206 bytes). Vector 1778:043a target (cmd_enter_outdoors). Prints the
/// settlement type `prompt` ("towne", "castle", ...), finds the location 0..31 whose
/// g_location_x/y equals the party position; if none, "What town?" and returns 1. Otherwise
/// prints "\n\n" + the inverse-video location name (except for indices 0xd..0x11, the
/// castles/keeps whose name was already given), a newline, makes sure disk 1 (BRIT.DAT) is
/// present, saves the monster table to BRIT.OOL/UNDER.OOL, sets g_location = index + 1,
/// g_party_z = 0 and the town entry position (15, 30), and returns 0.
pub fn enter_settlement(gm: &mut Game, param_1: u16) -> u16 {
    let mut result: u16 = 1;
    text::print_string(gm, param_1);
    let mut i: u16 = 0;
    loop {
        if gm.g8(g::LOCATION_X.wrapping_add(i)) == gm.g8(g::PARTY_X)
            && gm.g8(g::LOCATION_Y.wrapping_add(i)) == gm.g8(g::PARTY_Y) { break; }
        i = i.wrapping_add(1);
        if !(i < 0x20) { break; }
    }
    if i < 0x20 {
        if i < 0xd || 0x11 < i {
            text::print_string(gm, 0x399c); // "\n\n"
            text::put_char(gm, 0xfc);
            let nm = gm.g16(g::LOCATION_NAME_PTRS.wrapping_add(i.wrapping_mul(2)));
            text::print_string(gm, nm);
            text::put_char(gm, 0xfb);
        }
        text::put_char(gm, 10);
        if gm.g8(g::CUR_DISK) != 1 {
            files::select_disk(gm, 1);
            loop {
                if files::file_exists(gm, 0x399f) != 0 { break; } // "BRIT.DAT"
            }
        }
        let name = ool_file_name(gm);
        files::save_file_region(gm, 0x100, g::MONSTER_TABLE, name);
        gm.set_g8(g::LOCATION, (i as u8).wrapping_add(1));
        gm.set_g8(g::PARTY_Z, 0);
        gm.set_g8(g::PARTY_X, 0xf);
        gm.set_g8(g::PARTY_Y, 0x1e);
        result = 0;
    } else {
        text::print_string(gm, 0x39a8); // "\nWhat town?\n"
    }
    result
}

/// orig 1000:a6e8 (269 bytes). Vector 1778:03c2 target (read_overworld_command / the main
/// loop when a waterfall is south of / under the party). "F-A-L-L-S!!!": the party is carried
/// two cells south (move_party_by(1,0) twice with a frame between), a falling tone sweep
/// plays, the transport tile is hidden (0) for a frame, every living member whose dexterity
/// is <= a 1..30 roll takes 1 damage, two more frames, transport restored. If the party lands
/// at (0x36, 0x8a) - the falls into the Underworld - "Falling into underworld!!": g_party_z =
/// 0xff, the monster table is saved to BRIT.OOL and UNDER.OOL loaded, disk 5 (UNDER.DAT) is
/// requested, UNDER.OOL is written back and init_overworld (vector 1778:03fe) rebuilds the
/// view.
pub fn go_over_falls(gm: &mut Game) {
    text::print_string(gm, 0x39b5); // "F-A-L-L-S!!!\n"
    ovl::mainout::move_party_by(gm, 1, 0);
    util::animate_wait(gm, 1);
    ovl::mainout::move_party_by(gm, 1, 0);
    rest::play_tone_sweep(gm, 300, 1, 800, 0x9c4);
    let saved = gm.g8(g::TRANSPORT_TILE);
    gm.set_g8(g::TRANSPORT_TILE, 0);
    util::animate_wait(gm, 1);
    if gm.g8(g::PARTY_COUNT) != 0 {
        let mut i: u16 = 0;
        loop {
            let rec = i.wrapping_mul(0x20);
            if gm.g8(g::PARTY_STATUS.wrapping_add(rec)) != b'D' {
                let roll = rest::random_damage_1_30(gm) as u16;
                if gm.g8(g::PARTY_DEX.wrapping_add(rec)) as u16 <= roll {
                    rest::damage_party_member(gm, 1, i);
                }
            }
            i = i.wrapping_add(1);
            if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
        }
    }
    util::animate_wait(gm, 2);
    gm.set_g8(g::TRANSPORT_TILE, saved);
    if gm.g8(g::PARTY_X) == 0x36 && gm.g8(g::PARTY_Y) == 0x8a {
        text::print_string(gm, 0x39c3); // "Falling into underworld!!\n"
        gm.set_g8(g::PARTY_Z, 0xff);
        files::save_file_region(gm, 0x100, g::MONSTER_TABLE, 0x39de); // "BRIT.OOL"
        files::load_file_region(gm, 0, 0x100, g::MONSTER_TABLE, 0x39e7); // "UNDER.OOL"
        files::select_disk(gm, 5);
        loop {
            if files::file_exists(gm, 0x39f1) != 0 { break; } // "UNDER.DAT"
        }
        files::save_file_region(gm, 0x100, g::MONSTER_TABLE, 0x39fb); // "UNDER.OOL"
        ovl::mainout::init_overworld(gm);
    }
}

/// orig 1000:a7f6 (136 bytes). Vector 1778:0416 target (init_overworld). In the Underworld
/// only: if the Amulet of Lord British has not been found (g_amulet_lb == 0) monster slot 28
/// (0x5d3a) becomes the amulet object: tile/anim 0xb7 at (0x69, 0xe1) level 0xff, value1 0xf3.
/// Then for each shard i (0..2) that is not yet owned (g_shard_falsehood[i] == 0) while its
/// Shadowlord still exists (g_shadowlord_loc[i] < 0x80), slot 29+i (0x5d42 + 8i) becomes a
/// shard object: tile/anim 0xb4, x from 0x3a06[i], y from 0x3a0a[i], level 0xff, value1 from
/// 0x3a0e[i] (0xf0..0xf2, the Shadowlord id).
pub fn place_shards_and_amulet(gm: &mut Game) {
    if gm.g8(g::PARTY_Z) != 0 {
        if gm.g8(g::AMULET_LB) == 0 {
            gm.set_g8(0x5d3b, 0xb7); // slot 28 anim tile
            gm.set_g8(0x5d3a, 0xb7); // slot 28 tile
            gm.set_g8(0x5d3c, 0x69); // x
            gm.set_g8(0x5d3d, 0xe1); // y
            gm.set_g8(0x5d3e, 0xff); // z
            gm.set_g8(0x5d3f, 0xf3); // value1
        }
        let mut i: u16 = 0;
        let mut p: u16 = 0x5d42; // slot 29
        loop {
            if gm.g8(g::SHARD_FALSEHOOD.wrapping_add(i)) == 0 && gm.g8(g::SHADOWLORD_LOC.wrapping_add(i)) < 0x80 {
                gm.set_g8(p.wrapping_add(1), 0xb4);
                gm.set_g8(p, 0xb4);
                let v = gm.g8(0x3a06u16.wrapping_add(i)); // g_shard_x
                gm.set_g8(p.wrapping_add(2), v);
                let v = gm.g8(0x3a0au16.wrapping_add(i)); // g_shard_y
                gm.set_g8(p.wrapping_add(3), v);
                gm.set_g8(p.wrapping_add(4), 0xff);
                let v = gm.g8(0x3a0eu16.wrapping_add(i)); // g_shard_value1 (0xf0..0xf2)
                gm.set_g8(p.wrapping_add(5), v);
            }
            p = p.wrapping_add(8);
            i = i.wrapping_add(1);
            if !(i < 3) { break; }
        }
    }
}

/// orig 1000:a87e (14 bytes). Vector 1778:03da target (main loop, lava tile 0x8f under the
/// party): redraws, prints "Burning!" and damage_whole_party.
pub fn burn_on_lava(gm: &mut Game) {
    view::refresh_map_view(gm);
    text::print_string(gm, 0x3a11); // "Burning!\n"
    rest::damage_whole_party(gm);
}

/// orig 1000:a88c (92 bytes). Vector 1778:03ce target (main loop, swamp tile 4 on foot): every
/// member who is neither dead nor already poisoned whose dexterity is below rand_range(1,30)
/// becomes 'P' with "Poisoned!".
pub fn poison_in_swamp(gm: &mut Game) {
    if gm.g8(g::PARTY_COUNT) != 0 {
        let mut i: u16 = 0;
        loop {
            let rec = i.wrapping_mul(0x20);
            let st = gm.g8(g::PARTY_STATUS.wrapping_add(rec));
            if st != b'D' && st != b'P' {
                let roll = util::rand_range(gm, 0x1e, 1) as u16;
                if (gm.g8(g::PARTY_DEX.wrapping_add(rec)) as u16) < roll {
                    gm.set_g8(g::PARTY_STATUS.wrapping_add(rec), b'P');
                    text::print_string(gm, 0x3a1b); // "Poisoned!\n"
                }
            }
            i = i.wrapping_add(1);
            if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
        }
    }
}

/// orig 1000:a8e8 (125 bytes as split by Ghidra; the function really runs from a8e8 to the
/// RET at ac2b and includes the fragments a966/aa4e/ab8c/ab9e). Vector 1778:07d6 target,
/// called by CMDS_bf80 (camping) when the old man appears: "An apparition!", a sound sweep
/// plus one per word of the 6-entry table at 0x3a26, then monster slot 10 (0x5caa, the
/// combat-map object slot at view cell (5,5)) is set to tile 0x16 and dissolved in with
/// tile_transition_effect(5,5,0x174), after which it shows tile 0x74. For every party member
/// (dead ones only get the mana update): HP = max HP and status 'G'; the combat roster
/// (0xba14.., 32 entries of 8, +3 member, +4 monster slot) is searched for the member - its
/// monster slot (or slot 10 when absent) receives the class tile from 0x1ade indexed by the
/// class letter's position in "AMBFDTPRS"; the slot-10 anim state (0x5cb0) is cleared, a
/// frame and sound play, the 8x8..183x183 area is XOR-flashed, another sweep, three frames
/// with anim state 1. The new level is 1 + the number of times exp/100 can be halved while
/// positive (2 at 100, 3 at 200, 4 at 400 ...); if it differs from the stored level
/// (record +0x16) it is stored, max HP = HP = 30 * level, and "Hail, <name>! For thy valiant
/// deeds, I shall reward thee! Thou art now level N, and stronger!/quicker!/wiser!" raises
/// strength (+0xc), dexterity (+0xd) or intelligence (+0xe) by 1 up to 30, then waits for a key.
/// Mana (+0xf): Avatar/Mage = intelligence, Bard = intelligence / 2. Status window redrawn
/// per member. Finally the karma speech: KARMA.DAT paragraph (2000 bytes to g_ool_buffer)
/// at offset g_unk_1a74[karma/20] for karma < 80 else 0x29f, printed in quotes, a key,
/// "The strangely familiar old man vanishes...", slot 10 shown as 0x16 and dissolved into the
/// combat-map centre tile, then cleared; animate_wait(1); pass_time(0).
pub fn apparition_level_up(gm: &mut Game) {
    text::print_string(gm, 0x7750); // "An apparition!\n"
    util::play_sweep_noise(gm, 6, 0x9c4, 0x2710, 1, 0xa3c);
    let mut p: u16 = 0x3a26; // g_unk_3a26: 6 sweep phase words
    loop {
        let phase = gm.g16(p);
        util::play_sweep_noise(gm, 0xd, 200, 0x1388, 1, phase);
        p = p.wrapping_add(2);
        if !(p < 0x3a32) { break; }
    }
    gm.set_g8(0x5cad, 5); // slot 10 y
    gm.set_g8(0x5cac, 5); // slot 10 x
    gm.set_g8(0x5cab, 0x16); // slot 10 anim tile
    gm.set_g8(0x5caa, 0x16); // slot 10 tile
    gfx::tile_transition_effect(gm, 5, 5, 0x174);
    gm.set_g8(0x5cab, 0x74);
    gm.set_g8(0x5caa, 0x74);
    let mut m: u16 = 0;
    while m < gm.g8(g::PARTY_COUNT) as u16 {
        let rec = m.wrapping_mul(0x20);
        if gm.g8(g::PARTY_STATUS.wrapping_add(rec)) != b'D' {
            let mhp = gm.g16(g::PARTY_MAX_HP.wrapping_add(rec));
            gm.set_g16(g::PARTY_HP.wrapping_add(rec), mhp);
            gm.set_g8(g::PARTY_STATUS.wrapping_add(rec), b'G');
            // find the member's combat-roster entry; its monster slot gets the class tile
            let mut obj: u16 = 0x5caa;
            let mut r: u16 = g::ROSTER_MEMBER;
            loop {
                if gm.g8(r) as u16 == m {
                    obj = g::MONSTER_TABLE.wrapping_add((gm.g8(r.wrapping_add(1)) as u16).wrapping_mul(8));
                    break;
                }
                r = r.wrapping_add(8);
                if !(r.wrapping_add(1) < 0xbb18) { break; }
            }
            let cls = gm.g8(g::PARTY_CLASS.wrapping_add(rec)) as u16;
            let idx = rest::str_index_of(gm, cls, 0x7760); // "AMBFDTPRS"
            let tile = gm.g8((idx as u16).wrapping_add(0x1ade)); // g_class_tile
            gm.set_g8(obj.wrapping_add(1), tile);
            gm.set_g8(obj, tile);
            gm.set_g8(0x5cb0, 0); // slot 10 anim state
            util::animate_wait(gm, 1);
            util::play_sweep_noise(gm, 0xd, 200, 0x1388, 1, 0x157c);
            let c = gm.gi16(g::UI_COLOR1);
            gfx::set_color(gm, c);
            gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
            util::play_sweep_noise(gm, 1, 0x9c4, 60000, 1, 0x157c);
            let mut n: i16 = 3;
            loop {
                gm.set_g8(0x5cb0, 1);
                util::animate_wait(gm, 1);
                n -= 1;
                if n == 0 { break; }
            }
            let mut level: u16 = 1;
            let mut e = gm.gi16(0x55bcu16.wrapping_add(rec)) / 100; // record +0x14 experience
            while 0 < e {
                level = level.wrapping_add(1);
                e >>= 1;
            }
            if gm.g8(0x55beu16.wrapping_add(rec)) as u16 != level { // record +0x16 level
                gm.set_g8(0x55beu16.wrapping_add(rec), level as u8);
                gm.set_g16(g::PARTY_MAX_HP.wrapping_add(rec), level.wrapping_mul(0x1e));
                gm.set_g16(g::PARTY_HP.wrapping_add(rec), level.wrapping_mul(0x1e));
                text::print_string(gm, 0x776a); // "\n\"Hail, "
                text::print_string(gm, 0x55a8u16.wrapping_add(rec)); // member name
                text::print_string(gm, 0x7774); // "!\nFor thy valiant deeds, I shall reward thee!\n"
                text::print_string(gm, 0x77a4); // "Thou art now level "
                text::print_number(gm, 0x20, 1, level as i16);
                text::print_string(gm, 0x77b8); // ", and\n"
                let r = util::rand_range(gm, 3, 1);
                if r == 1 {
                    text::print_string(gm, 0x77c0); // "stronger!"
                    rest::add_u8_capped(gm, 0x1e, 1, 0x55b4u16.wrapping_add(rec)); // strength
                } else if r == 2 {
                    text::print_string(gm, 0x77ca); // "quicker!"
                    rest::add_u8_capped(gm, 0x1e, 1, g::PARTY_DEX.wrapping_add(rec));
                } else if r == 3 {
                    text::print_string(gm, 0x77d4); // "wiser!"
                    rest::add_u8_capped(gm, 0x1e, 1, 0x55b6u16.wrapping_add(rec)); // intelligence
                }
                text::print_string(gm, 0x77dc); // "\" "
                input::read_command_key(gm);
                text::put_char(gm, 10);
            }
        }
        // LAB_1000_aa2c: mana from intelligence by class
        let cls = gm.g8(g::PARTY_CLASS.wrapping_add(rec));
        if cls == b'A' || cls == b'M' {
            let v = gm.g8(0x55b6u16.wrapping_add(rec));
            gm.set_g8(0x55b7u16.wrapping_add(rec), v); // mana = intelligence
        } else if cls == b'B' {
            let v = gm.g8(0x55b6u16.wrapping_add(rec)) >> 1;
            gm.set_g8(0x55b7u16.wrapping_add(rec), v); // mana = intelligence / 2
        }
        rest::draw_status_window(gm);
        m = m.wrapping_add(1);
    }
    // 1000:ab9e: the karma speech
    text::print_string(gm, 0x77e0); // "\n\""
    let k = (gm.g8(g::KARMA) / 0x14) as u16;
    if k < 4 {
        let off = gm.g16(g::UNK_1A74.wrapping_add(k.wrapping_mul(2))); // KARMA.DAT paragraph offsets
        files::load_file_region(gm, off, 2000, g::OOL_BUFFER, 0x77e4); // "KARMA.DAT"
    } else {
        files::load_file_region(gm, 0x29f, 2000, g::OOL_BUFFER, 0x77ee); // "KARMA.DAT"
    }
    text::print_string(gm, g::OOL_BUFFER);
    text::put_char(gm, 0x22);
    input::read_command_key(gm);
    text::print_string(gm, 0x77f8); // "\n\nThe strangely familiar old man vanishes...\n"
    gm.set_g8(0x5cab, 0x16);
    gm.set_g8(0x5caa, 0x16);
    let centre = gm.g8(g::COMBAT_MAP_CENTRE) as i16;
    gfx::tile_transition_effect(gm, 5, 5, centre);
    gm.set_g8(0x5cab, 0);
    gm.set_g8(0x5caa, 0);
    util::animate_wait(gm, 1);
    rest::pass_time(gm, 0);
}

/// orig 1000:a966 (230 bytes). Not a function: the combat-roster scan loop and the per-member
/// body of `apparition_level_up`, split off by Ghidra. No-op.
pub fn apparition_fragment_a966(gm: &mut Game) {}

/// orig 1000:aa4e (316 bytes). Not a function: the "quicker!" branch of `apparition_level_up`;
/// vector 1778:087e (vec_SHOPPES_aa4e) points here when SHOPPES is loaded. No-op.
pub fn apparition_fragment_aa4e(gm: &mut Game) {}

/// orig 1000:ab8c (18 bytes). Not a function: the Bard mana = intelligence / 2 branch of
/// `apparition_level_up`. No-op.
pub fn apparition_fragment_ab8c(gm: &mut Game) {}

/// orig 1000:ab9e (141 bytes). Not a function: the karma-speech ending of
/// `apparition_level_up` (KARMA.DAT paragraph, "The strangely familiar old man vanishes...").
/// No-op.
pub fn apparition_fragment_ab9e(gm: &mut Game) {}
