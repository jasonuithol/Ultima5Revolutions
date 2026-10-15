# DNGLOOK.OVL — dungeon look / gem map / rooms / combat-map builder (22 functions, 1000:a290–1000:b630)

Translated to `rust/src/ovl/dnglook.rs`; per-function records in `ovl_DNGLOOK.jsonl`.
Level-1 overlay sharing the a290.. region with NPC / SHOPPES / FLAMES / COMBAT, which is why
Ghidra split two of its functions into pieces: a366, a42a, a430, a448 are fragments of
`look_in_dungeon` (vectors 1778:0326 / 085a land there when NPC / SHOPPES are loaded) and b542 is
the tail of `setup_room_combatants` (vector 1778:0842 = SHOPPES). The five fragments are
documented no-ops in Rust; their code lives in the parent functions. 17 real functions.

## Entry points (vectors) and callers

| vector | addr | Rust name | called by |
|---|---|---|---|
| 1778:07b2 | a290 | `look_in_dungeon()` | ROOT dispatch_command 'L' when 0x20 < location < 0x29 |
| 1778:07ca | a938 | `view_gem_dungeon()` | ROOT dispatch_command 'V' (gem) when location > 0x20 |
| 1778:049a | abca | `apply_cleared_rooms_to_map()` | DUNGEON dungeon_main on entry |
| 1778:04e2 | aad4 | `mark_dungeon_room_cleared(room)` | DUNGEON enter_dungeon_room after a won room |
| 1778:0536 | aefc | `build_dungeon_combat_walls()` | `build_dungeon_combat_map` (no outside caller found) |
| 1778:04a6 | afce | `build_dungeon_combat_map()` | DUNGEON dungeon_monster_attacks, ROOT hole_up_and_camp |
| 1778:04ee | b26a | `apply_combat_exit_to_dungeon()` | DUNGEON after rooms and monster fights |
| 1778:048e | b32e | `load_dungeon_images(reset_monster)` | DUNGEON (1), ROOT hole_up_and_camp |
| 1778:0482 | b3c0 | `unload_dungeon_images()` | DUNGEON (before rooms/fights/exit), ROOT hole_up_and_camp |
| 1778:04be | b40e | `setup_room_combatants(mode, cell)` | DUNGEON enter_dungeon_room (3, cell), ROOT run_encounter (2, 0), CMDS bf80 (0 / 1) |

Internal: `draw_gem_field_icon` (a514), `gem_map_visit_cell` (a5d0), `dungeon_room_is_cleared`
(ab64), `combat_map_wall_side` (ac0e), `combat_map_opening5` (acd8), `combat_map_opening7`
(ad7e), `combat_map_side_from_neighbour` (ae2e).

Outgoing: SJOG bfec `select_search_cell_dungeon` (vector 1778:07be; sets g_dx/g_dy), DUNGEON
`redraw_dungeon_view` (9db0) and `exit_dungeon` (9ed8, vector 1778:051e). ROOT: select_target_member,
damage_party_member, rand_range, read_command_key, wait_key, init_combat_party, add_combatant,
driver_tile_patch / driver_tile_patch_special, load_image_file / load_picture_file / free_picture /
free_far_block, text and gfx primitives.

## Control flow

**Look** (`look_in_dungeon`): target member → darkness check (torch 0x58a7 / light spell 0x58a6) →
SJOG picks left/right/ahead/here into g_dx/g_dy → cell = g_dungeon_map[z][dy&7][dx&7] (hidden pit
0x61 reads as a passage). Strings 0x752e–0x76f0 by type nibble; fields by low nibble; 0xc0 walls by
g_dungeon_style with the 1/255 "unfortunate software pirate" joke. Fountains (0x5x) ask "Will you
drink?": 0x50 cure (status 'G'), 0x51 heal (hp = max), 0x52 poison ('P'), else "Bad taste" and
0..7 damage.

**View gem** (`view_gem_dungeon`): the 22×22 map display is a breadth-first flood from the party
cell (11,11) through two 0x100-byte ring queues (x at 0xa528, y at 0xa628). g_light_map is reused
as the visited array (0xff unvisited → 0). `gem_map_visit_cell(row, col)` draws one glyph (font 1
chars 0x2d/0x2e/0x2f ladders, 0x70 chest, 0x71/0x72 pits, 0x73 door/room, 0x74–0x77 walls and
doors, fountain drawn with lines, field via `draw_gem_field_icon`) and returns 1 to continue the
flood except through walls (b, c, d). Ends with wait_key and DUNGEON `redraw_dungeon_view`.

