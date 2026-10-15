//! SJOG.OVL (translated)
//!
//! Level-3 overlay holding the S(earch), J(immy), O(pen), G(et) commands plus a bundle of combat
//! helpers (move a combatant / leave the battlefield, combat Klimb, set active player, per-turn
//! effects, the combat aftermath `end_combat` that ROOT `fight_monster` calls, and the Chaos-sword
//! pass-out check). The four commands each branch on g_location: dungeons (0x21..0x28) work on the
//! 8x8x8 dungeon map at g_dungeon_map (0x595a, cell = z*0x40 + (y&7)*8 + (x&7)); everything else
//! works on the monster/object table (0x5c5a, 8 bytes per slot: +0 tile, +1 anim tile, +2 x,
//! +3 y, +4 z, +5 value1, +6 value2/anim state, +7 value3) and the map through `get_map_tile`.
//! Hidden-object / loot tables live in DGROUP (0x3e66.. reagent patches, 0x3e78.. the 113 hidden
//! objects, 0x4123.. fixed chest loot, 0x413c.. random chest loot, 0x41bc.. dungeon chest loot,
//! 0x5830.. buried moonstones).
//!
//! Ghidra's `CMDS_dba0` label inside this overlay (1000:dba0) is only the tail of
//! `leave_combat` (db32) that the CMDS vector address happens to coincide with; it is kept as a
//! documented no-op (`fn_1000_dba0_fragment`).
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// Monster-table slot offset (8 bytes per slot at 0x5c5a).
#[inline] fn slot8(slot: i16) -> u16 { (slot as u16).wrapping_mul(8) }
/// Combat-roster entry offset (8 bytes per entry at 0xba14).
#[inline] fn ros8(i: i16) -> u16 { (i as u16).wrapping_mul(8) }
/// Party record offset (32 bytes per member at 0x55a8).
#[inline] fn mem32(i: i16) -> u16 { (i as u16).wrapping_mul(0x20) }
/// DGROUP offset of dungeon map cell (z, y, x) in g_dungeon_map (0x595a).
#[inline] fn dng_cell(z: u16, y: u16, x: u16) -> u16 {
    g::DUNGEON_MAP.wrapping_add(z.wrapping_mul(0x40)).wrapping_add((y & 7).wrapping_mul(8)).wrapping_add(x & 7)
}

/// orig 1000:bf80 (41 bytes). Scans the monster table from slot 31 down to slot 0 and returns
/// the first slot whose tile byte is 0 (free); returns 0 when every slot is in use (slot 0 is the
/// party's own entry, so 0 doubles as "none"). Used by `spawn_chest_loot`; Ghidra labelled it
/// CMDS_bf80 because the CMDS camp vector (1778:08f6) shares the address.
pub fn find_free_slot_from_top(gm: &mut Game) -> i16 {
    let mut i: i16 = 0x20;
    loop {
        i -= 1;
        if i < 0 { return 0; }
        if gm.gi8(g::MONSTER_TABLE.wrapping_add(slot8(i))) == 0 { return i; }
    }
}

/// orig 1000:bfaa (65 bytes). Sets g_dx/g_dy to the cell one step from (x,y) in dungeon facing
/// `facing` (0 = north: y-1, 1 = east: x+1, 2 = south: y+1, 3 = west: x-1). Ghidra order is
/// (y, x, facing).
pub fn set_dxy_from_facing(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16) {
    let (y, x, facing) = (param_1, param_2, param_3);
    gm.set_gi16(g::DX, x);
    gm.set_gi16(g::DY, y);
    if facing == 0 {
        gm.set_gi16(g::DY, y.wrapping_sub(1));
    } else if facing == 1 {
        gm.set_gi16(g::DX, x.wrapping_add(1));
    } else if facing == 2 {
        gm.set_gi16(g::DY, y.wrapping_add(1));
    } else if facing == 3 {
        gm.set_gi16(g::DX, x.wrapping_sub(1));
    }
}

/// orig 1000:bfec (185 bytes). Dungeon "Search-" cell selection. Prints the prompt (0x84e6)
/// and waits for a key: 1 (left) prints "Left" and turns the facing by +3, 2 (right) "Right" +1,
/// 3 (forward) "Ahead", 4 (back) "Here" (g_dx/g_dy = party cell, returns 1), space prints "Pass"
/// and returns 0. For left/right/ahead g_dx/g_dy are set one step in the resulting facing via
/// `set_dxy_from_facing` and 1 is returned. Vector 1778:07be.
pub fn select_search_cell_dungeon(gm: &mut Game, param_1: i16) -> u16 {
    let mut facing = param_1;
    text::print_string(gm, 0x84e6); // direction prompt
    let mut c: i16;
    loop {
        c = input::read_command_key(gm);
        if c == 0x20 || c == 3 || c == 4 { break; }
        if c == 2 || c == 1 { break; }
    }
    if c == 1 {
        text::print_string(gm, 0x8508); // "Left"
        facing = facing.wrapping_add(3);
    } else if c == 2 {
        text::print_string(gm, 0x8500); // "Right"
        facing = facing.wrapping_add(1);
    } else if c == 3 {
        text::print_string(gm, 0x84f2); // "Ahead"
        set_dxy_from_facing(gm, gm.g8(g::PARTY_Y) as i16, gm.g8(g::PARTY_X) as i16, facing);
        return 1;
    } else if c == 4 {
        text::print_string(gm, 0x84fa); // "Here"
        gm.set_g16(g::DX, gm.g8(g::PARTY_X) as u16);
        gm.set_g16(g::DY, gm.g8(g::PARTY_Y) as u16);
        return 1;
    } else if c == 0x20 {
        text::print_string(gm, 0x84ec); // "Pass"
        return 0;
    } else {
        return 1; // unreachable (loop only exits on the keys above)
    }
    facing = facing % 4;
    set_dxy_from_facing(gm, gm.g8(g::PARTY_Y) as i16, gm.g8(g::PARTY_X) as i16, facing);
    1
}

/// orig 1000:c0aa (121 bytes). Prints the description of a found object by its tile/kind code:
/// 1 chest, 2 sack of gold, 3 potion, 4 scroll, 5 weapon, 6 shield, 7 ring of keys, 8 gems
/// (0x856c), 9 torches/0x8574, 0xa ring, 0xb armour, 0xc amulet, 0xd torches, 0xf food, 0x19
/// strange rock (moonstone), 0x1e rotting body, 0x1f moldy corpse; anything else "Nothing of
/// note!".
pub fn print_found_object_name(gm: &mut Game, param_1: u16) {
    let s: u16 = match param_1 {
        1 => 0x850e,    // "a chest!"
        2 => 0x8518,    // "a sack of gold!"
        3 => 0x852a,    // "a potion!"
        4 => 0x8536,    // "a scroll!"
        5 => 0x8542,    // "a weapon!"
        6 => 0x854e,    // "a shield!"
        7 => 0x855a,    // "a ring of keys!"
        8 => 0x856c,
        9 => 0x8574,
        10 => 0x857e,   // "a ring!"
        0xb => 0x8588,  // "some armour!"
        0xc => 0x8596,  // "an amulet!"
        0xd => 0x85a2,  // "some torches!"
        0xf => 0x85b2,  // "some food!"
        0x19 => 0x85be, // "a strange rock!"
        0x1e => 0x85d0, // "a rotting body!"
        0x1f => 0x85e2, // "a moldy corpse!"
        _ => 0x85f4,    // "Nothing of note!"
    };
    text::print_string(gm, s);
}

/// orig 1000:c172 (243 bytes). Searching a moldy corpse (object 0x1f in monster slot `slot`)
/// by party member `member`. 1 in 8: the corpse turns out to be a monster: 1 in 4 a tile-0xf
/// creature (0x863a) else tile 2 (0x8642); the slot's tile/anim tile become that creature, its
/// value1 (+5) = rand 1..3, g_view_dirty |= 2 and the view is refreshed. Otherwise the slot is
/// cleared and: 1 in 32 (rand 0..31 == 0x13) the member is poisoned (0x8606, status 'P', tones);
/// else prints one of "nothing", 0x861a, 0x8622, "a bloody pulp" chosen by rand(0..rand(0..3)).
pub fn search_corpse(gm: &mut Game, param_1: i16, param_2: i16) {
    let (member, slot) = (param_1, param_2);
    if util::rand_range(gm, 7, 0) == 0 {
        let t: u8 = if util::rand_range(gm, 3, 0) == 0 {
            text::print_string(gm, 0x863a);
            0xf
        } else {
            text::print_string(gm, 0x8642);
            2
        };
        gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)).wrapping_add(1), t);
        gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)), t);
        let v = util::rand_range(gm, 3, 1) as u8;
        gm.set_g8(g::MONSTER_Z.wrapping_add(slot8(slot)).wrapping_add(1), v); // value1
        let d = gm.g8(g::VIEW_DIRTY) | 2;
        gm.set_g8(g::VIEW_DIRTY, d);
        rest::refresh_map_view(gm);
    } else {
        rest::set_monster_slot(gm, slot, 0, 0, 0, 0, 0, 0);
        if util::rand_range(gm, 0x1f, 0) == 0x13 {
            text::print_string(gm, 0x8606); // poisoned message
            util::play_random_tones(gm, 500, 3000, 0x28);
            gm.set_g8(g::PARTY_STATUS.wrapping_add(mem32(member)), 0x50); // 'P'
            gm.set_g8(g::STATUS_DIRTY, 1);
        } else {
            let hi = util::rand_range(gm, 3, 0);
            let r = util::rand_range(gm, hi, 0);
            let s: u16 = match r {
                0 => 0x8610, // "nothing!"
                1 => 0x861a,
                2 => 0x8622,
                3 => 0x862a, // "a bloody pulp!"
                _ => return,
            };
            text::print_string(gm, s);
        }
    }
}

/// orig 1000:c26a (187 bytes). Searching a chest (monster slot `slot`) for traps with member
/// `member`. value1 (+5) bit 7 = trapped, low 7 bits = trap difficulty. difficulty' =
/// (trapped ? (value1&0x7f) - INT + 0x1e : -(INT - 0x1e)) with INT = party record +0xe
/// (0x55b6); roll = rand 1..30; pass = difficulty'/2 <= roll. "no trap!" is printed when
/// (pass && untrapped) or (!pass && trapped) - i.e. a failed check on a trapped chest also says
/// "no trap". Otherwise: pass && d < 10 "a simple trap!", pass && d > 20 "a complex trap!",
/// else 0x8676 ("a trap!"). Purely informational, nothing is changed.
pub fn search_chest_for_trap(gm: &mut Game, param_1: i16, param_2: i16) {
    let (member, slot) = (param_1, param_2);
    let v1 = gm.g8(g::MONSTER_Z.wrapping_add(slot8(slot)).wrapping_add(1)); // value1
    let intel = gm.g8(0x55b6u16.wrapping_add(mem32(member))) as u16; // party +0xe intelligence
    let d: u16 = if (v1 & 0x80) == 0 {
        0u16.wrapping_sub(intel.wrapping_sub(0x1e))
    } else {
        ((v1 & 0x7f) as u16).wrapping_sub(intel).wrapping_add(0x1e)
    };
    let roll = util::rand_range(gm, 0x1e, 1);
    let pass = ((d >> 1) as i16) <= roll;
    let s: u16 = if (pass && (v1 & 0x80) == 0) || (!pass && (v1 & 0x80) != 0) {
        0x864a // "no trap!"
    } else {
        let diff = (v1 & 0x7f) as u16;
        if pass && diff < 10 {
            0x8654 // "a simple trap!"
        } else if pass && 0x14 < diff {
            0x8664 // "a complex trap!"
        } else {
            0x8676
        }
    };
    text::print_string(gm, s);
}

