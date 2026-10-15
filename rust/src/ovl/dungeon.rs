//! DUNGEON.OVL (translated)
//!
//! The first-person dungeon mode. ROOT main calls `dungeon_main` (vector 1778:0296) when
//! g_location is 0x21..0x28. The 8x8x8 map of the current dungeon lives in the save game at
//! g_dungeon_map (0x595a; cell = type nibble << 4 | parameter, bit 8 of open cells = ceiling
//! hole). The view is composed on page 1 from DNGn.16 wall slices (left half plain, right half
//! mirrored via draw_image_alt), ITEMS.16 objects (ladders up are the ladder-down image with the
//! vertical-flip flag) and the wandering monster's MONn.16 sprite, then copied to page 0.
//! Rooms (cells 0xf0 / 0xa0) are fought in the COMBAT overlay (`enter_dungeon_room`); DNGLOOK
//! owns the dungeon-specific look/search/camp code and the image loading (its stubs are called
//! by their fn_1000_xxxx names).
#![allow(unused_variables, dead_code, unused_imports, non_snake_case, unused_assignments)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// DGROUP offset of dungeon-map cell (z, y, x) in g_dungeon_map (0x595a, 8 levels of 8x8).
/// Coordinates are NOT masked here (callers that mask do it themselves).
#[inline] fn cell_off(z: u16, y: u16, x: u16) -> u16 {
    g::DUNGEON_MAP.wrapping_add(z.wrapping_mul(0x40)).wrapping_add(y.wrapping_mul(8)).wrapping_add(x)
}
/// Offset of the cell the party stands on: (g_party_z & 0xff) * 0x40 + g_party_y * 8 + g_party_x.
#[inline] fn party_cell_off(gm: &Game) -> u16 {
    cell_off(gm.g8(g::PARTY_Z) as u16, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16)
}
/// Dungeon monster record (monster-table slot 1, 0x5c62..) and its previous position (slot 2).
const DNG_MONSTER_KIND: u16 = 0x5c62;    // u8 x2: index 0..7 into the per-dungeon monster tables
const DNG_MONSTER_X: u16 = 0x5c64;
const DNG_MONSTER_Y: u16 = 0x5c65;
const DNG_MONSTER_Z: u16 = 0x5c66;
const DNG_MONSTER_TYPE: u16 = 0x5c67;    // i8: monster type, -1 = none
const DNG_MONSTER_ANIM: u16 = 0x5c68;    // u8: sprite state (bits 0x60 kind, 0x90 facing, 0xf frame)
const DNG_MONSTER_ALT: u16 = 0x5c69;     // u8: 0xff = use the alternate sprite set
const DNG_MONSTER_PREV_X: u16 = 0x5c6c;
const DNG_MONSTER_PREV_Y: u16 = 0x5c6d;
/// Image segments: DNGn.16 walls, ITEMS.16, MONn.16 (loaded by DNGLOOK abca / spawn_dungeon_monster).
const DNG_WALL_IMG_SEG: u16 = 0xa9c0;
const DNG_ITEMS_IMG_SEG: u16 = 0xa9c4;
const DNG_MONSTER_IMG_SEG: u16 = 0xa9c6;
/// Footstep sound state: remaining frequency (decremented 4 per tick) and the on/off toggle.
const DNG_STEP_SOUND: u16 = 0xa9fb;
const DNG_STEP_TOGGLE: u16 = 0x24e7;
/// Fountain animation frame counter 0..2 (byte inside the g_unk_2f16 table).
const DNG_FOUNTAIN_FRAME: u16 = 0x2f26;
/// Direction deltas by facing (0 N, 1 E, 2 S, 3 W): i16 dx[4] at 0x24d6, dy[4] at 0x24de.
const DNG_DX: u16 = 0x24d6;
const DNG_DY: u16 = 0x24de;

/// orig 1000:81d0 (307 bytes). Enter a dungeon room (cell type 0xa0 = cleared room, 0xf0 = room):
/// prints "Entering room...", converts g_dungeon_dir_copy into a facing through the table at
/// 0x2c76, clears the active member/combatant, zeroes the 0x160-byte g_light_map buffer and loads
/// room (cell & 0xf) of this dungeon from DUNGEON.CBT (8 dungeons x 16 rooms x 0x160 bytes; the
/// dungeon index is location-0x21, but 0x22.. map to location-0x22 - quirk kept). Saves monster
/// slots 1-2 (0x5c62..0x5c71), party x/y and g_location (backup at 0x5894), sets g_location=0xff,
/// g_combat_exit=0, runs DNGLOOK b40e(3, cell), refresh_map_view, g_combat_flags=0x82 and the
/// COMBAT room loop (ae24). If that returns 0: combat exit 'M' runs ENDGAME a8d8; g_location is
/// restored, DNGLOOK aad4(room) runs and the room cell's bits 0x50 are cleared (0xf0 -> 0xa0,
/// "cleared room"). Finally restores location, x/y, DNGLOOK b26a, the two monster slots and the
/// active member. Vector 1778:037a / 03fe (shared address with TOWN/MAINOUT 81d0).
pub fn enter_dungeon_room(gm: &mut Game, param_1: u8) {
    text::print_string(gm, 0x2c58); // "Entering room...\n"
    let v = gm.g8(0x2c76u16.wrapping_add(gm.g8(g::DUNGEON_DIR_COPY) as u16));
    gm.set_g8(g::DUNGEON_FACING, v);
    input::flush_keyboard(gm);
    let saved_active = gm.g8(g::ACTIVE_MEMBER);
    gm.set_g8(g::ACTIVE_MEMBER, 0xff);
    gm.set_g8(g::ACTIVE_COMBATANT, 0xff);
    let mut dng: i16 = gm.gi8(g::LOCATION) as i16 - 0x21;
    if 0 < dng {
        dng = gm.gi8(g::LOCATION) as i16 - 0x22; // TODO(verify): Deceit and Despise both map to 0
    }
    for i in 0..0x160u16 {
        gm.set_g8(g::LIGHT_MAP.wrapping_add(i), 0);
    }
    let off = (dng as u16).wrapping_mul(0x1600).wrapping_add(((param_1 & 0xf) as u16).wrapping_mul(0x160));
    files::load_file_region(gm, off, 0x160, g::LIGHT_MAP, 0x2c6a); // "dungeon.cbt"
    let mut saved_slots = [0u16; 8];
    for k in 0..8u16 {
        saved_slots[k as usize] = gm.g16(DNG_MONSTER_KIND.wrapping_add(k * 2));
    }
    let saved_y = gm.g8(g::PARTY_Y);
    let saved_x = gm.g8(g::PARTY_X);
    let loc = gm.g8(g::LOCATION);
    gm.set_g8(g::LOCATION_BACKUP, loc);
    let x = saved_x as u16;
    let y = saved_y as u16;
    gm.set_g8(g::LOCATION, 0xff);
    gm.set_g8(g::COMBAT_EXIT, 0);
    ovl::dnglook::setup_room_combatants(gm, 3, param_1 as i16);
    view::refresh_map_view(gm);
    gm.set_g8(g::COMBAT_FLAGS, 0x82);
    let r = ovl::combat::combat_main_loop(gm) as i16;
    if r == 0 {
        if gm.gi8(g::COMBAT_EXIT) == 0x4d { // 'M'
            ovl::endgame::endgame_sequence(gm);
        }
        let b = gm.g8(g::LOCATION_BACKUP);
        gm.set_g8(g::LOCATION, b);
        ovl::dnglook::mark_dungeon_room_cleared(gm, (param_1 & 0xf) as i16);
        let off = cell_off(gm.g8(g::PARTY_Z) as u16, y, x);
        let c = gm.g8(off) & 0xaf;
        gm.set_g8(off, c);
    }
    let b = gm.g8(g::LOCATION_BACKUP);
    gm.set_g8(g::LOCATION, b);
    gm.set_g8(g::PARTY_X, saved_x);
    gm.set_g8(g::PARTY_Y, saved_y);
    ovl::dnglook::apply_combat_exit_to_dungeon(gm);
    for k in 0..8u16 {
        gm.set_g16(DNG_MONSTER_KIND.wrapping_add(k * 2), saved_slots[k as usize]);
    }
    gm.set_g8(g::ACTIVE_MEMBER, saved_active);
}

/// orig 1000:8304 (157 bytes). (Re)spawns the single wandering dungeon monster and (re)loads
/// its MONn.16 sprite file. With `respawn` != 0: picks kind k = rand 0..7, stores k in both bytes
/// of 0x5c62, anim byte from table 0x1744[k], type from 0x173c[k] (14 15 16 17 18 19 1c 1b),
/// z = party level, alt flag 0; frees the old sprite segment (if a monster existed) and calls
/// `place_dungeon_monster`; if no free cell was found the kind is zeroed and the type set to -1.
/// Then, whenever a monster exists, loads file g_raw_file_names[8+k] (MON0..7.16) into
/// 0xa9c6, retrying until the load succeeds. Vector 1778:04fa (ROOT 3c9a calls it with 0).
pub fn spawn_dungeon_monster(gm: &mut Game, param_1: i16) {
    let old_type = gm.gi8(DNG_MONSTER_TYPE);
    if param_1 != 0 {
        let r = util::rand_range(gm, 7, 0);
        let k = r as u8;
        gm.set_g8(DNG_MONSTER_KIND + 1, k);
        gm.set_g8(DNG_MONSTER_KIND, k);
        gm.set_g8(DNG_MONSTER_ALT, 0);
        let a = gm.g8((r as u16).wrapping_add(0x1744));
        gm.set_g8(DNG_MONSTER_ANIM, a);
        let t = gm.g8((r as u16).wrapping_add(0x173c));
        gm.set_g8(DNG_MONSTER_TYPE, t);
        let z = gm.g8(g::PARTY_Z);
        gm.set_g8(DNG_MONSTER_Z, z);
        if old_type != -1 {
            let s = gm.g16(DNG_MONSTER_IMG_SEG);
            gfx::free_far_block(gm, s);
        }
        gm.set_g16(DNG_MONSTER_IMG_SEG, 0);
        if place_dungeon_monster(gm) == 0 {
            gm.set_g8(DNG_MONSTER_KIND + 1, 0);
            gm.set_g8(DNG_MONSTER_KIND, 0);
            gm.set_g8(DNG_MONSTER_TYPE, 0xff);
        }
    }
    if gm.gi8(DNG_MONSTER_TYPE) != -1 {
        loop {
            let name = gm.g16((gm.g8(DNG_MONSTER_KIND) as u16).wrapping_mul(2).wrapping_add(0x25fa));
            let s = gfx::load_image_file(gm, name) as u16;
            gm.set_g16(DNG_MONSTER_IMG_SEG, s);
            if s != 0 { break; }
        }
    }
}

