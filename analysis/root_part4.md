# ROOT part 4 (1000:5dfe – 1000:72d4, plus main() at 1000:0000) — overview

30 game functions (the tail of ULTIMA.EXE) plus the C `main()` that Ghidra never listed. Records:
`analysis/root_part4.jsonl` (31 lines). Everything is Pascal convention (Ghidra call sites and
parameter lists are reversed; the protos give the true order and the `[bp+n]` slot). The chunk
also contains the PLINK86 overlay manager (172e), its data (176c) and the 164 call vectors (1778),
which are described below and are NOT in the JSONL (they will not be ported).

## Startup sequence and main-loop entry (the thing to get right)

1. **DOS entry 181d:0000** (PLINK stub): sets SS:SP, pushes `PSP+0x73e : 0x0322` and `RETF` →
2. **172e:0322 overlay-manager init**: switches to the manager stack (1778:0004, SP 0x100), stores the
   relocation delta `CS − descriptor[0].word0` into the descriptor table (1778:0000), prints the
   176c:00a0 "Error...Call OSI" message if the table is damaged, and pre-loads every overlay whose
   descriptor flag word has bit 0x4000 set (none in this build), then `JMPF 1000:017e`.
3. **1000:017e = MSC 5.x crt0**: requires DOS ≥ 2, SS = DGROUP (0x1f64), SP = 0xbd3e, shrinks the DOS
   block (INT 21h/4Ah) to end at the heap, zeroes BSS 0xa524..0xbd3f (so every RAM global starts 0),
   hooks INT 00h, runs the FP/initialiser tables (0x6ada), parses argv/envp, and `CALL main`
   (push envp, argv, argc); the return value goes to `exit` (02f4).
4. **`main(argc, argv)` at 1000:0000** (record "1000:0000"): command-line letter C/E/T/H → the four
   driver-selection flags 0x52ba/0x52ef/0x52f1/0x52f3; `g_error_hook = disk_error_hook`; disk state
   (g_cur_disk 0, g_display_mode 0, g_floppy_count 1, drive letters from the current drive);
   `g_ool_buffer_ptr (0xb11c) = 0xb21e`; `g_flush_after_key = 1`; then **`INTRO_8b46()`** does all the
   real initialisation in this order: sound on; identity `g_tile_anim_map`; detect_video_adapter;
   init_system (INT 23h/24h, CPU calibration); load_video_driver (retry loop); init_graphics_driver(1);
   palette/colour globals (0x13ae..0x13ba) for EGA/Tandy or renaming `*.16` → `*.4` names for CGA/Herc;
   driver_init_screen; load_font(0,"IBM.CH"), load_font(1,"RUNES.CH") (`.HCS` on Hercules); window 0 =
   full screen; first key `J` = jump straight to "Journey Onward", otherwise the title pictures and the
   JCTUAR menu (Journey, Create, Transfer from Ultima IV, ?, Acknowledgements, Return to DOS; the
   title path temporarily loads BRITISH.PTH into the save-game area). Journey: `draw_game_frame`
   (637e), `set_wind_and_draw(0)`, windows 1/2, select_disk(3), `load_file_region("SAVED.GAM",
   0x55a6, 0x1060, 0)`; if a game exists: SAVED.OOL → 0xb21e, select_disk(1), write BRIT.OOL from
   0xb21e and UNDER.OOL from 0xb31e (plus UNDER.OOL on disk 5 when the party is in the Underworld),
   `set_wind_and_draw(-1)`, `g_cell_h = 8`, return.
5. `draw_status_window()`, then the **location loop**:
   * `g_location == 0` → `MAINOUT_8ef2()` (MAINOUT_81d0 map load, then the overworld main loop
     MAINOUT_8c54). It returns when the party enters a settlement/dungeon (location changed) or quits
     (location still 0 → leave the loop).
   * `1..0x20` → `TOWN_93c0(arrived_from_outside)` then `TOWN_95ee()` (town main loop).
   * `0x21..0x28` → `select_disk(2)`, `DUNGEON_8ffe(flag)` (dungeon main loop inside).
   * after each: select_disk(1), wait for BRIT.DAT, reload `g_monster_table` (0x100 bytes) from
     BRIT.OOL / UNDER.OOL (`OUTSUBS_a5f8` picks by g_party_z); in the Underworld overworld also
     rewrite UNDER.OOL on disk 5.
6. On quit: `restore_video_mode()` and return → `exit`.

So there is no main loop in ROOT: the three loops are MAINOUT_8c54, TOWN_95ee and DUNGEON (part of
8ffe); all three call `read_command_key` → `dispatch_command` (part 2).

