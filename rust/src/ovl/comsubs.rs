//! COMSUBS.OVL (translated)
//!
//! Level-4 overlay with the combat subroutines shared by COMBAT, CAST, CMDS, SJOG and MAINOUT:
//! the evasion roll (`target_evades`), the combatant name / "missed!" / after-hit result
//! messages (`print_combatant_name`, `print_missed`, `print_hit_result`), monster special
//! actions (possession, invisibility, daemon gating: `monster_special_action`), field
//! dissipation (`dissipate_fields`), distance helpers (`dist_squared`, `cell_distance`,
//! `roster_distance`), the crosshair aiming loop (`aim_crosshair`), roster lookup by cell
//! (`find_combatant_at`), random neighbour cells (`random_cell_near`), the missile / projectile
//! animation over the view (`animate_missile`, `animate_projectile`, `trace_line`,
//! `draw_missile`), ammunition bookkeeping (`use_missile_ammo`), monster interference
//! (`check_interference`), and the Attack command chain (`attack_command`,
//! `attack_with_equipped`, `attack_with`, `ranged_attack`, `melee_attack`).
//!
//! Ghidra split the single aiming loop `aim_crosshair` (e6e4) into seven extra "functions"
//! (e7a2, e84c, e856, e860, e8ca, e8d2, e900) because the SHOPPES2 vector 1778:09fe lands on
//! e84c (in SHOPPES2.OVL) and because of the jump tangle; their code is translated inside
//! `aim_crosshair` and the fragments are documented no-ops.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// Combat-roster entry offset (8 bytes per entry at 0xba14).
#[inline] fn ros8(i: i16) -> u16 { (i as u16).wrapping_mul(8) }
/// DGROUP 0x588f (save 0x2E9): spell-mode flag (first byte of g::SAVE_UNK_2E9); non-zero while
/// the current attack is a cast spell (its value scales the confirmation tone).
const SPELL_MODE: u16 = 0x588f;
/// DGROUP 0x5890 (save 0x2EA): "the current attack is a spell" flag (second byte of
/// g::SAVE_UNK_2E9), set by `attack_with` for weapon ids > 0x22.
const ATTACK_IS_SPELL: u16 = 0x5890;
/// DGROUP 0xa728: x pixel coordinates of the traced missile path (0xff terminated, 256+ bytes).
const MISSILE_PATH_X: u16 = 0xa728;
/// DGROUP 0xa872: y pixel coordinates of the traced missile path (0xff terminated).
const MISSILE_PATH_Y: u16 = 0xa872;

/// orig 1000:e1e0 (86 bytes). Ghidra order (kind, target, attacker). Evasion roll shared by
/// COMBAT-style attacks and CAST spells: for kinds other than 0x30, 0x31 and anything >= 0x33
/// (those always land) it takes the dexterity-ish stat (`get_combat_stat(-1, ..)`) of attacker
/// and target and returns 1 when `random_damage_1_30()` < (target - attacker + 30) / 2, i.e. the
/// target evades; otherwise 0. Vector 1778:099e (CAST c3be/c920/c9dc/cc18/dee0, e2d4).
pub fn target_evades(gm: &mut Game, param_1: i16, param_2: u16, param_3: u16) -> u16 {
    let (kind, target, attacker) = (param_1, param_2, param_3);
    if kind != 0x30 && kind != 0x31 && kind < 0x33 {
        let a = ovl::combat::get_combat_stat(gm, -1, attacker as i16) as i16;
        let d = ovl::combat::get_combat_stat(gm, -1, target as i16) as i16;
        let r = rest::random_damage_1_30(gm);
        if r < d.wrapping_sub(a).wrapping_add(0x1e) / 2 {
            return 1;
        }
    }
    0
}

/// orig 1000:e236 (61 bytes). Walks the 32 monster-table slots; every field object (tile & 0xfc
/// == 0xe8, i.e. 0xe8..0xeb poison/sleep/fire/energy fields) dissipates with probability 16/256
/// per call: `remove_combatant(slot + 1)`. Called by SJOG dfbe (combat aftermath).
pub fn dissipate_fields(gm: &mut Game) {
    let mut i: i16 = 0;
    let mut p = g::MONSTER_TABLE;
    loop {
        if (gm.g8(p) & 0xfc) == 0xe8 {
            if util::rand_range(gm, 0xff, 0) < 0x10 {
                ovl::combat::remove_combatant(gm, i.wrapping_add(1));
            }
        }
        p = p.wrapping_add(8);
        i = i.wrapping_add(1);
        if !(i < 0x20) { break; }
    }
}

/// orig 1000:e274 (61 bytes). Prints the name of roster entry `ri`: a party member (flags 0x80)
/// prints its record name at 0x55a8 + member*0x20, a monster prints g_creature_name_ptrs[type]
/// (0x1856). Vector 1778:061a.
pub fn print_combatant_name(gm: &mut Game, param_1: i16) {
    let off = ros8(param_1);
    let m = gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) as u16;
    let s = if (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) == 0 {
        gm.g16(m.wrapping_mul(2).wrapping_add(0x1856)) // g_creature_name_ptrs[type]
    } else {
        m.wrapping_mul(0x20).wrapping_add(0x55a8) // party record name
    };
    text::print_string(gm, s);
}

/// orig 1000:e2b2 (32 bytes). Miss message: outside spell mode (0x588f == 0) prints the
/// target's name followed by "missed!" (0x99aa); in spell mode prints "Failed!" (0x99a0).
pub fn print_missed(gm: &mut Game, param_1: u16) {
    let s;
    if gm.g8(SPELL_MODE) == 0 {
        print_combatant_name(gm, param_1 as i16);
        s = 0x99aa; // "missed!"
    } else {
        s = 0x99a0; // "Failed!"
    }
    text::print_string(gm, s);
}

