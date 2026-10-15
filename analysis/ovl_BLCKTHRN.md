# BLCKTHRN.OVL — Blackthorn's dungeon and Lord British's resurrection

Level-2 overlay (load segment 1a29, 3184 bytes, 1000:a290–1000:aef5). 13 Ghidra entries: 12
functions plus one fragment (a558). No code-space data; the move scripts and tables are in
DGROUP. Rust: `rust/src/ovl/blckthrn.rs`. Records: `analysis/ovl_BLCKTHRN.jsonl`.

## Entry points and callers
| vector | target | name | called by |
|---|---|---|---|
| 1778:033e | a89e | `blackthorn_dungeon_scene()` | TOWN `guard_arrest` when g_location == 0x12 (Castle Blackthorn) |
| 1778:02de | aba0 | `resurrection_scene()` | TOWN `town_main_loop` and MAINOUT loop when find_first_conscious_member == -1 |

Calls out: TOWN `enter_town(1)` (vector 02c6) at the end of the resurrection, CAST2 eaca (vector
0782, end the light spell) and e7c0(0xff, member) (vector 0776, resurrect/heal a member).
Ghidra's labels NPC_a290 and OUTSUBS_a558 are address collisions with other level-2 overlays.

## Control flow
`blackthorn_dungeon_scene`: count living → "subdued and blindfolded" → pick the first virtue not
yet betrayed (g_shrine_destroyed[0..7] == 0; none → straight to the exit) → fade, footsteps,
MISCMAPS.DAT cell (0xb0 bytes → g_view_objects → g_light_map rows) and MISCMSG.DAT text at 0xb21e
→ members seated (tables 0x1f0a/0x1f42/0x1f48, class tiles 0x1ade via "AMBFDTPRS") → guards
(slots 6/7, script 0x3702), Blackthorn (slot 8, transition 0x178) → `interrogation(living,
virtue)`: up to four `print_interrogation_question` / `response_contains_mantra` rounds; a correct
mantra betrays the shrine (0xff, −5 karma) and, with ≥2 living, still `execute_prisoner(0)`; a
refusal threatens a companion (script 0x36da), later refusals mutilate it (g_light_map[9][5] =
0xea/0xeb/0xe8) and kill it at round 3 (`execute_prisoner(1)`, "sliced in half"); release runs
script 0x369e → exit at (10,7), floor 0xff, keys 0, g_location 0x12.

`resurrection_scene`: wait for disk 1, darkness, empty room, "FORTIS FORTUNA AVENTARI", sparkles
and Lord British (slot 1, transitions 0x5e/0x5f/0x174), KARMA.DAT paragraph by karma/20, each
member resurrected (CAST2 e7c0) with full HP, "Vertigo", karma ≥ 75, party to Castle British
floor 1 (10,10) at 06:00, food ≥ 63, `enter_town(1)` if it died in a settlement.

## Data structures
- Move scripts (byte code, `run_move_script`): 0x369e release, 0x36da threaten, 0x3702 guards
  enter, 0x370e Blackthorn speaks, 0x3716 Blackthorn leaves. Opcodes 0..9 (end, pair, count,
  steps on/off, wait, poke light map, refresh, footsteps, clear slot) and move codes ≥ 0x10
  (slot by bits 2..7: 0x10→6, 0x14→7, 0x18→8, 0x1c→0, 0x20→1; direction by bits 0..1).
- Cut-scene room: g_light_map 0xad14 (11 rows × 32) doubles as the drawn room while g_location is
  0xff; individual cells poked: [7][5] 0xadf9, [9][5] 0xae39, [7][2] 0xadf6, [7][8] 0xadfc, [2][5]
  0xad59.
- Party record 15 (0x5788) receives the executed character with +0x1f = 0x7f; g_shrine_destroyed
  0x58d8 (save 0x332); mantras g_virtue_mantra_ptrs[8..] (0x1f5e); KARMA.DAT offsets 0x1a74;
  sweep tables 0x3720/0x372c/0x3738/0x3744; MISCMSG strings at 0xb21e + offsets.

## Port notes
- `decode_move_code` leaves the slot uninitialised for unknown codes (port: 0).
- `execute_prisoner` picks "the party count" as victim index when fewer than two members live
  (copies record[count]); kept as in the original.
- `response_contains_mantra` uses a 18-byte stack buffer (gm.with_buf).
