//! CAST.OVL (translated)
//!
//! Level-3 overlay implementing the Cast command (`cast_spell_command`, vector 1778:06da, from
//! ROOT `dispatch_command` 'C' and COMBAT a8ce) and the Use item command (`use_item_command`,
//! vector 1778:06c2, from `dispatch_command` 'U' and the combat Use). The caster is chosen with
//! ROOT `select_target_member`, the spell by name through CAST2 e2be (spell index 0..0x2f in
//! the DATA.OVL spell order In Lor .. An Tym; circle = index/6 + 1), then the per-context
//! permission byte g_spell_unk_1c90[spell] (bit 8 overworld, 4 town, 2 dungeon, 1 combat), the
//! mixture count g_mixed_spells[spell], the caster's MP (record +0xf) and level (record +0x16)
//! are checked and the effect routine is run (CAST2 e1e0(circle) plays the casting flash per
//! circle; e27e picks a member, e4e6 asks a direction). The wind/cone spells (In Zu, In Nox
//! Hur, In Flam Hur, In Vas Grav Corp) animate 21 particles with `animate_wind_cone` and hit
//! every combatant whose cell the particles reached. Use item covers scrolls, potions, the
//! magic carpet, skull keys, the amulet/crown/sceptre, moonstones, the gem shards (Flames of
//! Truth/Love/Courage), spyglass, HMS Cape plans, sextant, pocket watch and black badge.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// Combat-roster entry offset (8 bytes per entry at 0xba14).
#[inline] fn ros8(i: i16) -> u16 { (i as u16).wrapping_mul(8) }
/// Monster-table slot offset (8 bytes per slot at 0x5c5a).
#[inline] fn slot8(slot: u16) -> u16 { slot.wrapping_mul(8) }
/// Party record offset (32 bytes per member at 0x55a8).
#[inline] fn mem32(m: u16) -> u16 { m.wrapping_mul(0x20) }

/// DGROUP 0x5890 (save 0x2EA): "the current attack is a spell" flag (second byte of
/// g::SAVE_UNK_2E9), set to 1 by the Cast command.
const ATTACK_IS_SPELL: u16 = 0x5890;
/// Party record +0x0f: magic points.
const PARTY_MP: u16 = 0x55b7;
/// Party record +0x14: experience (i16).
const PARTY_EXP: u16 = 0x55bc;
/// Party record +0x16: level.
const PARTY_LEVEL: u16 = 0x55be;
/// Dungeon facing deltas (words): dx at 0x24d6 + facing*2, dy at 0x24de + facing*2.
const DUNGEON_DX: u16 = 0x24d6;
const DUNGEON_DY: u16 = 0x24de;
/// Field-spell tables: weapon ids 0x35,0x33,0x34,0x36 (combat) at 0x4592 and dungeon cell
/// codes 0x82,0x81,0x80,0x83 at 0x4596, followed by "POOF!\n" at 0x459a.
const FIELD_WEAPON: u16 = 0x4592;
const FIELD_DUNGEON_CELL: u16 = 0x4596;
/// BSS scratch copy of g_unk_1cf0 (21 words) used as the particle energies of a wind spell.
const WIND_ENERGY: u16 = 0xa9d0;

/// orig 1000:bf80 (50 bytes). Returns 1 when the combat-roster entry's +3 byte (party index
/// for members, type for monsters) is 0x2f, 0x0e or 0x0f: such combatants are immune to the
/// sleep/charm/fear/shrink spells and the sleep/kill winds; otherwise 0.
pub fn is_spell_immune_combatant(gm: &mut Game, param_1: i16) -> u16 {
    let t = gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(param_1)));
    if t == 0x2f || t == 0x0e || t == 0x0f { 1 } else { 0 }
}

/// orig 1000:bfb2 (25 bytes). Casts a missile spell in combat: g_attack_weapon = `weapon`
/// (0x30 Grav Por, 0x31 Vas Flam, 0x32 Xen Corp) and COMSUBS `attack_with(weapon,
/// g_active_combatant)` does the aiming, missile and damage.
pub fn cast_missile_spell(gm: &mut Game, param_1: u16) {
    gm.set_g8(g::ATTACK_WEAPON, param_1 as u8);
    let ri = gm.g8(g::ACTIVE_COMBATANT) as i16;
    ovl::comsubs::attack_with(gm, param_1 as i16, ri);
}

/// orig 1000:bfcc (197 bytes). Field spells In Flam Grav (0), In Nox Grav (1), In Zu Grav (2),
/// In Sanct Grav (3). Outside combat (dungeons only reach here): casting flash of circle 4 for
/// kind 3 else 3, then the dungeon cell ahead of the party (facing deltas 0x24d6/0x24de, masked
/// & 7) must be empty apart from bit 3 ((cell & 0xf7) == 0, else returns 0) and becomes
/// (cell & 8) | g_field_dungeon_cell[kind] (0x82/0x81/0x80/0x83). In combat the field is thrown
/// as weapon g_field_weapon[kind] (0x35/0x33/0x34/0x36) through COMSUBS `attack_with`. Returns
/// 0xffff (no Success/Failed message) in both cases.
pub fn cast_field_spell(gm: &mut Game, param_1: i16) -> u16 {
    if gm.g8(g::LOCATION) < 0x80 {
        let circle: i16 = if param_1 == 3 { 4 } else { 3 };
        ovl::cast2::casting_flash(gm, circle);
        let facing = (gm.g8(g::DUNGEON_FACING) as u16).wrapping_mul(2);
        let x = (gm.gi16(DUNGEON_DX.wrapping_add(facing)).wrapping_add(gm.g8(g::PARTY_X) as i16) as u16) & 7;
        let y = (gm.gi16(DUNGEON_DY.wrapping_add(facing)).wrapping_add(gm.g8(g::PARTY_Y) as i16) as u16) & 7;
        let cell = g::DUNGEON_MAP
            .wrapping_add((gm.g8(g::PARTY_Z) as u16).wrapping_mul(0x40))
            .wrapping_add(y.wrapping_mul(8))
            .wrapping_add(x);
        let b = gm.g8(cell);
        if (b & 0xf7) != 0 {
            return 0;
        }
        let code = gm.g8(FIELD_DUNGEON_CELL.wrapping_add(param_1 as u16));
        gm.set_g8(cell, (b & 8) | code);
    } else {
        let w = gm.g8(FIELD_WEAPON.wrapping_add(param_1 as u16));
        gm.set_g8(g::ATTACK_WEAPON, w);
        let ri = gm.g8(g::ACTIVE_COMBATANT) as i16;
        ovl::comsubs::attack_with(gm, w as i16, ri);
    }
    0xffff
}

/// orig 1000:c094 (151 bytes). An Zu (awaken). CAST2 e27e picks the member (negative = cancel,
/// returns 0xffff). A sleeping member ('S') becomes 'G'; in combat the roster entry with flags
/// (& 0xe8) == 0x88 and that member index is woken with `wake_combatant`; casting flash circle
/// 1, g_status_dirty = 1, returns 0xffff (no message). A member who is not asleep returns 0
/// ("Failed!").
pub fn cast_an_zu_awaken(gm: &mut Game) -> u16 {
    let m = ovl::cast2::pick_target_member(gm);
    if m < 0 {
        return 0xffff;
    }
    let st = g::PARTY_STATUS.wrapping_add(mem32(m as u16));
    if gm.g8(st) == b'S' {
        gm.set_g8(st, 0x47); // 'G'
        if 0x7f < gm.g8(g::LOCATION) {
            let mut i: i16 = 0;
            loop {
                let flags = gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(i)));
                let member = gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(i)));
                if (flags & 0xe8) == 0x88 && member as u16 == m as u16 {
                    rest::wake_combatant(gm, i);
                    break;
                }
                i += 1;
                if !(i < 0x20) { break; }
            }
        }
        ovl::cast2::casting_flash(gm, 1);
        gm.set_g8(g::STATUS_DIRTY, 1);
        0xffff
    } else {
        0
    }
}

/// orig 1000:c12e (75 bytes). An Nox (cure poison). CAST2 e27e picks the member (negative =
/// cancel, 0xffff). A poisoned member ('P') becomes 'G', casting flash circle 1, g_status_dirty
/// = 1, returns 1 ("Success!"); otherwise 0 ("Failed!").
pub fn cast_an_nox_cure(gm: &mut Game) -> u16 {
    let m = ovl::cast2::pick_target_member(gm);
    if m < 0 {
        return 0xffff;
    }
    let st = g::PARTY_STATUS.wrapping_add(mem32(m as u16));
    if gm.g8(st) == b'P' {
        gm.set_g8(st, b'G');
        ovl::cast2::casting_flash(gm, 1);
        gm.set_g8(g::STATUS_DIRTY, 1);
        1
    } else {
        0
    }
}

/// orig 1000:c17a (52 bytes). Mani (heal). CAST2 e27e picks the member (negative = cancel,
/// -1); CAST2 e5a2(member) heals and returns the result; when it is non-zero the casting flash
/// of circle 1 is shown. Returns the e5a2 result (1 Success, 0 Failed) or -1.
pub fn cast_mani_heal(gm: &mut Game) -> i16 {
    let m = ovl::cast2::pick_target_member(gm);
    let result: i16 = if m < 0 {
        -1
    } else {
        ovl::cast2::heal_member(gm, m) as i16
    };
    if result != 0 {
        ovl::cast2::casting_flash(gm, 1);
    }
    result
}

/// orig 1000:c1b0 (159 bytes). An Ylem (dissolve matter). CAST2 e4e6 asks the direction (0 =
/// cancel, returns 0xffff); casting flash circle 1; the map tile at (g_dy, g_dx) is replaced by
/// 0x44 when it is 0x9d, 0x5b, 0x90..0x93, 0xa5..0xa6, 0xa8..0xa9 or 0xad..0xaf: "POOF!" (0x459a),
/// g_view_dirty |= 2, refresh_map_view, play_tone_sweep(0x28, 1, 2000, 0x4b0), returns 0xffff.
/// Any other tile returns 0 ("Failed!").
pub fn cast_an_ylem_dissolve(gm: &mut Game) -> u16 {
    if ovl::cast2::ask_direction(gm) == 0 {
        return 0xffff;
    }
    ovl::cast2::casting_flash(gm, 1);
    let p = rest::get_map_tile(gm, gm.g16(g::DY), gm.g16(g::DX));
    let b = gm.g8(p);
    let hit = if b == 0x9d {
        true
    } else if b < 0x9e {
        b == 0x5b || (0x8f < b && b < 0x94)
    } else {
        0xa4 < b && (b < 0xa7 || (0xa7 < b && (b < 0xaa || (0xac < b && b < 0xb0))))
    };
    if hit {
        gm.set_g8(p, 0x44);
        text::print_string(gm, 0x459a); // "POOF!\n"
        let vd = gm.g8(g::VIEW_DIRTY);
        gm.set_g8(g::VIEW_DIRTY, vd | 2);
        rest::refresh_map_view(gm);
        rest::play_tone_sweep(gm, 0x28, 1, 2000, 0x4b0);
        0xffff
    } else {
        0
    }
}