/// orig 1000:83a2 (123 bytes). Prints the level number (z+1, width 1, pad ' ') at window-0
/// cursor (0,12) and the facing word at (23,12): "North"/" East"/"South"/" West"/" ????"
/// (0x2c7c/0x2c82/0x2c88/0x2c8e/0x2c94), restoring the current text window afterwards.
pub fn print_level_and_facing(gm: &mut Game) {
    let w = gm.g16(g::CUR_WINDOW);
    text::select_text_window(gm, 0);
    text::set_cursor_pos(gm, 0, 0xc);
    let lvl = gm.g8(g::PARTY_Z) as i16 + 1;
    text::print_number(gm, 0x20, 1, lvl);
    text::set_cursor_pos(gm, 0x17, 0xc);
    let f = gm.g8(g::DUNGEON_FACING);
    let s: u16 = if f == 0 { 0x2c7c } else if f == 1 { 0x2c82 } else if f == 2 { 0x2c88 } else if f == 3 { 0x2c8e } else { 0x2c94 };
    text::print_string(gm, s);
    text::select_text_window(gm, w);
}

/// orig 1000:8422 (223 bytes). Picks a cell for the dungeon monster on the party's level: up to
/// 8 random cells 0..63 are tried; the cell qualifies if its type nibble is < 0x60 (hallway,
/// ladders, chests, fountains) or 0x70 (open chest) and neither its x (cell%8) equals the party
/// x nor its y (cell/8) equals the party y. On success the monster x/y and previous x/y are set
/// to the cell, and for types 0x16/0x18 the alternate-sprite flag 0x5c69 is set to 0xff with
/// probability ~69% (rand(0..99) > 0x30); returns 1. After 8 failures all four position bytes
/// are set to 0xff and 0 is returned.
pub fn place_dungeon_monster(gm: &mut Game) -> u16 {
    let mut tries: i16 = 0;
    loop {
        let z = gm.g8(g::PARTY_Z) as u16;
        let cell = util::rand_range(gm, 0x3f, 0) as u16;
        let t = gm.g8(g::DUNGEON_MAP.wrapping_add(z.wrapping_mul(0x40)).wrapping_add(cell)) & 0xf0;
        if t < 0x60 || t == 0x70 {
            // signed division by 8 of the (non-negative) cell index
            let y = ((cell as i16) / 8) as u16;
            let x = ((cell as i16) % 8) as u16;
            if x != gm.g8(g::PARTY_X) as u16 && y != gm.g8(g::PARTY_Y) as u16 {
                gm.set_g8(DNG_MONSTER_X, x as u8);
                gm.set_g8(DNG_MONSTER_PREV_Y, y as u8);
                gm.set_g8(DNG_MONSTER_Y, y as u8);
                gm.set_g8(DNG_MONSTER_PREV_X, x as u8);
                let ty = gm.gi8(DNG_MONSTER_TYPE);
                if (ty == 0x16 || ty == 0x18) && util::rand_range(gm, 99, 0) > 0x30 {
                    gm.set_g8(DNG_MONSTER_ALT, 0xff);
                }
                return 1;
            }
        }
        tries += 1;
        if 7 < tries {
            gm.set_g8(DNG_MONSTER_PREV_Y, 0xff);
            gm.set_g8(DNG_MONSTER_Y, 0xff);
            gm.set_g8(DNG_MONSTER_PREV_X, 0xff);
            gm.set_g8(DNG_MONSTER_X, 0xff);
            return 0;
        }
    }
}

/// orig 1000:8502 (164 bytes). Draws the static dungeon-view frame on page 0: two bars in
/// g_ui_color2 (x 0x28..0x98 rows 0..7 and x 0x30..0x98 rows 0xb9..0xbf), two lines in
/// g_ui_color1 (row 7, x 0x28..0x98 and row 0xb8, x 0x30..0x98), then in text window 0 the
/// capped labels "L " at (0,10) and "Dir:      " at (23,6). Restores the current window.
pub fn draw_dungeon_frame(gm: &mut Game) {
    let w = gm.g16(g::CUR_WINDOW);
    let c2 = gm.gi16(g::UI_COLOR2);
    gfx::set_color(gm, c2);
    gfx::fill_rect(gm, 7, 0x98, 0, 0x28);
    gfx::fill_rect(gm, 0xbf, 0x98, 0xb9, 0x30);
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    gfx::draw_line(gm, 7, 0x98, 7, 0x28);
    gfx::draw_line(gm, 0xb8, 0x98, 0xb8, 0x30);
    text::select_text_window(gm, 0);
    text::set_cursor_pos(gm, 0, 10);
    rest::draw_bar_left_cap(gm);
    text::print_string(gm, 0x2c9a); // "L "
    rest::draw_bar_right_cap(gm);
    text::set_cursor_pos(gm, 0x17, 6);
    rest::draw_bar_left_cap(gm);
    text::print_string(gm, 0x2c9d); // "Dir:      "
    rest::draw_bar_right_cap(gm);
    text::select_text_window(gm, w);
}

/// orig 1000:85a6 (153 bytes). Waits for a command key while animating the view. Redraws the
/// status window if dirty, prints a newline and the prompt cap, then polls: a key 1..4 that did
/// not come from the keypad is turned into the 0xfb..0xfe direction codes. While no key is
/// pending it re-renders the view on page 1 (`render_dungeon_view(anim)`) and copies it to
/// page 0 (full width x 0x10..0xaf for the first/cave frames, x 0x28..0x96 otherwise, rows
/// 0xe..0xb2); the animated-water flag stays 1 only in style 1 (caves); then runs the footstep
/// sound tick. Returns the upper-cased key.
pub fn read_dungeon_command(gm: &mut Game) -> i16 {
    if gm.g8(g::STATUS_DIRTY) != 0 {
        rest::draw_status_window(gm);
        gm.set_g8(g::STATUS_DIRTY, 0);
    }
    text::put_char(gm, 10);
    rest::draw_bar_left_cap(gm);
    let mut anim: i16 = 1;
    loop {
        let k = input::poll_key_with_cursor(gm);
        let mut key = util::to_upper(k);
        if key != 0 && key < 5 && gm.g16(g::KEY_FROM_KEYPAD) == 0 {
            key = key.wrapping_add(0xfa);
        }
        if key == 0 {
            render_dungeon_view(gm, anim);
            let (x1, x2): (i16, i16) = if anim == 0 { (0x28, 0x96) } else { (0x10, 0xaf) };
            gfx::copy_rect_between_pages(gm, 0xb2, x2, 0xe, x1, 0, 1);
            if gm.g8(g::DUNGEON_STYLE) != 1 {
                anim = 0;
            }
            step_sound_tick(gm);
        }
        if key != 0 {
            return key;
        }
    }
}

/// orig 1000:8640 (145 bytes). The party walked into an energy field of type 3 (cell 0x83,
/// "Electric field!"): redraws the view, prints "Ouch!" / "Electric field!", flashes the whole
/// view rectangle (XOR fill in g_ui_color1, 1 tick), plays random tones, then pushes the party
/// one cell back the way it came (`step` = +1 advancing -> minus the facing delta, -1 backing up
/// -> plus the delta; the deltas are the low bytes of the dx/dy word tables 0x24d6/0x24de),
/// damages the whole party and redraws.
pub fn electric_field_push(gm: &mut Game, param_1: i16) {
    redraw_dungeon_view(gm);
    text::print_string(gm, 0x2ca8); // "Ouch!\n"
    text::print_string(gm, 0x2caf); // "Electric field!\n"
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
    util::wait_ticks(gm, 1);
    gfx::fill_rect_xor(gm, 0xb7, 0xb7, 8, 8);
    util::play_random_tones(gm, 0x4e20, 500, 1); // "s__Have_a_pleasant_night_ + 0xd" = 20000
    let f = (gm.g8(g::DUNGEON_FACING) as u16).wrapping_mul(2);
    let (dx, dy): (i8, i8) = if param_1 < 1 {
        (gm.gi8(DNG_DX.wrapping_add(f)), gm.gi8(DNG_DY.wrapping_add(f)))
    } else {
        (gm.gi8(DNG_DX.wrapping_add(f)).wrapping_neg(), gm.gi8(DNG_DY.wrapping_add(f)).wrapping_neg())
    };
    let y = gm.g8(g::PARTY_Y).wrapping_add(dy as u8);
    gm.set_g8(g::PARTY_Y, y);
    let x = gm.g8(g::PARTY_X).wrapping_add(dx as u8);
    gm.set_g8(g::PARTY_X, x);
    rest::damage_whole_party(gm);
    redraw_dungeon_view(gm);
}

/// orig 1000:86d2 (445 bytes). Executes a movement command on the party's current `cell`
/// (true order: cell, cmd). cmd 1 = "Turn left" (facing+3), 2 = "Turn right" (facing+1) - both
/// refused with "Not in doorway!" while standing in a door cell (0xe0); 3 = "Advance" (step +1),
/// 4 = "Back up" (step -1), anything else = "Turn around." (facing+2). g_dungeon_dir_copy is set
/// to the new facing. For a step the target cell is x/y + dx/dy*step wrapped 7<->0; an electric
/// field (0x83) there moves the party onto it and runs `electric_field_push`. Walls 0xb0/0xc0/0xd0
/// block, and rooms/cleared rooms (0xa0/0xf0) block when backing up ("Blocked!" 0x2d19); the
/// monster's cell blocks too ("Blocked!" 0x2d23). Otherwise the footstep sound is primed
/// (0xa9fb = 0xf, toggle cleared, one tick played) and the party x/y are set (&7). Always ends
/// with `redraw_dungeon_view`. Returns 1 if a step was attempted (time passes), else 0.
pub fn move_in_dungeon(gm: &mut Game, param_1: u8, param_2: i16) -> u16 {
    let cell = param_1;
    let cmd = param_2;
    let mut step: i16 = 0;
    let mut result: u16 = 0;
    if cmd == 1 {
        if (cell & 0xf0) == 0xe0 {
            text::print_string(gm, 0x2cef); // "Not in doorway!\n"
        } else {
            text::print_string(gm, 0x2d00); // "Turn left\n"
            let f = gm.g8(g::DUNGEON_FACING).wrapping_add(3) & 3;
            gm.set_g8(g::DUNGEON_FACING, f);
        }
    } else if cmd == 2 {
        if (cell & 0xf0) != 0xe0 {
            text::print_string(gm, 0x2cda); // "Turn right\n"
            let f = gm.g8(g::DUNGEON_FACING).wrapping_add(1) & 3;
            gm.set_g8(g::DUNGEON_FACING, f);
        } else {
            text::print_string(gm, 0x2cc9); // "Not in doorway!\n"
        }
    } else if cmd == 3 {
        text::print_string(gm, 0x2cc0); // "Advance\n"
        step = 1;
    } else if cmd != 4 {
        text::print_string(gm, 0x2d0b); // "Turn around.\n"
        let f = gm.g8(g::DUNGEON_FACING).wrapping_add(2) & 3;
        gm.set_g8(g::DUNGEON_FACING, f);
    } else {
        text::print_string(gm, 0x2ce6); // "Back up\n"
        step = -1;
    }
    let f = gm.g8(g::DUNGEON_FACING);
    gm.set_g8(g::DUNGEON_DIR_COPY, f);
    if step != 0 {
        result = 1;
        let f2 = (f as u16).wrapping_mul(2);
        let mut nx: i16 = gm.gi16(DNG_DX.wrapping_add(f2)).wrapping_mul(step).wrapping_add(gm.g8(g::PARTY_X) as i16);
        if nx < 0 { nx = 7; }
        if 7 < nx { nx = 0; }
        let mut ny: i16 = gm.gi16(DNG_DY.wrapping_add(f2)).wrapping_mul(step).wrapping_add(gm.g8(g::PARTY_Y) as i16);
        if ny < 0 { ny = 7; }
        if 7 < ny { ny = 0; }
        let mut c = gm.g8(cell_off(gm.g8(g::PARTY_Z) as u16, ny as u16, nx as u16));
        if c == 0x83 {
            gm.set_g8(g::PARTY_X, nx as u8);
            gm.set_g8(g::PARTY_Y, ny as u8);
            electric_field_push(gm, step);
            nx = gm.g8(g::PARTY_X) as i16;
            ny = gm.g8(g::PARTY_Y) as i16;
        }
        c &= 0xf0;
        let msg: u16;
        if (c < 0xa1 || 0xdf < c) && (step != -1 || (c != 0xa0 && c != 0xf0)) {
            if nx as u16 != gm.g8(DNG_MONSTER_X) as u16 || ny as u16 != gm.g8(DNG_MONSTER_Y) as u16 {
                gm.set_g8(DNG_STEP_SOUND, 0xf);
                gm.set_g8(DNG_STEP_TOGGLE, 0);
                step_sound_tick(gm);
                gm.set_g8(DNG_STEP_TOGGLE, 0);
                gm.set_g8(g::PARTY_X, (nx as u8) & 7);
                gm.set_g8(g::PARTY_Y, (ny as u8) & 7);
                redraw_dungeon_view(gm);
                return result;
            }
            msg = 0x2d23; // "Blocked!\n" (monster in the way)
        } else {
            msg = 0x2d19; // "Blocked!\n" (wall / room)
        }
        text::print_string(gm, msg);
    }
    redraw_dungeon_view(gm);
    result
}

