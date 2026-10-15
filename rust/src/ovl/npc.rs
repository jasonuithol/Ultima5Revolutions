//! NPC.OVL (translated)
//!
//! Town NPC schedules and movement (level-2 overlay, 1000:a290-1000:b5b9). TOWN's `enter_town`
//! calls `load_npc_schedules` (reads the settlement's 0x240-byte block of TOWNE/DWELLING/CASTLE/
//! KEEP.NPC into g_npc_schedules 0x5d5e, g_npc_types 0x659e and the dialogue index of each NPC
//! state record) then `init_npc_states(hour)`; every town turn calls `move_npcs(hour)` (vector
//! 1778:0339) and `schedule_index_for_hour` (vector 1778:03b6) is shared with TOWN and TALK.
//!
//! NPC state record (16 bytes at 0x5f5e + 16*npc): +0 state (0 inactive, 1 idle at its post,
//! 2 following a move list to the schedule target, 3 following a move list to a ladder/stair,
//! 4/5 coming down/up onto the party's floor through a ladder, 6/7 must leave the party's floor
//! upwards/downwards, 8 both NPC and target are off the party's floor), +2 x, +4 y, +6 z,
//! +8 type/tile, +0xa dialogue index (.NPC file bytes 0x220.., 0 none, < 0x80 .TLK entry,
//! 0x81..0x88 shopkeeper, 0xfd frightened, 0xfe Shadowlord, 0xff guard), +0xc monster slot,
//! +0xe schedule index (0..2). Schedule record (16 bytes at 0x5d5e): +0..2 AI type, +3..5 x,
//! +6..8 y, +9..11 z, +0xc..0xf hours. AI types: 0 fixed, 1 wander within 3 of the post,
//! 2 wander anywhere, 3 flee from the party when within 3, 4 guard post (chase when the party is
//! within 3 of the post, else wander near it), 5/7 chase (7 = people, raised by the alarm),
//! 6 chase when within 3 (Shadowlord).
//!
//! Pathing: `build_path_grid` fills the 32x32 byte grid at *g_ool_buffer_ptr (0 free, 0x90
//! blocked, 5 goal, 0x46 start), `find_path_bfs` floods it (each visited cell stores the move
//! direction that reached it in its high nibble) and `build_move_list` traces back into the
//! NPC's move list (16 x {count, direction 1 E, 2 N, 3 W, 4 S} at 0x615e + 32*npc, read offset
//! 0x655e + 2*npc, 0xffff = none; 0x65c2 + 2*npc = blocked/stuck counter). Contact with the
//! party is reported in 0x65be (kind 'a' attack / 't' talk-arrest) and 0x65bf (NPC index).
//!
//! Ghidra split the single loop of `move_npcs` (b044) into eight extra "functions" (b064,
//! b136, b224, b32e, b370, b3aa, b49e, b568) at the targets of its state switch; they are kept
//! as documented no-ops and their code lives in `move_npcs`. a444 is likewise the tail of a430.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// NPC record offset (16 bytes per NPC).
#[inline] fn npc16(npc: i16) -> u16 { (npc as u16).wrapping_mul(0x10) }
/// Cell address inside the 32x32 pathing grid at *g_ool_buffer_ptr.
#[inline] fn grid(gm: &Game, y: u16, x: u16) -> u16 {
    gm.g16(g::OOL_BUFFER_PTR).wrapping_add(y.wrapping_mul(0x20)).wrapping_add(x)
}

/// orig 1000:a290 (211 bytes). Loads the current settlement's NPC data: g_location-1 selects the
/// file (group = (loc-1)/8: towne.npc, dwelling.npc, castle.npc, keep.npc at DGROUP 0x6d46/0x6d50/
/// 0x6d5e/0x6d6a) and the 0x240-byte block ((loc-1) mod 8): 0x200 bytes of schedules to 0x5d5e,
/// 0x20 NPC types to 0x659e and 0x20 dialogue indices that are stored as 16-bit values in the
/// NPC state records at +0xa (0x5f68 + 16*npc). g_location is decremented while working and
/// restored afterwards (as the original does).
pub fn load_npc_schedules(gm: &mut Game) {
    let loc = gm.g8(g::LOCATION).wrapping_sub(1);
    gm.set_g8(g::LOCATION, loc);
    let group = (loc >> 3) as u16;
    let name: u16 = match group {
        0 => 0x6d46, // "towne.npc"
        1 => 0x6d50, // "dwelling.npc"
        2 => 0x6d5e, // "castle.npc"
        3 => 0x6d6a, // "keep.npc"
        _ => 0, // TODO(verify): uninitialised in the original (locations > 32 never reach here)
    };
    let off = (loc as u16).wrapping_sub(group.wrapping_mul(8)).wrapping_mul(0x240);
    files::load_file_region(gm, off, 0x200, g::NPC_SCHEDULES, name);
    files::load_file_region(gm, off.wrapping_add(0x200), 0x20, g::NPC_TYPES, name);
    gm.with_buf(32, |gm, buf| {
        files::load_file_region(gm, off.wrapping_add(0x220), 0x20, buf, name);
        let mut p: u16 = 0x5f68; // dialogue index field of NPC 0
        let mut i: u16 = 0;
        loop {
            let v = gm.g8(buf.wrapping_add(i)) as u16;
            gm.set_g16(p, v);
            p = p.wrapping_add(0x10);
            i += 1;
            if !(i < 0x20) { break; }
        }
    });
    let loc = gm.g8(g::LOCATION).wrapping_add(1);
    gm.set_g8(g::LOCATION, loc);
}

/// orig 1000:a366 (201 bytes). Initialises the state records of NPCs 1..31 for `hour` on
/// entering a settlement from outside: an NPC with a non-zero type gets state 1, position from
/// the schedule entry chosen by `schedule_index_for_hour`, +8 = its type, +0xe = the schedule
/// index, +0xc = slot 0, an empty move list (pointer 0xffff, first count 0) and a zero stuck
/// counter; an NPC with type 0 gets state 0.
pub fn init_npc_states(gm: &mut Game, param_1: u16) {
    let hour = param_1;
    let mut npc: i16 = 1;
    loop {
        let n16 = npc16(npc);
        let st = g::NPC_STATES.wrapping_add(n16);
        if gm.gi8(g::NPC_TYPES.wrapping_add(npc as u16)) != 0 {
            let idx = schedule_index_for_hour(gm, hour as i8, npc) as u16;
            gm.set_g16(st, 1);
            let a = n16.wrapping_add(idx);
            let v = gm.g8(a.wrapping_add(0x5d61)) as u16;
            gm.set_g16(st.wrapping_add(2), v);
            let v = gm.g8(a.wrapping_add(0x5d64)) as u16;
            gm.set_g16(st.wrapping_add(4), v);
            let v = gm.g8(a.wrapping_add(0x5d67)) as u16;
            gm.set_g16(st.wrapping_add(6), v);
            let v = gm.g8(g::NPC_TYPES.wrapping_add(npc as u16)) as u16;
            gm.set_g16(st.wrapping_add(8), v);
            gm.set_g16(st.wrapping_add(0xe), idx);
            gm.set_g16(st.wrapping_add(0xc), 0);
            gm.set_g16(g::NPC_MOVE_PTRS.wrapping_add((npc as u16).wrapping_mul(2)), 0xffff);
            gm.set_g8(g::NPC_MOVE_LISTS.wrapping_add((npc as u16).wrapping_mul(0x20)), 0);
            gm.set_g16(0x65c2u16.wrapping_add((npc as u16).wrapping_mul(2)), 0); // stuck counter
        } else {
            gm.set_g16(st, 0);
        }
        npc += 1;
        if !(npc < 0x20) { break; }
    }
}

