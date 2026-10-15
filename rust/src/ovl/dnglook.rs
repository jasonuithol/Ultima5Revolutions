//! DNGLOOK.OVL (translated)
//!
//! Dungeon helpers used by DUNGEON, ROOT and CMDS: the dungeon Look command (`look_in_dungeon`,
//! vector 1778:07b2), the View-a-gem dungeon map (`view_gem_dungeon`, 1778:07ca), the cleared-room
//! bookkeeping (g_dungeon_rooms_cleared bits <-> 0xf0/0xa0 map cells), the construction of the
//! 11x11 combat map in g_light_map for dungeon encounters/camps (`build_dungeon_combat_map`), the
//! placement of party members, monsters and items from a DUNGEON.CBT room (`setup_room_combatants`),
//! the party move after leaving a combat map (`apply_combat_exit_to_dungeon`) and the loading /
//! freeing of the DNGn.16 / ITEMS.16 / MONn.16 image files around every combat.
//!
//! Ghidra split `look_in_dungeon` (a290) into five pieces because the vectors 1778:0326 / 085a /
//! 0842 point into the same bytes when NPC / SHOPPES are loaded: a366, a42a, a430 and a448 are
//! fragments of a290 (kept as documented no-ops), and b542 is the tail of `setup_room_combatants`.
#![allow(unused_variables, dead_code, unused_imports, non_snake_case, unused_assignments)]
use crate::game::Game;
use crate::g;
use crate::engine::*;
use crate::ovl;

/// DGROUP offset of dungeon-map cell (z, y, x) in g_dungeon_map (0x595a, 8 levels of 8x8).
#[inline] fn cell_off(z: u16, y: u16, x: u16) -> u16 {
    g::DUNGEON_MAP.wrapping_add(z.wrapping_mul(0x40)).wrapping_add(y.wrapping_mul(8)).wrapping_add(x)
}
/// Offset of the cell the party stands on (unmasked x/y, as the original code computes it).
#[inline] fn party_cell_off(gm: &Game) -> u16 {
    cell_off(gm.g8(g::PARTY_Z) as u16, gm.g8(g::PARTY_Y) as u16, gm.g8(g::PARTY_X) as u16)
}
/// Offset of combat-map byte (row, col) in g_light_map (0xad14; 11 rows of 32 bytes).
#[inline] fn lm(row: u16, col: u16) -> u16 {
    g::LIGHT_MAP.wrapping_add(row.wrapping_mul(0x20)).wrapping_add(col)
}
/// View-gem flood-fill queues: 0x100 x bytes at 0xa528 and 0x100 y bytes at 0xa628 (ring buffers).
const GEM_QUEUE_X: u16 = 0xa528;
const GEM_QUEUE_Y: u16 = 0xa628;
/// Rooms that are never marked cleared: count at 0x3840, bytes (location & 0xf) * 0x10 + room at 0x383a.
const PERSISTENT_ROOMS: u16 = 0x383a;
const PERSISTENT_ROOM_COUNT: u16 = 0x3840;
/// Item quantity tables indexed by item value 3..15: range at 0x383f, base at 0x384d.
const ITEM_QTY_RANGE: u16 = 0x383f;
const ITEM_QTY_BASE: u16 = 0x384d;
/// Random monster types substituted for room tiles 0xec..0xef (8 bytes).
const RANDOM_ROOM_MONSTERS: u16 = 0x385e;
/// Wandering dungeon monster types (8 bytes) used for dungeon camps.
const DNG_MONSTER_TYPES: u16 = 0x173c;
/// Monster record bytes (monster-table slot 1) and the dungeon image segments.
const DNG_MONSTER_KIND: u16 = 0x5c62;
const DNG_MONSTER_X: u16 = 0x5c64;
const DNG_MONSTER_Y: u16 = 0x5c65;
const DNG_MONSTER_TYPE: u16 = 0x5c67;
const DNG_WALL_IMG_SEG: u16 = 0xa9c0;
const DNG_ITEMS_IMG_SEG: u16 = 0xa9c4;
const DNG_MONSTER_IMG_SEG: u16 = 0xa9c6;
/// g_raw_file_names[3] = "TILES.16" (0x25f0); [4] = "ITEMS.16" (0x25f2); [4+style] = "DNGn.16".
const RAW_NAME_TILES16: u16 = 0x25f0;
const RAW_NAME_ITEMS16: u16 = 0x25f2;
/// Combat-map centre tile per dungeon cell type (0x244a: 00 c8 c9 c8 dc d8 00 00).
const CELL_CENTRE_TILES: u16 = 0x244a;
/// Party positions for dungeon encounters (6 bytes each): camp x 0x2452 / y 0x2458, and the
/// four facing-dependent tables 0x245e / 0x2464 / 0x246a / 0x2470.
const CAMP_PARTY_X: u16 = 0x2452;
const CAMP_PARTY_Y: u16 = 0x2458;
/// Monster positions (16 bytes each): by facing 0x2476 / 0x2486 / 0x2496 / 0x24a6, camp x 0x24b6 / y 0x24c6.
const CAMP_MONSTER_X: u16 = 0x24b6;
const CAMP_MONSTER_Y: u16 = 0x24c6;

