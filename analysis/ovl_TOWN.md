# TOWN.OVL — settlement game loop (towns, castles, keeps, dwellings, lighthouses, huts)

Level-1 overlay (load segment 181d, 6256 bytes, 1000:81d0–1000:9a2f). 37 Ghidra "functions", of
which five (88bc, 88c8, 88ea, 890c, 892e) are fragments of `move_party_town`. No code-space data
tables: everything it reads lives in DGROUP. Rust: `rust/src/ovl/town.rs`. Records:
`analysis/ovl_TOWN.jsonl`.

## Entry points (vector targets) and callers
| vector | target | name | called by |
|---|---|---|---|
| 1778:02c6 | 93c0 | `enter_town(arrived)` | ROOT main (g_location 1..0x20), ROOT 47f4 (moongate arrival), `guard_arrest`, BLCKTHRN `resurrection_scene` |
| 1778:02d2 | 95ee | `town_main_loop()` | ROOT main, right after enter_town |
| 1778:036e | 8bb6 | `cmd_attack_town()` | ROOT dispatch_command 'A' |
| 1778:034a | 8d52 | `cmd_klimb_town()` | ROOT dispatch_command 'K' |
| 1778:037a | 81d0 | `is_npc_killed(npc)` | CMDS, SJOG |
| 1778:0386 | 8222 | `set_npc_killed(npc)` | CMDS, SJOG, TALK |
| 1778:0392 | 8280 | `remove_npc(npc)` | CAST, CMDS, SJOG, TALK |
| 1778:039e | 82ee | `find_npc_by_slot(slot)` | CAST, CMDS, SJOG, TALK |
| 1778:031a | 8340 | `toggle_night_tiles()` | CMDS, SHOPPES3 |
| 1778:0356 | 8b28 | `raise_alarm()` | CMDS, SHOPPES3, TALK |
| 1778:030e | 9864 | `init_town_npcs()` | CMDS, SHOPPES3 |
| 1778:03aa | 98f6 | `place_npc(z,y,x,npc)` | NPC overlay |
| (internal) | 92aa | `begone_vermin(npc)` | TALK c29e |

Calls out: NPC a290 (overlay reset) and a366(hour) (schedule evaluation) on entering, NPC
b570(hour,npc) (schedule entry for the hour), NPC b044(hour) (NPC movement each turn), TALK c29e
(NPC reaction/guard dialogue, returns non-zero = arrest), CAST2 f056 (moongate trip), BLCKTHRN
a89e (Blackthorn's dungeon) and aba0 (resurrection by Lord British when everybody is dead).

## Control flow
`enter_town(arrived)`: clear slots/drunkenness and run the NPC overlay reset when arriving from
outside; `load_town_map(arrived)`; `spawn_shadowlord`; remove killed NPCs; status window; announce
the Shadowlord ("An air of ... doth surround thee") and apply its influence.

`town_main_loop` per pass: asleep → "Zzzzzz..."; all dead → BLCKTHRN resurrection; else moongate
check (skipped once after entering, flag 0xa9bc), command loop `read_town_command` →
`dispatch_command` / `handle_number_key` / `move_party_town` / ^E ^K ^S ^V, repeated while the
result is 3. Turn end (result != 0): pass_time(1), night tiles at 20:00/05:00,
`apply_tile_hazards` (trapdoor, swamp, fire), door timer, party slot sync, then — unless 'T', and
every other turn on horseback or with 'Q' — `wander_horses`, NPC b044 (result < 2) and
`handle_npc_contact` (attack / Begone vermin / guard arrest / fight). Returns when g_location
becomes 0 (`move_party_town` "Exit to Britannia/Underworld", or BLCKTHRN changed it).

`move_party_town` (87d0 + fragments): direction, edge-of-map leave prompt, boardable objects,
can_enter_tile on g_view_tiles[5+dy][5+dx], `use_stairs` for the directional stair tiles 0xc4..0xc7.

## Data structures
- Map: g_map 0x6608, 32x32, one floor; floor block = `0x1e19[location] + z` (basement z >= 0x80
  subtracts 0x100) × 1 KB of the file `g_map_dat_name_ptrs[(location-1)/8]` (TOWNE / DWELLING /
  CASTLE / KEEP.DAT).
- NPC records: g_npc_types 0x659e (u8[32]); g_npc_schedules 0x5d5e (16 B: 3 AI, 3 x, 3 y, 3 z,
  3 times, pad); NPC state 0x5f5e (16 B: +0 active, +2 x, +4 y, +6 z, +0xa mode 0xfe/0xfd,
  +0xc monster slot, +0xe schedule index); movement list pointers 0x655e (u16[32]);
  contact from the NPC overlay: 0x65be kind ('a'/'t'), 0x65bf NPC index.
- NPC-killed bitmaps 0x5b56 (u32 per location); NPC special flags 0x28c2 (u32 per location →
  monster value1 = 0xff); location exit coordinates 0x1e89/0x1eb1; Shadowlord spawn y 0x13a5.
- Shadowlord: g_shadowlord_loc 0x58c8, g_shadowlord_present 0x5958 (save 0x3b2), names 0x27dc.
- Harpsichord: frequencies 0x2746 (u16[10]), melody 0x275a (13 bytes), position 0x2767;
  secret wall g_map[13][17] = 0x67b9 (Castle British floor 2).
- Annotation tiles 0x48/0x49 → g_annotation_* (0x58ee/0x590e/0x592e, count 0x594e); light beams
  '*' → g_beam1/2_x/y 0x217e..0x2184.

## Port notes
- `npc_affected_by_shadowlord` indexes g_npc_types with the loop counter (4) — original bug kept.
- `town_main_loop`'s `key` is uninitialised in the original on the asleep/dead paths (port: 0).
- Monster-table value1 (+5) is not cleared by `remove_npc`; `spawn_shadowlord` picks NPC index 31
  when no free NPC exists (overwriting it).
- Exit to DOS goes through ROOT `exit` (std::process::exit).