/// orig 1000:a430 (19 bytes, continues at a444). Breadth-first search from (y, x) for the
/// nearest ladder: `want_up` != 0 looks for ladder-up tiles 0xc8 (mode -1), 0 for ladder-down
/// 0xc9 (mode -2). Returns `find_path_bfs`'s result (1 found, g_dx/g_dy = the ladder; 0 none).
/// Ghidra order: (npc, want_up, y, x).
pub fn find_nearest_ladder(gm: &mut Game, param_1: u16, param_2: i16, param_3: u16, param_4: u16) -> i16 {
    let mode: i16 = if param_2 == 0 { -2 } else { -1 };
    find_path_bfs(gm, param_1 as i16, mode, 0, 0, param_3 as i16, param_4 as i16)
}

/// orig 1000:a444 (30 bytes). Not a function: the `want_up == 0` arm (mode -2) of
/// `find_nearest_ladder`, split off by Ghidra.
pub fn fn_1000_a444(gm: &mut Game) {}

/// orig 1000:a462 (345 bytes). Fills the 32x32 pathing grid at *g_ool_buffer_ptr for NPC `npc`
/// using the schedule index for g_hour: every cell is 0 (enterable per `npc_tile_passable` on
/// the party's floor) or 0x90 (blocked); for mode -1/-2 every map tile 0xc8/0xc9 (ladder up /
/// down) is a goal (5). Then every monster-table object with tile >= 0x40 in slots 31..1 and the
/// party (slot 0) within Manhattan distance 3 of the NPC's position block their cell (0x90).
/// For mode >= 0 the goal is (gy, gx) = 5; finally the start (sy, sx) is 0x46 (visited, start
/// marker 6, first expansion direction 4). Ghidra order: (npc, mode, gy, gx, sy, sx).
pub fn build_path_grid(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16, param_4: i16, param_5: i16, param_6: i16) {
    let (npc, mode, gy, gx, sy, sx) = (param_1, param_2, param_3, param_4, param_5, param_6);
    let mut ladder: i8 = 0;
    if mode == -1 { ladder = -0x38; } // 0xc8 ladder up
    if mode == -2 { ladder = -0x37; } // 0xc9 ladder down
    let sched = schedule_index_for_hour(gm, gm.g8(g::HOUR) as i8, npc) as i16;
    let mut y: i16 = 0;
    let mut row: u16 = 0;
    loop {
        let mut x: i16 = 0;
        loop {
            let r = npc_tile_passable(gm, sched, npc, gm.g8(g::PARTY_Z) as u16, y as u16, x as u16);
            let cell = gm.g16(g::OOL_BUFFER_PTR).wrapping_add(row).wrapping_add(x as u16);
            if r == 0 {
                gm.set_g8(cell, 0x90);
            } else {
                gm.set_g8(cell, 0);
            }
            if mode < 0 && gm.gi8(g::MAP.wrapping_add(row).wrapping_add(x as u16)) == ladder {
                gm.set_g8(cell, 5);
            }
            x += 1;
            if !(x < 0x20) { break; }
        }
        row = row.wrapping_add(0x20);
        y += 1;
        if !(y < 0x20) { break; }
    }
    let n16 = npc16(npc);
    let ny = gm.gi16(0x5f62u16.wrapping_add(n16));
    let nx = gm.gi16(0x5f60u16.wrapping_add(n16));
    let mut p: u16 = 0x5d52; // monster slot 31
    loop {
        if 0x3f < gm.g8(p) {
            let my = gm.g8(p.wrapping_add(3)) as i16;
            let mx = gm.g8(p.wrapping_add(2)) as i16;
            if manhattan_distance(gm, ny, nx, my, mx) < 4 {
                let cell = grid(gm, my as u16, mx as u16);
                gm.set_g8(cell, 0x90);
            }
        }
        p = p.wrapping_sub(8);
        if !(g::MONSTER_TABLE < p) { break; }
    }
    let py = gm.g8(0x5c5d) as i16; // party slot y
    let px = gm.g8(0x5c5c) as i16; // party slot x
    if manhattan_distance(gm, ny, nx, py, px) < 4 {
        let cell = grid(gm, py as u16, px as u16);
        gm.set_g8(cell, 0x90);
    }
    if -1 < mode {
        let cell = grid(gm, gy as u16, gx as u16);
        gm.set_g8(cell, 5);
    }
    let cell = grid(gm, sy as u16, sx as u16);
    gm.set_g8(cell, 0x46);
}