/// orig 1000:e2d4 (539 bytes). Monster special action for roster entry `ri` at the start of its
/// turn; returns 1 when it used its turn, 0 otherwise. Nothing happens for party members or
/// while the status icon is 0x1c or 'N' (protection). With g_monster_flags bit 0x40
/// (possession) a random roster entry 0..31 is picked; if it is a party member with none of
/// flags 0x3d and `target_evades(0, t, ri)` fails, the member gets flag 1 (charmed), loses the
/// active-member slot (0xff) if it was active, the status window is redrawn, "possessed!"
/// (0x99b4) is printed with a sweep noise and a type-0x26 attacker removes itself
/// (`remove_combatant(-1 - ri)`); either way the turn is used. Otherwise bit 0x800 with a 32/256
/// roll toggles invisibility: anim tile 0 -> "reappears!" (0x99c2, flag 0x10 cleared, anim tile
/// restored from the tile) else "disappears!" (0x99ce, flag 0x10 set, anim tile 0). Otherwise
/// bit 0x400 with a 32/256 roll gates in a daemon: `random_combat_cell` must succeed,
/// `combat_cell_free(g_dy, g_dx, 0xd8)` and `add_combatant(z, g_dy, g_dx, 0, 0x26)` must
/// succeed; "gates in a daemon!" (0x99dc) with a noise, the new slot's tile/anim tile are set
/// to 0x16 during `tile_transition_effect(g_dy, g_dx, 0x1d8)` then to 0xd8. Vector 1778:0626.
pub fn monster_special_action(gm: &mut Game, param_1: i16) -> u16 {
    let ri = param_1;
    let off = ros8(ri);
    let mtype = gm.g8(g::ROSTER_MEMBER.wrapping_add(off)) as u16;
    let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
    let icon = gm.gi8(g::STATUS_ICON);
    if icon == 0x1c || icon == b'N' as i8 || (gm.g8(g::ROSTER_FLAGS.wrapping_add(off)) & 0x80) != 0 {
        return 0;
    }
    let mflags = gm.g16(g::MONSTER_FLAGS.wrapping_add(mtype.wrapping_mul(2)));
    if (mflags & 0x40) != 0 {
        let t = util::rand_range(gm, 0x1f, 0);
        let toff = ros8(t);
        let tf = gm.g8(g::ROSTER_FLAGS.wrapping_add(toff));
        if (tf & 0x80) != 0 && (tf & 0x3d) == 0 {
            if target_evades(gm, 0, t as u16, ri as u16) == 0 {
                text::put_char(gm, 10);
                let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(toff));
                gm.set_g8(g::ROSTER_FLAGS.wrapping_add(toff), f | 1);
                if gm.g8(g::ROSTER_MEMBER.wrapping_add(toff)) == gm.g8(g::ACTIVE_MEMBER) {
                    gm.set_g8(g::ACTIVE_MEMBER, 0xff);
                }
                rest::draw_status_window(gm);
                print_combatant_name(gm, t);
                text::print_string(gm, 0x99b4); // "possessed!"
                util::play_sweep_noise(gm, 2, 1000, 0x752e + 2, 1, 0xc1c);
                if mtype == 0x26 {
                    ovl::combat::remove_combatant(gm, (-1i16).wrapping_sub(ri));
                }
            }
            return 1;
        }
    }
    // LAB_1000_e3aa
    if (mflags & 0x800) != 0 && util::rand_range(gm, 0xff, 0) < 0x20 {
        text::put_char(gm, 10);
        print_combatant_name(gm, ri);
        let tile_p = g::MONSTER_TABLE.wrapping_add(slot.wrapping_mul(8));
        let anim_p = tile_p.wrapping_add(1);
        if gm.gi8(anim_p) == 0 {
            text::print_string(gm, 0x99c2); // "reappears!"
            let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(off));
            gm.set_g8(g::ROSTER_FLAGS.wrapping_add(off), f & 0xef);
            let tile = gm.g8(tile_p);
            gm.set_g8(anim_p, tile);
        } else {
            text::print_string(gm, 0x99ce); // "disappears!"
            let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(off));
            gm.set_g8(g::ROSTER_FLAGS.wrapping_add(off), f | 0x10);
            gm.set_g8(anim_p, 0);
        }
        return 1;
    }
    if (mflags & 0x400) != 0 && util::rand_range(gm, 0xff, 0) < 0x20 && ovl::combat::random_combat_cell(gm) != 0 {
        let (dy, dx) = (gm.gi16(g::DY), gm.gi16(g::DX));
        if ovl::combat::combat_cell_free(gm, dy as u16, dx as u16, 0xd8) != 0 {
            let z = gm.g8(g::PARTY_Z);
            let n = rest::add_combatant(gm, z, dy as u8, dx as u8, 0, 0x26) as i16;
            if n != -1 {
                text::put_char(gm, 10);
                print_combatant_name(gm, ri);
                text::print_string(gm, 0x99dc); // "gates in a daemon!"
                util::play_sweep_noise(gm, 0xf, 1000, 0x137e + 10, 1, 0xac8);
                let s = gm.g8(g::ROSTER_SLOT.wrapping_add(ros8(n))) as u16;
                let sp = g::MONSTER_TABLE.wrapping_add(s.wrapping_mul(8));
                gm.set_g8(sp.wrapping_add(1), 0x16);
                gm.set_g8(sp, 0x16);
                let (dy, dx) = (gm.gi16(g::DY), gm.gi16(g::DX));
                gfx::tile_transition_effect(gm, dy, dx, 0x1d8);
                gm.set_g8(sp.wrapping_add(1), 0xd8);
                gm.set_g8(sp, 0xd8);
                return 1;
            }
        }
    }
    0
}

/// orig 1000:e4f2 (319 bytes). Ghidra order (attacker, target). After-hit messages driven by
/// g_combat_state_flags: bit 0 is cleared first; if bit 0x20 was set the target "grazed!"
/// (0x99f2) with a tone sweep; if bits 0x22 remain set they are cleared and nothing else is
/// printed. Target flags 0 or 0x20 (gone) -> name + "killed!" (0x99fc) and bit 0 set. Else bit 4
/// -> name + "slept!" (0x9a06); bit 8 -> no message; otherwise name + for monsters the
/// `monster_morale` grade 1..4 ("critical!" 0x9a64, "heavily wounded!" 0x9a50, "lightly
/// wounded!" 0x9a3c, "barely wounded!" 0x9a2a; other grades print nothing) and for party
/// members "hit!" (0x9a22) - unless the attacker (not 0xff) is monster type '-' (0x2d, a sea
/// creature): "dragged under!" (0x9a10), tone, target flag 4 (swallowed), its slot's anim tile
/// 0, `animate_wait(4)`. A party-member target finally redraws the status window and the map
/// view; bits 0xc are cleared. Vector 1778:0632 (CAST c89e/dee0, ec48, edd8, COMBAT).
pub fn print_hit_result(gm: &mut Game, param_1: i16, param_2: i16) {
    let (attacker, target) = (param_1, param_2);
    let toff = ros8(target);
    let tflags = gm.g8(g::ROSTER_FLAGS.wrapping_add(toff));
    let st = gm.g8(g::COMBAT_STATE_FLAGS);
    let b1 = st & 0xfe;
    let grazed = st & 0x20;
    gm.set_g8(g::COMBAT_STATE_FLAGS, b1);
    if grazed != 0 {
        print_combatant_name(gm, target);
        text::print_string(gm, 0x99f2); // "grazed!"
        rest::play_tone_sweep(gm, 0x28, 1, 2000, 0x4b0);
    }
    if (gm.g8(g::COMBAT_STATE_FLAGS) & 0x22) != 0 {
        let v = gm.g8(g::COMBAT_STATE_FLAGS) & 0xdd;
        gm.set_g8(g::COMBAT_STATE_FLAGS, v);
        return;
    }
    'end: {
        if tflags == 0 || (tflags & 0x20) != 0 {
            print_combatant_name(gm, target);
            text::print_string(gm, 0x99fc); // "killed!"
            let v = gm.g8(g::COMBAT_STATE_FLAGS) | 1;
            gm.set_g8(g::COMBAT_STATE_FLAGS, v);
        } else {
            let s: u16;
            if (gm.g8(g::COMBAT_STATE_FLAGS) & 4) == 0 {
                if (gm.g8(g::COMBAT_STATE_FLAGS) & 8) != 0 {
                    break 'end;
                }
                print_combatant_name(gm, target);
                if (tflags & 0x80) == 0 {
                    let m = ovl::combat::monster_morale(gm, target) as i16;
                    s = if m == 1 {
                        0x9a64 // "critical!"
                    } else if m == 2 {
                        0x9a50 // "heavily wounded!"
                    } else if m == 3 {
                        0x9a3c // "lightly wounded!"
                    } else if m == 4 {
                        0x9a2a // "barely wounded!"
                    } else {
                        break 'end;
                    };
                } else {
                    if attacker != 0xff && gm.gi8(g::ROSTER_MEMBER.wrapping_add(ros8(attacker))) == b'-' as i8 {
                        text::print_string(gm, 0x9a10); // "dragged under!"
                        rest::play_tone_sweep(gm, 0x28, 1, 2000, 0x4b0);
                        let f = gm.g8(g::ROSTER_FLAGS.wrapping_add(toff));
                        gm.set_g8(g::ROSTER_FLAGS.wrapping_add(toff), f | 4);
                        let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(toff)) as u16;
                        gm.set_g8(g::MONSTER_TABLE.wrapping_add(slot.wrapping_mul(8)).wrapping_add(1), 0);
                        util::animate_wait(gm, 4);
                        break 'end;
                    }
                    s = 0x9a22; // "hit!"
                }
            } else {
                print_combatant_name(gm, target);
                s = 0x9a06; // "slept!"
            }
            text::print_string(gm, s);
        }
    }
    // LAB_1000_e606
    if (tflags & 0x80) != 0 {
        rest::draw_status_window(gm);
        view::refresh_map_view(gm);
    }
    let v = gm.g8(g::COMBAT_STATE_FLAGS) & 0xf3;
    gm.set_g8(g::COMBAT_STATE_FLAGS, v);
}

