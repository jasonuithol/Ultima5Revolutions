//! COMBAT.OVL (translated)
//!
//! Level-2 overlay holding the combat main loop (`combat_main_loop`, vector 1778:04b2, run by ROOT
//! `run_encounter`) and everything a combat turn needs: the player command prompt for party-side
//! combatants (`party_member_turn`), the monster AI (`monster_turn`, `monster_move`,
//! `find_nearest_enemy`, `monster_choose_and_attack`, `monster_ranged_attack`, `monster_morale`),
//! attack resolution (`attack_hits`, `get_combat_stat`, `roll_attack_damage`, `deal_attack_damage`,
//! `poison_attack`, `apply_damage`), terrain/field hazards (`apply_cell_hazard`), dungeon-room
//! triggers (`check_room_trigger`), cell passability (`combat_cell_free`) and combatant removal
//! (`remove_combatant`). The 11x11 combat map lives in g_light_map (0xad14, stride 0x20); the
//! roster is g_combat_roster (0xba14, 32 x 8: +0 monster HP, +1 initiative, +2 flags, +3 member /
//! monster type, +4 monster-table slot, +5 turn counter, +6 x, +7 y) and the monster table
//! (0x5c5a) holds the on-map objects.
//!
//! Ghidra split three addresses off as separate "functions" because vectors of other overlays
//! (SHOPPES 1778:07ee, LOOKOBJ 1778:079a) land on them or because of a tail jump:
//! `fn_1000_a2b6_fragment` is the body of `combat_cell_free` (a290), `fn_1000_a5f6_fragment` and
//! `fn_1000_a670_fragment` are tails of `monster_choose_and_attack` (a4b6). Their code is
//! translated inside the owning functions; the fragments are documented no-ops.
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
/// DGROUP 0x5890 (save 0x2EA): "the current attack is a spell" flag set by the Cast command for
/// this turn (second byte of g::SAVE_UNK_2E9).
const ATTACK_IS_SPELL: u16 = 0x5890;
/// DGROUP 0x588f (save 0x2E9): spell-mode flag passed (negated) to `attack_hits`.
const SPELL_MODE: u16 = 0x588f;

/// C `strcat(dst, src)` over DGROUP strings (dst and src are DGROUP offsets).
fn strcat_g(gm: &mut Game, dst: u16, src: u16) {
    let mut d = dst;
    while gm.g8(d) != 0 { d = d.wrapping_add(1); }
    let mut s = src;
    loop {
        let c = gm.g8(s);
        gm.set_g8(d, c);
        if c == 0 { break; }
        d = d.wrapping_add(1);
        s = s.wrapping_add(1);
    }
}

/// orig 1000:a290 (38 bytes + the a2b6 body). Ghidra order (y, x, mover_tile). Returns 1 when
/// combat cell (y,x) can be entered by a combatant drawn with `mover_tile`: inside the 11x11
/// map the map tile must be enterable (`can_enter_tile`) and not 0xff, and no monster-table slot
/// may occupy the cell with a blocking object (tile 0xeb always blocks; tiles 0xe8..0xeb, 0x1e,
/// 0x1f and 0 never block; any other object blocks when its anim tile (+1) is non-zero), nor any
/// roster entry whose flags have neither 0x20 (out) nor 0x04 (swallowed) and are non-zero. Cells
/// outside the map return 1. Vectors 1778:02f6/057e/07b2 (NPC/FLAMES/DNGLOOK names in Ghidra).
pub fn combat_cell_free(gm: &mut Game, param_1: u16, param_2: u16, param_3: u16) -> u16 {
    let (y, x, mover) = (param_1, param_2, param_3);
    if (x as i16) < 0xb && (y as i16) < 0xb && -1 < (x as i16) && -1 < (y as i16) {
        // ---- fn_1000_a2b6 body ----
        let p = rest::get_map_tile(gm, y, x);
        let tile = gm.g8(p);
        if rest::can_enter_tile(gm, tile as i16, mover as i16) == 0 {
            return 0;
        }
        let q = rest::get_map_tile(gm, y, x);
        if gm.gi8(q) == -1 {
            return 0;
        }
        let mut off: u16 = 0;
        loop {
            if gm.g8(g::MONSTER_X.wrapping_add(off)) as u16 == x
                && gm.g8(g::MONSTER_Y.wrapping_add(off)) as u16 == y
            {
                let t = gm.g8(g::MONSTER_TABLE.wrapping_add(off));
                if t == 0xeb {
                    return 0;
                }
                if (t & 0xfc) != 0xe8 && t != 0x1e && t != 0x1f && t != 0 {
                    if gm.g8(g::MONSTER_TABLE.wrapping_add(off).wrapping_add(1)) != 0 {
                        return 0;
                    }
                }
            } else if gm.g8(g::ROSTER_X.wrapping_add(off)) as u16 == x
                && gm.g8(g::ROSTER_Y.wrapping_add(off)) as u16 == y
                && (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x24) == 0
            {
                if gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) != 0 {
                    return 0;
                }
            }
            off = off.wrapping_add(8);
            if !(g::ROSTER_SLOT.wrapping_add(off) < 0xbb18) { break; }
        }
    }
    1
}

/// orig 1000:a2b6 (293 bytes). Not a function: the body of `combat_cell_free` (a290) that Ghidra
/// split off because the SHOPPES vector 1778:07ee targets this address in SHOPPES.OVL. Its code
/// is translated inside `combat_cell_free`.
pub fn fn_1000_a2b6_fragment(gm: &mut Game) -> u16 { 1 }

/// orig 1000:a3de (215 bytes). Ghidra order (skip_hit_roll, target, attacker). Monster ranged
/// attack: fires when the attacker type is 0x1a (always) or a 0..255 roll is below 0x80, and
/// unless the type's flag 0x8000 is set while the status icon is 'N' or 0x1c (protection).
/// Plays the attack sweep, computes the hit flag with `attack_hits(0, -spell_mode, attacker,
/// target)` (or 0 when skip_hit_roll != 0), then COMSUBS ea02(missile tile = 0x15cc[type], hit,
/// target y, target x, attacker) animates the projectile and returns the roster entry struck
/// (-1 none): that entry is flashed, a newline printed, `deal_attack_damage(attacker, hit)` and
/// COMSUBS e4f2(attacker, hit). Returns 1 when the attack was attempted, 0 otherwise.
pub fn monster_ranged_attack(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16) -> u16 {
    let (skip, target, attacker) = (param_1, param_2, param_3);
    let mtype = gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(attacker))) as u16;
    let missile = gm.g8(mtype.wrapping_add(0x15cc)); // g_monster_missile_tile[type]
    let icon = gm.gi8(g::STATUS_ICON);
    let fire = mtype == 0x1a || util::rand_range(gm, 0xff, 0) < 0x80;
    if fire
        && ((gm.g16(mtype.wrapping_mul(2).wrapping_add(g::MONSTER_FLAGS)) & 0x8000) == 0
            || (icon != b'N' as i8 && icon != 0x1c))
    {
        rest::play_tone_sweep(gm, 0x96, 5, 400, 0x2ee);
        let hit: u16 = if skip == 0 {
            let neg = (gm.g8(SPELL_MODE) as u16).wrapping_neg();
            attack_hits(gm, 0, neg, attacker as u16, target as u16) as u16
        } else {
            0
        };
        let ty = gm.g8(g::ROSTER_Y.wrapping_add(ros8(target))) as u16;
        let tx = gm.g8(g::ROSTER_X.wrapping_add(ros8(target))) as u16;
        let r = ovl::comsubs::animate_missile(gm, missile as u16, hit as i16, ty, tx, attacker) as i16;
        if -1 < r {
            rest::flash_monster(gm, r as u16);
            text::put_char(gm, 10);
            deal_attack_damage(gm, attacker, r);
            ovl::comsubs::print_hit_result(gm, attacker, r);
        }
        return 1;
    }
    0
}