/// orig 1000:c328 (177 bytes). Digs for a buried moonstone at (z,y,x) in the current location.
/// Scans moonstones 7..0 (tables indexed 1..8 from 0x582f: x 0x5830, y 0x5838, location 0x5840
/// (g_moonstone_loc), z 0x5848; save 0x28a/0x292/0x29a/0x2a2) for one buried here; a moonstone
/// whose object (tile 0x19 with value1 == index) is already in the monster table (32 probes of
/// find_object_at, slot in g_dx) is skipped. Found: find_spawn_slot, set_monster_slot(slot,
/// index, z, y, x, 0x19, 0x19), "a strange rock!", g_view_dirty |= 2, returns 1; else 0.
pub fn dig_up_moonstone(gm: &mut Game, param_1: u16, param_2: u16, param_3: u16) -> u16 {
    let (z, y, x) = (param_1, param_2, param_3);
    let mut n: u16 = 8;
    let mut idx: u16 = n;
    loop {
        loop {
            n = idx;
            idx = n.wrapping_sub(1);
            if (idx as i16) < 0 { return 0; }
            let ok = gm.g8(n.wrapping_add(0x582f)) as u16 == x
                && gm.g8(n.wrapping_add(0x5837)) as u16 == y
                && gm.g8(n.wrapping_add(0x5847)) as u16 == z
                && gm.g8(n.wrapping_add(0x583f)) == gm.g8(g::LOCATION);
            if ok { break; }
        }
        let mut already = false;
        let mut k: i16 = 0x20;
        loop {
            k -= 1;
            if k < 0 { break; }
            let t = rest::find_object_at(gm, z, y, x);
            if t == 0x19 {
                let slot = gm.gi16(g::DX);
                if gm.g8(g::MONSTER_Z.wrapping_add(slot8(slot)).wrapping_add(1)) as u16 == idx {
                    already = true;
                }
            }
        }
        if !already { break; }
    }
    let slot = rest::find_spawn_slot(gm);
    rest::set_monster_slot(gm, slot, idx as u8, z as u8, y as u8, x as u8, 0x19, 0x19);
    text::print_string(gm, 0x8680); // "a strange rock!"
    let d = gm.g8(g::VIEW_DIRTY) | 2;
    gm.set_g8(g::VIEW_DIRTY, d);
    1
}

/// orig 1000:c3da (181 bytes). Reagent harvesting: three patches (x at 0x3e66[i], y at
/// 0x3e6a[i], reagent index 0x3e6e[i], name pointer 0x3e72[i]) can be searched at (y,x) only at
/// hour 0 (midnight) and once per day (g_harvest_day[i] != g_day). On success the day is
/// recorded, rand 2..15 sprigs are added to g_reagents[index] (capped at 99) and "N sprigs of
/// <name>" (0x86be, name, 0x86ca) is printed; returns 1, else 0. Ghidra order (y, x).
pub fn harvest_reagents(gm: &mut Game, param_1: u16, param_2: u16) -> u16 {
    let (y, x) = (param_1, param_2);
    let mut i: u16 = 0;
    let mut name_ptr: u16 = 0x3e72;
    loop {
        let miss = gm.g8(0x3e66u16.wrapping_add(i)) as u16 != x
            || gm.g8(0x3e6au16.wrapping_add(i)) as u16 != y
            || gm.g8(g::HOUR) != 0
            || gm.g8(g::HARVEST_DAY.wrapping_add(i)) == gm.g8(g::DAY);
        if !miss { break; }
        name_ptr = name_ptr.wrapping_add(2);
        i += 1;
        if 2 < i { return 0; }
    }
    let day = gm.g8(g::DAY);
    gm.set_g8(g::HARVEST_DAY.wrapping_add(i), day);
    let reagent = gm.g8(0x3e6eu16.wrapping_add(i)) as u16;
    let n = util::rand_range(gm, 0xf, 2);
    let v = gm.g8(g::REAGENTS.wrapping_add(reagent)).wrapping_add(n as u8);
    gm.set_g8(g::REAGENTS.wrapping_add(reagent), v);
    if 99 < gm.g8(g::REAGENTS.wrapping_add(reagent)) {
        gm.set_g8(g::REAGENTS.wrapping_add(reagent), 99);
    }
    let width: i16 = if n < 10 { 1 } else { 2 };
    text::print_number(gm, 0x20, width, n);
    text::print_string(gm, 0x86be); // "sprigs of "
    let p = gm.g16(name_ptr);
    text::print_string(gm, p);
    text::print_string(gm, 0x86ca);
    1
}

/// orig 1000:c494 (303 bytes). Hidden-object search at (y,x) on the current map/floor. Walks
/// the 113-entry hidden object tables (location 0x3f5c, z 0x3fce, x 0x4040, y 0x40b2, tile
/// 0x3e78, quality 0x3eea; indices 0..0x70). For a matching entry: entries 0xd..0xf are
/// special, never flagged and repeatable: 0xd (keys) only when g_keys == 0 and nothing lies on
/// the cell, 0xe (skull key) only when g_day != g_skull_key_day, 0xf (glass sword, 0x57e7) only
/// when none is owned and the cell is empty; any other entry is skipped when its bit in
/// g_hidden_object_flags is set, otherwise the bit is set. On a find: for 0xe g_skull_key_day =
/// g_day; the object is spawned with set_monster_slot(find_spawn_slot(), quality, z, y, x,
/// tile, tile), g_view_dirty |= 2 and `print_found_object_name(tile)`; otherwise "nothing of
/// note!". Ghidra order (y, x).
pub fn search_hidden_object(gm: &mut Game, param_1: u16, param_2: u16) {
    let (y, x) = (param_1, param_2);
    let mut found = false;
    let mut i: i16 = 0;
    loop {
        let m = gm.g8(g::HIDDEN_OBJECT_LOCATION.wrapping_add(i as u16)) == gm.g8(g::LOCATION)
            && gm.gi8(g::HIDDEN_OBJECT_Z.wrapping_add(i as u16)) == gm.gi8(g::PARTY_Z)
            && gm.g8(g::HIDDEN_OBJECT_X.wrapping_add(i as u16)) as u16 == x
            && gm.g8(g::HIDDEN_OBJECT_Y.wrapping_add(i as u16)) as u16 == y;
        if m {
            let pz = gm.g8(g::PARTY_Z) as u16;
            let normal = (i != 0xd || gm.g8(g::KEYS) != 0 || rest::find_object_at(gm, pz, y, x) != 0)
                && (i != 0xe || gm.g8(g::DAY) == gm.g8(g::SKULL_KEY_DAY))
                && (i != 0xf || (gm.g8(0x57e7) != 0 || rest::find_object_at(gm, pz, y, x) != 0)); // 0x57e7 glass swords
            if normal {
                let bit = (1u16 << ((i as u8) & 7)) as u8;
                let fo = g::HIDDEN_OBJECT_FLAGS.wrapping_add((i >> 3) as u16);
                if (bit & gm.g8(fo)) != 0 || (0xc < i && i < 0x10) {
                    // LAB_1000_c5aa: already found / repeatable special not available
                    i += 1;
                    if 0x70 < i { break; }
                    continue;
                }
                let v = gm.g8(fo) | bit;
                gm.set_g8(fo, v);
            }
            found = true;
            break;
        }
        i += 1;
        if 0x70 < i { break; }
    }
    // LAB_1000_c526
    if i == 0xe {
        let d = gm.g8(g::DAY);
        gm.set_g8(g::SKULL_KEY_DAY, d);
    }
    if found {
        let slot = rest::find_spawn_slot(gm);
        let tile = gm.g8(g::HIDDEN_OBJECT_TILE.wrapping_add(i as u16));
        let q = gm.g8(g::HIDDEN_OBJECT_QUALITY.wrapping_add(i as u16));
        let z = gm.g8(g::HIDDEN_OBJECT_Z.wrapping_add(i as u16));
        let oy = gm.g8(g::HIDDEN_OBJECT_Y.wrapping_add(i as u16));
        let ox = gm.g8(g::HIDDEN_OBJECT_X.wrapping_add(i as u16));
        rest::set_monster_slot(gm, slot, q, z, oy, ox, tile, tile);
        let d = gm.g8(g::VIEW_DIRTY) | 2;
        gm.set_g8(g::VIEW_DIRTY, d);
        print_found_object_name(gm, tile as u16);
    } else {
        text::print_string(gm, 0x86cc); // "nothing of note!"
    }
}