/// orig 1000:e638 (49 bytes). Ghidra order (y1, x1, y2, x2). Squared distance: (y2-y1)^2 is
/// computed as a 32-bit long, (x2-x1)^2 as a 16-bit int; the result's low word is
/// (x2-x1)^2 + low((y2-y1)^2) and the high word is high((y2-y1)^2). Only the low word is used
/// by `cell_distance`.
pub fn dist_squared(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16, param_4: i16) -> u32 {
    let dy = (param_3.wrapping_sub(param_1)) as i32;
    let l = dy.wrapping_mul(dy);
    let dx = param_4.wrapping_sub(param_2);
    let lo = (dx.wrapping_mul(dx)).wrapping_add(l as i16) as u16;
    (((l as u32) >> 16) << 16) | lo as u32
}

/// orig 1000:e66a (72 bytes). Ghidra order (y1, x1, y2, x2). Integer square root of
/// `dist_squared` (low word), computed by subtracting successive odd numbers 1, 3, 5, ... while
/// they fit; returns the count. Vector 1778:0752 (COMBAT find_nearest_enemy).
pub fn cell_distance(gm: &mut Game, param_1: u16, param_2: u16, param_3: u16, param_4: u16) -> i16 {
    let mut d = dist_squared(gm, param_1 as i16, param_2 as i16, param_3 as i16, param_4 as i16) as u16 as i16;
    let mut n: i16 = 0;
    let mut k: i16 = 1;
    while k <= d {
        d = d.wrapping_sub(k);
        n = n.wrapping_add(1);
        k = k.wrapping_add(2);
    }
    n
}

/// orig 1000:e6b4 (48 bytes). Ghidra order (a, b). `cell_distance` between roster entries a
/// and b (their x/y at +6/+7). The C shows no return but AX carries the e66a result, which
/// every caller uses. Vector 1778:063e.
pub fn roster_distance(gm: &mut Game, param_1: i16, param_2: i16) -> u16 {
    let (a, b) = (ros8(param_1), ros8(param_2));
    let y1 = gm.g8(g::ROSTER_Y.wrapping_add(a)) as u16;
    let x1 = gm.g8(g::ROSTER_X.wrapping_add(a)) as u16;
    let y2 = gm.g8(g::ROSTER_Y.wrapping_add(b)) as u16;
    let x2 = gm.g8(g::ROSTER_X.wrapping_add(b)) as u16;
    cell_distance(gm, y1, x1, y2, x2) as u16
}

/// orig 1000:e6e4 (189 bytes, plus the e7a2..e927 fragments). Ghidra order (range, ri).
/// Crosshair aiming loop for combatant `ri`. Shows the crosshair (g_crosshair_visible = 1)
/// and places it on the last target recorded in the attacker's monster-table slot (+7) when
/// that roster index is < 32, has none of flags 0x30, is non-zero, its slot tile is non-zero
/// and it lies within `range`; otherwise on the attacker itself. Then loops: redraw the view,
/// read a command key: 1/2/3/4 move the crosshair W/E/N/S and 0xd3/0xd4/0xd5/0xd6 NW/SW/NE/SE
/// (only when the new cell is inside 0..10 and within `range` of the attacker); Esc cancels;
/// Enter, 'A' or space confirm when the crosshair is not on the attacker (newline, and in
/// spell mode a random-tone burst play_random_tones(700, mode*0x640+8000, 800)); space on
/// the attacker's own cell cancels. Returns the distance from the attacker to the chosen cell
/// (the e66a value of the last move), or 0 after a cancel (which also prints a newline). The
/// crosshair is hidden on exit. Vector 1778:09c2 (CAST c920/c9dc/caa8, ec48, ee32).
pub fn aim_crosshair(gm: &mut Game, param_1: i16, param_2: u16) -> i16 {
    let (range, ri) = (param_1, param_2 as i16);
    gm.set_g8(g::CROSSHAIR_VISIBLE, 1);
    let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(ros8(ri))) as u16;
    let mut target = gm.g8(g::MONSTER_TABLE.wrapping_add(slot.wrapping_mul(8)).wrapping_add(7)) as i16; // slot +7 target
    let toff = ros8(target);
    let tslot = gm.g8(g::ROSTER_SLOT.wrapping_add(toff)) as u16;
    let tflags = gm.g8(g::ROSTER_FLAGS.wrapping_add(toff));
    if 0x1f < target
        || (tflags & 0x30) != 0
        || tflags == 0
        || gm.gi8(g::MONSTER_TABLE.wrapping_add(tslot.wrapping_mul(8))) == 0
        || range < roster_distance(gm, target, ri) as i16
    {
        target = ri;
    }
    gm.set_g8(g::CROSSHAIR_X, gm.g8(g::ROSTER_X.wrapping_add(ros8(target))));
    gm.set_g8(g::CROSSHAIR_Y, gm.g8(g::ROSTER_Y.wrapping_add(ros8(target))));
    let mut cancel = false; // [BP-0xc]
    let mut confirm = false; // [BP-0x8]
    let own_y_p = g::ROSTER_Y.wrapping_add(ros8(ri));
    let own_x_p = g::ROSTER_X.wrapping_add(ros8(ri));
    let mut dist: i16 = 0; // [BP-0x12]; uninitialised in the original but always written before use
    let mut sx: i16 = 0; // SI
    let mut sy: i16 = 0; // DI
    let mut first = true; // the original enters the loop at the flag check (e804) with SI/DI unused
    loop {
        if !first {
            // LAB_1000_e7a8
            let nx = sx.wrapping_add(gm.g8(g::CROSSHAIR_X) as i16);
            let ny = sy.wrapping_add(gm.g8(g::CROSSHAIR_Y) as i16);
            let oy = gm.g8(own_y_p) as u16;
            let ox = gm.g8(own_x_p) as u16;
            dist = cell_distance(gm, oy, ox, ny as u16, nx as u16);
            if dist <= range && -1 < nx && nx < 0xb && -1 < ny && ny < 0xb {
                gm.set_g8(g::CROSSHAIR_X, nx as u8);
                gm.set_g8(g::CROSSHAIR_Y, ny as u8);
            }
        }
        first = false;
        // LAB_1000_e804
        if confirm || cancel {
            break;
        }
        sy = 0;
        sx = 0;
        view::refresh_map_view(gm);
        let key = input::read_command_key(gm);
        if key == 0x1b {
            cancel = true; // e8ca
            continue;
        }
        if key < 0x1b {
            match key {
                1 => { sx = -1; continue; }
                2 => { sx = 1; continue; }
                3 => { sy = -1; continue; }
                4 => { sy = 1; continue; }
                0xd => {}
                _ => continue,
            }
        } else {
            // e8d2
            match key {
                0x20 => {
                    // e86a: space on the attacker's own cell cancels
                    if gm.g8(own_x_p) == gm.g8(g::CROSSHAIR_X) && gm.g8(own_y_p) == gm.g8(g::CROSSHAIR_Y) {
                        cancel = true;
                    }
                }
                0x41 => {}
                0xd3 => { sx = -1; sy = -1; continue; }
                0xd4 => { sy = 1; sx = -1; continue; }
                0xd5 => { sy = -1; sx = 1; continue; }
                0xd6 => { sx = 1; sy = 1; continue; }
                _ => continue,
            }
        }
        // LAB_1000_e883: confirm unless the crosshair sits on the attacker
        if gm.g8(own_x_p) != gm.g8(g::CROSSHAIR_X) || gm.g8(own_y_p) != gm.g8(g::CROSSHAIR_Y) {
            text::put_char(gm, 10);
            let mode = gm.g8(SPELL_MODE);
            if mode != 0 {
                util::play_random_tones(gm, 700, (mode as u16).wrapping_mul(0x640).wrapping_add(8000) as i16, 800);
            }
            confirm = true;
        }
    }
    // FUN_1000_e900
    gm.set_g8(g::CROSSHAIR_VISIBLE, 0);
    if cancel {
        text::put_char(gm, 10);
        0
    } else {
        dist
    }
}