/// orig 1000:c252 (360 bytes). An Sanct (unlock). Outside dungeons (towns and combat): CAST2
/// e4e6 asks the direction (0 = cancel, 0xffff); a locked door tile 0xb9/0xbb at (g_dy, g_dx)
/// becomes the unlocked door one less, g_view_dirty |= 2, flash circle 2, returns 1. Otherwise
/// the monster/object table is searched for a chest (tile byte 1) at (g_dx, g_dy) (and on the
/// party's z unless in combat): its +5 byte loses bit 7 (trap/lock flag), flash circle 2,
/// returns 1; nothing found returns 0. In a dungeon: flash circle 2, the cell under the party
/// (or, when that is not a chest, the cell ahead) with (cell & 0xf0) == 0x40 (chest): bit 0 set
/// prints "Disarmed!" (0x45a1), the cell becomes (cell & 8) | 0x70, "Chest opened!" (0x45ac),
/// returns 0xffff; no chest returns 0.
pub fn cast_an_sanct_unlock(gm: &mut Game) -> u16 {
    let loc = gm.g8(g::LOCATION);
    let mut result: u16;
    if loc < 0x21 || 0x7f < loc {
        if ovl::cast2::ask_direction(gm) == 0 {
            result = 0xffff;
        } else {
            result = 0;
            let p = rest::get_map_tile(gm, gm.g16(g::DY), gm.g16(g::DX));
            let t = gm.gi8(p);
            if t == -0x47 || t == -0x45 {
                gm.set_gi8(p, t.wrapping_sub(1));
                let vd = gm.g8(g::VIEW_DIRTY);
                gm.set_g8(g::VIEW_DIRTY, vd | 2);
                ovl::cast2::casting_flash(gm, 2);
                result = 1;
            } else {
                let mut i: i16 = 0;
                loop {
                    let e = g::MONSTER_TABLE.wrapping_add(ros8(i));
                    // (the asm compares z as plain bytes: CMP byte ptr [SI+4],AL)
                    if gm.gi8(e) == 1
                        && gm.g8(e.wrapping_add(2)) as u16 == gm.g16(g::DX)
                        && gm.g8(e.wrapping_add(3)) as u16 == gm.g16(g::DY)
                        && (0x7f < loc || gm.g8(e.wrapping_add(4)) == gm.g8(g::PARTY_Z))
                    {
                        let v = gm.g8(e.wrapping_add(5));
                        gm.set_g8(e.wrapping_add(5), v & 0x7f);
                        ovl::cast2::casting_flash(gm, 2);
                        return 1;
                    }
                    i += 1;
                    if !(i < 0x20) { break; }
                }
            }
        }
    } else {
        ovl::cast2::casting_flash(gm, 2);
        let z = gm.g8(g::PARTY_Z) as u16;
        let mut cell = g::DUNGEON_MAP
            .wrapping_add(z.wrapping_mul(0x40))
            .wrapping_add((gm.g8(g::PARTY_Y) as u16).wrapping_mul(8))
            .wrapping_add(gm.g8(g::PARTY_X) as u16);
        if (gm.g8(cell) & 0xf0) != 0x40 {
            let facing = (gm.g8(g::DUNGEON_FACING) as u16).wrapping_mul(2);
            let y = (gm.gi16(DUNGEON_DY.wrapping_add(facing)).wrapping_add(gm.g8(g::PARTY_Y) as i16) as u16) & 7;
            let x = (gm.gi16(DUNGEON_DX.wrapping_add(facing)).wrapping_add(gm.g8(g::PARTY_X) as i16) as u16) & 7;
            cell = g::DUNGEON_MAP
                .wrapping_add(y.wrapping_mul(8))
                .wrapping_add(x)
                .wrapping_add(z.wrapping_mul(0x40));
        }
        if (gm.g8(cell) & 0xf0) == 0x40 {
            if (gm.g8(cell) & 1) != 0 {
                text::print_string(gm, 0x45a1); // "Disarmed!\n"
            }
            let b = gm.g8(cell);
            gm.set_g8(cell, (b & 8) | 0x70);
            text::print_string(gm, 0x45ac); // "Chest opened!\n"
            result = 0xffff;
        } else {
            result = 0;
        }
    }
    result
}

/// orig 1000:c3be (102 bytes). An Xen Corp (repel undead). Casting flash circle 2, then every
/// roster entry that is not spell-immune, has (flags & 0xc0) == 0x40 (a monster) and whose type
/// (+3) has g_monster_flags bit 0x20 (undead) and fails COMSUBS `target_evades(0, i,
/// g_active_combatant)` gets byte +0 = 1 and flags |= 2 (fleeing).
pub fn cast_an_xen_corp_repel_undead(gm: &mut Game) {
    ovl::cast2::casting_flash(gm, 2);
    let mut i: i16 = 0;
    loop {
        let e = g::COMBAT_ROSTER.wrapping_add(ros8(i));
        if is_spell_immune_combatant(gm, i) == 0
            && (gm.g8(e.wrapping_add(2)) & 0xc0) == 0x40
            && (gm.g8(g::MONSTER_FLAGS.wrapping_add((gm.g8(e.wrapping_add(3)) as u16).wrapping_mul(2))) & 0x20) != 0
        {
            let active = gm.g8(g::ACTIVE_COMBATANT) as u16;
            if ovl::comsubs::target_evades(gm, 0, i as u16, active) == 0 {
                gm.set_g8(e, 1);
                let f = gm.g8(e.wrapping_add(2));
                gm.set_g8(e.wrapping_add(2), f | 2);
            }
        }
        i += 1;
        if !(i < 0x20) { break; }
    }
}

/// orig 1000:c424 (11 bytes). In Wis (locate): casting flash circle 2, then CAST2 e8cc prints
/// the party's position.
pub fn cast_in_wis_locate(gm: &mut Game) {
    ovl::cast2::casting_flash(gm, 2);
    ovl::cast2::in_wis_print_position(gm);
}

/// orig 1000:c430 (255 bytes). Kal Xen (summon animal) in combat. Casting flash circle 2; a
/// random type: rand 0..15 < 6 -> 0x14, < 11 -> 0x16, < 14 -> 0x15, else 0x22. Up to 8 attempts
/// to find a random free combat cell (COMBAT random_combat_cell + combat_cell_free(y, x, 0x90)),
/// else returns 0. `add_combatant(g_party_z, g_dy, g_dx, 0, type)` (negative = 0); the new
/// slot's tile and anim tile are set to 0x16 while `tile_transition_effect(g_dy, g_dx,
/// type*4+0x140)` plays, then to (type*4+0x40) as a byte; the roster flags get bit 0 (allied).
/// Returns 1.
pub fn cast_kal_xen_summon_animal(gm: &mut Game) -> u16 {
    ovl::cast2::casting_flash(gm, 2);
    let r = util::rand_range(gm, 0xf, 0);
    let mtype: i16 = if r < 6 { 0x14 } else if r < 0xb { 0x16 } else if r < 0xe { 0x15 } else { 0x22 };
    let mut tries: i16 = 0;
    let tile = ((mtype as i8).wrapping_mul(4)).wrapping_add(0x40);
    loop {
        if ovl::combat::random_combat_cell(gm) != 0 {
            let (dy, dx) = (gm.g16(g::DY), gm.g16(g::DX));
            if ovl::combat::combat_cell_free(gm, dy, dx, 0x90) != 0 {
                break;
            }
        }
        tries += 1;
        if 7 < tries {
            return 0;
        }
    }
    let (z, dy, dx) = (gm.g8(g::PARTY_Z), gm.g16(g::DY) as u8, gm.g16(g::DX) as u8);
    let ri = rest::add_combatant(gm, z, dy, dx, 0, mtype) as i16;
    if ri < 0 {
        return 0;
    }
    let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(ros8(ri))) as u16;
    let so = slot8(slot);
    gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), 0x16);
    gm.set_g8(g::MONSTER_TABLE.wrapping_add(so).wrapping_add(1), 0x16);
    let (dy, dx) = (gm.g16(g::DY) as i16, gm.g16(g::DX) as i16);
    gfx::tile_transition_effect(gm, dy, dx, mtype.wrapping_mul(4).wrapping_add(0x140));
    gm.set_gi8(g::MONSTER_TABLE.wrapping_add(so), tile);
    gm.set_gi8(g::MONSTER_TABLE.wrapping_add(so).wrapping_add(1), tile);
    let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(ri)));
    gm.set_g8(g::ROSTER_FLAGS.wrapping_add(ros8(ri)), f | 1);
    1
}

/// orig 1000:c534 (39 bytes). In Xen Mani (create food): casting flash circle 2, g_food +=
/// rand 1..3 capped at 9999 (`add_i16_capped`), g_status_dirty = 1, returns 1.
pub fn cast_in_xen_mani_create_food(gm: &mut Game) -> u16 {
    ovl::cast2::casting_flash(gm, 2);
    let n = util::rand_range(gm, 3, 1);
    rest::add_i16_capped(gm, 0x270f, n, g::FOOD);
    gm.set_g8(g::STATUS_DIRTY, 1);
    1
}

/// orig 1000:c55c (365 bytes). In Por (blink). Outside combat: CAST2 e4e6 asks the direction
/// (0 = cancel, -1); casting flash circle 3; starting at (g_dx, g_dy) the map is walked in steps
/// of (g_dx - g_party_x, g_dy - g_party_y) while inside the loaded window [g_chunk_x,
/// min(g_chunk_x+0x20, 0x100)) x [g_chunk_y, ...): every grass tile (5) met moves the party there
/// (g_party_x/y, g_view_dirty = 1, result -1) so the party lands on the last grass cell of the
/// line; on success MAINOUT `init_overworld` reloads the view. In combat (unless g_combat_flags
/// bit 1): casting flash circle 3, up to 7 random free cells are tried for the caster's own
/// tile (COMBAT random_combat_cell + combat_cell_free(y, x, tile)); the first one moves the
/// caster (roster x/y and monster-table x/y) and returns 1; otherwise 0.
pub fn cast_in_por_blink(gm: &mut Game) -> i16 {
    let mut result: i16 = 0;
    if gm.g8(g::LOCATION) < 0x80 {
        if ovl::cast2::ask_direction(gm) == 0 {
            result = -1;
        } else {
            ovl::cast2::casting_flash(gm, 3);
            let ddx = (gm.g16(g::DX) as i16).wrapping_sub(gm.g8(g::PARTY_X) as i16);
            let ddy = (gm.g16(g::DY) as i16).wrapping_sub(gm.g8(g::PARTY_Y) as i16);
            let mut xmax: u16 = (gm.g8(g::CHUNK_X) as u16).wrapping_add(0x20);
            if 0x100 < xmax {
                xmax = 0x100;
            }
            let mut ymax: u16 = (gm.g8(g::CHUNK_Y) as u16).wrapping_add(0x20);
            if 0x100 < ymax {
                ymax = 0x100;
            }
            result = 0;
            let mut x: u16 = gm.g16(g::DX) & 0xff;
            let mut y: u16 = gm.g16(g::DY) & 0xff;
            while gm.g8(g::CHUNK_X) as u16 <= x
                && (x as i16) < (xmax as i16)
                && gm.g8(g::CHUNK_Y) as u16 <= y
                && (y as i16) < (ymax as i16)
            {
                let p = rest::get_map_tile(gm, y, x);
                if gm.gi8(p) == 5 {
                    result = -1;
                    gm.set_g8(g::PARTY_X, x as u8);
                    gm.set_g8(g::PARTY_Y, y as u8);
                    gm.set_g8(g::VIEW_DIRTY, 1);
                }
                x = x.wrapping_add(ddx as u16);
                y = y.wrapping_add(ddy as u16);
            }
            if result != 0 {
                ovl::mainout::init_overworld(gm);
            }
        }
    } else {
        let active = gm.g8(g::ACTIVE_COMBATANT) as u16;
        let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(slot8(active))) as u16;
        if (gm.g8(g::COMBAT_FLAGS) & 2) == 0 {
            ovl::cast2::casting_flash(gm, 3);
            let mut tries: i16 = 0;
            let tile = gm.g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)));
            loop {
                if ovl::combat::random_combat_cell(gm) != 0 {
                    let (dy, dx) = (gm.g16(g::DY), gm.g16(g::DX));
                    if ovl::combat::combat_cell_free(gm, dy, dx, tile as u16) != 0 {
                        let (dx8, dy8) = (gm.g16(g::DX) as u8, gm.g16(g::DY) as u8);
                        gm.set_g8(g::ROSTER_X.wrapping_add(slot8(active)), dx8);
                        gm.set_g8(g::MONSTER_X.wrapping_add(slot8(slot)), dx8);
                        gm.set_g8(g::ROSTER_Y.wrapping_add(slot8(active)), dy8);
                        gm.set_g8(g::MONSTER_X.wrapping_add(slot8(slot)).wrapping_add(1), dy8);
                        return 1;
                    }
                }
                tries += 1;
                if !(tries < 7) { break; }
            }
        } else {
            result = 0;
        }
    }
    result
}

