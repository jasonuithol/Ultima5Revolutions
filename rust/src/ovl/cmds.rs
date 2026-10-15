//! CMDS.OVL (translated)
//!
//! Level-3 overlay with the remaining player commands dispatched from ROOT `dispatch_command`:
//! Board, Fire (ship broadsides outdoors, town cannons), Hole-up in a bed (town sleeping),
//! Ignite torch, New order, Xit, Yell (sails / words of power / Shadowlord summoning / shrine
//! meditation), Push, Mix reagents and Klimb with the grapple, plus the camping routine
//! `camp_and_watch` that ROOT `run_encounter` runs on the camp map (sleep, ambush, healing, the
//! apparition) and the combat "Escape" (`combat_escape_all`). All map access goes through
//! `get_map_tile`; the monster/object table (0x5c5a, 8 bytes per slot) holds vehicles and NPCs;
//! the 16-wide view tile/object buffers (g_view_tiles 0xab02 with 0x20 stride, g_view_objects
//! 0xac64 with 0x10 stride) are used for "land nearby" and cannon-direction tests.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// Monster-table slot offset (8 bytes per slot at 0x5c5a).
#[inline] fn slot8(slot: u16) -> u16 { slot.wrapping_mul(8) }
/// Combat-roster entry offset (8 bytes per entry at 0xba14).
#[inline] fn ros8(i: i16) -> u16 { (i as u16).wrapping_mul(8) }
/// Party record offset (32 bytes per member at 0x55a8).
#[inline] fn mem32(i: u16) -> u16 { i.wrapping_mul(0x20) }
/// Dungeon map cell under the party: g_dungeon_map + z*0x40 + y*8 + x (the C reads the 16-bit
/// word at 0x5895 = z | x<<8 and splits it).
fn party_dungeon_cell(gm: &mut Game) -> u8 {
    let zx = gm.g16(0x5895); // z (low byte) and x (high byte)
    let a = g::DUNGEON_MAP.wrapping_add(zx >> 8).wrapping_add((zx & 0xff).wrapping_mul(0x40)).wrapping_add((gm.g8(g::PARTY_Y) as u16).wrapping_mul(8));
    gm.g8(a)
}

/// orig 1000:bf80 (1359 bytes). Camping on the camp combat map, run by ROOT `run_encounter`
/// (vector 1778:08f6). Ghidra order (hours, watch member or -1, flags; flags bit 1 = dungeon
/// camp, bit 7 = dungeon room). Clears protection/status icon and the active member, builds the
/// combat party (`init_combat_party`, or DNGLOOK b40e(0, cell) in a dungeon), end hour = (hour +
/// hours) mod 24; the watch index is forced to -1 when outside -1..5. Every Good/Poisoned/
/// Sleeping member other than the watch is put to sleep (poisoned members counted per member);
/// a Bard on watch with sound on plays a tune (its sprite becomes 0x5f for animate_wait(0x34)).
/// Prints 0x41d4 and loops until the end hour: each pass refreshes the view, runs the
/// regeneration ring tick and the status window; whenever the hour has just changed there is a
/// 1/64 chance of an ambush, otherwise 5 minutes pass (pass_time(5), wait_ticks(1); the sky
/// strip is drawn with g_location temporarily = g_location_backup when camping outdoors / in
/// a town) and the watch wanders randomly on the field (1 in 4 per pass; combat_pos_in_bounds +
/// COMBAT a290). Ambush: new random seed from the clock, monster type = g_camp_monsters[rand
/// 0..7] (0x1734), "Ambushed!", with a watch every roster entry 0..5 is woken (status 'G', or 'P'
/// when it was poisoned), then setup_encounter(type, flags) (or DNGLOOK b40e(1, cell)) and 1 is
/// returned (a fight follows). No ambush: g_hours_until_heal != 0 or hours < 6 -> "No
/// effect...", else "Party rested!": each unpoisoned, living, non-watch member gains rand 1..63
/// HP (capped at max) and mana = INT (classes A/M) or INT/2 (B); outside dungeons 25% chance of
/// the apparition (g_save_unk_2e7 = g_month, OUTSUBS a8e8); g_hours_until_heal = 14. Finally
/// sleeping members become 'G', the active member is restored, the status window redrawn and 0
/// returned.
pub fn camp_and_watch(gm: &mut Game, param_1: i16, param_2: u16, param_3: u16) -> u16 {
    let (hours, mut watch, flags) = (param_1, param_2, param_3);
    let mut last_hour = gm.g8(g::HOUR);
    let saved_active = gm.g8(g::ACTIVE_MEMBER);
    let mut bard_ri: i16 = -1;
    let mut ambush: u16 = 0xffff;
    gm.set_g8(g::PROTECTION_TURNS, 0);
    gm.set_g8(g::STATUS_ICON, 0);
    gm.set_g8(g::ACTIVE_MEMBER, 0xff);
    if (flags & 2) == 0 {
        rest::init_combat_party(gm);
    } else {
        let cell = party_dungeon_cell(gm);
        ovl::dnglook::setup_room_combatants(gm, 0, cell as i16);
    }
    gm.set_g8(g::ACTIVE_COMBATANT, 0xff);
    let mut end_hour: u16 = (hours as u16).wrapping_add(gm.g8(g::HOUR) as u16);
    if 0x17 < (end_hour as i16) {
        end_hour = end_hour.wrapping_sub(0x18);
    }
    if 5 < (watch as i16) || (watch as i16) < -1 {
        watch = 0xffff;
    }
    let mut poisoned = [0i16; 8];
    let mut m: u16 = 0;
    if gm.g8(g::PARTY_COUNT) != 0 {
        loop {
            let mut ri: i16 = 0;
            loop {
                if m == gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(ri))) as u16 { break; }
                ri += 1;
                if !(ri < 0x20) { break; }
            }
            let st = gm.g8(g::PARTY_STATUS.wrapping_add(mem32(m)));
            if st == 0x47 || st == 0x50 || st == 0x53 { // 'G' 'P' 'S'
                if st == 0x50 {
                    poisoned[(m & 7) as usize] += 1;
                }
                // LAB_1000_c085
                if m != watch {
                    rest::sleep_combatant_unless_poisoned(gm, ri);
                }
                if gm.g8(g::PARTY_CLASS.wrapping_add(mem32(m))) == 0x42 && m == watch && gm.g8(g::SOUND_ENABLED) != 0 {
                    bard_ri = ri;
                }
            }
            m += 1;
            if !(m < gm.g8(g::PARTY_COUNT) as u16) { break; }
        }
    }
    rest::refresh_map_view(gm);
    if bard_ri != -1 {
        let so = slot8(gm.g8(g::ROSTER_SLOT.wrapping_add(ros8(bard_ri))) as u16);
        let t0 = gm.g8(g::MONSTER_TABLE.wrapping_add(so));
        let a0 = gm.g8(g::PARTY_VEHICLE_TILE.wrapping_add(so)); // anim tile of the slot
        gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(so), 0x5f);
        gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), 0x5f);
        gm.set_g16(g::TUNE_POS, 1);
        util::animate_wait(gm, 0x34);
        gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), t0);
        gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(so), a0);
        if gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(bard_ri))) as u16 != watch {
            rest::sleep_combatant_unless_poisoned(gm, bard_ri);
        }
        rest::refresh_map_view(gm);
    }
    text::print_string(gm, g::STR_CMDS_CAMP); // 0x41d4
    let mut watch_entry: u16 = g::COMBAT_ROSTER;
    let mut watch_tile: u8 = 0;
    if watch != 0xffff {
        let mut ri: i16 = 0;
        loop {
            if gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(ri))) as u16 == watch { break; }
            ri += 1;
            if !(ri < 0x20) { break; }
        }
        watch_entry = g::COMBAT_ROSTER.wrapping_add(ros8(ri));
        watch_tile = gm.g8(g::MONSTER_TABLE.wrapping_add(slot8(gm.g8(watch_entry.wrapping_add(4)) as u16)));
    }
    // LAB_1000_c16e
    loop {
        if end_hour == gm.g8(g::HOUR) as u16 { break; }
        if 0x17 < gm.g8(g::HOUR) {
            let h = gm.g8(g::HOUR) - 0x18;
            gm.set_g8(g::HOUR, h);
        }
        rest::refresh_map_view(gm);
        rest::regeneration_ring_tick(gm);
        rest::draw_status_window(gm);
        if last_hour == gm.g8(g::HOUR) || util::rand_range(gm, 0x3f, 0) != 0 {
            last_hour = gm.g8(g::HOUR);
            rest::pass_time(gm, 5);
            util::wait_ticks(gm, 1);
            if gm.g8(g::LOCATION_BACKUP) < 0x21 {
                let lb = gm.g8(g::LOCATION_BACKUP);
                gm.set_g8(g::LOCATION, lb);
                rest::draw_sky_strip(gm);
                gm.set_g8(g::LOCATION, 0xff);
            }
            if watch != 0xffff && util::rand_range(gm, 3, 0) == 2 {
                gm.set_g16(g::DX, gm.g8(watch_entry.wrapping_add(6)) as u16);
                gm.set_g16(g::DY, gm.g8(watch_entry.wrapping_add(7)) as u16);
                let r = util::rand_range(gm, 3, 0);
                if r == 0 {
                    let v = gm.g16(g::DY).wrapping_sub(1); gm.set_g16(g::DY, v);
                } else if r == 1 {
                    let v = gm.g16(g::DY).wrapping_add(1); gm.set_g16(g::DY, v);
                } else if r == 2 {
                    let v = gm.g16(g::DX).wrapping_add(1); gm.set_g16(g::DX, v);
                } else if r == 3 {
                    let v = gm.g16(g::DX).wrapping_sub(1); gm.set_g16(g::DX, v);
                }
                let dy = gm.g16(g::DY);
                let dx = gm.g16(g::DX);
                if rest::combat_pos_in_bounds(gm, dy as i16, dx as i16) != 0
                    && ovl::combat::combat_cell_free(gm, dy, dx, watch_tile as u16) != 0
                {
                    let so = slot8(gm.g8(watch_entry.wrapping_add(4)) as u16);
                    gm.set_g8(g::MONSTER_X.wrapping_add(so), dx as u8);
                    gm.set_g8(watch_entry.wrapping_add(6), dx as u8);
                    gm.set_g8(g::MONSTER_Y.wrapping_add(so), dy as u8);
                    gm.set_g8(watch_entry.wrapping_add(7), dy as u8);
                    rest::refresh_map_view(gm);
                }
            }
            continue;
        }
        // Ambush
        let seed = util::get_time_hash(gm);
        util::set_random_seed(gm, seed);
        let r = util::rand_range(gm, 7, 0);
        ambush = gm.g8(0x1734u16.wrapping_add(r as u16)) as u16; // camp ambush monster table
        text::print_string(gm, 0x41e0); // "Ambushed!"
        if -1 < (watch as i16) {
            let mut ri: i16 = 0;
            while ri < 6 {
                let mm = gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(ri))) as u16;
                if mm < gm.g8(g::PARTY_COUNT) as u16 {
                    rest::wake_combatant(gm, ri);
                    if poisoned[(mm & 7) as usize] == 0 {
                        gm.set_g8(g::PARTY_STATUS.wrapping_add(mem32(mm)), 0x47); // 'G'
                    } else {
                        gm.set_g8(g::PARTY_STATUS.wrapping_add(mem32(mm)), 0x50); // 'P'
                    }
                }
                ri += 1;
            }
        }
        rest::draw_status_window(gm);
        if (flags & 2) == 0 {
            rest::setup_encounter(gm, ambush, flags);
        } else {
            let cell = party_dungeon_cell(gm);
            ovl::dnglook::setup_room_combatants(gm, 1, cell as i16);
        }
        break;
    }
    if ambush < 0x8000 {
        return 1;
    }
    if gm.g8(g::HOURS_UNTIL_HEAL) != 0 || hours < 6 {
        text::print_string(gm, 0x41fb); // "No effect..."
    } else {
        text::print_string(gm, 0x41ec); // "Party rested!"
        if gm.g8(g::PARTY_COUNT) != 0 {
            let mut i: u16 = 0;
            loop {
                let o = mem32(i);
                if poisoned[(i & 7) as usize] == 0
                    && gm.g8(g::HOURS_UNTIL_HEAL) == 0
                    && 5 < hours
                    && gm.g8(g::PARTY_STATUS.wrapping_add(o)) != 0x44
                    && i != watch
                {
                    let add = util::rand_range(gm, 0x3f, 1);
                    let hp = gm.gi16(g::PARTY_HP.wrapping_add(o)).wrapping_add(add);
                    gm.set_gi16(g::PARTY_HP.wrapping_add(o), hp);
                    if gm.gi16(g::PARTY_MAX_HP.wrapping_add(o)) < gm.gi16(g::PARTY_HP.wrapping_add(o)) {
                        let mx = gm.gi16(g::PARTY_MAX_HP.wrapping_add(o));
                        gm.set_gi16(g::PARTY_HP.wrapping_add(o), mx);
                    }
                    if i != watch {
                        let c = gm.g8(g::PARTY_CLASS.wrapping_add(o));
                        if c == 0x41 || c == 0x4d { // 'A' / 'M': mana = INT
                            let v = gm.g8(0x55b6u16.wrapping_add(o));
                            gm.set_g8(0x55b7u16.wrapping_add(o), v);
                        } else if c == 0x42 { // 'B': mana = INT / 2
                            let v = gm.g8(0x55b6u16.wrapping_add(o)) >> 1;
                            gm.set_g8(0x55b7u16.wrapping_add(o), v);
                        }
                    }
                }
                // LAB_1000_c423
                i += 1;
                if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
            }
        }
        if (flags & 0x82) == 0 && util::rand_range(gm, 99, 0) < 0x19 {
            let mo = gm.g8(g::MONTH);
            gm.set_g8(g::SAVE_UNK_2E7, mo);
            ovl::outsubs::apparition_level_up(gm);
        }
        gm.set_g8(g::HOURS_UNTIL_HEAL, 0xe);
    }
    // LAB_1000_c493
    let n = gm.g8(g::PARTY_COUNT) as u16;
    if n != 0 {
        let mut i: u16 = 0;
        loop {
            let a = g::PARTY_STATUS.wrapping_add(mem32(i));
            if gm.g8(a) == 0x53 { gm.set_g8(a, 0x47); }
            i += 1;
            if !(i < n) { break; }
        }
    }
    gm.set_g8(g::ACTIVE_MEMBER, saved_active);
    rest::draw_status_window(gm);
    0
}

