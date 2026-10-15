# COMSUBS.OVL — combat subroutines

Level-4 overlay (load segment 1e1e, 5216 bytes, 1000:e1e0–1000:f63a). 30 Ghidra entries = 23
functions + 7 fragments of the crosshair aiming loop (e7a2, e84c, e856, e860, e8ca, e8d2, e900 —
Ghidra split `aim_crosshair` e6e4 because SHOPPES2's vector 1778:09fe lands on e84c in
SHOPPES2.OVL and because of the jump tangle). No code-space data. Rust: `rust/src/ovl/comsubs.rs`;
records `analysis/ovl_COMSUBS.jsonl`.

## Entry points and callers
| vector | target | name | called by |
|---|---|---|---|
| 1778:099e | e1e0 | `target_evades(kind, target, attacker)` | CAST c3be/c920/c9dc/cc18/dee0, e2d4 |
| — | e236 | `dissipate_fields()` | SJOG dfbe (combat aftermath) |
| 1778:061a | e274 | `print_combatant_name(ri)` | COMBAT, SJOG, CAST c920, local |
| 1778:0626 | e2d4 | `monster_special_action(ri)` | COMBAT monster_turn |
| 1778:0632 | e4f2 | `print_hit_result(attacker, target)` | COMBAT, CAST c89e/dee0, local |
| 1778:0752 | e66a | `cell_distance(y1,x1,y2,x2)` | COMBAT find_nearest_enemy |
| 1778:063e | e6b4 | `roster_distance(a, b)` | COMBAT monster_choose_and_attack, local |
| 1778:09c2 | e6e4 | `aim_crosshair(range, ri)` | CAST c920/c9dc/caa8, ec48, ee32 |
| 1778:09ce | e928 | `find_combatant_at(y, x)` | CAST c920/c9dc/caa8, local |
| 1778:075e | e9b4 | `random_cell_near(y, x)` | COMBAT monster_move, ea02 |
| 1778:0656 | ea02 | `animate_missile(tile, hit, ty, tx, attacker)` | COMBAT monster_ranged_attack, ec48 |
| 1778:0662 | ebdc | `check_interference(ri)` | COMBAT party_member_turn (Cast), ec48 |
| 1778:066e | edd8 | `melee_attack(weapon, target, attacker)` | COMBAT charmed attack, ee32 |
| 1778:094a | ee32 | `attack_with(weapon, ri)` | CAST bfb2/bfcc, ef1c, ef76 |
| 1778:067a | ef76 | `attack_command(weapon_count, ri)` | COMBAT party_member_turn 'A' |
| 1778:046a | f4be | `animate_projectile(kind, ty, tx, fy, fx)` | CMDS c8e2/ca6a, MAINOUT (cannons), ea02, ec48 |

Internal: e2b2 `print_missed`, e638 `dist_squared`, eb5c `use_missile_ammo`, ec48
`ranged_attack`, ef1c `attack_with_equipped`, f006 `trace_line`, f12a `draw_missile`.
Calls out: COMBAT (`get_combat_stat` via the near call to 1778:0746, `remove_combatant`,
`random_combat_cell`, `combat_cell_free`, `check_room_trigger`, `attack_hits`,
`deal_attack_damage`, `monster_morale`), SJOG `remove_item_from_all_members` (1778:088a), ROOT
text/sound/gfx wrappers, `add_combatant`, `remove_equipped_item`, `flash_monster`,
`pixel_to_view_cell`, `view_tile_missile_passable`, `combatant_side`.

## Control flow
* **Attack command**: `attack_command` → bare hands (monster side or no weapon) or, per equipped
  slot (helmet 0x55c1, left 0x55c3, right 0x55c4), `attack_with_equipped` (prints the item name
  + ":" when two weapons are wielded) → `attack_with`: range 0 (melee: g_item_range 0 / monster
  range 1) → "Aim! ", `aim_crosshair(1, ri)`, `find_combatant_at` → `melee_attack`; range > 0 →
  `ranged_attack` → interference check (limited weapons), "Aim! ", `aim_crosshair(range)`, ammo
  (`use_missile_ammo`), `attack_hits`, `animate_missile`, damage + `print_hit_result`; the
  magic axe (0x26) flies back with a second `animate_projectile`.
