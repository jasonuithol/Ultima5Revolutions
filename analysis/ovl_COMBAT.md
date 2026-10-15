# COMBAT.OVL — the combat turn loop

Level-2 overlay (load segment 1a29, 7408 bytes, 1000:a290–1000:bf75). 26 Ghidra entries: 23
functions plus three fragments (a2b6 = body of a290; a5f6 and a670 = tails of a4b6) that only
exist because vectors of other overlays (SHOPPES 07ee, LOOKOBJ 079a) or a tail jump land on
those addresses. No code-space data. Rust: `rust/src/ovl/combat.rs`. Records:
`analysis/ovl_COMBAT.jsonl`.

## Entry points and callers
| vector | target | name | called by |
|---|---|---|---|
| 1778:04b2 | ae24 | `combat_main_loop()` | ROOT `run_encounter` (fight/camp/room), DUNGEON room entry |
| 1778:02f6/057e/07b2 | a290 | `combat_cell_free(y, x, tile)` | SJOG move/klimb, CMDS camp, CAST/CAST2 spells, COMSUBS |
| 1778:0602 | b4c6 | `remove_combatant(arg)` | SJOG leave_combat, CMDS escape, CAST, COMSUBS e236/e2d4 |
| 1778:073a | b3aa | `check_room_trigger(y, x)` | SJOG move_combatant, CMDS |
| 1778:076a | bb4a | `poison_attack(attacker, target)` | CAST dee0 |
| — | b49e, b672, b766, b804, bbda, bcec | random cell, stats, hit roll, damage, morale | CAST, CAST2, COMSUBS (near calls into the resident overlay) |

Calls out: SJOG (`move_combatant`, `set_active_player`, `combat_turn_effects`, `print_refusal`,
`combat_check_absorbed`, `combat_klimb`, `chaos_sword_passout`, `count_combatants_by_side`,
`is_cell_blocked_for`, `is_combatant_surrounded`, `cmd_get/jimmy/open/search`), CMDS
(`cmd_yell`, `combat_escape_all`, `cmd_push`), COMSUBS (e274 print combatant name, e2d4 special
action, e4f2 after-hit effects, e6b4 distance, e66a distance metric, ea02 missile animation,
e9b4 random adjacent cell, ebdc spell selection, edd8 charmed attack, ef76 Attack command),
CAST (cd3a cast in combat, d712 Use item), ZSTATS (ec1a Z-stats, f476 Ready).

## Control flow
`combat_main_loop` cycles g_active_combatant 0..31 forever. Each active entry (flags 0x80 party
/ 0x40 monster, not 0x20) counts its turn counter (+5) down; at 0 it is reset to 36 − initiative
and the entry acts. Per-turn globals are cleared first (g_combat_state_flags, g_spell_mode 0x588f,
g_attack_is_spell 0x5890, g_attack_weapon, crosshair); every 10 actions `pass_time(1)`.
`combatant_side` decides who acts: party side → `party_member_turn`, enemies → `monster_turn`.
After the action: interference cleared, `apply_cell_hazard` (lava/swamp/fields), sides recounted
(`count_combatants_by_side`: g_dy party, g_dx enemies). No party left → 0 (map empty) or
"BATTLE IS LOST!" (1); no enemies → "VICTORY!" once; the party then walks off the map
(`leave_combat` removes entries) until the roster is empty.

* `party_member_turn`: command prompt "<name>, armed with …" from g_ool_buffer; keys: moves
  1..4, A(ttack via COMSUBS ef76), C(ast: COMSUBS ebdc + CAST cd3a, or "Absorbed!" under 'N'
  protection / in Castle Blackthorn without the crown), G/J/O/R/S/U through
  `run_party_only_command` ("Can't!" for player-controlled charmed monsters), K(limb), P(ush),
  Y(ell), Z(stats), Esc (escape all), 0–6 active player, space Pass, Ctrl-S sound, 0xfc buffer;
  the rest print refusals. A Chaos-sword wielder ('#' in a hand) gets flag 1 and is run by the AI.