/// orig 1000:c4d2 (409 bytes). Hole up in a bed inside a settlement (dispatch_command 'H' on
/// tile 0xab; vector 1778:08ae). "For how many hours?" reads a digit (space cancels; '0' does
/// nothing): the digit is echoed, end hour = hour + n (mod 24). 16 NPC movement ticks (NPC b044)
/// with view refreshes (aborted if g_save_unk_1018[0] == 'a'); all 'G' members become 'S'; status
/// window, 0x421e, the map window is blanked (fill_rect(0xb7,0xb7,8,8) in colour 0). Then until
/// the end hour: pass_time(10), the day/night tile switch (TOWN 8340) at hours 20 and 5, sky
/// strip, end_of_turn_party_update, status window, NPC placement (TOWN 9864), and if any
/// object/NPC stands on the party cell the sleep is interrupted ("Thrown out of bed!"). Sleepers
/// wake ('S' -> 'G'), the party (and slot 0) is moved one cell east, g_view_dirty = 1, the active
/// member is restored and the screen redrawn.
pub fn sleep_in_bed(gm: &mut Game) {
    text::print_string(gm, 0x4209); // "For how many hours?"
    let mut k: u8;
    loop {
        k = input::read_command_key(gm) as u8;
        if k == 0x20 { break; }
        if !(k < 0x30 || 0x39 < k) { break; }
    }
    if k != 0x20 && k != 0x30 {
        text::put_char(gm, k);
        text::put_char(gm, 10);
        let t = (gm.g8(g::HOUR) as i16).wrapping_add(k as i16);
        let mut end: u16 = t.wrapping_sub(0x30) as u16;
        if 0x17 < (end as i16) {
            end = t.wrapping_sub(0x47) as u16;
        }
        let mut saved_active: u8 = 0;
        let mut i: i16 = 0;
        loop {
            let h = gm.g8(g::HOUR) as u16;
            ovl::npc::move_npcs(gm, h);
            rest::refresh_map_view(gm);
            saved_active = gm.g8(g::ACTIVE_MEMBER);
            if gm.g8(g::SAVE_UNK_1018) == 0x61 { return; } // 'a'
            i += 1;
            if !(i < 0x10) { break; }
        }
        let n = gm.g8(g::PARTY_COUNT) as u16;
        if n != 0 {
            let mut j: u16 = 0;
            loop {
                let a = g::PARTY_STATUS.wrapping_add(mem32(j));
                if gm.g8(a) == 0x47 { gm.set_g8(a, 0x53); }
                j += 1;
                if !(j < n) { break; }
            }
        }
        rest::draw_status_window(gm);
        text::print_string(gm, 0x421e);
        gfx::set_color(gm, 0);
        gfx::fill_rect(gm, 0xb7, 0xb7, 8, 8);
        let mut last = gm.g8(g::HOUR);
        while end != gm.g8(g::HOUR) as u16 {
            rest::pass_time(gm, 10);
            if last != gm.g8(g::HOUR) && (gm.g8(g::HOUR) == 0x14 || gm.g8(g::HOUR) == 5) {
                ovl::town::toggle_night_tiles(gm);
            }
            last = gm.g8(g::HOUR);
            rest::draw_sky_strip(gm);
            rest::end_of_turn_party_update(gm);
            rest::draw_status_window(gm);
            ovl::town::init_town_npcs(gm);
            let (pz, py, px) = (gm.g8(g::PARTY_Z) as u16, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
            if rest::find_object_at(gm, pz, py, px) != 0 {
                end = 0xffff;
                break;
            }
            util::wait_ticks(gm, 1);
        }
        if end == 0xffff {
            text::print_string(gm, 0x422a); // "Thrown out of bed!"
        }
        let n = gm.g8(g::PARTY_COUNT) as u16;
        if n != 0 {
            let mut j: u16 = 0;
            loop {
                let a = g::PARTY_STATUS.wrapping_add(mem32(j));
                if gm.g8(a) == 0x53 { gm.set_g8(a, 0x47); }
                j += 1;
                if !(j < n) { break; }
            }
        }
        let px = gm.g8(g::PARTY_X).wrapping_add(1);
        gm.set_g8(g::PARTY_X, px);
        gm.set_g8(g::VIEW_DIRTY, 1);
        let mx = gm.g8(g::MONSTER_X).wrapping_add(1);
        gm.set_g8(g::MONSTER_X, mx);
        gm.set_g8(g::ACTIVE_MEMBER, saved_active);
        rest::draw_status_window(gm);
        rest::refresh_map_view(gm);
    }
}

/// orig 1000:c66e (29 bytes). Returns 1 when the party is on foot (transport tile 0x1c/0x1d),
/// else prints "On foot" (0x423e) and returns 0 — guard for boarding horses/carpets/skiffs.
pub fn require_on_foot(gm: &mut Game) -> u16 {
    let t = gm.g8(g::TRANSPORT_TILE);
    if t == 0x1c || t == 0x1d {
        1
    } else {
        text::print_string(gm, 0x423e); // "On foot"
        0
    }
}

/// orig 1000:c68c (50 bytes). Returns 1 when the party may board a frigate: transport tile in
/// 0x14..0x15 (carpet), 0x1c..0x1d (on foot) or 0x28..0x2b (skiff); otherwise prints "On foot"
/// (0x4248) and returns 0.
pub fn can_board_ship(gm: &mut Game) -> u16 {
    let t = gm.g8(g::TRANSPORT_TILE);
    if t < 0x14 || (0x15 < t && (t < 0x1c || (0x1d < t && (t < 0x28 || 0x2b < t)))) {
        text::print_string(gm, 0x4248); // "On foot"
        0
    } else {
        1
    }
}

/// orig 1000:c6be (72 bytes). Returns 1 when any of the four cells around the view centre
/// (5,4) (5,6) (6,5) (4,5) is walkable land per `is_land_view_cell`.
pub fn land_nearby(gm: &mut Game) -> u16 {
    if is_land_view_cell(gm, 5, 4) != 1 && is_land_view_cell(gm, 5, 6) != 1 && is_land_view_cell(gm, 6, 5) != 1 && is_land_view_cell(gm, 4, 5) != 1 {
        return 0;
    }
    1
}

/// orig 1000:c708 (109 bytes). View cell (x,y) test (Ghidra order (x, y)): if the view tile
/// (g_view_tiles, 0x20 stride) is 0 the object layer (g_view_objects, 0x10 stride) decides -
/// a carpet (0x1b), horse (0x10/0x1c groups), ship (0x24) or skiff (0x28) counts as land (1);
/// otherwise can_enter_tile(tile, 0x1c) (walkable on foot).
pub fn is_land_view_cell(gm: &mut Game, param_1: i16, param_2: i16) -> u16 {
    let (x, y) = (param_1, param_2);
    let vt = gm.g8(g::VIEW_TILES.wrapping_add(x as u16).wrapping_add((y as u16).wrapping_mul(0x20)));
    if vt == 0 {
        let mut ob = gm.g8(g::VIEW_OBJECTS.wrapping_add(x as u16).wrapping_add((y as u16).wrapping_mul(0x10)));
        if ob == 0x1b { return 1; }
        ob &= 0xfc;
        if ob == 0x1c || ob == 0x24 || ob == 0x10 || ob == 0x28 { 1 } else { 0 }
    } else {
        rest::can_enter_tile(gm, vt as i16, 0x1c)
    }
}

/// orig 1000:c776 (360 bytes). The Board command (dispatch 'B', vector 1778:08ba). Not in a
/// dungeon: the object on the party cell (find_object_at, slot in g_dx) is boarded: horse
/// (tile & 0xfe == 0x10; in a settlement an NPC owning the slot with a non-zero mode refuses,
/// 0x425e) -> "horse", transport = tile + 2; carpet 0x1b -> "carpet", transport 0x14; skiff
/// (0x28 group) -> 0x4275, transport = tile; frigate (0x24 group, `can_board_ship`) -> 0x427c,
/// "DANGER! SHIP BADLY DAMAGED!" when hull (value1) < 10, skiffs = value3 (+1 when arriving by
/// skiff; a carpet is folded back into g_magic_carpets), slot 0 value1 = hull, "WARNING! NO
/// SKIFFS ON BOARD!" when 0, slot 0 value3 = skiffs, status dirty, transport = tile. Any other
/// object prints 0x42bf and returns 0. The boarded slot is cleared and g_view_dirty |= 2. In a
/// dungeon "Not here!". Returns 1.
pub fn cmd_board(gm: &mut Game) -> u16 {
    let loc = gm.g8(g::LOCATION);
    if loc < 0x21 || 0x28 < loc {
        let (pz, py, px) = (gm.g8(g::PARTY_Z) as u16, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
        let t = rest::find_object_at(gm, pz, py, px);
        let slot = gm.gi16(g::DX);
        if (t & 0xfe) == 0x10 {
            if loc != 0 {
                let npc = ovl::town::find_npc_by_slot(gm, slot);
                if npc != -1 && gm.gi16((npc as u16).wrapping_mul(0x10).wrapping_add(0x5f68)) != 0 { // g_npc_mode
                    text::print_string(gm, 0x425e);
                    return 1;
                }
            }
            if require_on_foot(gm) == 0 { return 1; }
            text::print_string(gm, 0x4266); // "horse"
            gm.set_g8(g::TRANSPORT_TILE, t.wrapping_add(2));
        } else if t == 0x1b {
            if require_on_foot(gm) == 0 { return 1; }
            text::print_string(gm, 0x426d); // "carpet"
            gm.set_g8(g::TRANSPORT_TILE, 0x14);
        } else if (t & 0xfc) == 0x28 {
            if require_on_foot(gm) == 0 { return 1; }
            text::print_string(gm, 0x4275);
            gm.set_g8(g::TRANSPORT_TILE, t);
        } else {
            if (t & 0xfc) != 0x24 {
                text::print_string(gm, 0x42bf);
                return 0;
            }
            if can_board_ship(gm) == 0 { return 1; }
            text::print_string(gm, 0x427c);
            let so = slot8(slot as u16);
            let hull = gm.g8(g::MONSTER_Z.wrapping_add(so).wrapping_add(1)); // value1
            if hull < 10 {
                text::print_string(gm, 0x4282); // "DANGER! SHIP BADLY DAMAGED!"
            }
            let mut skiffs = gm.g8(g::MONSTER_ANIM_STATE.wrapping_add(so).wrapping_add(1)) as u16; // value3
            if (gm.g8(g::TRANSPORT_TILE) & 0xfe) == 0x14 {
                let c = gm.g8(g::MAGIC_CARPETS).wrapping_add(1);
                gm.set_g8(g::MAGIC_CARPETS, c);
            }
            if (gm.g8(g::TRANSPORT_TILE) & 0xfc) == 0x28 {
                skiffs = skiffs.wrapping_add(1);
            }
            gm.set_g8(0x5c5f, hull); // slot 0 value1 = hull
            if skiffs == 0 {
                text::print_string(gm, 0x42a0); // "WARNING! NO SKIFFS ON BOARD!"
            }
            gm.set_g8(0x5c61, skiffs as u8); // slot 0 value3 = skiffs
            gm.set_g8(g::STATUS_DIRTY, 1);
            gm.set_g8(g::TRANSPORT_TILE, t);
        }
        rest::set_monster_slot(gm, slot, 0, 0, 0, 0, 0, 0);
        let d = gm.g8(g::VIEW_DIRTY) | 2;
        gm.set_g8(g::VIEW_DIRTY, d);
    } else {
        text::print_string(gm, 0x4252); // "Not here!"
    }
    1
}

/// orig 1000:c8e2 (391 bytes). Fire the frigate's cannons outdoors. Needs a frigate transport
/// (0x20..0x27, else 0x42c6); asks a direction; the shot must be a broadside (dx != 0 for an
/// east/west facing ship (tile bit 0 clear), dy != 0 for north/south) else "Fire broadsides
/// only!". Tone, then up to three cells in that direction are probed with find_object_at; the
/// first moving object (MAINOUT 922c) that is not a 0xec-group tile is the target: COMSUBS
/// f4be(1, ty-py+5, tx-px+5, 5, 5) animates the ball (0 = aborted), the view is refreshed, the
/// target flashes and its value1 (hull/hp) loses rand 1..20; when it goes below 0 (>= 0x80) the
/// slot is cleared and g_view_dirty |= 2. Without a target the ball flies 3 cells
/// (f4be(1, 3dy+5, 3dx+5, 5, 5)).
pub fn fire_ship_cannon(gm: &mut Game) {
    let tt = gm.g8(g::TRANSPORT_TILE);
    if tt < 0x20 || 0x27 < tt {
        text::print_string(gm, 0x42c6);
        return;
    }
    let r = rest::get_direction(gm);
    let dy = gm.gi16(g::DY);
    let dx = gm.gi16(g::DX);
    if r == 0 { return; }
    if (dx != 0 || (tt & 1) != 0) && (dx == 0 || (tt & 1) == 0) {
        let mut x = gm.g8(g::PARTY_X) as u16;
        let mut y = gm.g8(g::PARTY_Y) as u16;
        rest::play_tone_sweep(gm, 300, 5, 200, 1000);
        let mut n: i16 = 0;
        let slot: i16;
        loop {
            x = x.wrapping_add(dx as u16);
            y = y.wrapping_add(dy as u16);
            let pz = gm.g8(g::PARTY_Z) as u16;
            let t = rest::find_object_at(gm, pz, y, x);
            let moving = ovl::mainout::is_active_monster_tile(gm, t as i16);
            let s = gm.gi16(g::DX);
            if moving != 0 && (t & 0xfc) != 0xec {
                slot = s;
                break;
            }
            n += 1;
            if 2 < n {
                ovl::comsubs::animate_projectile(gm, 1, dy.wrapping_mul(3).wrapping_add(5), dx.wrapping_mul(3).wrapping_add(5), 5, 5);
                return;
            }
        }
        let so = slot8(slot as u16);
        let ty = (gm.g8(g::MONSTER_Y.wrapping_add(so)) as i16).wrapping_sub(gm.g8(g::PARTY_Y) as i16).wrapping_add(5);
        let tx = (gm.g8(g::MONSTER_X.wrapping_add(so)) as i16).wrapping_sub(gm.g8(g::PARTY_X) as i16).wrapping_add(5);
        if ovl::comsubs::animate_projectile(gm, 1, ty, tx, 5, 5) == 0 {
            return;
        }
        rest::refresh_map_view(gm);
        rest::flash_tile_at(gm, y as i16, x as i16);
        let a = g::MONSTER_Z.wrapping_add(so).wrapping_add(1); // value1
        let dmg = util::rand_range(gm, 0x14, 1) as u8;
        let v = gm.g8(a).wrapping_sub(dmg);
        gm.set_g8(a, v);
        if gm.g8(a) < 0x80 {
            return;
        }
        rest::set_monster_slot(gm, slot, 0, 0, 0, 0, 0, 0);
        let d = gm.g8(g::VIEW_DIRTY) | 2;
        gm.set_g8(g::VIEW_DIRTY, d);
        return;
    }
    text::print_string(gm, 0x42cd); // "Fire broadsides only!"
}

/// orig 1000:ca6a (678 bytes). The Fire command (dispatch 'F', vector 1778:08c6). Dungeon:
/// 0x42e4. Overworld: `fire_ship_cannon`. Settlement: the timed door is put back, then a cannon
/// (tile group 0xb4) adjacent to the party is looked for in the view tiles N (0xab87), E
/// (0xaba8), S (0xabc7), W (0xaba6) - none: 0x42eb. The cannon's facing (tile & 3: 0 N, 1 E, 2 S,
/// 3 W) gives the shot direction; 0x42f2 is printed, a tone plays and the town is alarmed (TOWN
/// 8b28). Up to four cells from the cannon: an empty cell whose map tile is a door (0x97..0x99,
/// 0xb8..0xbb) stops the ball ("Door destroyed!", tile 0x44, g_view_dirty = 1, g_door_tile =
/// 0); an object with tile > 0x1b that is not 0x78-group or 0x2f-group, or a horse (0x10/0x11),
/// is hit (slot in g_dx). COMSUBS f4be animates the ball to the final cell; a hit flashes the
/// cell; a hit NPC/monster is removed (slot cleared, karma -5 min 0, NPC killed+removed); a hit
/// on slot 0 (the party itself) -> damage_whole_party.
pub fn cmd_fire(gm: &mut Game) {
    let loc = gm.g8(g::LOCATION);
    if 0x20 < loc && loc < 0x29 {
        text::print_string(gm, 0x42e4);
        return;
    }
    if loc == 0 {
        fire_ship_cannon(gm);
        return;
    }
    let (dy_, dx_, dt_) = (gm.g8(g::DOOR_Y), gm.g8(g::DOOR_X), gm.gi8(g::DOOR_TILE));
    rest::set_town_map_tile(gm, dy_, dx_, dt_);
    let mut sx: i16;
    let mut sy: i16 = 0;
    let cannon: u8;
    if (gm.g8(0xab87) & 0xfc) == 0xb4 { // view tile north of the party
        sx = 0;
        sy = -1;
        cannon = gm.g8(0xab87);
    } else if (gm.g8(0xaba8) & 0xfc) == 0xb4 { // east
        sx = 1;
        cannon = gm.g8(0xaba8);
    } else if (gm.g8(0xabc7) & 0xfc) == 0xb4 { // south
        sx = 0;
        sy = 1;
        cannon = gm.g8(0xabc7);
    } else if (gm.g8(0xaba6) & 0xfc) == 0xb4 { // west
        sx = -1;
        cannon = gm.g8(0xaba6);
    } else {
        text::print_string(gm, 0x42eb);
        return;
    }
    // LAB_1000_cb0a
    let mut cx = sx.wrapping_add(gm.g8(g::PARTY_X) as i16);
    let mut cy = sy.wrapping_add(gm.g8(g::PARTY_Y) as i16);
    let vx0 = sx.wrapping_add(5);
    let vy0 = sy.wrapping_add(5);
    let facing = cannon & 3;
    let (mut ddx, mut ddy): (i16, i16) = (sx, sy);
    if facing == 0 {
        ddx = 0;
        ddy = -1;
    } else if facing == 1 {
        ddx = 1;
        ddy = 0;
    } else if facing == 2 {
        ddx = 0;
        ddy = 1;
    } else if facing == 3 {
        ddx = -1;
        ddy = 0;
    }
    // LAB_1000_cb6e
    text::print_string(gm, 0x42f2);
    rest::play_tone_sweep(gm, 300, 5, 200, 1000);
    ovl::town::raise_alarm(gm);
    let mut hit_door = false;
    let mut hit_obj = false;
    let mut steps: i16 = 5;
    let mut vy = vy0;
    let mut vx = vx0;
    let mut hit_slot: i16 = 0;
    loop {
        if hit_door || hit_obj { break; }
        steps -= 1;
        if !(0 < steps) { break; }
        cx = cx.wrapping_add(ddx);
        cy = cy.wrapping_add(ddy);
        vx = vx.wrapping_add(ddx);
        vy = vy.wrapping_add(ddy);
        let pz = gm.g8(g::PARTY_Z) as u16;
        let t = rest::find_object_at_rev(gm, pz, cy as u16, cx as u16);
        if t == 0 {
            let p = rest::get_map_tile(gm, cy as u16, cx as u16);
            let mt = gm.g8(p);
            if 0x96 < mt && (mt < 0x9a || (0xb7 < mt && mt < 0xbc)) {
                hit_door = true;
            }
        } else if ((0x1b < t && (t & 0xf8) != 0x78) && (t & 0xfc) != 0x2f) || (t == 0x10 || t == 0x11) {
            hit_obj = true;
            hit_slot = gm.gi16(g::DX);
        }
    }
    ovl::comsubs::animate_projectile(gm, 1, vy, vx, vy0, vx0);
    if hit_door || hit_obj {
        rest::flash_tile_at(gm, cy, cx);
    }
    if hit_door {
        text::print_string(gm, 0x42fa); // "Door destroyed!"
        let p = rest::get_map_tile(gm, cy as u16, cx as u16);
        gm.set_g8(p, 0x44);
        gm.set_g8(g::VIEW_DIRTY, 1);
        gm.set_g8(g::DOOR_TILE, 0);
    }
    if hit_obj && hit_slot != 0 {
        rest::set_monster_slot(gm, hit_slot, 0, 0, 0, 0, 0, 0);
        let d = gm.g8(g::VIEW_DIRTY) | 2;
        gm.set_g8(g::VIEW_DIRTY, d);
        if gm.g8(g::KARMA) < 6 {
            gm.set_g8(g::KARMA, 0);
        } else {
            let k = gm.g8(g::KARMA) - 5;
            gm.set_g8(g::KARMA, k);
        }
        let npc = ovl::town::find_npc_by_slot(gm, hit_slot);
        if npc == -1 { return; }
        ovl::town::set_npc_killed(gm, npc);
        ovl::town::remove_npc(gm, npc);
    }
    if hit_obj && hit_slot == 0 {
        rest::damage_whole_party(gm);
    }
}

/// orig 1000:cd18 (67 bytes). Ignite torch (dispatch 'I', vector 1778:08d2). "None owned!"
/// without torches; else one torch is used: outside dungeons g_torch_turns = 0xf0, in a dungeon
/// g_torch_turns += 0x70 + rand 0..15 (capped at 0xff).
pub fn cmd_ignite_torch(gm: &mut Game) {
    if gm.g8(g::TORCHES) == 0 {
        text::print_string(gm, 0x430b); // "None owned!"
    } else {
        let t = gm.g8(g::TORCHES).wrapping_sub(1);
        gm.set_g8(g::TORCHES, t);
        let loc = gm.g8(g::LOCATION);
        if loc < 0x21 || 0x28 < loc {
            gm.set_g8(g::TORCH_TURNS, 0xf0);
        } else {
            let r = util::rand_range(gm, 0xf, 0);
            rest::add_u8_capped(gm, 0xff, r.wrapping_add(0x70), g::TORCH_TURNS);
        }
    }
}

/// orig 1000:cd5c (212 bytes). New Order (dispatch 'N', vector 1778:08de). Prints 0x4318 and
/// asks two members with select_party_member_no_zero (-1 -> 0x4320 / 0x4340); the first name is
/// echoed, "with" the second; choosing the leader (index 0) prints 0x4329/0x4349 + the leader's
/// name + "must lead!"; otherwise the two 32-byte party records are swapped through a local
/// buffer (0x4359 printed) and g_status_dirty = 1.
pub fn cmd_new_order(gm: &mut Game) {
    text::print_string(gm, 0x4318);
    let a = rest::select_party_member_no_zero(gm);
    let s: u16;
    if a == -1 {
        s = 0x4320;
    } else {
        text::print_string(gm, g::PARTY.wrapping_add(mem32(a as u16)));
        if a == 0 {
            text::print_string(gm, 0x4329);
            text::print_string(gm, g::PARTY);
            s = 0x432c; // "must lead!"
        } else {
            text::print_string(gm, 0x4339); // "with"
            let b = rest::select_party_member_no_zero(gm);
            if b == -1 {
                s = 0x4340;
            } else {
                text::print_string(gm, g::PARTY.wrapping_add(mem32(b as u16)));
                if b != 0 {
                    text::print_string(gm, 0x4359);
                    let pa = gm.ds(g::PARTY.wrapping_add(mem32(a as u16)));
                    let pb = gm.ds(g::PARTY.wrapping_add(mem32(b as u16)));
                    let tmp = gm.m.slice(pa, 0x20).to_vec();
                    gm.m.copy(pa, pb, 0x20);
                    gm.m.write(pb, &tmp);
                    gm.set_g8(g::STATUS_DIRTY, 1);
                    return;
                }
                text::print_string(gm, 0x4349);
                text::print_string(gm, g::PARTY);
                s = 0x434c; // "must lead!"
            }
        }
    }
    text::print_string(gm, s);
}

/// orig 1000:ce34 (371 bytes). The Xit command (dispatch 'X', vector 1778:08ea). (The first
/// test `loc < 0x20 && 0x29 < loc` can never be true.) By transport group: horse (0x10): 0x43a4,
/// the horse object (tile - 2) is left on the party cell, transport 0x1c; carpet (0x14): with
/// land nearby or a walkable tile underfoot "carpet!" leaves the carpet object (0x1b), else "No
/// land nearby!" / "Not here!"; on foot (0x1c): 0x4368; under sail (0x20): "Under sail!";
/// frigate (0x24): 0x43d2, with land nearby the party steps off (transport 0x1c), else without
/// skiffs a carpet is used (g_magic_carpets--, transport 0x14) or "No skiffs on board!", else
/// transport = tile + 4 (skiff) and the ship keeps skiffs-1; skiff (0x28): land nearby required,
/// not on 0x6a/0x6b tiles ("Not here!"), 0x43ca. Leaving a vehicle creates its object with
/// set_monster_slot(find_spawn_slot(), slot-0 value1 (hull), z, y, x, tile, tile) and value3 =
/// remaining skiffs, then redraws the status window.
pub fn cmd_xit(gm: &mut Game) {
    let loc = gm.g8(g::LOCATION);
    if loc < 0x20 && 0x29 < loc {
        text::print_string(gm, 0x435c); // "Not here!" (unreachable)
        return;
    }
    let p = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
    let under = gm.g8(p);
    let grp = gm.g8(g::TRANSPORT_TILE) & 0xfc;
    let obj_tile: u8;
    let mut skiffs: u8 = 0; // TODO(verify): uninitialised in the original for horse/carpet/skiff
    let s: u16;
    'spawn: {
        if grp == 0x10 {
            text::print_string(gm, 0x43a4);
            obj_tile = gm.g8(g::TRANSPORT_TILE).wrapping_sub(2);
            gm.set_g8(g::TRANSPORT_TILE, 0x1c);
            break 'spawn;
        } else if grp == 0x14 {
            if land_nearby(gm) != 0 || rest::can_enter_tile(gm, under as i16, 0x1c) != 0 {
                text::print_string(gm, 0x437d); // "carpet!"
                obj_tile = 0x1b;
                gm.set_g8(g::TRANSPORT_TILE, 0x1c);
                break 'spawn;
            }
            s = if land_nearby(gm) == 0 { 0x4386 } else { 0x4398 }; // "No land nearby!" / "Not here!"
        } else if grp == 0x1c {
            s = 0x4368;
        } else if grp != 0x20 {
            if grp == 0x24 {
                text::print_string(gm, 0x43d2);
                let land = land_nearby(gm);
                obj_tile = gm.g8(g::TRANSPORT_TILE);
                skiffs = gm.g8(0x5c61); // slot 0 value3
                if land == 0 {
                    if gm.g8(0x5c61) == 0 {
                        if gm.g8(g::MAGIC_CARPETS) == 0 {
                            text::print_string(gm, 0x43d9); // "No skiffs on board!"
                            return;
                        }
                        let c = gm.g8(g::MAGIC_CARPETS).wrapping_sub(1);
                        gm.set_g8(g::MAGIC_CARPETS, c);
                        gm.set_g8(g::TRANSPORT_TILE, 0x14);
                    } else {
                        let t = gm.g8(g::TRANSPORT_TILE).wrapping_add(4);
                        gm.set_g8(g::TRANSPORT_TILE, t);
                        skiffs = gm.g8(0x5c61).wrapping_sub(1);
                    }
                } else {
                    gm.set_g8(g::TRANSPORT_TILE, 0x1c);
                }
                break 'spawn;
            } else if grp == 0x28 {
                if land_nearby(gm) == 0 {
                    s = 0x43ac; // "No land nearby!"
                } else {
                    if (under & 0xfe) != 0x6a {
                        text::print_string(gm, 0x43ca);
                        obj_tile = gm.g8(g::TRANSPORT_TILE);
                        gm.set_g8(g::TRANSPORT_TILE, 0x1c);
                        break 'spawn;
                    }
                    s = 0x43be; // "Not here!"
                }
            } else {
                // other transport groups: fall straight into the spawn with garbage - keep literal
                obj_tile = 0; // TODO(verify): uninitialised local_4 in the original
                break 'spawn;
            }
        } else {
            s = 0x436f; // "Under sail!"
        }
        text::print_string(gm, s);
        return;
    }
    // LAB_1000_cf74
    let slot = rest::find_spawn_slot(gm);
    let hull = gm.g8(0x5c5f); // slot 0 value1
    let (pz, py, px) = (gm.g8(g::PARTY_Z), gm.g8(g::PARTY_Y), gm.g8(g::PARTY_X));
    rest::set_monster_slot(gm, slot, hull, pz, py, px, obj_tile, obj_tile);
    gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(slot8(slot as u16)).wrapping_add(1), skiffs); // value3
    rest::draw_status_window(gm);
}

