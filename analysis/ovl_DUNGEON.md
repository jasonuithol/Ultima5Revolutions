# DUNGEON.OVL — first-person dungeon mode (34 functions, 1000:81d0–1000:a11a)

Translated to `rust/src/ovl/dungeon.rs`; per-function records in `ovl_DUNGEON.jsonl`.
Level-1 overlay (shares the 181d region with TOWN / MAINOUT / INTRO, so 81d0, 8bb6 … collide in
the catalog: `TOWN_81d0` inside DUNGEON.c is DUNGEON's own `enter_dungeon_room`).

## Entry points (vectors) and callers

| vector | addr | Rust name | called by |
|---|---|---|---|
| 1778:0296 | 8ffe | `dungeon_main(arrived)` | ROOT main when 0x21 ≤ g_location ≤ 0x28 (arrived = 1 when entering from outside) |
| 1778:04fa | 8304 | `spawn_dungeon_monster(respawn)` | ROOT 3c9a (after a dungeon camp/room, respawn = 0: only reload the sprite) |
| 1778:0476 | 9db0 | `redraw_dungeon_view()` | DNGLOOK a938 |
| 1778:0506 | 9fe0 | `cmd_klimb_dungeon()` | ROOT dispatch_command 'K' |
| 1778:052a | 9f1a | `cmd_attack_dungeon()` | ROOT dispatch_command 'A' |
| 1778:0512 | 9e3a | `dungeon_klimb_ladder(check, dir)` | CAST cd3a (Uus Por / Des Por) |
| 1778:051e | 9ed8 | `exit_dungeon()` | CAST cd3a |
| — | 9c60 | `render_dungeon_view(full)` | SJOG c5c6 (direct, same level region) |

Outgoing vectors: DNGLOOK abca (load DNGn.16/ITEMS.16), b3c0 / b32e(1) (bracket every combat or
room: hide/restore the dungeon state), afce / aefc (prepare encounter), b26a (after a room),
b40e(3, cell) and aad4(room) (room setup / teardown), COMBAT ae24 (room fight loop), ENDGAME a8d8
(combat exit 'M'), BLCKTHRN resurrection_scene (party dead). ROOT: run_encounter, dispatch_command,
find_first_conscious_member, pass_time, end_of_turn_party_update, damage_whole_party, text/gfx.

## Control flow

`dungeon_main`: g_dungeon_flags = 2; DNGLOOK abca; `draw_dungeon_frame`; `print_level_and_facing`;
tile set by location−0x20 (1,4,5 → wall 0x4f / floor 0x45 / style 3; 6,7 → 0x4d / 5 / style 2;
else style 1 = cave). If the party stands on a room cell, enter it first. Then DNGLOOK b32e,
`spawn_dungeon_monster`, `redraw_dungeon_view`, and the turn loop:

1. `find_first_conscious_member`: −1 → leave the loop (resurrection afterwards); 1 → "Zzzzzz..."
   and repeat; 0 → continue.
2. Status icon: 'T' (time stop) → no pass_time, monsters frozen; 'Q' (quickness) → pass_time and
   monster movement every other turn; else pass_time(1).
3. `read_dungeon_command` animates the view (page 1 → page 0 copy each frame; footstep sound
   tick) until a key arrives; `dungeon_command` dispatches it (1–4 / Enter / '.' →
   `move_in_dungeon`; ^E exit, ^S sound, ^V version, ^K karma, digits → set_active_player,
   everything else → ROOT dispatch_command, whose 'A'/'K' come back to this overlay).
4. If time passed: `dungeon_turn_events(monsters_move, cell)` — sleepers wake (4/64), the monster
   moves (`move_dungeon_monster`) and attacks on contact (`dungeon_monster_attacks` →
   run_encounter), then the cell effect: room (0xa0/0xf0) → `enter_dungeon_room` (COMBAT ae24
   room loop; a finished room becomes 0xa0), fields 0x80–0x8a (sleep / poison / fire; electric
   0x83 is handled inside `move_in_dungeon` as `electric_field_push`), hidden pits 0x61/0x69
   (`fall_through_pit`, can chain levels; z == 8 drops the party to the Underworld by setting
   g_location = 0), bomb 0x62/0x6a.
5. Leaving (g_location < 0x21): g_dungeon_flags bit 4 set if bit 2 clear (else cleared);
   DNGLOOK b3c0; BLCKTHRN resurrection if everybody died.

Ladders: `cmd_klimb_dungeon` → `dungeon_klimb_ladder(0, ±1)` → `can_use_ladder`; climbing off
level 0 (up) or level 7 (down) → `exit_dungeon` (party x/y from the entrance tables, z = 0
Britannia or 0xff Underworld, g_location = 0). Combat exits 5/6 after an attack do the same.

## Rendering (page 1, then copy_rect page1→page0)

`render_dungeon_view(full)`: draw page 1, clear the right half x 0x60..0xaf (mirrored blits OR
into the page), clip window (0x28,0x2a)–(0x98,0xb7) with g_clip_enabled = 0. No light (no light
spell, no torch) → black square. Otherwise for depth 0..3 along the facing (dx/dy words at
0x24d6/0x24de): `draw_front_wall` (stops on a solid cell; a door at depth 0 enables clipping so
the corridor beyond shows through the frame), `draw_side_wall` left/right for the neighbour cells
(perpendicular offsets, byte tables 0x2f28/0x2f2c), then walking back `draw_cell_contents`
(ladders, chests, fountain + `animate_fountain`, pits, ceiling holes, energy fields via
`draw_energy_field`, the monster via `draw_dungeon_monster`). All images go through
`draw_wall_image(side, flip, index)`: side 1 = mirrored (driver entry 33) right half; index <
0x1f = DNGn.16 wall slice at y 0x0e (x tables 0x2e62); index ≥ 0x1f = ITEMS.16 object, image =
(index+1)/2 − 0x10, depth = bits 1–2, `flip` = driver mode bit 0 (vertical flip — ladders up are
flipped ladder-down images on the ceiling). Caves (style 1) animate a sparkle on 0xc0 walls
(`animate_wall_sparkle`, state kept in the cell's low 3 bits); style 3 blinks the skeleton's eyes;
0xb0 walls with a low nibble show rune signs (`draw_dungeon_sign`, tables 0x2de8/0x2df0/0x2df7/
0x2e03/0x2e10).

## Data

* g_dungeon_map 0x595a (save 0x3B4): 8 levels × 8 × 8; cell = type nibble | parameter (wiki
  DUNGEON.DAT legend: 0 hall, 1/2/3 ladders, 4 chest, 5 fountain, 6 pit/trap, 7 open chest,
  8 field, b wall (low nibble = sign), c alt wall, d secret door, e door, f room). Bit 8 of open
  cells = visible ceiling hole; the sparkle state reuses the low 3 bits of 0xc0 walls.
* Party: g_party_z 0x5895 (level), g_party_x 0x5896, g_party_y 0x5897; g_dungeon_facing 0x6603
  (0 N 1 E 2 S 3 W), g_dungeon_dir_copy 0x6602 (last move / 4 came from below / 5 from above;
  converted to a facing by table 0x2c76 on room entry), g_dungeon_style 0x6604,
  g_dungeon_wall_tile / floor_tile 0xbb14/0xbb15, g_dungeon_flags 0xbb17.
* Wandering monster = monster-table slot 1 (0x5c62 kind ×2, 0x5c64 x, 0x5c65 y, 0x5c66 z,
  0x5c67 type or −1, 0x5c68 anim state, 0x5c69 alt-sprite flag) + slot 2 bytes 0x5c6c/0x5c6d =
  previous position. Kind tables 0x173c (types 14 15 16 17 18 19 1c 1b) and 0x1744 (initial anim).
* Image segments: 0xa9c0 DNGn.16, 0xa9c4 ITEMS.16 (loaded by DNGLOOK abca), 0xa9c6 MONn.16
  (g_raw_file_names[8+kind], loaded here). Footstep sound 0xa9fb / toggle 0x24e7; fountain
  frame 0x2f26.
* Files: DUNGEON.CBT (8 dungeons × 16 rooms × 0x160 bytes → g_light_map 0xad14; dungeon index is
  location−0x21 for Deceit but location−0x22 for the rest — quirk preserved, TODO(verify)).
* Entrance coordinates for `exit_dungeon`: byte tables 0x1e89 (x) / 0x1eb1 (y) indexed by
  location.

## Port notes

* All blits rely on driver semantics in `driver_api.md` §4.1 (mirror ORs and shifts one pixel
  left; flag bit 0 = vertical flip; row clipping by the software clip window only when
  g_clip_enabled). The right-half clear in `render_dungeon_view` is what makes the OR-mirroring
  correct — keep it.
* `play_random_tones(0x4e20, …)`: the original passes a string address + 0xd as the frequency.
* DNGLOOK is still a stub module; `dungeon_main` will panic at `fn_1000_abca` until it is
  translated.