**Rooms**: `apply_cleared_rooms_to_map` converts 0xfX room cells whose bit in
g_dungeon_rooms_cleared (0x58e0, bit = dungeon_index*16 + room) is set into 0xa0 "cleared room"
cells; `mark_dungeon_room_cleared` sets that bit after a won room unless the room is in the
persistent list at 0x383a (count 0x3840: Covetous 0 and 11, Wrong 1, 6, 11, 12 always respawn).
Dungeon index quirk: location−0x21, but location−0x22 when positive (Deceit and Despise share
index 0, as in DUNGEON's CBT offset) — TODO(verify).

**Combat-map construction** (dungeon fights and camps; room maps come straight from DUNGEON.CBT):
`build_dungeon_combat_map` fills 11×11 with g_dungeon_floor_tile (camp: centre 0xb3 fire),
`build_dungeon_combat_walls` paints the wall ring (rows/cols 1 and 9, corners 0xff), sets the
centre object from table 0x244a (ladder 0xc8/0xc9, chest 0xdc, fountain 0xd8; two-way ladder also
g_tiles_patched = 1 + driver_tile_patch(0)), then per side `combat_map_side_from_neighbour`: open
neighbour → 7-wide gap (`combat_map_opening7`) + 5-wide doorway; wall neighbour (0xb/0xc/0xd) →
outer edge 0xff (`combat_map_wall_side`, door pillars when the party stands in a door); otherwise
the 5-wide doorway (`combat_map_opening5`). The CBT side columns are then filled from DGROUP
tables: party rows 1–4 (0x245e/0x2464/0x246a/0x2470; camp 0x2452/0x2458), monster x/y rows 6–7
by facing (0x2476/0x2486/0x2496/0x24a6; camp 0x24b6/0x24c6), trigger row 8 = 0xff, and the
monster tile row 5 gets `count` copies of tile 0x40 + type*4 in a random permutation (count =
rand 1..group size at g_monster_stats+5, or the full group when that is 8 or 16; camps pick a
random type from 0x173c).

**Placement** (`setup_room_combatants`): party start positions come from the CBT row chosen by
facing (N→row 3 "from south", E→row 2, W→row 1, else row 4) into both g_roster_x/y and
g_monster_x/y (stride 8); then (modes 1, 2, or 3 with an uncleared ≥ 0xf0 room) the 16 monster
entries: tile < 0x40 / 0xb4–0xb7 / 0xe8–0xeb → add_combatant mode 2 object with quantity
g_ship_hull[slot*8] from 0x383f/0x384d (value 1 → z*3+7, 2 → rand 1..z*10+10); 0xec–0xef →
random monster type from 0x385e; others → monster type (tile−0x40)/4.

**Exit** (`apply_combat_exit_to_dungeon`): g_combat_exit 1–4 moves one cell and turns the party
(wrapping 0↔7), 5/6 change level with g_dungeon_dir_copy 4/5, leaving level 0 or 7 → DUNGEON
`exit_dungeon` + g_location = 0.

**Images**: `unload_dungeon_images` (before any combat) frees MONn.16 / ITEMS.16 / DNGn.16 when
flag bit 0, loads TILES.16 unless bit 1, applies driver tile-patch command 3 (the dungeon tile
colour swap, `driver_tile_patch_special(0)`) unless bit 2, and sets g_dungeon_flags = 6.
`load_dungeon_images` (after) frees the TILES.16 picture if bit 1, loads DNGn.16
(g_raw_file_names[4+style] → 0xa9c0) and ITEMS.16 ([4] → 0xa9c4) unless bit 0, sets flags = 1,
optionally resets monster slot 1, and clears the view on both pages. (ovl_DUNGEON.md attributes the
image loading to abca; it is b32e.)

## Data

* g_light_map 0xad14: 11 rows × 32 bytes in DUNGEON.CBT layout (cols 0..10 tiles; row 1–4 cols
  11–16/17–22 party x/y by entry side; row 5 cols 11–26 monster tiles; rows 6/7 monster x/y;
  row 8 trigger x/y; rows 9/10 new-tile positions). Also reused as the gem-map visited flags.
* g_dungeon_map 0x595a (8×8×8 cells), g_party_x/y/z 0x5896/0x5897/0x5895, g_dungeon_facing
  0x6603, g_dungeon_dir_copy 0x6602, g_dungeon_style 0x6604, g_dungeon_wall/floor_tile
  0xbb14/0xbb15, g_tiles_patched 0xbb16, g_dungeon_flags 0xbb17, g_combat_flags 0x58a1,
  g_combat_exit 0x58a0, g_combat_map_centre 0xadb9, g_dungeon_rooms_cleared 0x58e0.
* Monster slot 1 (0x5c62 kind, 0x5c64/65 x/y, 0x5c67 type); image segments 0xa9c0 DNGn.16,
  0xa9c4 ITEMS.16, 0xa9c6 MONn.16; raw file-name pointers 0x25f0 TILES.16, 0x25f2 ITEMS.16,
  0x25f4.. DNG1–3.16.
* DGROUP tables: 0x244a centre tiles, 0x2452/0x2458 camp party x/y, 0x245e–0x2470 party
  positions, 0x2476–0x24a6 monster positions by facing, 0x24b6/0x24c6 camp monster x/y, 0x173c
  dungeon monster types, 0x13c2 group sizes (stride 8), 0x383a/0x3840 persistent rooms,
  0x383f/0x384d item quantity range/base, 0x385e random room monster types.
* Strings 0x752e–0x7744 (look descriptions and the fountain dialogue).

## Port notes

* Ring queues and the visited array are kept in emulated memory (0xa528/0xa628, g_light_map) so
  the exact wrap behaviour at 0xa627 is preserved.
* `load_dungeon_images` / `unload_dungeon_images` loop forever if a file never loads — original
  behaviour (the loader prompts for the disk); load_image_file's −1 result is accepted as loaded.
* `setup_room_combatants` with mode 3 skips placement for 0xa0 (cleared) rooms, which is how a
  cleared room stays empty; the party positions are still set from the CBT rows.
* `build_dungeon_combat_map` writes no monster positions for facing values 4+ (LAB_1000_b135):
  row 6/7 then keep whatever the previous map held — preserved.