/// orig 1000:a290 (209 bytes, runs on through the fragments a366/a42a/a430/a448 to the RET at
/// a513). The dungeon Look command (ROOT dispatch_command 'L' when 0x20 < location < 0x29;
/// vector 1778:07b2). Picks the looking member (select_target_member, -1 aborts); without a torch
/// or light spell prints "You see:\ndarkness." Otherwise SJOG `select_search_cell_dungeon`
/// (left/right/ahead/here) sets g_dx/g_dy or returns 0 (Pass). The cell at (z, dy & 7, dx & 7)
/// is described after "You see:" - a hidden pit 0x61 counts as 0x00: fields 0x80-0x83 by low
/// nibble (sleep / poison gas / wall of fire / electric, else "An energy field"), 0xc0 walls by
/// g_dungeon_style (1 "a dripping stalactite", 2 "a caved in passage", else 1/255 "an
/// unfortunate software pirate" or "a less fortunate adventurer"), and by type nibble 0 passage,
/// 1 up ladder, 2 down ladder, 3 ladder, 4 wooden chest, 5 fountain, 6 pit, 7 open chest, 9
/// "nothing of note", a/e/f heavy door, b/d wall, c "SPEC WALL ERR". A fountain (0x5x) then asks
/// "Will you drink?" (Y/N): N prints "No."; Y prints "Yes.  Gulp!" and 0x50 cures (status 'G'),
/// 0x51 heals (hp = max hp), 0x52 poisons (status 'P'), anything else "Bad taste." and
/// damage_party_member(rand 0..7).
pub fn look_in_dungeon(gm: &mut Game) {
    let member = rest::select_target_member(gm);
    if member == 0xffff {
        return;
    }
    if gm.g8(g::TORCH_TURNS) == 0 && gm.g8(g::LIGHT_SPELL_TURNS) == 0 {
        text::print_string(gm, 0x752e); // "You see:\ndarkness.\n"
        return;
    }
    let facing = gm.gi8(g::DUNGEON_FACING) as i16;
    if ovl::sjog::select_search_cell_dungeon(gm, facing) == 0 {
        return;
    }
    let dx = gm.g16(g::DX);
    let dy = gm.g16(g::DY);
    let mut cell = gm.g8(cell_off(gm.g8(g::PARTY_Z) as u16, dy & 7, dx & 7)) as u16;
    text::print_string(gm, 0x7542); // "You see:\n"
    if cell == 0x61 {
        cell = 0;
    }
    let hi = cell & 0xf0;
    if hi == 0x80 {
        let s = match cell {
            0x80 => 0x754c, // "A sleep field.\n"
            0x81 => 0x755c, // "A poison gas field.\n"
            0x82 => 0x7572, // "A wall of fire.\n"
            0x83 => 0x7584, // "An electric field.\n"
            _ => 0x7598,    // "An energy field.\n"
        };
        text::print_string(gm, s);
        return;
    }
    if hi == 0xc0 {
        let style = gm.g8(g::DUNGEON_STYLE) & 0xf;
        let s = if style == 1 {
            0x75aa // "a dripping stalactite.\n"
        } else if style == 2 {
            0x75c2 // "a caved in passage.\n"
        } else if util::rand_range(gm, 0xff, 1) == 0xff {
            0x75d8 // "an unfortunate software pirate.\n"
        } else {
            0x75fa // "a less fortunate adventurer.\n"
        };
        text::print_string(gm, s);
        return;
    }
    // fragments a430 (0x30..0x60) and a448 (0x80..0xf0) are the remaining cases of this switch
    let s: u16 = match hi {
        0x00 => 0x7618, // "a passage.\n"
        0x10 => 0x7624, // "an up ladder.\n"
        0x20 => 0x7634, // "a down ladder.\n"
        0x30 => 0x7644, // "a ladder.\n"
        0x40 => 0x7650, // "a wooden chest.\n"
        0x50 => 0x7662, // "a fountain.\n"
        0x60 => 0x7670, // "a pit.\n"
        0x70 => 0x7678, // "an open chest.\n"
        0x80 => 0x7688, // "an energy field.\n" (unreachable: handled above)
        0x90 => 0x769a, // "nothing of note.\n"
        0xa0 => 0x76ac, // "a heavy door.\n"
        0xb0 => 0x76bc, // "a wall.\n"
        0xc0 => 0x76c6, // "SPEC WALL ERR.\n" (unreachable: handled above)
        0xd0 => 0x76d6, // "a wall.\n"
        0xe0 => 0x76e0, // "a heavy door.\n"
        0xf0 => 0x76f0, // "a heavy door.\n"
        _ => 0,
    };
    if s != 0 {
        text::print_string(gm, s);
    }
    // LAB_1000_a3bf: fountains offer a drink
    if hi != 0x50 {
        return;
    }
    text::print_string(gm, 0x7700); // "Will you drink?\n"
    let mut c: i8;
    loop {
        c = input::read_command_key(gm) as i8;
        if c == 0x59 { // 'Y'
            break;
        }
        if c == 0x4e { // 'N'
            break;
        }
    }
    if c != 0x4e {
        text::print_string(gm, 0x7718); // "Yes.  Gulp!\n"
        let rec = member.wrapping_mul(0x20);
        if cell == 0x50 {
            text::print_string(gm, 0x7726); // "Cured!\n"
            gm.set_g8(g::PARTY_STATUS.wrapping_add(rec), 0x47); // 'G'
            return;
        }
        if cell != 0x51 {
            if cell != 0x52 {
                text::print_string(gm, 0x7744); // "Bad taste.\n"
                let amount = util::rand_range(gm, 7, 0);
                rest::damage_party_member(gm, amount, member);
                return;
            }
            text::print_string(gm, 0x7738); // "Poisoned!\n"
            gm.set_g8(g::PARTY_STATUS.wrapping_add(rec), 0x50); // 'P'
            return;
        }
        text::print_string(gm, 0x772e); // "Healed!\n"
        let max = gm.g16(g::PARTY_MAX_HP.wrapping_add(rec));
        gm.set_g16(g::PARTY_HP.wrapping_add(rec), max);
        return;
    }
    text::print_string(gm, 0x7712); // "No.\n"
}

/// orig 1000:a366 (42 bytes). Not a function: the 0xc0-wall message tail of `look_in_dungeon`
/// (Ghidra split it off because vector 1778:0326 points here when NPC is loaded).
pub fn look_fragment_a366(gm: &mut Game) {}

/// orig 1000:a42a (31 bytes). Not a function: the 0xf0 "a heavy door." case of `look_in_dungeon`
/// plus the fountain-drink tail (vector 1778:085a lands here when SHOPPES is loaded).
pub fn look_fragment_a42a(gm: &mut Game) {}

/// orig 1000:a430 (43 bytes). Not a function: the 0x30..0x60 cases (ladder / wooden chest /
/// fountain / pit) of `look_in_dungeon`.
pub fn look_fragment_a430(gm: &mut Game) {}

/// orig 1000:a448 (233 bytes). Not a function: the 0x80..0xf0 cases of `look_in_dungeon` and the
/// shared "Will you drink?" tail (runs to the RET at a513).
pub fn look_fragment_a448(gm: &mut Game) {}

/// orig 1000:a514 (187 bytes). Draws the 6x8 striped energy-field icon of the View-gem dungeon
/// map at pixel (x+1..x+6, y..y+7): two rows each in g_ui_color4+8, g_ui_color0+8,
/// g_ui_color2+8 and g_ui_color3+8. Ghidra order: param_1 = y, param_2 = x (source order x, y).
pub fn draw_gem_field_icon(gm: &mut Game, param_1: i16, param_2: i16) {
    let c4 = gm.gi16(g::UI_COLOR4).wrapping_add(8);
    gfx::set_color(gm, c4);
    let x2 = param_2.wrapping_add(6);
    let x1 = param_2.wrapping_add(1);
    gfx::draw_line(gm, param_1, x2, param_1, x1);
    gfx::draw_line(gm, param_1.wrapping_add(1), x2, param_1.wrapping_add(1), x1);
    let c0 = gm.gi16(g::UI_COLOR0).wrapping_add(8);
    gfx::set_color(gm, c0);
    gfx::draw_line(gm, param_1.wrapping_add(2), x2, param_1.wrapping_add(2), x1);
    gfx::draw_line(gm, param_1.wrapping_add(3), x2, param_1.wrapping_add(3), x1);
    let c2 = gm.gi16(g::UI_COLOR2).wrapping_add(8);
    gfx::set_color(gm, c2);
    gfx::draw_line(gm, param_1.wrapping_add(4), x2, param_1.wrapping_add(4), x1);
    gfx::draw_line(gm, param_1.wrapping_add(5), x2, param_1.wrapping_add(5), x1);
    let c3 = gm.gi16(g::UI_COLOR3).wrapping_add(8);
    gfx::set_color(gm, c3);
    gfx::draw_line(gm, param_1.wrapping_add(6), x2, param_1.wrapping_add(6), x1);
    gfx::draw_line(gm, param_1.wrapping_add(7), x2, param_1.wrapping_add(7), x1);
}