/// orig 1000:a4b6 (316 bytes, plus the a5f6/a670 tails). Monster attack decision for roster
/// entry `ri`: `find_nearest_enemy` picks the target (none -> 0). A charmed entry (flag bit 0)
/// attacks only when adjacent (COMSUBS e6b4 == 1) with g_attack_weapon = 0x21 via COMSUBS
/// edd8(0x21, target, ri) and returns 1. Otherwise: a wasted 0..255 roll when the target is a
/// party member wearing amulet '-' (0x2d) and the type has flag 0x8000; the attack happens only
/// when the distance (e6b4) is <= the type's range in 0x159c. Type 0x1a sets its monster anim
/// state to 0x20. Adjacent (distance 1): type 0x2d also sets anim state 0x20; attack sweep,
/// g_monster_interference[target] = ri, and `attack_hits(0, -spell_mode, ri, target)`: a miss
/// returns 1; on a hit a thief type (flag bit 1) with a 1..3 roll and food left prints "\nA
/// <name> stole some food!", food -= 5, tone, status window, returns 1; else the target is
/// flashed, newline, `deal_attack_damage(ri, target)`, COMSUBS e4f2(ri, target), returns 1.
/// Not adjacent (within range): returns `monster_ranged_attack(0, target, ri)`.
pub fn monster_choose_and_attack(gm: &mut Game, param_1: i16) -> u16 {
    let ri = param_1;
    let off = ros8(ri);
    let mtype = gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) as u16;
    let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
    let target = find_nearest_enemy(gm, ri);
    if -1 < target {
        let toff = ros8(target);
        if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 1) == 0 {
            if (gm.g8(g::ROSTER_FLAGS.wrapping_add(toff)) & 0x80) != 0
                && gm.gi8(mem32(gm.g8(g::ROSTER_MEMBER.wrapping_add(toff)) as u16).wrapping_add(g::PARTY_AMULET)) == b'-' as i8
                && (gm.g16(mtype.wrapping_mul(2).wrapping_add(g::MONSTER_FLAGS)) & 0x8000) != 0
            {
                util::rand_range(gm, 0xff, 0);
            }
            let dist = ovl::comsubs::roster_distance(gm, target, ri);
            if dist <= gm.g8(mtype.wrapping_add(0x159c)) as u16 {
                if mtype == 0x1a {
                    gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(slot8(slot)), 0x20);
                }
                let d2 = ovl::comsubs::roster_distance(gm, target, ri) as i16;
                if d2 == 1 {
                    if mtype == 0x2d {
                        gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(slot8(slot)), 0x20);
                    }
                    rest::play_tone_sweep(gm, 0x96, 5, 400, 0x2ee);
                    gm.set_g8(g::MONSTER_INTERFERENCE.wrapping_add(target as u16), ri as u8);
                    let neg = (gm.g8(SPELL_MODE) as u16).wrapping_neg();
                    let hit = attack_hits(gm, 0, neg, ri as u16, target as u16);
                    if hit == 0 {
                        return 1;
                    }
                    // ---- fn_1000_a5f6 tail ----
                    if (gm.g8(mtype.wrapping_mul(2).wrapping_add(g::MONSTER_FLAGS)) & 2) != 0
                        && util::rand_range(gm, 3, 0) != 0
                        && gm.g16(g::FOOD) != 0
                    {
                        text::print_string(gm, 0x6d74); // "\nA "
                        ovl::comsubs::print_combatant_name(gm, ri);
                        text::print_string(gm, 0x6d78); // " stole some food!"
                        rest::sub_i16_floor(gm, 5, g::FOOD);
                        rest::play_tone_sweep(gm, 0x32, 1, 2000, 800);
                        rest::draw_status_window(gm);
                        return 1;
                    }
                    rest::flash_monster(gm, target as u16);
                    text::put_char(gm, 10);
                    deal_attack_damage(gm, ri, target);
                    ovl::comsubs::print_hit_result(gm, ri, target);
                    return 1;
                }
                // ---- fn_1000_a670 tail ----
                return monster_ranged_attack(gm, 0, target, ri);
            }
        } else {
            let d = ovl::comsubs::roster_distance(gm, target, ri) as i16;
            if d == 1 {
                gm.set_g8(g::ATTACK_WEAPON, 0x21);
                ovl::comsubs::melee_attack(gm, 0x21, target as u16, ri as u16);
                return 1;
            }
        }
    }
    0
}

/// orig 1000:a5f6 (122 bytes). Not a function: the "stole some food" / melee-hit tail of
/// `monster_choose_and_attack` (a4b6) that Ghidra split off because the LOOKOBJ vector
/// 1778:079a targets this address in LOOKOBJ.OVL. Translated inside the owner.
pub fn fn_1000_a5f6_fragment(gm: &mut Game) -> u16 { 1 }

/// orig 1000:a670 (19 bytes). Not a function: the tail jump of `monster_choose_and_attack`
/// (a4b6) that calls `monster_ranged_attack(0, target, ri)` with the caller's locals and shares
/// its epilogue. Translated inside the owner.
pub fn fn_1000_a670_fragment(gm: &mut Game) {}

/// orig 1000:a684 (333 bytes). AI turn for g_active_combatant. g_attack_weapon = 0. Nothing
/// happens while the status icon is 'T' (time stop), on a 50% roll while it is 'Q' (quickness),
/// or when the entry is swallowed (flag 4). Asleep (flag 8): wakes on a 1/17 roll. Otherwise:
/// type '-' (0x2d) resets its monster anim state; a non-fleeing entry (flag 2 clear) first lets
/// COMSUBS e2d4 act (non-zero = turn used) then `monster_choose_and_attack` (non-zero = turn
/// used); a fleeing monster regains 1 HP on a 1/4 roll and `monster_morale` re-evaluates the
/// flee flag. Then `monster_move`: 0 (did not move) -> a fleeing entry still attacks; moved ->
/// when g_combat_state_flags has 0x10 (stepped off the map) prints "\n<name> escapes!" with a
/// tone, removes the entry (`remove_combatant(-1 - index)`) and, for a monster of type '/'
/// (0x2f), runs `chaos_sword_passout`; the view is refreshed.
pub fn monster_turn(gm: &mut Game) {
    let ac = gm.g8(g::ACTIVE_COMBATANT) as i16;
    let off = ros8(ac);
    gm.set_g8(g::ATTACK_WEAPON, 0);
    let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
    let icon = gm.gi8(g::STATUS_ICON);
    if icon != b'T' as i8
        && (icon != b'Q' as i8 || util::rand_range(gm, 1, 0) != 0)
        && (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 4) == 0
    {
        if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 8) == 0 {
            if gm.gi8(g::ROSTER_MEMBER.wrapping_add(off)) == b'-' as i8 {
                gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(slot8(slot)), 0);
            }
            if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 2) == 0 {
                if ovl::comsubs::monster_special_action(gm, ac) != 0 {
                    return;
                }
                if monster_choose_and_attack(gm, ac) != 0 {
                    return;
                }
            } else {
                if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) == 0 && util::rand_range(gm, 3, 0) == 3 {
                    let hp = gm.g8(g::COMBAT_ROSTER.wrapping_add(off)).wrapping_add(1);
                    gm.set_g8(g::COMBAT_ROSTER.wrapping_add(off), hp);
                }
                monster_morale(gm, ac);
            }
            let moved = monster_move(gm, ac);
            if moved == 0 {
                if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 2) != 0 {
                    monster_choose_and_attack(gm, ac);
                }
            } else {
                if (gm.g8(g::COMBAT_STATE_FLAGS) & 0x10) != 0 {
                    text::put_char(gm, 10);
                    rest::play_tone_sweep(gm, 0x28, 1, 2000, 0x4b0);
                    ovl::comsubs::print_combatant_name(gm, ac);
                    text::print_string(gm, 0x6d8c); // " escapes!"
                    remove_combatant(gm, (-1i16).wrapping_sub(ac));
                    if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) == 0
                        && gm.gi8(g::ROSTER_MEMBER.wrapping_add(off)) == b'/' as i8
                    {
                        ovl::sjog::chaos_sword_passout(gm);
                    }
                }
                rest::refresh_map_view(gm);
            }
        } else {
            if util::rand_range(gm, 0x10, 0) == 0x10 {
                rest::wake_combatant(gm, ac);
            }
        }
    }
}

/// orig 1000:a7d4 (108 bytes). Ghidra order (kind, msg). Prints the command name `msg`; when
/// the active combatant is not a party member (a charmed monster under player control) prints
/// "Can't!" (0x6d98) and returns 1 (re-prompt). Otherwise runs the overlay command: 0 Get
/// (SJOG cmd_get), 1 Jimmy (SJOG cmd_jimmy), 2 Open (SJOG cmd_open), 3 Ready (ZSTATS f476),
/// 4 Search (SJOG cmd_search), 5 Use item (CAST d712); returns 0 (turn ends).
pub fn run_party_only_command(gm: &mut Game, param_1: i16, param_2: u16) -> u16 {
    let (kind, msg) = (param_1, param_2);
    text::print_string(gm, msg);
    let ac = gm.g8(g::ACTIVE_COMBATANT) as i16;
    if (gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(ac))) & 0x80) == 0 {
        text::print_string(gm, 0x6d98); // "Can't!"
        1
    } else {
        if kind == 0 {
            ovl::sjog::cmd_get(gm);
        } else if kind == 1 {
            ovl::sjog::cmd_jimmy(gm);
        } else if kind == 2 {
            ovl::sjog::cmd_open(gm);
        } else if kind == 3 {
            ovl::zstats::ready_command(gm);
        } else if kind == 4 {
            ovl::sjog::cmd_search(gm);
        } else if kind == 5 {
            ovl::cast::use_item_command(gm);
        }
        0
    }
}

/// orig 1000:a846 (135 bytes). Ghidra order (need_separator, item). Appends the long name of
/// equipped item `item` (g_name_ptrs[item], 0x17f6) to g_ool_buffer (used as the prompt string
/// buffer), preceded by ", " (0x6da0) when `need_separator` != 0. Nothing is appended and 0 is
/// returned when item == 0xff (none) or the item has attack value 0 in g_item_attack (0x15fc);
/// otherwise returns 1.
pub fn append_equipped_item_name(gm: &mut Game, param_1: i16, param_2: i16) -> u16 {
    let (sep, item) = (param_1, param_2);
    if item == 0xff || gm.g8((item as u16).wrapping_add(0x15fc)) == 0 {
        0
    } else {
        if sep != 0 {
            strcat_g(gm, g::OOL_BUFFER, 0x6da0); // ", "
        }
        let name = gm.g16((item as u16).wrapping_mul(2).wrapping_add(g::NAME_PTRS));
        strcat_g(gm, g::OOL_BUFFER, name);
        1
    }
}

