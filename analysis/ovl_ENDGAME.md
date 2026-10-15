# ENDGAME.OVL — the ending

Level-3 overlay (1000:a290–acf9) translated in `rust/src/ovl/endgame.rs`; records in
`analysis/ovl_ENDGAME.jsonl`. Single entry point `endgame_sequence` (a8d8, vector 1778:04ca), called by
SJOG `end_combat` and DUNGEON's room-combat path when `g_combat_exit == 'M'` (Lord British's cell in the
Underworld freed).

## Control flow
1. **Scene** (a8d8): status redraw, driver tile-patch command 4 (`driver_tile_patch_special(1)`),
   MISCMAPS.DAT offset 0x210 (11×16) → g_view_objects → g_light_map rows (stride 32), ENDMSG.DAT (1000
   bytes) → 0xb21e. All monster-slot tiles cleared; slot 0x1f = Lord British (tile 0x7c) at (5,8) walks
   to (5,3) with `move_slot_toward` (one axis per step, footstep pause a78e). Each party member: a dead
   one is revived ("<name> lives!", hp = max) with a flash + sweep; its figure (tile 0x1ade[class index
   in "AMBFDTPRS"]) appears in slot m at (5,9) and walks to (0x3e5a[m], 0x3e60[m]).
2. **Dialogue**: ENDMSG 0xb21e + name, question 0xb23f, Y/N (N → 0xb267 asked again).
3. **Yes + g_sandalwood_box**: LB walks to (5,4),(5,5); box = slot 6 tile 0xe at (5,4); six ENDMSG
   pages; box tile 8, sweep, box removed; g_light_map[4][5] = 0xdc and g_moongate_height 1→16; LB then
   every member walks into (5,4) and vanishes; gate 15→0; tile 0x44 drawn at (4,5); page 1 cleared
   black; `end_story_screens`; `print_quest_report` (never returns).
4. **Otherwise**: "I see...", 0xb4f3, slots 2/0x1f/0 walk to (8,6)/(4,1)/(8,4), then slots 1,3,4,5
   `wander_slot` forever (random step onto g_view_tiles == 0x44).
5. **Story screens** (a290 incl. the a366/a454/a476 JMP fragments): disk 2; PROPORT.PCS via
   `load_file_to_mem`, TEXT.16 and END1.16/END2.16 via `load_image_file`; `rect_effect` dissolve; six
   screens driven by the DATA.OVL tables g_end_story_number/g_end_hook/g_end_image_index/g_end_pic_x/y/
   g_end_left_x/g_end_right_x/g_end_para1_y/g_end_para2_y/g_end_first_line_x/g_end_text_y/
   g_end_text_offsets (0x3da6..0x3dfe): fill the FONT layout block 0x5146..0x5158, load END.DAT text
   (2000 bytes at the offset) to 0xb21e and render with FONT `draw_proportional_text`; key between
   screens; finally ENDSC.16 at (0,0x28) and back to text mode.
6. **Certificate** (a5b6): "Be it known that on the <ordinal day> Day of the <ordinal month> Month of
   the Year <N> Hundred <N> … <name> the Avatar saved the life of our sovereign Lord British …", then
   "Report now, thy Quest compleat in N years, N months, N days to Lord British at Origin Systems!".
   Words are accumulated in 0xbcb2 (len 0xbcda) by `append_wrapped_word` and flushed at '\n'. Number
   words from 0x3e0a (One..Twenty), 0x3e2e (tens), 0x3e40 (ordinals); itoa for the elapsed time
   (start date year 139, month 4, day 5). The original then spins forever.

## Data structures
- Monster table slots used as scene actors: 0 party figure, 1..5 members, 6 box, 0x1f Lord British.
- DGROUP: 0x260c "TEXT.16", 0x261a END1.16/END2.16 pointers, 0x2622 "ENDSC.16", 0x25ea[0] PROPORT.PCS,
  0x1ade class tiles, 0x3e5a/0x3e60 member targets, 0x5146.. FONT layout block, 0xad99 = light_map[4][5].
- Files: MISCMAPS.DAT, ENDMSG.DAT, END.DAT, END1.16, END2.16, TEXT.16, ENDSC.16, PROPORT.PCS.

## Port notes
- a366/a454/a476 are JMP targets inside a290 (Ghidra split them, decompiling with unassigned BP
  locals); their logic is in `end_story_screens`, the stubs are no-ops.
- `itoa` is reimplemented inline (`itoa_decimal` into a `with_buf` scratch buffer).
- The two infinite loops (certificate, wandering) pump the window via `wait_ticks`/`animate_wait`.
- `calls_driver`: entry 36 through `driver_tile_patch_special(1)` (tile-set patch command 4).