/// orig 1000:8894 (281 bytes). Dispatches one dungeon command key. Keys 1..4 (direction codes
/// after read_dungeon_command's remap... see notes), 0xd (Enter) and 0x2e ('.') go to
/// `move_in_dungeon(cell, key)`; 0xb (^K) prints karma; 5 (^E) asks "Exit to DOS? " and exits on
/// 'Y' (else prints "N"); 0x13 (^S) toggles sound printing "Sound On/Off"; 0x16 (^V) prints the
/// version "1.16"; digits '0'..'9' call set_active_player (returns 0 = no time passes); anything
/// else goes to ROOT dispatch_command. Returns the "time passes" flag.
pub fn dungeon_command(gm: &mut Game, param_1: u16) -> u16 {
    let key = param_1;
    if key == 0xb {
        let k = gm.g8(g::KARMA) as i16;
        text::print_number(gm, 0x20, 1, k);
        text::put_char(gm, 10);
        return 1;
    }
    let mut do_move = false;
    if (key as i16) < 0xc {
        if 0 < (key as i16) {
            if (key as i16) < 5 {
                do_move = true;
            } else if key == 5 {
                text::print_string(gm, 0x2d2d); // "Exit to DOS? "
                let k = input::read_command_key(gm);
                if k == 0x59 { // 'Y'
                    gfx::restore_video_mode(gm);
                    std::process::exit(0);
                }
                text::print_string(gm, 0x2d3b); // "N\n"
                return 1;
            }
        }
    } else {
        if key != 0xd {
            if key == 0x13 {
                text::print_string(gm, 0x2d43); // "Sound "
                let s: u16 = if gm.gi8(g::SOUND_ENABLED) == 0 { 0x2d4f } else { 0x2d4a }; // "On\n" / "Off\n"
                text::print_string(gm, s);
                let v = (gm.gi8(g::SOUND_ENABLED) == 0) as u8;
                gm.set_g8(g::SOUND_ENABLED, v);
                return 1;
            }
            if key == 0x16 {
                text::print_string(gm, 0x2d3e); // "1.16"
                text::put_char(gm, 10);
                return 1;
            }
            if key == 0x2e {
                do_move = true;
            }
        } else {
            do_move = true;
        }
    }
    if do_move {
        let cell = gm.g8(party_cell_off(gm));
        return move_in_dungeon(gm, cell, key as i16);
    }
    if key < 0x30 || 0x39 < key {
        rest::dispatch_command(gm, key as i16)
    } else {
        rest::set_active_player(gm, key as i16);
        0
    }
}

/// orig 1000:89b2 (356 bytes). Moves the wandering monster one random step (type 0x1b never
/// moves). Up to 8 tries: a random direction 0..3 is applied to the monster x/y (wrapping 7<->0);
/// the target cell on the party's level is accepted if its type is not 0x60 (pit), not 0x80
/// (field) and below 0xa0 (no walls/doors/rooms), and if it is the previous position only with
/// probability 1/8. On acceptance prev x/y <- x/y and x/y <- target. Afterwards, if the monster
/// is on the party's cell it is moved back to its previous position and 1 is returned (the
/// monster attacks); else 0. Does nothing (returns 0) when no monster exists (type -1).
pub fn move_dungeon_monster(gm: &mut Game) -> u16 {
    let mut result: u16 = 0;
    let mut state: i16 = 999;
    if gm.gi8(DNG_MONSTER_TYPE) != -1 {
        if gm.gi8(DNG_MONSTER_TYPE) != 0x1b {
            let mut tries: i16 = 0;
            let py = gm.g8(DNG_MONSTER_PREV_Y) as u16;
            let px = gm.g8(DNG_MONSTER_PREV_X) as u16;
            let my = gm.g8(DNG_MONSTER_Y) as u16;
            let mx = gm.g8(DNG_MONSTER_X) as u16;
            let mut nx: u16 = 0;
            let mut ny: u16 = 0;
            loop {
                let d = util::rand_range(gm, 3, 0) as u16;
                nx = (gm.gi16(d.wrapping_mul(2).wrapping_add(DNG_DX)) as u16).wrapping_add(mx);
                if 7 < nx as i16 { nx = 0; }
                if (nx as i16) < 0 { nx = 7; }
                ny = (gm.gi16(d.wrapping_mul(2).wrapping_add(DNG_DY)) as u16).wrapping_add(my);
                if 7 < ny as i16 { ny = 0; }
                if (ny as i16) < 0 { ny = 7; }
                let t = gm.g8(cell_off(gm.g8(g::PARTY_Z) as u16, ny, nx)) & 0xf0;
                if t != 0x60 && t != 0x80 && t < 0xa0
                    && ((nx != px || ny != py) || util::rand_range(gm, 7, 0) == 1)
                {
                    state = 900;
                    break;
                }
                tries += 1;
                if !(tries < 8) { break; }
            }
            if state == 900 {
                let x = gm.g8(DNG_MONSTER_X);
                gm.set_g8(DNG_MONSTER_PREV_X, x);
                let y = gm.g8(DNG_MONSTER_Y);
                gm.set_g8(DNG_MONSTER_PREV_Y, y);
                gm.set_g8(DNG_MONSTER_X, nx as u8);
                gm.set_g8(DNG_MONSTER_Y, ny as u8);
            }
        }
        if gm.g8(DNG_MONSTER_X) == gm.g8(g::PARTY_X) && gm.g8(DNG_MONSTER_Y) == gm.g8(g::PARTY_Y) {
            result = 1;
            let x = gm.g8(DNG_MONSTER_PREV_X);
            gm.set_g8(DNG_MONSTER_X, x);
            let y = gm.g8(DNG_MONSTER_PREV_Y);
            gm.set_g8(DNG_MONSTER_Y, y);
        }
    }
    result
}

/// orig 1000:8b18 (157 bytes). Sleep energy field (cell 0x80/0x88): prints "Sleep spell!";
/// every party member whose dexterity <= rand(1..30) and who is not dead ('D') falls asleep
/// ('S'), its roster line is highlighted, random tones play and the status window is marked
/// dirty. Finally the party's map cell is ANDed with 8 (the field vanishes; only the
/// ceiling-hole bit survives).
pub fn sleep_field_effect(gm: &mut Game) {
    text::print_string(gm, 0x2d53); // "Sleep spell!\n"
    if gm.g8(g::PARTY_COUNT) != 0 {
        let mut i: u16 = 0;
        loop {
            let r = util::rand_range(gm, 0x1e, 1) as u16;
            let dex = gm.g8(g::PARTY_DEX.wrapping_add(i.wrapping_mul(0x20))) as u16;
            let st = g::PARTY_STATUS.wrapping_add(i.wrapping_mul(0x20));
            if dex <= r && gm.g8(st) != b'D' {
                gm.set_g8(st, b'S');
                rest::highlight_party_line(gm, i as i16);
                util::play_random_tones(gm, 0xdac, 0x32, 1);
                gm.set_g8(g::STATUS_DIRTY, 1);
            }
            i += 1;
            if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
        }
    }
    let off = party_cell_off(gm);
    let c = gm.g8(off) & 8;
    gm.set_g8(off, c);
}

/// orig 1000:8bb6 (102 bytes). Poison energy field (cell 0x81/0x89): prints "Poison!"; every
/// living member whose dexterity <= rand(1..30) becomes poisoned ('P') with random tones. The
/// field is not removed and the status window is not marked dirty (caller redraws it).
pub fn poison_field_effect(gm: &mut Game) {
    text::print_string(gm, 0x2d61); // "Poison!\n"
    let mut i: u16 = 0;
    if gm.g8(g::PARTY_COUNT) != 0 {
        loop {
            let r = util::rand_range(gm, 0x1e, 1) as u16;
            let dex = gm.g8(g::PARTY_DEX.wrapping_add(i.wrapping_mul(0x20))) as u16;
            let st = g::PARTY_STATUS.wrapping_add(i.wrapping_mul(0x20));
            if dex <= r && gm.g8(st) != b'D' {
                gm.set_g8(st, b'P');
                util::play_random_tones(gm, 0xdac, 0x32, 1);
            }
            i += 1;
            if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
        }
    }
}