/// orig 1000:a5d0 (833 bytes). Visits one cell of the View-gem dungeon map (row/col 0..0x15 of
/// the 22x22 display, the party at 11,11). Returns 0 when out of range or already visited
/// (g_light_map[row][col] == 0); otherwise marks it visited, draws the glyph of dungeon cell
/// (z, (row + party_y - 11) & 7, (col + party_x - 11) & 7) at text cursor (row+1, col+1) and
/// returns 1 so the flood fill continues - except for walls (b, c, d), which draw and return 0.
/// Glyphs (font 1 unless noted): hall with a ceiling hole (bit 8) 0x18 font 0 in g_ui_color_moon,
/// ladders up/down/both 0x2e/0x2d/0x2f (moon), chest 0x70 (g_ui_color_sun), fountain = drawn
/// with draw_line/plot_pixel in g_ui_color1 / g_ui_color2+8 at pixel (col*8, row*8), pits: 0x60
/// -> 0x19 font 0, 0x61/0x69 (hidden) -> 0x71, 0x68 -> 0x12 font 0, else 0x72 in g_ui_color0+8;
/// field (8) -> `draw_gem_field_icon`; door a / room f -> 0x73 (sun); wall b: 0xb0 -> 0x7f font 0,
/// signed wall -> 0x74; c -> 0x75 on g_ui_color2 background; secret door d -> 0x76 on g_ui_color2
/// (CGA) or g_ui_color_moon (EGA/Tandy) background; door e -> 0x77 (sun); 7 and 9 draw nothing.
/// Always restores g_ui_color1 text colour and font 0. Ghidra order: param_1 = row, param_2 = col.
pub fn gem_map_visit_cell(gm: &mut Game, param_1: i16, param_2: i16) -> u16 {
    let mut result: u16 = 0;
    if param_2 < 0 || 0x15 < param_2 || param_1 < 0 || 0x15 < param_1 {
        return 0;
    }
    let visited = lm(param_1 as u16, param_2 as u16);
    'body: {
        if gm.g8(visited) == 0 {
            break 'body;
        }
        result = 1;
        gm.set_g8(visited, 0);
        let cx = (param_2 as u16).wrapping_add(gm.g8(g::PARTY_X) as u16).wrapping_sub(0xb) & 7;
        let cy = (param_1 as u16).wrapping_add(gm.g8(g::PARTY_Y) as u16).wrapping_sub(0xb) & 7;
        let coff = cell_off(gm.g8(g::PARTY_Z) as u16, cy, cx);
        let cell = gm.g8(coff);
        text::set_cursor_pos(gm, param_1.wrapping_add(1), param_2.wrapping_add(1));
        let px = param_2.wrapping_mul(8);
        let py = param_1.wrapping_mul(8);
        let c1 = gm.gi16(g::UI_COLOR1);
        gfx::set_color(gm, c1);
        let moon = gm.g16(g::UI_COLOR_MOON) as u8;
        let sun = gm.g16(g::UI_COLOR_SUN) as u8;
        let glyph: u8;
        match cell >> 4 {
            0 => {
                if gm.g8(coff) & 8 == 0 {
                    break 'body;
                }
                text::set_text_color(gm, moon);
                glyph = 0x18;
            }
            1 => {
                text::select_font(gm, 1);
                text::set_text_color(gm, moon);
                glyph = 0x2e;
            }
            2 => {
                text::select_font(gm, 1);
                text::set_text_color(gm, moon);
                glyph = 0x2d;
            }
            3 => {
                text::select_font(gm, 1);
                text::set_text_color(gm, moon);
                glyph = 0x2f;
            }
            4 => {
                text::select_font(gm, 1);
                text::set_text_color(gm, sun);
                glyph = 0x70;
            }
            5 => {
                // fountain: a small basin drawn with lines and pixels
                let xa = px.wrapping_add(0xe);
                let xb = px.wrapping_add(9);
                gfx::draw_line(gm, py.wrapping_add(0xc), xa, py.wrapping_add(0xc), xb);
                let xc = px.wrapping_add(0xd);
                let xd = px.wrapping_add(10);
                gfx::draw_line(gm, py.wrapping_add(0xd), xc, py.wrapping_add(0xd), xd);
                let ye = py.wrapping_add(0xe);
                gfx::draw_line(gm, ye, xd, ye, xb);
                gfx::draw_line(gm, ye, xa, ye, xc);
                let c2 = gm.gi16(g::UI_COLOR2).wrapping_add(8);
                gfx::set_color(gm, c2);
                let ya = py.wrapping_add(10);
                gfx::plot_pixel(gm, ya, xb);
                gfx::plot_pixel(gm, py.wrapping_add(9), xd);
                gfx::draw_line(gm, ya, px.wrapping_add(0xc), ya, px.wrapping_add(0xb));
                gfx::draw_line(gm, py.wrapping_add(0xb), px.wrapping_add(0xc), py.wrapping_add(0xb), px.wrapping_add(0xb));
                gfx::plot_pixel(gm, py.wrapping_add(9), xc);
                gfx::plot_pixel(gm, ya, xa);
                break 'body;
            }
            6 => {
                text::set_text_color(gm, moon);
                let c = gm.gi8(coff);
                if c == 0x60 {
                    glyph = 0x19;
                } else if c == 0x61 || c == 0x69 {
                    text::select_font(gm, 1);
                    glyph = 0x71;
                } else if c == 0x68 {
                    glyph = 0x12;
                } else {
                    text::select_font(gm, 1);
                    let c0 = gm.g16(g::UI_COLOR0).wrapping_add(8) as u8;
                    text::set_text_color(gm, c0);
                    glyph = 0x72;
                }
            }
            8 => {
                draw_gem_field_icon(gm, py.wrapping_add(8), px.wrapping_add(8));
                break 'body;
            }
            0xa | 0xf => {
                text::select_font(gm, 1);
                text::set_text_color(gm, sun);
                glyph = 0x73;
            }
            0xb => {
                let ch: u8 = if gm.g8(coff) == 0xb0 {
                    0x7f
                } else {
                    text::select_font(gm, 1);
                    0x74
                };
                text::put_char(gm, ch);
                result = 0;
                break 'body;
            }
            0xc => {
                text::select_font(gm, 1);
                let c2 = gm.g16(g::UI_COLOR2) as u8;
                text::set_text_bg(gm, c2);
                text::put_char(gm, 0x75);
                text::set_text_bg(gm, 0);
                result = 0;
                break 'body;
            }
            0xd => {
                text::select_font(gm, 1);
                let adapter = gm.gi16(g::ADAPTER_TYPE);
                let mut bg = gm.g16(g::UI_COLOR2) as u8;
                let set = if adapter == 0 {
                    true
                } else {
                    bg = gm.g16(g::UI_COLOR_MOON) as u8;
                    adapter == 1 || adapter == 2
                };
                if set {
                    text::set_text_bg(gm, bg);
                }
                text::put_char(gm, 0x76);
                text::set_text_bg(gm, 0);
                result = 0;
                break 'body;
            }
            0xe => {
                text::select_font(gm, 1);
                text::set_text_color(gm, sun);
                glyph = 0x77;
            }
            _ => break 'body, // 7 (open chest) and 9: nothing drawn, flood continues
        }
        text::put_char(gm, glyph);
    }
    let c1 = gm.g16(g::UI_COLOR1) as u8;
    text::set_text_color(gm, c1);
    text::select_font(gm, 0);
    result
}