/// orig 1000:c6cc (103 bytes). Wis Quas (reveal invisible): casting flash circle 4, then every
/// roster entry with non-zero flags that is not a party member (bit 7 clear) and is invisible
/// (bit 4) loses bit 4 and its monster-table anim tile (+1) is restored from the tile (+0);
/// `place_objects_in_view` and `draw_map_view` redraw after each one.
pub fn cast_wis_quas_reveal(gm: &mut Game) {
    ovl::cast2::casting_flash(gm, 4);
    let mut i: i16 = 0;
    loop {
        let fa = g::ROSTER_FLAGS.wrapping_add(ros8(i));
        let f = gm.g8(fa);
        if f != 0 && (f & 0x80) == 0 && (f & 0x10) != 0 {
            gm.set_g8(fa, f & 0xef);
            let so = slot8(gm.g8(g::ROSTER_SLOT.wrapping_add(ros8(i))) as u16);
            let t = gm.g8(g::MONSTER_TABLE.wrapping_add(so));
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(so).wrapping_add(1), t);
            rest::place_objects_in_view(gm);
            rest::draw_map_view(gm);
        }
        i += 1;
        if !(i < 0x20) { break; }
    }
}

/// orig 1000:c734 (144 bytes). In Bet Xen (insect swarm): casting flash circle 5; up to 8
/// attempts at a random free combat cell (combat_cell_free(y, x, 0xbc)); when one is found up
/// to four type-0x1f combatants are added there (`add_combatant(g_party_z, g_dy, g_dx, 0,
/// 0x1f)`, roster flags |= 1 = allied), stopping at the first failure. Returns 1 when at least
/// one was added, else 0.
pub fn cast_in_bet_xen_insect_swarm(gm: &mut Game) -> i16 {
    ovl::cast2::casting_flash(gm, 5);
    let mut found: i16 = 0;
    let mut tries: i16 = 0;
    loop {
        if ovl::combat::random_combat_cell(gm) != 0 {
            let (dy, dx) = (gm.g16(g::DY), gm.g16(g::DX));
            if ovl::combat::combat_cell_free(gm, dy, dx, 0xbc) != 0 {
                found = 1;
                break;
            }
        }
        tries += 1;
        if 7 < tries {
            break;
        }
    }
    // LAB_1000_c76d
    if found != 0 {
        found = 0;
        let mut k: i16 = 0;
        loop {
            let (z, dy, dx) = (gm.g8(g::PARTY_Z), gm.g16(g::DY) as u8, gm.g16(g::DX) as u8);
            let ri = rest::add_combatant(gm, z, dy, dx, 0, 0x1f) as i16;
            if ri < 0 {
                return found;
            }
            let fa = g::ROSTER_FLAGS.wrapping_add(ros8(ri));
            let f = gm.g8(fa);
            gm.set_g8(fa, f | 1);
            found = 1;
            k += 1;
            if !(k < 4) { break; }
        }
    }
    found
}

/// orig 1000:c7c6 (102 bytes). An Ex Por (magic lock). CAST2 e4e6 asks the direction (0 ->
/// 0xffff); casting flash circle 5; the tile at (g_dy, g_dx): 0xb8..0xb9 (doors) becomes 0x97
/// (magically locked door), 0xba..0xbb becomes 0x98 (magically locked window door); then
/// g_view_dirty |= 2 and 1 is returned. Any other tile returns 0.
pub fn cast_an_ex_por_magic_lock(gm: &mut Game) -> u16 {
    if ovl::cast2::ask_direction(gm) == 0 {
        return 0xffff;
    }
    ovl::cast2::casting_flash(gm, 5);
    let p = rest::get_map_tile(gm, gm.g16(g::DY), gm.g16(g::DX));
    let mut result: u16 = 0;
    let b = gm.g8(p);
    if 0xb7 < b {
        if b < 0xba {
            gm.set_g8(p, 0x97);
        } else {
            if b < 0xba {
                return 0;
            }
            if 0xbb < b {
                return 0;
            }
            gm.set_g8(p, 0x98);
        }
        result = 1;
        let vd = gm.g8(g::VIEW_DIRTY);
        gm.set_g8(g::VIEW_DIRTY, vd | 2);
    }
    result
}

/// orig 1000:c82c (113 bytes). Vas Mani (great heal). CAST2 e27e picks the member (negative ->
/// 0xffff). A dead member ('D'), or any member while in combat with enemies still present
/// (g_location > 0x7f and g_no_more_enemies == 0), returns 0 ("Failed!"). Otherwise HP (record
/// +0x10) = max HP (+0x12), casting flash circle 5, g_status_dirty = 1, returns 1.
pub fn cast_vas_mani_great_heal(gm: &mut Game) -> u16 {
    let m = ovl::cast2::pick_target_member(gm);
    if m < 0 {
        return 0xffff;
    }
    let rec = mem32(m as u16);
    let loc = gm.g8(g::LOCATION);
    if gm.g8(g::PARTY_STATUS.wrapping_add(rec)) == b'D'
        || (0x7f < loc && gm.g8(g::NO_MORE_ENEMIES) == 0)
        || (0x7f < loc && gm.g8(g::NO_MORE_ENEMIES) == 0)
    {
        0
    } else {
        let max = gm.g16(g::PARTY_MAX_HP.wrapping_add(rec));
        gm.set_g16(g::PARTY_HP.wrapping_add(rec), max);
        ovl::cast2::casting_flash(gm, 5);
        gm.set_g8(g::STATUS_DIRTY, 1);
        1
    }
}

/// orig 1000:c89e (129 bytes). In Vas Por Ylem (earthquake) in combat; `caster` is the party
/// index of the caster. Casting flash circle 6 and `earthquake_shake`; every occupied roster
/// entry on the enemy side (`combatant_side`) whose dexterity stat (COMBAT get_combat_stat(-2,
/// i)) is <= `random_damage_1_30()` is flashed and takes rand 1..20 damage (COMBAT
/// apply_damage); the experience returned by apply_damage is added to the caster's record
/// (+0x14, capped at 9999) and COMSUBS `print_hit_result(g_active_combatant, i)` reports it.
pub fn cast_in_vas_por_ylem_earthquake(gm: &mut Game, param_1: i16) {
    let caster = param_1;
    ovl::cast2::casting_flash(gm, 6);
    rest::earthquake_shake(gm);
    let mut i: i16 = 0;
    loop {
        if gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(i))) != 0 {
            if rest::combatant_side(gm, i) {
                let roll = rest::random_damage_1_30(gm);
                let stat = ovl::combat::get_combat_stat(gm, -2, i) as i16;
                if stat <= roll {
                    rest::flash_monster(gm, i as u16);
                    let exp = (caster as u16).wrapping_mul(0x20).wrapping_add(PARTY_EXP);
                    let dmg = util::rand_range(gm, 0x14, 1);
                    let xp = ovl::combat::apply_damage(gm, dmg as u16, i) as i16;
                    rest::add_i16_capped(gm, 0x270f, xp, exp);
                    let active = gm.g8(g::ACTIVE_COMBATANT) as i16;
                    ovl::comsubs::print_hit_result(gm, active, i);
                }
            }
        }
        i += 1;
        if !(i < 0x20) { break; }
    }
}

/// orig 1000:c920 (186 bytes). An Xen Ex (charm). "Creature: " (0x45bb) and COMSUBS
/// `aim_crosshair(15, g_active_combatant)` (0 = cancel -> 0xffff). Casting flash circle 6. The
/// crosshair cell must not be a tile 0x84/0x85; the combatant there (`find_combatant_at`) must
/// exist, not be spell-immune, be on the enemy side (`combatant_side`) and fail
/// `target_evades(0, t, active)`: then its roster flag bit 0 (allied) is toggled, a party member
/// (bit 7) is also set to status 'G' with a status redraw, its name is printed followed by
/// " charmed!" (0x45c6) and 0xffff is returned. Any failure returns 0.
pub fn cast_an_xen_ex_charm(gm: &mut Game) -> u16 {
    text::print_string(gm, 0x45bb); // "Creature: "
    let active = gm.g8(g::ACTIVE_COMBATANT) as u16;
    if ovl::comsubs::aim_crosshair(gm, 0xf, active) == 0 {
        return 0xffff;
    }
    ovl::cast2::casting_flash(gm, 6);
    let (cy, cx) = (gm.g8(g::CROSSHAIR_Y) as u16, gm.g8(g::CROSSHAIR_X) as u16);
    let p = rest::get_map_tile(gm, cy, cx);
    if (gm.g8(p) & 0xfe) != 0x84 {
        let t = ovl::comsubs::find_combatant_at(gm, cy, cx);
        if -1 < t {
            if is_spell_immune_combatant(gm, t) == 0 {
                if rest::combatant_side(gm, t) {
                    if ovl::comsubs::target_evades(gm, 0, t as u16, active) == 0 {
                        let fa = g::ROSTER_FLAGS.wrapping_add(ros8(t));
                        let f = gm.g8(fa) ^ 1;
                        gm.set_g8(fa, f);
                        if (f & 0x80) != 0 {
                            let m = gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(t))) as u16;
                            gm.set_g8(g::PARTY_STATUS.wrapping_add(mem32(m)), 0x47); // 'G'
                            rest::draw_status_window(gm);
                        }
                        ovl::comsubs::print_combatant_name(gm, t);
                        text::print_string(gm, 0x45c6); // " charmed!\n"
                        return 0xffff;
                    }
                }
            }
        }
    }
    0
}

/// orig 1000:c9dc (159 bytes). Rel Xen Bet (shrink into a small creature). "Creature: "
/// (0x45d1), `aim_crosshair(15, active)` (0 -> 0xffff), casting flash circle 6. The combatant
/// at the crosshair must exist, not be spell-immune and fail `target_evades(0, t, active)`: it
/// is removed (COMBAT remove_combatant(-1 - t)) and a type-0x14 creature is added on its cell
/// (`add_combatant(g_party_z, y, x, 0, 0x14)`); returns 1. Otherwise 0.
pub fn cast_rel_xen_bet_shrink(gm: &mut Game) -> u16 {
    text::print_string(gm, 0x45d1); // "Creature: "
    let active = gm.g8(g::ACTIVE_COMBATANT) as u16;
    if ovl::comsubs::aim_crosshair(gm, 0xf, active) == 0 {
        return 0xffff;
    }
    ovl::cast2::casting_flash(gm, 6);
    let (cy, cx) = (gm.g8(g::CROSSHAIR_Y) as u16, gm.g8(g::CROSSHAIR_X) as u16);
    let t = ovl::comsubs::find_combatant_at(gm, cy, cx);
    if -1 < t
        && is_spell_immune_combatant(gm, t) == 0
        && ovl::comsubs::target_evades(gm, 0, t as u16, active) == 0
    {
        let x = gm.g8(g::ROSTER_X.wrapping_add(ros8(t)));
        let y = gm.g8(g::ROSTER_Y.wrapping_add(ros8(t)));
        ovl::combat::remove_combatant(gm, (-1i16).wrapping_sub(t));
        let z = gm.g8(g::PARTY_Z);
        rest::add_combatant(gm, z, y, x, 0, 0x14);
        return 1;
    }
    0
}