* `monster_turn`: time-stop/quickness/swallowed checks; asleep wakes 1/17; COMSUBS e2d4 special,
  else `monster_choose_and_attack` (target from `find_nearest_enemy`; adjacent melee with
  `attack_hits` → thief food theft or `deal_attack_damage`; in range → `monster_ranged_attack`
  via COMSUBS ea02 missile); fleeing monsters heal 1 HP on 1/4 and re-check `monster_morale`;
  then `monster_move` (teleporters, step toward/away from the target, random walk fallback;
  stepping off the map sets state 0x10 → "escapes!" and removal).
* Attack resolution: `attack_hits(weapon, −spell, attacker, defender)` — hit when
  rand(1..30) ≥ (defender dex − attacker stat + 30)/2, auto-hit for glass/chaos sword, 0x28, and
  spells 0x2a..0x31/0x33; `roll_attack_damage` (weapon value from g_item_attack 0x15fc rolled
  1..n, glass sword 99 and shatters; monster stats+3; minus rand(1..armour)); `deal_attack_damage`
  (poison types 0x204 / poison spell 0x33 → `poison_attack`; type 0x1c / sleep spell 0x34 →
  sleep; negative damage on a party member = absorbed 0x20); `apply_damage` (undead half damage
  unless spell, death → exp (stats+4 >> 2)+1, corpse 0x1f or chest with stats+6 quality/trap,
  type 0x1c respawn, 0x1e leaves tile 0x4c, vanishing types 0x1000, dividing types 0x10; party HP
  at record +0x10, death status 'D' and object tile 0x1e).

## Data structures
* Roster 0xba14, 32 × 8: +0 monster HP, +1 initiative (party dex), +2 flags (0x80 party, 0x40
  monster, 0x20 out, 0x10 room-bound, 0x08 asleep, 0x04 swallowed, 0x02 fleeing / may leave,
  0x01 charmed or chaos-sword), +3 member index / monster type, +4 monster-table slot, +5 turn
  counter, +6 x, +7 y. Party entries 0..5, monsters from 6.
* Monster table 0x5c5a (+0 tile, +1 anim tile, +2 x, +3 y, +4 z, +5 value1, +6 anim state,
  +7 target); g_monster_interference 0x58a8[32].
* Combat map = g_light_map 0xad14, 11×11 with stride 0x20; room triggers inside the same
  buffer: 0xae1f/0xae27 trigger x/y, 0xad1f replacement tile, 0xae3f/0xae47 and 0xae5f/0xae67
  target cells (8 each).
* DATA.OVL tables: g_monster_stats 0x13bd[48][8] (+1 .. +6 used at 0x13be/0x13bf/0x13c0/
  0x13c1/0x13c3; 0x13bc is the byte before), g_monster_flags 0x153c[48] (0x2 thief, 0x8
  immune, 0x10 divides, 0x20 undead, 0x80 strength attacker, 0x204 poisonous, 0x1000 vanishes,
  0x2000 teleports, 0x8000 absorbed by 'N'), range 0x159c[48], missile tile 0x15cc[48],
  g_item_attack 0x15fc, item class 0x169c (8 = strength weapon), names 0x17f6 / 0x1856.
* Save fields: 0x588f g_spell_mode, 0x5890 g_attack_is_spell, 0x5882 turn tick, 0x589d
  g_attack_weapon, 0x589e g_active_combatant, 0x58a0 exit, 0x58a2 state flags, 0x58a3
  no-more-enemies. Strings 0x6d74..0x6f6e (prompt, command names, "VICTORY!" etc.).

## Port notes
* COMSUBS `fn_1000_e6b4` stub return type changed to `u16` (its result is compared in a4b6).
* `party_member_turn`'s key byte is uninitialised in the original on the Chaos-sword path; Rust
  uses 0 (the end-of-turn effects therefore run). `find_nearest_enemy`'s target cell is
  uninitialised when `chaos_sword_passout` supplies the target (Rust 0); `get_combat_stat`
  returns 0 for unknown kinds (uninitialised local). All marked `TODO(verify)`.
* `play_sweep_noise` in the Cast "Absorbed!" branch is called with string addresses (0x6d60,
  0x2648) as numeric arguments, exactly as the binary does; `add_i16_capped` gets 0x270f
  (= 9999) the same way.
* `remove_equipped_item(0x27, attacker)` receives the roster index where a member index is
  expected (original quirk, kept).