/// orig 1000:a5bc (378 bytes). Breadth-first flood of the pathing grid from (sy, sx) after
/// `build_path_grid`: a 32-entry circular queue of cells; each dequeued cell is expanded in the
/// four directions starting with the direction stored in its own high nibble (1 x-1, 2 y+1,
/// 3 x+1, 4 y-1) and rotating ((d & 3) + 1); unvisited cells (< 0x10) receive the direction in
/// their high nibble and are enqueued (unless the queue is full). Reaching a cell whose low
/// nibble is 5 (goal) returns 1 with g_dx/g_dy = that cell; an exhausted queue returns 0. Quirks
/// kept: bounds checks allow x or y == 0x20 (reads the next row / past the grid), the queue
/// wraps at 32. Ghidra order: (npc, mode, gy, gx, sy, sx).
pub fn find_path_bfs(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16, param_4: i16, param_5: i16, param_6: i16) -> i16 {
    let (npc, mode, gy, gx, sy, sx) = (param_1, param_2, param_3, param_4, param_5, param_6);
    let mut found: i16 = 0;
    let mut head: usize = 0;
    let mut tail: usize = 1;
    build_path_grid(gm, npc, mode, gy, gx, sy, sx);
    let mut qx = [0u8; 32];
    let mut qy = [0u8; 32];
    qx[0] = sx as u8;
    qy[0] = sy as u8;
    let mut dx: i16 = found;
    loop {
        // LAB_1000_a5f7
        if dx != 0 {
            found = dx;
            return if found == 0 { 0 } else { 1 };
        }
        let mut x = qx[head] as i16;
        let mut y = qy[head] as i16;
        let mut dir = (gm.g8(grid(gm, y as u16, x as u16)) >> 4) as u16;
        let mut k: i16 = 0;
        loop {
            // LAB_1000_a62c
            match dir {
                1 => { x -= 1; if x < 0 { dx = 2; } }
                2 => { y += 1; if 0x20 < y { dx = 2; } }
                3 => { x += 1; if 0x20 < x { dx = 2; } }
                4 => { y -= 1; if y < 0 { dx = 2; } }
                _ => {}
            }
            // LAB_1000_a64c
            if dx != 2 {
                let cell = grid(gm, y as u16, x as u16);
                if gm.g8(cell) < 0x10 {
                    let old = gm.g8(cell) & 0xf;
                    gm.set_g8(cell, (dir << 4) as u8);
                    if old == 5 {
                        dx = 1;
                        gm.set_gi16(g::DX, x);
                        gm.set_gi16(g::DY, y);
                        break;
                    }
                    if tail != head {
                        qx[tail] = x as u8;
                        qy[tail] = y as u8;
                        tail += 1;
                    }
                    if 0x1f < tail { tail = 0; }
                }
            }
            // LAB_1000_a6d0
            dx = 0;
            dir = (dir & 3) + 1;
            x = qx[head] as i16;
            y = qy[head] as i16;
            k += 1;
            if !(k < 4) { break; }
        }
        // LAB_1000_a6fe
        if dx != 1 {
            head += 1;
            if head == 0x20 { head = 0; }
            if head == tail {
                found = dx;
                return if found == 0 { 0 } else { 1 };
            }
            continue;
        }
        found = dx;
        return 1;
    }
}

/// orig 1000:a73c (386 bytes). Traces the BFS result back from the goal (y, x) to the start and
/// writes NPC `npc`'s move list (0x615e + 32*npc): each cell's high nibble is the direction that
/// reached it (1 x-1, 2 y+1, 3 x+1, 4 y-1) so the trace steps the opposite way, counting runs of
/// equal direction into {count, direction} pairs until the start marker (low nibble 6) or 16
/// pairs. The list is then reversed in place and every direction converted to the walking code
/// ((d + 1) & 3) + 1 (1 E, 2 N, 3 W, 4 S). The read offset (0x655e + 2*npc) is reset to 0.
/// Returns the list length in bytes. `mode` is unused. Ghidra order: (y, x, mode, npc).
pub fn build_move_list(gm: &mut Game, param_1: i16, param_2: i16, param_3: u16, param_4: i16) -> i16 {
    let (mut y, mut x, npc) = (param_1, param_2, param_4);
    let mut run: i8 = 0;
    let mut n: i16 = 0;
    gm.set_g16(g::NPC_MOVE_PTRS.wrapping_add((npc as u16).wrapping_mul(2)), 0);
    let list = g::NPC_MOVE_LISTS.wrapping_add((npc as u16).wrapping_mul(0x20));
    let mut cell = gm.g8(grid(gm, y as u16, x as u16));
    let mut d = cell >> 4;
    let mut prev = d;
    loop {
        let low = cell & 0xf;
        if d == 1 {
            x += 1;
        } else if d == 2 {
            y -= 1;
        } else if d == 3 {
            x -= 1;
        } else if d == 4 {
            y += 1;
        }
        if prev == d && low != 6 {
            run = run.wrapping_add(1);
        }
        if prev != d || low == 6 {
            gm.set_gi8(list.wrapping_add(n as u16), run);
            gm.set_g8(list.wrapping_add(n as u16).wrapping_add(1), prev);
            n += 2;
            if low == 6 { break; }
            run = 1;
            prev = d;
        }
        cell = gm.g8(grid(gm, y as u16, x as u16));
        d = cell >> 4;
        if !(n < 0x20) { break; }
    }
    let mut j = n - 2;
    if -1 < j {
        let mut i: i16 = 0;
        loop {
            let pi = list.wrapping_add(i as u16);
            let pj = list.wrapping_add(j as u16);
            let ci = gm.g8(pi);
            let cj = gm.g8(pj);
            gm.set_g8(pi, cj);
            gm.set_g8(pj, ci);
            let di = gm.g8(pi.wrapping_add(1));
            let dj = gm.g8(pj.wrapping_add(1));
            gm.set_g8(pi.wrapping_add(1), (dj.wrapping_add(1) & 3).wrapping_add(1));
            gm.set_g8(pj.wrapping_add(1), (di.wrapping_add(1) & 3).wrapping_add(1));
            i += 2;
            j -= 2;
            if !(i <= j) { break; }
        }
    }
    n
}

/// orig 1000:a8c2 (106 bytes). Steps g_dx/g_dy one cell in walking direction `dir` (1 E, 2 N,
/// 3 W, 4 S) clamping to 0..0x20 - but the clamps test the OTHER coordinate (g_dy for x moves,
/// g_dx for y moves), an original bug kept for fidelity.
pub fn step_dxdy(gm: &mut Game, param_1: i16) {
    let dir = param_1;
    if dir == 1 {
        let v = gm.gi16(g::DX).wrapping_add(1);
        gm.set_gi16(g::DX, v);
        if 0x20 < gm.gi16(g::DY) { gm.set_gi16(g::DX, 0x20); }
    } else if dir == 2 {
        let v = gm.gi16(g::DY).wrapping_sub(1);
        gm.set_gi16(g::DY, v);
        if gm.gi16(g::DX) < 0 { gm.set_gi16(g::DY, 0); }
    } else if dir == 3 {
        let v = gm.gi16(g::DX).wrapping_sub(1);
        gm.set_gi16(g::DX, v);
        if gm.gi16(g::DY) < 0 { gm.set_gi16(g::DX, 0); }
    } else if dir == 4 {
        let v = gm.gi16(g::DY).wrapping_add(1);
        gm.set_gi16(g::DY, v);
        if 0x20 < gm.gi16(g::DX) { gm.set_gi16(g::DY, 0x20); }
    }
}

/// orig 1000:a930 (67 bytes). Manhattan distance |x2 - x1| + |y2 - y1|. Ghidra order:
/// (y1, x1, y2, x2).
pub fn manhattan_distance(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16, param_4: i16) -> i16 {
    let mut ddx = param_4.wrapping_sub(param_2);
    let mut ddy = param_3.wrapping_sub(param_1);
    if ddx < 0 { ddx = ddx.wrapping_neg(); }
    if ddy < 0 { ddy = ddy.wrapping_neg(); }
    ddx.wrapping_add(ddy)
}