/// orig 1000:ca7e (42 bytes). Sanct Lor (invisibility): the active combatant's monster-table
/// anim tile (+1) becomes 0x1d (blank) and its roster flags get bit 4 (invisible); casting
/// flash circle 7; returns 1.
pub fn cast_sanct_lor_invisibility(gm: &mut Game) -> u16 {
    let a = gm.g8(g::ACTIVE_COMBATANT) as u16;
    let so = slot8(gm.g8(g::ROSTER_SLOT.wrapping_add(slot8(a))) as u16);
    gm.set_g8(g::MONSTER_TABLE.wrapping_add(so).wrapping_add(1), 0x1d);
    let fa = g::ROSTER_FLAGS.wrapping_add(slot8(a));
    let f = gm.g8(fa);
    gm.set_g8(fa, f | 0x10);
    ovl::cast2::casting_flash(gm, 7);
    1
}

/// orig 1000:caa8 (364 bytes). In Quas Xen (clone). "Creature: " (0x45dc), `aim_crosshair(15,
/// active)` (0 -> -1), casting flash circle 7. The combatant at the crosshair must exist (else
/// 0). The first free monster-table slot (tile byte 0) and the first free roster entry (flags
/// 0) are found (0xff = none; the result is then uninitialised in the original); the whole
/// 8-byte roster entry and the whole 8-byte monster-table slot of the target are copied into
/// them (the new entry's slot byte = the new slot), then random free cells are tried
/// (random_combat_cell + combat_cell_free(y, x, 0x1c)) until one is found, which becomes the
/// clone's roster x/y and monster-table x/y; returns 1.
pub fn cast_in_quas_xen_clone(gm: &mut Game) -> i16 {
    text::print_string(gm, 0x45dc); // "Creature: "
    let active = gm.g8(g::ACTIVE_COMBATANT) as u16;
    if ovl::comsubs::aim_crosshair(gm, 0xf, active) == 0 {
        return -1;
    }
    ovl::cast2::casting_flash(gm, 7);
    let (cy, cx) = (gm.g8(g::CROSSHAIR_Y) as u16, gm.g8(g::CROSSHAIR_X) as u16);
    let t = ovl::comsubs::find_combatant_at(gm, cy, cx);
    if t < 0 {
        return 0;
    }
    // TODO(verify): when no free slot/entry exists the original returns an uninitialised
    // local; 0 ("Failed!") is used here.
    let mut result: i16 = 0;
    let mut slot: i16 = 0xff;
    let mut i: i16 = 0;
    loop {
        if gm.g8(g::MONSTER_TABLE.wrapping_add(ros8(i))) == 0 {
            slot = i;
            break;
        }
        i += 1;
        if !(i < 0x20) { break; }
    }
    if slot != 0xff {
        let mut ri: i16 = 0xff;
        let mut j: i16 = 0;
        loop {
            if gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(j))) == 0 {
                ri = j;
                break;
            }
            j += 1;
            if !(j < 0x20) { break; }
        }
        if ri != 0xff {
            let r = ros8(ri);
            let s = ros8(t);
            for off in [0u16, 2, 4, 6] {
                let w = gm.g16(g::COMBAT_ROSTER.wrapping_add(s).wrapping_add(off));
                gm.set_g16(g::COMBAT_ROSTER.wrapping_add(r).wrapping_add(off), w);
            }
            gm.set_g8(g::ROSTER_SLOT.wrapping_add(r), slot as u8);
            let src = gm.g8(g::ROSTER_SLOT.wrapping_add(s)) as u16;
            for off in [0u16, 2, 4, 6] {
                let w = gm.g16(g::MONSTER_TABLE.wrapping_add(slot8(src)).wrapping_add(off));
                gm.set_g16(g::MONSTER_TABLE.wrapping_add(slot8(slot as u16)).wrapping_add(off), w);
            }
            loop {
                let ok = ovl::combat::random_combat_cell(gm) != 0 && {
                    let (dy, dx) = (gm.g16(g::DY), gm.g16(g::DX));
                    ovl::combat::combat_cell_free(gm, dy, dx, 0x1c) != 0
                };
                if !ok {
                    result = 0;
                } else {
                    let (dx8, dy8) = (gm.g16(g::DX) as u8, gm.g16(g::DY) as u8);
                    gm.set_g8(g::ROSTER_X.wrapping_add(r), dx8);
                    gm.set_g8(g::MONSTER_X.wrapping_add(slot8(slot as u16)), dx8);
                    gm.set_g8(g::ROSTER_Y.wrapping_add(r), dy8);
                    gm.set_g8(g::MONSTER_X.wrapping_add(slot8(slot as u16)).wrapping_add(1), dy8);
                    result = 1;
                }
                if result != 0 { break; }
            }
        }
    }
    result
}

/// orig 1000:cc18 (88 bytes). In Quas Corp (fear): casting flash circle 7, then every monster
/// entry ((flags & 0xc0) == 0x40) that is not spell-immune and fails `target_evades(0, i,
/// active)` gets byte +0 = 1 and flags |= 2 (flees).
pub fn cast_in_quas_corp_fear(gm: &mut Game) {
    ovl::cast2::casting_flash(gm, 7);
    let mut i: i16 = 0;
    loop {
        let e = g::COMBAT_ROSTER.wrapping_add(ros8(i));
        if (gm.g8(e.wrapping_add(2)) & 0xc0) == 0x40 {
            if is_spell_immune_combatant(gm, i) == 0 {
                let active = gm.g8(g::ACTIVE_COMBATANT) as u16;
                if ovl::comsubs::target_evades(gm, 0, i as u16, active) == 0 {
                    gm.set_g8(e, 1);
                    let f = gm.g8(e.wrapping_add(2));
                    gm.set_g8(e.wrapping_add(2), f | 2);
                }
            }
        }
        i += 1;
        if !(i < 0x20) { break; }
    }
}

/// orig 1000:cc70 (92 bytes). Vas Rel Por (gate travel). Not aboard a ship ((g_transport_tile
/// & 0xf0) != 0x20): "To phase: " (0x45e7), a command key (echoed when printable, then
/// newline); '1'..'8' selects moonstone key-'1', casting flash circle 8 and ROOT
/// `moongate_travel(n)`; when that succeeds 0xffff is returned (the caller then returns 0 =
/// no turn). Everything else returns 0 ("Failed!").
pub fn cast_vas_rel_por_gate_travel(gm: &mut Game) -> u16 {
    if (gm.g8(g::TRANSPORT_TILE) & 0xf0) != 0x20 {
        text::print_string(gm, 0x45e7); // "To phase: "
        let k = input::read_command_key(gm) as u8;
        if 0x1f < k {
            text::put_char(gm, k);
        }
        text::put_char(gm, 10);
        if 0x30 < k && k < 0x39 {
            let n = k.wrapping_sub(0x31) as i8;
            ovl::cast2::casting_flash(gm, 8);
            if rest::moongate_travel(gm, n as i16) != 0 {
                return 0xffff;
            }
        }
    }
    0
}

/// orig 1000:cccc (109 bytes). An Tym (negate time): casting flash circle 8; if any
/// monster-table slot holds tile 0xfc (a Shadowlord) "Magic absorbed!" (0x45f2) with
/// play_sweep_noise(2, 1000, 0x6d60, 1, 0x2648) and 0 is returned; otherwise g_status_icon =
/// 'T', g_protection_turns = 10, the status window is redrawn and -1 (no message) is returned.
pub fn cast_an_tym_negate_time(gm: &mut Game) -> i16 {
    ovl::cast2::casting_flash(gm, 8);
    let mut result: i16 = -1;
    let mut i: i16 = 0;
    loop {
        if gm.gi8(g::MONSTER_TABLE.wrapping_add(ros8(i))) == -4 {
            text::print_string(gm, 0x45f2); // "Magic absorbed!\n"
            util::play_sweep_noise(gm, 2, 1000, 0x6d5e + 2, 1, 0x2648);
            result = 0;
            break;
        }
        i += 1;
        if !(i < 0x20) { break; }
    }
    if result != 0 {
        gm.set_g8(g::STATUS_ICON, 0x54); // 'T'
        gm.set_g8(g::PROTECTION_TURNS, 10);
        rest::draw_status_window(gm);
    }
    result
}