/// orig 1000:a8ce (1301 bytes). Player-controlled turn for g_active_combatant. g_party_x/y are
/// set to the entry's position. If an active player is set (g_active_member != 0xff) and this
/// entry is not that member, only `combat_turn_effects` runs. A party member wielding the Chaos
/// sword ('#' = 0x23 in either hand) gets flag bit 0 set, g_active_member = 0xff, the status
/// window marked dirty and is run by `monster_turn` instead. Otherwise the member's status line
/// is highlighted and the prompt "\n<name>, armed with <helm, left, right | bare hands>" is
/// built in g_ool_buffer (0x6dbe appended) and printed; then the command loop: swallowed
/// (flag 4) prints "ARGH!", noise, `regurgitate_check` and ends the turn; asleep (flag 8) wakes
/// on a 1/16 roll, prints 0x6dc8 and ends the turn; else `read_command_key`: directions 1..4
/// `move_combatant`; Ctrl-S toggles sound; Esc `combat_escape_all`; space "Pass"; '0' clears
/// the active player; '1'..'6' `set_active_player`; 'A' COMSUBS ef76(weapon count, entry);
/// 'C' Cast (COMSUBS ebdc selects, then g_attack_is_spell/g_spell_mode = 1 and either
/// "Absorbed!" under 'N' protection / in Castle Blackthorn without the crown, or CAST cd3a);
/// 'G','J','O','R','S','U' via `run_party_only_command`; 'K' `combat_klimb`; 'P' CMDS cmd_push;
/// 'Y' CMDS cmd_yell; 'Z' ZSTATS ec1a; 'T' print_refusal(3, "Talk"); 'B','X' print_refusal(1);
/// 'E','F','H','I','L','M','N','Q','V' print_refusal(2); 'D','W' print the name and re-prompt;
/// 0xfc toggles the key buffer; anything else prints 0x6ee6 and re-reads. A command returning
/// "re-prompt" (local_4 != 0) rebuilds the prompt. Afterwards the highlight is removed and,
/// unless the key was '0'..'6', `combat_turn_effects`, a view refresh and
/// `combat_check_absorbed` run.
pub fn party_member_turn(gm: &mut Game) {
    let ac = gm.g8(g::ACTIVE_COMBATANT) as i16;
    let off = ros8(ac);
    gm.set_g8(g::PARTY_X, gm.g8(g::ROSTER_X.wrapping_add(off)));
    gm.set_g8(g::PARTY_Y, gm.g8(g::ROSTER_Y.wrapping_add(off)));
    let member = gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) as u16;
    let active = gm.g8(g::ACTIVE_MEMBER);
    if active != 0xff && ((gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) == 0 || member != active as u16) {
        ovl::sjog::combat_turn_effects(gm);
        return;
    }
    let mut key: u8 = 0; // TODO(verify): uninitialised local in the original on the Chaos-sword path
    let is_party = (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) != 0;
    if !is_party
        || (gm.gi8(mem32(member).wrapping_add(0x55c3)) != b'#' as i8
            && gm.gi8(mem32(member).wrapping_add(0x55c4)) != b'#' as i8)
    {
        if is_party {
            rest::highlight_party_line(gm, gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) as i16);
        }
        if gm.g8(g::STATUS_DIRTY) != 0 {
            rest::draw_status_window(gm);
            gm.set_g8(g::STATUS_DIRTY, 0);
        }
        let mut weapons: i16 = 0; // local_c
        'prompt: loop {
            // LAB_1000_a981
            text::put_char(gm, 10);
            ovl::comsubs::print_combatant_name(gm, ac);
            gm.set_g8(g::OOL_BUFFER, 0);
            if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) != 0 {
                text::print_string(gm, 0x6da4); // ", armed with "
                let rec = mem32(member);
                let a = append_equipped_item_name(gm, 0, gm.g8(rec.wrapping_add(0x55c1)) as i16) as i16;
                let b = append_equipped_item_name(gm, a, gm.g8(rec.wrapping_add(0x55c3)) as i16) as i16;
                let c = append_equipped_item_name(gm, a.wrapping_add(b), gm.g8(rec.wrapping_add(0x55c4)) as i16) as i16;
                weapons = a.wrapping_add(b).wrapping_add(c);
                if weapons == 0 {
                    strcat_g(gm, g::OOL_BUFFER, 0x6db2); // "bare hands"
                }
            }
            strcat_g(gm, g::OOL_BUFFER, 0x6dbe);
            text::print_string(gm, g::OOL_BUFFER);
            let mut reprompt: i16 = 0; // local_4
            let mut done = false; // bVar3
            loop {
                // LAB_1000_aa4a
                if done { break; }
                text::put_char(gm, 10);
                rest::draw_bar_left_cap(gm);
                if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 4) != 0 {
                    text::print_string(gm, 0x6dc0); // "ARGH!"
                    util::play_random_tones(gm, 500, 3000, 0x28);
                    regurgitate_check(gm, ac);
                    done = true;
                    continue;
                }
                if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 8) != 0 {
                    if util::rand_range(gm, 0xff, 0) < 0x10 {
                        rest::wake_combatant(gm, ac);
                    }
                    text::print_string(gm, 0x6dc8);
                    done = true;
                    continue;
                }
                key = input::read_command_key(gm) as u8;
                done = true;
                reprompt = 0;
                // helper closures are not usable with &mut gm; use small enum of outcomes
                enum Act { None, Refusal(i16, u16), PrintReprompt(u16), PartyCmd(i16, u16), Huh, Again, MoveRes(i16) }
                let act: Act = if key == 0x4a {
                    Act::PartyCmd(1, 0x6e1a) // "Jimmy"
                } else if key < 0x4b {
                    if key == 0x41 {
                        ovl::comsubs::attack_command(gm, weapons, ac);
                        Act::None
                    } else if key < 0x42 {
                        if key == 0x1b {
                            reprompt = ovl::cmds::combat_escape_all(gm);
                            Act::None
                        } else if key < 0x1c {
                            if key != 0 && key < 5 {
                                let r = ovl::sjog::move_combatant(gm, key as i16, ac) as i16;
                                Act::MoveRes(r)
                            } else if key == 0x13 {
                                text::print_string(gm, 0x6de4); // "Sound"
                                let on = gm.gi8(g::SOUND_ENABLED);
                                text::print_string(gm, if on == 0 { 0x6df2 } else { 0x6dec });
                                gm.set_g8(g::SOUND_ENABLED, (on == 0) as u8);
                                Act::Again
                            } else {
                                Act::Huh
                            }
                        } else if key == 0x20 {
                            text::print_string(gm, 0x6e60); // "Pass"
                            Act::None
                        } else if key == 0x30 {
                            gm.set_g8(g::ACTIVE_MEMBER, 0xff);
                            text::print_string(gm, 0x6e66); // "Set active plr: None!"
                            rest::draw_status_window(gm);
                            Act::None
                        } else if 0x30 < key && key < 0x37 {
                            let r = ovl::sjog::set_active_player(gm, (key - 0x31) as u16);
                            if r == 0 { reprompt = 1; }
                            Act::None
                        } else {
                            Act::Huh
                        }
                    } else {
                        match key {
                            0x42 => Act::Refusal(1, 0x6e7e), // "Board"
                            0x43 => {
                                text::print_string(gm, 0x6df6); // "Cast..."
                                reprompt = 1;
                                if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) == 0 {
                                    text::print_string(gm, 0x6e0c);
                                } else {
                                    let r = ovl::comsubs::check_interference(gm, ac);
                                    if r == 0 {
                                        gm.set_g8(ATTACK_IS_SPELL, 1);
                                        gm.set_g8(SPELL_MODE, 1);
                                        reprompt = 0;
                                        if gm.gi8(g::STATUS_ICON) == b'N' as i8
                                            || (gm.g8(g::CROWN_LB) == 0 && gm.gi8(g::LOCATION_BACKUP) == 0x12)
                                        {
                                            text::print_string(gm, 0x6e00); // "Absorbed!"
                                            util::play_sweep_noise(gm, 2, 1000, 0x6d5e + 2, 1, 0x2648);
                                        } else {
                                            ovl::cast::cast_spell_command(gm);
                                        }
                                    }
                                }
                                Act::None
                            }
                            0x44 => Act::PrintReprompt(0x6e84),
                            0x45 => Act::Refusal(2, 0x6e8e), // "Enter"
                            0x46 => Act::Refusal(2, 0x6e94),
                            0x47 => Act::PartyCmd(0, 0x6e14), // "Get"
                            0x48 => Act::Refusal(2, 0x6e9a), // "Hole up"
                            0x49 => Act::Refusal(2, 0x6ea2), // "Ignite torch"
                            _ => Act::Huh,
                        }
                    }
                } else if key == 0x52 {
                    Act::PartyCmd(3, 0x6e2e) // "Ready..."
                } else if 0x52 < key {
                    if key == 0x56 {
                        Act::Refusal(2, 0x6ed0)
                    } else if key < 0x57 {
                        if key == 0x53 {
                            Act::PartyCmd(4, 0x6e3a) // "Search."
                        } else if key == 0x54 {
                            Act::Refusal(3, 0x6eca)
                        } else if key != 0x55 {
                            Act::Huh
                        } else {
                            Act::PartyCmd(5, 0x6e42) // "Use item"
                        }
                    } else if key == 0x57 {
                        Act::PrintReprompt(0x6ed6)
                    } else if key == 0x58 {
                        Act::Refusal(1, 0x6ee0)
                    } else if key == 0x59 {
                        text::print_string(gm, 0x6e4e);
                        ovl::cmds::cmd_yell(gm);
                        Act::None
                    } else if key == 0x5a {
                        text::print_string(gm, 0x6e54); // "Z-stats..."
                        ovl::zstats::zstats_command(gm);
                        Act::None
                    } else if key != 0xfc {
                        Act::Huh
                    } else {
                        text::print_string(gm, 0x6dd2); // "Buffer O"
                        let v = (gm.g16(g::FLUSH_AFTER_KEY) == 0) as u16;
                        gm.set_g16(g::FLUSH_AFTER_KEY, v);
                        text::print_string(gm, if v == 0 { 0x6de0 } else { 0x6ddc });
                        Act::Again
                    }
                } else {
                    match key {
                        0x4b => {
                            let r = ovl::sjog::combat_klimb(gm) as i16;
                            Act::MoveRes(r)
                        }
                        0x4c => Act::Refusal(2, 0x6eb0),
                        0x4d => Act::Refusal(2, 0x6eb6),
                        0x4e => Act::Refusal(2, 0x6eba), // "New order"
                        0x4f => Act::PartyCmd(2, 0x6e22), // "Open"
                        0x50 => {
                            text::print_string(gm, 0x6e28);
                            ovl::cmds::cmd_push(gm);
                            Act::None
                        }
                        0x51 => Act::Refusal(2, 0x6ec4),
                        _ => Act::Huh,
                    }
                };
                match act {
                    Act::None => {}
                    Act::Refusal(reason, msg) => {
                        // LAB_1000_acc3
                        reprompt = ovl::sjog::print_refusal(gm, reason, msg) as i16;
                    }
                    Act::PrintReprompt(msg) => {
                        // LAB_1000_accd / acd1
                        text::print_string(gm, msg);
                        reprompt = 1;
                    }
                    Act::PartyCmd(kind, msg) => {
                        // LAB_1000_ac00
                        reprompt = run_party_only_command(gm, kind, msg) as i16;
                    }
                    Act::Huh => {
                        // switchD_1000_ad83_default
                        text::print_string(gm, 0x6ee6);
                        done = false;
                    }
                    Act::Again => {
                        // LAB_1000_ab3d
                        done = false;
                    }
                    Act::MoveRes(r) => {
                        // LAB_1000_acb1
                        if r == 0 { done = false; }
                    }
                }
            }
            // LAB_1000_ade6
            if reprompt != 0 {
                continue 'prompt;
            }
            break;
        }
        // LAB_1000_adef
        if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) != 0 {
            rest::highlight_party_line(gm, gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) as i16);
        }
    } else {
        let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) | 1;
        gm.set_g8(g::ROSTER_FLAGS.wrapping_add(off), f);
        gm.set_g8(g::ACTIVE_MEMBER, 0xff);
        gm.set_g8(g::STATUS_DIRTY, 1);
        monster_turn(gm);
    }
    // LAB_1000_ae09
    if key < 0x30 || 0x36 < key {
        ovl::sjog::combat_turn_effects(gm);
        rest::refresh_map_view(gm);
        ovl::sjog::combat_check_absorbed(gm);
    }
}

