# ROOT part 3 (1000:3ef0 – 1000:5d0a) — overview

40 functions, all Pascal convention (Ghidra call sites and parameter lists are reversed; the JSONL
protos give the true source order and the `param_N` / `[bp+n]` slot of each argument).
Records: `analysis/root_part3.jsonl` (40 lines).

## Address ranges / subsystems

| Range | Subsystem |
|---|---|
| 3ef0–3f54 | Saturating arithmetic helpers: `add_u8_capped(p,amt,max)`, `add_i16_capped`, `sub_u8_floor(p,amt)`, `sub_i16_floor` (used by ~40 callers for HP, counters, timers). |
| 3f6e–3fb4 | Targeting helpers for CAST/COMSUBS: `view_tile_missile_passable(col,row)` (bitmap 0x6a14), `pixel_to_view_cell(px,py)` → `g_dx/g_dy`. |
| 400c–4080 | `regeneration_ring_tick` (ring item 44 = ',' heals 1 HP with p=1/8), `set_active_player(key)` (digit keys of the main loops). |
| 4102–43ae | Sound: `play_ambient_sounds` (clock tick/tock/chimes, waterfall, fountain, musician's 53-note tune), `play_footstep_sound`, `play_triple_sweep_sound`, `play_tone_sweep(start,end,step,total)`. |
| 4402 | **`get_map_tile(x,y)`** – the single map accessor (70 callers): overworld chunk cache, 32×32 town map, combat map. |
| 44b8–4552 | Animation: `cycle_tile_anim_map` (waterfall/fountain/pendulum/flag/clock remap table 0xb11e), **`animate_monsters`** (per-slot script interpreter driven by DATA.OVL tables 0x1b18/0x1bc8). |
| 4702–48a8 | Moongates: `moonstone_on_current_map`, `update_moongates` (gates rise at night at every buried moonstone, 16-frame grow/shrink), `moongate_travel(n)` (.OOL save/load, TOWN_93c0 / MAINOUT_81d0 reload), `enter_moongate` (dissolve effect; phase → destination; midnight 0:00–0:09 special-cased for the caller). |
| 4988 | `select_target_member` – "Player:" prompt / active member / combat roster member. |
| 4a84–4f3c | Status/UI drawing: `draw_sky_strip` (sun '*' and moon-phase glyphs '0'..'7' of font 1 on text row 0, cols 6–17), `draw_sky_strip_frame`, `draw_bar_left_cap`/`draw_bar_right_cap` (glyphs 2/1 + '>'/'<' outlines), `str_index_of`, `draw_status_icon_bar`/`draw_status_icon` (col 30,row 10), `draw_status_top_bar`, `draw_status_title`, `clear_status_separator`/`draw_status_separator` (text row 7). |
| 4f7c | **`pass_time(minutes)`** – clock, calendar (28-day months, 13 months), Shadowlord daily moves, inn months, light level, chimes, sky strip. |
| 51a0–5d0a | **The map view pipeline**: `random_sitting_frame`, `place_object_in_view`, `place_objects_in_view`, `combatant_side`, `draw_map_view`, **`refresh_map_view`**, `flood_visibility`, `build_lit_view`. |

## The per-frame view pipeline (`refresh_map_view`, 5910)

Called from `read_command_key` on every poll, so the map animates while idle.

1. `g_disk_drive_set = 0xff`; time stop ('T' icon) forces `g_animate_flag = 0`.
2. If `g_map_update_enabled` and animating: `animate_monsters` (unless flag == 0xff), `random_wind_change`,
   outside combat `update_moongates` and ROOT 70a6.
3. Terrain layer `g_view_tiles` (0xab02, 11 rows × stride 0x20):
   * not dirty → only cells that are 0 (object cells of the previous frame) are refilled from the map;
   * dirty → `build_lit_view(g_light_level, px−chunk_x, py−chunk_y, 11)`: fill 0xff, BFS
     `flood_visibility` from (5,5): cells with `view_dist_sq ≤ light` are visible; beyond that only cells lit
     in `g_light_map` (0xad14, built by part-4 `rebuild_light_map` from the light-source tiles
     dc bd be b2 de bf b0 b1 b3 bc) with a clear path; the flood passes only through transparent tiles
     (not in `g_opaque_tiles` 0x6a86; windows/arrow slits/doors-with-window only when adjacent);
     unreached/dark cells end as 0xff.
   * combat: 11×32 bytes copied from the combat map at 0xad14.
4. `place_objects_in_view`: party written into slot 0 (`g_transport_tile`), 0xdd/0x1c desert swap by
   distance, then slots 31..0 → `place_object_in_view(col,row,x,y,tile)` with the furniture rules
   (chairs → occupied/eating chairs 0x130–0x13b, beds → 0x11a, stocks/manacles → 0x160–0x167, mirrors
   → 0x13c–0x13f and a reflection 0x9e written one row up, ladders → 0x117/0x118, creatures hidden under
   the standard 0xec, in tropical forest 0x0a and under bridges 0x6a/0x6b for sea monsters/skiffs,
   creatures on crenellation 0x57 shown as a gargoyle statue 0x38, anything under an archway 0x87 or in
   darkness 0xff hidden). Object cells get `g_view_tiles = 0`, sprite in `g_view_objects` (0xac64, stride 0x10).
5. `draw_map_view`: view 0 → `draw_tile(0x100|object)` (0x16 = leave untouched), 0xdc with
   1 ≤ `g_moongate_height` ≤ 15 → `draw_effect_frame`, else `draw_tile(g_tile_anim_map[tile])`.
   In combat: blinking 2-px box around the active party-side combatant and the crosshair.
6. `play_ambient_sounds` when animating; `g_animate_flag = 1`.

## Key data structures (DGROUP offsets)

* **Map buffer `g_map` 0x6608 (0x400 bytes, RAM 0x1062 after the save image)**: towns/dungeon levels are
  a row-major 32×32 grid; the overworld keeps 4 cached 16×16 chunks (TL 0x6608, TR 0x6708, BL 0x6808,
  BR 0x6908, each row-major) whose top-left world coordinate is `g_chunk_x/y` (0x589b/0x589c, save
  0x2F5/0x2F6). `get_map_tile` masks coordinates with 0x1f, so the cache window wraps. Out-of-range town
  coordinates return 0x6a07 (map[31][31]). Directly after: 0x6a08 `g_tune_pos` (u16), 0x6a0a pow10.
* **View buffers (RAM)**: 0xab02 `g_view_tiles[11][32]`, 0xac64 `g_view_objects[11][16]`,
  0xad14 `g_light_map[32][32]` (towns: 0 unlit / tile lit; combat: the combat map).
  0xb11e `g_tile_anim_map[256]` (DATA.OVL has zeros – must be initialised to identity elsewhere).
* **Monster table `g_monster_table` 0x5c5a (save 0x6B4, 32×8)**: +0 tile, +1 anim frame tile, +2 x, +3 y,
  +4 z, +5 value1, **+6 anim state (hi nibble = script position, lo nibble = delay, 0xf = frozen)**,
  +7 value3. Slot 0 is the party (filled from `g_party_x/y/z` and `g_transport_tile` each frame).
* **Combat roster 0xba14 (8 bytes/entry)**: +2 flags (0x80 party member, 0x20 inactive, 0x01 charmed),
  +3 party index / monster type, +6 x, +7 y. `combatant_side` returns 1 for enemies; the traitor Saduj
  is detected by name[4] == 'j'.
* **Save-game fields met here** (DGROUP = 0x55a6 + save offset): 0x5830/0x5838/0x5840/0x5848 moonstone
  x/y/location(0xff = carried)/z; 0x5858 harvest days; 0x57b2 skull-key day; 0x55bf record+0x17 months at
  inn; 0x5880 prev hour; 0x5881 minute; 0x5884 clock chimes pending; 0x5885/0x5886 Trammel/Felucca
  phase chars '0'..'7'; 0x5887 moongate height 0..16; 0x588c hours until heal; 0x5898–0x589a
  crosshair visible/x/y; 0x589f focus blink; 0x58a5 light level (2 night … 0x32 full, 0x12 light
  spell, 10 torch); 0x58a6/0x58a7 light-spell/torch minutes; 0x58c8[3] Shadowlord towns; 0x5959 unknown
  byte reset monthly.
* **DATA.OVL tables (DGROUP = file − 0x10)**: 0x1b18 `g_anim_scripts[11][16]`, 0x1bc8
  `g_anim_script_index[51]`, 0x1ed8 `g_moon_phase_table[29][2]`, 0x6a14 `g_missile_pass_bitmap[32]`,
  0x6a36 `g_tune_notes[9]`, 0x6a48 `g_tune_sequence[53]`, 0x6a80 `g_dawn_light_table[6]` (2,5,10,20,34,49),
  0x6a86 `g_opaque_tiles[19]`, 0x6a9a light-source tiles[10], 0x6aa8 `g_view_dist_sq[6][6]`
  (x²+y² quadrant, mirrored by 6ff0). Counters living in DATA.OVL: 0x6a34 ambient frame, 0x6a7e anim frame.
* **UI colours**: 0x13b0 `g_ui_color1` (outline), 0x13b2 `g_ui_color2` (fill), 0x13b8 sun, 0x13ba moon.
  Status window pixel layout: x 192..311; top bar y 0..7; party rows y 8+8i; separator y 56..63;
  icon bar x 240..263, y 80..87 (text col 30, row 10). Sky strip: text row 0, caps at cols 5 and 18.

## Things the Rust port must get right

1. `get_map_tile` addressing (chunk order, & 0x1f wrap, 0x6a07 filler, combat map at 0xad14) – every
   movement/NPC routine in the overlays relies on it.
2. `flood_visibility` exactly: neighbour order W,SW,S,SE,E,NE,N,NW, the "visited" markers (0xff / 0),
   the opaque-beyond-radius rule, and `tile_transparent`'s adjacent-only tiles; then the 0 → 0xff pass.
3. The object-placement substitutions and hiding rules (they are visible gameplay: gargoyle statues,
   jungle, bridges, mirrors, eating NPCs) and the slot order 31→0 so the party wins.
4. `animate_monsters` script interpreter and its RNG consumption (rand_range(0,255) gates, 50 %
   animate chance) – it affects `g_rand_seed` every frame, so combat/encounter randomness depends on it.
5. `pass_time`: minute/hour/day/month/year rollover, Shadowlord relocation loop, light-level table and
   `g_view_dirty`, chimes = hour mod 12, and the Quickness/Time-stop icons.
6. Moongates: height 0..16, day/night rule (5 ≤ hour < 20 shrinks), phase split at noon, `.OOL`
   round-trip in `moongate_travel`, 0x16 blank sprite trick in `enter_moongate`.
7. Ambient sounds use the frame counters at 0x6a34/0x6a7e – keep them in DGROUP (they are in DATA.OVL).