/// orig 1000:cfb0 (461 bytes). Yelling a Shadowlord's name in a settlement (buffer = the
/// typed text). Only in locations 0x1e..0x20 (Stonegate? / the three shrines of the
/// shards - the towns where the Shadowlords can be summoned); the text must contain one of the
/// three names at 0x444a[0..2]; the party must be at y > 1 and the Shadowlord not destroyed
/// (g_shadowlord_loc[i] != 0xff), else "No effect!" (0x440b / 0x443c). A Shadowlord already in
/// the monster table (tile 0xfc) -> "No effect!" (0x4418). Otherwise g_save_unk_325 = i, a slot
/// is spawned 2 cells north of the party with tile 0xfc, the highest free NPC (g_npc_types == 0,
/// searched from 31 down) gets a state record (active, slot, x, y-2, z), schedule words
/// 0x5d6a/0x5d6c cleared, three schedule entries with AI type 6 at (x, y-2, z) and type 0xfc;
/// "A shadowlord appears", a noise sweep, and the object fades in (tile 0x16 ->
/// tile_transition_effect(3,5,0x1fc) -> 0xfc). Returns 0 after a summoning, otherwise whatever
/// print_string left in AX (treated as 1).
pub fn yell_shadowlord_name(gm: &mut Game, param_1: u16) -> u16 {
    let buf = param_1;
    let loc = gm.g8(g::LOCATION);
    let s: u16;
    if loc == 0x1e || loc == 0x1f || loc == 0x20 {
        let mut i: i16 = 0;
        loop {
            let name = gm.g16(0x444au16.wrapping_add((i as u16).wrapping_mul(2)));
            if (rest::str_find_ci(gm, buf as i16, name) as i16) > -1 { break; }
            i += 1;
            if !(i < 3) { break; }
        }
        if i != 3 && 1 < gm.g8(g::PARTY_Y) && gm.gi8(g::SHADOWLORD_LOC.wrapping_add(i as u16)) != -1 {
            let mut k: i16 = 0;
            loop {
                if gm.gi8(g::MONSTER_TABLE.wrapping_add(slot8(k as u16))) == -4 {
                    text::print_string(gm, 0x4418); // "No effect!"
                    return 1; // TODO(verify): original returns print_string's AX
                }
                k += 1;
                if !(k < 0x20) { break; }
            }
            gm.set_g8(g::SAVE_UNK_325, i as u8);
            let slot = rest::find_spawn_slot(gm);
            let (pz, py, px) = (gm.g8(g::PARTY_Z), gm.g8(g::PARTY_Y), gm.g8(g::PARTY_X));
            rest::set_monster_slot(gm, slot, 0, pz, py.wrapping_sub(2), px, 0xfc, 0xfc);
            let mut npc: i16 = 0x1f;
            let n: i16;
            loop {
                let cur = npc;
                if gm.g8(g::NPC_TYPES.wrapping_add(cur as u16)) == 0 { n = cur; break; }
                npc = cur - 1;
                if !(-1 < npc) { n = 0x1f; break; }
            }
            let n16 = (n as u16).wrapping_mul(0x10);
            gm.set_g16(g::NPC_STATES.wrapping_add(n16), 1);
            gm.set_gi16(0x5f6au16.wrapping_add(n16), slot); // g_npc_slot
            gm.set_g16(0x5f60u16.wrapping_add(n16), px as u16); // g_npc_x
            gm.set_gi16(0x5f62u16.wrapping_add(n16), (py as i16).wrapping_sub(2)); // g_npc_y
            gm.set_g16(0x5f64u16.wrapping_add(n16), pz as u16); // g_npc_z
            gm.set_g16(0x5d6au16.wrapping_add(n16), 0); // schedule +0xc
            gm.set_g16(0x5d6cu16.wrapping_add(n16), 0); // schedule +0xe
            let ny = py.wrapping_sub(2);
            let mut j: u16 = 0;
            loop {
                let b = n16.wrapping_add(j);
                gm.set_g8(0x5d5eu16.wrapping_add(b), 6); // AI type
                gm.set_g8(0x5d61u16.wrapping_add(b), px); // x
                gm.set_g8(0x5d64u16.wrapping_add(b), ny); // y
                gm.set_g8(0x5d67u16.wrapping_add(b), pz); // z
                j += 1;
                if !(j < 3) { break; }
            }
            gm.set_g8(g::NPC_TYPES.wrapping_add(n as u16), 0xfc);
            text::print_string(gm, 0x4425); // "A shadowlord appears"
            util::play_sweep_noise(gm, 2, 2000, 0x7530, 1, 0x28a0);
            let so = slot8(slot as u16);
            gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(so), 0x16); // anim tile
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), 0x16);
            gfx::tile_transition_effect(gm, 3, 5, 0x1fc);
            gm.set_g8(g::PARTY_VEHICLE_TILE.wrapping_add(so), 0xfc);
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), 0xfc);
            return 0;
        }
        s = 0x440b; // "No effect!"
    } else {
        s = 0x443c; // "No effect!"
    }
    text::print_string(gm, s);
    1 // TODO(verify): original returns print_string's AX
}

