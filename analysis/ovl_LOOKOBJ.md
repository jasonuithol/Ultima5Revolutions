# LOOKOBJ.OVL — the Look command (surface/settlements) and the gem map

Level-2 overlay (load segment 1a29 = 1000:a290, shares its region with NPC/COMBAT/BLCKTHRN/…).
26 listed functions, 1000:a290–1000:b45b. Rust: `rust/src/ovl/lookobj.rs`. Records:
`analysis/ovl_LOOKOBJ.jsonl`.

## Entry points (vector targets) and callers
| vector | target | name | called by |
|---|---|---|---|
| 1778:07a6 | 1000:ac2c | `cmd_look` | ROOT `dispatch_command` 'L' when g_location is not a dungeon (dungeons use DNGLOOK) |
| 1778:078e | 1000:b38c | `view_gem_map(y, x)` | ROOT 'V'iew a gem (surface), CAST cd3a/d15e (gem-view spell), `cmd_look` mirror |
| 1778:079a | 1000:a5f6 | `look_telescope` | `look_terrain`, CAST d712 |

Everything else is internal. 1000:a8d8 (ENDGAME vector 04ca) is a fragment of `look_terrain` in
this overlay; the bytes adb8..adef are an unreferenced fragment.

## Control flow of `cmd_look`
1. `get_direction()`; target = party x/y + g_dx/g_dy; `tile = *get_map_tile(y, x)`;
   `obj = find_object_at(z, y, x)`.
2. Mirror (tile 0x29): `select_target_member`; rand(1..30) >= intelligence → "Death vision!"
   (1 damage), else "Strange vision!" → `view_gem_map(party y, party x)`.
3. "\nThou dost see\n". Object present → `print_object_description(obj)` (LOOK2.DAT entry
   0x100+tile). Sign tiles 0x89, 0x8a, 0xa0, 0xa4, 0xf8 → "\n" + `look_sign(y, x, z)`.
   Anything else → `look_terrain(y, x, tile)`.
4. `look_terrain`: chases big-object parts 0xe0 (up), 0xe1 (right), 0xe2 (left) to their base
   tile; 0x59 telescope, 0xa1 wishing well, 0xd8–0xdb fountain; otherwise the LOOK2.DAT text plus
   clock time (0xfa/0xfb), the Lycaeum/Empath Abbey/Serpent's Hold word (0xde), the dungeon name
   from the x coordinate (0xdf), or a "\n" to close the line.
5. `look_telescope`: day → "the sun!" and 1 damage to the active member; night → blank view with
   the telescope in the centre, 80 random stars, the eight planets positioned by stepping their
   orbit index back one valid column per day since 139-04-05 (tables 0x3750/0x3758/0x3760), a
   crescent next to a planet whose number equals a Shadowlord's town, "the night sky! ", key wait.
6. `look_sign`: Yew wanted poster (loc 4, z 0, x 0x11, y 0x15) is built from the party names;
   otherwise signs.dat's 33 u16 offsets select the location's group, the group is loaded over
   g_ool_buffer (pre-filled with 0xff) and scanned for a header {loc, z, x, y} matching z/x/y.
   `print_sign_text` renders it: bytes with bit 7 clear are runes (font 1), set = plain (font 0);
   0x26/0x27 → glyph 0x6c; 0x29–0x31 → one of 9 box-drawing rows (pointers at 0x37be); 0x0d =
   page break (font 0 + wait for key). Offset 0 / end marker 0xff → the "LIVE BY THE EIGHT LAWS"
   default box.

## Gem map (`view_gem_map`)
Page 1: black 8..0xb7 square, then 32×32 cells of 4×4 pixels at (0x20 + col*4, 0x20 + row*4)
(`set_gem_cell_origin` → g_dx/g_dy), tile from `get_map_tile(chunk_y+row, chunk_x+col)`.
`draw_gem_cell` dispatches on `g_tile_class_1d1a[tile]` (0x1d1a):
1 speckles, 2 colour3+8 fill, 3 colour0 fill, 4 top/bottom lines, 5 centre 2×2, 6 frame, 7/0x5a
colour1 fill, 8 sun checker, 9 stripes, 10 water (per-pixel colour from nibble table 0x3822
[tile & 0xf], tiles 0x60–0x6f), 11 deep dots, 12 one deep pixel, 13 mixed dots, 14 two vertical
lines, 15 colour2+8 fill, 16 building (edge bits 0x3812[tile], doorway pixel for 0x22–0x25).
"Deep" colour = g_ui_color4 on CGA/Hercules (adapter 0/3), else g_ui_color2+8. The square is
copied to page 0, the party cell blinks (fill_rect_xor every 4 polls) until a key, then
`refresh_map_view`.

## Data
* Files: LOOK2.DAT (0x200 u16 offsets, NUL-terminated strings; 0x80 bytes read per look),
  SIGNS.DAT (33 u16 offsets; sign = {location, z, x, y} + text + NUL; 2000-byte group reads).
* DGROUP: strings 0x71f8–0x752c; 0x3750 planet start index, 0x3758 planet row, 0x3760 8×22 orbit
  visibility, 0x37be sign row pointers (0x29..0x31 → 0x7388..0x7418), 0x3812/0x3822 gem masks,
  0x1d1a tile class; RAM: g_ool_buffer 0xb21e (2000-byte overrun to 0xb9ed), g_view_tiles
  0xab02 (stride 0x20, centre 0xac47), g_dx/g_dy 0x5876/0x5878 as the gem cell origin.

## Port notes
* `draw_gem_cell` originally returned junk in AX; the Rust version returns nothing.
* `look_well` returns the message offset from inside the `with_buf` closure (0 = already done).
* The telescope date loop is kept literal (one iteration per day since year 139).
* `view_gem_map` calls `gm.pf.maybe_present()` inside the blink loop so the window keeps pumping.
