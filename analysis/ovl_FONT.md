# FONT.OVL — proportional text, "the View" demo, Create New Character

Level-3 overlay (load segment 1e1e = 1000:e1e0, shares its region with CAST2/ZSTATS/COMSUBS/
SHOPPES2/SHOPPES3). 10 functions, 1000:e1e0–1000:f076, no code-space data tables. Rust:
`rust/src/ovl/font.rs`. Records: `analysis/ovl_FONT.jsonl`.

## Entry points (vector targets) and callers
| vector | target | name | called by |
|---|---|---|---|
| 1778:0566 | 1000:e1e0 | `draw_proportional_text(text, proport_seg)` | INTRO `play_story_sequence` / story screens, FONT Create |
| 1778:055a | 1000:e684 | `run_the_view` | INTRO `run_intro` after the title, and menu 'R' |
| 1778:054e | 1000:ecea | `create_new_character` | INTRO menu 'C' |
| 1778:0a3a | 1000:e4dc | `view_idle_tick(count)` | ROOT `tile_transition_effect` while g_location == 'B'; internal |
| 1778:0986/099e | — | (vectors into the same region used by other level-3 overlays) | — |

Calls out: INTRO `title_fire_tick` (1778:0572) and `draw_bottom_banner` (1778:0542), FLAMES
`flames_entry` (1778:057e).

## Proportional text (`draw_proportional_text`)
Glyphs come from PROPORT.PCS (1-bpp, index = char − 0x20, `draw_image_masked` on page 1). The
caller fills the layout block: 0x5146/0x5148 left x and 0x514c/0x514e right x of two column
bands, band 1 active while 0x5150 < y < 0x5152; 0x5156/0x5158 cursor; 0x5154 space width.
Width table: byte per character at 0x50ca (0x50f7 = '-'). `'{'` is a 15-pixel gap, `'_'` a soft
hyphen (0 width; a line broken there gets glyph 0xd), bytes < 0x21 are spaces. Lines are
measured forward to the band's right edge, broken at the last space (dropped) or a fitting soft
hyphen, and justified by distributing the slack over the spaces (not on a line ended by NUL or
'\n'). Line pitch 9, nothing drawn at y >= 0xc0. The intro's story layout tables (`g_story_*`)
feed this block; Create uses full width 0..0x140 then a right column 0..0xa6.

## The View (`run_the_view`)
A scripted animation of MISCMAPS.DAT bytes 0x2c0.. (2000 bytes into g_ool_buffer 0xb21e: four
4×19 scene maps at 0x000/0x080/0x100/0x180, script from 0x200). Tiles are 16 pixels high
(g_cell_h = 16); the scene is drawn at screen rows 7..10, columns 0xbd26..0xbd27 (starting at 9
and widening one column each side every other frame). g_map 0x6608 is the current layer, 0x6708
the ground copy, 0x6688 the object layer stamped from g_monster_table each frame; 0xfe cells are
not drawn. Opcodes: 0 place object, 1 remove, 2 move (dir tables 0x24d6/0x24de), 3 wait n,
4/5 explosion/implosion effect frames at (x,y) leaving tile 0xdc / 5, 6 load scene (caption via
`draw_bottom_banner`, pointers 0x515c), 7/8 tile dissolve to the object's tile / the ground,
9 restart, 10 set ground tile, 11 lightning, 12 clear objects, 13 move + 7 frames, 14/15 loop.
Every frame (`view_idle_tick`): animate_monsters, title fire tick, scene redraw, key check,
scene 2 sea noise, scene 3 bell chimes. Any key leaves. First entry ('@'): the LZW scratch
segment is parked in the free level-2 area (FLAMES trick) while TILES.16 is registered.
g_location is used as state: '@' nothing registered, 'B' View data resident.

## Create New Character (`create_new_character`)
PROPORT.PCS + CREATE.16 loaded, INIT.GAM over 0x55a6, name (read_line 8 into g_party[0]) and
M/F (0x55b1 = 0xb/0xc). Seven gypsy questions (`ask_gypsy_question`): two random cards not yet
shown/eliminated (0xbd2a/0xbd32), images card+2 at (0x51fc[c], 0x5204[c]) (+0xb8 right), text
from QUESTION.DAT offset 0x517c[a][b], answer A/B; the chosen card adds 0x5164/0x516c/0x5174
[card] to the int/dex/str accumulators 0xbd3c/0xbd3d/0xbd3e, the other is eliminated. Rounds of
4, 2, 1 questions (shown flags cleared between). Result: int = MP = 0xbd3c, dex = 0xbd3d,
str = max(0xbd3e, 20); INIT.OOL → 0xb31e; SAVED.OOL (0x200) and SAVED.GAM (0x1060) written on
disk 3. g_location = '@' afterwards.

## Port notes
* `view_idle_tick` keeps the stub's `(gm, i16) -> u16` signature; gfx.rs calls it qualified.
* `save/restore_scratch_to_overlay_area` copy 0x3000 bytes between linear `scratch_seg << 4`
  and `0x10000 + g_unk_520c` (= 0x1a299, the FLAMES scratch offset). No level-2 overlay has
  code-space data tables, so clobbering that area is harmless in the port.
* Busy-wait key loops (`read_key` until non-zero) call `gm.pf.maybe_present()`.
* 0x5154 (g_font_space_width) is accessed as a 16-bit word by the code.