/// orig 1000:c5c6 (784 bytes). Dungeon Search. Needs a target member; without light (no torch
/// and no light spell) "You find: darkness!". Otherwise `select_search_cell_dungeon` picks the
/// cell (g_dx/g_dy); cell = g_dungeon_map[z][y&7][x&7], skill = (2z - DEX + 0x1e)/2. "You find:"
/// then by cell high nibble: 0x00 "Nothing of note!"; 0x10/0x20/0x30 (ladders) "Nothing hidden
/// on the ladder!"; 0x40 chest: roll rand 1..30 > skill -> cell == 0x40 "No trap" else the trap
/// level is z (else a random 1..8): <4 "A simple trap", <7 "A trap", else "A complex trap";
/// 0x50 fountain "Nothing hidden on the fountain!"; 0x60 pit: 0x60 "Nothing hidden in the pit!",
/// 0x61 (trapdoor) prints 0x87a2, cell becomes 0x60 and the cell below (z+1, if z < 7) gets bit
/// 3 set, then the dungeon view is redrawn (DUNGEON 9c60(1) + rect_effect); 0x62 bomb trap: on a
/// successful roll "A bomb trap!" and cell &= 8, else "Nothing of note!"; 0x70 "Treasure!";
/// 0x80..0x83 names the field (sleep/poison/fire/electric, else energy); 0x90 "This tile is
/// impossible!"; 0xa0 "Nothing hidden on the door!"; 0xb0 "...wall!"; 0xc0: style 1
/// "...stalactite!", style 2 "...caved in passage!", else skeleton: "It crumbles away!" and the
/// cell becomes 0xb0|(cell&8) (wall) + redraw; 0xd0 "A hidden door!": cell becomes
/// (cell&8)-0x20 (0xe0/0xe8 door) + redraw; 0xe0/0xf0 "Nothing hidden on the door!".
pub fn search_dungeon_cell(gm: &mut Game) {
    let member = rest::select_target_member(gm) as i16;
    if member == -1 { return; }
    let s: u16;
    if gm.g8(g::TORCH_TURNS) == 0 && gm.g8(g::LIGHT_SPELL_TURNS) == 0 {
        s = 0x86de; // "You find: darkness!"
    } else {
        let facing = gm.g8(g::DUNGEON_FACING) as i16;
        let r = select_search_cell_dungeon(gm, facing);
        let cy = gm.g16(g::DY);
        let cx = gm.g16(g::DX);
        if r == 0 { return; }
        let z = gm.g8(g::PARTY_Z) as u16;
        let cell = gm.g8(dng_cell(z, cy, cx));
        let skill: u16 = (z.wrapping_mul(2).wrapping_sub(gm.g8(g::PARTY_DEX.wrapping_add(mem32(member))) as u16).wrapping_add(0x1e)) >> 1;
        text::print_string(gm, 0x86f4); // "You find: "
        let hi = cell & 0xf0;
        if hi == 0x70 {
            s = 0x87ca; // "Treasure!"
        } else if hi < 0x71 {
            if hi != 0x20 {
                if hi < 0x21 {
                    if hi == 0 {
                        text::print_string(gm, 0x8700); // "Nothing of note!"
                        return;
                    }
                    if hi != 0x10 { return; }
                } else if hi != 0x30 {
                    if hi == 0x40 {
                        let roll = util::rand_range(gm, 0x1e, 1);
                        let lvl: i16;
                        if (skill as i16) < roll {
                            if cell == 0x40 {
                                text::print_string(gm, 0x8732); // "No trap"
                                return;
                            }
                            lvl = gm.g8(g::PARTY_Z) as i16;
                        } else {
                            lvl = util::rand_range(gm, 8, 1);
                        }
                        let s2: u16 = if lvl < 4 { 0x873c } else if lvl < 7 { 0x875c } else { 0x874c }; // simple / trap / complex
                        text::print_string(gm, s2);
                        return;
                    }
                    if hi == 0x50 {
                        text::print_string(gm, 0x8764); // "Nothing hidden on the fountain!"
                        return;
                    }
                    if hi != 0x60 { return; }
                    if cell == 0x60 {
                        text::print_string(gm, 0x8786); // "Nothing hidden in the pit!"
                        return;
                    }
                    if cell != 0x61 {
                        if cell != 0x62 { return; }
                        let roll = util::rand_range(gm, 0x1e, 1);
                        if (skill as i16) < roll {
                            text::print_string(gm, 0x87aa); // "A bomb trap!"
                            let a = dng_cell(gm.g8(g::PARTY_Z) as u16, cy, cx);
                            let v = gm.g8(a) & 8;
                            gm.set_g8(a, v);
                            return;
                        }
                        text::print_string(gm, 0x87b8); // "Nothing of note!"
                        return;
                    }
                    text::print_string(gm, 0x87a2);
                    let a = dng_cell(gm.g8(g::PARTY_Z) as u16, cy, cx);
                    gm.set_g8(a, 0x60);
                    if gm.g8(g::PARTY_Z) < 7 {
                        let b = a.wrapping_add(0x40); // same cell one level down
                        let v = gm.g8(b) | 8;
                        gm.set_g8(b, v);
                    }
                    // LAB_1000_c81e
                    ovl::dungeon::render_dungeon_view(gm, 1);
                    gfx::rect_effect(gm, 0xb7, 0xb7, 8, 8);
                    return;
                }
            }
            s = 0x8712; // "Nothing hidden on the ladder!"
        } else if hi == 0xb0 {
            s = 0x886c; // "Nothing hidden on the wall!"
        } else {
            if 0xb0 < hi {
                let newv: u8;
                if hi == 0xc0 {
                    if (gm.g8(g::DUNGEON_STYLE) & 0xf) == 1 {
                        text::print_string(gm, 0x88ac); // "Nothing on the stalactite!"
                        return;
                    }
                    if (gm.g8(g::DUNGEON_STYLE) & 0xf) == 2 {
                        text::print_string(gm, 0x888a); // "Nothing in the caved in passage!"
                        return;
                    }
                    text::print_string(gm, 0x88c8); // "Nothing hidden on the skeleton!"
                    text::print_string(gm, 0x88ea); // "It crumbles away!"
                    newv = (cell & 8).wrapping_add(0xb0);
                } else {
                    if hi != 0xd0 {
                        if hi != 0xe0 && hi != 0xf0 { return; }
                        text::print_string(gm, 0x890e); // "Nothing hidden on the door!"
                        return;
                    }
                    text::print_string(gm, 0x88fe); // "A hidden door!"
                    newv = (cell & 8).wrapping_sub(0x20);
                }
                let a = dng_cell(gm.g8(g::PARTY_Z) as u16, cy, cx);
                gm.set_g8(a, newv);
                // LAB_1000_c81e
                ovl::dungeon::render_dungeon_view(gm, 1);
                gfx::rect_effect(gm, 0xb7, 0xb7, 8, 8);
                return;
            }
            if hi == 0x80 {
                s = match cell {
                    0x80 => 0x87d6, // "A sleep field!"
                    0x81 => 0x87e6, // "A poison gas field!"
                    0x82 => 0x87fc, // "A wall of fire!"
                    0x83 => 0x880e, // "An electric field!"
                    _ => 0x8822,    // "An energy field!"
                };
            } else if hi == 0x90 {
                s = 0x8834; // "This tile is impossible!"
            } else {
                if hi != 0xa0 { return; }
                s = 0x884e; // "Nothing hidden on the door!"
            }
        }
    }
    text::print_string(gm, s);
}

/// orig 1000:c8dc (524 bytes). The Search command (dispatch_command 'S', vector 1778:06aa). In a
/// dungeon (0x21..0x28) -> `search_dungeon_cell`. Else asks a direction and a member; the target
/// cell is party + (g_dx,g_dy). A chest object (tile 1) at the cell (slots 1..31; z must match
/// outside combat) -> "Thou dost find " + `search_chest_for_trap`. A moldy corpse (0x1f, topmost
/// object via find_object_at_rev) -> "Thou dost find " + `search_corpse(member, g_dx)`. Otherwise
/// a furniture-specific prefix by map tile (0x2b stump, 0x4f wall, 0x5a shelf, 0x5c/0x5d
/// bookshelf, 0xa1 well, 0xa5 desk, 0xa6 barrel, 0xa8 vanity, 0xab/0xac bed, 0xad dresser,
/// 0xaf trunk, 0xb2 brazier, 0xbc fireplace, else 0x8a34 "T") + "hou dost find ": tile 0x4e
/// (secret wall) reveals "a hidden door!" (tile 0xb9 outside combat, 0xb8 in combat, view
/// dirty); otherwise tries, in order, `dig_up_moonstone` (skipped on tile 0xdc),
/// `harvest_reagents`, `search_hidden_object`.
pub fn cmd_search(gm: &mut Game) {
    let loc = gm.g8(g::LOCATION);
    if 0x20 < loc && loc < 0x29 {
        search_dungeon_cell(gm);
        return;
    }
    if rest::get_direction(gm) == 0 { return; }
    let x = (gm.g8(g::PARTY_X) as u16).wrapping_add(gm.g16(g::DX));
    let y = (gm.g8(g::PARTY_Y) as u16).wrapping_add(gm.g16(g::DY));
    let member = rest::select_target_member(gm) as i16;
    if member == -1 { return; }
    let p = rest::get_map_tile(gm, y, x);
    let tile = gm.g8(p);
    let mut i: i16 = 1;
    loop {
        let o = slot8(i);
        if gm.g8(g::MONSTER_X.wrapping_add(o)) as u16 == x
            && gm.g8(g::MONSTER_Y.wrapping_add(o)) as u16 == y
            && (0x7f < loc || gm.g8(g::MONSTER_Z.wrapping_add(o)) == gm.g8(g::PARTY_Z))
            && gm.gi8(g::MONSTER_TABLE.wrapping_add(o)) == 1
        {
            break;
        }
        i += 1;
        if i >= 0x20 { break; }
    }
    if i < 0x20 {
        text::print_string(gm, 0x892c); // "Thou dost find "
        search_chest_for_trap(gm, member, i);
        return;
    }
    let pz = gm.g8(g::PARTY_Z) as u16;
    if rest::find_object_at_rev(gm, pz, y, x) == 0x1f {
        text::print_string(gm, 0x893e); // "Thou dost find "
        let slot = gm.gi16(g::DX);
        search_corpse(gm, member, slot);
        return;
    }
    let prefix: u16 = if tile == 0xa5 {
        0x8996 // "In the desk t"
    } else if tile < 0xa6 {
        if tile == 0x4f { 0x8a24 }          // "In the wall t"
        else if tile < 0x50 {
            if tile == 0x2b { 0x8950 } else { 0x8a34 } // "In the stump t"
        } else if tile == 0x5a { 0x8960 }   // "On the shelf t"
        else if 0x5b < tile {
            if tile < 0x5e { 0x8970 }       // "In the bookshelf t"
            else if tile == 0xa1 { 0x8984 } // "Near the well t"
            else { 0x8a34 }
        } else { 0x8a34 }
    } else {
        match tile {
            0xa6 => 0x89a6,        // "In the barrel t"
            0xa8 => 0x89b8,        // "In the vanity t"
            0xab | 0xac => 0x89ca, // "Under the bed t"
            0xad => 0x89dc,        // "In the dresser t"
            0xaf => 0x89ee,        // "In the trunk t"
            0xb2 => 0x8a12,        // "In the brazier t"
            0xbc => 0x89fe,        // "In the fireplace t"
            _ => 0x8a34,
        }
    };
    text::print_string(gm, prefix);
    text::print_string(gm, 0x8a38); // "hou dost find "
    if tile == 0x4e {
        text::print_string(gm, 0x8a48); // "a hidden door!"
        let q = rest::get_map_tile(gm, y, x);
        if gm.g8(g::PARTY_Z) < 0x80 {
            gm.set_g8(q, 0xb9);
        } else {
            gm.set_g8(q, 0xb8);
        }
        let d = gm.g8(g::VIEW_DIRTY) | 2;
        gm.set_g8(g::VIEW_DIRTY, d);
    } else if (tile == 0xdc || dig_up_moonstone(gm, pz, y, x) == 0) && harvest_reagents(gm, y, x) == 0 {
        search_hidden_object(gm, y, x);
    }
}

/// orig 1000:cb2a (146 bytes). Jimmy a chest object in monster slot `slot`. Needs a target
/// member. value1 (+5) < 0x80 (not locked/trapped) -> "Key broke!"; else roll rand 1..30 against
/// ((value1&0x7f) - DEX + 0x1e)/2 (byte): roll greater -> "Success!" and value1 &= 0x7f (chest
/// unlocked), otherwise "Key broke!". Every "Key broke!" plays a tone sweep and costs one key.
pub fn jimmy_chest(gm: &mut Game, param_1: i16) {
    let slot = param_1;
    let member = rest::select_target_member(gm) as i16;
    if member != -1 {
        let a = g::MONSTER_Z.wrapping_add(slot8(slot)).wrapping_add(1); // value1
        let v1 = gm.g8(a);
        let s: u16;
        if v1 < 0x80 {
            s = 0x8a58; // "Key broke!"
        } else {
            let dex = gm.g8(g::PARTY_DEX.wrapping_add(mem32(member))) as u16;
            let roll = util::rand_range(gm, 0x1e, 1) as u16;
            let diff = ((((v1 & 0x7f) as u16).wrapping_sub(dex).wrapping_add(0x1e)) >> 1) & 0xff;
            if diff < roll {
                text::print_string(gm, 0x8a64); // "Success!"
                gm.set_g8(a, v1 & 0x7f);
                return;
            }
            s = 0x8a6e; // "Key broke!"
        }
        text::print_string(gm, s);
        rest::play_tone_sweep(gm, 0x32, 1, 2000, 800);
        let k = gm.g8(g::KEYS).wrapping_sub(1);
        gm.set_g8(g::KEYS, k);
    }
}

/// orig 1000:cbbe (261 bytes). Dungeon Jimmy (0x8a7a prompt). Needs a target member; looks at
/// the party's own cell. (cell & 0xf7) == 0x40 (an unlocked chest): with keys "Key broke!" and
/// one key is lost, else "No keys!". Other 0x4x chests (locked): no keys -> "No keys!"; else
/// rand 1..30 > (2z - DEX + 0x1e)/2 -> "Chest unlocked", cell = 0x40|(cell&8); else "Key broke!"
/// and a key is lost. 0x7x (open chest) "Already open!"; anything else 0x8ac8 ("Not here!").
pub fn jimmy_dungeon_chest(gm: &mut Game) {
    text::print_string(gm, 0x8a7a);
    let member = rest::select_target_member(gm) as i16;
    if member == -1 { return; }
    let x = gm.g8(g::PARTY_X) as u16;
    let y = gm.g8(g::PARTY_Y) as u16;
    let z = gm.g8(g::PARTY_Z) as u16;
    let cell = gm.g8(dng_cell(z, y, x));
    let dex = gm.g8(g::PARTY_DEX.wrapping_add(mem32(member))) as u16;
    let s: u16;
    if (cell & 0xf7) == 0x40 {
        if gm.g8(g::KEYS) != 0 {
            text::print_string(gm, 0x8a86); // "Key broke!"
            let k = gm.g8(g::KEYS).wrapping_sub(1);
            gm.set_g8(g::KEYS, k);
            return;
        }
        s = 0x8a7c; // "No keys!"
    } else if (cell & 0xf0) == 0x40 {
        if gm.g8(g::KEYS) != 0 {
            let roll = util::rand_range(gm, 0x1e, 1);
            if (((z.wrapping_mul(2).wrapping_sub(dex).wrapping_add(0x1e)) >> 1) as i16) < roll {
                text::print_string(gm, 0x8a9c); // "Chest unlocked"
                let a = dng_cell(gm.g8(g::PARTY_Z) as u16, y, x);
                gm.set_g8(a, (cell & 8).wrapping_add(0x40));
                return;
            }
            text::print_string(gm, 0x8aac); // "Key broke!"
            let k = gm.g8(g::KEYS).wrapping_sub(1);
            gm.set_g8(g::KEYS, k);
            return;
        }
        s = 0x8a92; // "No keys!"
    } else if (cell & 0xf0) == 0x70 {
        s = 0x8ab8; // "Already open!"
    } else {
        s = 0x8ac8;
    }
    text::print_string(gm, s);
}