/// orig 1000:8c1c (306 bytes). Pit trap: while the cell is 0x61/0x69 (invisible floor hole,
/// with/without ceiling hole) and the level is < 8: prints "Pit Trap!" / "Falling...", clears
/// the low 3 bits of the current cell, moves the party one level down, marks the landing cell
/// with bit 8 (visible ceiling hole) if it is not a wall/door/room (< 0x90), redraws, prints
/// "...splat!" and damages the party; repeats if the landing cell is another hidden pit.
/// Falling past level 7 (z == 8) sets g_location = 0 (the party drops into the Underworld).
/// Otherwise, landing in a room cell (0xa0/0xf0) runs DNGLOOK b3c0, `enter_dungeon_room` and
/// (if still in the dungeon) DNGLOOK b32e(1); then a new monster is spawned on the new level.
/// g_dungeon_dir_copy is set to 5 (entered from above).
pub fn fall_through_pit(gm: &mut Game, param_1: u8) {
    let mut cell = param_1;
    gm.set_g8(g::DUNGEON_DIR_COPY, 5);
    while (cell == 0x61 || cell == 0x69) && gm.g8(g::PARTY_Z) < 8 {
        text::print_string(gm, 0x2d6a); // "Pit Trap!\n"
        text::print_string(gm, 0x2d75); // "Falling...\n"
        let x = gm.g8(g::PARTY_X) as u16;
        let y = gm.g8(g::PARTY_Y) as u16;
        let off = cell_off(gm.g8(g::PARTY_Z) as u16, y, x);
        let c = gm.g8(off) & 0xf8;
        gm.set_g8(off, c);
        let z2 = gm.g8(g::PARTY_Z).wrapping_add(1);
        gm.set_g8(g::PARTY_Z, z2);
        cell = gm.g8(cell_off(z2 as u16, y, x));
        if cell < 0x90 {
            cell |= 8;
            let off2 = cell_off(z2 as u16, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16);
            gm.set_g8(off2, cell);
        }
        redraw_dungeon_view(gm);
        text::print_string(gm, 0x2d81); // "      ...splat!\n"
        rest::damage_whole_party(gm);
    }
    if gm.g8(g::PARTY_Z) == 8 {
        gm.set_g8(g::LOCATION, 0);
    }
    if gm.g8(g::LOCATION) != 0 {
        let t = gm.g8(party_cell_off(gm)) & 0xf0;
        if t == 0xa0 || t == 0xf0 {
            ovl::dnglook::unload_dungeon_images(gm);
            let c = gm.g8(party_cell_off(gm));
            enter_dungeon_room(gm, c);
            if gm.g8(g::LOCATION) == 0 {
                return;
            }
            ovl::dnglook::load_dungeon_images(gm, 1);
        }
        spawn_dungeon_monster(gm, 1);
    }
}

/// orig 1000:8d4e (241 bytes). The wandering monster reached the party: prints "Attacked",
/// derives the attack direction from the monster's previous cell (prev x-1 == party x -> 1 E,
/// prev x+1 == x -> 3 W, prev y-1 == y -> 2 S, else 0 N), and if it differs from the facing
/// prints " from the north/east/south/west" and turns the party to face it. Prints "!", redraws,
/// flushes keys, waits 2 ticks, DNGLOOK b3c0, g_combat_exit = 0, g_combat_flags = 2, DNGLOOK afce,
/// run_encounter(0, monster type, 2), DNGLOOK b26a, DNGLOOK b32e(1), spawns a new monster and
/// redraws if still in the dungeon.
pub fn dungeon_monster_attacks(gm: &mut Game) {
    text::print_string(gm, 0x2d92); // "Attacked"
    let px = gm.g8(DNG_MONSTER_PREV_X) as u16;
    let py = gm.g8(DNG_MONSTER_PREV_Y) as u16;
    let x = gm.g8(g::PARTY_X) as u16;
    let y = gm.g8(g::PARTY_Y) as u16;
    if (px.wrapping_sub(1) & 7) == x {
        gm.set_g8(g::DUNGEON_DIR_COPY, 1);
    } else if (px.wrapping_add(1) & 7) == x {
        gm.set_g8(g::DUNGEON_DIR_COPY, 3);
    } else if (py.wrapping_sub(1) & 7) == y {
        gm.set_g8(g::DUNGEON_DIR_COPY, 2);
    } else {
        gm.set_g8(g::DUNGEON_DIR_COPY, 0);
    }
    let d = gm.g8(g::DUNGEON_DIR_COPY);
    if d != gm.g8(g::DUNGEON_FACING) {
        text::print_string(gm, 0x2d9b); // " from the "
        if d == 0 {
            text::print_string(gm, 0x2da6); // "north"
        } else if d == 1 {
            text::print_string(gm, 0x2dac); // "east"
        } else if d == 2 {
            text::print_string(gm, 0x2db1); // "south"
        } else if d == 3 {
            text::print_string(gm, 0x2db7); // "west"
        }
        gm.set_g8(g::DUNGEON_FACING, d);
    }
    text::print_string(gm, 0x2dbc); // "!\n"
    redraw_dungeon_view(gm);
    input::flush_keyboard(gm);
    util::wait_ticks(gm, 2);
    ovl::dnglook::unload_dungeon_images(gm);
    gm.set_g8(g::COMBAT_EXIT, 0);
    gm.set_g8(g::COMBAT_FLAGS, 2);
    ovl::dnglook::build_dungeon_combat_map(gm);
    let ty = gm.g8(DNG_MONSTER_TYPE) as i16;
    rest::run_encounter(gm, 0, ty, 2);
    ovl::dnglook::apply_combat_exit_to_dungeon(gm);
    ovl::dnglook::load_dungeon_images(gm, 1);
    spawn_dungeon_monster(gm, 1);
    if gm.g8(g::LOCATION) != 0 {
        redraw_dungeon_view(gm);
    }
}

/// orig 1000:8e46 (434 bytes). End-of-turn processing (true order: monsters_move, cell).
/// Sleeping members wake with probability 4/64 each (status 'S' -> 'G'; status window redrawn
/// if any woke). If `monsters_move`, the wandering monster moves and, when it reaches the party,
/// `dungeon_monster_attacks` runs and the cell is re-read. Then by cell: room/cleared room
/// (0xf0/0xa0) -> DNGLOOK b3c0, repeatedly `enter_dungeon_room` while the party keeps standing on
/// a room cell with a conscious member and g_location != 0, then DNGLOOK b32e(1) and a redraw if
/// still in a dungeon (location > 0x20), skipping the status redraw; 0x80/0x88 sleep field;
/// 0x81/0x89 poison field; 0x82/0x8a "Fire!!" damage; 0x61/0x69 pit trap; 0x62/0x6a "Bomb
/// Trap!" "KABOOM!!" (cell &= 8) + damage. Other cells: nothing. Then draw_status_window and
/// ROOT end_of_turn_party_update.
pub fn dungeon_turn_events(gm: &mut Game, param_1: i16, param_2: u8) {
    let mut cell = param_2;
    let mut attacked: i16 = 0;
    let mut woke: i8 = 0;
    if gm.g8(g::PARTY_COUNT) != 0 {
        let mut i: u16 = 0;
        loop {
            let st = g::PARTY_STATUS.wrapping_add(i.wrapping_mul(0x20));
            if gm.g8(st) == b'S' && util::rand_range(gm, 0x3f, 0) < 4 {
                gm.set_g8(st, b'G');
                woke = woke.wrapping_add(1);
            }
            i += 1;
            if !(i < gm.g8(g::PARTY_COUNT) as u16) { break; }
        }
    }
    if woke != 0 {
        rest::draw_status_window(gm);
    }
    if param_1 != 0 {
        attacked = move_dungeon_monster(gm) as i16;
    }
    if attacked != 0 {
        dungeon_monster_attacks(gm);
        cell = gm.g8(party_cell_off(gm));
    }
    let mut t = cell & 0xf0;
    if t == 0xf0 || t == 0xa0 {
        ovl::dnglook::unload_dungeon_images(gm);
        loop {
            if !(t == 0xf0 || t == 0xa0) { break; }
            if (rest::find_first_conscious_member(gm) as i16) < 0 { break; }
            enter_dungeon_room(gm, cell);
            if gm.g8(g::LOCATION) == 0 { break; }
            cell = gm.g8(party_cell_off(gm));
            t = cell & 0xf0;
        }
        ovl::dnglook::load_dungeon_images(gm, 1);
        if 0x20 < gm.g8(g::LOCATION) {
            redraw_dungeon_view(gm);
        }
        rest::end_of_turn_party_update(gm);
        return;
    }
    if cell == 0x80 {
        sleep_field_effect(gm);
    } else if cell < 0x81 {
        if cell == 0x61 {
            fall_through_pit(gm, cell);
        } else if cell == 0x62 || cell == 0x6a {
            text::print_string(gm, 0x2dc7); // "Bomb Trap!\n"
            text::print_string(gm, 0x2dd3); // "KABOOM!!\n"
            let off = party_cell_off(gm);
            let c = gm.g8(off) & 8;
            gm.set_g8(off, c);
            rest::damage_whole_party(gm);
        } else if cell == 0x69 {
            fall_through_pit(gm, cell);
        }
    } else if cell == 0x81 {
        poison_field_effect(gm);
    } else if cell == 0x82 || cell == 0x8a {
        text::print_string(gm, 0x2dbf); // "Fire!!\n"
        rest::damage_whole_party(gm);
    } else if cell == 0x88 {
        sleep_field_effect(gm);
    } else if cell == 0x89 {
        poison_field_effect(gm);
    }
    rest::draw_status_window(gm);
    rest::end_of_turn_party_update(gm);
}