/// orig 1000:cd3a (931 bytes). The Cast command (vector 1778:06da; ROOT dispatch_command 'C'
/// and COMBAT a8ce). Sets g_save_unk_2e9 = 1 and the attack-is-spell flag (0x5890) = 1, picks
/// the caster with `select_target_member` (negative = cancelled, returned as is), prints
/// "Spell name:" (0x4603) and reads the spell with CAST2 e2be: -1 "None!" (0x4611), -2 "No
/// effect!" (0x4618). circle = spell/6 + 1 (also stored in g_save_unk_2e9). Context check on
/// g_spell_unk_1c90[spell]: overworld bit 8, town (< 0x21) bit 4, dungeon bit 2, combat bit 1;
/// in Lord British's castle (0x12) without the crown, or location 0x1d, "Absorbed!" (0x4624)
/// with a sweep noise; a disallowed context "Not here!" (0x462f) with the failure tone. No
/// mixture ("None mixed!" 0x463a) returns; else the mixture count is decremented. MP (record
/// +0xf) < circle: "M.P. too low!" (0x4647) then "Failed!"; else MP -= circle, and a level
/// (+0x16) below the circle fails silently to "Failed!". The spell routine then runs (see the
/// per-spell functions and CAST2: 0 In Lor eaca(100), 1 Grav Por, 8 Rel Hur e5ea(0, dir), 12
/// Vas Lor eaca(255), 18 An Grav e99c(1), 19 In Sanct ead8(4,20,'P'), 21/22 Uus/Des Por
/// (not at location 0x28) DUNGEON klimb -1/+1 then exit_dungeon, 26 In Ex Por e948, 29 Rel
/// Tym ead8(5,30,'Q'), 31 Quas An Wis ead8(6,20,'C'), 32 In An ead8(6,10,'N'), 33 Wis An Ylem
/// e64c, 39 In Quas Wis gem view, 42 In Mani Corp e7c0(0, e27e()), 43 Kal Xen Corp e6a2(0),
/// winds 28/40/44/45 `cast_wind_spell`). A result of 1 prints "Success!" (0x4656), 0 prints
/// "Failed!" (0x4660) with the failure tone, anything else nothing. Returns 1 (turn used),
/// or 0 after a successful Vas Rel Por.
pub fn cast_spell_command(gm: &mut Game) -> i16 {
    let mut turn: i16 = 1;
    gm.set_g8(g::SAVE_UNK_2E9, 1);
    gm.set_g8(ATTACK_IS_SPELL, 1);
    let mut status: i16 = 1;
    let member = rest::select_target_member(gm) as i16;
    if member < 0 {
        return member;
    }
    text::print_string(gm, 0x4603); // "Spell name:\n:"
    let spell = ovl::cast2::read_spell_name(gm);
    // TODO(verify): these early exits return whatever print_string/the sound routine left
    // in AX (undefined); 1 (turn used) is assumed.
    if spell == -1 {
        text::print_string(gm, 0x4611); // "None!\n"
        return 1;
    }
    if spell == -2 {
        text::print_string(gm, 0x4618); // "No effect!\n"
        return 1;
    }
    let circle: u16 = (spell / 6 + 1) as u16;
    let circle8 = circle as i8;
    gm.set_g8(g::SAVE_UNK_2E9, circle as u8);
    let flag = gm.g8(g::SPELL_UNK_1C90.wrapping_add(spell as u16));
    let loc = gm.g8(g::LOCATION);
    let not_allowed: bool;
    if loc == 0 {
        not_allowed = (flag & 8) == 0;
    } else if loc < 0x80 {
        if (loc == 0x12 && gm.g8(g::CROWN_LB) == 0) || loc == 0x1d {
            text::print_string(gm, 0x4624); // "Absorbed!\n"
            util::play_sweep_noise(gm, 2, 1000, 0x6d5e + 2, 1, 0x2648);
            return 1;
        }
        if loc < 0x21 {
            not_allowed = (flag & 4) == 0;
        } else {
            not_allowed = (flag & 2) == 0;
        }
    } else {
        not_allowed = (flag & 1) == 0;
    }
    if not_allowed {
        status = 0;
    }
    if status == 0 {
        text::print_string(gm, 0x462f); // "Not here!\n"
        rest::play_tone_sweep(gm, 0x32, 1, 2000, 800);
        return 1;
    }
    let mixed = g::MIXED_SPELLS.wrapping_add(spell as u16);
    if gm.g8(mixed) == 0 {
        text::print_string(gm, 0x463a); // "None mixed!\n"
        return 1;
    }
    let mv = gm.g8(mixed);
    gm.set_g8(mixed, mv.wrapping_sub(1));
    let rec = mem32(member as u16);
    'dispatch: {
        if (gm.g8(PARTY_MP.wrapping_add(rec)) as u16) < circle {
            text::print_string(gm, 0x4647); // "M.P. too low!\n"
            status = 0;
            break 'dispatch;
        }
        let mp = gm.gi8(PARTY_MP.wrapping_add(rec));
        gm.set_gi8(PARTY_MP.wrapping_add(rec), mp.wrapping_sub(circle8));
        status = -1;
        if (gm.g8(PARTY_LEVEL.wrapping_add(rec)) as u16) < circle {
            status = 0;
            break 'dispatch;
        }
        let active = gm.g8(g::ACTIVE_COMBATANT) as i16;
        match spell {
            0 => {
                ovl::cast2::set_light_spell_turns(gm, 100);
                ovl::cast2::casting_flash(gm, 1);
            }
            1 => cast_missile_spell(gm, 0x30),
            2 => status = cast_an_zu_awaken(gm) as i16,
            3 => status = cast_an_nox_cure(gm) as i16,
            4 => status = cast_mani_heal(gm),
            5 => status = cast_an_ylem_dissolve(gm) as i16,
            6 => status = cast_an_sanct_unlock(gm) as i16,
            7 => cast_an_xen_corp_repel_undead(gm),
            8 => {
                let dir = ovl::cast2::ask_direction(gm);
                ovl::cast2::rel_hur_change_wind(gm, 0, dir);
            }
            9 => cast_in_wis_locate(gm),
            10 => status = cast_kal_xen_summon_animal(gm) as i16,
            0xb => status = cast_in_xen_mani_create_food(gm) as i16,
            0xc => {
                ovl::cast2::set_light_spell_turns(gm, 0xff);
                ovl::cast2::casting_flash(gm, 3);
            }
            0xd => cast_missile_spell(gm, 0x31),
            0xe => status = cast_field_spell(gm, 0) as i16,
            0xf => status = cast_field_spell(gm, 1) as i16,
            0x10 => status = cast_field_spell(gm, 2) as i16,
            0x11 => status = cast_in_por_blink(gm),
            0x12 => status = ovl::cast2::an_grav_dispel_field(gm, 1) as i16,
            0x13 => ovl::cast2::start_timed_effect(gm, 4, 0x14, 0x50),
            0x14 => status = cast_field_spell(gm, 3) as i16,
            0x15 | 0x16 => {
                if loc == 0x28 {
                    status = 0;
                } else {
                    ovl::cast2::casting_flash(gm, 4);
                    let dir: i16 = if spell == 0x15 { -1 } else { 1 };
                    if ovl::dungeon::dungeon_klimb_ladder(gm, 1, dir) != 0 {
                        ovl::dungeon::exit_dungeon(gm);
                    }
                }
            }
            0x17 => cast_wis_quas_reveal(gm),
            0x18 => status = cast_in_bet_xen_insect_swarm(gm),
            0x19 => status = cast_an_ex_por_magic_lock(gm) as i16,
            0x1a => {
                status = ovl::cast2::in_ex_por_unlock_door(gm) as i16;
                if status != -1 {
                    ovl::cast2::casting_flash(gm, 5);
                }
            }
            0x1b => status = cast_vas_mani_great_heal(gm) as i16,
            0x1c => {
                let c = gm.g16(g::UI_COLOR4);
                cast_wind_spell(gm, c, 1, active);
            }
            0x1d => ovl::cast2::start_timed_effect(gm, 5, 0x1e, 0x51),
            0x1e => cast_in_vas_por_ylem_earthquake(gm, member),
            0x1f => ovl::cast2::start_timed_effect(gm, 6, 0x14, 0x43),
            0x20 => ovl::cast2::start_timed_effect(gm, 6, 10, 0x4e),
            0x21 => {
                ovl::cast2::casting_flash(gm, 6);
                ovl::cast2::wis_an_ylem_xray_view(gm);
            }
            0x22 => status = cast_an_xen_ex_charm(gm) as i16,
            0x23 => status = cast_rel_xen_bet_shrink(gm) as i16,
            0x24 => status = cast_sanct_lor_invisibility(gm) as i16,
            0x25 => cast_missile_spell(gm, 0x32),
            0x26 => status = cast_in_quas_xen_clone(gm),
            0x27 => {
                ovl::cast2::casting_flash(gm, 7);
                if loc < 0x21 {
                    let (y, x) = (gm.gi8(g::PARTY_Y), gm.gi8(g::PARTY_X));
                    ovl::lookobj::view_gem_map(gm, y, x);
                } else {
                    ovl::dnglook::view_gem_dungeon(gm);
                }
            }
            0x28 => {
                let c = gm.g16(g::UI_COLOR3);
                cast_wind_spell(gm, c, 2, active);
            }
            0x29 => cast_in_quas_corp_fear(gm),
            0x2a => {
                let m = ovl::cast2::pick_target_member(gm);
                status = ovl::cast2::in_mani_corp_resurrect(gm, 0, m) as i16;
                rest::draw_status_window(gm);
            }
            0x2b => status = ovl::cast2::kal_xen_corp_summon_daemon(gm, 0) as i16,
            0x2c => {
                let c = gm.g16(g::UI_COLOR2);
                cast_wind_spell(gm, c, 4, active);
            }
            0x2d => {
                let c = gm.g16(g::UI_COLOR0);
                cast_wind_spell(gm, c, 3, active);
            }
            0x2e => {
                status = cast_vas_rel_por_gate_travel(gm) as i16;
                if status != 0 {
                    turn = 0;
                }
            }
            0x2f => status = cast_an_tym_negate_time(gm),
            _ => {}
        }
    }
    // switchD_1000_ce9a_default
    if status == 1 {
        text::print_string(gm, 0x4656); // "Success!\n"
    } else if status == 0 {
        text::print_string(gm, 0x4660); // "Failed!\n"
        rest::play_tone_sweep(gm, 0x32, 1, 2000, 800);
    }
    turn
}

/// orig 1000:d15e (353 bytes). Uses scroll `scroll` (0..7): g_scrolls[scroll] -= 1, "Scroll"
/// (0x466a), then: 0 Vas Lor: CAST2 eaca(0xf0), "Light!" (0x4673), casting flash 0; 1 Rel Hur:
/// "Wind change!" (0x467b), direction e4e6, outside dungeons e5ea(1, dir) and 1 else 0; 2 In
/// Sanct: "Protection!" (0x4689) ead8(2, 100, 'P'); 3 In An: "Negate magic!" (0x4696) ead8(3,
/// 20, 'N'); 4 In Quas Wis: "View!" (0x46a5), outside combat flash 4 and the gem view
/// (LOOKOBJ view_gem_map / DNGLOOK view_gem_dungeon) else "Not here!" (0x46ac); 5 Kal Xen
/// Corp: "Summon Daemon!" (0x46b7), in combat returns e6a2(1) else "Not here!" (0x46c7); 6 In
/// Mani Corp: "Resurrection!" (0x46d2), outside combat e7c0(1, e27e()) with a status redraw,
/// else "Not here!" (0x46e1); 7 An Tym: at locations 0x1d/0x28 "No effect!" (0x46ec) with the
/// failure tone, else "Negate time!" (0x46f8) ead8(7, 20, 'T'). Returns 1 except where noted
/// (0 = "Failed!" printed by the caller).
pub fn use_scroll(gm: &mut Game, param_1: i16) -> u16 {
    let scroll = param_1;
    let sa = g::SCROLLS.wrapping_add(scroll as u16);
    let v = gm.g8(sa);
    gm.set_g8(sa, v.wrapping_sub(1));
    text::print_string(gm, 0x466a); // "Scroll\n\n"
    let loc = gm.g8(g::LOCATION);
    let (kind, turns, icon): (u16, u8, u8);
    match scroll {
        0 => {
            ovl::cast2::set_light_spell_turns(gm, 0xf0);
            text::print_string(gm, 0x4673); // "Light!\n"
            ovl::cast2::casting_flash(gm, 0);
            return 1;
        }
        1 => {
            text::print_string(gm, 0x467b); // "Wind change!\n"
            let dir = ovl::cast2::ask_direction(gm);
            if loc < 0x21 {
                ovl::cast2::rel_hur_change_wind(gm, 1, dir);
                return 1;
            }
            return 0;
        }
        2 => {
            text::print_string(gm, 0x4689); // "Protection!\n"
            icon = 0x50;
            turns = 100;
            kind = 2;
        }
        3 => {
            text::print_string(gm, 0x4696); // "Negate magic!\n"
            icon = 0x4e;
            turns = 0x14;
            kind = 3;
        }
        4 => {
            text::print_string(gm, 0x46a5); // "View!\n"
            if loc < 0x80 {
                ovl::cast2::casting_flash(gm, 4);
                if loc < 0x21 {
                    let (y, x) = (gm.gi8(g::PARTY_Y), gm.gi8(g::PARTY_X));
                    ovl::lookobj::view_gem_map(gm, y, x);
                    return 1;
                }
                ovl::dnglook::view_gem_dungeon(gm);
                return 1;
            }
            text::print_string(gm, 0x46ac); // "Not here!\n"
            return 1;
        }
        5 => {
            text::print_string(gm, 0x46b7); // "Summon Daemon!\n"
            if 0x7f < loc {
                return ovl::cast2::kal_xen_corp_summon_daemon(gm, 1);
            }
            text::print_string(gm, 0x46c7); // "Not here!\n"
            return 1;
        }
        6 => {
            text::print_string(gm, 0x46d2); // "Resurrection!\n"
            if loc < 0x80 {
                let m = ovl::cast2::pick_target_member(gm);
                let r = ovl::cast2::in_mani_corp_resurrect(gm, 1, m);
                rest::draw_status_window(gm);
                return r;
            }
            text::print_string(gm, 0x46e1); // "Not here!\n"
            return 1;
        }
        7 => {
            if loc == 0x1d || loc == 0x28 {
                text::print_string(gm, 0x46ec); // "No effect!\n"
                rest::play_tone_sweep(gm, 0x32, 1, 2000, 800);
                return 1;
            }
            text::print_string(gm, 0x46f8); // "Negate time!\n"
            icon = 0x54;
            turns = 0x14;
            kind = 7;
        }
        _ => return 1,
    }
    ovl::cast2::start_timed_effect(gm, kind, turns, icon);
    1
}