/// orig 1000:ccca (563 bytes). The Jimmy command (dispatch_command 'J', vector 1778:0692). In
/// a dungeon -> `jimmy_dungeon_chest`. No keys -> "No Keys!". Else asks a direction; target tile
/// t = map[party+(dx,dy)]: 0x97/0x98 (magic-locked doors) "Key broke!" (key lost); 0x84/0x85
/// (stocks/manacles with a prisoner): outside combat the cell must hold an object ("No one is
/// there!"); then a member is chosen and rand 0..29 >= DEX -> "Key broke!" (key lost); success
/// in a settlement: the NPC in that slot (find_npc_by_slot(g_dx)) has its mode (0x5f68) cleared,
/// if not already killed its three schedule AI types become 5 (freed, 0x5d5e..0x5d60), prints
/// " I thank thee!" and karma += 2 (cap 99); then set_npc_killed(npc) (removes the prisoner for
/// good) - "Couldn't find this npc" if no NPC owns the slot; in combat the tile becomes 0x44
/// (floor) and "Unlocked". 0xb9/0xbb (locked doors): member DEX check as above, success turns
/// the door into tile-1 (unlocked), "Unlocked!". Anything else: a chest object (tile 1) at the
/// cell -> `jimmy_chest(slot)`, else "No lock!".
pub fn cmd_jimmy(gm: &mut Game) {
    let loc = gm.g8(g::LOCATION);
    if 0x20 < loc && loc < 0x29 {
        jimmy_dungeon_chest(gm);
        return;
    }
    let s: u16;
    'done: {
        if gm.g8(g::KEYS) == 0 {
            s = 0x8ad0; // "No Keys!"
            break 'done;
        }
        if rest::get_direction(gm) == 0 { return; }
        let x = (gm.g8(g::PARTY_X) as u16).wrapping_add(gm.g16(g::DX));
        let y = (gm.g8(g::PARTY_Y) as u16).wrapping_add(gm.g16(g::DY));
        let p = rest::get_map_tile(gm, y, x);
        let t = gm.g8(p);
        if t < 0x99 {
            if 0x96 < t {
                text::print_string(gm, 0x8af2); // "Key broke!"
                let k = gm.g8(g::KEYS).wrapping_sub(1);
                gm.set_g8(g::KEYS, k);
                return;
            }
            if 0x83 < t && t < 0x86 {
                let pz = gm.g8(g::PARTY_Z) as u16;
                if loc < 0x80 && rest::find_object_at(gm, pz, y, x) == 0 {
                    s = 0x8afe; // "No one is there!"
                } else {
                    let slot = gm.gi16(g::DX);
                    let member = rest::select_target_member(gm) as i16;
                    if member == -1 { return; }
                    let roll = util::rand_range(gm, 0x1d, 0) as u16;
                    if gm.g8(g::PARTY_DEX.wrapping_add(mem32(member))) as u16 <= roll {
                        text::print_string(gm, 0x8b10); // "Key broke!"
                        let k = gm.g8(g::KEYS).wrapping_sub(1);
                        gm.set_g8(g::KEYS, k);
                        return;
                    }
                    if loc < 0x7f {
                        let npc = ovl::town::find_npc_by_slot(gm, slot);
                        if npc != -1 {
                            let mode = (npc as u16).wrapping_mul(0x10).wrapping_add(0x5f68); // g_npc_mode
                            if gm.gi16(mode) != 0 {
                                gm.set_gi16(mode, 0);
                            }
                            if ovl::town::is_npc_killed(gm, npc) == 0 {
                                let n16 = (npc as u16).wrapping_mul(0x10);
                                gm.set_g8(0x5d5eu16.wrapping_add(n16), 5); // schedule AI type 1
                                gm.set_g8(0x5d5fu16.wrapping_add(n16), 5); // AI type 2
                                gm.set_g8(0x5d60u16.wrapping_add(n16), 5); // AI type 3
                                text::print_string(gm, 0x8b36); // " I thank thee!"
                                rest::add_u8_capped(gm, 99, 2, g::KARMA);
                            }
                            ovl::town::set_npc_killed(gm, npc);
                            return;
                        }
                        s = 0x8b1c; // "Couldn't find this npc"
                    } else {
                        let q = rest::get_map_tile(gm, y, x);
                        gm.set_g8(q, 0x44);
                        let d = gm.g8(g::VIEW_DIRTY) | 2;
                        gm.set_g8(g::VIEW_DIRTY, d);
                        s = 0x8b48; // "Unlocked"
                    }
                }
                break 'done;
            }
        } else if t == 0xb9 || t == 0xbb {
            let member = rest::select_target_member(gm) as i16;
            if member == -1 { return; }
            let roll = util::rand_range(gm, 0x1d, 0) as u16;
            if gm.g8(g::PARTY_DEX.wrapping_add(mem32(member))) as u16 <= roll {
                text::print_string(gm, 0x8ada); // "Key broke!"
                let k = gm.g8(g::KEYS).wrapping_sub(1);
                gm.set_g8(g::KEYS, k);
                return;
            }
            let q = rest::get_map_tile(gm, y, x);
            gm.set_g8(q, t.wrapping_sub(1));
            let d = gm.g8(g::VIEW_DIRTY) | 2;
            gm.set_g8(g::VIEW_DIRTY, d);
            s = 0x8ae6; // "Unlocked!"
            break 'done;
        }
        let mut i: i16 = 1;
        loop {
            let o = slot8(i);
            if gm.g8(g::MONSTER_X.wrapping_add(o)) as u16 == x
                && gm.g8(g::MONSTER_Y.wrapping_add(o)) as u16 == y
                && (0x7f < loc || gm.g8(g::MONSTER_Z.wrapping_add(o)) == gm.g8(g::PARTY_Z))
                && gm.gi8(g::MONSTER_TABLE.wrapping_add(o)) == 1
            {
                break;
            }
            i += 1;
            if i >= 0x20 { break; }
        }
        if i < 0x20 {
            jimmy_chest(gm, i);
            return;
        }
        s = 0x8b52; // "No lock!"
    }
    text::print_string(gm, s);
}

/// orig 1000:cf08 (183 bytes). Spawns one piece of chest loot next to the chest. Ghidra order:
/// (found flag, quality, z, y, x, amount, kind). kind 3/4 (potion/scroll): amount = rand 1..quality
/// resp. 1..3*quality (the item index); kind 1/2 (chest/gold): amount-1 is used; for 1 and 2 the
/// amount is then re-rolled as rand 1..quality. A free slot from `find_free_slot_from_top`
/// receives set_monster_slot(slot, amount, z, y, x, kind, kind); outside combat anim state (+6)
/// = 0 and z (+4) = z, in combat anim state = 0x20. g_view_dirty |= 2, refresh, "Found:" once
/// (via the found flag) and `print_found_object_name(kind)`. Nothing happens when no slot is
/// free.
pub fn spawn_chest_loot(gm: &mut Game, found: &mut i16, param_2: i16, param_3: u16, param_4: u16, param_5: u16, param_6: i16, param_7: i16) {
    let (mut quality, z, y, x, mut amount, kind) = (param_2, param_3, param_4, param_5, param_6, param_7);
    if kind == 3 || kind == 4 {
        amount = amount.wrapping_sub(1);
    }
    let mut reroll = true;
    if kind != 1 {
        if kind != 2 {
            reroll = false; // LAB_1000_cf49
        } else {
            quality = quality.wrapping_mul(3);
        }
    }
    if reroll {
        amount = util::rand_range(gm, quality, 1);
    }
    let slot = find_free_slot_from_top(gm);
    if slot != 0 {
        rest::set_monster_slot(gm, slot, amount as u8, z as u8, y as u8, x as u8, kind as u8, kind as u8);
        let o = slot8(slot);
        if gm.g8(g::LOCATION) < 0x80 {
            gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(o), 0);
            gm.set_g8(g::MONSTER_Z.wrapping_add(o), z as u8);
        } else {
            gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(o), 0x20);
        }
        let d = gm.g8(g::VIEW_DIRTY) | 2;
        gm.set_g8(g::VIEW_DIRTY, d);
        rest::refresh_map_view(gm);
        if *found == 0 {
            text::print_string(gm, 0x8b5c); // "Found:"
            *found = 1;
        }
        print_found_object_name(gm, kind as u16);
    }
}

/// orig 1000:cfc0 (119 bytes). Fixed chest loot roll. Ghidra order (found, z, y, x, quality).
/// For each of the 8 loot classes i = 7..0 (kind 0x4123[i], minimum quality 0x412b[i], max
/// count 0x4133[i]): if min <= quality and min <= rand 1..30, amount = 1 when max == 1 else
/// rand 1..max, and `spawn_chest_loot(found, quality, z, y, x, amount, kind)`.
pub fn roll_fixed_chest_loot(gm: &mut Game, found: &mut i16, param_2: u16, param_3: u16, param_4: u16, param_5: u16) {
    let (z, y, x, quality) = (param_2, param_3, param_4, param_5);
    let mut i: i16 = 8;
    loop {
        let cur = i;
        i = cur - 1;
        if i < 0 { break; }
        let min = gm.g8(0x412bu16.wrapping_add(cur as u16)) as u16;
        if min <= quality {
            let roll = util::rand_range(gm, 0x1e, 1) as u16;
            if min <= roll {
                let max = gm.gi8(0x4133u16.wrapping_add(cur as u16));
                let amount: i16 = if max == 1 { 1 } else { util::rand_range(gm, max as i16, 1) };
                let kind = gm.g8(0x4123u16.wrapping_add(cur as u16)) as i16;
                spawn_chest_loot(gm, found, quality as i16, z, y, x, amount, kind);
            }
        }
    }
}

/// orig 1000:d038 (115 bytes). Random chest loot roll. Ghidra order (found, z, y, x, quality).
/// quality/2 + 1 times: pick entry r = rand 0..47 of the random loot table (kind 0x413c[r],
/// minimum quality 0x416c[r]); if min <= quality and min <= rand 1..30 then
/// `spawn_chest_loot(found, quality, z, y, x, r, kind)` (the table index doubles as the amount
/// / item index).
pub fn roll_random_chest_loot(gm: &mut Game, found: &mut i16, param_2: u16, param_3: u16, param_4: u16, param_5: u16) {
    let (z, y, x, quality) = (param_2, param_3, param_4, param_5);
    let mut n: i16 = (quality as i16) / 2;
    loop {
        let next = n - 1;
        if n < 0 { break; }
        let r = util::rand_range(gm, 0x2f, 0);
        n = next;
        let min = gm.g8(0x416cu16.wrapping_add(r as u16)) as u16;
        if min <= quality {
            let roll = util::rand_range(gm, 0x1e, 1) as u16;
            if min <= roll {
                let kind = gm.g8(0x413cu16.wrapping_add(r as u16)) as i16;
                spawn_chest_loot(gm, found, quality as i16, z, y, x, r, kind);
            }
        }
    }
}