/// orig 1000:ae24 (410 bytes). The combat main loop (vector 1778:04b2, called by ROOT
/// `run_encounter` and DUNGEON room entry). g_combat_exit = 0, view + status window drawn,
/// keyboard flushed, the sides counted (`count_combatants_by_side`: g_dy party, g_dx enemies)
/// and g_no_more_enemies = (no enemies). Then forever, for roster entries 0..31: g_door_tile =
/// 0; an active entry (flags & 0xc0, not 0x20) that is a dead party member ('D') is marked out
/// (0x20), flashed and killed with `apply_damage(99)`; an entry standing on a tile whose value
/// & 0xfe == 0x84 is skipped; otherwise its turn counter (+5) is decremented and, on reaching
/// 0, reset to 36 - initiative and the entry acts: crosshair hidden, g_combat_state_flags,
/// g_spell_mode, g_attack_is_spell and g_attack_weapon cleared, g_turn_tick (0x5882) counts to
/// 10 -> `pass_time(1)`; g_focus_blink = 1; party side -> `party_member_turn`, enemy side ->
/// `monster_turn`; then g_monster_interference[entry] = 0xff, `apply_cell_hazard(entry)` and
/// the sides are recounted. No party left: no enemies either -> result 0, else
/// `chaos_sword_passout` (an entry woke -> continue) or "BATTLE IS LOST!" (unless
/// g_no_more_enemies) with result 1; the loop ends. Party left and no enemies -> "VICTORY!"
/// once (g_no_more_enemies = 1, triple sweep, flush). On exit the keyboard is flushed and
/// g_beam_frame = 0xff. Returns 0 when everyone is gone from the map (victory/escape
/// completed), 1 when the battle was lost.
pub fn combat_main_loop(gm: &mut Game) -> u16 {
    let mut result: u16 = 1; // local_4
    gm.set_g8(g::COMBAT_EXIT, 0);
    rest::refresh_map_view(gm);
    rest::draw_status_window(gm);
    input::flush_keyboard(gm);
    ovl::sjog::count_combatants_by_side(gm);
    gm.set_g8(g::NO_MORE_ENEMIES, (gm.gi16(g::DX) == 0) as u8);
    let mut finished = false; // bVar3
    loop {
        gm.set_g8(g::ACTIVE_COMBATANT, 0);
        loop {
            gm.set_g8(g::DOOR_TILE, 0);
            let ac = gm.g8(g::ACTIVE_COMBATANT) as i16;
            let off = ros8(ac);
            let flags = gm.g8(g::ROSTER_FLAGS.wrapping_add(off));
            if (flags & 0xc0) != 0 && (flags & 0x20) == 0 {
                let member = gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) as u16;
                if (flags & 0x80) == 0 || gm.gi8(g::PARTY_STATUS.wrapping_add(mem32(member))) != b'D' as i8 {
                    let ry = gm.g8(g::ROSTER_Y.wrapping_add(off)) as u16;
                    let rx = gm.g8(g::ROSTER_X.wrapping_add(off)) as u16;
                    if (gm.g8(g::LIGHT_MAP.wrapping_add(ry.wrapping_mul(0x20)).wrapping_add(rx)) & 0xfe) != 0x84 {
                        let tc = gm.g8(g::ROSTER_TURN_COUNTER.wrapping_add(off)).wrapping_sub(1);
                        gm.set_g8(g::ROSTER_TURN_COUNTER.wrapping_add(off), tc);
                        if tc == 0 {
                            let init = gm.g8(g::ROSTER_INITIATIVE.wrapping_add(off));
                            gm.set_g8(g::ROSTER_TURN_COUNTER.wrapping_add(off), (b'$').wrapping_sub(init));
                            gm.set_g8(g::CROSSHAIR_VISIBLE, 0);
                            gm.set_g8(g::COMBAT_STATE_FLAGS, 0);
                            gm.set_g8(SPELL_MODE, 0);
                            gm.set_g8(ATTACK_IS_SPELL, 0);
                            gm.set_g8(g::ATTACK_WEAPON, 0);
                            let t = gm.gi8(g::SAVE_UNK_2DC).wrapping_add(1); // turn tick 0x5882
                            gm.set_gi8(g::SAVE_UNK_2DC, t);
                            if t == 10 {
                                gm.set_gi8(g::SAVE_UNK_2DC, 0);
                                rest::pass_time(gm, 1);
                            }
                            gm.set_g8(g::FOCUS_BLINK, 1);
                            if !rest::combatant_side(gm, ac) {
                                party_member_turn(gm);
                            } else {
                                monster_turn(gm);
                            }
                            let ac2 = gm.g8(g::ACTIVE_COMBATANT) as u16;
                            gm.set_g8(g::MONSTER_INTERFERENCE.wrapping_add(ac2), 0xff);
                            apply_cell_hazard(gm, ac2 as i16);
                            ovl::sjog::count_combatants_by_side(gm);
                            if gm.gi16(g::DY) == 0 {
                                let mut skip = false;
                                if gm.gi16(g::DX) == 0 {
                                    result = 0;
                                } else {
                                    let r = ovl::sjog::chaos_sword_passout(gm);
                                    if r != -1 {
                                        skip = true; // goto LAB_1000_af98
                                    } else {
                                        rest::refresh_map_view(gm);
                                        if gm.g8(g::NO_MORE_ENEMIES) == 0 {
                                            text::print_string(gm, 0x6eee); // "BATTLE IS LOST!"
                                            result = 1;
                                        }
                                    }
                                }
                                if !skip {
                                    finished = true;
                                    break;
                                }
                            } else if gm.gi16(g::DX) == 0 && gm.g8(g::NO_MORE_ENEMIES) == 0 {
                                text::print_string(gm, 0x6f00); // "VICTORY!"
                                gm.set_g8(g::NO_MORE_ENEMIES, 1);
                                rest::play_triple_sweep_sound(gm);
                                input::flush_keyboard(gm);
                            }
                        }
                    }
                } else {
                    gm.set_g8(g::ROSTER_FLAGS.wrapping_add(off), flags | 0x20);
                    rest::flash_monster(gm, ac as u16);
                    apply_damage(gm, 99, ac);
                }
            }
            // LAB_1000_af98
            let n = gm.g8(g::ACTIVE_COMBATANT).wrapping_add(1);
            gm.set_g8(g::ACTIVE_COMBATANT, n);
            if !(n < 0x20) { break; }
        }
        if finished {
            input::flush_keyboard(gm);
            gm.set_g8(g::BEAM_FRAME, 0xff);
            return result;
        }
    }
}

/// orig 1000:afc0 (436 bytes). Picks the closest enemy of roster entry `ri` and sets g_dx/g_dy
/// to the unit step towards it (reversed when the entry is fleeing, flag 2). The entry's own
/// side comes from `combatant_side`; under the 'C' status icon (charm?) a `get_combat_stat(-1)`
/// roll below `random_damage_1_30` makes it treat the party side as its own. Candidates are
/// entries 31..0 other than `ri` with non-zero flags, not out (0x20), on the other side, not
/// swallowed (4) and, unless in location 0x28 or the entry is type '/' (0x2f), without flag
/// 0x10; entries with index < 5 are counted; the one with the smallest COMSUBS e66a distance
/// wins. No candidate at all -> `chaos_sword_passout` provides the target; still none -> every
/// monster entry 6..31 (flag 0x40) gets HP 1 and the flee flag, and the target cell is (5,5).
/// Returns the target index, or -1 when none / it is `ri` itself.
pub fn find_nearest_enemy(gm: &mut Game, param_1: i16) -> i16 {
    let ri = param_1;
    let mut party_count: i16 = -1; // local_10
    let mut best: i16 = -1; // local_16
    let mut best_dist: i16 = 99; // local_4
    let off = ros8(ri);
    let mut side = rest::combatant_side(gm, ri) as i16; // local_8
    if gm.gi8(g::STATUS_ICON) == b'C' as i8 {
        let v = get_combat_stat(gm, -1, ri) as i16;
        let r = rest::random_damage_1_30(gm);
        if v < r {
            side = 0;
        }
    }
    let mut tx: u16 = 0; // local_e  TODO(verify): uninitialised in the original when chaos_sword_passout supplies the target
    let mut ty: u16 = 0; // local_12
    let y = gm.g8(g::ROSTER_Y.wrapping_add(off));
    let x = gm.g8(g::ROSTER_X.wrapping_add(off));
    let mut i: i16 = 0x1f;
    let mut e: u16 = 0xbb0c; // roster entry 31
    loop {
        let ef = gm.g8(e.wrapping_add(2));
        if i != ri
            && ef != 0
            && (ef & 0x20) == 0
            && side != rest::combatant_side(gm, i) as i16
            && ((gm.gi8(g::LOCATION_BACKUP) == b'(' as i8
                || gm.gi8(g::ROSTER_MEMBER.wrapping_add(off)) == b'/' as i8
                || (ef & 0x10) == 0)
                && (ef & 4) == 0)
        {
            if i < 5 {
                party_count += 1;
            }
            let d = ovl::comsubs::cell_distance(gm, gm.g8(e.wrapping_add(7)) as u16, gm.g8(e.wrapping_add(6)) as u16, y as u16, x as u16);
            if d < best_dist {
                tx = gm.g8(e.wrapping_add(6)) as u16;
                ty = gm.g8(e.wrapping_add(7)) as u16;
                best = i;
                best_dist = d;
            }
        }
        e = e.wrapping_sub(8);
        i -= 1;
        if !(-1 < i) { break; }
    }
    if party_count == -1 && best == -1 {
        best = ovl::sjog::chaos_sword_passout(gm);
    }
    if best == -1 {
        let mut p: u16 = 0xbb0c;
        loop {
            let f = gm.g8(p.wrapping_add(2));
            if (f & 0x40) != 0 {
                gm.set_g8(p, 1);
                gm.set_g8(p.wrapping_add(2), f | 2);
            }
            p = p.wrapping_sub(8);
            if !(0xba3c < p) { break; }
        }
        tx = 5;
        ty = 5;
    }
    gm.set_gi16(g::DY, 0);
    gm.set_gi16(g::DX, 0);
    let rx = gm.g8(g::ROSTER_X.wrapping_add(off)) as u16;
    let ry = gm.g8(g::ROSTER_Y.wrapping_add(off)) as u16;
    if tx < rx {
        gm.set_gi16(g::DX, -1);
    }
    if rx < tx {
        gm.set_gi16(g::DX, 1);
    }
    if ty < ry {
        gm.set_gi16(g::DY, -1);
    }
    if ry < ty {
        gm.set_gi16(g::DY, 1);
    }
    if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 2) != 0 {
        gm.set_gi16(g::DX, gm.gi16(g::DX).wrapping_neg());
        gm.set_gi16(g::DY, gm.gi16(g::DY).wrapping_neg());
    }
    if best == ri {
        best = -1;
    }
    best
}