/// orig 1000:a938 (388 bytes). The View command with a gem inside a dungeon (ROOT
/// dispatch_command 'V' when location > 0x20; vector 1778:07ca). Clears the view (x/y 8..0xb7),
/// fills 0x2e0 bytes of g_light_map with 0xff (= unvisited), seeds the two 0x100-byte ring
/// queues at 0xa528 (x) / 0xa628 (y) with the party cell (11,11) and marks it visited
/// (g_light_map[11][11] = 0). Draws the party glyph 0x60 (font 1, g_ui_color3+8, or g_ui_color1
/// on Hercules) at text (12,12) of window 0, then breadth-first visits the 8 neighbours of every
/// queued cell with `gem_map_visit_cell`, enqueueing those that return non-zero, until the queue
/// is empty (write pointer == read pointer). Finally selects window 2, waits for a key, clears
/// the view again and calls DUNGEON `redraw_dungeon_view`.
pub fn view_gem_dungeon(gm: &mut Game) {
    gfx::set_color(gm, 0);
    gfx::fill_rect(gm, 0xb7, 0xb7, 8, 8);
    let mut rd_x: u16 = GEM_QUEUE_X;
    let mut rd_y: u16 = GEM_QUEUE_Y;
    for i in 0..0x2e0u16 {
        gm.set_g8(g::LIGHT_MAP.wrapping_add(i), 0xff);
    }
    gm.set_g8(GEM_QUEUE_X, 0xb);
    let mut wr_x: u16 = GEM_QUEUE_X + 1;
    gm.set_g8(GEM_QUEUE_Y, 0xb);
    let mut wr_y: u16 = GEM_QUEUE_Y + 1;
    gm.set_g8(0xae7f, 0); // g_light_map[11][11]: the party cell is visited
    let c1 = gm.gi16(g::UI_COLOR1);
    gfx::set_color(gm, c1);
    text::select_text_window(gm, 0);
    text::set_cursor_pos(gm, 0xc, 0xc);
    text::select_font(gm, 1);
    let c3 = gm.g16(g::UI_COLOR3).wrapping_add(8) as u8;
    text::set_text_color(gm, c3);
    if gm.gi16(g::ADAPTER_TYPE) == 3 {
        text::set_text_color(gm, c1 as u8);
    }
    text::put_char(gm, 0x60);
    text::set_text_color(gm, c1 as u8);
    text::select_font(gm, 0);
    loop {
        if wr_x == rd_x {
            text::select_text_window(gm, 2);
            input::wait_key(gm, 0);
            gfx::set_color(gm, 0);
            gfx::fill_rect(gm, 0xb7, 0xb7, 8, 8);
            ovl::dungeon::redraw_dungeon_view(gm);
            return;
        }
        let y = gm.g8(rd_y) as u16;
        let x = gm.g8(rd_x) as u16;
        for n in 0..8i16 {
            let (ny, nx): (u16, u16) = match n {
                0 => (y.wrapping_sub(1), x.wrapping_sub(1)),
                1 => (y.wrapping_sub(1), x),
                2 => (y.wrapping_sub(1), x.wrapping_add(1)),
                3 => (y, x.wrapping_sub(1)),
                4 => (y, x.wrapping_add(1)),
                5 => (y.wrapping_add(1), x.wrapping_sub(1)),
                6 => (y.wrapping_add(1), x),
                _ => (y.wrapping_add(1), x.wrapping_add(1)),
            };
            if gem_map_visit_cell(gm, ny as i16, nx as i16) != 0 {
                gm.set_g8(wr_x, nx as u8);
                wr_x = wr_x.wrapping_add(1);
                gm.set_g8(wr_y, ny as u8);
                wr_y = wr_y.wrapping_add(1);
                if 0xa627 < wr_x {
                    wr_x = GEM_QUEUE_X;
                    wr_y = GEM_QUEUE_Y;
                }
            }
        }
        rd_x = rd_x.wrapping_add(1);
        rd_y = rd_y.wrapping_add(1);
        if 0xa627 < rd_x {
            rd_x = GEM_QUEUE_X;
            rd_y = GEM_QUEUE_Y;
        }
    }
}

/// Dungeon index used by the cleared-room bits: location - 0x21, but location - 0x22 when that
/// is > 0 (Deceit and Despise share index 0 - quirk preserved, TODO(verify)).
#[inline] fn cleared_room_bit(gm: &Game, room: i16) -> i16 {
    let mut idx: i16 = gm.gi8(g::LOCATION) as i16 - 0x21;
    if 0 < idx {
        idx = gm.gi8(g::LOCATION) as i16 - 0x22;
    }
    idx.wrapping_mul(0x10).wrapping_add(room)
}

/// orig 1000:aad4 (143 bytes). Marks dungeon room `room` (0..15) of the current dungeon as
/// cleared (vector 1778:04e2, DUNGEON `enter_dungeon_room` after a room fight ends normally).
/// Rooms listed in the 6-byte table at 0x383a (byte = (location & 0xf) * 0x10 + room: 50 5b 41
/// 46 4b 4c, count at 0x3840) are never marked. Otherwise bit (dungeon_index * 16 + room) of
/// g_dungeon_rooms_cleared (0x58e0) is set, the byte index being the signed quotient by 8.
pub fn mark_dungeon_room_cleared(gm: &mut Game, param_1: i16) {
    let n = gm.g8(PERSISTENT_ROOM_COUNT) as u16;
    if n != 0 {
        let key = ((gm.g8(g::LOCATION) & 0xf) as i16).wrapping_mul(0x10).wrapping_add(param_1);
        let mut i: u16 = 0;
        loop {
            if gm.g8(PERSISTENT_ROOMS.wrapping_add(i)) as i16 == key {
                return;
            }
            i += 1;
            if !(i < n) {
                break;
            }
        }
    }
    let bit = cleared_room_bit(gm, param_1);
    let off = g::DUNGEON_ROOMS_CLEARED.wrapping_add((bit / 8) as u16);
    let v = gm.g8(off) | (1u8 << ((bit as u8) & 7));
    gm.set_g8(off, v);
}

/// orig 1000:ab64 (101 bytes). True when bit (dungeon_index * 16 + room) of
/// g_dungeon_rooms_cleared is set (same index quirk as `mark_dungeon_room_cleared`).
pub fn dungeon_room_is_cleared(gm: &mut Game, param_1: i16) -> bool {
    let bit = cleared_room_bit(gm, param_1);
    let off = g::DUNGEON_ROOMS_CLEARED.wrapping_add((bit / 8) as u16);
    (gm.g8(off) & (1u8 << ((bit as u8) & 7))) != 0
}

/// orig 1000:abca (68 bytes). Applies the cleared-room bits to the freshly loaded dungeon map
/// (vector 1778:049a, DUNGEON `dungeon_main` on entry): every one of the 0x200 cells whose type
/// nibble is 0xf (room) and whose room number (low nibble) is cleared gets `&= 0xaf`, turning it
/// into a 0xa0 "cleared room" cell.
pub fn apply_cleared_rooms_to_map(gm: &mut Game) {
    for i in 0..0x200u16 {
        let off = g::DUNGEON_MAP.wrapping_add(i);
        let c = gm.g8(off);
        if (c & 0xf0) == 0xf0 {
            if dungeon_room_is_cleared(gm, (c & 0xf) as i16) {
                gm.set_g8(off, c & 0xaf);
            }
        }
    }
}