* **`aim_crosshair`** loop: keys 1–4 / 0xd3–0xd6 move the crosshair inside 0..10 and within
  range of the attacker (`cell_distance`), Esc cancels (returns 0), Enter/'A'/space confirm off
  the attacker's own cell (space on it cancels). Returns the distance of the last move.
* **`animate_missile`**: a miss (hit == 0) scatters the target cell with `random_cell_near`;
  weapons 0x19/0x22 skip the animation; otherwise `animate_projectile` flies the sprite and a
  blocked path lands the missile where it stopped. Fields are dropped for 0x13 (fire 0xea) and
  spell ids 0x33–0x36 (0xe8–0xeb). Returns the struck roster entry unless it is the active
  combatant or (in spell mode) the attack missed.
* **`animate_projectile`**: `copy_page(1,0)`, `trace_line` between cell centres into the path
  buffers, then per 13-pixel step (6 in settlements, 8 for kind 7) `pixel_to_view_cell`,
  `draw_missile`, `delay_loop(0x28,1)` and background restore from page 1; blocked when
  `view_tile_missile_passable` fails off the shooter's cell (returns 0, g_dx/g_dy = cell).
* **`monster_special_action`**: possession (flag 0x40, random party member, `target_evades`),
  invisibility toggle (0x800, 1/8), daemon gating (0x400, 1/8, `add_combatant(.., 0, 0x26)`
  with a 0x16→0xd8 transition).
* **`print_hit_result`** decodes g_combat_state_flags (0x20 grazed, 0x22 silent, 4 slept, 8
  silent, bit 0 killed) and prints morale grades for monsters / "hit!" or "dragged under!" for
  members.

## Data structures
* Roster 0xba14 (32 × 8, see ovl_COMBAT.md), monster table 0x5c5a (+1 anim tile, +7 last
  target), g_monster_interference 0x58a8, crosshair 0x5898/0x5899/0x589a, g_attack_weapon 0x589d,
  g_active_combatant 0x589e, spell mode 0x588f / attack-is-spell 0x5890, g_combat_state_flags
  0x58a2, g_dx/g_dy 0x5876/0x5878.
* DATA.OVL tables: monster flags 0x153c (0x40 possess, 0x400 gate, 0x800 invisible), monster
  range 0x159c, monster missile tile 0x15cc, g_item_attack 0x15fc, g_item_range 0x1664, item
  missile kind 0x169c, names 0x17f6/0x1856, missile shapes 0x2188 (kind 2) / 0x2198 (kinds > 2,
  signed (y,x) pairs), sparkle radii 0x21ba, colours g_ui_color0..4 0x13ae..0x13b6.
* Path buffers 0xa728 (x) / 0xa872 (y), 256+ bytes each, 0xff terminated. Inventory counts
  0x57c0 (ammo 0x57db for 0x1a/0x24, 0x57dd for 0x1c, 0x57d3 flaming oil).
* Strings 0x99a0–0x9aa8 ("Failed!", "missed!", "possessed!", "reappears!", "disappears!",
  "gates in a daemon!", "grazed!", "killed!", "slept!", "dragged under!", "hit!", wound grades,
  "interferes!", "Aim! ", "Nothing!", ":", "Attack-").

## Port notes
* `roster_distance` returns u16 (COMBAT compares it) although Ghidra shows void; `aim_crosshair`
  returns i16 (its callers test the result).
* `animate_missile`: the landed flag is uninitialised in the original when the weapon is 0x19 or
  0x22 and the roll missed; Rust uses 0 (missile scatters, returns -1). `TODO(verify)`.
* `dist_squared` keeps the odd long/int mix; only the low word is used.
* `draw_missile` kinds < 2 never touch the shape tables (Rust initialises the pointers to 0).