/// orig 1000:d0ac (420 bytes). Opens the chest object at (z,y,x) (Ghidra order). Scans slots
/// 1..31 for an object at the cell (z ignored in combat): tile 1 = chest -> found; tile 0xe
/// (a sleeping/held NPC?) -> prints 0x8b64 and returns. None -> "Nothing to open!". Then a
/// target member is chosen, the chest's value1 (quality, bit 7 = trapped) is read and the slot
/// cleared (view dirty |= 2). In a settlement karma -= 2 (min 0). Trapped: "Trapped!" +
/// trap_effect(member); if that killed the member during combat (status 'D', location >= 0x80)
/// the member's roster entry (flags bit 7, +3 == member) gets flag 0x20 and its monster slot
/// tile/anim tile become 0x1e (corpse), the active member is cleared if it was this one, and the
/// view refreshed. Finally `roll_fixed_chest_loot` and `roll_random_chest_loot` with the
/// quality; "Chest empty!" if neither produced anything.
pub fn open_chest(gm: &mut Game, param_1: u16, param_2: u16, param_3: u16) {
    let (z, y, x) = (param_1, param_2, param_3);
    let mut found: i16 = 0;
    let mut i: i16 = 1;
    let loc = gm.g8(g::LOCATION);
    let s: u16;
    'done: {
        loop {
            let o = slot8(i);
            if gm.g8(g::MONSTER_X.wrapping_add(o)) as u16 == x
                && gm.g8(g::MONSTER_Y.wrapping_add(o)) as u16 == y
                && (0x7f < loc || (loc < 0x80 && gm.g8(g::MONSTER_Z.wrapping_add(o)) as u16 == z))
            {
                let t = gm.gi8(g::MONSTER_TABLE.wrapping_add(o));
                if t == 1 { break; }
                if t == 0xe {
                    s = 0x8b64;
                    break 'done;
                }
            }
            i += 1;
            if i >= 0x20 { break; }
        }
        if i == 0x20 {
            s = 0x8b6c; // "Nothing to open!"
            break 'done;
        }
        let member = rest::select_target_member(gm);
        if member == 0xffff { return; }
        let mut quality = gm.g8(g::MONSTER_Z.wrapping_add(slot8(i)).wrapping_add(1)); // value1
        rest::set_monster_slot(gm, i, 0, 0, 0, 0, 0, 0);
        let d = gm.g8(g::VIEW_DIRTY) | 2;
        gm.set_g8(g::VIEW_DIRTY, d);
        if loc != 0 && loc < 0x21 {
            if gm.g8(g::KARMA) < 3 {
                gm.set_g8(g::KARMA, 0);
            } else {
                let k = gm.g8(g::KARMA) - 2;
                gm.set_g8(g::KARMA, k);
            }
        }
        if 0x7f < quality {
            quality &= 0x7f;
            text::print_string(gm, 0x8b7e); // "Trapped!"
            rest::trap_effect(gm, member);
            if 0x7f < gm.g8(g::LOCATION) && gm.g8(g::PARTY_STATUS.wrapping_add(mem32(member as i16))) == 0x44 {
                let mut r: u16 = g::COMBAT_ROSTER;
                loop {
                    if (gm.g8(r.wrapping_add(2)) & 0x80) != 0 && gm.g8(r.wrapping_add(3)) as u16 == member {
                        let f = gm.g8(r.wrapping_add(2)) | 0x20;
                        gm.set_g8(r.wrapping_add(2), f);
                        let slot = gm.g8(r.wrapping_add(4)) as u16;
                        gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot.wrapping_mul(8)).wrapping_add(1), 0x1e);
                        gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot.wrapping_mul(8)), 0x1e);
                        break;
                    }
                    r = r.wrapping_add(8);
                    if !(r < 0xbb14) { break; }
                }
                if member == gm.g8(g::ACTIVE_MEMBER) as u16 {
                    gm.set_g8(g::ACTIVE_MEMBER, 0xff);
                }
                rest::refresh_map_view(gm);
            }
        }
        roll_fixed_chest_loot(gm, &mut found, z, y, x, quality as u16);
        roll_random_chest_loot(gm, &mut found, z, y, x, quality as u16);
        if found != 0 { return; }
        s = 0x8b88; // "Chest empty!"
    }
    text::print_string(gm, s);
}

