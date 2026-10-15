# CMDS.OVL — Board / Fire / Hole-up / Ignite / New order / Xit / Yell / Push / Mix / Klimb, camping

Level-3 overlay (load segment 1bf8, 7440 bytes, 1000:bf80–1000:dc88). 26 functions, no code-space
data (tables are in DGROUP). Rust: `rust/src/ovl/cmds.rs`. Records: `analysis/ovl_CMDS.jsonl`.

## Entry points and callers
| vector | target | name | called by |
|---|---|---|---|
| 1778:08f6 | bf80 | `camp_and_watch(hours, watch, flags)` | ROOT `run_encounter` (camping, flags bit 2 set) |
| 1778:08ae | c4d2 | `sleep_in_bed()` | ROOT `dispatch_command` 'H' on a bed (tile 0xab) |
| 1778:08ba | c776 | `cmd_board()` | `dispatch_command` 'B' |
| 1778:08c6 | ca6a | `cmd_fire()` | `dispatch_command` 'F' |
| 1778:08d2 | cd18 | `cmd_ignite_torch()` | `dispatch_command` 'I' |
| 1778:08de | cd5c | `cmd_new_order()` | `dispatch_command` 'N' |
| 1778:08ea | ce34 | `cmd_xit()` | `dispatch_command` 'X' |
| 1778:05ea | d398 | `cmd_yell()` | `dispatch_command` 'Y' (result returned to the main loop) |
| 1778:058a | d59a | `cmd_push()` | `dispatch_command` 'P', COMBAT a8ce (combat Push) |
| 1778:060e | d76c | `combat_escape_all()` | COMBAT |
| 1778:0902 | da58 | `cmd_mix_reagents()` | `dispatch_command` 'M' |
| 1778:091a | dba0 | `cmd_klimb_outdoors()` | `dispatch_command` 'K' when g_location == 0 |

Calls out: TOWN `find_npc_by_slot` / `set_npc_killed` / `remove_npc` / `toggle_night_tiles` /
`init_town_npcs` / `raise_alarm`, NPC b044 (NPC tick), MAINOUT `is_active_monster_tile` (922c)
and `move_party_by` (8524), COMSUBS f4be (projectile animation, vector 046a), COMBAT a290
(passability, 05f6), b4c6 (remove combatant, 0602), b3aa (room trigger, 073a), DNGLOOK b40e
(dungeon camp setup, 04be), OUTSUBS `apparition_level_up` (a8e8, 07d6), CAST2 e2be (spell
picker, 090e).

## Control flow
* `camp_and_watch`: everybody but the watch sleeps (poisoned members tracked per member), a
  bard on watch plays (sprite 0x5f, animate_wait(0x34)); the hour loop advances 5 minutes per
  pass, with a 1/64 ambush roll each time the hour changes (random seed reset from the clock,
  monster from the 8-entry table 0x1734, `setup_encounter`) and random wandering of the watch.
  Without an ambush: healing (rand 1..63 HP, mana = INT or INT/2) only when
  g_hours_until_heal == 0 and hours >= 6, 25% apparition (OUTSUBS) outside dungeons, then
  g_hours_until_heal = 14. Returns 1 when a fight follows, 0 otherwise.
* `sleep_in_bed`: digit prompt, 16 NPC ticks, members 'S', 10-minute steps until the hour with
  day/night switching and NPC placement; any object on the bed throws the party out; the party
  ends one cell east of the bed.
* Vehicles: `cmd_board` consumes the object on the party cell and copies hull (value1) / skiffs
  (value3) into monster slot 0 (0x5c5f / 0x5c61); `cmd_xit` recreates the vehicle object with
  `find_spawn_slot` and uses `land_nearby` (view cells around (5,5), `is_land_view_cell`) to
  decide between stepping off, taking a skiff (transport + 4) or a carpet.
* Fire: outdoors a broadside (perpendicular to the ship facing) probes 3 cells for a moving
  object and damages its value1; in towns the adjacent cannon's facing (tile & 3) gives the
  direction, doors (0x97..0x99, 0xb8..0xbb) are destroyed, NPCs killed (karma -5), slot 0 =
  the party itself takes `damage_whole_party`.
* Yell: sails toggle on a frigate; otherwise a text line: overworld `yell_word_of_power`
  (adjacent view tiles W/S/E/N matched against g_dungeon_open_tiles / 0xdf / 0x1a; toggles
  g_dungeon_open and the entrance tile, or `meditate_at_shrine` with virtue name + 3 mantras),
  settlement `yell_shadowlord_name` (locations 0x1e..0x20: spawns a Shadowlord object and NPC
  record two cells north).
* Push: `is_pushable_tile`, floor 'D' (0x44) or 'E' (0x45 for 0xb4 cannons), `push_tile` /
  `pull_tile` with `orient_pushed_tile` for directional groups 0x90/0xb4; in combat the roster
  position is used and moved.
* Mix: `select_reagents` (status-window picker returning a bitmask, Esc = 0xffff),
  `ask_mix_count`, recipe check against g_spell_reagents (0x1cc0) → g_mixed_spells or an
  explosion (`trap_effect` on the first conscious member).
* Klimb outdoors: grapple + on foot; mountains (0x0c) cost DEX checks (rand 1..5 damage) then
  `move_party_by(dy, dx)`; peaks (0x0d) impassable.

## Data structures
* Monster table slot 0 = party: +1 anim tile (g_party_vehicle_tile 0x5c5b), +5 hull 0x5c5f,
  +7 skiffs 0x5c61. Vehicle tiles: horse 0x10/0x11 (ridden 0x12/0x13), carpet 0x1b (ridden
  0x14/0x15), on foot 0x1c/0x1d, frigate 0x20..0x27 (0x20 group sails hoisted, 0x24 furled),
  skiff 0x28..0x2b.
* View buffers: g_view_tiles 0xab02 (32-byte rows; N/E/S/W of the party at 0xab87 / 0xaba8 /
  0xabc7 / 0xaba6), g_view_objects 0xac64 (16-byte rows).
* Combat roster 0xba14 (+2 flags, +3 member, +4 slot, +6 x, +7 y); NPC state records 0x5f5e
  (16 bytes: +0 active, +2 x, +4 y, +6 z, +0xc slot) and schedules 0x5d5e (+0..2 AI types,
  +3..5 x, +6..8 y, +9..0xb z); NPC types 0x659e.
* Tables: camp monsters 0x1734[8]; Shadowlord names 0x444a[3]; virtue names 0x1f4e, mantras
  0x1f5e, shrine x/y 0x1f6e/0x1f76; words of power 0x4502, dungeon open tiles 0x4512, dungeon
  entrances g_location_x/y + 0x20; reagent names 0x19d2; spell recipes 0x1cc0; mixed spells
  0x57f0.

## Port notes
* `cmd_yell` / `yell_shadowlord_name` return whatever AX print_string left behind in several
  paths; the port returns 1 (the main loops only distinguish 0, 2 and 3). TODO(verify).
* `cmd_xit` stores an uninitialised local (`local_a`) as value3 of the dropped horse/carpet/
  skiff object; the port stores 0.
* `cmd_push` in combat indexes the monster table with roster +3 (member) — kept literal.
* The Ghidra `local_1a` pointer juggling in `camp_and_watch` is just the watch's roster entry;
  `read_text_line` buffers use `gm.with_buf`.