/// orig 1000:a974 (591 bytes). Movement relative to the party for AI types 3..7 of NPC `npc`
/// (schedule entry `sched`). If the NPC is adjacent (distance 1 to monster slot 0) and the AI
/// type is > 3 it reports contact instead of moving: AI 4/5 with a non-zero dialogue index ->
/// kind 't' (0x74, talk/arrest), AI 6/7 -> 'a' (0x61, attack), index in 0x65bf (AI 4/5 with
/// dialogue 0 falls through to movement). Otherwise the four walking directions are scored with
/// the distance to the party after the step (99 = not enterable per `npc_can_move_to`); AI 3
/// (flee) picks a direction that increases the distance (ties resolved by a coin flip, stops at
/// direction 4); other types pick the first direction that decreases it. The scan continues to
/// "directions" 5..7 where the original reads past its array (state pointer, current distance,
/// AI type compared with 99 - never equal) and picks direction 5..7 when candidate 1..3 keeps
/// the distance equal (such a direction moves nothing). AI 5 and 7 then have a 1-in-4 chance of
/// swapping to a random other enterable direction. The chosen step updates the state record and
/// the monster slot and sets g_view_dirty |= 2. Ghidra order: (sched, npc).
pub fn npc_move_relative_party(gm: &mut Game, param_1: i16, param_2: i16) {
    let (sched, npc) = (param_1, param_2);
    let n16 = npc16(npc);
    let st = g::NPC_STATES.wrapping_add(n16);
    let ai = gm.g8(g::NPC_SCHEDULES.wrapping_add(n16).wrapping_add(sched as u16)) as u16;
    let dist = manhattan_distance(
        gm,
        gm.gi16(st.wrapping_add(4)),
        gm.gi16(st.wrapping_add(2)),
        gm.g8(0x5c5d) as i16,
        gm.g8(0x5c5c) as i16,
    );
    let mut contact = false;
    if dist == 1 && 3 < ai as i16 {
        if ai == 4 || ai == 5 {
            if gm.gi16(st.wrapping_add(10)) != 0 {
                gm.set_g8(0x65be, 0x74); // g_npc_contact_kind = 't'
                contact = true;
            }
        } else {
            gm.set_g8(0x65be, 0x61); // g_npc_contact_kind = 'a'
            contact = true;
        }
        if contact {
            gm.set_g8(0x65bf, npc as u8); // g_npc_contact_idx
        }
    }
    if !contact {
        // LAB_1000_a9ea
        let py = gm.g8(0x5c5d) as i16; // party slot y
        let px = gm.g8(0x5c5c) as i16; // party slot x
        let mut cand = [0i16; 4];
        let mut d: i16 = 1;
        loop {
            gm.set_gi16(g::DX, gm.gi16(st.wrapping_add(2)));
            gm.set_gi16(g::DY, gm.gi16(st.wrapping_add(4)));
            step_dxdy(gm, d);
            let nx = gm.gi16(g::DX);
            let ny = gm.gi16(g::DY);
            if npc_can_move_to(gm, -1, npc, ny, nx) == 0 {
                cand[(d - 1) as usize] = 99;
            } else {
                cand[(d - 1) as usize] = manhattan_distance(gm, ny, nx, py, px);
            }
            d += 1;
            if !(d < 5) { break; }
        }
        let mut best: i16 = -1;
        let cur = manhattan_distance(
            gm,
            gm.gi16(st.wrapping_add(4)),
            gm.gi16(st.wrapping_add(2)),
            gm.g8(0x5c5d) as i16,
            gm.g8(0x5c5c) as i16,
        );
        // the original's local frame seen through the over-running "cand[4..6]" reads
        let over = |i: usize| -> i16 { match i { 4 => st as i16, 5 => cur, _ => ai as i16 } };
        let mut chosen: i16 = best;
        let mut d: i16 = 1;
        loop {
            // LAB_1000_aaa7
            let mut exit = false;
            if d < 5 && cand[(d - 1) as usize] != 99 {
                if ai == 3 {
                    if cur < cand[(d - 1) as usize] {
                        if best == -1 || util::rand_range(gm, 1, 0) != 0 {
                            best = d;
                        }
                        chosen = best;
                        if d == 4 { exit = true; }
                    }
                } else {
                    chosen = d;
                    if cand[(d - 1) as usize] < cur {
                        best = d;
                        exit = true;
                    }
                }
            }
            if exit { break; }
            // LAB_1000_aae4
            if 4 < d && over((d - 1) as usize) != 99 {
                chosen = d;
                if cand[(d - 5) as usize] == cur {
                    best = d;
                    break;
                }
            }
            d += 1;
            chosen = best;
            if !(d < 8) { break; }
        }
        let _ = chosen;
        // LAB_1000_ab22
        if (ai == 5 || ai == 7) && util::rand_range(gm, 0x3f, 0) < 0x10 {
            let mut alt = best;
            let mut d: i16 = 1;
            loop {
                if d != best && cand[(d - 1) as usize] != 99 && (alt == best || util::rand_range(gm, 0x3f, 0) < 0x10) {
                    alt = d;
                }
                d += 1;
                if !(d < 5) { break; }
            }
            best = alt;
        }
        if -1 < best {
            gm.set_gi16(g::DX, gm.gi16(st.wrapping_add(2)));
            gm.set_gi16(g::DY, gm.gi16(st.wrapping_add(4)));
            step_dxdy(gm, best);
            let slot8 = (gm.gi16(st.wrapping_add(0xc)) as u16).wrapping_mul(8);
            let nx = gm.gi16(g::DX);
            gm.set_gi16(st.wrapping_add(2), nx);
            gm.set_g8(0x5c5cu16.wrapping_add(slot8), nx as u8);
            let ny = gm.gi16(g::DY);
            gm.set_gi16(st.wrapping_add(4), ny);
            gm.set_g8(0x5c5du16.wrapping_add(slot8), ny as u8);
            let v = gm.g8(g::VIEW_DIRTY) | 2;
            gm.set_g8(g::VIEW_DIRTY, v);
        }
    }
}

