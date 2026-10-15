# INTRO.OVL — start-up, title sequence, main menu, story, Ultima IV transfer

Level-1 overlay (load segment 181d, 8384 bytes + 16-byte relocation header; no code-space data
tables — every table it reads lives in DGROUP). 21 functions, 1000:81d0–1000:a286. Rust:
`rust/src/ovl/intro.rs`. Records: `analysis/ovl_INTRO.jsonl`.

## Entry points (vector targets) and callers
| vector | target | name | called by |
|---|---|---|---|
| 1778:02ae | 1000:8b46 | `run_intro` | ROOT `main_entry` (once, step 3 of start-up) |
| 1778:0542 | 1000:85fe | `draw_bottom_banner(text)` | run_intro, FONT_e5f8 (the View) |
| 1778:0572 | 1000:a250 | `title_fire_tick` | run_intro (idle), show_title_screen, FONT_e4dc |
| — | 1000:81d0 | `draw_title_fire_frames` | INTRO-internal only (Ghidra mislabelled it TOWN_81d0: TOWN shares the load address) |

Calls out: FONT_e1e0 (vector 1778:0566, proportional text drawer, `(text, PROPORT.PCS seg)`),
FONT_ecea (vector 1778:054e, "Create New Character"), FONT_e684 (vector 1778:055a, "the View" =
idle title animation). Ghidra labelled 054e/0566 as ROOT functions because it mis-decoded the
12-byte vectors; the bytes are `CALLF plink ; dw 0x17 ; JMPF 1000:ecea` / `… 1000:e1e0`.

## Control flow of `run_intro`
1. Hardware/driver init (sound on, identity tile map, adapter detection, Tandy low-memory → CGA,
   driver load, `init_graphics_driver(1)`, CGA/Hercules rename of `*.16` → `*.4` inside
   `g_raw_file_names`, otherwise the seven EGA palette words 0x13ae..0x13ba = 4,15,1,2,5,14,7,
   `driver_init_screen`, fonts IBM.CH/RUNES.CH or `.HCS`, window 0 full screen).
2. Title (skipped when a `J` is already in the keyboard buffer): TITLE.BIT + BRITISH.BIT raw,
   BRITISH.PTH over the save area 0x55a6, entry-37 zoom (`draw_stacked_pictures`, its AX=1
   abort flag decides whether the rest plays), TITLE.BIT images 7/8/9, the four-segment dotted
   route traced on the Britannia map (`trace_path_segment`, path bytes = dx/dy nibbles), then
   `show_title_screen(animated)` (ULTIMA.16, entry-34 dissolve, WD.BIT burn via entry 35 CF=1)
   and, if no key was pressed, FONT_e684.
3. Menu loop (`LAB_1000_8e90`): windows 0/1/2, copyright banner, `JCTUAR` menu with arrows
   (1/3 previous, 2/4 next), Enter/Space, hot keys, 200-poll idle timeout → `R`. Idle polls run
   `title_fire_tick` so the title keeps burning.
   * `J` Journey: `draw_game_frame`, wind, TILES.16 registered if needed, `select_disk(3)`,
     SAVED.GAM → 0x55a6 (0x1060). With a party: SAVED.OOL → 0xb21e, `select_disk(1)`, BRIT.OOL
     ← 0xb21e, UNDER.OOL ← 0xb31e (+ UNDER.OOL on disk 5 when outdoors in the Underworld),
     `set_wind_and_draw(-1)`, `g_cell_h = 8`, **return to main**. Without: message, back to menu.
   * `C` FONT_ecea; `T` `transfer_from_ultima4`; both then redraw the title (`show_title_screen(0)`).
   * `U` `play_story_sequence` (21 screens from STORYn.16/TEXT.16/STORY.DAT) then title.
   * `A` `show_acknowledgements` (STARTSC.16 curtain animation). `R` FONT_e684.

## Data
* DGROUP tables: story layout `g_story_*` 0x2f98..0x3104 (21 screens), file-name pointers
  0x25ea.. (0x25ec BRITISH.BIT, 0x25ee TITLE.BIT, 0x25f0 TILES.16, 0x260c TEXT.16, 0x260e..
  STORY1-6.16, 0x261e ULTIMA.16, 0x2620 STARTSC.16), strings 0x2f31..0x367c, `JCTUAR` 0x3270,
  stat label rows 0x3656 / pointers 0x3666, "WD.BIT" 0x3105, " Ultima IV " 0x6d3a.
* RAM: 0xbb18 `g_intro_path_pos` (u16), 0xbb1a ULTIMA.16 segment, 0x5146..0x5158 the
  proportional-text layout block consumed by FONT_e1e0, 0x3304 Avatar flag, 0xbc88 first U4
  party.sav record (hp,maxhp,exp,str,dex,int,mp u16; name +0x14; sex +0x24; class +0x25), 0xbb1c
  party.sav 0x140..0x1f5 (karma words at 0xbb22.., all 0 = Avatar).
* Files: EGA.DRV etc., IBM.CH, RUNES.CH, TITLE.BIT, BRITISH.BIT, BRITISH.PTH, ULTIMA.16, WD.BIT,
  STARTSC.16, PROPORT.PCS, TEXT.16, STORY1..6.16, STORY.DAT, TILES.16, SAVED.GAM, SAVED.OOL,
  BRIT.OOL, UNDER.OOL, UNDER.DAT, INIT.GAM, INIT.OOL, party.sav (Ultima IV).
* `g_location` doubles as the picture-registration state during the intro: `'@'` = nothing
  registered, `'A'` = TILES.16 registered (`'B'` is normalised to `'A'` by the transfer).

## Port notes
* `gfx::draw_stacked_pictures` now returns the entry-37 abort flag (i16) because run_intro tests it.
* `driver_init_screen` cannot fail in the port; the `exit(1)` branch is kept but dead.
* FONT_e1e0/ecea/e684 are still stubs: the story, Create and the View panic until FONT is done.
* `transfer_from_ultima4`: the original writes an uninitialised register to g_location on Esc
  in the drive-letter loop; the port restores the saved location (`TODO(verify)`).
* STORY.DAT paragraphs are 2000 bytes read into the 0x200-byte `g_ool_buffer` (overrun kept).
* `load_overlay("INTRO")` is called at the start of `run_intro` for uniformity (no tables needed).