/// orig 1000:8ffe (495 bytes). The dungeon game loop (vector 1778:0296; ROOT main calls it with
/// `arrived` = 1 when the party just entered from the surface/Underworld). Sets g_dungeon_flags
/// = 2, DNGLOOK abca (loads DNGn.16 / ITEMS.16), draws the frame and labels, chooses the tile set
/// by dungeon: locations 0x21/0x24/0x25 -> wall 0x4f floor 0x45 style 3, 0x26/0x27 -> wall 0x4d
/// floor 5 style 2, others style 1. If the party starts on a room cell the room is entered
/// first. Then (if still in a dungeon) DNGLOOK b32e(arrived), spawn monster, redraw. Each turn:
/// find_first_conscious_member (-1 = all dead -> leave the loop; 1 = all asleep -> "Zzzzzz..."
/// and loop again); with status icon 'T' time does not pass and monsters do not move, with 'Q'
/// time passes every other turn; read a command, dispatch it, and if time passes run
/// `dungeon_turn_events`. Leaving (g_location < 0x21 or no conscious member): bit 4 of
/// g_dungeon_flags is set if bit 2 is clear, else cleared; DNGLOOK b3c0; and if everyone is dead
/// BLCKTHRN resurrection_scene.
pub fn dungeon_main(gm: &mut Game, param_1: u16) {
    let mut running: i16 = 1;
    let mut first: i16 = 0;
    let mut time_passes: u16 = 0;
    let mut monsters_move: u16 = 0;
    gm.set_g8(g::DUNGEON_FLAGS, 2);
    ovl::dnglook::apply_cleared_rooms_to_map(gm);
    let mut t = gm.g8(party_cell_off(gm)) & 0xf0;
    draw_dungeon_frame(gm);
    print_level_and_facing(gm);
    let d = gm.gi8(g::LOCATION) as i16 - 0x20;
    if d == 1 || d == 4 || d == 5 {
        gm.set_g8(g::DUNGEON_WALL_TILE, 0x4f);
        gm.set_g8(g::DUNGEON_FLOOR_TILE, 0x45);
        gm.set_g8(g::DUNGEON_STYLE, 3);
    } else {
        gm.set_g8(g::DUNGEON_WALL_TILE, 0x4d);
        gm.set_g8(g::DUNGEON_FLOOR_TILE, 5);
        if d == 6 || d == 7 {
            gm.set_g8(g::DUNGEON_STYLE, 2);
        } else {
            gm.set_g8(g::DUNGEON_STYLE, 1);
        }
    }
    if t == 0xa0 || t == 0xf0 {
        ovl::dnglook::unload_dungeon_images(gm);
        let c = gm.g8(party_cell_off(gm));
        enter_dungeon_room(gm, c);
    }
    if 0x20 < gm.g8(g::LOCATION) {
        ovl::dnglook::load_dungeon_images(gm, param_1 as i16);
        spawn_dungeon_monster(gm, param_1 as i16);
        redraw_dungeon_view(gm);
    }
    loop {
        loop {
            if running == 0 || gm.g8(g::LOCATION) < 0x21 {
                if (gm.g8(g::DUNGEON_FLAGS) & 2) == 0 {
                    let f = gm.g8(g::DUNGEON_FLAGS) | 4;
                    gm.set_g8(g::DUNGEON_FLAGS, f);
                } else {
                    let f = gm.g8(g::DUNGEON_FLAGS) & 0xfb;
                    gm.set_g8(g::DUNGEON_FLAGS, f);
                }
                ovl::dnglook::unload_dungeon_images(gm);
                if first < 0 {
                    ovl::blckthrn::resurrection_scene(gm);
                }
                return;
            }
            running = 1;
            first = rest::find_first_conscious_member(gm) as i16;
            if first == 1 {
                text::put_char(gm, 10);
                rest::draw_bar_left_cap(gm);
                text::print_string(gm, 0x2ddd); // "Zzzzzz...\n"
            }
            if -1 >= first {
                running = 0;
            }
            time_passes = (-1 < first) as u16;
            if first == 0 { break; }
        }
        let icon = gm.g8(g::STATUS_ICON);
        if icon == b'T' || icon == b'Q' {
            if icon == b'Q' {
                monsters_move ^= 1;
                if monsters_move != 0 {
                    rest::pass_time(gm, 1);
                }
            } else {
                monsters_move = 0;
            }
        } else {
            monsters_move = 1;
            rest::pass_time(gm, 1);
        }
        let key = read_dungeon_command(gm);
        if -1 < key {
            time_passes = dungeon_command(gm, key as u16);
        }
        if gm.g8(g::LOCATION) < 0x21 {
            time_passes = 0;
            running = 0;
        }
        let cell = gm.g8(party_cell_off(gm));
        t = cell & 0xf0;
        if time_passes != 0 {
            dungeon_turn_events(gm, monsters_move as i16, cell);
        }
        if gm.g8(g::LOCATION) < 0x21 {
            time_passes = 0;
            running = 0;
        }
    }
}

/// orig 1000:91f0 (43 bytes). Footstep sound: toggles 0x24e7 and on every "on" tick with a
/// non-zero countdown 0xa9fb plays random tones (total = countdown) and lowers the countdown
/// by 4 (floored at 0). `move_in_dungeon` primes the countdown with 0xf.
pub fn step_sound_tick(gm: &mut Game) {
    let t = gm.g8(DNG_STEP_TOGGLE) ^ 1;
    gm.set_g8(DNG_STEP_TOGGLE, t);
    if t != 0 && gm.g8(DNG_STEP_SOUND) != 0 {
        let n = gm.g8(DNG_STEP_SOUND) as i16;
        util::play_random_tones(gm, 0x4e20, n, 1);
        rest::sub_u8_floor(gm, 4, DNG_STEP_SOUND);
    }
}

/// orig 1000:921c (144 bytes). Draws the text of wall sign `n` (low nibble of a 0xb0 cell,
/// 1-based) of the current dungeon onto the view: tables indexed by g_location (0x21..0x28) at
/// 0x2de8 (first sign index per dungeon: 0 1 2 2 3 7 10 11) and 0x2df0 (sign count: 1 1 0 1 4
/// 3 1 0); if n <= count, i = base + n selects the text-cell position (x table 0x2df7, y table
/// 0x2e03) and the string pointer g_dungeon_sign_text_ptrs[i-1] (0x2e10). A temporary window
/// 0 is defined there (define_text_window(0x18, 0x27, y, x, 0)), font 1 (runes) selected and
/// the text printed between two 0xfd control codes, then font 0 and window 2 are restored and
/// window 0 reset to (0,0).
pub fn draw_dungeon_sign(gm: &mut Game, param_1: u16) {
    let loc = gm.g8(g::LOCATION) as u16;
    if param_1 <= gm.g8(0x2dcfu16.wrapping_add(loc)) as u16 {
        let i = (gm.g8(0x2dc7u16.wrapping_add(loc)) as u16).wrapping_add(param_1);
        let y = gm.g8(i.wrapping_add(0x2e03)) as i16;
        let x = gm.g8(i.wrapping_add(0x2df7)) as i16;
        text::define_text_window(gm, 0x18, 0x27, y, x, 0);
        text::select_text_window(gm, 0);
        text::select_font(gm, 1);
        text::set_cursor_pos(gm, 0, 0);
        text::put_char(gm, 0xfd);
        let sp = gm.g16(i.wrapping_sub(1).wrapping_mul(2).wrapping_add(0x2e10));
        text::print_string(gm, sp);
        text::put_char(gm, 0xfd);
        text::select_font(gm, 0);
        text::select_text_window(gm, 2);
        text::define_text_window(gm, 0x18, 0x27, 0, 0, 0);
    }
}

/// orig 1000:92ac (65 bytes). Returns the map cell at (y & 7, x & 7) on the party's level
/// (true order: y, x); for non-wall cells (< 0x90) the ceiling-hole bit 8 is masked off.
pub fn dungeon_cell_at(gm: &mut Game, param_1: u16, param_2: u16) -> u8 {
    let mut c = gm.g8(cell_off(gm.g8(g::PARTY_Z) as u16, param_1 & 7, param_2 & 7));
    if c < 0x90 {
        c &= 0xf7;
    }
    c
}

/// orig 1000:92ee (348 bytes). Draws the wandering monster at view depth `depth` (1..3) from
/// its MONn.16 file and advances its animation state 0x5c68 (bits 0x60 = animation kind, 0x90 =
/// facing bits, 0xf = frame). Under time stop ('T') the state is reset from table 0x1744 and
/// both halves use frame set 1. Otherwise each half's frame set (0/1) is random (50%); kind 0x20
/// alternates both halves with the frame bit, kind 0x60 cycles frame 3..0 through table 0x2e26
/// (01 03 02 03: bit 1 -> left half set, bit 0 -> right half set); when facing bits are set the
/// right half = (both facing bits set) XOR left set. The sprite is two images: the left half
/// drawn at x = 0x2e2a[depth] (0x28/0x48/0x50/0x58), y = 0x2e32[alt][depth], with the vertical
/// flip flag = alt (0x5c69 == 0xff), index = set*3 + depth-1; the right half is the mirrored
/// image at x 0x60. Prints "Hey!! What's going on here???" if no sprite file is loaded.
pub fn draw_dungeon_monster(gm: &mut Game, param_1: i16) {
    let depth = param_1;
    if gm.g8(g::STATUS_ICON) == b'T' {
        let a = gm.g8((gm.g8(DNG_MONSTER_KIND) as u16).wrapping_add(0x1744));
        gm.set_g8(DNG_MONSTER_ANIM, a);
    }
    let alt: u16 = (gm.gi8(DNG_MONSTER_ALT) == -1) as u16;
    let a = gm.g8(DNG_MONSTER_ANIM) as u16;
    let facing_bits: u8 = (a as u8) & 0x90;
    let kind: u16 = a & 0x60;
    let frame_bits: u8 = (a as u8) & 0xf;
    let mut frame: u16 = a & 0xf;
    let mut set_l: u16;
    let mut set_r: u16;
    if gm.g8(g::STATUS_ICON) == b'T' {
        set_r = 1;
        set_l = 1;
    } else {
        let r = util::rand_range(gm, 100, 0);
        set_l = (r < 0x32) as u16;
        let r = util::rand_range(gm, 100, 0);
        set_r = (r < 0x32) as u16;
        if kind == 0x20 {
            set_r = (frame_bits == 0) as u16;
            frame = set_r;
            set_l = set_r;
        } else if kind != 0x40 && kind == 0x60 {
            frame = frame.wrapping_sub(1);
            if (frame as i16) < 0 {
                frame = 3;
            }
            set_l = (gm.g8(frame.wrapping_add(0x2e26)) >> 1) as u16;
            set_r = (gm.g8(frame.wrapping_add(0x2e26)) & 1) as u16;
        }
    }
    if facing_bits != 0 {
        set_r = (((a & 0x90) == 0x90) as u16) ^ set_l;
    }
    let seg = gm.g16(DNG_MONSTER_IMG_SEG);
    if seg == 0 {
        text::print_string(gm, 0x6c4e); // "Hey!! What's going on here???"
    } else {
        let y = gm.g16(alt.wrapping_mul(8).wrapping_add((depth as u16).wrapping_mul(2)).wrapping_add(0x2e32)) as i16;
        let x = gm.g16((depth as u16).wrapping_mul(2).wrapping_add(0x2e2a)) as i16;
        gfx::draw_image(gm, alt as i16, y, x, (set_l as i16).wrapping_mul(3).wrapping_add(depth).wrapping_sub(1), seg);
        gfx::draw_image_alt(gm, alt as i16, y, 0x60, (set_r as i16).wrapping_mul(3).wrapping_add(depth).wrapping_sub(1), seg);
        let v = (kind as u8).wrapping_add((a & 0x90) as u8).wrapping_add(frame as u8);
        gm.set_g8(DNG_MONSTER_ANIM, v);
    }
}