/// orig 1000:e7a2 (170 bytes). Not a function: the NW-diagonal entry and the body of the
/// `aim_crosshair` (e6e4) loop that Ghidra split off. Translated inside `aim_crosshair`.
pub fn fn_1000_e7a2_fragment(gm: &mut Game) {}

/// orig 1000:e84c (9 bytes). Not a function: the SE-diagonal entry (SI=1, DI=1) of the
/// `aim_crosshair` loop; Ghidra names it SHOPPES2_e84c because the SHOPPES2 vector 1778:09fe
/// lands on this address in SHOPPES2.OVL. Translated inside `aim_crosshair`.
pub fn fn_1000_e84c_fragment(gm: &mut Game) {}

/// orig 1000:e856 (9 bytes). Not a function: the NE-diagonal entry (DI=-1, SI=1) of the
/// `aim_crosshair` loop. Translated inside `aim_crosshair`.
pub fn fn_1000_e856_fragment(gm: &mut Game) {}

/// orig 1000:e860 (79 bytes). Not a function: the SW-diagonal entry (DI=1, SI=-1), the west
/// entry and the confirm path (e883: newline, spell-mode tones, confirm flag) of the
/// `aim_crosshair` loop. Translated inside `aim_crosshair`.
pub fn fn_1000_e860_fragment(gm: &mut Game) {}

/// orig 1000:e8ca (8 bytes). Not a function: the Esc path of the `aim_crosshair` loop (sets
/// the cancel flag). Translated inside `aim_crosshair`.
pub fn fn_1000_e8ca_fragment(gm: &mut Game) {}

/// orig 1000:e8d2 (45 bytes). Not a function: the key dispatch for keys > 0x1b (space, 'A',
/// 0xd3..0xd6 diagonals) of the `aim_crosshair` loop. Translated inside `aim_crosshair`.
pub fn fn_1000_e8d2_fragment(gm: &mut Game) {}

/// orig 1000:e900 (39 bytes). Not a function: the epilogue of `aim_crosshair` (hides the
/// crosshair, returns 0 + newline on cancel, else the distance). Translated inside
/// `aim_crosshair`.
pub fn fn_1000_e900_fragment(gm: &mut Game) -> u16 { 0 }

/// orig 1000:e928 (140 bytes). Ghidra order (y, x). Returns the index of the first roster
/// entry standing on combat cell (y, x) whose monster-table slot's anim tile (+1) is not 0xf4,
/// whose flags contain 0x80 or 0x40 and neither 0x20 (out) nor 0x04 (swallowed); -1 when no
/// entry matches. Vector 1778:09ce (CAST c920/c9dc/caa8, ea02, ec48, ee32).
pub fn find_combatant_at(gm: &mut Game, param_1: u16, param_2: u16) -> i16 {
    let (y, x) = (param_1, param_2);
    let mut i: i16 = 0;
    let mut off: u16 = 0;
    loop {
        let fl = gm.g8(g::ROSTER_FLAGS.wrapping_add(off));
        let slot = gm.g8(g::ROSTER_SLOT.wrapping_add(off)) as u16;
        let matched = gm.g8(g::ROSTER_X.wrapping_add(off)) as u16 == x
            && gm.g8(g::ROSTER_Y.wrapping_add(off)) as u16 == y
            && gm.gi8(g::MONSTER_TABLE.wrapping_add(slot.wrapping_mul(8)).wrapping_add(1)) != -0xc
            && (fl & 0xc0) != 0
            && (fl & 0x20) == 0
            && (fl & 4) == 0;
        if matched {
            return i;
        }
        off = off.wrapping_add(8);
        i = i.wrapping_add(1);
        if 0x1f < i {
            return -1;
        }
    }
}

/// orig 1000:e9b4 (78 bytes). Ghidra order (y, x). Picks a random cell adjacent to (y, x)
/// (each axis offset by rand(1..3) - 2, i.e. -1..1, which may be the cell itself) into g_dx /
/// g_dy, re-rolling until both lie in 0..10. Vector 1778:075e (COMBAT monster_move, ea02).
pub fn random_cell_near(gm: &mut Game, param_1: i16, param_2: i16) {
    let (y, x) = (param_1, param_2);
    loop {
        loop {
            let r = util::rand_range(gm, 3, 1);
            gm.set_gi16(g::DX, r.wrapping_add(x).wrapping_sub(2));
            let r = util::rand_range(gm, 3, 1);
            gm.set_gi16(g::DY, r.wrapping_add(y).wrapping_sub(2));
            if !(gm.gi16(g::DX) < 0) { break; }
        }
        let (dx, dy) = (gm.gi16(g::DX), gm.gi16(g::DY));
        if !(10 < dx || dy < 0 || 10 < dy) { break; }
    }
}

/// orig 1000:ea02 (339 bytes). Ghidra order (missile_tile, hit, ty, tx, attacker). Flies a
/// missile from `attacker` towards combat cell (ty, tx) and returns the roster entry it
/// strikes, or 0xffff. When `hit` is 0 the target cell is first replaced by a random
/// neighbouring cell (`random_cell_near`) that is not the attacker's own. For g_attack_weapon
/// 0x19 or 0x22 ('"') no projectile is animated: a hit is accepted as landing (a miss leaves
/// the landed flag uninitialised in the original; Rust treats it as 0). Otherwise
/// `animate_projectile(missile_tile, y, x, attacker_y, attacker_x)`; when it was blocked the
/// cell becomes g_dy/g_dx (where it stopped), else COMBAT `check_room_trigger(y, x)` runs. Not
/// landed -> g_dx/g_dy = cell, return 0xffff. Otherwise the victim is `find_combatant_at(y,
/// x)`; weapons 0x13 (flaming oil), 0x33, 0x34, 0x35, 0x36 also drop a field object
/// (add_combatant(z, y, x, 2, tile) with tiles 0xea, 0xe8, 0xe9, 0xea, 0xeb). The victim is
/// returned only when (spell mode is off or hit != 0), victim >= 0 and victim is not
/// g_active_combatant; g_dx/g_dy always end up as the landing cell. Vector 1778:0656 (COMBAT
/// monster_ranged_attack, ec48).
pub fn animate_missile(gm: &mut Game, param_1: u16, param_2: i16, param_3: u16, param_4: u16, param_5: i16) -> u16 {
    let (missile, hit, ty, tx, attacker) = (param_1, param_2, param_3, param_4, param_5);
    let mut cx = tx; // local_6
    let mut cy = ty; // local_c
    let aoff = ros8(attacker);
    let ax = gm.g8(g::ROSTER_X.wrapping_add(aoff));
    let ay = gm.g8(g::ROSTER_Y.wrapping_add(aoff));
    if hit == 0 {
        loop {
            random_cell_near(gm, ty as i16, tx as i16);
            if ax as i16 != gm.gi16(g::DX) { break; }
            if !(ay as i16 == gm.gi16(g::DY)) { break; }
        }
        cx = gm.gi16(g::DX) as u16;
        cy = gm.gi16(g::DY) as u16;
    }
    // TODO(verify): `landed` is uninitialised in the original when the weapon is 0x19/0x22 and hit == 0.
    let mut landed: i16 = 0; // local_e
    let w = gm.g8(g::ATTACK_WEAPON);
    if w == 0x19 || w == 0x22 {
        if hit != 0 {
            cx = tx;
            cy = ty;
            landed = 1;
        }
    } else {
        landed = animate_projectile(gm, missile as i16, cy as i16, cx as i16, ay as i16, ax as i16) as i16;
        if landed == 0 {
            cx = gm.gi16(g::DX) as u16;
            cy = gm.gi16(g::DY) as u16;
        } else {
            ovl::combat::check_room_trigger(gm, cy, cx);
        }
    }
    if landed == 0 {
        gm.set_gi16(g::DX, cx as i16);
        gm.set_gi16(g::DY, cy as i16);
        return 0xffff;
    }
    let victim = find_combatant_at(gm, cy, cx);
    let w = gm.g8(g::ATTACK_WEAPON);
    let field: Option<i16> = match w {
        0x13 => Some(0xea),
        0x33 => Some(0xe8),
        0x34 => Some(0xe9),
        0x35 => Some(0xea),
        0x36 => Some(0xeb),
        _ => None,
    };
    if let Some(tile) = field {
        let z = gm.g8(g::PARTY_Z);
        rest::add_combatant(gm, z, cy as u8, cx as u8, 2, tile);
    }
    // LAB_1000_eb03
    if (gm.g8(SPELL_MODE) == 0 || hit != 0) && -1 < victim && victim as u16 != gm.g8(g::ACTIVE_COMBATANT) as u16 {
        gm.set_gi16(g::DX, cx as i16);
        gm.set_gi16(g::DY, cy as i16);
        return victim as u16;
    }
    gm.set_gi16(g::DY, cy as i16);
    gm.set_gi16(g::DX, cx as i16);
    0xffff
}

