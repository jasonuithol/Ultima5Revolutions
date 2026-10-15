# SJOG.OVL — Search / Jimmy / Open / Get commands and combat helpers

Level-3 overlay (load segment 1bf8, 8800 bytes, 1000:bf80–1000:e1d3). 37 Ghidra entries: 36
functions plus one fragment (dba0, the "Escape!" tail of `leave_combat`). No code-space data; all
tables are in DGROUP. Rust: `rust/src/ovl/sjog.rs`. Records: `analysis/ovl_SJOG.jsonl`.

## Entry points and callers
| vector | target | name | called by |
|---|---|---|---|
| 1778:06aa | c8dc | `cmd_search()` | ROOT `dispatch_command` 'S' |
| 1778:0692 | ccca | `cmd_jimmy()` | ROOT `dispatch_command` 'J' |
| 1778:069e | d2f4 | `cmd_open()` | ROOT `dispatch_command` 'O' |
| 1778:0686 | d84e | `cmd_get()` | ROOT `dispatch_command` 'G' |
| 1778:0896 | dfbe | `end_combat(slot)` | ROOT `fight_monster` after `run_encounter` |
| 1778:07be | bfec | `select_search_cell_dungeon(facing)` | SJOG itself (DNGLOOK/CAST may use it) |
| 1778:088a | dab4 | `remove_item_from_all_members(item)` | CAST/ZSTATS (item effects) |
| 1778:05ba | dbd6 | `move_combatant(dir, roster)` | COMBAT turn loop |
| 1778:05c6 | defa | `set_active_player(member)` | COMBAT |
| 1778:0596 | df92 | `combat_turn_effects()` | COMBAT per turn |
| 1778:064a | dea6 | `print_refusal(reason, msg)` | COMBAT command handlers |
| 1778:06e6 | de24 | `combat_check_absorbed()` | COMBAT (endgame room) |
| 1778:06f2 | dcea | `combat_klimb()` | COMBAT 'K' |
| 1778:06fe | e14e | `chaos_sword_passout()` | COMBAT |
| 1778:070a | daec | `count_combatants_by_side()` | COMBAT |
| 1778:0722 / 072e | e058 / e0c8 | `is_cell_blocked_for`, `is_combatant_surrounded` | COMBAT AI |

Calls out: TOWN `find_npc_by_slot` / `is_npc_killed` / `set_npc_killed` / `remove_npc` (vectors
039e/037a/0386/0392), DUNGEON 9c60(1) (redraw dungeon view, vector 04d6), ENDGAME a8d8 (vector
04ca), COMBAT a290 (passability, 05f6), b4c6 (remove combatant, 0602), b3aa (room trigger at a
cell, 073a), COMSUBS e274 (print combatant name, 061a), e236 (per-turn status, 08a2).

## Control flow
Each command first tests `g_location`: 0x21..0x28 (dungeons) uses the dungeon-map variants
(`search_dungeon_cell`, `jimmy_dungeon_chest`, `open_dungeon_chest`,
`get_dungeon_chest_contents`) that operate on the party's own cell (Search asks left/right/
ahead/here); everything else asks a direction (`get_direction`, result in g_dx/g_dy), targets
party + (dx,dy), usually asks a member (`select_target_member`, skill checks use DEX at party
record +0xd, Search-for-traps uses INT at +0xe) and then looks for an object in monster-table
slots 1..31 at that cell (z must match unless in combat, g_location >= 0x80) before falling back
to the map tile.

* Search: chest object → trap report; moldy corpse (0x1f) → `search_corpse` (may spawn a
  creature); furniture prefix by tile; tile 0x4e → hidden door; then moonstone dig
  (`dig_up_moonstone`, skipped on 0xdc) → reagent patch (`harvest_reagents`, midnight only) →
  hidden object tables (`search_hidden_object`, 113 entries, found bitmap 0x585c; entries
  0xd/0xe/0xf keys / skull key / glass sword are repeatable).
* Jimmy: keys required; magic doors 0x97/0x98 break keys; stocks/manacles 0x84/0x85 free an NPC
  (AI types = 5, +2 karma, NPC flagged killed); locked doors 0xb9/0xbb → tile-1; chest object →
  `jimmy_chest` (value1 bit 7 = locked, low bits difficulty).
* Open: puts back the timed door, doors 0xb8/0xba open for 4 turns (g_door_* 0x594f..0x5952),
  else `open_chest`: karma -2 in towns, trap (value1 bit 7) via ROOT `trap_effect`, then loot:
  `roll_fixed_chest_loot` (classes 0x4123/0x412b/0x4133) and `roll_random_chest_loot`
  (0x413c/0x416c) spawn objects through `spawn_chest_loot` into free slots scanned from the top.
* Get: object tiles < 0x10, 0x19, 0x1b, 0xb4..0xb7 → `take_item(slot, value1, tile)` which
  updates the inventory (gold/food capped 9999, counts 99) and clears the slot; map tiles:
  plates/food on tables (+1 food, -1 karma), crops 0x2d, torch sconces 0xb0/0xb1.
* Combat helpers: `move_combatant` → `leave_combat` when stepping off the 11x11 field (first
  leaver fixes g_combat_exit; "Stay with ship!" on a frigate); `combat_klimb` handles ladders
  (0xc8/0xc9 up/down exits 5/6, 0x4c ladder tiles); `combat_check_absorbed` sets g_combat_exit
  = 'M' which makes `end_combat` run the ENDGAME overlay. `end_combat` restores the monster table
  from the 0xa9fc backup and destroys the fought slot or captures a pirate ship.

## Data structures
* Monster/object table 0x5c5a: 32 × 8 bytes (+0 tile, +1 anim tile, +2 x, +3 y, +4 z, +5
  value1 = quantity/quality/hull, +6 anim state, +7). Chest objects: tile 1, value1 bit 7
  trapped/locked.
* Dungeon map 0x595a: 8 levels × 8 × 8; cell = z*0x40 + (y&7)*8 + (x&7); high nibble = type
  (4 chest, 6 pit/trapdoor/bomb, 7 open chest, 8 fields, 0xa/0xe/0xf doors, 0xb wall, 0xc
  skeleton/stalactite, 0xd hidden door), bit 3 marks a lit/secondary variant.
* Combat roster 0xba14: 32 × 8 (+2 flags: 0x80 party member, 0x20 out, 0x2c sleeping/fled/dead,
  bit 1 may leave, bit 0 chaos-sword; +3 member, +4 slot, +6 x, +7 y).
* Tables: hidden objects 0x3e78/0x3eea/0x3f5c/0x3fce/0x4040/0x40b2 (113 each); reagent
  patches 0x3e66/0x3e6a/0x3e6e/0x3e72; chest loot 0x4123/0x412b/0x4133 (8) and
  0x413c/0x416c (48); dungeon chest loot 0x41bc/0x41c4/0x41cc (7); moonstones 0x5830/0x5838/
  0x5840/0x5848; potion names 0x419c, scroll names 0x41ac, item names 0x17f6.

## Port notes
* The found-flag pointer of `spawn_chest_loot` is a Rust `&mut i16`.
* `add_i16_capped` is called with the address of the "Missed!" string (0x270f) as the cap — it
  is the number 9999; translated literally.
* `combat_check_absorbed` reads g_view_objects row 1 (0xac74) for a combatant on row 2;
  flagged TODO(verify).
* `select_search_cell_dungeon` exposes the Ghidra junk-argument calls (`jimmy_chest` is called
  with 4 arguments, only the slot is real).
