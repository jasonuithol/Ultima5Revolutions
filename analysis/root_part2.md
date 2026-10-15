# ROOT part 2 (1000:1ef7 – 1000:3c9a) — overview

58 functions plus the missed 1000:1232. Records: `analysis/root_part2.jsonl` (59 lines).

## Convention correction that applies to EVERY function here

Every function in this range ends in `RET n` — the whole engine was compiled with the **Pascal
calling convention** (MSC `/Gc`), not just the graphics wrappers. Arguments are pushed left to
right, so **Ghidra's `param_1` ([bp+4]) is the LAST source argument**; all Ghidra call sites show
the arguments reversed. The JSONL protos give the true source order and the `[bp+n]` slot of each
parameter. Examples: `FUN_2092(8,1)` = `rand_range(1,8)`; `FUN_2a52(amount,index)` =
`damage_party_member(index,amount)`; `FUN_3a74(slot,...)` = `set_monster_slot(..., slot)`;
`FUN_10e0(row,col,tile)` = `draw_tile(tile,col,row)`. Only the no-argument functions (`RET`)
and the register-called assembly helpers (1ef7, 1f77) are exceptions.

## Address ranges / subsystems

| Range | Subsystem |
|---|---|
| 1232–1247 | `null_error_hook` (RETF, DATA.OVL initial value of `g_error_hook`), INT 24h handler (sets 0x535c=1, AL=0 ignore), INT 23h handler (IRET). |
| 1ef7–1fa0 | Text-window helpers: `validate_drive_letter`, `get_cursor_col`, `set_text_bg`, `scroll_window_up` (dead), `window_pixel_rect`, `backspace_chars`. |
| 2032–216c | Small utilities: `to_upper`, `get_time_hash` (INT 21h/2Ch → 12-bit seed), `set_random_seed`, **`rand_range`** (the game RNG), `delay_loop`, **`wait_ticks`** (INT 1Ch hook, frame timer), `str_len`. |
| 2192–230e | PC-speaker sound: `play_sweep_noise` (port 61h bit-banging), `play_random_tones` (random pitches via PIT ch.2), `beep`, `start_tone`, `stop_tone`. All gated by `g_sound_enabled` (0xa9ce). |
| 2322–25d8 | Disk/file layer: `disk_error_hook` (the floppy-swap prompt; patched to a no-prompt path in this GOG build), `select_disk`, `load_file_region`, `save_file_region`. |
| 266c | **`read_command_key`** – blocking key read with map animation and keypad/arrow → direction translation. |
| 2726–2aa8 | Party status window: `print_party_member_line`, `print_gold_line`, **`draw_status_window`** (92 callers), `highlight_party_line` (XOR), `damage_party_member`, `damage_whole_party`. |
| 2ae8 | `end_of_turn_party_update` – poison ticks, starvation / meals at 6-12-18 h, turn counter, protection-spell timer, regeneration ring. |
| 2bd4–2c4c | Passability rules: `tile_walkable`, `tile_is_water`, **`can_enter_tile`** (movement classes). |
| 2d7a–2fd0 | Prompts and effects: `select_party_member`, `set_wind_and_draw`, `random_wind_change`, `poison_party_member`, `trap_effect`. |
| 3072 | `earthquake_shake` – page-1 strip scrolling + rumble. |
| 3178 | **`dispatch_command`** – the A..Z command dispatcher used by all three main loops. |
| 3522–35ec | `flash_tile_at`, `flash_monster` (hit feedback), `get_direction`. |
| 368e–3a74 | Monster/object table queries: `find_object_at`, `find_object_at_rev`, `find_slot_for_tile_offscreen` (dead), `find_slot_with_tile_range`, `find_spawn_slot`, `set_town_map_tile`, `find_first_conscious_member`, `set_monster_slot`. |
| 3aae–3b9e | `rand_0_to`, `random_damage_1_30`, `animate_wait`, `read_text_line`, `read_number`. |
| 3c9a | `hole_up_and_camp` (ship repair / camping, with watch selection). |

## Control flow

There is no main loop in this chunk; the loops live in the level-1 overlays (MAINOUT 8c54,
TOWN 95ee, DUNGEON 8894). Their shape, visible from the callees here, is:
`key = read_command_key()` (which calls `refresh_map_view` 5910 every poll, so the map animates
while idle) → `r = dispatch_command(key)` → if `r != 0` advance time, `end_of_turn_party_update`,
monster moves… → `draw_status_window` when `g_status_dirty`. `dispatch_command` prints the command
name into message window 2 and jumps to the overlay implementation (see the record for the full
key table); it returns 0 for "What?"/Quit/Push-in-dungeon (no turn), 1 for a used turn, 2 for a
Talk that returned nonzero. Direction codes everywhere are **1=W 2=E 3=N 4=S** (`get_direction`
writes `g_dx`/`g_dy` at 0x5876/0x5878).

### Timing and sound
* Frame pacing is `wait_ticks(n)` (20fa): temporarily hooks INT 1Ch with a counter at 0x5448 and
  spins; a 1-tick wait is skipped when `g_cpu_speed` ≤ 0xf0 (slow machine). `animate_wait(n)` =
  n × (refresh_map_view + wait_ticks(1)).