/// orig 1000:944e (199 bytes). Draws an energy field (cell 0x8n) seen at view depth `depth`
/// (0..3) as a shower of random short horizontal lines (true order: field_type, depth). Colour =
/// bright (+8) g_ui_color4 / 3 / 0 / 2 for sleep / poison / fire / electric. Per depth the
/// tables give line count g_field_lines (0x2e52: 300 100 50 15), width-1 g_field_line_width
/// (0x2e5a: 7 7 5 2), top g_field_top (0x2e42: 0x10 0x38 0x50 0x5c) and bottom g_field_bottom
/// (0x2e4a: 0xa7 0x87 0x6f 0x63); each line starts at x = rand(top..bottom-width), y =
/// rand(top..bottom). Software clipping is disabled for the duration.
pub fn draw_energy_field(gm: &mut Game, param_1: i16, param_2: i16) {
    let saved_clip = gm.g16(g::CLIP_ENABLED);
    gm.set_g16(g::CLIP_ENABLED, 0);
    let col: Option<i16> = if param_1 == 0 {
        Some(gm.gi16(g::UI_COLOR4))
    } else if param_1 == 1 {
        Some(gm.gi16(g::UI_COLOR3))
    } else if param_1 == 2 {
        Some(gm.gi16(g::UI_COLOR0))
    } else if param_1 == 3 {
        Some(gm.gi16(g::UI_COLOR2))
    } else {
        None
    };
    if let Some(c) = col {
        gfx::set_color(gm, c.wrapping_add(8));
    }
    let d2 = (param_2 as u16).wrapping_mul(2);
    if 0 < gm.gi16(d2.wrapping_add(0x2e52)) {
        let mut n = gm.gi16(d2.wrapping_add(0x2e52));
        let w = gm.gi16(d2.wrapping_add(0x2e5a));
        let bottom = gm.gi16(d2.wrapping_add(0x2e4a));
        let top = gm.gi16(d2.wrapping_add(0x2e42));
        loop {
            let x = util::rand_range(gm, bottom.wrapping_sub(w), top);
            let y = util::rand_range(gm, bottom, top);
            gfx::draw_hline(gm, w.wrapping_add(x), y, x);
            n = n.wrapping_sub(1);
            if n == 0 { break; }
        }
    }
    gm.set_g16(g::CLIP_ENABLED, saved_clip);
}

/// orig 1000:951a (269 bytes). Blits one view image onto page 1 (true order: side, flip,
/// index). `side` 0 = left half drawn normally, 1 = right half drawn mirrored (entry 33).
/// Index < 0x1f: a wall slice from DNGn.16 at y 0x0e, x from table 0x2e62[side][index & 3]
/// (left 0x10/0x28/0x48/0x58, right 0x98/0x78/0x68/0x60; right-side indices 8..15 and
/// 0x18..0x1b use x 0x60), no flags. Index >= 0x1f: an ITEMS.16 object: i = index+1, image =
/// (i>>1)-0x10, depth k = (i&6)>>1; x = 0x2e72[k] (0x38/0x48/0x50/0x58) for the left half, 0x60
/// for the mirrored right half; y = 0x60 (floor objects, images < 8) or, when `flip` (vertical
/// flip = ceiling objects such as ladders up) 0x2e82[k] (0x0f/0x27/0x47/0x57); images >= 8 use
/// y = 0x2e7a[flip][k] (0x98/0x78/0x68/0x60 or the same 0x0f.. row). `flip` is passed as the
/// driver mode flag (bit 0 = vertical flip).
pub fn draw_wall_image(gm: &mut Game, param_1: i16, param_2: i16, param_3: u16) {
    let side = param_1;
    let mut flip = param_2;
    let mut index = param_3;
    if (index as i16) < 0x1f {
        let mut x = gm.g16((index & 3).wrapping_mul(2).wrapping_add((side as u16).wrapping_mul(8)).wrapping_add(0x2e62)) as i16;
        if side != 0 && (((index as u8) & 0xf8) == 8 || ((index as u8) & 0xfc) == 0x18) {
            x = 0x60;
        }
        let seg = gm.g16(DNG_WALL_IMG_SEG);
        flip = 0;
        if side == 0 {
            gfx::draw_image(gm, flip, 0xe, x, index as i16, seg);
        } else {
            gfx::draw_image_alt(gm, flip, 0xe, x, index as i16, seg);
        }
    } else {
        let i = index.wrapping_add(1);
        index = (((i as i16) >> 1) - 0x10) as u16;
        let k = (i & 6) >> 1;
        let x: i16 = if side == 0 { gm.g16(k.wrapping_mul(2).wrapping_add(0x2e72)) as i16 } else { 0x60 };
        let y: i16 = if (index as i16) < 8 {
            if flip == 0 { 0x60 } else { gm.g16(k.wrapping_mul(2).wrapping_add(0x2e82)) as i16 }
        } else {
            gm.g16(k.wrapping_mul(2).wrapping_add((flip as u16).wrapping_mul(8)).wrapping_add(0x2e7a)) as i16
        };
        let seg = gm.g16(DNG_ITEMS_IMG_SEG);
        if side == 0 {
            gfx::draw_image(gm, flip, y, x, index as i16, seg);
        } else {
            gfx::draw_image_alt(gm, flip, y, x, index as i16, seg);
        }
    }
}

/// orig 1000:962c (170 bytes). Cave "alternate wall" (0xc0) sparkle at (y, x) (true order:
/// depth, state, y, x). State 0..3: draws a small cross (hline x-1..x+1, vline y-1..y+1) in
/// g_ui_color2 with a bright centre pixel; state 4: the cross in colour 3 (CGA/Hercules) or 0xb;
/// state 5: resets to 0 and plays a tone sweep (total 20-8*depth ticks, 3500 -> 3200 Hz). The
/// state advances every frame once started, and starts with probability 4/64. Returns the new
/// state (the caller keeps it in the cell's low 3 bits).
pub fn animate_wall_sparkle(gm: &mut Game, param_1: i16, param_2: i16, param_3: i16, param_4: i16) -> i16 {
    let depth = param_1;
    let mut state = param_2;
    let y = param_3;
    let x = param_4;
    if state == 5 {
        state = 0;
        rest::play_tone_sweep(gm, (depth.wrapping_mul(8).wrapping_sub(0x14)).wrapping_neg(), 1, 0xdac, 0xc80);
    } else {
        let mut col = gm.gi16(g::UI_COLOR2);
        if state == 4 {
            let a = gm.gi16(g::ADAPTER_TYPE);
            col = if a == 0 || a == 3 { 3 } else { 0xb };
        }
        gfx::set_color(gm, col);
        gfx::draw_hline(gm, x.wrapping_add(1), y, x.wrapping_sub(1));
        gfx::draw_vline(gm, y.wrapping_add(1), y.wrapping_sub(1), x);
        if state < 4 {
            let c = gm.gi16(g::UI_COLOR2).wrapping_add(8);
            gfx::set_color(gm, c);
            gfx::plot_pixel(gm, y, x);
        }
        if state != 0 || util::rand_range(gm, 0x40, 0) < 4 {
            state = state.wrapping_add(1);
        }
    }
    state
}

/// orig 1000:96da (374 bytes). Draws the cell straight ahead at view `depth` (0..3) if it is
/// solid (type >= 0xa0) (true order: depth, y, x). Both halves of wall image
/// 0x2e8a[type-0xa] + depth are drawn (type a/e/f -> 0x0c.., b/d -> 0x08.., c -> 0x18..; at
/// depth 0 always image 0x0c = door frame around the party). Extras: in caves (style 1) a 0xc0
/// wall at depth 1-2 runs `animate_wall_sparkle` at y 0x2e8b[depth*5 + state] + 0xe, x 0x5f and
/// stores the state in the cell's low 3 bits; at depth 1 a 0xb0 wall with a non-zero low nibble
/// shows its sign (`draw_dungeon_sign`), and in style 3 a 0xc0 wall (skeleton) blinks its eyes
/// (4 short lines in bright g_ui_color0) with probability 4/64. Returns 0 (stop drawing deeper)
/// for any solid cell except a door (0xe0) at depth 0, where software clipping is enabled and
/// 1 is returned; open cells return 1.
pub fn draw_front_wall(gm: &mut Game, param_1: i16, param_2: u16, param_3: u16) -> u16 {
    let depth = param_1;
    let c = dungeon_cell_at(gm, param_2, param_3) as u16;
    if 0x9f < c as i16 {
        let mut idx: i16 = (gm.g8((((c as i16) >> 4) as u16).wrapping_add(0x2e80)) as i16).wrapping_add(depth);
        if depth == 0 {
            idx = 0xc;
        }
        draw_wall_image(gm, 0, 0, idx as u16);
        draw_wall_image(gm, 1, 0, idx as u16);
        let lo = c as u8;
        if (depth == 1 || depth == 2) && gm.g8(g::DUNGEON_STYLE) == 1 && (lo & 0xf0) == 0xc0 {
            let y = (gm.g8((c & 7).wrapping_add((depth as u16).wrapping_mul(5)).wrapping_add(0x2e8b)) as i16).wrapping_add(0xe);
            let st = animate_wall_sparkle(gm, depth, (c & 7) as i16, y, 0x5f) as i8;
            let off = cell_off(gm.g8(g::PARTY_Z) as u16, param_2 & 7, param_3 & 7);
            let v = gm.g8(off) & 0xf8;
            gm.set_g8(off, v);
            let v = gm.g8(off).wrapping_add(st as u8);
            gm.set_g8(off, v);
        }
        if depth == 1 {
            if (lo & 0xf0) == 0xb0 && (c & 0xf) != 0 {
                draw_dungeon_sign(gm, c & 0xf);
            } else if gm.g8(g::DUNGEON_STYLE) == 3 && (lo & 0xf0) == 0xc0 && util::rand_range(gm, 0x40, 0) < 4 {
                let col = gm.gi16(g::UI_COLOR0).wrapping_add(8);
                gfx::set_color(gm, col);
                gfx::draw_hline(gm, 0x5d, 0x57, 0x5c);
                gfx::draw_hline(gm, 0x5d, 0x58, 0x5b);
                gfx::draw_hline(gm, 0x62, 0x57, 0x61);
                gfx::draw_hline(gm, 99, 0x58, 0x61);
            }
        }
        if (lo & 0xf0) != 0xe0 || depth != 0 {
            return 0;
        }
        gm.set_g16(g::CLIP_ENABLED, 1);
    }
    1
}