/// orig 1000:eb5c (123 bytes). Ghidra order (distance, weapon). Consumes ammunition after a
/// ranged attack by the active combatant; nothing happens in spell mode. Thrown/limited items
/// (weapon < 0x17 and either 0x10 or >= 0x15) are consumed only when the shot travelled 2 or
/// more cells: the inventory count (0x57c0 + weapon) is decremented, or when it is already 0
/// the item is unequipped from the active member (`remove_equipped_item`). Weapons 0x1a and
/// 0x24 decrement the count at 0x57db (0x1b) and 0x1c decrements 0x57dd (0x1d); when that
/// count reaches 0 SJOG `remove_item_from_all_members(weapon)` runs and the number of
/// members that had it equipped is added back to the inventory count (0x57c0 + weapon).
pub fn use_missile_ammo(gm: &mut Game, param_1: i16, param_2: i16) {
    let (distance, weapon) = (param_1, param_2);
    if gm.g8(SPELL_MODE) != 0 {
        return;
    }
    if weapon < 0x17 {
        if weapon < 0x15 && weapon != 0x10 {
            return;
        }
        if distance < 2 {
            return;
        }
        let p = (weapon as u16).wrapping_add(0x57c0); // g_inventory[weapon]
        if gm.gi8(p) != 0 {
            let v = gm.gi8(p).wrapping_sub(1);
            gm.set_gi8(p, v);
            return;
        }
        let member = gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(gm.g8(g::ACTIVE_COMBATANT) as i16))) as i16;
        rest::remove_equipped_item(gm, weapon as u16, member);
        return;
    }
    let c: i8;
    if weapon != 0x1a {
        if weapon == 0x1c {
            c = gm.gi8(0x57dd).wrapping_sub(1); // g_inventory[0x1d]
            gm.set_gi8(0x57dd, c);
            if c == 0 {
                let n = ovl::sjog::remove_item_from_all_members(gm, weapon as u16) as i8;
                let p = (weapon as u16).wrapping_add(0x57c0);
                let v = gm.gi8(p).wrapping_add(n);
                gm.set_gi8(p, v);
            }
            return;
        }
        if weapon != 0x24 {
            return;
        }
    }
    c = gm.gi8(0x57db).wrapping_sub(1); // g_inventory[0x1b]
    gm.set_gi8(0x57db, c);
    // LAB_1000_eb80
    if c == 0 {
        let n = ovl::sjog::remove_item_from_all_members(gm, weapon as u16) as i8;
        let p = (weapon as u16).wrapping_add(0x57c0);
        let v = gm.gi8(p).wrapping_add(n);
        gm.set_gi8(p, v);
    }
}

/// orig 1000:ebdc (106 bytes). Ghidra order (ri). Monster interference check: m =
/// g_monster_interference[ri] (0x58a8); when m is not 0xff, roster entry m is live (flags
/// non-zero), on the enemy side (`combatant_side`), has none of flags 0xc (asleep/swallowed),
/// the status icon is not 'T' (time stop) and `roster_distance(m, ri)` == 1, prints a newline,
/// m's name and "interferes!" (0x9a70) and returns 1; else 0. Vector 1778:0662 (COMBAT
/// party_member_turn Cast, ec48).
pub fn check_interference(gm: &mut Game, param_1: i16) -> u16 {
    let ri = param_1;
    let m = gm.g8((ri as u16).wrapping_add(0x58a8)) as u16; // g_monster_interference[ri]
    if m != 0xff
        && gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(m as i16))) != 0
        && rest::combatant_side(gm, m as i16)
        && (gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(m as i16))) & 0xc) == 0
        && gm.gi8(g::STATUS_ICON) != b'T' as i8
        && roster_distance(gm, m as i16, ri) as i16 == 1
    {
        text::put_char(gm, 10);
        print_combatant_name(gm, m as i16);
        text::print_string(gm, 0x9a70); // "interferes!"
        return 1;
    }
    0
}