/// orig 1000:b174 (564 bytes). Movement for roster entry `ri`; returns non-zero when it moved
/// (1), 0 when it stayed. Types 0x1b and 0x1a never move. A monster whose type has flag 0x2000
/// (teleport) - unless the status icon is 'N' or 0x1c - teleports when it is surrounded or on
/// a 3/4 roll, to a random free cell from `random_combat_cell` that passes `combat_cell_free`
/// ("<name> teleports!"). Otherwise, unless surrounded: `find_nearest_enemy` sets g_dx/g_dy;
/// on a 50% roll (or when the horizontal step is blocked, `is_cell_blocked_for`) the vertical
/// step is tried (g_dx = 0, code 0x3e6), else the horizontal one (g_dy = 0, code 999); if the
/// chosen step is blocked up to 4 random directions (0 S, 1 E, 2 N, 3 W) are tried until one is
/// free (0x3df). When a step code remains non-zero the roster and monster-table position are
/// advanced by (g_dx, g_dy) and, when the new position is off the map, g_combat_state_flags =
/// 0x10.
pub fn monster_move(gm: &mut Game, param_1: i16) -> i16 {
    let ri = param_1;
    let mut moved: i16 = 0; // local_8
    let mut code: u16 = 0; // local_6
    let off = ros8(ri);
    let member = gm.gi8(g::ROSTER_MEMBER.wrapping_add(off));
    if member != 0x1b && member != 0x1a {
        let icon = gm.gi8(g::STATUS_ICON);
        if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) == 0
            && (gm.g16((member as u8 as u16).wrapping_mul(2).wrapping_add(g::MONSTER_FLAGS)) & 0x2000) != 0
            && icon != b'N' as i8
            && icon != 0x1c
            && (ovl::sjog::is_combatant_surrounded(gm, ri) || rest::rand_0_to(gm, 3) != 3)
            && random_combat_cell(gm) != 0
        {
            let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
            let tile = gm.g8(g::MONSTER_TABLE.wrapping_add(slot8(slot))) as u16;
            if combat_cell_free(gm, gm.g16(g::DY), gm.g16(g::DX), tile) != 0 {
                let nx = gm.gi16(g::DX) as u8;
                gm.set_g8(g::ROSTER_X.wrapping_add(off), nx);
                gm.set_g8(g::MONSTER_X.wrapping_add(slot8(slot)), nx);
                let ny = gm.gi16(g::DY) as u8;
                gm.set_g8(g::ROSTER_Y.wrapping_add(off), ny);
                gm.set_g8(g::MONSTER_Y.wrapping_add(slot8(slot)), ny);
                let name = gm.g16((member as u8 as u16).wrapping_mul(2).wrapping_add(g::CREATURE_NAME_PTRS));
                text::print_string(gm, name);
                text::print_string(gm, 0x6f0c); // " teleports!"
                moved = 1;
            }
        }
        if moved == 0 && !ovl::sjog::is_combatant_surrounded(gm, ri) {
            find_nearest_enemy(gm, ri);
            let r = rest::rand_0_to(gm, 0xff);
            let ry = gm.g8(g::ROSTER_Y.wrapping_add(off)) as i16;
            let rx = gm.g8(g::ROSTER_X.wrapping_add(off)) as i16;
            if r < 0x80 || ovl::sjog::is_cell_blocked_for(gm, ri, ry, rx.wrapping_add(gm.gi16(g::DX))) != 0 {
                if ovl::sjog::is_cell_blocked_for(gm, ri, ry.wrapping_add(gm.gi16(g::DY)), rx) == 0 {
                    gm.set_gi16(g::DX, 0);
                    code = 0x3e6;
                }
            } else {
                gm.set_gi16(g::DY, 0);
                code = 999;
            }
            if code < 0x3de {
                moved = 4;
                let by = gm.g8(g::ROSTER_Y.wrapping_add(off)) as u16;
                let bx = gm.g8(g::ROSTER_X.wrapping_add(off)) as u16;
                loop {
                    code = rest::rand_0_to(gm, 3) as u16;
                    if code == 0 {
                        gm.set_gi16(g::DY, 1);
                        gm.set_gi16(g::DX, 0);
                    } else if code == 1 {
                        gm.set_gi16(g::DX, 1);
                        gm.set_gi16(g::DY, 0);
                    } else if code == 2 {
                        gm.set_gi16(g::DY, -1);
                        gm.set_gi16(g::DX, 0);
                    } else if code == 3 {
                        gm.set_gi16(g::DX, -1);
                        gm.set_gi16(g::DY, 0);
                    }
                    let ny = (by as i16).wrapping_add(gm.gi16(g::DY));
                    let nx = (bx as i16).wrapping_add(gm.gi16(g::DX));
                    if ovl::sjog::is_cell_blocked_for(gm, ri, ny, nx) == 0 {
                        code = 0x3df;
                    }
                    if 0x3de < code as i16 { break; }
                    gm.set_gi16(g::DY, 0);
                    gm.set_gi16(g::DX, 0);
                    moved -= 1;
                    if !(0 < moved) { break; }
                }
            }
            if code != 0 {
                let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
                let nx = gm.g8(g::ROSTER_X.wrapping_add(off)).wrapping_add(gm.gi16(g::DX) as u8);
                gm.set_g8(g::ROSTER_X.wrapping_add(off), nx);
                gm.set_g8(g::MONSTER_X.wrapping_add(slot8(slot)), nx);
                let ny = gm.g8(g::ROSTER_Y.wrapping_add(off)).wrapping_add(gm.gi16(g::DY) as u8);
                gm.set_g8(g::ROSTER_Y.wrapping_add(off), ny);
                gm.set_g8(g::MONSTER_Y.wrapping_add(slot8(slot)), ny);
                moved = 1;
                if rest::combat_pos_in_bounds(gm, ny as i16, nx as i16) == 0 {
                    gm.set_g8(g::COMBAT_STATE_FLAGS, 0x10);
                }
            }
        }
    }
    moved
}

/// orig 1000:b3aa (243 bytes). Ghidra order (y, x). Dungeon-room trigger check, only when
/// g_combat_flags has 0x80 or 2 (room combat). The room record loaded at g_light_map holds 8
/// triggers: trigger position x/y at 0xae1f/0xae27, replacement tile at 0xad1f, and two target
/// cells (0xae3f/0xae47 and 0xae5f/0xae67). For every trigger at (x,y) the trigger is consumed
/// (position set to 0xff) and each target cell inside the 11x11 map is set to the replacement
/// tile. Returns 1 (after a view refresh) when any trigger fired, else 0.
pub fn check_room_trigger(gm: &mut Game, param_1: u16, param_2: u16) -> i16 {
    let (y, x) = (param_1, param_2);
    let mut fired: i16 = 0;
    let cf = gm.g8(g::COMBAT_FLAGS);
    if (cf & 0x80) != 0 || (cf & 2) != 0 {
        let mut i: u16 = 0;
        loop {
            let tx = 0xae1fu16.wrapping_add(i); // trigger x
            let ty = 0xae27u16.wrapping_add(i); // trigger y
            if gm.g8(tx) as u16 == x && gm.g8(ty) as u16 == y {
                gm.set_g8(ty, 0xff);
                gm.set_g8(tx, 0xff);
                let tile = gm.g8(0xad1fu16.wrapping_add(i));
                let x1 = gm.g8(0xae3fu16.wrapping_add(i)) as u16;
                let y1 = gm.g8(0xae47u16.wrapping_add(i)) as u16;
                if x1 < 0xb && y1 < 0xb {
                    gm.set_g8(g::LIGHT_MAP.wrapping_add(y1.wrapping_mul(0x20)).wrapping_add(x1), tile);
                }
                let x2 = gm.g8(0xae5fu16.wrapping_add(i)) as u16;
                let y2 = gm.g8(0xae67u16.wrapping_add(i)) as u16;
                if x2 < 0xb && y2 < 0xb {
                    gm.set_g8(g::LIGHT_MAP.wrapping_add(y2.wrapping_mul(0x20)).wrapping_add(x2), tile);
                }
                fired = 1;
            }
            i += 1;
            if i == 8 { break; }
        }
        if fired != 0 {
            rest::refresh_map_view(gm);
        }
    }
    fired
}