/// orig 1000:9852 (260 bytes). Draws the left (`side` 0) or right (`side` 1, mirrored) wall
/// slice at view `depth` 0..3 for the cell at (y, x) beside the corridor (true order: depth,
/// side, y, x). Open cells (< 0xa0) use the side-opening images 0x10+depth; rooms, doors and
/// cleared rooms (0xa0/0xe0/0xf0) the door-side images 4+depth; normal/secret walls (0xb0/0xd0)
/// the plain images 0+depth; the alternate wall 0xc0 uses images 0x14+depth and, in caves
/// (style 1) at depth 0-1, runs `animate_wall_sparkle` at y 0x2e9a[depth*5 + state] + 0xe and
/// x 0x21 (depth 0) / 0x43 (depth 1), mirrored as 0xbe - x on the right, storing the state in
/// the cell's low 3 bits.
pub fn draw_side_wall(gm: &mut Game, param_1: i16, param_2: i16, param_3: u16, param_4: u16) {
    let depth = param_1;
    let side = param_2;
    let c = dungeon_cell_at(gm, param_3, param_4) as u16;
    let idx: i16;
    if (c as i16) < 0xa0 {
        idx = depth.wrapping_add(0x10);
    } else {
        let t = c & 0xf0;
        if t != 0xa0 {
            if t == 0xc0 {
                draw_wall_image(gm, side, 0, depth.wrapping_add(0x14) as u16);
                if 1 < depth {
                    return;
                }
                if gm.g8(g::DUNGEON_STYLE) != 1 {
                    return;
                }
                let mut x: i16 = if depth == 0 { 0x21 } else { 0x43 };
                if side != 0 {
                    x = 0xbe - x;
                }
                let y = (gm.g8((c & 7).wrapping_add((depth as u16).wrapping_mul(5)).wrapping_add(0x2e9a)) as i16).wrapping_add(0xe);
                let st = animate_wall_sparkle(gm, depth, (c & 7) as i16, y, x) as i8;
                let off = cell_off(gm.g8(g::PARTY_Z) as u16, param_3 & 7, param_4 & 7);
                let v = gm.g8(off) & 0xf8;
                gm.set_g8(off, v);
                let v = gm.g8(off).wrapping_add(st as u8);
                gm.set_g8(off, v);
                return;
            }
            if t != 0xe0 && t != 0xf0 {
                draw_wall_image(gm, side, 0, depth as u16);
                return;
            }
        }
        idx = depth.wrapping_add(4);
    }
    draw_wall_image(gm, side, 0, idx as u16);
}

/// orig 1000:9956 (456 bytes). Animates the fountain (ITEMS image 5..8 by depth) by plotting
/// per-frame water pixels symmetrically on both halves (true order: frame 0..2, depth 0..3).
/// Colour: 0 on CGA/Hercules, else g_ui_color2 (bright +8 at depth 0). Depth 0: 8 screen (x,y)
/// pairs from g_fountain_cycle_px (0x2ea4 + frame*16) plus one from g_fountain_cycle_px2
/// (0x2f10 + frame*2), each also mirrored at x 0xbe-x. Depth 1: 5 pairs (0x2ed4 + frame*10)
/// at (x+0x48, y+0x60) and (0x76-x, y+0x60). Depth 2: 4 pairs (0x2ef2 + frame*8) at (x+0x50,
/// y+0x60) and (0x6e-x, ..). Depth 3: 1 pair (0x2f0a + frame*2) at (x+0x58, y+0x60) and
/// (0x66-x, ..). Pairs are stored x first, y second.
pub fn animate_fountain(gm: &mut Game, param_1: i16, param_2: i16) {
    let frame = param_1;
    let depth = param_2;
    let a = gm.gi16(g::ADAPTER_TYPE);
    let col: i16 = if a == 0 || a == 3 { 0 } else { gm.gi16(g::UI_COLOR2) };
    gfx::set_color(gm, col);
    if depth == 0 {
        if a != 0 && a != 3 {
            let c = gm.gi16(g::UI_COLOR2).wrapping_add(8);
            gfx::set_color(gm, c);
        }
        let base = (frame as u16).wrapping_mul(0x10);
        let mut p = base.wrapping_add(0x2ea4);
        loop {
            let y = gm.g8(p.wrapping_add(1)) as i16;
            let x = gm.g8(p) as i16;
            gfx::plot_pixel(gm, y, x);
            gfx::plot_pixel(gm, y, 0xbe - x);
            p = p.wrapping_add(2);
            if !(p.wrapping_add(1) < base.wrapping_add(0x2eb5)) { break; }
        }
        let y = gm.g8((frame as u16).wrapping_mul(2).wrapping_add(0x2f11)) as i16;
        let x = gm.g8((frame as u16).wrapping_mul(2).wrapping_add(0x2f10)) as i16;
        gfx::plot_pixel(gm, y, x);
        gfx::plot_pixel(gm, y, 0xbe - x);
    } else if depth == 1 {
        let base = (frame as u16).wrapping_mul(10);
        let mut p = base.wrapping_add(0x2ed4);
        loop {
            let y = gm.g8(p.wrapping_add(1)) as i16;
            let x = gm.g8(p) as i16;
            gfx::plot_pixel(gm, y + 0x60, x + 0x48);
            gfx::plot_pixel(gm, y + 0x60, 0x76 - x);
            p = p.wrapping_add(2);
            if !(p.wrapping_add(1) < base.wrapping_add(0x2edf)) { break; }
        }
    } else if depth == 2 {
        let base = (frame as u16).wrapping_mul(8);
        let mut p = base.wrapping_add(0x2ef2);
        loop {
            let y = gm.g8(p.wrapping_add(1)) as i16;
            let x = gm.g8(p) as i16;
            gfx::plot_pixel(gm, y + 0x60, x + 0x50);
            gfx::plot_pixel(gm, y + 0x60, 0x6e - x);
            p = p.wrapping_add(2);
            if !(p.wrapping_add(1) < base.wrapping_add(0x2efb)) { break; }
        }
    } else if depth == 3 {
        let y = (gm.g8((frame as u16).wrapping_mul(2).wrapping_add(0x2f0b)) as i16) + 0x60;
        let x = gm.g8((frame as u16).wrapping_mul(2).wrapping_add(0x2f0a)) as i16;
        gfx::plot_pixel(gm, y, x + 0x58);
        gfx::plot_pixel(gm, y, 0x66 - x);
    }
}

/// orig 1000:9b22 (315 bytes). Draws the objects inside the corridor cell at view `depth`
/// (true order: depth, y, x), nearest cells last. Types 1..7 (except 6 with a non-zero low
/// nibble = hidden traps): ceiling object from table 0x2f16[type] (ladder up / up+down -> ITEMS
/// index 0x1f + depth*2, vertically flipped) and floor object from 0x2f1e[type] (ladder down
/// 0x1f, chest 0x37, fountain 0x27, pit 0x2f, open chest 0x3f, + depth*2); a fountain also gets
/// its animated water (`animate_fountain` with frame 0x2f26). Type 8 draws an energy field of
/// kind (cell & 7). For any type < 9 with the raw cell's bit 8 set, the ceiling-hole image
/// (0x2f24 = 0x2f + depth*2, flipped) is drawn. At depth >= 1 the wandering monster is drawn
/// if it stands in this cell. At depth 0 the fountain frame counter cycles 0..2.
pub fn draw_cell_contents(gm: &mut Game, param_1: i16, param_2: u16, param_3: u16) {
    let depth = param_1;
    let c = dungeon_cell_at(gm, param_2, param_3) as u16;
    let t = (c as i16) >> 4;
    if t < 1 || 7 < t || (t == 6 && (c & 7) != 0) {
        if t == 8 {
            draw_energy_field(gm, (c & 7) as i16, depth);
        }
    } else {
        if gm.g8((t as u16).wrapping_add(0x2f16)) != 0 {
            let i = (gm.g8((t as u16).wrapping_add(0x2f16)) as u16).wrapping_add((depth as u16).wrapping_mul(2));
            draw_wall_image(gm, 0, 1, i);
            draw_wall_image(gm, 1, 1, i);
        }
        if gm.g8((t as u16).wrapping_add(0x2f1e)) != 0 {
            let i = (gm.g8((t as u16).wrapping_add(0x2f1e)) as u16).wrapping_add((depth as u16).wrapping_mul(2));
            draw_wall_image(gm, 0, 0, i);
            draw_wall_image(gm, 1, 0, i);
        }
        if t == 5 {
            let f = gm.g8(DNG_FOUNTAIN_FRAME) as i16;
            animate_fountain(gm, f, depth);
        }
    }
    if t < 9 && (gm.g8(cell_off(gm.g8(g::PARTY_Z) as u16, param_2 & 7, param_3 & 7)) & 8) != 0 {
        let i = (depth as u16).wrapping_mul(2).wrapping_add(gm.g8(0x2f24) as u16);
        draw_wall_image(gm, 0, 1, i);
        draw_wall_image(gm, 1, 1, i);
    }
    if depth != 0 && ((param_3 as u8) & 7) == gm.g8(DNG_MONSTER_X) && ((param_2 as u8) & 7) == gm.g8(DNG_MONSTER_Y) {
        draw_dungeon_monster(gm, depth);
    }
    if depth == 0 {
        let f = gm.g8(DNG_FOUNTAIN_FRAME).wrapping_add(1);
        gm.set_g8(DNG_FOUNTAIN_FRAME, f);
        if 2 < f {
            gm.set_g8(DNG_FOUNTAIN_FRAME, 0);
        }
    }
}

/// orig 1000:9c60 (333 bytes). Renders the first-person view onto page 1. Sets
/// g_disk_drive_set = 0xff, draw page 1, clears the right half (x 0x60..0xaf, rows 0xe..0xb2 -
/// mirrored blits OR into the destination) and sets the clip window to (0x28,0x2a)-(0x98,0xb7)
/// with clipping off. Without light (no light spell and no torch) the whole view (8..0xb7) is
/// black. Otherwise walks up to 4 cells ahead along the facing: `draw_front_wall` (stops at a
/// solid cell; a door at depth 0 enables clipping) then, except at depth 0 when standing in a
/// doorway or `full` == 0, the left/right side walls of the cells beside the corridor (offsets
/// from the byte tables 0x2f28/0x2f2c by facing); then walks back drawing `draw_cell_contents`
/// farthest first. Restores draw page 0 and clipping off. Vector target 1778:04ca is NOT this
/// (ENDGAME); SJOG calls it with 1 after spells.
pub fn render_dungeon_view(gm: &mut Game, param_1: i16) {
    gm.set_g8(g::DISK_DRIVE_SET, 0xff);
    gfx::set_draw_page(gm, 1);
    gfx::set_color(gm, 0);
    gfx::fill_rect(gm, 0xb2, 0xaf, 0xe, 0x60);
    gfx::set_clip_window(gm, 0xb7, 0x98, 0x2a, 0x28);
    gm.set_g16(g::CLIP_ENABLED, 0);
    if gm.g8(g::LIGHT_SPELL_TURNS) == 0 && gm.g8(g::TORCH_TURNS) == 0 {
        gfx::fill_rect(gm, 0xb7, 0xb7, 8, 8);
    } else {
        let mut x = gm.g8(g::PARTY_X) as u16;
        let mut y = gm.g8(g::PARTY_Y) as u16;
        let f = gm.g8(g::DUNGEON_FACING) as u16;
        let dx = gm.gi16(f.wrapping_mul(2).wrapping_add(DNG_DX)) as u16;
        let dy = gm.gi16(f.wrapping_mul(2).wrapping_add(DNG_DY)) as u16;
        let mut depth: i16 = 0;
        loop {
            if draw_front_wall(gm, depth, y, x) == 0 {
                break;
            }
            let in_door = {
                let py = gm.g8(g::PARTY_Y) as u16;
                let px = gm.g8(g::PARTY_X) as u16;
                dungeon_cell_at(gm, py, px) as i16 == 0xe0
            };
            if depth != 0 || (!in_door && param_1 != 0) {
                let sy = gm.g8(f.wrapping_add(0x2f2c)) as u16;
                let sx = gm.g8(f.wrapping_add(0x2f28)) as u16;
                draw_side_wall(gm, depth, 0, sy.wrapping_add(y), sx.wrapping_add(x));
                draw_side_wall(gm, depth, 1, y.wrapping_sub(sy), x.wrapping_sub(sx));
            }
            x = x.wrapping_add(dx);
            y = y.wrapping_add(dy);
            depth += 1;
            if !(depth < 4) {
                break;
            }
        }
        loop {
            depth -= 1;
            if depth < 0 {
                break;
            }
            x = x.wrapping_sub(dx);
            y = y.wrapping_sub(dy);
            draw_cell_contents(gm, depth, y, x);
        }
    }
    gfx::set_draw_page(gm, 0);
    gm.set_g16(g::CLIP_ENABLED, 0);
}