/// orig 1000:d2da (459 bytes). Uses potion `potion` (0..7: blue, yellow, red, green, orange,
/// purple, black, white): g_potions[potion] -= 1, "Potion" (0x4706); the drinker is CAST2 e27e
/// outside combat or the active combatant's member in combat (negative = cancel, returned).
/// Casting flash of circle `potion`. A rand 0..15 of 0 turns the potion into orange (4) and 1
/// into a random potion 0..7. Effects: 0 awaken: 'S' -> 'G' (in combat the active combatant
/// must be that member with flags (& 0xe8) == 0x88, then `wake_combatant`, 1) else 0; 1 heal:
/// e5a2(member), 0 when it fails, "Healed!" (0x470e); 2 cure: 'P' -> 'G' "Poison cured!"
/// (0x4717) else 0; 3 poison: 'G' -> 'P' "POISONED!" (0x4726) else 0; 4 sleep: 'G' -> 'S' (in
/// combat `incapacitate_combatant`) "Slept!" (0x4731) else 0; 5 in combat "Poof!" (0x4739) and
/// the combatant's tiles become 0x90; 6 in combat roster flags |= 0x10, "Invisible!" (0x4740),
/// tiles 0x1d; 7 outside dungeons CAST2 e64c (x-ray); 5..7 otherwise "No noticeable effect
/// now!" (0x474c). Messages 1..4 are followed by a status redraw. Returns 1 unless noted.
pub fn use_potion(gm: &mut Game, param_1: i16) -> u16 {
    let mut potion = param_1;
    let mut result: u16 = 1;
    let pa = g::POTIONS.wrapping_add(potion as u16);
    let v = gm.g8(pa);
    gm.set_g8(pa, v.wrapping_sub(1));
    text::print_string(gm, 0x4706); // "Potion\n"
    let loc = gm.g8(g::LOCATION);
    let active = gm.g8(g::ACTIVE_COMBATANT) as u16;
    let m: u16 = if loc < 0x80 {
        ovl::cast2::pick_target_member(gm) as u16
    } else {
        gm.g8(g::ROSTER_MEMBER.wrapping_add(slot8(active))) as u16
    };
    if (m as i16) < 0 {
        return m;
    }
    ovl::cast2::casting_flash(gm, potion);
    let r = util::rand_range(gm, 0xf, 0);
    if r == 0 {
        potion = 4;
    } else if r == 1 {
        potion = util::rand_range(gm, 7, 0);
    }
    let st = g::PARTY_STATUS.wrapping_add(mem32(m));
    let msg: u16;
    match potion {
        0 => {
            if gm.g8(st) != b'S' {
                return 0;
            }
            gm.set_g8(st, b'G');
            if 0x7f < loc {
                let e = slot8(active);
                if gm.g8(g::ROSTER_MEMBER.wrapping_add(e)) as u16 != m {
                    return 0;
                }
                if (gm.g8(g::ROSTER_FLAGS.wrapping_add(e)) & 0xe8) != 0x88 {
                    return 0;
                }
                rest::wake_combatant(gm, active as i16);
                return 1;
            }
            // LAB_1000_d3c1
            rest::draw_status_window(gm);
            return result;
        }
        1 => {
            result = ovl::cast2::heal_member(gm, m as i16) as u16;
            if result == 0 {
                return 0;
            }
            msg = 0x470e; // "Healed!\n"
        }
        2 => {
            if gm.g8(st) != b'P' {
                return 0;
            }
            gm.set_g8(st, b'G');
            msg = 0x4717; // "Poison cured!\n"
        }
        3 => {
            if gm.g8(st) != b'G' {
                return 0;
            }
            gm.set_g8(st, b'P');
            msg = 0x4726; // "POISONED!\n"
        }
        4 => {
            if gm.g8(st) != b'G' {
                return 0;
            }
            if loc < 0x80 {
                gm.set_g8(st, b'S');
            } else {
                rest::incapacitate_combatant(gm, m);
            }
            msg = 0x4731; // "Slept!\n"
        }
        5 => {
            if 0x7f < loc {
                text::print_string(gm, 0x4739); // "Poof!\n"
                let so = slot8(gm.g8(g::ROSTER_SLOT.wrapping_add(slot8(active))) as u16);
                gm.set_g8(g::MONSTER_TABLE.wrapping_add(so).wrapping_add(1), 0x90);
                gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), 0x90);
                return 1;
            }
            text::print_string(gm, 0x474c); // "\nNo noticeable effect now!\n"
            return result;
        }
        6 => {
            if 0x7f < loc {
                let fa = g::ROSTER_FLAGS.wrapping_add(slot8(active));
                let f = gm.g8(fa);
                gm.set_g8(fa, f | 0x10);
                let so = slot8(gm.g8(g::ROSTER_SLOT.wrapping_add(slot8(active))) as u16);
                text::print_string(gm, 0x4740); // "Invisible!\n"
                gm.set_g8(g::MONSTER_TABLE.wrapping_add(so).wrapping_add(1), 0x1d);
                gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), 0x1d);
                return 1;
            }
            text::print_string(gm, 0x474c); // "\nNo noticeable effect now!\n"
            return result;
        }
        7 => {
            if loc < 0x21 {
                ovl::cast2::wis_an_ylem_xray_view(gm);
                return 1;
            }
            // LAB_1000_d452
            text::print_string(gm, 0x474c); // "\nNo noticeable effect now!\n"
            return result;
        }
        _ => return result,
    }
    text::print_string(gm, msg);
    rest::draw_status_window(gm);
    result
}

/// orig 1000:d4bc (119 bytes). Buries moonstone `n` (0..7): "Moonstone " (0x4768); outside
/// dungeons on grass/swamp-like tiles (0x2c, 0x2d or 4..10) "buried!" (0x4773) and
/// g_moonstone_x/y[n] = party x/y, g_moonstone_loc[n] = g_location, g_moonstone_z[n] =
/// g_party_z; otherwise "cannot be buried here!" (0x477c).
pub fn bury_moonstone(gm: &mut Game, param_1: i16) {
    let n = param_1 as u16;
    let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
    let t = gm.g8(p);
    text::print_string(gm, 0x4768); // "Moonstone "
    if gm.g8(g::LOCATION) < 0x21 && (t == 0x2c || t == 0x2d || (3 < t && t < 0xb)) {
        text::print_string(gm, 0x4773); // "buried!\n"
        gm.set_g8(g::MOONSTONE_X.wrapping_add(n), gm.g8(g::PARTY_X));
        gm.set_g8(g::MOONSTONE_Y.wrapping_add(n), gm.g8(g::PARTY_Y));
        gm.set_g8(g::MOONSTONE_LOC.wrapping_add(n), gm.g8(g::LOCATION));
        gm.set_g8(g::MOONSTONE_Z.wrapping_add(n), gm.g8(g::PARTY_Z));
    } else {
        text::print_string(gm, 0x477c); // "cannot be buried here!\n"
    }
}

/// orig 1000:d534 (420 bytes). Uses gem shard `shard` (0 Falsehood, 1 Hatred, 2 Cowardice).
/// "Gem Shard / Thou dost hold above thee the evil Shard of" (0x4794) + the shard name
/// (0x47cc/0x47d9/0x47e3), a rising then falling sweep noise (2000..25000 step 50). The party
/// must stand at the shard's flame position g_shard_sites (0x4882 x, 0x4886 y, 0x488a
/// location, 0x488e z, one byte per shard) else "No effect!" (0x47f0) with the failure tone.
/// animate_wait(7), "...and cast it into the Flame of" (0x47fe) Truth/Love/Courage
/// (0x4822/0x482a/0x4831), three `earthquake_shake`s, animate_wait(3). The object north of the
/// party (`find_object_at(z, y-1, x)`) must be a Shadowlord (0xfc) and `shard` must equal
/// g_save_unk_325 (which Shadowlord is present): then the NPC owning that slot (TOWN
/// find_npc_by_slot(g_dx)) is removed (TOWN remove_npc), the cell north flashes 7 times,
/// g_shadowlord_loc[shard] = 0xff (destroyed), the shard (0x57b6+shard) is consumed, the
/// Shadowlord-slain bits at 0x5bca |= g_shard_doom_bits[shard] (0x4892: 2, 4, 8), "The doom
/// of the Shadowlord" (0x483b) Faulinei/Astaroth/Nosfentor (0x4858/0x4861/0x486a) "is
/// wrought!" (0x4874) and `play_triple_sweep_sound`.
pub fn use_gem_shard(gm: &mut Game, param_1: u16) {
    let shard = param_1;
    text::print_string(gm, 0x4794);
    if shard == 0 {
        text::print_string(gm, 0x47cc); // "Falsehood..."
    } else if shard == 1 {
        text::print_string(gm, 0x47d9); // "Hatred..."
    } else if shard == 2 {
        text::print_string(gm, 0x47e3); // "Cowardice..."
    }
    let mut f: i16 = 2000;
    loop {
        util::play_sweep_noise(gm, 0, f as u16, 200, 1, 0xa50);
        f = f.wrapping_add(0x32);
        if !(f < 25000) { break; }
    }
    f = 25000;
    loop {
        util::play_sweep_noise(gm, 0, f as u16, 200, 1, 0xa50);
        f = f.wrapping_sub(0x32);
        if !(2000 < f) { break; }
    }
    if gm.g8(g::UNK_4882.wrapping_add(shard)) != gm.g8(g::PARTY_X)
        || gm.g8(0x4886u16.wrapping_add(shard)) != gm.g8(g::PARTY_Y)
        || gm.g8(0x488au16.wrapping_add(shard)) != gm.g8(g::LOCATION)
        || gm.g8(0x488eu16.wrapping_add(shard)) != gm.g8(g::PARTY_Z)
    {
        text::print_string(gm, 0x47f0); // "\n\nNo effect!\n"
        rest::play_tone_sweep(gm, 0x32, 1, 2000, 800);
        return;
    }
    util::animate_wait(gm, 7);
    text::print_string(gm, 0x47fe); // "\n\n...and cast it into the Flame of "
    if shard == 0 {
        text::print_string(gm, 0x4822); // "Truth!\n"
    } else if shard == 1 {
        text::print_string(gm, 0x482a); // "Love!\n"
    } else if shard == 2 {
        text::print_string(gm, 0x4831); // "Courage!\n"
    }
    rest::earthquake_shake(gm);
    rest::earthquake_shake(gm);
    rest::earthquake_shake(gm);
    util::animate_wait(gm, 3);
    let z = gm.g8(g::PARTY_Z) as u16;
    let yn = (gm.g8(g::PARTY_Y) as u16).wrapping_sub(1);
    let x = gm.g8(g::PARTY_X) as u16;
    let obj = rest::find_object_at(gm, z, yn, x);
    if obj != 0xfc {
        return;
    }
    if shard != gm.g8(g::SAVE_UNK_325) as u16 {
        return;
    }
    let npc = ovl::town::find_npc_by_slot(gm, gm.g16(g::DX) as i16);
    ovl::town::remove_npc(gm, npc);
    let mut k: i16 = 7;
    loop {
        let (py, px) = (gm.g8(g::PARTY_Y) as i16, gm.g8(g::PARTY_X) as i16);
        rest::flash_tile_at(gm, py.wrapping_sub(1), px);
        k -= 1;
        if k == 0 { break; }
    }
    gm.set_g8(g::SHADOWLORD_LOC.wrapping_add(shard), 0xff);
    gm.set_g8(g::SHARD_FALSEHOOD.wrapping_add(shard), 0);
    let bits = gm.g8(0x4892u16.wrapping_add(shard));
    let v = gm.g8(0x5bca); // Shadowlords-destroyed bits (save 0x624)
    gm.set_g8(0x5bca, v | bits);
    text::print_string(gm, 0x483b); // "\nThe doom of the Shadowlord "
    'name: {
        let s: u16 = if shard == 0 {
            0x4858 // "Faulinei"
        } else if shard == 1 {
            0x4861 // "Astaroth"
        } else {
            if shard != 2 {
                break 'name;
            }
            0x486a // "Nosfentor"
        };
        text::print_string(gm, s);
    }
    text::print_string(gm, 0x4874); // " is wrought!\n"
    rest::play_triple_sweep_sound(gm);
}

/// orig 1000:d6e4 (44 bytes). If `icon` is the current g_status_icon: "Removed!" (0x4895),
/// g_protection_turns = 0, g_status_icon = 0, g_status_dirty = 1. Returns true when the icon
/// was NOT active (i.e. the item effect may now be applied), false when it was just removed.
pub fn remove_status_effect_if(gm: &mut Game, param_1: u16) -> bool {
    let cur = gm.g8(g::STATUS_ICON) as u16;
    if param_1 == cur {
        text::print_string(gm, 0x4895); // "Removed!\n"
        gm.set_g8(g::PROTECTION_TURNS, 0);
        gm.set_g8(g::STATUS_ICON, 0);
        gm.set_g8(g::STATUS_DIRTY, 1);
    }
    param_1 != cur
}