/// orig 1000:b49e (40 bytes). g_dx = rand(0..15), g_dy = rand(0..15); returns 1 when both are
/// inside the 11x11 combat map (< 11), else 0. Used for teleports and by CAST/CAST2.
pub fn random_combat_cell(gm: &mut Game) -> u16 {
    let dx = rest::rand_0_to(gm, 0xf);
    gm.set_gi16(g::DX, dx);
    let dy = rest::rand_0_to(gm, 0xf);
    gm.set_gi16(g::DY, dy);
    if gm.gi16(g::DX) < 0xb && gm.gi16(g::DY) < 0xb { 1 } else { 0 }
}

/// orig 1000:b4c6 (121 bytes). Removes a combatant. `arg` < 0 names roster entry (-1 - arg):
/// the entry's slot is read and all its bytes (+4,+1,+0,+7,+6,+5,+2) zeroed; `arg` >= 0 names
/// monster-table slot `arg` - 1 directly. Then the monster-table slot's x, anim tile, tile,
/// value1, z and y are zeroed (the slot becomes free). Vector 1778:0602.
pub fn remove_combatant(gm: &mut Game, param_1: i16) {
    let slot: u16;
    if param_1 < 0 {
        let off = ros8((-1i16).wrapping_sub(param_1));
        slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
        gm.set_g8(g::ROSTER_SLOT.wrapping_add(off), 0);
        gm.set_g8(g::ROSTER_INITIATIVE.wrapping_add(off), 0);
        gm.set_g8(g::COMBAT_ROSTER.wrapping_add(off), 0);
        gm.set_g8(g::ROSTER_Y.wrapping_add(off), 0);
        gm.set_g8(g::ROSTER_X.wrapping_add(off), 0);
        gm.set_g8(g::ROSTER_TURN_COUNTER.wrapping_add(off), 0);
        gm.set_g8(g::ROSTER_FLAGS.wrapping_add(off), 0);
    } else {
        slot = (param_1 as u16).wrapping_sub(1);
    }
    let so = slot8(slot);
    gm.set_g8(g::MONSTER_X.wrapping_add(so), 0);
    gm.set_g8(g::MONSTER_TABLE.wrapping_add(so).wrapping_add(1), 0);
    gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), 0);
    gm.set_g8(g::MONSTER_Z.wrapping_add(so).wrapping_add(1), 0);
    gm.set_g8(g::MONSTER_Z.wrapping_add(so), 0);
    gm.set_g8(g::MONSTER_Y.wrapping_add(so), 0);
}

/// orig 1000:b540 (231 bytes). Ghidra order (defender, attacker). Rolls the damage of an
/// attack. Party-side attacker (flag 0x40 clear) uses g_attack_weapon: 0x27 glass sword ->
/// "Thy sword hath shattered!", the sword is unequipped (`remove_equipped_item`) and damage is
/// 99 (instant kill); 0x28 -> 0; 0xff -> 1; else g_item_attack[weapon], rolled 1..value when
/// the value is > 1 and not 99. A monster attacker uses g_monster_stats[type]+3 (0x13c0). 99
/// stays 99; otherwise the defender's armour (party: record +0x18 at 0x55c0; monster:
/// g_monster_stats+2 at 0x13bf), when non-zero, subtracts rand(1..armour). Returns the (possibly
/// negative) damage.
pub fn roll_attack_damage(gm: &mut Game, param_1: i16, param_2: i16) -> i16 {
    let (defender, attacker) = (param_1, param_2);
    let aoff = ros8(attacker);
    let mut dmg: u16; // local_8
    if (gm.g8(g::ROSTER_FLAGS.wrapping_add(aoff)) & 0x40) == 0 {
        let w = gm.g8(g::ATTACK_WEAPON) as u16;
        if w == 0x27 {
            text::print_string(gm, 0x6f1a); // "Thy sword hath shattered!"
            rest::remove_equipped_item(gm, 0x27, attacker);
            dmg = 99;
        } else if w == 0x28 {
            dmg = 0;
        } else if w == 0xff {
            dmg = 1;
        } else {
            dmg = gm.g8(w.wrapping_add(g::ITEM_ATTACK)) as u16;
            if 1 < gm.g8(w.wrapping_add(g::ITEM_ATTACK)) && dmg != 99 {
                dmg = util::rand_range(gm, dmg as i16, 1) as u16;
            }
        }
    } else {
        let t = gm.g8(g::ROSTER_MEMBER.wrapping_add(aoff)) as u16;
        dmg = gm.g8(t.wrapping_mul(8).wrapping_add(0x13c0)) as u16; // g_monster_stats +3
    }
    let result: i16;
    if dmg == 99 {
        result = 99;
    } else {
        let doff = ros8(defender);
        let m = gm.g8(g::ROSTER_MEMBER.wrapping_add(doff)) as u16;
        let armour: i8 = if (gm.g8(g::ROSTER_FLAGS.wrapping_add(doff)) & 0x40) == 0 {
            gm.gi8(mem32(m).wrapping_add(0x55c0)) // party record +0x18
        } else {
            gm.gi8(m.wrapping_mul(8).wrapping_add(0x13bf)) // g_monster_stats +2
        };
        let mut r = dmg as i16;
        if armour != 0 {
            let a = util::rand_range(gm, armour as i16, 1);
            r = (dmg as i16).wrapping_sub(a);
        }
        result = r;
    }
    result
}

/// orig 1000:b62a (70 bytes). Initiative-type stat of roster entry `ri` for the hit roll:
/// returns 1 (defenceless) when the status icon is 'T' and the entry is a monster (flag 0x40),
/// when the entry is type 0x1a, or when it is asleep (flag 8); otherwise the entry's
/// initiative byte (+1).
pub fn combatant_dexterity(gm: &mut Game, param_1: i16) -> u8 {
    let off = ros8(param_1);
    let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(off));
    if (gm.gi8(g::STATUS_ICON) == b'T' as i8 && (f & 0x40) != 0)
        || gm.gi8(g::ROSTER_MEMBER.wrapping_add(off)) == 0x1a
        || (f & 8) != 0
    {
        1
    } else {
        gm.g8(g::ROSTER_INITIATIVE.wrapping_add(off))
    }
}

/// orig 1000:b672 (242 bytes). Ghidra order (kind, ri). Combat stat lookup for roster entry
/// `ri`. `kind` >= 1 is a weapon id for a party entry: items whose 0x169c class byte is 8
/// select kind -3 (strength), others -2 (dexterity); for a monster entry kind != 0 is kept and
/// kind 0 selects -3 when the low byte of g_monster_flags has 0x80, else -2. Then: -4 ->
/// `compute_member_defense(member)` for party, g_monster_stats+2 (0x13bf) for monsters; -3 ->
/// party record +0x0c (strength, 0x55b4) / 0x13bc (stats byte before the type's record);
/// -2 -> `combatant_dexterity`; -1 -> record +0x0e (0x55b6) / 0x13be (stats +1); any other
/// kind returns an uninitialised local (0 here).
pub fn get_combat_stat(gm: &mut Game, param_1: i16, param_2: i16) -> u16 {
    let (mut kind, ri) = (param_1, param_2);
    let off = ros8(ri);
    let is_monster = (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x40) != 0;
    let m = gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) as u16;
    let mut to_m3 = false;
    if !is_monster {
        if kind >= 1 {
            if gm.gi8((kind as u16).wrapping_add(0x169c)) != 8 {
                kind = -2;
            } else {
                to_m3 = true;
            }
        }
    } else if kind == 0 {
        if (gm.g8(m.wrapping_mul(2).wrapping_add(g::MONSTER_FLAGS)) & 0x80) == 0 {
            kind = -2;
        } else {
            to_m3 = true;
        }
    }
    if to_m3 {
        kind = -3;
    }
    // LAB_1000_b6bd
    let b: u8;
    if kind == -4 {
        if !is_monster {
            return rest::compute_member_defense(gm, m as i16);
        }
        b = gm.g8(m.wrapping_mul(8).wrapping_add(0x13bf));
    } else if kind == -3 {
        b = if !is_monster { gm.g8(mem32(m).wrapping_add(0x55b4)) } else { gm.g8(m.wrapping_mul(8).wrapping_add(0x13bc)) };
    } else if kind == -2 {
        return combatant_dexterity(gm, ri) as u16;
    } else if kind != -1 {
        return 0; // TODO(verify): uninitialised local_6 returned in the original
    } else {
        b = if !is_monster { gm.g8(mem32(m).wrapping_add(0x55b6)) } else { gm.g8(m.wrapping_mul(8).wrapping_add(0x13be)) };
    }
    b as u16
}

/// orig 1000:b766 (157 bytes). Ghidra order (weapon, unused, attacker, defender). The hit roll.
/// Automatic hit (1) for weapons 0x27 (glass sword), 0x23 (chaos sword) and 0x28 outside spell
/// mode, and for spells 0x2a..0x31 or 0x33 in spell mode (g_spell_mode != 0). Otherwise the
/// defender stat is `get_combat_stat(-2, defender)` (dexterity; -1 in spell mode) and the
/// attacker stat `get_combat_stat(weapon, attacker)` (-1 in spell mode): hit when
/// random_damage_1_30() >= (defender - attacker + 30) / 2. Returns 1 hit, 0 miss.
pub fn attack_hits(gm: &mut Game, param_1: i16, param_2: u16, param_3: u16, param_4: u16) -> i16 {
    let (weapon, _unused, attacker, defender) = (param_1, param_2, param_3 as i16, param_4 as i16);
    let mut akind: i16 = -1; // local_a
    let mut dkind: i16 = 0; // local_8 (uninitialised when auto-hit)
    let mut hit: i16 = 0; // local_6
    let mut auto = true;
    if gm.g8(SPELL_MODE) == 0 {
        if weapon != 0x27 && weapon != 0x23 && weapon != 0x28 {
            dkind = -2;
            akind = weapon;
            auto = false;
        }
    } else if (weapon < 0x2a || 0x31 < weapon) && weapon != 0x33 {
        akind = -1;
        dkind = -1;
        auto = false;
    }
    if auto {
        hit = 1;
    }
    // LAB_1000_b7bf
    if hit == 0 {
        let d = get_combat_stat(gm, dkind, defender) as i16;
        let a = get_combat_stat(gm, akind, attacker) as i16;
        let roll = rest::random_damage_1_30(gm);
        if d.wrapping_sub(a).wrapping_add(0x1e) / 2 <= roll {
            hit = 1;
        }
    }
    hit
}