/// orig 1000:abc8 (267 bytes). Schedule-change check for NPC `npc` at `hour`: if one of its
/// four schedule hours equals `hour`, the entry for the hour is chosen and the state set: 1 when
/// it is the entry already in effect; otherwise, with the NPC or its target on the party's
/// floor: 2 (both on it), 6/7 (NPC on it, target above/below), 4/5 (target on it, NPC
/// above/below); 8 when neither is. If the NPC already stands exactly at the chosen entry's
/// (x, y, z) the state becomes 1 and the result 0 (-> returns 1). Returns 0 when no hour
/// matched, else entry+1 (non-zero = the caller runs the state machine instead of the AI).
/// Ghidra order: (hour, npc).
pub fn check_schedule_change(gm: &mut Game, param_1: u16, param_2: i16) -> i16 {
    let (hour, npc) = (param_1, param_2);
    let mut r: i16 = -1;
    let n16 = npc16(npc);
    let st = g::NPC_STATES.wrapping_add(n16);
    let sc = g::NPC_SCHEDULES.wrapping_add(n16);
    let pz = gm.g8(g::PARTY_Z);
    let mut i: u16 = 0;
    loop {
        if gm.g8(sc.wrapping_add(i).wrapping_add(0xc)) as u16 == hour {
            r = schedule_index_for_hour(gm, hour as i8, npc) as i16;
            let tz = sc.wrapping_add(r as u16).wrapping_add(9);
            if gm.gi16(st.wrapping_add(0xe)) == r {
                gm.set_g16(st, 1);
            } else if gm.g16(st.wrapping_add(6)) == pz as u16 || gm.g8(tz) == pz {
                if gm.g16(st.wrapping_add(6)) == pz as u16 {
                    if gm.g8(tz) == pz {
                        gm.set_g16(st, 2);
                    } else if (pz as i8) < gm.gi8(tz) {
                        gm.set_g16(st, 6);
                    } else {
                        gm.set_g16(st, 7);
                    }
                } else if (pz as i8) < gm.gi8(st.wrapping_add(6)) {
                    gm.set_g16(st, 4);
                } else {
                    gm.set_g16(st, 5);
                }
            } else {
                gm.set_g16(st, 8);
            }
            break;
        }
        i += 1;
        if !(i < 4) { break; }
    }
    if -1 < r {
        let e = sc.wrapping_add(r as u16);
        if gm.g16(st.wrapping_add(2)) == gm.g8(e.wrapping_add(3)) as u16
            && gm.g16(st.wrapping_add(4)) == gm.g8(e.wrapping_add(6)) as u16
            && gm.g16(st.wrapping_add(6)) == gm.g8(e.wrapping_add(9)) as u16
        {
            r = 0;
            gm.set_g16(st, 1);
        }
    }
    if r == -1 { 0 } else { r + 1 }
}

/// orig 1000:acda (146 bytes). Whether NPC `npc` (via its monster slot position) stands on the
/// ladder that leads towards schedule entry `sched`'s floor: 0xc9 (down) when the target z is
/// below the party's floor, 0xc8 (up) otherwise; any tile with (tile & 0xf4) == 0xc4 (stairs)
/// also counts. Returns 1/0. Ghidra order: (sched, npc).
pub fn npc_on_ladder_for_schedule(gm: &mut Game, param_1: i16, param_2: i16) -> u16 {
    let (sched, npc) = (param_1, param_2);
    let n16 = npc16(npc);
    let slot8 = (gm.gi16(0x5f6au16.wrapping_add(n16)) as u16).wrapping_mul(8);
    let p = rest::get_map_tile(gm, gm.g8(0x5c5du16.wrapping_add(slot8)) as u16, gm.g8(0x5c5cu16.wrapping_add(slot8)) as u16);
    let mut tile = gm.g8(p);
    if gm.gi8(0x5d67u16.wrapping_add(sched as u16).wrapping_add(n16)) < gm.gi8(g::PARTY_Z) {
        if tile == 0xc9 { return 1; }
    } else {
        let p = rest::get_map_tile(gm, gm.g8(0x5c5du16.wrapping_add(slot8)) as u16, gm.g8(0x5c5cu16.wrapping_add(slot8)) as u16);
        tile = gm.g8(p);
        if tile == 200 { return 1; }
    }
    if (tile & 0xf4) == 0xc4 { return 1; }
    0
}

/// orig 1000:ad6c (192 bytes). Whether NPC `npc` may stand on map cell (y, x) of floor `z`:
/// returns 2 when (x, y, z) is exactly the position of its schedule entry `sched` (>= 0);
/// otherwise reads the tile (g_map_last_cell when out of 0..31) and returns 1 for a ladder
/// (0xc8/0xc9) while the NPC is in state 3 (walking to a ladder), else 1 when the tile's bit in
/// the NPC walk bitmap at DGROUP 0x367e (MSB first, bit set = blocked) is clear, 0 when blocked.
/// Ghidra order: (sched, npc, z, y, x).
pub fn npc_tile_passable(gm: &mut Game, param_1: i16, param_2: i16, param_3: u16, param_4: u16, param_5: u16) -> u8 {
    let (sched, npc, z, y, x) = (param_1, param_2, param_3, param_4, param_5);
    let e = (sched as u16).wrapping_add(npc16(npc)).wrapping_add(g::NPC_SCHEDULES);
    if sched < 0
        || gm.g8(e.wrapping_add(3)) as u16 != x
        || gm.g8(e.wrapping_add(6)) as u16 != y
        || gm.g8(e.wrapping_add(9)) as u16 != z
    {
        let mut tile = gm.g8(g::MAP_LAST_CELL);
        if -1 < x as i16 && -1 < y as i16 && (x as i16) < 0x20 && (y as i16) < 0x20 {
            tile = gm.g8(x.wrapping_add(y.wrapping_mul(0x20)).wrapping_add(g::MAP));
        }
        if gm.gi16(g::NPC_STATES.wrapping_add(npc16(npc))) == 3 && (tile == 200 || tile == 0xc9) {
            1
        } else {
            // g_npc_walk_bitmaps + 8 (0x367e): one bit per tile, MSB first
            ((gm.g8(((tile >> 3) as u16).wrapping_add(0x367e)) as u16 & (0x80u16 >> (tile & 7))) == 0) as u8
        }
    } else {
        2
    }
}