/// orig 1000:d254 (160 bytes). Dungeon Open: the party's own cell. 0x4x chest: a target member
/// is chosen, cell & 7 != 0 (trapped) -> trap_effect(member); the cell becomes 0x70|(cell&8)
/// (open chest) and "Chest opened". 0x7x "Already Open!"; else 0x8bb6 ("Nothing to open!").
pub fn open_dungeon_chest(gm: &mut Game) {
    let a = dng_cell(gm.g8(g::PARTY_Z) as u16, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
    let cell = gm.g8(a);
    let s: u16;
    if (cell & 0xf0) == 0x40 {
        let member = rest::select_target_member(gm);
        if member == 0xffff { return; }
        if (cell & 7) != 0 {
            rest::trap_effect(gm, member);
        }
        let a2 = dng_cell(gm.g8(g::PARTY_Z) as u16, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
        gm.set_g8(a2, (cell & 8).wrapping_add(0x70));
        s = 0x8b96; // "Chest opened"
    } else if (cell & 0xf0) == 0x70 {
        s = 0x8ba6; // "Already Open!"
    } else {
        s = 0x8bb6;
    }
    text::print_string(gm, s);
}

/// orig 1000:d2f4 (222 bytes). The Open command (dispatch_command 'O', vector 1778:069e). In a
/// dungeon -> `open_dungeon_chest`. Else the previously opened door is put back
/// (set_town_map_tile(g_door_y, g_door_x, g_door_tile)), a direction is asked and the target
/// tile examined: 0xaf (trunk) "It's open!"; 0x99 "Too heavy!"; 0xb8/0xba (unlocked doors) open:
/// g_door_timer = 4, g_door_x/y/tile remember the door, the tile becomes 0x44, g_view_dirty = 1,
/// "Opened!"; 0xb9/0xbb (locked doors) and 0x97/0x98 (magic doors) "Locked!"; everything else
/// -> `open_chest(z, y, x)`.
pub fn cmd_open(gm: &mut Game) {
    let loc = gm.g8(g::LOCATION);
    if 0x20 < loc && loc < 0x29 {
        open_dungeon_chest(gm);
        return;
    }
    let (dy, dx, dt) = (gm.g8(g::DOOR_Y), gm.g8(g::DOOR_X), gm.gi8(g::DOOR_TILE));
    rest::set_town_map_tile(gm, dy, dx, dt);
    if rest::get_direction(gm) == 0 { return; }
    let x = (gm.g8(g::PARTY_X) as u16).wrapping_add(gm.g16(g::DX));
    let y = (gm.g8(g::PARTY_Y) as u16).wrapping_add(gm.g16(g::DY));
    let p = rest::get_map_tile(gm, y, x);
    let t = gm.g8(p);
    let s: u16;
    if t == 0xaf {
        s = 0x8bbe; // "It's open!"
    } else if t < 0xb0 {
        if t < 0x97 {
            open_chest(gm, gm.g8(g::PARTY_Z) as u16, y, x);
            return;
        }
        if 0x98 < t {
            if t == 0x99 {
                s = 0x8bca; // "Too heavy!"
            } else {
                open_chest(gm, gm.g8(g::PARTY_Z) as u16, y, x);
                return;
            }
        } else {
            s = 0x8bd6; // "Locked!"
        }
    } else if t == 0xb8 || t == 0xba {
        gm.set_g8(g::DOOR_TIMER, 4);
        gm.set_g8(g::DOOR_X, x as u8);
        gm.set_g8(g::DOOR_Y, y as u8);
        gm.set_g8(g::DOOR_TILE, t);
        let q = rest::get_map_tile(gm, y, x);
        gm.set_g8(q, 0x44);
        gm.set_g8(g::VIEW_DIRTY, 1);
        s = 0x8be0; // "Opened!"
    } else if t == 0xb9 || t == 0xbb {
        s = 0x8bd6; // "Locked!"
    } else {
        open_chest(gm, gm.g8(g::PARTY_Z) as u16, y, x);
        return;
    }
    text::print_string(gm, s);
}

/// orig 1000:d3d8 (813 bytes). Takes an item into the inventory. Ghidra order (slot, value,
/// kind); slot 0x20 means "no object to clear" (dungeon chests). kind: 1 chest "Open it first!"
/// (nothing cleared); 2 gold: "N" + 0x8cf0, g_gold += value (cap 9999); 3 potion: name from
/// 0x419c[value] + " potion!", g_potions[value]++ (cap 99); 4 scroll: value 0xff = "The plans
/// for the HMS Cape!" (g_hms_cape_plans = 0xff), else "A scroll!" + rune-font name
/// 0x41ac[value&7] + 0x8cec, g_scrolls[value]++ (cap 99); 5,6,9..0xc (weapon/shield/ring/armour/
/// amulet): g_inventory[value]++ (cap 99; values 0x1b/0x1d get +5), prints the item name
/// (0x17f6[value]) + 0x8d06; 7 keys: value < 0x80 "N" + 0x8cb4 keys, else (value&0x7f) "odd
/// key"s (0x57b1 skull keys), capped at 99, then 0x8cba/0x8cbe singular/plural; 8 gems; 0xd
/// torches; 0xe sandalwood box (g_sandalwood_box = 0xff, 0x5b9d |= 0x80); 0xf food (cap 9999);
/// 0x19 moonstone: g_moonstone_loc[value] = 0xff ("A moonstone!"); 0x1b magic carpet
/// (g_magic_carpets++, cap 99; in location 0x11 NPC 0x16 is removed); 0xb4..0xb7: shard of
/// Falsehood/Hatred/Cowardice (g_shards[value&3] = 0xff), the Crown (also kills and removes the
/// NPC owning the slot), Sceptre, Amulet of Lord British; other kinds "Nothing to get!" (nothing
/// cleared). Afterwards (except for the early-return cases) slot < 0x20 is cleared,
/// g_status_dirty = 1 and g_view_dirty |= 2.
pub fn take_item(gm: &mut Game, param_1: i16, param_2: u16, param_3: i16) {
    let (slot, mut value, kind) = (param_1, param_2, param_3);
    let mut s: u16 = 0;
    let mut print_s = false;
    'tail: {
        if kind < 0xd {
            if kind < 9 {
                match kind {
                    1 => {
                        text::print_string(gm, 0x8c3e); // "Open it first!"
                        return;
                    }
                    2 => {
                        text::print_number(gm, 0x20, 1, value as i16);
                        text::print_string(gm, 0x8cf0);
                        rest::add_i16_capped(gm, 0x270f, value as i16, g::GOLD);
                        break 'tail;
                    }
                    3 => {
                        text::print_string(gm, 0x8cf8);
                        let p = gm.g16(g::STR_POTION_SCROLL_PTRS.wrapping_add(value.wrapping_mul(2)));
                        text::print_string(gm, p);
                        text::print_string(gm, 0x8cfc); // " potion!"
                        let a = g::POTIONS.wrapping_add(value);
                        let v = gm.g8(a).wrapping_add(1);
                        gm.set_g8(a, v);
                        if gm.g8(a) == 100 { gm.set_g8(a, 99); }
                        break 'tail;
                    }
                    4 => {
                        if value == 0xff {
                            text::print_string(gm, 0x8cc2); // "The plans for the HMS Cape!"
                            gm.set_g8(g::HMS_CAPE_PLANS, 0xff);
                        } else {
                            text::print_string(gm, 0x8ce0); // "A scroll!"
                            text::select_font(gm, 1);
                            let p = gm.g16(0x41acu16.wrapping_add((value & 7).wrapping_mul(2)));
                            text::print_string(gm, p);
                            text::select_font(gm, 0);
                            text::print_string(gm, 0x8cec);
                            let a = g::SCROLLS.wrapping_add(value);
                            let v = gm.g8(a).wrapping_add(1);
                            gm.set_g8(a, v);
                            if gm.g8(a) == 100 { gm.set_g8(a, 99); }
                        }
                        break 'tail;
                    }
                    5 | 6 => { /* falls to the equipment case below */ }
                    7 => {
                        let ptr: u16;
                        if (value as i16) < 0x80 {
                            text::print_number(gm, 0x20, 1, value as i16);
                            text::print_string(gm, 0x8cb4);
                            ptr = g::KEYS;
                        } else {
                            value &= 0x7f;
                            text::print_number(gm, 0x20, 1, value as i16);
                            text::print_string(gm, 0x8caa); // "odd key"
                            ptr = 0x57b1; // skull keys
                        }
                        rest::add_u8_capped(gm, 99, value as i16, ptr);
                        s = if value == 1 { 0x8cba } else { 0x8cbe };
                        print_s = true;
                        break 'tail;
                    }
                    8 => {
                        text::print_number(gm, 0x20, 1, value as i16);
                        text::print_string(gm, 0x8c9c);
                        let t: u16 = if value == 1 { 0x8ca2 } else { 0x8ca6 };
                        text::print_string(gm, t);
                        rest::add_u8_capped(gm, 99, value as i16, g::GEMS);
                        break 'tail;
                    }
                    _ => {
                        text::print_string(gm, 0x8d92); // "Nothing to get!"
                        return;
                    }
                }
            }
            // switchD_1000_d3fd_caseD_5: weapons / shields / rings / armour / amulets
            if value == 0x1b || value == 0x1d {
                rest::add_u8_capped(gm, 99, 5, g::INVENTORY.wrapping_add(value));
            } else {
                let a = g::INVENTORY.wrapping_add(value);
                let v = gm.g8(a).wrapping_add(1);
                gm.set_g8(a, v);
                if gm.g8(a) == 100 { gm.set_g8(a, 99); }
            }
            let p = gm.g16(g::NAME_PTRS.wrapping_add(value.wrapping_mul(2)));
            text::print_string(gm, p);
            s = 0x8d06;
            print_s = true;
        } else {
            if kind == 0x19 {
                text::print_string(gm, 0x8c4e); // "A moonstone!"
                gm.set_g8(g::MOONSTONE_LOC.wrapping_add(value), 0xff);
                break 'tail;
            }
            if kind < 0x1a {
                if kind == 0xd {
                    text::print_number(gm, 0x20, 1, value as i16);
                    text::print_string(gm, 0x8c8a); // "torch"
                    let t: u16 = if value == 1 { 0x8c92 } else { 0x8c96 };
                    text::print_string(gm, t);
                    rest::add_u8_capped(gm, 99, value as i16, g::TORCHES);
                    break 'tail;
                }
                if kind == 0xe {
                    text::print_string(gm, 0x8c76); // "A sandalwood box!"
                    gm.set_g8(g::SANDALWOOD_BOX, 0xff);
                    let v = gm.g8(0x5b9d) | 0x80; // DAT_1f64_5b9d (save 0x5f7)
                    gm.set_g8(0x5b9d, v);
                    break 'tail;
                }
                if kind != 0xf {
                    text::print_string(gm, 0x8d92); // "Nothing to get!"
                    return;
                }
                text::print_number(gm, 0x20, 1, value as i16);
                text::print_string(gm, 0x8c6e);
                rest::add_i16_capped(gm, 0x270f, value as i16, g::FOOD);
                break 'tail;
            }
            if kind == 0x1b {
                text::print_string(gm, 0x8c5c); // "A magic carpet!"
                let v = gm.g8(g::MAGIC_CARPETS).wrapping_add(1);
                gm.set_g8(g::MAGIC_CARPETS, v);
                if gm.g8(g::MAGIC_CARPETS) == 100 { gm.set_g8(g::MAGIC_CARPETS, 99); }
                if gm.g8(g::LOCATION) != 0x11 { break 'tail; }
                ovl::town::remove_npc(gm, 0x16);
                break 'tail;
            }
            if kind == 0xb4 {
                value &= 3;
                gm.set_g8(g::SHARD_FALSEHOOD.wrapping_add(value), 0xff);
                text::print_string(gm, 0x8d0a); // "The Shard of "
                s = if value == 0 { 0x8d18 } else if value == 1 { 0x8d24 } else { 0x8d2e }; // Falsehood/Hatred/Cowardice
                print_s = true;
            } else {
                if kind == 0xb5 {
                    gm.set_g8(g::CROWN_LB, 0xff);
                    text::print_string(gm, 0x8d3a); // "The Crown of Lord British!"
                    let npc = ovl::town::find_npc_by_slot(gm, slot);
                    ovl::town::set_npc_killed(gm, npc);
                    let npc = ovl::town::find_npc_by_slot(gm, slot);
                    ovl::town::remove_npc(gm, npc);
                    break 'tail;
                }
                if kind == 0xb6 {
                    gm.set_g8(g::SCEPTRE, 0xff);
                    s = 0x8d56; // "The Sceptre of Lord British!"
                    print_s = true;
                } else {
                    if kind != 0xb7 {
                        text::print_string(gm, 0x8d92); // "Nothing to get!"
                        return;
                    }
                    gm.set_g8(g::AMULET_LB, 0xff);
                    s = 0x8d74; // "The Amulet of Lord British!"
                    print_s = true;
                }
            }
        }
    }
    if print_s {
        text::print_string(gm, s);
    }
    // LAB_1000_d6fa
    if slot < 0x20 {
        rest::set_monster_slot(gm, slot, 0, 0, 0, 0, 0, 0);
    }
    gm.set_g8(g::STATUS_DIRTY, 1);
    let d = gm.g8(g::VIEW_DIRTY) | 2;
    gm.set_g8(g::VIEW_DIRTY, d);
}

/// orig 1000:d71e (300 bytes). Dungeon Get (0x8da4 prompt) on the party's own cell. 0x4x chest
/// "Must open first!"; 0x7x open chest: the cell is reduced to cell & 8 (emptied), "contents of
/// chest You find:" and seven loot rolls i = 0..6: roll = rand 1..(4z+4) must be >= g_unk_41bc[i];
/// then i == 5 -> potion (kind 3) rand 0..7, i == 6 -> scroll (kind 4) rand 0..7, else kind
/// 0x41cc[i] with value rand 1..max where max = 8z for i == 1 (gold) else 0x41c4[i]; each goes
/// through `take_item(0x20, value, kind)`. Anything else "Not here!".
pub fn get_dungeon_chest_contents(gm: &mut Game) {
    text::print_string(gm, 0x8da4);
    let a = dng_cell(gm.g8(g::PARTY_Z) as u16, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
    let cell = gm.g8(a);
    let s: u16;
    if (cell & 0xf0) == 0x40 {
        s = 0x8daa; // "Must open first!"
    } else {
        if (cell & 0xf0) == 0x70 {
            let a2 = dng_cell(gm.g8(g::PARTY_Z) as u16, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
            let v = gm.g8(a2) & 8;
            gm.set_g8(a2, v);
            text::print_string(gm, 0x8dbc); // "contents of chest You find:"
            let mut i: i16 = 0;
            loop {
                let hi = (gm.g8(g::PARTY_Z) as i16).wrapping_mul(4).wrapping_add(4);
                let roll = util::rand_range(gm, hi, 1) as u16;
                if gm.g8(g::UNK_41BC.wrapping_add(i as u16)) as u16 <= roll {
                    let value: i16;
                    let kind: u8;
                    if i == 5 {
                        value = util::rand_range(gm, 7, 0);
                        kind = 3;
                    } else if i == 6 {
                        value = util::rand_range(gm, 7, 0);
                        kind = 4;
                    } else {
                        let max: u16 = if i == 1 {
                            (gm.g8(g::PARTY_Z) as u16) << 3
                        } else {
                            gm.g8(0x41c4u16.wrapping_add(i as u16)) as u16
                        };
                        value = util::rand_range(gm, max as i16, 1);
                        kind = gm.g8(0x41ccu16.wrapping_add(i as u16));
                    }
                    take_item(gm, 0x20, value as u16, kind as i16);
                }
                i += 1;
                if !(i < 7) { break; }
            }
            return;
        }
        s = 0x8dda; // "Not here!"
    }
    text::print_string(gm, s);
}

/// orig 1000:d84e (608 bytes). The Get command (dispatch_command 'G', vector 1778:0686). In a
/// dungeon -> `get_dungeon_chest_contents`. Else asks a direction, prints 0x8de6 and scans slots
/// 1..31 for a takeable object at party+(dx,dy) (z must match outside combat): tile < 0x10,
/// 0x19 (moonstone), 0x1b (carpet) or 0xb4..0xb7 (shards/crown/sceptre/amulet) ->
/// `take_item(slot, value1, tile)`. Otherwise by map tile: 0x9a/0x9b (plates on a table, only
/// reachable from dy == +1 resp. -1 - "Can't reach plate!") and 0x9c (a table with food, not
/// from the sides) take the food (tile becomes 0x95 / the other plate), +1 food, karma -1;
/// 0x2d (ripe crops) -> 0x2c "Crops picked!" +1 food, status dirty, karma -1 (min 0);
/// 0xb0/0xb1 (torch sconces): the tile becomes 0x44, g_view_dirty = 1, rebuild_light_map
/// outside combat, g_torch_turns = 100, "Borrowed!" + tone; anything else "Nothing to get!".
pub fn cmd_get(gm: &mut Game) {
    let loc = gm.g8(g::LOCATION);
    if 0x20 < loc && loc < 0x29 {
        get_dungeon_chest_contents(gm);
        return;
    }
    let r = rest::get_direction(gm);
    let dy = gm.gi16(g::DY);
    let dx = gm.gi16(g::DX);
    if r == 0 { return; }
    let x = (gm.g16(g::DX)).wrapping_add(gm.g8(g::PARTY_X) as u16);
    let y = (gm.g16(g::DY)).wrapping_add(gm.g8(g::PARTY_Y) as u16);
    text::print_string(gm, 0x8de6);
    let mut i: i16 = 1;
    let mut tile: u16 = 0;
    loop {
        let o = slot8(i);
        if gm.g8(g::MONSTER_X.wrapping_add(o)) as u16 == x
            && gm.g8(g::MONSTER_Y.wrapping_add(o)) as u16 == y
            && (0x7f < loc || (loc < 0x80 && gm.g8(g::MONSTER_Z.wrapping_add(o)) == gm.g8(g::PARTY_Z)))
        {
            let t = gm.g8(g::MONSTER_TABLE.wrapping_add(o));
            tile = t as u16;
            if t < 0x10 || tile == 0x19 || tile == 0x1b || (t & 0xfc) == 0xb4 {
                break;
            }
        }
        i += 1;
        if i >= 0x20 { break; }
    }
    if i < 0x20 {
        let v1 = gm.g8(g::MONSTER_Z.wrapping_add(slot8(i)).wrapping_add(1)) as u16;
        take_item(gm, i, v1, tile as i16);
        return;
    }
    let p = rest::get_map_tile(gm, y, x);
    let t = gm.g8(p);
    let s: u16;
    let mut karma_tail = false; // LAB_1000_d9d8 only
    if t == 0x9a {
        if dy != 1 {
            text::print_string(gm, 0x8e10); // "Can't reach plate!"
            return;
        }
        let q = rest::get_map_tile(gm, y, x);
        gm.set_g8(q, 0x95);
        s = 0x8e04;
    } else if 0x9a < t {
        let s2: u16;
        if t == 0x9b {
            if dy != -1 {
                text::print_string(gm, 0x8e30); // "Can't reach plate!"
                return;
            }
            let q = rest::get_map_tile(gm, y, x);
            gm.set_g8(q, 0x95);
            s2 = 0x8e24;
        } else if t != 0x9c {
            if 0xaf < t && t < 0xb2 {
                let q = rest::get_map_tile(gm, y, x);
                gm.set_g8(q, 0x44);
                gm.set_g8(g::VIEW_DIRTY, 1);
                if gm.g8(g::LOCATION) < 0x80 {
                    rest::rebuild_light_map(gm);
                }
                gm.set_g8(g::TORCH_TURNS, 100);
                text::print_string(gm, 0x8de8); // "Borrowed!"
                rest::play_tone_sweep(gm, 0x32, 1, 2000, 800);
                rest::refresh_map_view(gm);
                return;
            }
            text::print_string(gm, 0x8e64); // "Nothing to get!"
            return;
        } else {
            if dx == 1 || dx == -1 {
                text::print_string(gm, 0x8e44); // "Can't reach plate!"
                return;
            }
            if dy == 1 {
                let q = rest::get_map_tile(gm, y, x);
                gm.set_g8(q, 0x9b);
            }
            if dy == -1 {
                let q = rest::get_map_tile(gm, y, x);
                gm.set_g8(q, 0x9a);
            }
            s2 = 0x8e58;
        }
        let d = gm.g8(g::VIEW_DIRTY) | 2;
        gm.set_g8(g::VIEW_DIRTY, d);
        text::print_string(gm, s2);
        rest::add_i16_capped(gm, 0x270f, 1, g::FOOD);
        karma_tail = true;
        s = 0;
    } else if t != 0x2d {
        text::print_string(gm, 0x8e64); // "Nothing to get!"
        return;
    } else {
        let q = rest::get_map_tile(gm, y, x);
        gm.set_g8(q, 0x2c);
        s = 0x8df4; // "Crops picked!"
    }
    if !karma_tail {
        let d = gm.g8(g::VIEW_DIRTY) | 2;
        gm.set_g8(g::VIEW_DIRTY, d);
        text::print_string(gm, s);
        rest::add_i16_capped(gm, 0x270f, 1, g::FOOD);
        gm.set_g8(g::STATUS_DIRTY, 1);
    }
    // LAB_1000_d9d8
    if gm.g8(g::KARMA) != 0 {
        let k = gm.g8(g::KARMA).wrapping_sub(1);
        gm.set_g8(g::KARMA, k);
    }
}

/// orig 1000:dab4 (55 bytes). Unequips `item` from every party member (remove_equipped_item for
/// members 0..g_party_count-1) and returns how many members had it equipped. Vector 1778:088a.
pub fn remove_item_from_all_members(gm: &mut Game, param_1: u16) -> i16 {
    let item = param_1;
    let mut n: i16 = 0;
    let mut i: u16 = 0;
    while i < gm.g8(g::PARTY_COUNT) as u16 {
        if rest::remove_equipped_item(gm, item, i as i16) != 0 {
            n += 1;
        }
        i += 1;
    }
    n
}

/// orig 1000:daec (70 bytes). Counts the live combatants per side into g_dy (party side,
/// combatant_side == 0) and g_dx (enemy side): roster entries 0..31 with non-zero flags and
/// flag 0x20 (out of combat) clear. Vector 1778:070a.
pub fn count_combatants_by_side(gm: &mut Game) {
    gm.set_g16(g::DY, 0);
    gm.set_g16(g::DX, 0);
    let mut i: i16 = 0;
    loop {
        let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(i)));
        if f != 0 && (f & 0x20) == 0 {
            if !rest::combatant_side(gm, i) {
                let v = gm.g16(g::DY).wrapping_add(1);
                gm.set_g16(g::DY, v);
            } else {
                let v = gm.g16(g::DX).wrapping_add(1);
                gm.set_g16(g::DX, v);
            }
        }
        i += 1;
        if !(i < 0x20) { break; }
    }
}

/// orig 1000:db32 (108 bytes). A combatant steps off the battlefield in direction `exit_dir`
/// (Ghidra order (exit_dir, roster_index)). On a frigate (transport tile & 0xf8 == 0x20) "Stay
/// with ship!" and 0. For a party member (flags bit 7): the first leaver sets g_combat_exit =
/// dir; later ones must use the same exit when g_combat_flags & 0x80 ("All must use the same
/// exit!", beep, 0). Then g_focus_blink = 1, the sides are counted and 0x8ea6 ("Victory!") is
/// printed when no enemies remain, else "Escape!"; tone sweep, g_active_member = 0xff, COMBAT
/// b4c6(-1 - roster_index) removes the combatant, status window + view refresh; returns 1. The
/// 1000:dba0 block Ghidra split off is the "Escape!" branch of this function.
pub fn leave_combat(gm: &mut Game, param_1: u16, param_2: i16) -> u16 {
    let (dir, ri) = (param_1, param_2);
    if (gm.g8(g::TRANSPORT_TILE) & 0xf8) == 0x20 {
        text::print_string(gm, 0x8e76); // "Stay with ship!"
        return 0;
    }
    if (gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(ri))) & 0x80) != 0 {
        if gm.g8(g::COMBAT_EXIT) == 0 {
            gm.set_g8(g::COMBAT_EXIT, dir as u8);
        } else if dir != gm.g8(g::COMBAT_EXIT) as u16 && (gm.g8(g::COMBAT_FLAGS) & 0x80) != 0 {
            text::print_string(gm, 0x8e88); // "All must use the same exit!"
            util::beep(gm, 200, 0xa5);
            return 0;
        }
    }
    gm.set_g8(g::FOCUS_BLINK, 1);
    count_combatants_by_side(gm);
    let s: u16 = if gm.g16(g::DX) == 0 { 0x8ea6 } else { 0x8eae }; // "Victory!" / "Escape!"
    text::print_string(gm, s);
    rest::play_tone_sweep(gm, 0x28, 1, 2000, 0x4b0);
    gm.set_g8(g::ACTIVE_MEMBER, 0xff);
    ovl::combat::remove_combatant(gm, (-1i16).wrapping_sub(ri));
    rest::draw_status_window(gm);
    rest::refresh_map_view(gm);
    1
}