/// orig 1000:d182 (196 bytes). Meditation at a shrine cell (y,x) of virtue `virtue` (Ghidra
/// order (y, x, virtue)). "Upon what virtue dost thou meditate?" reads up to 15 characters which
/// must contain the virtue name (0x1f4e[virtue]); then three "Mantra:" prompts each of which
/// must contain the mantra (0x1f5e[virtue]). All correct and the cell is the shrine's position
/// (g_shrine_x/y 0x1f6e/0x1f76): the shrine's destroyed flag (g_shrine_destroyed[virtue] &=
/// 0x7f) is cleared, "The Shrine is restored!", earthquake_shake, the tile becomes 0x19 and
/// g_view_dirty |= 2; otherwise just a newline.
pub fn meditate_at_shrine(gm: &mut Game, param_1: u16, param_2: u16, param_3: i16) {
    let (y, x, virtue) = (param_1, param_2, param_3);
    gm.with_buf(16, |gm, buf| {
        text::print_string(gm, 0x4450); // "Upon what virtue dost thou meditate?" (0x444f + 1)
        rest::read_text_line(gm, 0xf, buf);
        let vname = gm.g16(g::VIRTUE_MANTRA_PTRS.wrapping_add((virtue as u16).wrapping_mul(2)));
        let mut ok = rest::str_find_ci(gm, buf as i16, vname) != 0xffff;
        let mut n: i16 = 3;
        loop {
            text::print_string(gm, 0x4479); // "Mantra:"
            rest::read_text_line(gm, 0xf, buf);
            let mantra = gm.g16(0x1f5eu16.wrapping_add((virtue as u16).wrapping_mul(2)));
            if rest::str_find_ci(gm, buf as i16, mantra) == 0xffff {
                ok = false;
            }
            n -= 1;
            if n == 0 { break; }
        }
        if ok && gm.g8(g::SHRINE_X.wrapping_add(virtue as u16)) as u16 == x && gm.g8(g::SHRINE_Y.wrapping_add(virtue as u16)) as u16 == y {
            let a = g::SHRINE_DESTROYED.wrapping_add(virtue as u16);
            let v = gm.g8(a) & 0x7f;
            gm.set_g8(a, v);
            text::print_string(gm, 0x4482); // "The Shrine is restored!"
            rest::earthquake_shake(gm);
            let p = rest::get_map_tile(gm, y, x);
            gm.set_g8(p, 0x19);
            let d = gm.g8(g::VIEW_DIRTY) | 2;
            gm.set_g8(g::VIEW_DIRTY, d);
        } else {
            text::put_char(gm, 10);
        }
    });
}