* `delay_loop` and the sound routines are busy loops scaled by `g_cpu_speed` (0x5356).
* Sound: `start_tone`/`stop_tone`/`beep` program PIT channel 2 (divisor 0x1234de/freq) and gate
  port 61h; `play_random_tones(step,total,max_freq)` is the generic "hit/damage" noise;
  `play_sweep_noise` toggles the gate by software (PWM-like). `g_sound_enabled` (0xa9ce) gates
  the speaker but not the delays, so timing is identical with sound off.

### RNG
`rand_range(lo,hi)` (2092): `s = (((s + 0x9248) ROR 3) ^ 0x9248) + 0x11` on `g_rand_seed`
(0x5420); result `(s & 0x7fff) % (hi-lo+1) + lo`. Seeded by `set_random_seed(get_time_hash())`.
`play_random_tones` uses the same formula on a private seed 0x545c. 102 callers – must be bit-exact.

## Key data structures (DGROUP offsets)

* **Save game image `g_save_game` 0x55a6, 0x1060 bytes** – `save_file_region("SAVED.GAM", 0x55a6,
  0x1060)` proves that save-file offset S = DGROUP 0x55a6+S (wiki_SAVED_GAM offsets apply directly).
  Members used here: 0x55a8 `g_party[16]` (0x20-byte records: name +0, status +0xb = 0x55b3
  G/P/S/D, hp +0x10 = 0x55b8, ring +0x1d), 0x57a8 food, 0x57aa gold, 0x57ad gems, 0x585b party
  count, 0x5874 year, 0x5876/0x5878 dx/dy scratch, 0x587a status icon, 0x587b active member,
  0x587c transport tile (−0x100; 0x1c foot, 0x10 horse, 0x14 carpet, 0x20–27 frigate, 0x28 skiff),
  0x587d/e/f month/day/hour, 0x5880 previous hour, 0x5881 minute, 0x5883 wind timer, 0x588b turn
  counter, 0x588e protection turns, 0x5891 animate flag, 0x5892 wind dir, **0x5893 location**
  (0 overworld, 1..0x20 town, 0x21..0x28 dungeon, ≥0x80 combat), 0x5895/6/7 z/x/y, 0x589e active
  combatant, 0x58a1 combat flags, 0x58a4 map-update enabled, 0x594f–0x5951 open-door tile/x/y,
  0x5955 sail direction, **0x5c5a `g_monster_table[32][8]`** (tile, anim tile, x, y, z, v1, v2,
  v3; slot 0 = party: 0x5c5b vehicle tile, 0x5c5f hull strength).
* **RAM-only**: 0xab02 view tile buffer (11×11, stride 0x20); 0xa9fc monster-table backup;
  0xba14 combat roster (8 bytes: +2 flags 0x80 = party member, +3 member/char, +4 monster slot,
  +6/+7 x,y); 0xb21e SAVED.OOL buffer (0x200).
* **Flags**: 0x24e6 `g_view_dirty`, 0xa9fa `g_status_dirty`, 0xa9ce `g_sound_enabled`,
  0xa9bd `g_cur_disk`, 0xa9be `g_display_mode` (0 text / 2 graphics), 0xa9c2 floppy count,
  0xa9c8 drive letter per disk, 0x545e disk whose drive is selected.
* **Tables (DATA.OVL, file = DGROUP+0x10)**: 0x5426 delay shift table; 0x54d4 impassable-tile
  bitmap (32 bytes); 0x54f4 movement class per 4 tiles (64 bytes); 0x5534 water-edge/skiff facing
  masks (20 bytes); 0x559e trap distribution; 0x6a0a powers of ten; strings 0x54b6–0x54c8 (status
  window), 0x5554–0x5598 (prompts/traps), 0xa110–0xa386 (command names, camping).
* **UI colours**: 0x13b0 `g_ui_color1` (3, XOR highlight), 0x13b2 `g_ui_color2` (1).
* Text windows: 0 = full screen/wind line (row 23), 1 = party status (right, pixels x≥192),
  2 = message window; the status highlight rect is x 192..311, y 8+8·i..15+8·i.

## Things the Rust port must get right

1. Pascal argument order for every function (see above) – the Ghidra text is misleading everywhere.
2. `rand_range` and the seed hash exactly; `find_spawn_slot` eviction order; `trap_effect` table.
3. `read_command_key` translation (Ctrl-A..D → 0xfb..0xfe, keypad digits → directions/Home/End
   /PgUp/PgDn) and the fact that the map refresh runs inside the key wait.
4. `can_enter_tile` movement classes and the two direction-mask tables (bit = 8 >> facing).
5. The save image is a raw DGROUP copy; keep every global at its original offset and keep the
   RAM-only tables adjacent (0x5c5a monster table is inside the saved range; 0xab02/0xba14 are not).
6. Disk logic can collapse to "retry after error": `load_file_region` loops until the read succeeds
   and `save_file_region` waits for `file_exists` then writes; the floppy prompt is dead code here.
7. `damage_party_member`, `draw_status_window` and `end_of_turn_party_update` side effects (active
   member cleared on death/sleep, meals at hours 6/12/18 only when the hour changed, starvation).