/// orig 1000:dba0 (54 bytes). Not a function: the "Escape!" tail of `leave_combat` (db32),
/// which Ghidra split off because the CMDS Klimb vector (1778:091a) jumps to this address in
/// the CMDS overlay. Its code is translated inside `leave_combat`.
pub fn fn_1000_dba0_fragment(gm: &mut Game) -> u16 { 1 }

/// orig 1000:dbd6 (274 bytes). Moves combatant `roster_index` one step in `dir` (1 W, 2 E, 3 N,
/// 4 S; the direction name is printed). Ghidra order (dir, roster_index). Inside the 11x11 map:
/// COMBAT a290(y, x, tile) decides passability - blocked: "Blocked!", beep, flush keyboard,
/// returns 0; else the roster x/y (+6/+7) and the monster slot x/y are updated, a footstep
/// plays, and in dungeon-room combat (g_combat_flags & 0x82) COMBAT b3aa(y, x) handles room
/// triggers; returns 1. Off the map -> `leave_combat(dir, roster_index)`. Vector 1778:05ba.
pub fn move_combatant(gm: &mut Game, param_1: i16, param_2: i16) -> u16 {
    let (dir, ri) = (param_1, param_2);
    let mut dy: i16 = 0;
    let mut dx: i16 = 0;
    let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(ros8(ri))) as u16;
    let s: u16 = if dir == 1 {
        dx = -1;
        0x8ece // "West"
    } else if dir == 2 {
        dx = 1;
        0x8ec8 // "East"
    } else if dir == 3 {
        dy = -1;
        0x8eb8 // "North"
    } else if dir == 4 {
        dy = 1;
        0x8ec0 // "South"
    } else {
        0
    };
    if s != 0 {
        text::print_string(gm, s);
    }
    let nx = (gm.g8(g::ROSTER_X.wrapping_add(ros8(ri))) as i16).wrapping_add(dx);
    let ny = (gm.g8(g::ROSTER_Y.wrapping_add(ros8(ri))) as i16).wrapping_add(dy);
    if nx < 0xb && ny < 0xb && -1 < nx && -1 < ny {
        let tile = gm.g8(g::MONSTER_TABLE.wrapping_add(slot.wrapping_mul(8))) as u16;
        if ovl::combat::combat_cell_free(gm, ny as u16, nx as u16, tile) == 0 {
            text::print_string(gm, 0x8ed4); // "Blocked!"
            util::beep(gm, 200, 0xa5);
            input::flush_keyboard(gm);
            0
        } else {
            gm.set_g8(g::ROSTER_X.wrapping_add(ros8(ri)), nx as u8);
            gm.set_g8(g::MONSTER_X.wrapping_add(slot.wrapping_mul(8)), nx as u8);
            gm.set_g8(g::ROSTER_Y.wrapping_add(ros8(ri)), ny as u8);
            gm.set_g8(g::MONSTER_Y.wrapping_add(slot.wrapping_mul(8)), ny as u8);
            rest::play_footstep_sound(gm);
            if (gm.g8(g::COMBAT_FLAGS) & 0x82) != 0 {
                ovl::combat::check_room_trigger(gm, ny as u16, nx as u16);
            }
            1
        }
    } else {
        leave_combat(gm, dir as u16, ri)
    }
}

/// orig 1000:dcea (311 bytes). Combat Klimb (vector 1778:06f2; prints 0x8ede). The tile under
/// the party position is examined: in a dungeon room (g_combat_flags & 2) on tile 0xc8 with
/// g_tiles_patched set, asks 0x8ee6 ("Up or Down?") - 3/'U' keeps 0xc8, 4/'D' makes it 0xc9. Tile
/// 0xc8 -> prints 0x8eec and leaves via exit 5 (up); tile 0x86 in a 0x80-flagged fight or tile
/// 0xc9 -> 0x8ef2, exit 6 (down); both return `leave_combat(exit, g_active_combatant)`.
/// Otherwise a direction is asked and the active combatant climbs onto an adjacent tile 0x4c
/// (ladder) with no object on it (roster and monster x/y updated, COMBAT b3aa(y,x)); a wrong
/// target prints 0x8efa and returns 0; returns 1.
pub fn combat_klimb(gm: &mut Game) -> u16 {
    text::print_string(gm, 0x8ede);
    let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
    let mut t = gm.gi8(p);
    if (gm.g8(g::COMBAT_FLAGS) & 2) != 0 && t == -0x38 && gm.g8(g::TILES_PATCHED) != 0 {
        text::print_string(gm, 0x8ee6);
        loop {
            let mut k = input::read_command_key(gm);
            if k != 3 {
                if k == 4 || k == 0x44 {
                    t = -0x37;
                } else if k != 0x55 {
                    k = 0;
                }
            }
            if k != 0 { break; }
        }
    }
    let exit: u16;
    if t == -0x38 {
        text::print_string(gm, 0x8eec);
        exit = 5;
    } else {
        if (t != -0x7a || (gm.g8(g::COMBAT_FLAGS) & 0x80) == 0) && t != -0x37 {
            if rest::get_direction(gm) != 0 {
                let x = (gm.g8(g::PARTY_X) as u16).wrapping_add(gm.g16(g::DX));
                let y = (gm.g8(g::PARTY_Y) as u16).wrapping_add(gm.g16(g::DY));
                let q = rest::get_map_tile(gm, y, x);
                if gm.g8(q) != 0x4c || rest::find_object_at_rev(gm, 0, y, x) != 0 {
                    text::print_string(gm, 0x8efa);
                    return 0;
                }
                let ri = gm.g8(g::ACTIVE_COMBATANT) as u16;
                let ro = ri.wrapping_mul(8);
                let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(ro)) as u16;
                gm.set_g8(g::ROSTER_X.wrapping_add(ro), x as u8);
                gm.set_g8(g::MONSTER_X.wrapping_add(slot.wrapping_mul(8)), x as u8);
                gm.set_g8(g::ROSTER_Y.wrapping_add(ro), y as u8);
                gm.set_g8(g::MONSTER_Y.wrapping_add(slot.wrapping_mul(8)), y as u8);
                ovl::combat::check_room_trigger(gm, y, x);
            }
            return 1;
        }
        text::print_string(gm, 0x8ef2);
        exit = 6;
    }
    let ac = gm.g8(g::ACTIVE_COMBATANT) as i16;
    leave_combat(gm, exit, ac)
}