/// orig 1000:d712 (996 bytes). The Use item command (vector 1778:06c2; ROOT dispatch_command
/// 'U' and the combat Use). ZSTATS `snapshot_items_page` builds the 0x26-entry Items page at
/// 0xb9ee; `next_owned(0xff, 0xb9ee, 0x26, -1)` finds the first owned entry (-1: "No usable
/// items!" 0x489f). "Item: " (0x48b1), the status window gets the "Items:" title (0x48b8) and
/// an 8-row list frame and `ready_browser('U', 0xff, first)` picks an entry; the status window
/// is redrawn (negative = cancel). Entries: 0..7 scrolls (`use_scroll`), 8..15 potions
/// (`use_potion`), 0x15..0x1c moonstones (`bury_moonstone`), 0x10 carpet: outdoors/town on a
/// non-0x0c tile, on foot (0x1c) "Boarded!" (0x48c8) transport = 0x14 + rand 0..1 and
/// g_magic_carpets -= 1, on a ship "X-it ship first!" (0x48d2), else "Only on foot!"
/// (0x48e4), elsewhere "Not here!" (0x48f3); 0x11 skull key: g_skull_keys -= 1, outside
/// dungeons CAST2 e948 (failure -> "Failed!") with `flash_tile_at(g_dy, g_dx)` outside
/// combat, in a dungeon "Not here!" (0x4909); 0x12 amulet / 0x13 crown: toggled through
/// `remove_status_effect_if` (icons 0x0e / 0x1c), when newly worn "Wearing the Amulet" /
/// "Thou dost don the Crown" "of Lord British..." and CAST2 ead8(9, 0xff, icon); 0x14 sceptre:
/// text, play_sweep_noise(1, 5000, 50000, 1, 0x1450), outside 0x21..0x28 every tile 0x70..0x7f
/// (force field) in the 3x3 around the party becomes 5 with a refresh and random tones; if
/// none, CAST2 e99c(0): 1 "Field dissolved!" (0x496f), 0 "No effect!" (0x4981); 0x1d..0x1f gem
/// shards (`use_gem_shard`); 0x20 spyglass: outdoors by night "Looking..." (0x4998) LOOKOBJ
/// look_telescope, by day "No stars!" (0x49a4), else "Not here!" (0x49af); 0x21 plans: on a
/// ship g_hms_cape_plans |= 0x80 "Ship rigged for double speed!" (0x49c2) else "Only usable on
/// shipboard!" (0x49e1); 0x22 sextant: overworld by night "Position:" (0x4a26) CAST2 e8cc,
/// by day "Only at night!" (0x4a16), else "Only outdoors!" (0x4a06); 0x23 watch: "The pocket
/// watch reads " h:mm AM/PM; 0x24 badge: `remove_status_effect_if(0x1d)` then "Badge worn!"
/// (0x4a63), g_status_icon = 0x1d, g_protection_turns = 0xff; 0x25 box: "Box / How?" (0x4a70).
/// A zero result from scroll/potion/skull key prints "Failed!" (0x4a7b) with the failure tone.
/// The original's return value is a leftover register; the port returns that result flag.
pub fn use_item_command(gm: &mut Game) -> u16 {
    let mut ok: u16 = 1;
    ovl::zstats::snapshot_items_page(gm);
    let first = ovl::zstats::next_owned(gm, 0xff, 0xb9eeu16 as i16, 0x26, -1);
    if first == -1 {
        text::print_string(gm, 0x489f); // "No usable items!\n"
        return ok;
    }
    text::print_string(gm, 0x48b1); // "Item: "
    rest::clear_status_separator(gm);
    text::select_text_window(gm, 1);
    rest::draw_status_title(gm, 0x48b8); // "Items:"
    ovl::zstats::draw_list_frame(gm, 8);
    let sel = ovl::zstats::ready_browser(gm, 0x55, 0xff, first);
    rest::draw_status_icon_bar(gm);
    rest::draw_status_top_bar(gm);
    rest::draw_status_separator(gm);
    rest::draw_status_window(gm);
    if sel < 0 {
        return ok;
    }
    let loc = gm.g8(g::LOCATION);
    'tail: {
        if sel < 8 {
            ok = use_scroll(gm, sel);
            break 'tail;
        }
        if sel < 0x10 {
            ok = use_potion(gm, sel - 8);
            break 'tail;
        }
        if 0x14 < sel && sel < 0x1d {
            bury_moonstone(gm, sel - 0x15);
            break 'tail;
        }
        let msg: u16;
        match sel - 0x10 {
            0 => {
                text::print_string(gm, 0x48bf); // "Carpet\n\n"
                if loc < 0x21 && {
                    let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
                    gm.gi8(p) != 0x0c
                } {
                    if gm.g8(g::TRANSPORT_TILE) == 0x1c {
                        text::print_string(gm, 0x48c8); // "Boarded!\n"
                        let r = util::rand_range(gm, 1, 0) as i8;
                        gm.set_g8(g::TRANSPORT_TILE, (r.wrapping_add(0x14)) as u8);
                        let c = gm.g8(g::MAGIC_CARPETS);
                        gm.set_g8(g::MAGIC_CARPETS, c.wrapping_sub(1));
                        break 'tail;
                    }
                    if (gm.g8(g::TRANSPORT_TILE) & 0xf8) == 0x20 {
                        msg = 0x48d2; // "X-it ship first!\n"
                    } else {
                        msg = 0x48e4; // "Only on foot!\n"
                    }
                } else {
                    msg = 0x48f3; // "Not here!\n"
                }
            }
            1 => {
                let k = gm.g8(g::SKULL_KEYS);
                gm.set_g8(g::SKULL_KEYS, k.wrapping_sub(1));
                text::print_string(gm, 0x48fe); // "Skull Key\n"
                if loc < 0x21 || 0x7f < loc {
                    ok = ovl::cast2::in_ex_por_unlock_door(gm);
                    if ok != 0 && loc < 0x80 {
                        let (dy, dx) = (gm.gi16(g::DY), gm.gi16(g::DX));
                        rest::flash_tile_at(gm, dy, dx);
                    }
                    break 'tail;
                }
                msg = 0x4909; // "Not here!\n"
            }
            2 => {
                text::print_string(gm, 0x4914); // "Amulet\n\n"
                if !remove_status_effect_if(gm, 0xe) {
                    break 'tail;
                }
                text::print_string(gm, 0x491d); // "Wearing the Amulet"
                text::print_string(gm, 0x4a84); // " of Lord British...\n"
                ovl::cast2::start_timed_effect(gm, 9, 0xff, 0xe);
                break 'tail;
            }
            3 => {
                text::print_string(gm, 0x4930); // "Crown\n\n"
                if !remove_status_effect_if(gm, 0x1c) {
                    break 'tail;
                }
                text::print_string(gm, 0x4938); // "Thou dost don the Crown"
                text::print_string(gm, 0x4a84); // " of Lord British...\n"
                ovl::cast2::start_timed_effect(gm, 9, 0xff, 0x1c);
                break 'tail;
            }
            4 => {
                text::print_string(gm, 0x4950); // "Sceptre\n\n"
                text::print_string(gm, 0x495a); // "Wielding the Sceptre"
                text::print_string(gm, 0x4a84); // " of Lord British...\n"
                util::play_sweep_noise(gm, 1, 0x137e + 10, 50000, 1, 0x1450);
                let mut count: i16 = 0;
                if loc < 0x21 || 0x28 < loc {
                    let mut ex: i16 = -1;
                    loop {
                        let mut ey: i16 = -1;
                        loop {
                            let y = (gm.g8(g::PARTY_Y) as u16).wrapping_add(ey as u16);
                            let x = (ex as u16).wrapping_add(gm.g8(g::PARTY_X) as u16);
                            let p = rest::get_map_tile(gm, y, x);
                            if (gm.g8(p) & 0xf0) == 0x70 {
                                gm.set_g8(p, 5);
                                rest::refresh_map_view(gm);
                                util::play_random_tones(gm, 2000, 3000, 10);
                                count += 1;
                            }
                            ey += 1;
                            if !(ey < 2) { break; }
                        }
                        ex += 1;
                        if !(ex < 2) { break; }
                    }
                }
                if count != 0 {
                    break 'tail;
                }
                let r = ovl::cast2::an_grav_dispel_field(gm, 0);
                if r == 1 {
                    msg = 0x496f; // "Field dissolved!\n"
                } else {
                    if r != 0 {
                        break 'tail;
                    }
                    msg = 0x4981; // "No effect!\n"
                }
            }
            0xd | 0xe | 0xf => {
                use_gem_shard(gm, (sel - 0x1d) as u16);
                break 'tail;
            }
            0x10 => {
                text::print_string(gm, 0x498d); // "Spyglass\n\n"
                if loc < 0x21 && gm.g8(g::PARTY_Z) < 0x80 {
                    let h = gm.g8(g::HOUR);
                    if h < 6 || 0x12 < h {
                        text::print_string(gm, 0x4998); // "Looking...\n"
                        ovl::lookobj::look_telescope(gm);
                        break 'tail;
                    }
                    msg = 0x49a4; // "No stars!\n"
                } else {
                    msg = 0x49af; // "Not here!\n"
                }
            }
            0x11 => {
                text::print_string(gm, 0x49ba); // "Plans\n\n"
                if (gm.g8(g::TRANSPORT_TILE) & 0xf8) == 0x20 {
                    let p = gm.g8(g::HMS_CAPE_PLANS);
                    gm.set_g8(g::HMS_CAPE_PLANS, p | 0x80);
                    msg = 0x49c2; // "Ship rigged for double speed!\n"
                } else {
                    msg = 0x49e1; // "Only usable on shipboard!\n"
                }
            }
            0x12 => {
                text::print_string(gm, 0x49fc); // "Sextant\n\n"
                if gm.g8(g::PARTY_Z) < 0x80 && loc == 0 {
                    let h = gm.g8(g::HOUR);
                    if h < 6 || 0x12 < h {
                        text::print_string(gm, 0x4a26); // "Position:"
                        ovl::cast2::in_wis_print_position(gm);
                        break 'tail;
                    }
                    msg = 0x4a16; // "Only at night!\n"
                } else {
                    msg = 0x4a06; // "Only outdoors!\n"
                }
            }
            0x13 => {
                text::print_string(gm, 0x4a30); // "Watch\n\nThe pocket watch reads "
                let mut h = gm.g8(g::HOUR) % 0xc;
                if h == 0 {
                    h = 0xc;
                }
                text::print_number(gm, 0x20, 1, h as i16);
                text::put_char(gm, 0x3a);
                let mi = gm.g8(g::MINUTE);
                text::print_number(gm, 0x30, 2, mi as i16);
                if gm.g8(g::HOUR) < 0xc {
                    msg = 0x4a55; // " AM.\n"
                } else {
                    msg = 0x4a4f; // " PM.\n"
                }
            }
            0x14 => {
                text::print_string(gm, 0x4a5b); // "Badge\n\n"
                if remove_status_effect_if(gm, 0x1d) {
                    text::print_string(gm, 0x4a63); // "Badge worn!\n"
                    gm.set_g8(g::STATUS_ICON, 0x1d);
                    gm.set_g8(g::PROTECTION_TURNS, 0xff);
                    gm.set_g8(g::STATUS_DIRTY, 1);
                }
                break 'tail;
            }
            0x15 => {
                msg = 0x4a70; // "Box\n\nHow?\n"
            }
            _ => break 'tail,
        }
        text::print_string(gm, msg);
    }
    // switchD_1000_d7dd_caseD_5
    if ok == 0 {
        text::print_string(gm, 0x4a7b); // "Failed!\n"
        rest::play_tone_sweep(gm, 0x32, 1, 2000, 800);
    }
    ok
}