/// orig 1000:d248 (332 bytes). Yelling a word of power outdoors. For each of the 8 dungeons
/// (word pointers g_word_of_power_ptrs 0x4502) whose word appears in the text: "A word of
/// power is uttered" + earthquake; the adjacent view tiles W (0xaba6), S (0xabc7), E (0xaba8), N
/// (0xab87) are checked in that order for the dungeon's open tile (g_dungeon_open_tiles[i]),
/// 0xdf (sealed entrance) or 0x1a (shrine); a shrine tile -> `meditate_at_shrine(y, x, i)`;
/// otherwise when that cell is the dungeon's entrance (g_location_x/y[0x20+i], 0x1eaa/0x1ed2)
/// g_dungeon_open[i] ^= 0x80 and the map tile ^= open_tile ^ 0xdf (toggles sealed/open),
/// g_view_dirty |= 2. No adjacent match ends the search silently; no word matched -> "No
/// effect!".
pub fn yell_word_of_power(gm: &mut Game, param_1: u16) {
    let buf = param_1;
    let mut uttered = false;
    let mut i: i16 = 0;
    loop {
        let word = gm.g16(g::WORD_OF_POWER_PTRS.wrapping_add((i as u16).wrapping_mul(2)));
        if rest::str_find_ci(gm, buf as i16, word) < 0x8000 {
            text::print_string(gm, 0x44d7); // "A word of power is uttered"
            rest::earthquake_shake(gm);
            let open_tile = gm.gi8(g::DUNGEON_OPEN_TILES.wrapping_add(i as u16));
            let (mut tile, dx, dy): (i8, i16, i16);
            let w = gm.gi8(0xaba6);
            tile = w;
            if open_tile == w || w == -0x21 || w == 0x1a {
                dx = -1;
                dy = 0;
            } else {
                let s_ = gm.gi8(0xabc7);
                tile = s_;
                if open_tile == s_ || s_ == -0x21 || s_ == 0x1a {
                    dx = 0;
                    dy = 1;
                } else {
                    let e = gm.gi8(0xaba8);
                    tile = e;
                    if open_tile == e || e == -0x21 || e == 0x1a {
                        dx = 1;
                        dy = 0;
                    } else {
                        let n = gm.gi8(0xab87);
                        tile = n;
                        if open_tile != n && n != -0x21 && n != 0x1a {
                            break;
                        }
                        dx = 0;
                        dy = -1;
                    }
                }
            }
            let ty = dy.wrapping_add(gm.g8(g::PARTY_Y) as i16) as u16;
            let tx = dx.wrapping_add(gm.g8(g::PARTY_X) as i16) as u16;
            if tile == 0x1a {
                meditate_at_shrine(gm, ty, tx, i);
                return;
            }
            if tx == gm.g8(g::LOCATION_X.wrapping_add(0x20).wrapping_add(i as u16)) as u16
                && ty == gm.g8(g::LOCATION_Y.wrapping_add(0x20).wrapping_add(i as u16)) as u16
            {
                let a = g::DUNGEON_OPEN.wrapping_add(i as u16);
                let v = gm.g8(a) ^ 0x80;
                gm.set_g8(a, v);
                let p = rest::get_map_tile(gm, ty, tx);
                let nv = gm.g8(p) ^ (open_tile as u8) ^ 0xdf;
                gm.set_g8(p, nv);
                let d = gm.g8(g::VIEW_DIRTY) | 2;
                gm.set_g8(g::VIEW_DIRTY, d);
            }
            uttered = true;
            break;
        }
        i += 1;
        if !(i < 8) { break; }
    }
    if !uttered {
        text::print_string(gm, 0x44f4); // "No effect!"
    }
}