/// orig 1000:ec48 (397 bytes). Ghidra order (missile_tile, weapon, ri). Ranged attack by
/// roster entry `ri`. Weapons 0x1a, 0x1c, 0x24, 0x13 and 0x11 are first subject to
/// `check_interference` (an interfering monster aborts). Prints "Aim! " (0x9a7e) and runs
/// `aim_crosshair(g_item_range[weapon], ri)`; a cancelled aim does nothing. Otherwise: tone
/// sweep; flaming oil (0x13) consumes one from g_inventory[0x13] or unequips it from the active
/// member when the count is 0; the attacker's slot target (+7) is set to 0xff; victim =
/// `find_combatant_at(crosshair)`: none -> hit flag 1 (the shot just flies), else the target is
/// recorded in slot +7, `use_missile_ammo(distance, weapon)` and hit = COMBAT
/// `attack_hits(weapon, -spell_mode, ri, victim)`. `animate_missile(missile_tile, hit,
/// crosshair_y, crosshair_x, ri)` then yields the struck entry (-1: newline + `print_missed`
/// when a victim had been aimed at) or flashes it, prints a newline, COMBAT
/// `deal_attack_damage(ri, struck)` and `print_hit_result(ri, struck)`. Weapon 0x26 (magic
/// axe) finally animates the projectile flying back from the landing cell to the attacker.
pub fn ranged_attack(gm: &mut Game, param_1: u16, param_2: i16, param_3: i16) {
    let (missile, weapon, ri) = (param_1, param_2, param_3);
    let limited = weapon == 0x1a || weapon == 0x1c || weapon == 0x24 || weapon == 0x13 || weapon == 0x11;
    if !limited || check_interference(gm, ri) == 0 {
        text::print_string(gm, 0x9a7e); // "Aim! "
        let range = gm.g8((weapon as u16).wrapping_add(0x1664)) as i16; // g_item_range[weapon]
        let dist = aim_crosshair(gm, range, ri as u16);
        if dist != 0 {
            rest::play_tone_sweep(gm, 100, 5, 300, 0x514);
            if weapon == 0x13 {
                if gm.gi8(0x57d3) == 0 { // g_inventory[0x13] flaming oil
                    let member = gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(gm.g8(g::ACTIVE_COMBATANT) as i16))) as i16;
                    rest::remove_equipped_item(gm, 0x13, member);
                } else {
                    let v = gm.gi8(0x57d3).wrapping_sub(1);
                    gm.set_gi8(0x57d3, v);
                }
            }
            let soff = (gm.g8(g::ROSTER_SLOT.wrapping_add(ros8(ri))) as u16).wrapping_mul(8);
            gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(soff).wrapping_add(1), 0xff); // slot +7 target
            let cy = gm.g8(g::CROSSHAIR_Y) as u16;
            let cx = gm.g8(g::CROSSHAIR_X) as u16;
            let victim = find_combatant_at(gm, cy, cx);
            let hit: i16;
            if victim < 0 {
                hit = 1;
            } else {
                gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(soff).wrapping_add(1), victim as u8);
                use_missile_ammo(gm, dist, weapon);
                let neg_mode = 0u16.wrapping_sub(gm.g8(SPELL_MODE) as u16);
                hit = ovl::combat::attack_hits(gm, weapon, neg_mode, ri as u16, victim as u16);
            }
            let cy = gm.g8(g::CROSSHAIR_Y) as u16;
            let cx = gm.g8(g::CROSSHAIR_X) as u16;
            let struck = animate_missile(gm, missile, hit, cy, cx, ri) as i16;
            let ly = gm.gi16(g::DY);
            let lx = gm.gi16(g::DX);
            if struck < 0 {
                if -1 < victim {
                    text::put_char(gm, 10);
                    print_missed(gm, victim as u16);
                }
            } else {
                rest::flash_monster(gm, struck as u16);
                text::put_char(gm, 10);
                ovl::combat::deal_attack_damage(gm, ri, struck);
                print_hit_result(gm, ri, struck);
            }
            if weapon == 0x26 {
                let ry = gm.g8(g::ROSTER_Y.wrapping_add(ros8(ri))) as i16;
                let rx = gm.g8(g::ROSTER_X.wrapping_add(ros8(ri))) as i16;
                animate_projectile(gm, missile as i16, ry, rx, ly, lx);
            }
        }
    }
}

/// orig 1000:edd8 (89 bytes). Ghidra order (weapon, target, attacker). Melee blow: tone sweep
/// (0x96, 5, 0x2ee, 400), newline, COMBAT `attack_hits(weapon, -spell_mode, attacker,
/// target)`; a miss prints `print_missed(target)`, a hit flashes the target, COMBAT
/// `deal_attack_damage(attacker, target)` and `print_hit_result(attacker, target)`. Vector
/// 1778:066e (COMBAT charmed-member attack, ee32).
pub fn melee_attack(gm: &mut Game, param_1: u16, param_2: u16, param_3: u16) {
    let (weapon, target, attacker) = (param_1, param_2, param_3);
    rest::play_tone_sweep(gm, 0x96, 5, 0x2ee, 400);
    text::put_char(gm, 10);
    let neg_mode = 0u16.wrapping_sub(gm.g8(SPELL_MODE) as u16);
    let hit = ovl::combat::attack_hits(gm, weapon as i16, neg_mode, attacker, target);
    if hit == 0 {
        print_missed(gm, target);
    } else {
        rest::flash_monster(gm, target);
        ovl::combat::deal_attack_damage(gm, attacker as i16, target as i16);
        print_hit_result(gm, attacker as i16, target as i16);
    }
}

/// orig 1000:ee32 (231 bytes). Ghidra order (weapon, ri). Attack with `weapon` (0xff = bare
/// hands / monster natural attack) by roster entry `ri`. Weapon ids > 0x22 are spells and set
/// the attack-is-spell byte 0x5890. The range and missile tile come from the monster tables
/// for monsters (range 0x159c[type], a range of 1 counts as melee; missile 0x15cc[type]) or
/// from g_item_range (0x1664, 0 for 0xff) and 0x169c[weapon] for party members. Range 0:
/// prints "Aim! " (0x9a84), `aim_crosshair(1, ri)` and `find_combatant_at(crosshair)`; a
/// cancelled aim or empty cell prints "Nothing!\n" (0x9a8a), else the victim is recorded in
/// the attacker's slot target (+7) and `melee_attack(weapon, victim, ri)` runs. Otherwise
/// `ranged_attack(missile, weapon, ri)`. Vector 1778:094a (CAST bfb2/bfcc, ef1c, ef76).
pub fn attack_with(gm: &mut Game, param_1: i16, param_2: i16) {
    let (weapon, ri) = (param_1, param_2);
    if 0x22 < weapon {
        gm.set_g8(ATTACK_IS_SPELL, 1);
    }
    let roff = ros8(ri);
    let range: i8;
    let missile: u8;
    if (gm.g8(g::ROSTER_FLAGS.wrapping_add(roff)) & 0x80) == 0 {
        let mtype = gm.g8(g::ROSTER_MEMBER.wrapping_add(roff)) as u16;
        let mut r = gm.gi8(mtype.wrapping_add(0x159c)); // monster range table
        if r == 1 {
            r = 0;
        }
        range = r;
        missile = gm.g8(mtype.wrapping_add(0x15cc)); // monster missile tile
    } else {
        range = if weapon == 0xff { 0 } else { gm.gi8((weapon as u16).wrapping_add(0x1664)) }; // g_item_range
        missile = gm.g8((weapon as u16).wrapping_add(0x169c)); // item missile tile / class
    }
    if range == 0 {
        text::print_string(gm, 0x9a84); // "Aim! "
        let dist = aim_crosshair(gm, 1, ri as u16);
        let cy = gm.g8(g::CROSSHAIR_Y) as u16;
        let cx = gm.g8(g::CROSSHAIR_X) as u16;
        let victim = find_combatant_at(gm, cy, cx);
        if dist == 0 || victim == -1 {
            text::print_string(gm, 0x9a8a); // "Nothing!\n"
        } else {
            let soff = (gm.g8(g::ROSTER_SLOT.wrapping_add(roff)) as u16).wrapping_mul(8);
            gm.set_g8(g::MONSTER_ANIM_STATE.wrapping_add(soff).wrapping_add(1), victim as u8);
            melee_attack(gm, weapon as u16, victim as u16, ri as u16);
        }
    } else {
        ranged_attack(gm, missile as u16, weapon, ri);
    }
}

/// orig 1000:ef1c (89 bytes). Ghidra order (weapon_count, item). Attacks with one equipped
/// item of the active combatant when it is a weapon (item != 0xff and g_item_attack[item] !=
/// 0). With more than one weapon equipped the spell-mode bytes 0x588f/0x5890 are cleared and
/// the item's name (g_name_ptrs 0x17f6) plus ":\n" (0x9a94) are printed first. Sets
/// g_attack_weapon = item, prints "Attack-" (0x9a98) and runs `attack_with(item,
/// g_active_combatant)`.
pub fn attack_with_equipped(gm: &mut Game, param_1: i16, param_2: i16) {
    let (count, item) = (param_1, param_2);
    if item != 0xff && gm.gi8((item as u16).wrapping_add(0x15fc)) != 0 {
        if 1 < count {
            gm.set_g8(SPELL_MODE, 0);
            gm.set_g8(ATTACK_IS_SPELL, 0);
            text::put_char(gm, 10);
            let s = gm.g16((item as u16).wrapping_mul(2).wrapping_add(0x17f6)); // g_name_ptrs[item]
            text::print_string(gm, s);
            text::print_string(gm, 0x9a94); // ":\n"
        }
        gm.set_g8(g::ATTACK_WEAPON, item as u8);
        text::print_string(gm, 0x9a98); // "Attack-"
        let ac = gm.g8(g::ACTIVE_COMBATANT) as i16;
        attack_with(gm, item, ac);
    }
}