/// orig 1000:de24 (130 bytes). End-game absorption check for the active combatant (vector
/// 1778:06e6): if its roster entry is live (flags != 0, bit 0x20 clear), it stands on row y == 2
/// and the view object at g_view_objects+0x10+x (0xac74, tile & 0xfc == 0x3c) then
/// g_combat_exit = 'M', newline, COMSUBS e274(active) prints its name, "is absorbed!", tone,
/// g_active_member = 0xff, status window, COMBAT b4c6(-1 - active) removes it and the view is
/// refreshed. `end_combat` later runs ENDGAME a8d8 when g_combat_exit == 'M'.
pub fn combat_check_absorbed(gm: &mut Game) {
    let ac = gm.g8(g::ACTIVE_COMBATANT) as u16;
    let ro = ac.wrapping_mul(8);
    let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(ro));
    if f != 0 && (f & 0x20) == 0 && gm.g8(g::ROSTER_Y.wrapping_add(ro)) == 2 {
        let rx = gm.g8(g::ROSTER_X.wrapping_add(ro)) as u16;
        // TODO(verify): 0xac74 = g_view_objects (0xac64) + 0x10, i.e. object row 1 although the combatant is on row 2.
        if (gm.g8(rx.wrapping_add(0xac74)) & 0xfc) == 0x3c {
            gm.set_g8(g::COMBAT_EXIT, 0x4d); // 'M'
            text::put_char(gm, 10);
            ovl::comsubs::print_combatant_name(gm, ac as i16);
            text::print_string(gm, 0x8f02); // "is absorbed!"
            rest::play_tone_sweep(gm, 0x28, 1, 2000, 0x4b0);
            gm.set_g8(g::ACTIVE_MEMBER, 0xff);
            rest::draw_status_window(gm);
            ovl::combat::remove_combatant(gm, (-1i16).wrapping_sub(ac as i16));
            rest::refresh_map_view(gm);
        }
    }
}

/// orig 1000:dea6 (81 bytes). Prints a refusal: `msg` then, by `reason`, 1: 0x8f12, 2: " Not
/// here", 3: " Funny, no response!" (other reasons nothing), newline and two beeps; returns 1.
/// Ghidra order (reason, msg). Vector 1778:064a.
pub fn print_refusal(gm: &mut Game, param_1: i16, param_2: u16) -> u16 {
    let (reason, msg) = (param_1, param_2);
    text::print_string(gm, msg);
    let s: u16 = if reason == 1 { 0x8f12 } else if reason == 2 { 0x8f1a } else if reason == 3 { 0x8f24 } else { 0 };
    if s != 0 {
        text::print_string(gm, s);
    }
    text::put_char(gm, 10);
    util::beep(gm, 0x96, 0xdc);
    util::beep(gm, 0x96, 0x96);
    1
}

/// orig 1000:defa (151 bytes). "Set active plr:" for party member `member` during combat.
/// Finds the roster entry that is a party member (flags bit 7), on the party side and whose
/// member index (+3) == member; valid when its flags have none of 0x2c (sleeping/fled/dead).
/// Valid: g_active_member = member, COMSUBS e274(entry) prints the name, newline, status window;
/// otherwise "Invalid!". Returns 1 when set. Vector 1778:05c6.
pub fn set_active_player(gm: &mut Game, param_1: u16) -> i16 {
    let member = param_1;
    let mut ok: i16 = 0;
    text::print_string(gm, 0x8f3a); // "Set active plr:"
    let mut i: i16 = 0;
    loop {
        let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(i)));
        let m = gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(i))) as u16;
        if (f & 0x80) != 0 && !rest::combatant_side(gm, i) && m == member {
            ok = if (f & 0x2c) == 0 { 1 } else { 0 };
            break;
        }
        i += 1;
        if 0x1f < i { break; }
    }
    // LAB_1000_df4d
    if ok == 0 {
        text::print_string(gm, 0x8f4c); // "Invalid!"
    } else {
        gm.set_g8(g::ACTIVE_MEMBER, member as u8);
        ovl::comsubs::print_combatant_name(gm, i);
        text::put_char(gm, 10);
        rest::draw_status_window(gm);
    }
    ok
}

/// orig 1000:df92 (43 bytes). Per-turn combat effects for the active combatant (vector
/// 1778:0596): apply_ring_effects(g_active_combatant), COMSUBS e236, and the protection spell
/// countdown (g_protection_turns, 0xff = permanent): when it reaches 0 the status icon is
/// cleared and the status window marked dirty.
pub fn combat_turn_effects(gm: &mut Game) {
    let ac = gm.g8(g::ACTIVE_COMBATANT) as i16;
    rest::apply_ring_effects(gm, ac);
    ovl::comsubs::dissipate_fields(gm);
    let p = gm.gi8(g::PROTECTION_TURNS);
    if p != 0 && p != -1 {
        let np = p.wrapping_sub(1);
        gm.set_gi8(g::PROTECTION_TURNS, np);
        if np == 0 {
            gm.set_g8(g::STATUS_ICON, 0);
            gm.set_g8(g::STATUS_DIRTY, 1);
        }
    }
}

/// orig 1000:dfbe (153 bytes). Combat aftermath (ROOT fight_monster; vector 1778:0896). If
/// g_combat_exit == 'M' the ENDGAME sequence (a8d8) runs. The saved monster table (0xa9fc, 256
/// bytes) is copied back over g_monster_table. Outside dungeon-room fights (g_combat_flags &
/// 0x82 == 0) the attacked slot (< 0x20) is handled: normally its tile, anim tile, x, y and z are
/// zeroed (monster destroyed); but when g_no_more_enemies is set and the slot holds a pirate ship
/// (tile & 0xfc == 0x2c) the ship is captured: tile/anim tile -= 8 (becomes a party frigate),
/// value1 (hull) = 99, anim state (+6) = 2. Then a newline and g_location = g_location_backup.
pub fn end_combat(gm: &mut Game, param_1: i16) {
    let slot = param_1;
    if gm.g8(g::COMBAT_EXIT) == 0x4d {
        ovl::endgame::endgame_sequence(gm);
    }
    let dst = gm.ds(g::MONSTER_TABLE);
    let src = gm.ds(g::MONSTER_TABLE_BACKUP_A9FC);
    gm.m.copy(dst, src, 0x100);
    if (gm.g8(g::COMBAT_FLAGS) & 0x82) == 0 {
        if slot < 0x20 {
            let o = slot8(slot);
            let tile = gm.g8(g::MONSTER_TABLE.wrapping_add(o));
            if gm.g8(g::NO_MORE_ENEMIES) == 0 || (tile & 0xfc) != 0x2c {
                gm.set_g8(g::MONSTER_Z.wrapping_add(o), 0);
                gm.set_g8(g::MONSTER_Y.wrapping_add(o), 0);
                gm.set_g8(g::MONSTER_X.wrapping_add(o), 0);
                gm.set_g8(g::MONSTER_TABLE.wrapping_add(o).wrapping_add(1), 0);
                gm.set_g8(g::MONSTER_TABLE.wrapping_add(o), 0);
            } else {
                gm.set_g8(g::MONSTER_TABLE.wrapping_add(o), tile.wrapping_sub(8));
                let a = gm.g8(g::MONSTER_TABLE.wrapping_add(o).wrapping_add(1)).wrapping_sub(8);
                gm.set_g8(g::MONSTER_TABLE.wrapping_add(o).wrapping_add(1), a);
                gm.set_g8(g::MONSTER_Z.wrapping_add(o).wrapping_add(1), 99); // value1 = hull
                gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(o), 2);
            }
        }
        text::put_char(gm, 10);
        let lb = gm.g8(g::LOCATION_BACKUP);
        gm.set_g8(g::LOCATION, lb);
    }
}

/// orig 1000:e058 (110 bytes). Returns 1 when combat cell (y,x) is blocked for roster entry
/// `ri` (Ghidra order (ri, y, x)): outside the 11x11 map a cell counts as open (0) only for a
/// combatant whose flags have bit 1 set (can leave), otherwise blocked; inside the map the
/// result is 1 - COMBAT a290(y, x, tile of the entry's monster slot). Vector 1778:0722.
pub fn is_cell_blocked_for(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16) -> i16 {
    let (ri, y, x) = (param_1, param_2, param_3);
    let mut r: i16 = 1;
    if x < 0 || 10 < x || y < 0 || 10 < y {
        if (gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(ri))) & 2) != 0 {
            r = 0;
        }
    } else {
        let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(ros8(ri))) as u16;
        let tile = gm.g8(g::MONSTER_TABLE.wrapping_add(slot.wrapping_mul(8))) as u16;
        r = ovl::combat::combat_cell_free(gm, y as u16, x as u16, tile) as i16;
        r = 1 - r;
    }
    r
}

/// orig 1000:e0c8 (131 bytes). True when all four orthogonal neighbours of roster entry `ri`
/// (south, east, west, north in that order) are blocked per `is_cell_blocked_for`. Vector
/// 1778:072e.
pub fn is_combatant_surrounded(gm: &mut Game, param_1: i16) -> bool {
    let ri = param_1;
    let mut n: i16 = 0;
    let mut x = gm.g8(g::ROSTER_X.wrapping_add(ros8(ri))) as u16;
    let mut y = gm.g8(g::ROSTER_Y.wrapping_add(ros8(ri))) as u16;
    let mut k: i16 = 0;
    loop {
        if k == 0 {
            y = y.wrapping_add(1);
        } else if k == 1 || k == 3 {
            y = y.wrapping_sub(1);
            x = x.wrapping_add(1);
        } else if k == 2 {
            x = x.wrapping_sub(2);
        }
        if is_cell_blocked_for(gm, ri, y as i16, x as i16) != 0 {
            n += 1;
        }
        k += 1;
        if 3 < k {
            return n == 4;
        }
    }
}

/// orig 1000:e14e (134 bytes). Chaos-sword check over roster entries 0..5: the first party
/// member (flags bit 7) with flag bit 0 set has the bit cleared, "<name> passes out!" is printed
/// with a noise sweep, item 0x23 (Chaos sword, inventory 35) is unequipped from that member and
/// the entry is put to sleep (incapacitate_combatant). Returns the entry index, or -1 when no
/// entry qualified. Vector 1778:06fe.
pub fn chaos_sword_passout(gm: &mut Game) -> i16 {
    let mut i: i16 = 0;
    loop {
        let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(i)));
        if (f & 1) != 0 && (f & 0x80) != 0 { break; }
        i += 1;
        if 5 < i {
            // LAB_1000_e1ad
            if i == 6 { i = -1; }
            return i;
        }
    }
    let fa = g::ROSTER_FLAGS.wrapping_add(ros8(i));
    let f = gm.g8(fa) & 0xfe;
    gm.set_g8(fa, f);
    let member = gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(i))) as u16;
    text::print_string(gm, g::PARTY.wrapping_add(member.wrapping_mul(0x20))); // member name
    text::print_string(gm, 0x8f56); // "passes out!"
    util::play_sweep_noise(gm, 2, 1000, 0x7530, 1, 0xc1c);
    rest::remove_equipped_item(gm, 0x23, member as i16);
    rest::incapacitate_combatant(gm, i as u16);
    if i == 6 { i = -1; }
    i
}