/// orig 1000:ac0e (199 bytes). Closes side `dir` (0 N, 1 E, 2 S, 3 W) of the 11x11 dungeon
/// combat map with 11 black (0xff) cells along the outer edge: row 0 / col 10 / row 10 / col 0.
/// When the party's own cell is a door (`cell_hi` == 0xe0) a north wall also puts the dungeon
/// wall tile at (2,5) and (8,5), and a west wall at (5,2) and (5,8) - the door pillars.
/// Ghidra order: param_1 = dir, param_2 = cell type nibble << 4 (as a signed char).
pub fn combat_map_wall_side(gm: &mut Game, param_1: i16, param_2: i8) {
    let mut dx: i16 = 0;
    let mut dy: i16 = 0;
    let mut x: i16 = 0;
    let mut y: i16 = 0;
    if param_1 == 0 {
        dx = 1;
    } else if param_1 == 1 {
        dy = 1;
        x = 10;
    } else if param_1 == 2 {
        dx = 1;
        y = 10;
    } else if param_1 == 3 {
        dy = 1;
    }
    y <<= 5;
    for _ in 0..11 {
        gm.set_g8(g::LIGHT_MAP.wrapping_add(y.wrapping_add(x) as u16), 0xff);
        x = x.wrapping_add(dx);
        y = y.wrapping_add(dy.wrapping_mul(0x20));
    }
    let wall = gm.g8(g::DUNGEON_WALL_TILE);
    if param_1 == 0 && param_2 == -0x20 {
        gm.set_g8(0xae19, wall); // g_light_map[8][5]
        gm.set_g8(0xad59, wall); // g_light_map[2][5]
    }
    if param_1 == 3 && param_2 == -0x20 {
        gm.set_g8(0xadbc, wall); // g_light_map[5][8]
        gm.set_g8(0xadb6, wall); // g_light_map[5][2]
    }
}

/// orig 1000:acd8 (165 bytes). Opens a 5-cell doorway of g_dungeon_floor_tile in the inner wall
/// ring on side `dir`: N row 1 cols 3..7, E col 9 rows 3..7, S row 9 cols 3..7, W col 1 rows 3..7.
pub fn combat_map_opening5(gm: &mut Game, param_1: i16) {
    let mut dx: i16 = 0;
    let mut dy: i16 = 0;
    let mut x: i16 = 3;
    let mut y: i16 = 3;
    if param_1 == 0 {
        dx = 1;
        y = 1;
    } else if param_1 == 1 {
        dy = 1;
        x = 9;
    } else if param_1 == 2 {
        dx = 1;
        y = 9;
    } else if param_1 == 3 {
        dy = 1;
        x = 1;
    }
    y <<= 5;
    for _ in 0..5 {
        let floor = gm.g8(g::DUNGEON_FLOOR_TILE);
        gm.set_g8(g::LIGHT_MAP.wrapping_add(y.wrapping_add(x) as u16), floor);
        x = x.wrapping_add(dx);
        y = y.wrapping_add(dy.wrapping_mul(0x20));
    }
}

/// orig 1000:ad7e (172 bytes). Opens a 7-cell gap of g_dungeon_floor_tile in the inner wall ring
/// on side `dir` (open corridor): N row 1 cols 2..8, E col 9 rows 2..8, S row 9 cols 2..8,
/// W col 1 rows 2..8. For a `dir` outside 0..3 the start position is uninitialised in the
/// original (never happens: callers pass 0..3).
pub fn combat_map_opening7(gm: &mut Game, param_1: i16) {
    let mut dx: i16 = 0;
    let mut dy: i16 = 0;
    let mut x: i16 = 0; // uninitialised in the original for dir > 3
    let mut y: i16 = 0;
    if param_1 == 0 {
        dx = 1;
        x = 2;
        y = 1;
    } else if param_1 == 1 {
        x = 9;
        dy = 1;
        y = 2;
    } else if param_1 == 2 {
        dx = 1;
        x = 2;
        y = 9;
    } else if param_1 == 3 {
        x = 1;
        dy = 1;
        y = 2;
    }
    let mut row: i16 = y << 5;
    for _ in 0..7 {
        let floor = gm.g8(g::DUNGEON_FLOOR_TILE);
        gm.set_g8(g::LIGHT_MAP.wrapping_add(row.wrapping_add(x) as u16), floor);
        x = x.wrapping_add(dx);
        row = row.wrapping_add(dy.wrapping_mul(0x20));
    }
}

/// orig 1000:ae2e (204 bytes). Shapes side `dir` (0 N, 1 E, 2 S, 3 W) of the dungeon combat map
/// from the neighbouring dungeon cell in that direction (x/y wrapped & 7 on the party's level):
/// a neighbour below 0xa0 (open cell) gets the 7-wide gap (`combat_map_opening7`); a wall
/// neighbour (0xb0, 0xc0, 0xd0) gets the outer black edge (`combat_map_wall_side`, passing the
/// party's own cell type for the door pillars); anything else (doors, rooms, 0xa0) gets the
/// 5-wide doorway (`combat_map_opening5`) - so open neighbours get both the 7 and the 5 gap.
pub fn combat_map_side_from_neighbour(gm: &mut Game, param_1: i16) {
    let mut x = gm.g8(g::PARTY_X) as u16;
    let mut y = gm.g8(g::PARTY_Y) as u16;
    if param_1 == 0 {
        y = y.wrapping_sub(1);
    } else if param_1 == 1 {
        x = x.wrapping_add(1);
    } else if param_1 == 2 {
        y = y.wrapping_add(1);
    } else if param_1 == 3 {
        x = x.wrapping_sub(1);
    }
    let nb = gm.g8(cell_off(gm.g8(g::PARTY_Z) as u16, y & 7, x & 7)) & 0xf0;
    if nb < 0xa0 {
        combat_map_opening7(gm, param_1);
    }
    if nb == 0xb0 || nb == 0xc0 || nb == 0xd0 {
        let own = (gm.g8(party_cell_off(gm)) & 0xf0) as i8;
        combat_map_wall_side(gm, param_1, own);
    } else {
        combat_map_opening5(gm, param_1);
    }
}

/// orig 1000:aefc (209 bytes). Builds the walls of the dungeon combat map in g_light_map
/// (vector 1778:0536; also called by `build_dungeon_combat_map`): rows 1 and 9 (cols 0..10) and
/// columns 1 and 9 (rows 0..10) get g_dungeon_wall_tile, the four corners (0,0) (0,10) (10,0)
/// (10,10) become 0xff. If the party's cell type is 1..7 the centre tile g_combat_map_centre is
/// taken from the table at 0x244a (00 c8 c9 c8 dc d8 00 00: ladder up/down/both, chest,
/// fountain) when non-zero; for a two-way ladder (3) g_tiles_patched = 1 and driver_tile_patch(0)
/// alters the ladder tile, otherwise g_tiles_patched = 0. Finally the four sides are shaped by
/// `combat_map_side_from_neighbour`.
pub fn build_dungeon_combat_walls(gm: &mut Game) {
    for i in 0..0xbu16 {
        let wall = gm.g8(g::DUNGEON_WALL_TILE);
        gm.set_g8(0xad34u16.wrapping_add(i), wall); // g_light_map[1][0..11]
    }
    for i in 0..0xbu16 {
        let wall = gm.g8(g::DUNGEON_WALL_TILE);
        gm.set_g8(0xae34u16.wrapping_add(i), wall); // g_light_map[9][0..11]
    }
    let mut p1: u16 = 0xad15; // g_light_map[0][1]
    let mut p9: u16 = 0xad1d; // g_light_map[0][9]
    loop {
        let wall = gm.g8(g::DUNGEON_WALL_TILE);
        gm.set_g8(p9, wall);
        gm.set_g8(p1, wall);
        p1 = p1.wrapping_add(0x20);
        p9 = p9.wrapping_add(0x20);
        if !(p9 < 0xae7d) {
            break;
        }
    }
    gm.set_g8(0xae5e, 0xff); // [10][10]
    gm.set_g8(0xae54, 0xff); // [10][0]
    gm.set_g8(0xad1e, 0xff); // [0][10]
    gm.set_g8(g::LIGHT_MAP, 0xff); // [0][0]
    let cell = gm.g8(party_cell_off(gm));
    let hi = cell & 0xf0;
    if hi != 0 && hi < 0x80 {
        let t = (cell >> 4) as u16;
        let c = gm.gi8(CELL_CENTRE_TILES.wrapping_add(t));
        if c != 0 {
            gm.set_g8(g::COMBAT_MAP_CENTRE, c as u8);
        }
        if t == 3 {
            gm.set_g8(g::TILES_PATCHED, 1);
            rest::driver_tile_patch(gm, 0);
        } else {
            gm.set_g8(g::TILES_PATCHED, 0);
        }
    }
    for d in 0..4i16 {
        combat_map_side_from_neighbour(gm, d);
    }
}