/// orig 1000:ef76 (143 bytes). Ghidra order (weapon_count, ri). The Attack command for roster
/// entry `ri`: a monster-side combatant or a member with no weapon equipped (weapon_count ==
/// 0) attacks bare-handed (g_attack_weapon = 0xff, "Attack-" 0x9aa0, `attack_with(0xff, ri)`).
/// Otherwise, with more than one weapon, the cursor is moved to column 0 of its row and a space
/// printed; then `attack_with_equipped(count, item)` is tried for the member's helmet slot
/// (0x55c1), left hand (0x55c3) and right hand (0x55c4) in turn. Vector 1778:067a (COMBAT
/// party_member_turn 'A').
pub fn attack_command(gm: &mut Game, param_1: i16, param_2: i16) {
    let (count, ri) = (param_1, param_2);
    if (gm.g8(g::ROSTER_FLAGS.wrapping_add(ros8(ri))) & 0x80) == 0 || count == 0 {
        gm.set_g8(g::ATTACK_WEAPON, 0xff);
        text::print_string(gm, 0x9aa0); // "Attack-"
        attack_with(gm, 0xff, ri);
    } else {
        let m = gm.g8(g::ROSTER_MEMBER.wrapping_add(ros8(ri))) as u16;
        if 1 < count {
            let row = text::get_cursor_row(gm);
            text::set_cursor_pos(gm, row, 0);
            text::put_char(gm, 0x20);
        }
        let rec = m.wrapping_mul(0x20);
        let helmet = gm.g8(rec.wrapping_add(0x55c1)) as i16;
        attack_with_equipped(gm, count, helmet);
        let left = gm.g8(rec.wrapping_add(0x55c3)) as i16;
        attack_with_equipped(gm, count, left);
        let right = gm.g8(rec.wrapping_add(0x55c4)) as i16;
        attack_with_equipped(gm, count, right);
    }
}

/// orig 1000:f006 (290 bytes). Ghidra order (ybuf, xbuf, y1, x1, y0, x0) - all pixel
/// coordinates. Traces the line from (x0, y0) to (x1, y1) into the two byte buffers (x into
/// `xbuf`, y into `ybuf`), one entry per pixel step, terminated by 0xff in both. Slope =
/// |dy| * 100 / |dx| (0x4b00 for a vertical line); each outer step advances x by one and
/// accumulates the slope, the inner loop advances y by one per 100 units while y steps remain.
/// At most 0x148 points are written. The C's `void` return is kept.
pub fn trace_line(gm: &mut Game, param_1: u16, param_2: u16, param_3: i16, param_4: i16, param_5: i16, param_6: i16) {
    let (mut yp, mut xp) = (param_1, param_2);
    let (y1, x1, mut y, mut x) = (param_3, param_4, param_5, param_6);
    let mut xstep: i16 = 1; // local_6
    let mut ystep: i16 = 1; // local_8
    let mut budget: i16 = 0x148; // local_c
    let slope: i16 = if x1 == x { 0x4b00 } else { (y1.wrapping_sub(y)).wrapping_mul(100) / (x1.wrapping_sub(x)) };
    let slope = if slope < 0 { slope.wrapping_neg() } else { slope };
    if x1 < x {
        xstep = -1;
    }
    if y1 < y {
        ystep = -1;
    }
    let mut nx = x1.wrapping_sub(x); // local_10
    if nx < 0 {
        nx = nx.wrapping_neg();
    }
    let mut ny = y1.wrapping_sub(y); // local_4
    if ny < 0 {
        ny = ny.wrapping_neg();
    }
    gm.set_g8(xp, x as u8);
    gm.set_g8(yp, y as u8);
    let mut acc = slope; // iVar1
    loop {
        xp = xp.wrapping_add(1);
        yp = yp.wrapping_add(1);
        if !(0 < nx || 0 < ny) { break; }
        loop {
            if acc < 1 || ny < 1 { break; }
            acc = acc.wrapping_sub(100);
            y = y.wrapping_add(ystep);
            gm.set_g8(yp, y as u8);
            yp = yp.wrapping_add(1);
            ny = ny.wrapping_sub(1);
            gm.set_g8(xp, x as u8);
            xp = xp.wrapping_add(1);
            budget = budget.wrapping_sub(1);
            if budget == 0 { break; }
        }
        if budget == 0 { break; }
        nx = nx.wrapping_sub(1);
        acc = acc.wrapping_add(slope);
        x = x.wrapping_add(xstep);
        gm.set_g8(yp, y as u8);
        gm.set_g8(xp, x as u8);
        budget = budget.wrapping_sub(1);
    }
    gm.set_g8(xp, 0xff);
    gm.set_g8(yp, 0xff);
}