/// orig 1000:ae2e (178 bytes). Whether NPC `npc` can step onto (y, x) this turn: 0 outside
/// 0..31; a Shadowlord (type 0xfc) or a tile of group 0x30 is always allowed (1); tile group
/// 0x90 is refused unless the NPC is in state 2; otherwise `npc_tile_passable` on the party's
/// floor (1, or 2 = the schedule target itself). Any object already at (y, x)
/// (find_object_at_rev) makes the result 0. Ghidra order: (sched, npc, y, x).
pub fn npc_can_move_to(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16, param_4: i16) -> i16 {
    let (sched, npc, y, x) = (param_1, param_2, param_3, param_4);
    if x < 0 || 0x1f < x || y < 0 || 0x1f < y {
        return 0;
    }
    let p = rest::get_map_tile(gm, y as u16, x as u16);
    let tile = gm.g8(p);
    let mut r: i16;
    if gm.gi8(g::NPC_TYPES.wrapping_add(npc as u16)) == -4 || (tile & 0xfc) == 0x30 {
        r = 1;
    } else if (tile & 0xfc) == 0x90 && gm.gi16(g::NPC_STATES.wrapping_add(npc16(npc))) != 2 {
        r = 0;
    } else {
        r = npc_tile_passable(gm, sched, npc, gm.g8(g::PARTY_Z) as u16, y as u16, x as u16) as i16;
    }
    let obj = rest::find_object_at_rev(gm, gm.g8(g::PARTY_Z) as u16, y as u16, x as u16);
    if r == 0 || obj != 0 {
        r = 0;
    }
    r
}

/// orig 1000:aee0 (175 bytes). Random wandering: with probability 1/2 (bit 3 of a random byte)
/// a random walking direction is tried from the NPC's position; the step is taken when
/// `max_dist` is 0 or the new cell is within `max_dist` of schedule entry `sched`'s (x, y) and
/// `npc_can_move_to` allows it; the state record and monster slot are updated and g_view_dirty
/// |= 2. Ghidra order: (sched_rec, npc, sched, max_dist, state_rec).
pub fn npc_wander(gm: &mut Game, param_1: i16, param_2: u16, param_3: i16, param_4: i16, param_5: i16) {
    let (sched_rec, npc, sched, max_dist, st) = (param_1 as u16, param_2 as i16, param_3, param_4, param_5 as u16);
    let r = util::rand_range(gm, 0xff, 0) as u16;
    if (r & 8) != 0 {
        let r2 = rest::rand_0_to(gm, 0x40) as u16;
        gm.set_gi16(g::DX, gm.gi16(st.wrapping_add(2)));
        gm.set_gi16(g::DY, gm.gi16(st.wrapping_add(4)));
        step_dxdy(gm, ((r2 & 3) + 1) as i16);
        let ny = gm.gi16(g::DY);
        let nx = gm.gi16(g::DX);
        let near_ok = max_dist == 0 || {
            let e = sched_rec.wrapping_add(sched as u16);
            manhattan_distance(gm, ny, nx, gm.g8(e.wrapping_add(6)) as i16, gm.g8(e.wrapping_add(3)) as i16) <= max_dist
        };
        if near_ok && npc_can_move_to(gm, sched, npc, ny, nx) != 0 {
            let slot8 = (gm.gi16(st.wrapping_add(0xc)) as u16).wrapping_mul(8);
            gm.set_gi16(st.wrapping_add(2), nx);
            gm.set_g8(0x5c5cu16.wrapping_add(slot8), nx as u8);
            gm.set_gi16(st.wrapping_add(4), ny);
            gm.set_g8(0x5c5du16.wrapping_add(slot8), ny as u8);
            let v = gm.g8(g::VIEW_DIRTY) | 2;
            gm.set_g8(g::VIEW_DIRTY, v);
        }
    }
}

/// orig 1000:af90 (159 bytes). Per-turn AI of NPC `npc` by the AI type of schedule entry
/// `sched`: 1 wander within 3 of the entry's position; 2 wander freely; 3 and 6 act
/// (`npc_move_relative_party`: flee / chase) only when the party is within 3 cells; 4 chases
/// when the party is within 3 cells of the post, else wanders within 3 of it; 5 and 7 always
/// chase; 0 (and others) stand still. Ghidra order: (sched, npc).
pub fn run_npc_ai(gm: &mut Game, param_1: i16, param_2: i16) {
    let (sched, npc) = (param_1, param_2);
    let n16 = npc16(npc);
    let sc = n16.wrapping_add(g::NPC_SCHEDULES);
    let st = n16.wrapping_add(g::NPC_STATES);
    let ai = gm.g8(sc.wrapping_add(sched as u16));
    match ai {
        2 => {
            npc_wander(gm, sc as i16, npc as u16, sched, 0, st as i16);
        }
        3 | 6 => {
            let d = manhattan_distance(
                gm,
                gm.gi16(n16.wrapping_add(0x5f62)),
                gm.gi16(n16.wrapping_add(0x5f60)),
                gm.g8(g::PARTY_Y) as i16,
                gm.g8(g::PARTY_X) as i16,
            );
            if 3 < d { return; }
            npc_move_relative_party(gm, sched, npc);
        }
        5 | 7 => {
            npc_move_relative_party(gm, sched, npc);
        }
        4 => {
            let e = sc.wrapping_add(sched as u16);
            let d = manhattan_distance(gm, gm.g8(e.wrapping_add(6)) as i16, gm.g8(e.wrapping_add(3)) as i16, gm.g8(g::PARTY_Y) as i16, gm.g8(g::PARTY_X) as i16);
            if d < 4 {
                npc_move_relative_party(gm, sched, npc);
            } else {
                npc_wander(gm, sc as i16, npc as u16, sched, 3, st as i16);
            }
        }
        1 => {
            npc_wander(gm, sc as i16, npc as u16, sched, 3, st as i16);
        }
        _ => {}
    }
}