/// orig 1000:afce (661 bytes). Builds a complete 0x160-byte combat map in g_light_map for a
/// dungeon encounter or camp (vector 1778:04a6: DUNGEON `dungeon_monster_attacks` and ROOT
/// hole_up_and_camp before run_encounter). Rows 0..10 cols 0..10 = g_dungeon_floor_tile; a camp
/// (g_combat_flags & 4) sets the centre tile to 0xb3 (camp fire); `build_dungeon_combat_walls`.
/// Then the CBT-format side data: row 8 cols 11..26 (trigger positions) = 0xff; party positions
/// rows 1..4 (x cols 11..16, y cols 17..22) from the DGROUP tables 0x245e/0x2464/0x246a/0x2470
/// (camp: x 0x2452, y 0x2458 in every row); monster positions row 6 (x) / row 7 (y), cols
/// 11..26, from the facing tables 0x2476/0x2486/0x2496/0x24a6 (N: x 2476 y 2496; E: 2486/24a6;
/// S: 24a6/2486; W: 2496/2476; camp: x 0x24b6, y 0x24c6). Row 5 cols 11..26 (monster tiles) is
/// zeroed and a random permutation of 0..15 is built by 16 swaps with rand_range(0,15). The
/// monster type is monster slot 1's type (0x5c67), or for a camp a random entry of 0x173c; the
/// group size is rand_range(1, g_monster_stats[type].group) (0x13c2 + type*8), or the full
/// group when that byte is 8 or 16; the first `count` permuted entries get tile 0x40 + type*4.
pub fn build_dungeon_combat_map(gm: &mut Game) {
    let mut p: u16 = g::LIGHT_MAP;
    loop {
        let floor = gm.g8(g::DUNGEON_FLOOR_TILE);
        for i in 0..0xbu16 {
            gm.set_g8(p.wrapping_add(i), floor);
        }
        p = p.wrapping_add(0x20);
        if !(p < 0xae74) {
            break;
        }
    }
    if (gm.g8(g::COMBAT_FLAGS) & 4) != 0 {
        gm.set_g8(g::COMBAT_MAP_CENTRE, 0xb3);
    }
    build_dungeon_combat_walls(gm);
    for i in 0..8u16 {
        gm.set_g8(0xae27u16.wrapping_add(i), 0xff); // [8][19..27]
        gm.set_g8(0xae1fu16.wrapping_add(i), 0xff); // [8][11..19]
    }
    let mut flags = gm.g8(g::COMBAT_FLAGS);
    let mut r3: u16 = 0xad7f; // [3][11]
    let mut r2: u16 = 0xad5f; // [2][11]
    let mut r4: u16 = 0xad9f; // [4][11]
    let mut r1: u16 = 0xad3f; // [1][11]
    for i in 0..6u16 {
        if (flags & 4) == 0 {
            let v = gm.g8(0x2470u16.wrapping_add(i)); gm.set_g8(r3, v);
            let v = gm.g8(0x246au16.wrapping_add(i)); gm.set_g8(r2, v);
            let v = gm.g8(0x245eu16.wrapping_add(i)); gm.set_g8(r4, v);
            let v = gm.g8(0x2464u16.wrapping_add(i)); gm.set_g8(r1, v);
        } else {
            let v = gm.g8(CAMP_PARTY_X.wrapping_add(i));
            gm.set_g8(r1, v);
            gm.set_g8(r4, v);
            gm.set_g8(r2, v);
            gm.set_g8(r3, v);
        }
        flags = gm.g8(g::COMBAT_FLAGS);
        r3 = r3.wrapping_add(1);
        r2 = r2.wrapping_add(1);
        r4 = r4.wrapping_add(1);
        r1 = r1.wrapping_add(1);
    }
    for i in 0..6u16 {
        if (flags & 4) == 0 {
            let v = gm.g8(0x2464u16.wrapping_add(i)); gm.set_g8(r3, v);
            let v = gm.g8(0x245eu16.wrapping_add(i)); gm.set_g8(r2, v);
            let v = gm.g8(0x246au16.wrapping_add(i)); gm.set_g8(r4, v);
            let v = gm.g8(0x2470u16.wrapping_add(i)); gm.set_g8(r1, v);
        } else {
            let v = gm.g8(CAMP_PARTY_Y.wrapping_add(i));
            gm.set_g8(r1, v);
            gm.set_g8(r4, v);
            gm.set_g8(r2, v);
            gm.set_g8(r3, v);
        }
        flags = gm.g8(g::COMBAT_FLAGS);
        r3 = r3.wrapping_add(1);
        r2 = r2.wrapping_add(1);
        r4 = r4.wrapping_add(1);
        r1 = r1.wrapping_add(1);
    }
    let facing = gm.gi8(g::DUNGEON_FACING);
    let mut mx: u16 = 0xaddf; // [6][11]
    let mut my: u16 = 0xadff; // [7][11]
    for i in 0..0x10u16 {
        if (flags & 4) == 0 {
            let (xt, yt): (u16, u16) = match facing {
                0 => (0x2476, 0x2496),
                1 => (0x2486, 0x24a6),
                2 => (0x24a6, 0x2486),
                3 => (0x2496, 0x2476),
                _ => (0, 0), // LAB_1000_b135: nothing written
            };
            if xt != 0 {
                let v = gm.g8(xt.wrapping_add(i));
                gm.set_g8(mx, v);
                let v = gm.g8(yt.wrapping_add(i));
                gm.set_g8(my, v);
            }
        } else {
            let v = gm.g8(CAMP_MONSTER_Y.wrapping_add(i));
            gm.set_g8(my, v);
            let v = gm.g8(CAMP_MONSTER_X.wrapping_add(i));
            gm.set_g8(mx, v);
        }
        mx = mx.wrapping_add(1);
        my = my.wrapping_add(1);
    }
    // random placement order for the monster tiles (row 5 cols 11..26)
    let mut order = [0i16; 16];
    for i in 0..16 {
        order[i] = i as i16;
    }
    for i in 0..16usize {
        let j = util::rand_range(gm, 0xf, 0) as usize;
        let t = order[i];
        order[i] = order[j];
        order[j] = t;
        gm.set_g8(0xadbfu16.wrapping_add(i as u16), 0);
    }
    let mut mtype = gm.g8(DNG_MONSTER_TYPE);
    if (gm.g8(g::COMBAT_FLAGS) & 4) != 0 {
        let k = util::rand_range(gm, 7, 0);
        mtype = gm.g8(DNG_MONSTER_TYPES.wrapping_add(k as u16));
    }
    let stats = (mtype as u16).wrapping_mul(8).wrapping_add(g::MONSTER_GROUP_SIZE);
    let group = gm.gi8(stats);
    let mut count: i16 = util::rand_range(gm, group as i16, 1);
    if group == 8 || group == 0x10 {
        count = gm.g8(stats) as i16;
    }
    if 0 < count {
        let tile = mtype.wrapping_mul(4).wrapping_add(0x40);
        for k in 0..count as usize {
            gm.set_g8(0xadbfu16.wrapping_add(order[k] as u16), tile);
        }
    }
}