/// orig 1000:b804 (831 bytes). Ghidra order (amount, ri). Applies `amount` damage to roster
/// entry `ri`; returns the experience value of a monster it killed ((g_monster_stats+4 >> 2) +
/// 1), else 0. amount < 1 sets g_combat_state_flags = 0x20 and becomes 0. Monster entry: half
/// damage for undead types (flag 0x20) unless the attack is a spell, none for flag 8 types; HP
/// (+0) is reduced (floor 0). Death (HP 0 or amount 99): the entry is marked out (0x20); types
/// without flags 0x1001: type 0x1c leaves a 0x1f object and spawns a new type-0x1f combatant
/// (`add_combatant`) at its cell; type 0x1e turns the map cell into tile 0x4c; on a cell whose
/// combat-map tile is > 3 and not 0x87 it drops a corpse (0x1f) when the type's stats+6 value is
/// below a 1..30 roll, else a chest (tile 1, value1 = stats+6, trapped (bit 7) when a second
/// roll is below it) and keeps the slot; otherwise `remove_combatant(-1 - ri)`. Types with
/// 0x1001: flag 0x1000 prints "<name> vanishes!", g_combat_state_flags = 2, tile 0x16, a tile
/// transition, `remove_combatant(-(ri+1))` and `chaos_sword_passout`; else `remove_combatant
/// (-(ri+1))`. A surviving monster of a dividing type (flag 0x10) tries up to 8 random
/// adjacent cells (COMSUBS e9b4 -> g_dx/g_dy) that `combat_cell_free` accepts and
/// `add_combatant`s a copy ((tile - 0x40) >> 2 = type) with the same HP: "<name> divides!",
/// returns 0. Party entry: HP (record +0x10) -= amount; at <= 0 or amount 99: HP 0, flag 0x20,
/// status 'D', object tile 0x1e, and the active player is cleared when it was this member;
/// status window redrawn.
pub fn apply_damage(gm: &mut Game, param_1: u16, param_2: i16) -> i8 {
    let (mut amount, mut ri) = (param_1, param_2);
    let mut exp: i8 = 0;
    let off = ros8(ri);
    let hp_p = g::COMBAT_ROSTER.wrapping_add(off);
    if (amount as i16) < 1 {
        gm.set_g8(g::COMBAT_STATE_FLAGS, 0x20);
        amount = 0;
    }
    if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) == 0 {
        let t = gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) as u16;
        let mflags_lo = gm.g8(t.wrapping_mul(2).wrapping_add(g::MONSTER_FLAGS));
        if (mflags_lo & 0x20) != 0 && gm.g8(ATTACK_IS_SPELL) == 0 {
            amount = ((amount as i16) / 2) as u16;
        }
        if (mflags_lo & 8) != 0 {
            amount = 0;
        }
        let hp = gm.g8(hp_p);
        if (hp as u16) < amount {
            gm.set_g8(hp_p, 0);
        } else {
            gm.set_g8(hp_p, hp.wrapping_sub(amount as u8));
        }
        if gm.g8(hp_p) == 0 || amount == 99 {
            let st = t.wrapping_mul(8);
            exp = ((gm.g8(st.wrapping_add(0x13c1)) >> 2).wrapping_add(1)) as i8;
            let drop_val = gm.g8(st.wrapping_add(0x13c3));
            gm.set_g8(g::ROSTER_FLAGS.wrapping_add(off), 0x20);
            gm.set_g8(hp_p, 0);
            let x = gm.g8(g::ROSTER_X.wrapping_add(off)) as u16;
            let y = gm.g8(g::ROSTER_Y.wrapping_add(off)) as u16;
            if (gm.g16(t.wrapping_mul(2).wrapping_add(g::MONSTER_FLAGS)) & 0x1001) == 0 {
                if gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) == 0x1c {
                    let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)).wrapping_add(1), 0x1f);
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)), 0x1f);
                    rest::add_combatant(gm, gm.g8(g::PARTY_Z), y as u8, x as u8, 0, 0x1f);
                    rest::refresh_map_view(gm);
                    return exp;
                }
                if gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) == 0x1e {
                    let p = rest::get_map_tile(gm, y, x);
                    gm.set_g8(p, 0x4c);
                } else {
                    let cell = gm.g8(g::LIGHT_MAP.wrapping_add(y.wrapping_mul(0x20)).wrapping_add(x));
                    if cell != 0x87 && 3 < cell {
                        let roll = rest::random_damage_1_30(gm);
                        if (drop_val as i16) < roll {
                            let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
                            gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)).wrapping_add(1), 0x1f);
                            gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)), 0x1f);
                            return exp;
                        }
                        let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
                        let so = slot8(slot);
                        gm.set_g8(g::MONSTER_TABLE.wrapping_add(so).wrapping_add(1), 1);
                        gm.set_g8(g::MONSTER_TABLE.wrapping_add(so), 1);
                        gm.set_g8(g::SHIP_HULL.wrapping_add(so), drop_val); // value1
                        let roll2 = rest::random_damage_1_30(gm);
                        if (drop_val as i16) <= roll2 {
                            return exp;
                        }
                        let v = gm.g8(g::SHIP_HULL.wrapping_add(so)) | 0x80;
                        gm.set_g8(g::SHIP_HULL.wrapping_add(so), v);
                        return exp;
                    }
                }
                ri = (-1i16).wrapping_sub(ri);
            } else {
                let t2 = t.wrapping_mul(2);
                if (gm.g16(t2.wrapping_add(g::MONSTER_FLAGS)) & 0x1000) != 0 {
                    text::print_string(gm, gm.g16(t2.wrapping_add(g::CREATURE_NAME_PTRS)));
                    text::print_string(gm, 0x6f36); // " vanishes!"
                    gm.set_g8(g::COMBAT_STATE_FLAGS, 2);
                    let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)).wrapping_add(1), 0x16);
                    gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)), 0x16);
                    let ry = gm.g8(g::ROSTER_Y.wrapping_add(off)) as u16;
                    let rx = gm.g8(g::ROSTER_X.wrapping_add(off)) as u16;
                    let tile = gm.g8(g::LIGHT_MAP.wrapping_add(ry.wrapping_mul(0x20)).wrapping_add(rx));
                    gfx::tile_transition_effect(gm, ry as i16, rx as i16, tile as i16);
                    remove_combatant(gm, ri.wrapping_add(1).wrapping_neg());
                    ovl::sjog::chaos_sword_passout(gm);
                    return exp;
                }
                ri = ri.wrapping_add(1).wrapping_neg();
            }
            remove_combatant(gm, ri);
        } else if (gm.g8(t.wrapping_mul(2).wrapping_add(g::MONSTER_FLAGS)) & 0x10) != 0 {
            let mut n: i16 = 0;
            loop {
                let ry = gm.g8(g::ROSTER_Y.wrapping_add(off)) as i16;
                let rx = gm.g8(g::ROSTER_X.wrapping_add(off)) as i16;
                ovl::comsubs::random_cell_near(gm, ry, rx);
                let ny = gm.g16(g::DY);
                let nx = gm.g16(g::DX);
                let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
                let tile = gm.g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)));
                if combat_cell_free(gm, ny, nx, tile as u16) != 0 {
                    let ntype = ((tile as i16).wrapping_sub(0x40)) >> 2;
                    let ni = rest::add_combatant(gm, gm.g8(g::PARTY_Z), ny as u8, nx as u8, 0, ntype) as i16;
                    if -1 < ni {
                        let hp = gm.g8(hp_p);
                        gm.set_g8(g::COMBAT_ROSTER.wrapping_add(ros8(ni)), hp);
                        text::print_string(gm, gm.g16(t.wrapping_mul(2).wrapping_add(g::CREATURE_NAME_PTRS)));
                        text::print_string(gm, 0x6f42); // " divides!"
                        return 0;
                    }
                }
                n += 1;
                if !(n < 8) { break; }
            }
        }
    } else {
        let m = gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) as u16;
        let hp_off = mem32(m).wrapping_add(g::PARTY_HP);
        let nhp = gm.gi16(hp_off).wrapping_sub(amount as i16);
        gm.set_gi16(hp_off, nhp);
        if gm.gi16(hp_off) < 1 || amount == 99 {
            gm.set_gi16(hp_off, 0);
            let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) | 0x20;
            gm.set_g8(g::ROSTER_FLAGS.wrapping_add(off), f);
            gm.set_g8(g::PARTY_STATUS.wrapping_add(mem32(m)), 0x44); // 'D'
            let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)).wrapping_add(1), 0x1e);
            gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)), 0x1e);
            if gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) == gm.g8(g::ACTIVE_MEMBER) {
                gm.set_g8(g::ACTIVE_MEMBER, 0xff);
            }
        }
        rest::draw_status_window(gm);
    }
    exp
}