/// orig 1000:d398 (160 bytes). The Yell command (dispatch 'Y', vector 1778:05ea). On a frigate
/// outside combat the sails are toggled: 0x20 group -> 0x451a (hoist, transport += 4), 0x24
/// group -> 0x4521 (furl, transport -= 4). Otherwise 0x4529 prompts for up to 30 characters;
/// empty -> "Nothing"; else newline and: overworld `yell_word_of_power`, dungeon "No effect!",
/// settlement `yell_shadowlord_name`. Returns 1 (a turn) except what the sub-commands return.
pub fn cmd_yell(gm: &mut Game) -> u16 {
    let mut result: u16 = 1;
    let tt = gm.g8(g::TRANSPORT_TILE);
    if (tt & 0xf8) == 0x20 && gm.g8(g::LOCATION) < 0x80 {
        if (tt & 0xfc) == 0x20 {
            text::print_string(gm, 0x451a);
            result = 1; // TODO(verify): original returns print_string's AX
            gm.set_g8(g::TRANSPORT_TILE, tt.wrapping_add(4));
        } else {
            text::print_string(gm, 0x4521);
            result = 1; // TODO(verify): original returns print_string's AX
            gm.set_g8(g::TRANSPORT_TILE, tt.wrapping_sub(4));
        }
    } else {
        text::print_string(gm, 0x4529);
        result = gm.with_buf(32, |gm, buf| {
            rest::read_text_line(gm, 0x1e, buf);
            if gm.g8(buf) == 0 {
                text::print_string(gm, 0x4531); // "Nothing"
                return 1; // TODO(verify): original returns print_string's AX
            }
            text::put_char(gm, 10);
            let loc = gm.g8(g::LOCATION);
            if loc == 0 || 0x20 < loc {
                if loc == 0 {
                    yell_word_of_power(gm, buf);
                } else {
                    text::print_string(gm, 0x453a); // "No effect!"
                }
                1
            } else {
                yell_shadowlord_name(gm, buf)
            }
        });
    }
    result
}

/// orig 1000:d43a (74 bytes). Returns 1 when map tile `tile` can be pushed/pulled: 0x5b,
/// 0x90..0x93, 0xa5, 0xa6, 0xa8, 0xa9, 0xad..0xaf, 0xb4..0xb7; else 0.
pub fn is_pushable_tile(gm: &mut Game, param_1: u8) -> u16 {
    let t = param_1;
    if t < 0xa7 {
        if t < 0xa5 && t != 0x5b && (t < 0x90 || 0x93 < t) {
            return 0;
        }
    } else {
        if t < 0xa8 { return 0; }
        if 0xa9 < t {
            if t < 0xad { return 0; }
            if 0xaf < t {
                if t < 0xb4 { return 0; }
                if 0xb7 < t { return 0; }
            }
        }
    }
    1
}

/// orig 1000:d484 (67 bytes). Orientation of a directional tile group (0x90 cannons / 0xb4
/// town cannons) after a push: base + 1 for east (dx 1), + 2 for south (dy 1), + 3 for west (dx
/// -1), + 0 for north; `pull` != 0 flips the facing (^ 2). Ghidra order (pull, base, dy, dx).
pub fn orient_pushed_tile(gm: &mut Game, param_1: i16, param_2: u16, param_3: i16, param_4: i16) -> u16 {
    let (pull, mut t, dy, dx) = (param_1, param_2, param_3, param_4);
    if dx == 1 && dy == 0 { t = t.wrapping_add(1); }
    if dx == 0 && dy == 1 { t = t.wrapping_add(2); }
    if dx == -1 && dy == 0 { t = t.wrapping_add(3); }
    if pull != 0 { t ^= 2; }
    t
}

/// orig 1000:d4c8 (104 bytes). "Pushed!": the object tile moves from (y,x) to (ty,tx) and the
/// far tile takes its place; directional groups (0x90/0xb4) are re-oriented by
/// `orient_pushed_tile(0, ...)`. Ghidra order (dy, dx, ty, tx, y, x, far_tile, tile).
pub fn push_tile(gm: &mut Game, param_1: i16, param_2: i16, param_3: u16, param_4: u16, param_5: u16, param_6: u16, param_7: u8, param_8: u8) {
    let (dy, dx, ty, tx, y, x, far_tile, mut tile) = (param_1, param_2, param_3, param_4, param_5, param_6, param_7, param_8);
    text::print_string(gm, 0x4547); // "Pushed!"
    let p = rest::get_map_tile(gm, ty, tx);
    gm.set_g8(p, tile);
    let q = rest::get_map_tile(gm, y, x);
    gm.set_g8(q, far_tile);
    tile &= 0xfc;
    if tile == 0x90 || tile == 0xb4 {
        let v = orient_pushed_tile(gm, 0, tile as u16, dy, dx);
        let p = rest::get_map_tile(gm, ty, tx);
        gm.set_g8(p, v as u8);
    }
}