/// orig 1000:b26a (193 bytes). Moves the party after a dungeon combat/room according to
/// g_combat_exit (vector 1778:04ee; DUNGEON after `enter_dungeon_room` and
/// `dungeon_monster_attacks`): 1 (W) x-1, facing/dir 3, x > 7 -> 7; 2 (E) x+1, facing 1,
/// x > 7 -> 0; 3 (N) y-1, facing 0, y > 7 -> 7; 4 (S) y+1, facing 2, y > 7 -> 0; 5 (up) z-1 and
/// g_dungeon_dir_copy = 4 unless already on level 0; 6 (down) z+1 and dir 5 unless on level 7;
/// climbing off the top or bottom calls DUNGEON `exit_dungeon` and sets g_location = 0.
/// Any other exit code leaves the party where it is.
pub fn apply_combat_exit_to_dungeon(gm: &mut Game) {
    let exit = gm.gi8(g::COMBAT_EXIT);
    if exit == 1 {
        let x = gm.g8(g::PARTY_X).wrapping_sub(1);
        gm.set_g8(g::PARTY_X, x);
        gm.set_g8(g::DUNGEON_FACING, 3);
        gm.set_g8(g::DUNGEON_DIR_COPY, 3);
        if 7 < gm.g8(g::PARTY_X) {
            gm.set_g8(g::PARTY_X, 7);
        }
    } else if exit == 2 {
        let x = gm.g8(g::PARTY_X).wrapping_add(1);
        gm.set_g8(g::PARTY_X, x);
        gm.set_g8(g::DUNGEON_FACING, 1);
        gm.set_g8(g::DUNGEON_DIR_COPY, 1);
        if 7 < gm.g8(g::PARTY_X) {
            gm.set_g8(g::PARTY_X, 0);
        }
    } else if exit == 3 {
        let y = gm.g8(g::PARTY_Y).wrapping_sub(1);
        gm.set_g8(g::PARTY_Y, y);
        gm.set_g8(g::DUNGEON_FACING, 0);
        gm.set_g8(g::DUNGEON_DIR_COPY, 0);
        if 7 < gm.g8(g::PARTY_Y) {
            gm.set_g8(g::PARTY_Y, 7);
        }
    } else if exit == 4 {
        let y = gm.g8(g::PARTY_Y).wrapping_add(1);
        gm.set_g8(g::PARTY_Y, y);
        gm.set_g8(g::DUNGEON_FACING, 2);
        gm.set_g8(g::DUNGEON_DIR_COPY, 2);
        if 7 < gm.g8(g::PARTY_Y) {
            gm.set_g8(g::PARTY_Y, 0);
        }
    } else {
        if exit == 5 {
            if gm.g8(g::PARTY_Z) != 0 {
                let z = gm.g8(g::PARTY_Z).wrapping_sub(1);
                gm.set_g8(g::PARTY_Z, z);
                gm.set_g8(g::DUNGEON_DIR_COPY, 4);
                return;
            }
        } else {
            if exit != 6 {
                return;
            }
            if gm.g8(g::PARTY_Z) != 7 {
                let z = gm.g8(g::PARTY_Z).wrapping_add(1);
                gm.set_g8(g::PARTY_Z, z);
                gm.set_g8(g::DUNGEON_DIR_COPY, 5);
                return;
            }
        }
        ovl::dungeon::exit_dungeon(gm);
        gm.set_g8(g::LOCATION, 0);
    }
}

/// orig 1000:b32e (145 bytes). (Re)loads the first-person dungeon images when (re)entering the
/// dungeon view (vector 1778:048e; DUNGEON after every room/combat and ROOT hole_up_and_camp).
/// Does nothing when g_location == 0 (the party left). If g_dungeon_flags bit 1 is set the
/// combat tile picture (TILES.16) is freed (free_picture); if bit 0 is clear DNGn.16
/// (g_raw_file_names[4 + g_dungeon_style] -> 0xa9c0) and ITEMS.16 (g_raw_file_names[4] -> 0xa9c4)
/// are loaded, retrying while load_image_file returns 0. g_dungeon_flags = 1. With
/// `reset_monster` != 0 monster slot 1 is cleared: kind bytes 0x5c62/0x5c63 = 0, x/y
/// 0x5c64/0x5c65 = 0xff, type 0x5c67 = 0xff (none). Both pages' view rectangle (8..0xb7) is
/// cleared to colour 0.
pub fn load_dungeon_images(gm: &mut Game, param_1: i16) {
    if gm.g8(g::LOCATION) != 0 {
        if (gm.g8(g::DUNGEON_FLAGS) & 2) != 0 {
            gfx::free_picture(gm);
        }
        if (gm.g8(g::DUNGEON_FLAGS) & 1) == 0 {
            loop {
                let name = gm.g16(RAW_NAME_ITEMS16.wrapping_add((gm.g8(g::DUNGEON_STYLE) as u16).wrapping_mul(2)));
                let seg = gfx::load_image_file(gm, name) as u16;
                gm.set_g16(DNG_WALL_IMG_SEG, seg);
                if seg != 0 {
                    break;
                }
            }
            loop {
                let name = gm.g16(RAW_NAME_ITEMS16);
                let seg = gfx::load_image_file(gm, name) as u16;
                gm.set_g16(DNG_ITEMS_IMG_SEG, seg);
                if seg != 0 {
                    break;
                }
            }
        }
        gm.set_g8(g::DUNGEON_FLAGS, 1);
        if param_1 != 0 {
            gm.set_g8(DNG_MONSTER_KIND.wrapping_add(1), 0);
            gm.set_g8(DNG_MONSTER_KIND, 0);
            gm.set_g8(DNG_MONSTER_Y, 0xff);
            gm.set_g8(DNG_MONSTER_X, 0xff);
            gm.set_g8(DNG_MONSTER_TYPE, 0xff);
        }
        gfx::set_color(gm, 0);
        gfx::fill_rect(gm, 0xb7, 0xb7, 8, 8);
        gfx::set_draw_page(gm, 1);
        gfx::fill_rect(gm, 0xb7, 0xb7, 8, 8);
        gfx::set_draw_page(gm, 0);
    }
}