/// orig 1000:bb4a (143 bytes). Ghidra order (attacker, target). Poison attack (vector
/// 1778:076a): a party-member target in good health ('G') becomes 'P', "<name> is poisoned!"
/// is printed, g_combat_state_flags = 8 and the status window marked dirty. Any other target
/// takes rand(0..20) damage via `apply_damage`; when `attacker` >= 0 is a party member the
/// kill value is added to its experience (record +0x14, capped at 9999).
pub fn poison_attack(gm: &mut Game, param_1: i16, param_2: i16) {
    let (attacker, target) = (param_1, param_2);
    let toff = ros8(target);
    let rec = mem32(gm.g8(g::ROSTER_MEMBER.wrapping_add(toff)) as u16);
    if (gm.g8(g::ROSTER_FLAGS.wrapping_add(toff)) & 0x80) == 0
        || gm.gi8(g::PARTY_STATUS.wrapping_add(rec)) != b'G' as i8
    {
        let d = rest::rand_0_to(gm, 0x14);
        let v = apply_damage(gm, d as u16, target);
        if -1 < attacker && (gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(attacker))) & 0x80) != 0 {
            let arec = mem32(gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(attacker))) as u16);
            rest::add_i16_capped(gm, 0x270f, v as i16, arec.wrapping_add(0x55bc)); // exp, cap 9999
        }
    } else {
        gm.set_g8(g::PARTY_STATUS.wrapping_add(rec), 0x50); // 'P'
        text::print_string(gm, rec.wrapping_add(g::PARTY));
        text::print_string(gm, 0x6f4e); // " is poisoned!"
        gm.set_g8(g::COMBAT_STATE_FLAGS, 8);
        gm.set_g8(g::STATUS_DIRTY, 1);
    }
}

/// orig 1000:bbda (270 bytes). Ghidra order (attacker, target). Resolves a successful hit. A
/// monster attacker whose type has flags 0x204 (poisonous) poisons (`poison_attack`) on a
/// rand(0..3) != 0, as does the poison spell (0x33) in spell mode. Monster type 0x1c puts an
/// awake target to sleep (`incapacitate_combatant`); the sleep spell (0x34) in spell mode does
/// the same. Otherwise `roll_attack_damage(target, attacker)`: a negative result on a party
/// target sets g_combat_state_flags = 0x20 (absorbed); else `apply_damage` and, for a party
/// attacker, the kill value is added to its experience (cap 9999).
pub fn deal_attack_damage(gm: &mut Game, param_1: i16, param_2: i16) {
    let (attacker, target) = (param_1, param_2);
    let aoff = ros8(attacker);
    let toff = ros8(target);
    let is_monster = (gm.g8(g::ROSTER_FLAGS.wrapping_add(aoff)) & 0x80) == 0;
    let t = gm.g8(g::ROSTER_MEMBER.wrapping_add(aoff)) as u16;
    if !is_monster
        || (gm.g16(t.wrapping_mul(2).wrapping_add(g::MONSTER_FLAGS)) & 0x204) == 0
        || util::rand_range(gm, 3, 0) == 0
    {
        let mut slept = false;
        if is_monster
            && gm.g8(g::ROSTER_MEMBER.wrapping_add(aoff)) == 0x1c
            && (gm.g8(g::ROSTER_FLAGS.wrapping_add(toff)) & 8) == 0
        {
            rest::incapacitate_combatant(gm, target as u16);
            slept = true;
        }
        if slept {
            return;
        }
        if gm.g8(ATTACK_IS_SPELL) != 0 && gm.g8(g::ATTACK_WEAPON) == 0x34 {
            rest::incapacitate_combatant(gm, target as u16);
            return;
        }
        if gm.g8(ATTACK_IS_SPELL) == 0 || gm.g8(g::ATTACK_WEAPON) != 0x33 {
            let dmg = roll_attack_damage(gm, target, attacker);
            if dmg < 0 && (gm.g8(g::ROSTER_FLAGS.wrapping_add(toff)) & 0x80) != 0 {
                gm.set_g8(g::COMBAT_STATE_FLAGS, 0x20);
                return;
            }
            let v = apply_damage(gm, dmg as u16, target);
            if (gm.g8(g::ROSTER_FLAGS.wrapping_add(aoff)) & 0x80) == 0 {
                return;
            }
            let arec = mem32(gm.g8(g::ROSTER_MEMBER.wrapping_add(aoff)) as u16);
            rest::add_i16_capped(gm, 0x270f, v as i16, arec.wrapping_add(0x55bc));
            return;
        }
    }
    poison_attack(gm, attacker, target);
}

/// orig 1000:bcec (194 bytes). Morale check for roster entry `ri`; sets or clears the flee
/// flag (bit 1) and returns a morale class. Party entries return their roster entry offset
/// unchanged. With q = g_monster_stats[type]+4 >> 2 (a quarter of the base HP): HP < q ->
/// class 1 and flee; HP >= 2q -> class 3 when HP < 3q, else 4, no flee; q <= HP < 2q -> class
/// 2, flee only when rand(0..256) >= 0xfc.
pub fn monster_morale(gm: &mut Game, param_1: i16) -> u16 {
    let off = ros8(param_1);
    let hp_p = g::COMBAT_ROSTER.wrapping_add(off);
    if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) != 0 {
        return hp_p;
    }
    let t = gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) as u16;
    let mut q = (gm.g8(t.wrapping_mul(8).wrapping_add(0x13c1)) >> 2) as u16;
    let hp = gm.g8(hp_p) as u16;
    let result: u16;
    let mut flee = false;
    if hp < q {
        result = 1;
        flee = true;
    } else {
        q <<= 1;
        if q <= hp {
            result = if hp < (q / 2).wrapping_mul(3) { 3 } else { 4 };
        } else {
            result = 2;
            if !(rest::rand_0_to(gm, 0x100) < 0xfc) {
                flee = true;
            }
        }
    }
    // LAB_1000_bd8f
    let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(off));
    if flee {
        gm.set_g8(g::ROSTER_FLAGS.wrapping_add(off), f | 2);
    } else {
        gm.set_g8(g::ROSTER_FLAGS.wrapping_add(off), f & 0xfd);
    }
    result
}

/// orig 1000:bdae (325 bytes). Hazard check for the cell roster entry `ri` stands on, run
/// after its turn. Combat-map tile 0x8f or 0xbc (lava/fire) -> hazard 100; tile 4 (swamp) ->
/// 50; otherwise the first other monster-table object at the same cell decides: 0xea (fire
/// field) 100, 0xe8 (poison field) 50, 0xe9 (sleep field) 150. 50: unless the entry's own
/// object tile is >= 0x80 (flying/field creature) `poison_attack(-1, ri)` and a flash; 100:
/// flash, rand(0..10) damage via `apply_damage`, COMSUBS e4f2(0xff, ri), status dirty; 150:
/// `incapacitate_combatant(ri)`.
pub fn apply_cell_hazard(gm: &mut Game, param_1: i16) {
    let ri = param_1;
    let off = ros8(ri);
    let mut hazard: i16 = 0; // local_a
    let rx = gm.g8(g::ROSTER_X.wrapping_add(off)) as u16;
    let ry = (gm.g16(g::ROSTER_Y.wrapping_add(off)) & 0xff).wrapping_mul(0x20);
    let c = gm.gi8(g::LIGHT_MAP.wrapping_add(rx).wrapping_add(ry));
    if c == -0x71 || c == -0x44 {
        hazard = 100;
    }
    if c == 4 {
        hazard = 0x32;
    }
    if hazard == 0 {
        let my_slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off));
        let mut slot: u16 = 0;
        loop {
            let so = slot8(slot);
            if slot != my_slot as u16
                && gm.g8(g::ROSTER_X.wrapping_add(off)) == gm.g8(g::MONSTER_X.wrapping_add(so))
                && gm.g8(g::ROSTER_Y.wrapping_add(off)) == gm.g8(g::MONSTER_Y.wrapping_add(so))
            {
                let t = gm.gi8(g::MONSTER_TABLE.wrapping_add(so));
                if t == -0x16 {
                    hazard = 100;
                }
                if t == -0x18 {
                    hazard = 0x32;
                }
                if t == -0x17 {
                    hazard = 0x96;
                }
                if hazard != 0 { break; }
            }
            slot += 1;
            if !((slot as i16) < 0x20) { break; }
        }
    }
    if hazard == 0x32 {
        let my_slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
        if gm.g8(g::MONSTER_TABLE.wrapping_add(slot8(my_slot))) < 0x80 {
            poison_attack(gm, -1, ri);
            rest::flash_monster(gm, ri as u16);
        }
    } else if hazard == 100 {
        rest::flash_monster(gm, ri as u16);
        let d = rest::rand_0_to(gm, 10);
        apply_damage(gm, d as u16, ri);
        ovl::comsubs::print_hit_result(gm, 0xff, ri);
        gm.set_g8(g::STATUS_DIRTY, 1);
    } else if hazard == 0x96 {
        rest::incapacitate_combatant(gm, ri as u16);
    }
}

/// orig 1000:bef6 (128 bytes). Escape roll for a swallowed roster entry `ri` (flag 4): when
/// random_damage_1_30() < the entry's initiative byte, "<name> regurgitated!" (party name from
/// the record, monster name from g_creature_name_ptrs) is printed with a noise, flag 4 is
/// cleared and the entry's object anim tile (+1) is restored from its tile (+0).
pub fn regurgitate_check(gm: &mut Game, param_1: i16) {
    let ri = param_1;
    let off = ros8(ri);
    let roll = rest::random_damage_1_30(gm) as u16;
    if roll < gm.g8(g::ROSTER_INITIATIVE.wrapping_add(off)) as u16 {
        let m = gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) as u16;
        let name: u16 = if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) == 0 {
            gm.g16(m.wrapping_mul(2).wrapping_add(g::CREATURE_NAME_PTRS))
        } else {
            mem32(m).wrapping_add(g::PARTY)
        };
        text::print_string(gm, name);
        text::print_string(gm, 0x6f5e); // " regurgitated!"
        util::play_random_tones(gm, 600, 7000, 1);
        let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0xfb;
        gm.set_g8(g::ROSTER_FLAGS.wrapping_add(off), f);
        let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
        let tile = gm.g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)));
        gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot8(slot)).wrapping_add(1), tile);
    }
}