/// orig 1000:d530 (105 bytes). "Pulled!": the object tile moves from (y,x) onto the party cell
/// (py,px) and the tile under the party replaces it; directional groups re-oriented with
/// `orient_pushed_tile(1, ...)`. Ghidra order (dy, dx, y, x, py, px, tile, under).
pub fn pull_tile(gm: &mut Game, param_1: i16, param_2: i16, param_3: u16, param_4: u16, param_5: u16, param_6: u16, param_7: u8, param_8: u8) {
    let (dy, dx, y, x, py, px, mut tile, under) = (param_1, param_2, param_3, param_4, param_5, param_6, param_7, param_8);
    text::print_string(gm, 0x4550); // "Pulled!"
    let p = rest::get_map_tile(gm, py, px);
    gm.set_g8(p, tile);
    let q = rest::get_map_tile(gm, y, x);
    gm.set_g8(q, under);
    tile &= 0xfc;
    if tile == 0x90 || tile == 0xb4 {
        let v = orient_pushed_tile(gm, 1, tile as u16, dy, dx);
        let p = rest::get_map_tile(gm, py, px);
        gm.set_g8(p, v as u8);
    }
}

/// orig 1000:d59a (464 bytes). The Push command (dispatch 'P', vector 1778:058a; also COMBAT
/// a8ce). Restores the timed door, asks a direction. In combat the active combatant's roster
/// position temporarily replaces g_party_x/y and COMBAT b3aa(y,x) != 0 aborts. The target tile
/// must hold no object and be `is_pushable_tile`; the floor it needs is 'E' (0x45) for the 0xb4
/// group, else 'D' (0x44). If the cell beyond is free and has that floor -> `push_tile`; else
/// if the party stands on that floor -> `pull_tile`; else "Won't budge" (0x4567). The party then
/// steps into the vacated cell (g_view_dirty = 1); in combat the roster entry and its monster
/// slot move instead, g_party_x/y are restored and the view refreshed. Non-pushable: "Won't
/// budge!" (0x4559).
pub fn cmd_push(gm: &mut Game) {
    let (dy_, dx_, dt_) = (gm.g8(g::DOOR_Y), gm.g8(g::DOOR_X), gm.gi8(g::DOOR_TILE));
    rest::set_town_map_tile(gm, dy_, dx_, dt_);
    let r = rest::get_direction(gm);
    let dy = gm.gi16(g::DY);
    let dx = gm.gi16(g::DX);
    if r == 0 { return; }
    let mut px = gm.g8(g::PARTY_X);
    let mut py = gm.g8(g::PARTY_Y);
    let mut saved_x: u8 = 0;
    let mut saved_y: u8 = 0;
    let loc = gm.g8(g::LOCATION);
    if 0x7f < loc {
        let ro = (gm.g8(g::ACTIVE_COMBATANT) as u16).wrapping_mul(8);
        px = gm.g8(g::ROSTER_X.wrapping_add(ro));
        py = gm.g8(g::ROSTER_Y.wrapping_add(ro));
        saved_x = gm.g8(g::PARTY_X);
        saved_y = gm.g8(g::PARTY_Y);
    }
    gm.set_g8(g::PARTY_Y, py);
    gm.set_g8(g::PARTY_X, px);
    let x = dx.wrapping_add(gm.g8(g::PARTY_X) as i16);
    let y = dy.wrapping_add(gm.g8(g::PARTY_Y) as i16);
    if 0x7f < loc && ovl::combat::check_room_trigger(gm, y as u16, x as u16) != 0 {
        return;
    }
    let p = rest::get_map_tile(gm, y as u16, x as u16);
    let tile = gm.g8(p);
    let pz = gm.g8(g::PARTY_Z) as u16;
    if rest::find_object_at(gm, pz, y as u16, x as u16) == 0 && is_pushable_tile(gm, tile) != 0 {
        let floor: u8 = if (tile & 0xfc) == 0xb4 { 0x45 } else { 0x44 };
        let fx = x.wrapping_add(dx);
        let fy = y.wrapping_add(dy);
        let q = rest::get_map_tile(gm, fy as u16, fx as u16);
        let far = gm.g8(q);
        let u = rest::get_map_tile(gm, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
        let under = gm.g8(u);
        if rest::find_object_at(gm, pz, fy as u16, fx as u16) == 0 && far == floor {
            push_tile(gm, dy, dx, fy as u16, fx as u16, y as u16, x as u16, far, tile);
        } else {
            if under != floor {
                text::print_string(gm, 0x4567); // "Won't budge"
                return;
            }
            let (ppy, ppx) = (gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
            pull_tile(gm, dy, dx, y as u16, x as u16, ppy, ppx, tile, under);
        }
        let sdx = dx as i8;
        let sdy = dy as i8;
        let nx = gm.g8(g::PARTY_X).wrapping_add(sdx as u8);
        gm.set_g8(g::PARTY_X, nx);
        let ny = gm.g8(g::PARTY_Y).wrapping_add(sdy as u8);
        gm.set_g8(g::PARTY_Y, ny);
        gm.set_g8(g::VIEW_DIRTY, 1);
        if 0x7f < loc {
            let ro = (gm.g8(g::ACTIVE_COMBATANT) as u16).wrapping_mul(8);
            let slot = gm.g8(g::ROSTER_MEMBER.wrapping_add(ro)) as u16; // TODO(verify): the code indexes the monster table with roster +3 (member), not +4 (slot)
            let rx = gm.g8(g::ROSTER_X.wrapping_add(ro)).wrapping_add(sdx as u8);
            gm.set_g8(g::ROSTER_X.wrapping_add(ro), rx);
            let ry = gm.g8(g::ROSTER_Y.wrapping_add(ro)).wrapping_add(sdy as u8);
            gm.set_g8(g::ROSTER_Y.wrapping_add(ro), ry);
            let mx = gm.g8(g::MONSTER_X.wrapping_add(slot8(slot))).wrapping_add(sdx as u8);
            gm.set_g8(g::MONSTER_X.wrapping_add(slot8(slot)), mx);
            let my = gm.g8(g::MONSTER_Y.wrapping_add(slot8(slot))).wrapping_add(sdy as u8);
            gm.set_g8(g::MONSTER_Y.wrapping_add(slot8(slot)), my);
            gm.set_g8(g::PARTY_X, saved_x);
            gm.set_g8(g::PARTY_Y, saved_y);
            rest::refresh_map_view(gm);
        }
    } else {
        text::print_string(gm, 0x4559); // "Won't budge!"
    }
}

/// orig 1000:d76c (208 bytes). Combat "Escape" for the whole party (vector 1778:060e). If any
/// live party member (flags & 0xa0 == 0x80) remains: in a 0x80-flagged fight " Not here!", or
/// when enemies remain (g_no_more_enemies == 0) " Not yet!" - both return 1 (refused). Else "!"
/// and every roster entry with flags != 0 (COMBAT b4c6(-1 - i)) and every monster slot with a
/// tile (b4c6(i + 1)) is removed with view refreshes, a tone sweep plays and 0 is returned.
/// g_status_dirty = 1 either way.
pub fn combat_escape_all(gm: &mut Game) -> i16 {
    text::print_string(gm, 0x4574); // "Escape"
    let mut refused: i16 = 0;
    let mut alive = false;
    let mut i: i16 = 0;
    loop {
        if (gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(i))) & 0xa0) == 0x80 {
            alive = true;
            break;
        }
        i += 1;
        if !(i < 0x20) { break; }
    }
    if alive {
        let s: u16;
        let mut go = true;
        if (gm.g8(g::COMBAT_FLAGS) & 0x80) == 0 {
            if gm.g8(g::NO_MORE_ENEMIES) != 0 {
                go = false; // LAB_1000_d7cd
            }
            s = 0x4587; // " Not yet!"
        } else {
            s = 0x457b; // " Not here!"
        }
        if go {
            text::print_string(gm, s);
            refused = 1;
        }
    }
    // LAB_1000_d7cd
    if refused == 0 {
        text::put_char(gm, 0x21);
        let mut i: i16 = 0;
        loop {
            if gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(i))) != 0 {
                ovl::combat::remove_combatant(gm, (-1i16).wrapping_sub(i));
                rest::refresh_map_view(gm);
            }
            i += 1;
            if !(i < 0x20) { break; }
        }
        let mut i: i16 = 0;
        loop {
            if gm.g8(g::MONSTER_TABLE.wrapping_add(slot8(i as u16))) != 0 {
                ovl::combat::remove_combatant(gm, i.wrapping_add(1));
                rest::refresh_map_view(gm);
            }
            i += 1;
            if !(i < 0x20) { break; }
        }
        rest::play_tone_sweep(gm, 0x28, 1, 2000, 0x4b0);
    }
    gm.set_g8(g::STATUS_DIRTY, 1);
    refused
}