/// orig 1000:db30 (132 bytes). Ghidra order (color, y, x). One wind-particle step: a pixel
/// outside 8..0xb6 (either axis) sets g_dx = g_dy = -1 and returns 0 (particle finished).
/// Otherwise the colour is set and the two pixels (y, x) and (y, x+1) plotted, a random tone
/// 100..10000 is started; on even rows 1 is returned without touching g_dx/g_dy; on odd rows
/// `pixel_to_view_cell(y, x)` and, when both are >= 0, `view_tile_missile_passable(g_dy,
/// g_dx)` is returned (0 when the cell blocks), else 0.
pub fn plot_wind_pixel(gm: &mut Game, param_1: u16, param_2: u16, param_3: i16) -> u16 {
    let (color, y, x) = (param_1, param_2, param_3);
    if x < 8 || 0xb6 < x || (y as i16) < 8 || 0xb6 < (y as i16) {
        gm.set_gi16(g::DY, -1);
        gm.set_gi16(g::DX, -1);
    } else {
        gfx::set_color(gm, color as i16);
        gfx::plot_pixel(gm, y as i16, x);
        gfx::plot_pixel(gm, y as i16, x.wrapping_add(1));
        let f = util::rand_range(gm, 0x270f + 1, 100);
        util::start_tone(gm, f as u16);
        if (y & 1) == 0 {
            return 1;
        }
        rest::pixel_to_view_cell(gm, y as i16, x);
        if -1 < gm.gi16(g::DX) && -1 < gm.gi16(g::DY) {
            let (dy, dx) = (gm.gi16(g::DY), gm.gi16(g::DX));
            return rest::view_tile_missile_passable(gm, dy, dx) as u16;
        }
    }
    0
}

/// orig 1000:dbb6 (806 bytes). Ghidra order (dir, ri, color, ys, xs). Animates the 21
/// particles of a wind/cone spell cast by roster entry `ri` towards `dir` (1 W, 2 E, 3 N, 4 S)
/// and collects the view cells they pass through. All particles start at the caster's cell
/// pixel (x*16, y*16) offset (8,8) and shifted to the leading edge for the direction (W/E:
/// y+16, E: x+24; N: x+16; S: x+16, y+24). Particle energies are a copy of g_unk_1cf0 (21
/// words, at 0xa9d0). Passes repeat until all 21 particles are finished: each unfinished
/// particle moves rand 0..15 steps; a step draws it with `plot_wind_pixel(color+8, y, x)` (0
/// finishes it) and, when the step reported a passable cell (g_dx/g_dy in 0..10) whose
/// g_view_tiles byte is 0 and fewer than 63 cells are stored, records (g_dx, g_dy) at index
/// count+1 of xs/ys (index 0 is never written) and marks the view byte 0xff. Then energy -=
/// 10, the particle advances one pixel in `dir`, and when the energy drops below 1 it drifts
/// sideways (particles 0..9 one way, 10..20 the other) and is recharged with g_unk_1cf0[i].
/// `stop_tone` at the end; returns the number of cells recorded.
pub fn animate_wind_cone(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16, ys: &mut [i16; 64], xs: &mut [i16; 64]) -> i16 {
    let (dir, ri, color) = (param_1, param_2, param_3);
    let x0 = (gm.g8(g::ROSTER_X.wrapping_add(ros8(ri))) as i16).wrapping_mul(0x10);
    let mut px = x0.wrapping_add(8);
    let y0 = (gm.g8(g::ROSTER_Y.wrapping_add(ros8(ri))) as i16).wrapping_mul(0x10);
    let mut py = y0.wrapping_add(8);
    'edge: {
        if dir != 1 {
            if dir != 2 {
                if dir == 3 {
                    px = x0.wrapping_add(0x10);
                } else if dir == 4 {
                    px = x0.wrapping_add(0x10);
                    py = y0.wrapping_add(0x18);
                }
                break 'edge;
            }
            px = x0.wrapping_add(0x18);
        }
        py = y0.wrapping_add(0x10);
    }
    // LAB_1000_dc08
    let mut done = [0i16; 21];
    for i in 0..21u16 {
        let w = gm.g16(g::UNK_1CF0.wrapping_add(i.wrapping_mul(2)));
        gm.set_g16(WIND_ENERGY.wrapping_add(i.wrapping_mul(2)), w);
    }
    let mut pxs = [px; 21];
    let mut pys = [py; 21];
    let mut count: i16 = 0;
    loop {
        let mut finished: i16 = 0;
        for i in 0..21usize {
            if done[i] == 0 {
                let steps = rest::rand_0_to(gm, 0xf);
                let mut k: i16 = 0;
                if 0 < steps {
                    let ea = WIND_ENERGY.wrapping_add((i as u16).wrapping_mul(2));
                    loop {
                        let r = plot_wind_pixel(gm, color.wrapping_add(8) as u16, pys[i] as u16, pxs[i]);
                        if r == 0 {
                            done[i] = 1;
                        } else {
                            let (dx, dy) = (gm.gi16(g::DX), gm.gi16(g::DY));
                            if count < 63 && -1 < dx && -1 < dy && dx < 0xb && dy < 0xb
                                && gm.g8(g::VIEW_TILES.wrapping_add((dy.wrapping_mul(0x20).wrapping_add(dx)) as u16)) == 0
                            {
                                count += 1;
                                xs[count as usize] = dx;
                                ys[count as usize] = dy;
                                gm.set_g8(g::VIEW_TILES.wrapping_add((dy.wrapping_mul(0x20).wrapping_add(dx)) as u16), 0xff);
                            }
                        }
                        if done[i] == 0 {
                            let e = gm.gi16(ea).wrapping_sub(10);
                            gm.set_gi16(ea, e);
                            if dir == 1 {
                                pxs[i] = pxs[i].wrapping_sub(1);
                            } else if dir == 2 {
                                pxs[i] = pxs[i].wrapping_add(1);
                            } else if dir == 3 {
                                pys[i] = pys[i].wrapping_sub(1);
                            } else if dir == 4 {
                                pys[i] = pys[i].wrapping_add(1);
                            }
                            if gm.gi16(ea) < 1 {
                                let s: i16 = if (i as i16) < 10 { -1 } else { 1 };
                                if dir == 1 {
                                    pys[i] = pys[i].wrapping_sub(s);
                                } else if dir == 2 {
                                    pys[i] = pys[i].wrapping_add(s);
                                } else if dir == 3 {
                                    pxs[i] = pxs[i].wrapping_add(s);
                                } else if dir == 4 {
                                    pxs[i] = pxs[i].wrapping_sub(s);
                                }
                                let base = gm.gi16(g::UNK_1CF0.wrapping_add((i as u16).wrapping_mul(2)));
                                let e = gm.gi16(ea).wrapping_add(base);
                                gm.set_gi16(ea, e);
                            }
                        }
                        if done[i] != 0 { break; }
                        k += 1;
                        if !(k < steps) { break; }
                    }
                }
            } else {
                finished += 1;
            }
        }
        if 0x14 < finished {
            util::stop_tone(gm);
            return count;
        }
        gm.pf.maybe_present();
    }
}

/// orig 1000:dee0 (514 bytes). Ghidra order (color, kind, ri). The wind/cone spells cast by
/// roster entry `ri`: kind 1 In Zu (sleep), 2 In Nox Hur (poison), 3 In Flam Hur (flame, rand
/// 0..30 damage), 4 In Vas Grav Corp (death, 99 damage). CAST2 e4e6 asks the direction, a
/// random-tone burst plays (700, 16000/19200/20800 by kind, 800) and `animate_wind_cone`
/// records the cells reached. For every recorded cell (indices 0..count; index 0 is
/// uninitialised in the original) the roster is scanned from entry 31 down for a live entry
/// (flags != 0 and not 0x20) at that x/y whose +5 byte lacks bit 7; that bit is set (hit once
/// per spell) and the effect applied: 1 `target_evades(0, j, ri)` or spell-immune aborts,
/// else flash and `incapacitate_combatant`; 2 aborts when random_damage_1_30() <
/// get_combat_stat(-2, j), else flash and COMBAT `poison_attack(ri, j)`; 3 flash, COMBAT
/// `apply_damage(rand 0..30, j)` with the experience added to the caster's record (+0x14,
/// cap 9999); 4 as 1 for the checks, then `apply_damage(99, j)` with experience; other kinds
/// abort. Each hit ends with COMSUBS `print_hit_result(ri, j)` and only the first matching
/// entry per cell is hit. Finally bit 7 of the +5 byte is cleared in all 32 entries.
pub fn cast_wind_spell(gm: &mut Game, param_1: u16, param_2: i16, param_3: i16) {
    let (color, kind, ri) = (param_1, param_2, param_3);
    let rec = mem32(gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(ri))) as u16);
    let dir = ovl::cast2::ask_direction(gm);
    let freq: i16 = if kind == 1 { 16000 } else if kind == 2 { 0x4b00 } else { 0x5140 };
    util::play_random_tones(gm, 700, freq, 800);
    // TODO(verify): index 0 of both lists is never written by animate_wind_cone and holds
    // stack garbage in the original; -1 (never matches a byte) is used here.
    let mut ys = [-1i16; 64];
    let mut xs = [-1i16; 64];
    let n = animate_wind_cone(gm, dir, ri, color as i16, &mut ys, &mut xs);
    if 1 < n.wrapping_add(1) {
        let mut i: usize = 0;
        loop {
            let mut j: i16 = 0x1f;
            let mut e: u16 = g::COMBAT_ROSTER.wrapping_add(0x1f * 8);
            loop {
                if gm.g8(e.wrapping_add(6)) as u16 == xs[i] as u16
                    && gm.g8(e.wrapping_add(7)) as u16 == ys[i] as u16
                    && (gm.g8(e.wrapping_add(5)) & 0x80) == 0
                    && (gm.g8(e.wrapping_add(2)) & 0x20) == 0
                    && gm.gi8(e.wrapping_add(2)) != 0
                {
                    let v = gm.g8(e.wrapping_add(5));
                    gm.set_g8(e.wrapping_add(5), v | 0x80);
                    if kind == 1 {
                        if ovl::comsubs::target_evades(gm, 0, j as u16, ri as u16) != 0
                            || is_spell_immune_combatant(gm, j) != 0
                        {
                            break;
                        }
                        rest::flash_monster(gm, j as u16);
                        rest::incapacitate_combatant(gm, j as u16);
                    } else if kind == 2 {
                        let stat = ovl::combat::get_combat_stat(gm, -2, j) as i16;
                        let roll = rest::random_damage_1_30(gm);
                        if roll < stat {
                            break;
                        }
                        rest::flash_monster(gm, j as u16);
                        ovl::combat::poison_attack(gm, ri, j);
                    } else {
                        let dmg: i16;
                        if kind == 3 {
                            rest::flash_monster(gm, j as u16);
                            dmg = rest::rand_0_to(gm, 0x1e);
                        } else {
                            if kind != 4
                                || ovl::comsubs::target_evades(gm, 0, j as u16, ri as u16) != 0
                                || is_spell_immune_combatant(gm, j) != 0
                            {
                                break;
                            }
                            rest::flash_monster(gm, j as u16);
                            dmg = 99;
                        }
                        let exp = rec.wrapping_add(PARTY_EXP);
                        let xp = ovl::combat::apply_damage(gm, dmg as u16, j) as i16;
                        rest::add_i16_capped(gm, 0x270f, xp, exp);
                    }
                    ovl::comsubs::print_hit_result(gm, ri, j);
                    break;
                }
                e = e.wrapping_sub(8);
                j -= 1;
                if !(-1 < j) { break; }
            }
            i += 1;
            if i == (n.wrapping_add(1)) as usize { break; }
        }
    }
    let mut p: u16 = 0xbb11; // roster entry 31 byte +5
    loop {
        let v = gm.g8(p);
        gm.set_g8(p, v & 0x7f);
        p = p.wrapping_sub(8);
        if !(0xba11 < p) { break; }
    }
}