## Subsystems in this range

| Range | Subsystem |
|---|---|
| 5dfe–5e4a | Light: `tile_transparent(tile, dist_sq)`; **`rebuild_light_map`** (static light sources → `g_light_map` 0xad14 via `flood_visibility`, radius 10 = squared distance). |
| 5f86–6360 | **Combat entry**: `run_encounter(flags, slot/watch, hours)` (saves location/party/monster table, picks CMDS_bf80 camp / DNGLOOK_b40e room / `setup_encounter`, runs `COMBAT_ae24`, restores); `load_combat_map(index)` (BRIT.CBT, 0x160 bytes at 0xad14 + position tables 0x1704/0x1714/0x1724/0x172c); `fight_monster(slot)` (creature name, terrain → combat map table, SJOG_dfbe aftermath); `camp_outdoors(watch, hours)`. |
| 637e | `draw_game_frame` – the static screen frame (map view 7..184, status box 191..312). |
| 6506–6e60 | **Combat roster** (0xba14, RAM, 32×8): `add_combatant(type/member, mode, x, y, z)`, `init_combat_party`, `setup_encounter(flags, type)` (group size, 1/9 companion creature from 0x16d4, '*** CONFLICT ***'), `apply_ring_effects`, `wake_combatant`, `incapacitate_combatant`, `sleep_combatant_unless_poisoned`, `combat_pos_in_bounds`, `compute_member_defense` (Ghidra lost its body – see listing), `remove_equipped_item(member, item)`. |
| 6f1e–6f90 | `str_find_ci(needle, haystack)` – the keyword matcher used by TALK/CAST2/CMDS (quirky skip-ahead), `str_len_es`. |
| 6f9e–6fd6, 71aa | Driver entry **36** (tile-set patching: 0 save / 1 restore two tile rows, 2 draw the clock hands for hour:minute, 3/4 plane swaps) and entry **32 CF=0** (`driver_animate_tile_bitmaps`: per-frame noise in the fire sprites 0x1e8.. and row rotation of water tiles). |
| 6ff0 | `view_dist_sq(a,b)` – quadrant table 0x6aa8. |
| 7040–70a6 | **Rotating light beam** (lighthouse): `draw_light_beam_frame(frame,on,x,y)` from the 16×16 (dx,dy) pattern at 0x1f7e; `animate_light_beam` (night only, two sources 0x217e/0x2180 and 0x2182/0x2184, 3 lit orientations sweeping one step per frame, frame byte 0x2186). |
| 71ca–7200 | `scroll_rect_up/down(x1,y1,x2,y2,rows)` via driver 13 (earthquake). |
| 7234–7296 | `dos_read_region(name,buf,count,offset)`, `dos_write_file(name,buf,count)` – the INT 21h layer under load/save_file_region (error → `g_last_dos_error`, `g_error_hook`, return 0). |

## Key data structures

* **Combat roster `g_combat_roster` 0xba14 (RAM, 32 × 8)**: +0 attack (monster stat byte 4), +1 initiative
  (dexterity, or stat0 ± 3), **+2 flags: 0x80 party member, 0x40 monster, 0x20 inactive, 0x10 invisible,
  0x08 asleep, 0x01 charmed**, +3 party index / creature type, +4 monster-table slot, +5 turn counter
  (36 − initiative), +6/+7 x,y. Party members occupy 0..5, monsters start at 6. During combat
  `g_monster_table` holds the combatants (slot +5 = HP for monsters / member index for party, +7 =
  target roster index or 0xff) and the pre-combat table lives in `g_monster_table_backup` 0xa9fc.
* **RAM bytes after the roster**: 0xbb14/0xbb15 dungeon wall/floor tiles (DUNGEON_8ffe), 0xbb16
  `g_tiles_patched`, 0xbb17 dungeon flags, 0xbb18 (INTRO).
* **Save fields met here**: 0x5894 `g_location_backup` (0x2EE), 0x58a0 `g_combat_exit` (0x2FA; 'M' →
  ENDGAME), 0x58a1 `g_combat_flags` (0x2FB: 0 fight, 2 room, 4 camp, 6 dungeon camp), 0x58a2
  `g_combat_state_flags` (0x2FC), 0x58a3 `g_no_more_enemies` (0x2FD), 0x57b5 `g_sceptre` (0x20F),
  0x5959 (0x3B3, halves encounter sizes). Character record: +0xa class, +0xb status, +0xd dex,
  +0x19 helmet, +0x1a armour, +0x1b/+0x1c hands, +0x1d ring, +0x1e amulet (items 42/44 rings, 15 Mystic
  Armour, 19/35/39 consumed weapons).