/// orig 1000:d83e (431 bytes). Reagent picker for Mix. Lists the owned reagents (index list of
/// up to 8) in the status window (window 1 re-defined to rows 9..0x26/0x27, "Reagents:" title,
/// "NN name" lines) with the first line highlighted. Keys from window 2: 2/4 (right/down) move
/// the highlight down, 1/3 up; Enter (0xd) or space toggles the reagent's bit (0x80 >> index)
/// and draws/erases the 0x0f marker at column 3 (between 0xfd attribute codes); Esc returns
/// 0xffff (newline printed); 'M' finishes with 0x8f6e. Returns the bit mask of selected
/// reagents (bit 7 = reagent 0).
pub fn select_reagents(gm: &mut Game) -> u16 {
    let mut list = [0u8; 8];
    let mut n: i16 = 0;
    let mut i: i16 = 0;
    loop {
        if gm.g8(g::REAGENTS.wrapping_add(i as u16)) != 0 {
            list[(n & 7) as usize] = i as u8;
            n += 1;
        }
        i += 1;
        if !(i < 8) { break; }
    }
    text::select_text_window(gm, 1);
    text::define_text_window(gm, 9, 0x26, 1, 0x18, 1);
    text::put_char(gm, 0xff);
    text::define_text_window(gm, 9, 0x27, 1, 0x18, 1);
    rest::clear_status_separator(gm);
    rest::draw_status_title(gm, 0x8f64); // "Reagents:"
    let mut k: i16 = 0;
    while k != n {
        text::put_char(gm, 10);
        text::put_char(gm, 0x20);
        let r = list[(k & 7) as usize];
        let cnt = gm.g8(g::REAGENTS.wrapping_add(r as u16)) as i16;
        text::print_number(gm, 0x30, 2, cnt);
        text::put_char(gm, 0x20);
        let p = gm.g16(g::REAGENT_NAME_PTRS.wrapping_add((r as u16).wrapping_mul(2)));
        text::print_string(gm, p);
        k += 1;
    }
    let mut cur: i16 = 0;
    rest::highlight_party_line(gm, 1);
    let mut done = false;
    let mut mask: u16 = 0;
    loop {
        text::select_text_window(gm, 2);
        let key = input::read_command_key(gm);
        let mut move_down = false;
        let mut move_up = false;
        if key == 4 {
            move_down = true;
        } else if key < 5 {
            if key == 1 || key == 3 {
                move_up = true;
            } else if key == 2 {
                move_down = true;
            }
        } else if key == 0xd || key == 0x20 {
            // LAB_1000_d96e
            let bit = 0x80u16 >> (list[(cur & 7) as usize] & 0x1f);
            mask ^= bit;
            text::select_text_window(gm, 1);
            text::set_cursor_pos(gm, cur + 1, 3);
            text::put_char(gm, 0xfd);
            let c: u8 = if (bit & mask) == 0 { 0x20 } else { 0xf };
            text::put_char(gm, c);
            text::put_char(gm, 0xfd);
        } else if key == 0x1b {
            mask = 0xffff;
            done = true;
            text::put_char(gm, 10);
        } else if key == 0x4d {
            done = true;
            text::print_string(gm, 0x8f6e);
        }
        if move_down {
            if cur < n - 1 {
                rest::highlight_party_line(gm, cur + 1);
                cur += 1;
                rest::highlight_party_line(gm, cur + 1);
            }
        } else if move_up {
            if 0 < cur {
                rest::highlight_party_line(gm, cur + 1);
                cur -= 1;
                rest::highlight_party_line(gm, cur + 1);
            }
        }
        // LAB_1000_d945
        if done {
            rest::highlight_party_line(gm, cur + 1);
            text::select_text_window(gm, 2);
            return mask;
        }
    }
}

/// orig 1000:d9f0 (102 bytes). "How much?" - reads a 2-digit count and re-asks with
/// "Insufficient reagents!" while any selected reagent (bit 0x80 >> i of `mask`) has fewer than
/// that many; 0 is accepted. Returns the count.
pub fn ask_mix_count(gm: &mut Game, param_1: u16) -> u16 {
    let mask = param_1;
    loop {
        let mut ok = true;
        text::print_string(gm, 0x8f72); // "How much?"
        let n = rest::read_number(gm, 2) as u16;
        if n != 0 {
            let mut bit: u16 = 0x80;
            let mut i: i16 = 0;
            loop {
                if (mask & bit) != 0 && (gm.g8(g::REAGENTS.wrapping_add(i as u16)) as u16) < n {
                    text::print_string(gm, 0x8f7e); // "Insufficient reagents!"
                    ok = false;
                    break;
                }
                bit = ((bit as i16) >> 1) as u16;
                i += 1;
                if !(i < 8) { break; }
            }
        }
        if ok {
            return n;
        }
    }
}

/// orig 1000:da58 (326 bytes). Mix reagents (dispatch 'M', vector 1778:0902). "No reagents
/// owned!" when all counts are 0. "For what spell..." -> CAST2 e2be picks the spell (-1 ->
/// 0x8fbe); the arrow-key help line is printed, `select_reagents` and `ask_mix_count`; Esc or a
/// count < 1 just redraws the status window. With a non-empty mask: "Mixing..." (animate_wait
/// (10) in towns/outdoors, wait_ticks(10) in dungeons), the count is subtracted from each
/// selected reagent, and if the mask equals the spell's recipe (g_spell_reagents 0x1cc0[spell])
/// 0x8ffc is printed and g_mixed_spells[spell] += count (cap 99), otherwise the mixture explodes:
/// newline, trap_effect on the first conscious member (g_dx from find_first_conscious_member).
/// Empty mask: "Nothing to mix!". The status window is rebuilt at the end.
pub fn cmd_mix_reagents(gm: &mut Game) {
    let mut total: i16 = 0;
    let mut i: i16 = 0;
    loop {
        total = total.wrapping_add(gm.g8(g::REAGENTS.wrapping_add(i as u16)) as i16);
        i += 1;
        if !(i < 8) { break; }
    }
    if total == 0 {
        text::print_string(gm, 0x8f98); // "No reagents owned!"
        return;
    }
    text::print_string(gm, 0x8fac); // "For what spell..."
    let spell = ovl::cast2::read_spell_name(gm);
    let s: u16;
    'tail: {
        if spell == -1 {
            s = 0x8fbe;
        } else {
            text::put_char(gm, 10);
            text::put_char(gm, 0x1b);
            text::put_char(gm, 0x2c);
            text::put_char(gm, 0x1a);
            text::put_char(gm, 0x2c);
            text::put_char(gm, 0x18);
            text::put_char(gm, 0x2c);
            text::put_char(gm, 0x19);
            text::print_string(gm, 0x8fc6); // "to move, RETURN selects, Type M to mix"
            let mask = select_reagents(gm);
            if (mask as i16) < 0 { break 'tail; }
            let count = ask_mix_count(gm, mask) as i16;
            if count < 1 { break 'tail; }
            if mask != 0 {
                text::print_string(gm, 0x8ff0); // "Mixing..."
                if gm.g8(g::LOCATION) < 0x21 {
                    util::animate_wait(gm, 10);
                } else {
                    util::wait_ticks(gm, 10);
                }
                let mut bit: u16 = 0x80;
                let mut j: i16 = 0;
                loop {
                    if (bit & mask) != 0 {
                        let a = g::REAGENTS.wrapping_add(j as u16);
                        let v = gm.g8(a).wrapping_sub(count as u8);
                        gm.set_g8(a, v);
                    }
                    bit = ((bit as i16) >> 1) as u16;
                    j += 1;
                    if !(j < 8) { break; }
                }
                if spell < 0 || gm.g8(g::SPELL_UNK_1CC0.wrapping_add(spell as u16)) as u16 != mask {
                    text::put_char(gm, 10);
                    rest::find_first_conscious_member(gm);
                    let m = gm.g16(g::DX);
                    rest::trap_effect(gm, m);
                } else {
                    text::print_string(gm, 0x8ffc);
                    let a = g::MIXED_SPELLS.wrapping_add(spell as u16);
                    let v = gm.g8(a).wrapping_add(count as u8);
                    gm.set_g8(a, v);
                    if 99 < gm.g8(a) { gm.set_g8(a, 99); }
                }
                break 'tail;
            }
            s = 0x9004; // "Nothing to mix!"
        }
        text::print_string(gm, s);
    }
    // LAB_1000_db91
    rest::draw_status_top_bar(gm);
    rest::draw_status_separator(gm);
    rest::draw_status_window(gm);
}

/// orig 1000:dba0 (233 bytes). Klimb outdoors with the grapple (dispatch 'K' when g_location
/// == 0; vector 1778:091a). No grapple: "With what?"; not on foot (transport != 0x1c): "On
/// foot!". Asks a direction; the target tile 0x0d (peaks) is "Impassable!", 0x0c (mountains) is
/// climbed: every living member rolls rand 1..30 against DEX and on failure prints 0x904a and
/// takes rand 1..5 damage; then MAINOUT 8524(dy, dx) moves the party. Other tiles "Not
/// climbable!".
pub fn cmd_klimb_outdoors(gm: &mut Game) {
    let s: u16;
    if gm.g8(g::GRAPPLE) == 0 {
        s = 0x9016; // "With what?"
    } else if gm.g8(g::TRANSPORT_TILE) == 0x1c {
        let r = rest::get_direction(gm);
        let dy = gm.g16(g::DY);
        let dx = gm.g16(g::DX);
        if r == 0 { return; }
        let p = rest::get_map_tile(gm, (gm.g8(g::PARTY_Y) as u16).wrapping_add(dy), (gm.g8(g::PARTY_X) as u16).wrapping_add(dx));
        let t = gm.g8(p);
        if t == 0xd {
            s = 0x902c; // "Impassable!"
        } else if t == 0xc {
            if gm.g8(g::PARTY_COUNT) != 0 {
                let mut m: u16 = 0;
                loop {
                    let o = mem32(m);
                    if gm.g8(g::PARTY_STATUS.wrapping_add(o)) != 0x44 {
                        let roll = util::rand_range(gm, 0x1e, 1) as u16;
                        if (gm.g8(g::PARTY_DEX.wrapping_add(o)) as u16) < roll {
                            text::print_string(gm, 0x904a);
                            let dmg = util::rand_range(gm, 5, 1);
                            rest::damage_party_member(gm, dmg, m);
                        }
                    }
                    m += 1;
                    if !(m < gm.g8(g::PARTY_COUNT) as u16) { break; }
                }
            }
            ovl::mainout::move_party_by(gm, dy, dx);
            return;
        } else {
            s = 0x903a; // "Not climbable!"
        }
    } else {
        s = 0x9022; // "On foot!"
    }
    text::print_string(gm, s);
}