/// orig 1000:b3c0 (78 bytes). Leaves the first-person dungeon view before a combat, room, camp
/// or exit (vector 1778:0482; DUNGEON and ROOT hole_up_and_camp). If g_dungeon_flags bit 0
/// (images loaded): frees MONn.16 (0xa9c6) when a monster exists (type 0x5c67 != -1), zeroes
/// 0xa9c6, frees ITEMS.16 (0xa9c4) and DNGn.16 (0xa9c0). If bit 1 is clear loads the tile
/// picture TILES.16 (g_raw_file_names[3], retrying while load_picture_file returns 0). If bit 2
/// is clear runs driver_tile_patch_special(0) (driver tile-patch command 3: the dungeon colour
/// swap of the tile set). g_dungeon_flags = 6.
pub fn unload_dungeon_images(gm: &mut Game) {
    if (gm.g8(g::DUNGEON_FLAGS) & 1) != 0 {
        if gm.gi8(DNG_MONSTER_TYPE) != -1 {
            let seg = gm.g16(DNG_MONSTER_IMG_SEG);
            gfx::free_far_block(gm, seg);
        }
        gm.set_g16(DNG_MONSTER_IMG_SEG, 0);
        let seg = gm.g16(DNG_ITEMS_IMG_SEG);
        gfx::free_far_block(gm, seg);
        let seg = gm.g16(DNG_WALL_IMG_SEG);
        gfx::free_far_block(gm, seg);
    }
    if (gm.g8(g::DUNGEON_FLAGS) & 2) == 0 {
        loop {
            let name = gm.g16(RAW_NAME_TILES16);
            if gfx::load_picture_file(gm, name) != 0 {
                break;
            }
        }
    }
    if (gm.g8(g::DUNGEON_FLAGS) & 4) == 0 {
        rest::driver_tile_patch_special(gm, 0);
    }
    gm.set_g8(g::DUNGEON_FLAGS, 6);
}

/// orig 1000:b40e (307 bytes, runs through the fragment b542 to the RET at b630). Places the
/// combatants of a combat map held in g_light_map (CBT layout) - vector 1778:04be. Callers:
/// DUNGEON `enter_dungeon_room` (mode 3, cell = the room cell), ROOT run_encounter (mode 2, 0)
/// for dungeon fights, CMDS bf80 (modes 0 and 1). Unless mode == 1: the party's start row is
/// chosen by g_dungeon_facing (0 N -> row 3 "from south", 1 E -> row 2 "from west", 3 W -> row 1
/// "from east", 5 -> row 3, else row 4 "from north"), init_combat_party, and for each of
/// g_party_count members both g_roster_x/g_monster_x and g_roster_y/g_monster_y (stride 8) are
/// set from that row's cols 11+i (x) and 17+i (y). Then, if mode > 0 and (mode < 3, or mode ==
/// 3 with an uncleared room cell >= 0xf0): four random monster types are drawn from 0x385e for
/// the 0xec..0xef "random monster" tiles, and the 16 monster tiles of row 5 (cols 11..26) are
/// placed at (row 6 x, row 7 y): tile 0 = none; tile < 0x40, (tile & 0xfc) == 0xb4 or == 0xe8 ->
/// add_combatant mode 2 (object/item, value = tile); otherwise a monster, type = (tile-0x40)/4
/// (0xec.. -> random type). For objects the quantity byte (g_ship_hull + slot*8) is set: value
/// 1 -> z*3+7, 2 -> rand_range(1, z*10+10), 3..15 -> 0x384d[value] + rand_range(0,
/// 0x383f[value]-1), > 15 -> untouched. Ghidra order: param_1 = mode, param_2 = cell.
pub fn setup_room_combatants(gm: &mut Game, param_1: i16, param_2: i16) {
    let mut row: i16 = 4;
    if param_1 != 1 {
        let f = gm.gi8(g::DUNGEON_FACING);
        if f == 0 {
            row = 3;
        } else if f == 1 {
            row = 2;
        } else if f == 3 {
            row = 1;
        } else if f == 5 {
            row = 3;
        }
        rest::init_combat_party(gm);
        let n = gm.g8(g::PARTY_COUNT) as u16;
        if n != 0 {
            let mut rx = g::ROSTER_X;
            let mut mx = g::MONSTER_X;
            let mut ry = g::ROSTER_Y;
            let mut my = g::MONSTER_Y;
            let mut cx: u16 = 0xb;
            let mut cy: u16 = 0x11;
            let mut i: u16 = 0;
            loop {
                let v = gm.g8(lm(row as u16, cx));
                gm.set_g8(mx, v);
                gm.set_g8(rx, v);
                let v = gm.g8(lm(row as u16, cy));
                gm.set_g8(my, v);
                gm.set_g8(ry, v);
                cx += 1;
                cy += 1;
                rx = rx.wrapping_add(8);
                mx = mx.wrapping_add(8);
                ry = ry.wrapping_add(8);
                my = my.wrapping_add(8);
                i += 1;
                if i == n {
                    break;
                }
            }
        }
    }
    // LAB_1000_b4d8
    if 0 < param_1 && ((0xef < param_2 && param_1 == 3) || param_1 < 3) {
        let mut random_types = [0u8; 4];
        for i in 0..4 {
            let k = util::rand_range(gm, 7, 0);
            random_types[i] = gm.g8(RANDOM_ROOM_MONSTERS.wrapping_add(k as u16));
        }
        // fragment b542: the placement loop over the 16 monster entries
        for k in 0..0x10u16 {
            let tile = gm.g8(lm(5, 0xb + k));
            if tile != 0 {
                let mode: i16;
                let mut val: u16;
                if tile < 0x40 || (tile & 0xfc) == 0xb4 || (tile & 0xfc) == 0xe8 {
                    mode = 2;
                    val = tile as u16;
                } else {
                    mode = 0;
                    val = (tile as u16).wrapping_sub(0x40) >> 2;
                }
                if (tile & 0xfc) == 0xec {
                    val = random_types[(tile & 3) as usize] as u16;
                }
                let z = gm.g8(g::PARTY_Z);
                let y = gm.g8(lm(7, 0xb + k));
                let x = gm.g8(lm(6, 0xb + k));
                let slot = rest::add_combatant(gm, z, y, x, mode, val as i16);
                if mode == 2 {
                    let qty: i8;
                    if val == 1 {
                        qty = (z as i8).wrapping_mul(3).wrapping_add(7);
                    } else if val == 2 {
                        qty = util::rand_range(gm, (z as i16).wrapping_mul(10).wrapping_add(10), 1) as i8;
                    } else {
                        if 0xf < val {
                            continue; // LAB_1000_b615
                        }
                        let range = gm.g8(ITEM_QTY_RANGE.wrapping_add(val)) as i16;
                        let r = util::rand_range(gm, range.wrapping_sub(1), 0) as i8;
                        qty = gm.gi8(ITEM_QTY_BASE.wrapping_add(val)).wrapping_add(r);
                    }
                    gm.set_g8(g::SHIP_HULL.wrapping_add(slot.wrapping_mul(8)), qty as u8);
                }
            }
        }
    }
}

/// orig 1000:b542 (239 bytes). Not a function: the monster/item placement loop of
/// `setup_room_combatants` (Ghidra split it off because vector 1778:0842 = SHOPPES b542 lands
/// here). No-op in Rust; the code lives in `setup_room_combatants`.
pub fn setup_room_fragment_b542(gm: &mut Game) {}