/// orig 1000:f12a (892 bytes). Ghidra order (ybuf_ptr, xbuf_ptr, kind, phase, py, px). Draws
/// one frame of a missile of `kind` at pixel (px, py) in g_ui_color1. `phase` 1..4 selects the
/// shape table orientation (table 0x2188 for kind 2, 0x2198 for kinds > 2: signed (y, x)
/// coordinate pairs; phases offset the table by one byte to swap axes and flip signs via
/// local_1e (y) / local_1c (x)). Kind 0 draws up to 9 two-pixel horizontal dashes along the
/// previous 4.. path points (a sling-stone trail read from ybuf-4/xbuf-4); kind 1 a 5x5 blob
/// of vertical lines; kind 2 four lines from the table (2 pairs each, stride 4); kinds 3..6 a
/// 48-pixel sparkle in g_ui_color0/2/3/4 + 8 using the radius table at 0x21ba with random sign
/// flips; kind 7 seven lines then three extra pixels from the table.
pub fn draw_missile(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16, param_4: i16, param_5: u8, param_6: u8) {
    let (ybuf, xbuf, kind, phase, py, px) = (param_1 as u16, param_2 as u16, param_3, param_4, param_5 as u16, param_6 as u16);
    let mut sx: i16 = 1; // local_1c
    let mut sy: i16 = 1; // local_1e
    let mut sparkle: u16 = 0xffff; // local_4
    let mut ty: u16 = 0; // local_e (y table; uninitialised for kinds < 2 in the original)
    let mut tx: u16 = 0; // local_a (x table)
    if kind == 2 {
        ty = 0x2188;
        tx = 0x2188;
    }
    if 2 < kind {
        ty = 0x2198;
        tx = 0x2198;
    }
    if phase == 1 {
        ty = ty.wrapping_add(1);
    } else if phase == 2 {
        tx = tx.wrapping_add(1);
        sx = -1;
    } else if phase == 3 {
        ty = ty.wrapping_add(1);
        sy = -1;
        sx = -1;
    } else if phase == 4 {
        tx = tx.wrapping_add(1);
        sy = -1;
    }
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    match kind {
        0 => {
            let mut xq = xbuf.wrapping_sub(4);
            let mut yq = ybuf.wrapping_sub(4);
            let mut n: i16 = 0;
            loop {
                let xv = gm.g8(xq);
                if xv == 0xff { break; }
                let yv = gm.g8(yq) as i16;
                gfx::draw_line(gm, yv, (xv as i16).wrapping_add(1), yv, xv as i16);
                xq = xq.wrapping_add(1);
                yq = yq.wrapping_add(1);
                n += 1;
                if !(n < 9) { break; }
            }
        }
        1 => {
            let (y, x) = (py as i16, px as i16);
            gfx::draw_line(gm, y - 2, x + 1, y - 2, x - 1);
            let (y2, y1) = (y + 2, y - 2);
            gfx::draw_line(gm, y - 1, y2, y - 1, y1);
            gfx::draw_line(gm, y, y2, y, y1);
            gfx::draw_line(gm, y + 1, y2, y + 1, y1);
            gfx::draw_line(gm, y + 2, x + 1, y + 2, x - 1);
        }
        2 => {
            let mut n: i16 = 4;
            loop {
                let (x0, x2) = (gm.gi8(tx) as i16, gm.gi8(tx.wrapping_add(2)) as i16);
                let (y0, y2) = (gm.gi8(ty) as i16, gm.gi8(ty.wrapping_add(2)) as i16);
                tx = tx.wrapping_add(4);
                ty = ty.wrapping_add(4);
                gfx::draw_line(gm, y2.wrapping_mul(sy).wrapping_add(py as i16), x2.wrapping_mul(sx).wrapping_add(px as i16), y0.wrapping_mul(sy).wrapping_add(py as i16), x0.wrapping_mul(sx).wrapping_add(px as i16));
                n -= 1;
                if n == 0 { break; }
            }
        }
        3 => { sparkle = (gm.gi16(g::UI_COLOR0) as u16).wrapping_add(8); }
        4 => { sparkle = (gm.gi16(g::UI_COLOR2) as u16).wrapping_add(8); }
        5 => { sparkle = (gm.gi16(g::UI_COLOR3) as u16).wrapping_add(8); }
        6 => { sparkle = (gm.gi16(g::UI_COLOR4) as u16).wrapping_add(8); }
        7 => {
            let mut n: i16 = 7;
            let (mut lx, mut ly) = (tx, ty);
            loop {
                lx = tx;
                ly = ty;
                let (x0, x2) = (gm.gi8(lx) as i16, gm.gi8(lx.wrapping_add(2)) as i16);
                let (y0, y2) = (gm.gi8(ly) as i16, gm.gi8(ly.wrapping_add(2)) as i16);
                gfx::draw_line(gm, y2.wrapping_mul(sy).wrapping_add(py as i16), x2.wrapping_mul(sx).wrapping_add(px as i16), y0.wrapping_mul(sy).wrapping_add(py as i16), x0.wrapping_mul(sx).wrapping_add(px as i16));
                n -= 1;
                ty = ly.wrapping_add(4);
                tx = lx.wrapping_add(4);
                if n == 0 { break; }
            }
            for k in [4u16, 6, 8] {
                let yv = gm.gi8(ly.wrapping_add(k)) as i16;
                let xv = gm.gi8(lx.wrapping_add(k)) as i16;
                gfx::plot_pixel(gm, yv.wrapping_mul(sy).wrapping_add(py as i16), xv.wrapping_mul(sx).wrapping_add(px as i16));
            }
        }
        _ => {}
    }
    if sparkle < 0x8000 {
        let mut fy: i16 = -1;
        let mut fx: i16 = 1;
        let mut n: i16 = 0x30;
        loop {
            let ry = util::rand_range(gm, 0xf, 0);
            let rx = util::rand_range(gm, 0xf, 0);
            let flip_y = util::rand_range(gm, 1, 0);
            let flip_x = util::rand_range(gm, 1, 0);
            if flip_y != 0 {
                fy = fy.wrapping_neg();
            }
            if flip_x != 0 {
                fx = fx.wrapping_neg();
            }
            gfx::set_color(gm, sparkle as i16);
            let ox = gm.g8((rx as u16).wrapping_add(0x21ba)) as i16; // radius table
            let oy = gm.g8((ry as u16).wrapping_add(0x21ba)) as i16;
            gfx::plot_pixel(gm, ox.wrapping_mul(fx).wrapping_add(py as i16), oy.wrapping_mul(fy).wrapping_add(px as i16));
            n -= 1;
            if n == 0 { break; }
        }
    }
}

/// orig 1000:f4be (380 bytes). Ghidra order (kind, ty, tx, from_y, from_x) - combat cells.
/// Animates a projectile of `kind` from cell (from_x, from_y) to (tx, ty) across the map view
/// and returns 1 when it reaches the target (or the path runs out / leaves the view), 0 when a
/// cell on the way blocks missiles. Saves the screen (`copy_page(1, 0)`), fills both path
/// buffers (0xa728 x, 0xa872 y, 256 bytes each) with 0xff and traces the line between the
/// cell centres (cell*16+16) with `trace_line`. The step between drawn frames is 13 pixels
/// (6 inside settlements 1..0x20, 8 for kind 7 elsewhere); kind 0 skips the first 4 points.
/// Each frame: phase cycles 1..4, `pixel_to_view_cell` (g_dx == -1 -> return 1),
/// `draw_missile`, `delay_loop(0x28, 1)`, restore the 24x24 background from page 1
/// (`copy_rect_between_pages(py+15, (px&0xf8)+15, py-8, (px&0xf8)-8, 0, 1)`), advance; when
/// the x pointer runs past 0xa872 or hits the 0xff terminator the flight is complete. A cell
/// that fails `view_tile_missile_passable(g_dy, g_dx)` and is not the shooter's own cell
/// returns 0 with g_dx/g_dy at that cell. Vector 1778:046a (CMDS c8e2/ca6a, MAINOUT, ea02,
/// ec48).
pub fn animate_projectile(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16, param_4: i16, param_5: i16) -> u16 {
    let (kind, ty, tx, fy, fx) = (param_1, param_2, param_3, param_4, param_5);
    let mut step: u16 = 0xd; // local_c
    let mut phase: u16 = 0; // local_a
    gfx::copy_page(gm, 1, 0);
    for i in 0..0x100u16 {
        gm.set_g8(MISSILE_PATH_X.wrapping_add(i), 0xff);
    }
    for i in 0..0x100u16 {
        gm.set_g8(MISSILE_PATH_Y.wrapping_add(i), 0xff);
    }
    trace_line(gm, MISSILE_PATH_Y, MISSILE_PATH_X,
        ty.wrapping_mul(0x10).wrapping_add(0x10), tx.wrapping_mul(0x10).wrapping_add(0x10),
        fy.wrapping_mul(0x10).wrapping_add(0x10), fx.wrapping_mul(0x10).wrapping_add(0x10));
    let mut xp = MISSILE_PATH_X; // local_e
    let mut yp = MISSILE_PATH_Y; // local_12
    let loc = gm.g8(g::LOCATION);
    if loc == 0 || 0x20 < loc {
        if kind == 7 {
            step = 8;
        }
    } else {
        step = 6;
    }
    if kind == 0 {
        xp = MISSILE_PATH_X.wrapping_add(4);
        yp = MISSILE_PATH_Y.wrapping_add(4);
    }
    loop {
        if gm.g8(xp) == 0xff {
            return 1;
        }
        phase = (phase & 3).wrapping_add(1);
        let (py, px) = (gm.g8(yp), gm.g8(xp));
        rest::pixel_to_view_cell(gm, py as i16, px as i16);
        if gm.gi16(g::DX) == -1 {
            return 1;
        }
        draw_missile(gm, yp as i16, xp as i16, kind, phase as i16, py, px);
        util::delay_loop(gm, 0x28, 1);
        gm.pf.maybe_present();
        let (py, px) = (gm.g8(yp) as i16, gm.g8(xp) as i16);
        gfx::copy_rect_between_pages(gm, py + 0xf, (px & 0xf8) + 0xf, py - 8, (px & 0xf8) - 8, 0, 1);
        xp = xp.wrapping_add(step);
        yp = yp.wrapping_add(step);
        if MISSILE_PATH_Y < xp {
            return 1;
        }
        if gm.g8(xp) == 0xff {
            break;
        }
        let (dy, dx) = (gm.gi16(g::DY), gm.gi16(g::DX));
        if !rest::view_tile_missile_passable(gm, dy, dx) && (fx != dx || fy != dy) {
            return 0;
        }
    }
    1
}