/// orig 1000:b044 (31 bytes + the fragments b064..b568, 1318 bytes in all). One movement turn
/// for NPCs 1..31 (vector 1778:0339; TOWN main loop and CMDS). Clears the contact report
/// (0x65be/0x65bf). For each NPC with a non-zero type the schedule entry for `hour` is chosen;
/// an NPC in state 0/1 whose `check_schedule_change` returns 0 just runs its AI (`run_npc_ai`)
/// when on the party's floor. Otherwise by state: 2/3 (and 1) follow the move list - one step per
/// turn via `step_dxdy` and `npc_can_move_to`; a blocked step bumps the stuck counter, wanders,
/// and after 4 drops the list; an empty list in state 3 marks the NPC 6/7 and ENDS the whole
/// turn (original early return); otherwise, if no NPC has path-found yet this turn, a stuck
/// counter < 200 (and a 1-in-3 chance when non-zero) triggers a new BFS to the schedule target
/// (failure sets the counter to 200, counts 200..204 then wrap to 0) else the NPC wanders; after
/// a path-find the remaining NPCs on this floor only run their AI with their stored entry.
/// State 4/5 (arriving from another floor; one per turn): find the ladder nearest the target
/// (0xc8 for 4, 0xc9 for 5), path from it to the target, place the NPC on the ladder cell when
/// it really is that ladder (or a stair 0xc4..0xc7) and go to state 2. State 6/7 (must leave the
/// floor): standing on the right ladder (`npc_on_ladder_for_schedule`) or state 8 -> `place_npc`
/// at the target (other floor), state 1, list cleared; else (once per turn) path to the nearest
/// ladder (0xc8 for 6, 0xc9 for 7) and go to state 3.
pub fn move_npcs(gm: &mut Game, param_1: u16) {
    let hour = param_1;
    let mut pathed: i16 = 0; // BP-0x12: a BFS has been run this turn
    gm.set_g8(0x65be, 0); // g_npc_contact_kind
    gm.set_g8(0x65bf, 0); // g_npc_contact_idx
    let mut npc: i16 = 1;
    while npc < 0x20 {
        'next: {
            if gm.gi8(g::NPC_TYPES.wrapping_add(npc as u16)) == 0 { break 'next; }
            let sched = schedule_index_for_hour(gm, hour as i8, npc) as i16; // BP-0xc
            let n16 = npc16(npc);
            let st = g::NPC_STATES.wrapping_add(n16); // BP-6
            let p2 = (npc as u16).wrapping_mul(2);
            let list = g::NPC_MOVE_LISTS.wrapping_add((npc as u16).wrapping_mul(0x20));
            let pz = gm.g8(g::PARTY_Z) as u16;
            if gm.gi16(st) <= 1 && check_schedule_change(gm, hour, npc) == 0 {
                // LAB_1000_b4de
                if gm.g16(st.wrapping_add(6)) == pz {
                    run_npc_ai(gm, sched, npc);
                }
                break 'next;
            }
            // LAB_1000_b532
            let s = gm.gi16(st);
            if s <= 3 {
                // FUN_1000_b224: follow the move list
                let ptr = gm.gi16(g::NPC_MOVE_PTRS.wrapping_add(p2));
                let has_step = ptr > -1 && gm.gi8(list.wrapping_add(ptr as u16)) != 0;
                if has_step {
                    gm.set_gi16(g::DX, gm.gi16(st.wrapping_add(2)));
                    gm.set_gi16(g::DY, gm.gi16(st.wrapping_add(4)));
                    let dir = gm.g8(list.wrapping_add(ptr as u16).wrapping_add(1)) as i16;
                    step_dxdy(gm, dir);
                    let tx = gm.gi16(g::DX);
                    let ty = gm.gi16(g::DY);
                    let ok = npc_can_move_to(gm, sched, npc, ty, tx);
                    if ok == 0 {
                        // DNGLOOK_b32e: blocked
                        let c = gm.gi16(0x65c2u16.wrapping_add(p2)).wrapping_add(1);
                        gm.set_gi16(0x65c2u16.wrapping_add(p2), c);
                        npc_wander(gm, n16.wrapping_add(g::NPC_SCHEDULES) as i16, npc as u16, sched, 0, st as i16);
                        if 3 < gm.gi16(0x65c2u16.wrapping_add(p2)) {
                            gm.set_g16(g::NPC_MOVE_PTRS.wrapping_add(p2), 0xffff);
                            gm.set_g16(0x65c2u16.wrapping_add(p2), 0);
                        }
                        break 'next;
                    }
                    let slot8 = (gm.gi16(st.wrapping_add(0xc)) as u16).wrapping_mul(8);
                    gm.set_g8(0x5c5cu16.wrapping_add(slot8), tx as u8);
                    gm.set_g16(st.wrapping_add(2), tx as u8 as u16);
                    gm.set_g8(0x5c5du16.wrapping_add(slot8), ty as u8);
                    gm.set_g16(st.wrapping_add(4), ty as u8 as u16);
                    let v = gm.g8(g::VIEW_DIRTY) | 2;
                    gm.set_g8(g::VIEW_DIRTY, v);
                    let cell = list.wrapping_add(ptr as u16);
                    let c = gm.g8(cell).wrapping_sub(1);
                    gm.set_g8(cell, c);
                    gm.set_g16(0x65c2u16.wrapping_add(p2), 0);
                    if c != 0 { break 'next; }
                    let mp = g::NPC_MOVE_PTRS.wrapping_add(p2);
                    let a = gm.g16(mp).wrapping_add(1);
                    gm.set_g16(mp, a.wrapping_add(1));
                    gm.set_g8(list.wrapping_add(a), 0);
                    let np = gm.gi16(mp);
                    if 0x1f < np || gm.g8(list.wrapping_add(np as u16)) < 1 {
                        gm.set_g16(mp, 0xffff);
                    }
                    if ok == 2 {
                        gm.set_g16(st.wrapping_add(0xe), sched as u16);
                        gm.set_g16(st, 1);
                        gm.set_g16(mp, 0xffff);
                    }
                    break 'next;
                }
                // FUN_1000_b370: no (more) list
                if gm.gi16(g::NPC_MOVE_PTRS.wrapping_add(p2)) == -1 && gm.gi16(st) == 3 {
                    let idx = schedule_index_for_hour(gm, hour as i8, npc) as u16;
                    if (gm.g8(g::PARTY_Z) as i8) < gm.gi8(idx.wrapping_add(n16).wrapping_add(0x5d67)) {
                        gm.set_g16(st, 6);
                    } else {
                        gm.set_g16(st, 7);
                    }
                    return; // original quirk: ends the whole turn (FUN_1000_b568)
                }
                if 0 < pathed {
                    // COMBAT_b49e
                    if gm.g16(st.wrapping_add(6)) != pz { break 'next; }
                    let _unused: i16 = if sched == 1 || sched == 2 { 0 } else if sched == 0 { 2 } else { sched - 1 };
                    run_npc_ai(gm, gm.gi16(st.wrapping_add(0xe)), npc);
                    break 'next;
                }
                if gm.gi16(st) == 1 { break 'next; }
                let stuck = gm.gi16(0x65c2u16.wrapping_add(p2));
                if stuck < 200 && (stuck == 0 || util::rand_range(gm, 2, 0) == 1) {
                    // LAB_1000_b3f0
                    if gm.gi16(g::NPC_MOVE_PTRS.wrapping_add(p2)) == -1 {
                        pathed += 1;
                        let e = n16.wrapping_add(sched as u16);
                        let ok = find_path_bfs(
                            gm,
                            npc,
                            0,
                            gm.g8(e.wrapping_add(0x5d64)) as i16,
                            gm.g8(e.wrapping_add(0x5d61)) as i16,
                            gm.gi16(st.wrapping_add(4)),
                            gm.gi16(st.wrapping_add(2)),
                        );
                        if ok != 0 {
                            build_move_list(gm, gm.g8(e.wrapping_add(0x5d64)) as i16, gm.g8(e.wrapping_add(0x5d61)) as i16, 0, npc);
                            gm.set_g16(0x65c2u16.wrapping_add(p2), 0); // LAB_1000_b366
                            break 'next;
                        }
                        gm.set_g16(0x65c2u16.wrapping_add(p2), 200);
                    }
                    // LAB_1000_b455
                    npc_wander(gm, n16.wrapping_add(g::NPC_SCHEDULES) as i16, npc as u16, sched, 0, st as i16);
                } else {
                    // LAB_1000_b478
                    let c = 0x65c2u16.wrapping_add(p2);
                    if 199 < gm.gi16(c) {
                        let v = gm.gi16(c).wrapping_add(1);
                        gm.set_gi16(c, v);
                    }
                    if 0xcc < gm.gi16(c) {
                        gm.set_gi16(c, 0);
                    }
                }
                break 'next;
            }
            if s == 4 || s == 5 {
                // LAB_1000_b54c
                if pathed == 1 { break 'next; }
                pathed = 1;
                let mode: i16 = if s != 4 { 4 } else { 3 }; // FUN_1000_b064 / LAB_1000_b55f
                // LAB_1000_b069
                let e = n16.wrapping_add(sched as u16);
                let ty_ = gm.g8(e.wrapping_add(0x5d64)) as u16;
                let tx_ = gm.g8(e.wrapping_add(0x5d61)) as u16;
                let mut ok = find_nearest_ladder(gm, npc as u16, (mode == 3) as i16, ty_, tx_);
                let mut tx: i16 = 0;
                let mut ty: i16 = 0;
                if ok != 0 {
                    tx = gm.gi16(g::DX);
                    ty = gm.gi16(g::DY);
                    ok = find_path_bfs(gm, npc, mode, ty_ as i16, tx_ as i16, ty, tx);
                }
                if ok == 0 { break 'next; }
                build_move_list(gm, ty_ as i16, tx_ as i16, mode as u16, npc);
                let tile = gm.g8((tx as u16).wrapping_add((ty as u16).wrapping_mul(0x20)).wrapping_add(g::MAP));
                if (mode == 3 && tile == 0xc8) || (mode == 4 && tile == 0xc9) || (tile & 0xfc) == 0xc4 {
                    ovl::town::place_npc(gm, pz, ty as u16, tx as u16, npc);
                }
                gm.set_g16(st, 2);
                break 'next;
            }
            // FUN_1000_b136: states 6, 7 (and 8)
            if (s == 6 || s == 7) && npc_on_ladder_for_schedule(gm, sched, npc) == 0 {
                // LAB_1000_b18e
                let s = gm.gi16(st);
                if s == 2 { break 'next; }
                if pathed == 1 { break 'next; }
                pathed = 1;
                let mode: i16 = if s == 6 { 1 } else { 2 };
                let mut ok = find_nearest_ladder(gm, npc as u16, (mode == 1) as i16, gm.g16(st.wrapping_add(4)), gm.g16(st.wrapping_add(2)));
                let mut tx: i16 = 0;
                let mut ty: i16 = 0;
                if ok != 0 {
                    tx = gm.gi16(g::DX);
                    ty = gm.gi16(g::DY);
                    ok = find_path_bfs(gm, npc, mode, ty, tx, gm.gi16(st.wrapping_add(4)), gm.gi16(st.wrapping_add(2)));
                }
                if ok == 0 { break 'next; }
                build_move_list(gm, ty, tx, mode as u16, npc);
                gm.set_g16(st, 3);
                break 'next;
            }
            // LAB_1000_b14f: jump straight to the schedule target (other floor)
            let e = n16.wrapping_add(sched as u16);
            ovl::town::place_npc(
                gm,
                gm.g8(e.wrapping_add(0x5d67)) as u16,
                gm.g8(e.wrapping_add(0x5d64)) as u16,
                gm.g8(e.wrapping_add(0x5d61)) as u16,
                npc,
            );
            gm.set_g16(st.wrapping_add(0xe), sched as u16);
            gm.set_g16(g::NPC_MOVE_PTRS.wrapping_add(p2), 0xffff);
            gm.set_g16(st, 1);
        }
        npc += 1;
    }
}

/// orig 1000:b064 (210 bytes). Not a function: the state-5 arm (mode 4) of `move_npcs`.
pub fn fn_1000_b064(gm: &mut Game) {}

/// orig 1000:b136 (236 bytes). Not a function: the state-6/7/8 arm of `move_npcs`.
pub fn fn_1000_b136(gm: &mut Game) {}

/// orig 1000:b224 (266 bytes). Not a function: the move-list following arm of `move_npcs`.
pub fn fn_1000_b224(gm: &mut Game) {}

/// orig 1000:b32e (65 bytes). Not a function: the blocked-step arm of `move_npcs`.
pub fn fn_1000_b32e(gm: &mut Game) {}

/// orig 1000:b370 (288 bytes). Not a function: the no-list arm (BFS / wander) of `move_npcs`.
pub fn fn_1000_b370(gm: &mut Game) {}

/// orig 1000:b3aa (10 bytes). Not a function: `state = 7; return` inside `move_npcs`.
pub fn fn_1000_b3aa(gm: &mut Game) {}

/// orig 1000:b49e (200 bytes). Not a function: the after-path-find AI arm of `move_npcs`.
pub fn fn_1000_b49e(gm: &mut Game) {}

/// orig 1000:b568 (8 bytes). Not a function: the epilogue of `move_npcs`.
pub fn fn_1000_b568(gm: &mut Game) {}

/// orig 1000:b570 (74 bytes). Chooses NPC `npc`'s schedule entry for `hour` (vector 1778:03b6;
/// TOWN, TALK): the entry whose hour (bytes +0xc..+0xf) is the most recent, i.e. the smallest
/// (hour - entry_hour) as an unsigned byte, ties going to the earlier entry. Entries 0..2 return
/// 0..2; the 4th hour returns 1 (the NPC goes back to its second position). Ghidra order:
/// (hour, npc).
pub fn schedule_index_for_hour(gm: &mut Game, param_1: i8, param_2: i16) -> u8 {
    let (hour, npc) = (param_1 as u8, param_2);
    let n16 = npc16(npc);
    let d0 = hour.wrapping_sub(gm.g8(0x5d6au16.wrapping_add(n16)));
    let d1 = hour.wrapping_sub(gm.g8(0x5d6bu16.wrapping_add(n16)));
    let d2 = hour.wrapping_sub(gm.g8(0x5d6cu16.wrapping_add(n16)));
    let d3 = hour.wrapping_sub(gm.g8(0x5d6du16.wrapping_add(n16)));
    let mut best = d0;
    let mut r: u8 = (d1 < d0) as u8;
    if d1 < best { best = d1; }
    if d2 < best {
        r = 2;
        best = d2;
    }
    if d3 < best {
        r = 1;
    }
    r
}