/// orig 1000:9db0 (43 bytes). Full redraw: status window, `render_dungeon_view(1)`, copy the
/// view (x 0x10..0xaf, rows 0xe..0xb2) from page 1 to page 0, footstep sound tick and the
/// level/facing labels. Vector 1778:0476 (DNGLOOK a938 uses it).
pub fn redraw_dungeon_view(gm: &mut Game) {
    rest::draw_status_window(gm);
    render_dungeon_view(gm, 1);
    gfx::copy_rect_between_pages(gm, 0xb2, 0xaf, 0xe, 0x10, 0, 1);
    step_sound_tick(gm);
    print_level_and_facing(gm);
}

/// orig 1000:9ddc (93 bytes). Returns 1 if the party may arrive on `level` at its current x/y
/// (true order: check, level): always when `check` == 0 (ordinary ladders), otherwise only if
/// the destination cell is an open hallway (type nibble 0; the extra wall/door comparisons in
/// the original are redundant). Used by the Klimb and the CAST (Uus/Des Por) paths.
pub fn can_use_ladder(gm: &mut Game, param_1: i16, param_2: i16) -> u16 {
    let mut ok: u16 = 1;
    let c = gm.g8(cell_off(param_2 as u16, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16));
    let t = c & 0xf0;
    if param_1 != 0 && ((c & 0xf0) != 0 || t == 0xe0 || t == 0xd0 || t == 0xc0 || t == 0xb0) {
        ok = 0;
    }
    ok
}

/// orig 1000:9e3a (156 bytes). Climbs one level (true order: check, dir; dir -1 = up, +1 =
/// down). Prints "Up!" / "Down!". Going up from level 0 or down from level 7 leaves the dungeon:
/// returns 1 and the caller runs `exit_dungeon`. Otherwise `can_use_ladder(check, z+dir)`
/// decides: failure prints "Failed!" with a falling tone sweep; success changes g_party_z,
/// sets g_dungeon_dir_copy 4 (came from below) / 5 (from above), spawns a new monster and
/// redraws. Returns 0 in both cases. Vector 1778:0512 (CAST cd3a uses it for Uus/Des Por).
pub fn dungeon_klimb_ladder(gm: &mut Game, param_1: u16, param_2: i16) -> u16 {
    let dir = param_2;
    let mut leave: u16 = 0;
    let s: u16 = if dir < 1 { 0x6c74 } else { 0x6c6c }; // "Up!\n" / "Down!\n"
    text::print_string(gm, s);
    if (dir < 1 || gm.g8(g::PARTY_Z) != 7) && (-1 < dir || gm.g8(g::PARTY_Z) != 0) {
        let lvl = dir.wrapping_add(gm.g8(g::PARTY_Z) as i16);
        if can_use_ladder(gm, param_1 as i16, lvl) == 0 {
            text::print_string(gm, 0x6c7a); // "Failed!\n"
            rest::play_tone_sweep(gm, 0x32, 1, 2000, 800);
        } else {
            let z = gm.g8(g::PARTY_Z).wrapping_add(dir as u8);
            gm.set_g8(g::PARTY_Z, z);
            if dir < 1 {
                gm.set_g8(g::DUNGEON_DIR_COPY, 4);
            } else {
                gm.set_g8(g::DUNGEON_DIR_COPY, 5);
            }
            spawn_dungeon_monster(gm, 1);
            redraw_dungeon_view(gm);
        }
    } else {
        leave = 1;
    }
    leave
}

/// orig 1000:9ed8 (65 bytes). Leaves the dungeon: the party x/y become the dungeon entrance's
/// overworld coordinates (byte tables 0x1e89 / 0x1eb1 indexed by g_location), prints "Exit to
/// Britannia!" (z stays 0) or "Exit to Underworld!" (z = 0xff) depending on whether the party
/// was on level 0, and sets g_location = 0 so ROOT main returns to the overworld. Vector
/// 1778:051e (CAST cd3a uses it too).
pub fn exit_dungeon(gm: &mut Game) {
    let loc = gm.g8(g::LOCATION) as u16;
    let x = gm.g8(loc.wrapping_add(0x1e89));
    gm.set_g8(g::PARTY_X, x);
    let y = gm.g8(loc.wrapping_add(0x1eb1));
    gm.set_g8(g::PARTY_Y, y);
    text::print_string(gm, 0x6c84); // "\nExit to "
    let s: u16;
    if gm.gi8(g::PARTY_Z) == 0 {
        gm.set_g8(g::PARTY_Z, 0);
        s = 0x6c9c; // "Britannia!\n\n"
    } else {
        gm.set_g8(g::PARTY_Z, 0xff);
        s = 0x6c8e; // "Underworld!\n\n"
    }
    text::print_string(gm, s);
    gm.set_g8(g::LOCATION, 0);
}

/// orig 1000:9f1a (194 bytes). The Attack command in a dungeon (ROOT dispatch_command 'A'
/// with location > 0x20; vector 1778:052a). Prints "Attack"; unless the wandering monster is in
/// the cell straight ahead prints "What?" and returns 0. Otherwise g_combat_flags = 2, DNGLOOK
/// b3c0, DNGLOOK aefc, DNGLOOK afce and run_encounter(0, monster type, 2). Combat exit 5 (up)
/// moves the party one level up, 6 (down) one level down; leaving level 0 upwards or level 7
/// downwards runs `exit_dungeon`. Then DNGLOOK b32e(1) and, if still in a dungeon, a monster
/// respawn and redraw. Always returns 0 (no time passes).
pub fn cmd_attack_dungeon(gm: &mut Game) -> u16 {
    text::print_string(gm, 0x6caa); // "Attack\n"
    let f = (gm.g8(g::DUNGEON_FACING) as u16).wrapping_mul(2);
    let ax = ((gm.gi16(f.wrapping_add(DNG_DX)) as u16).wrapping_add(gm.g8(g::PARTY_X) as u16)) & 7;
    let ay = ((gm.gi16(f.wrapping_add(DNG_DY)) as u16).wrapping_add(gm.g8(g::PARTY_Y) as u16)) & 7;
    if ax != gm.g8(DNG_MONSTER_X) as u16 || ay != gm.g8(DNG_MONSTER_Y) as u16 {
        text::print_string(gm, 0x6cb2); // "What?\n"
        return 0;
    }
    gm.set_g8(g::COMBAT_FLAGS, 2);
    ovl::dnglook::unload_dungeon_images(gm);
    ovl::dnglook::build_dungeon_combat_walls(gm);
    ovl::dnglook::build_dungeon_combat_map(gm);
    let ty = gm.g8(DNG_MONSTER_TYPE) as i16;
    rest::run_encounter(gm, 0, ty, 2);
    let mut leave = false;
    let e = gm.gi8(g::COMBAT_EXIT);
    if e == 5 {
        if gm.g8(g::PARTY_Z) != 0 {
            let z = gm.g8(g::PARTY_Z).wrapping_sub(1);
            gm.set_g8(g::PARTY_Z, z);
        } else {
            leave = true;
        }
    } else if e == 6 {
        if gm.g8(g::PARTY_Z) < 7 {
            let z = gm.g8(g::PARTY_Z).wrapping_add(1);
            gm.set_g8(g::PARTY_Z, z);
        } else {
            leave = true;
        }
    }
    if leave {
        exit_dungeon(gm);
    }
    ovl::dnglook::load_dungeon_images(gm, 1);
    if gm.g8(g::LOCATION) != 0 {
        spawn_dungeon_monster(gm, 1);
        redraw_dungeon_view(gm);
    }
    0
}

/// orig 1000:9fe0 (315 bytes). The Klimb command in a dungeon (ROOT dispatch_command 'K' with
/// location > 0x20; vector 1778:0506). Up is possible on a ladder-up cell (0x10/0x30) or, with
/// a grapple, under a ceiling hole (raw cell bit 8); down on a ladder-down (0x20/0x30) or a
/// pit (0x60). Both: prints "Klimb-U/D-" and reads a key: 3/'U' up, 4/'D' down, space = "Pass".
/// Neither: "Klimb-what?" or "Klimb-\nWith What?" (ceiling hole but no grapple), returns 0.
/// One: prints "Klimb-". Then `dungeon_klimb_ladder(0, -1 or +1)`; if it reports leaving the
/// dungeon, `exit_dungeon`. Returns 1 (time passes).
pub fn cmd_klimb_dungeon(gm: &mut Game) -> u16 {
    let mut up = false;
    let mut down = false;
    let py = gm.g8(g::PARTY_Y) as u16;
    let px = gm.g8(g::PARTY_X) as u16;
    let t = (dungeon_cell_at(gm, py, px) as u16) & 0xf0;
    let raw = gm.g8(party_cell_off(gm));
    if t == 0x10 || t == 0x30 || ((raw & 8) != 0 && gm.g8(g::GRAPPLE) != 0) {
        up = true;
    }
    if t == 0x20 || t == 0x30 || t == 0x60 {
        down = true;
    }
    if up && down {
        text::print_string(gm, 0x6cba); // "Klimb-U/D-"
        while up && down {
            let k = input::read_command_key(gm);
            if k == 3 {
                down = false;
            } else if k == 4 {
                up = false;
            } else if k == 0x20 {
                text::print_string(gm, 0x6cc6); // "Pass\n\n"
                down = false;
                up = false;
            } else if k == 0x44 { // 'D'
                up = false;
            } else if k == 0x55 { // 'U'
                down = false;
            }
        }
    } else {
        if !up && !down {
            let s: u16 = if (raw & 8) == 0 { 0x6cea } else { 0x6cd6 }; // "Klimb-what?" / "Klimb-\nWith What?"
            text::print_string(gm, s);
            return 0;
        }
        text::print_string(gm, 0x6cce); // "Klimb-"
    }
    if (up && dungeon_klimb_ladder(gm, 0, -1) != 0) || (down && dungeon_klimb_ladder(gm, 0, 1) != 0) {
        exit_dungeon(gm);
    }
    1
}