* **DATA.OVL tables (DGROUP = file − 0x10)**: 0x13bd `g_monster_stats[48][8]` (+0 initiative, +4 HP,
  +5 group size), 0x1634 `g_item_defense`, 0x16d4 `g_monster_alt_type[48]`, 0x18b6 creature-name
  pointers, 0x1f7e `g_light_beam_pattern`, 0x217e–0x2186 beam variables (DATA.OVL bytes used as RAM),
  0x6a86 opaque tiles, 0x6a9a light-source tiles, 0x6aa8 `g_view_dist_sq`, 0x6aa4/0x6aa6 str_find
  scratch, strings 0x1393 BRIT.DAT / 0x139c UNDER.DAT / 0xa3f0 BRIT.CBT / 0xa3fa PIRATES / 0xa406
  Sceptre / 0xa422 ring / 0xa438 CONFLICT.
* **Combat map in memory**: BRIT.CBT map n at file n·0x160 → 0xad14 (11 rows × 32; row tails hold the
  party positions (row 3) and monster x/y (rows 6/7) that `load_combat_map` copies to 0x1704..0x1731).
  Refresh/draw code (part 3) reads it from 0xad14 while `g_location == 0xff`.
* **File-loading order at start**: driver (.DRV) → fonts IBM.CH, RUNES.CH → title pictures → SAVED.GAM
  (0x1060 bytes to 0x55a6) → SAVED.OOL (0x200 to 0xb21e, split into BRIT.OOL/UNDER.OOL) → map by the
  overlay (BRIT.DAT chunks / TOWNE.DAT…) → per encounter BRIT.CBT.

## PLINK86 overlay manager (172e / 176c / 1778) – not ported

* 1778:0000: 24 descriptors × 16 bytes: +0/+2 (entry 0: relocation delta after init), +6 flags
  (0x8000 resident, 0x4000 preload, low 13 bits = parent overlay number, 0xffff terminator), +8
  relocation-record count, +0xa load segment, +0xc end segment, +0xe file-name offset; the next entry's
  +0/+2 double as the file offset of this overlay (0 — one file each). Names "TOWN.OVL"… follow at
  0x188+. Then the vectors: `CALLF 172e:02ec ; dw ovl#+1 ; JMPF 1000:target` (12 bytes).
* `plink_ovl_load` (02ec) saves all registers, reads the overlay number word after the return
  address (and skips it), calls 0297: for the overlay and each ancestor (flags & 0x1fff) not yet
  resident: 0035 evicts any resident overlay whose [load,end) overlaps, 01c1 loads it (hook INT 24h,
  open file – on failure calls the DGROUP error hook 0x5394 and retries, seek past the relocation
  header, read in 0xfff0-byte pieces into the load segment, then read the (off,seg) relocation records
  32 at a time into 176c:0004 and add the load delta), sets 0x8000. Fatal errors print 176c:00a0 and
  `exit(0xff)`. Then RETF to the vector, which jumps to the target. For the Rust port overlays are
  ordinary modules: every `vec_X_yyyy` call is a direct call.
* 176c: 0002 current file handle, 008c handle-open flag, 008e/0090 name ptr / AH flag, 0092 name buffer.

## Things the Rust port must get right

1. The startup order above, in particular that `INTRO_8b46` (not ROOT) owns driver/font/save loading
   and that `main` reloads the .OOL object list after every town/dungeon visit.
2. `run_encounter`'s save/restore discipline (location 0xff, monster-table backup, active member reset
   when dead/asleep) and the flag meanings 0/2/4/6; `fight_monster`'s terrain → combat-map table.
3. Roster layout and flag bits; `add_combatant` slot allocation order (roster 6.. for monsters, first
   free monster-table slot, stale-target reset); `setup_encounter`'s RNG consumption order (shuffle only
   when camping, 1/9 companion roll only for i ≤ n/4) and its 16-entry position array overrun for
   groups > 16 (decide on a guard).
4. `str_find_ci`'s skip-ahead quirk (keyword matching in conversations depends on it).
5. Light: `rebuild_light_map` + `animate_light_beam` write into the same 0xad14 map that the combat map
   reuses; the beam runs only while `g_light_level < 0x32` and marks `g_view_dirty` every frame.
6. Driver entry 36/32 effects are cosmetic (clock hands, animated water/fire bitmaps, swapped tiles) but
   `driver_tile_patch(0/1)` must be paired or the two tiles stay altered.
7. `compute_member_defense`: Doom (0x28) rule and the +3 Protection bonus; the decompiled C is wrong.
