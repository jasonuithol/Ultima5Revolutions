# NPC.OVL — settlement NPC schedules and movement

Level-2 overlay (load segment 1a29, 4912 bytes, 1000:a290–1000:b5b9). 26 Ghidra "functions", of
which eight (b064, b136, b224, b32e, b370, b3aa, b49e, b568) are arms of the single loop of
`move_npcs` (b044) and a444 is the else arm of `find_nearest_ladder` (a430). No code-space data
tables. Rust: `rust/src/ovl/npc.rs`. Records: `analysis/ovl_NPC.jsonl`.

## Entry points and callers
| vector | target | name | called by |
|---|---|---|---|
| (direct) | a290 | `load_npc_schedules()` | TOWN `enter_town` (arriving from outside) |
| (direct) | a366 | `init_npc_states(hour)` | TOWN `enter_town`, right after a290 |
| 1778:0339 | b044 | `move_npcs(hour)` | TOWN `town_main_loop` (every turn, result < 2), CMDS (sleeping at an inn: 16 turns) |
| 1778:03b6 | b570 | `schedule_index_for_hour(hour, npc)` | TOWN `init_town_npcs`, TALK `talk_to_npc` (shop open = entry 1) |

Calls out: TOWN `place_npc(z, y, x, npc)` (vector 1778:03aa); ROOT get_map_tile, find_object_at_rev,
rand_range, rand_0_to, load_file_region.

## Data structures
- `.NPC` file: per settlement 0x240 bytes = 32 × 16-byte schedules (3 AI types, 3 x, 3 y, 3 z,
  4 hours, pad) → DGROUP 0x5d5e, 32 types → 0x659e, 32 dialogue indices → NPC state +0xa.
  File chosen by (location-1)/8: towne/dwelling/castle/keep.npc (names at 0x6d46/50/5e/6a).
- NPC state record (0x5f5e + 16·npc): +0 state, +2 x, +4 y, +6 z, +8 type, +0xa dialogue index
  (0 none, <0x80 TLK entry, 0x81..0x88 shop type+0x81, 0xfd frightened, 0xfe Shadowlord, 0xff
  guard), +0xc monster slot, +0xe schedule entry (0..2).
  States: 0 inactive · 1 idle at post · 2 walking a move list to the target · 3 walking to a
  ladder · 4/5 arriving on the party's floor from above/below · 6/7 must leave upwards/downwards
  · 8 NPC and target both off the party's floor (teleport).
- Move lists 0x615e (32 × 16 pairs {count, dir 1 E 2 N 3 W 4 S}), read offsets 0x655e (u16[32],
  0xffff = none), stuck counters 0x65c2 (i16[32]). Note: these walking directions differ from
  the keyboard codes (1 W 2 E 3 N 4 S).
- Pathing grid: 32×32 bytes at `*g_ool_buffer_ptr` (0xb11c): 0 free, 0x90 blocked, 5 goal,
  0x46 start; after the BFS every visited cell holds (direction that reached it) << 4.
- Walk bitmap 0x367e (inside g_npc_walk_bitmaps 0x3676): bit set = tile blocked for NPCs.
- Contact report to TOWN/CMDS: 0x65be kind ('a' attack / 't' talk-arrest), 0x65bf NPC index.
- g_dx/g_dy (0x5876/0x5878) carry step results and BFS goal coordinates.

## Control flow
`schedule_index_for_hour`: the entry with the smallest (hour − entry_hour) mod 256; the 4th hour
maps back to entry 1. AI types: 0 fixed, 1 wander ≤3 from post, 2 wander, 3 flee when the party is
within 3, 4 guard post (chase when the party is within 3 of the post), 5/7 chase, 6 chase when
within 3 (Shadowlord; 7 is what TOWN's alarm gives people).

`move_npcs` per NPC: schedule change (`check_schedule_change`, an hour byte == hour) sets the
state from the floors involved; otherwise idle NPCs run `run_npc_ai`. Walking states follow the
list one step per turn (`npc_can_move_to`; == 2 means the step lands on the schedule target →
state 1). Only ONE NPC may run a BFS per turn (`pathed`); afterwards the others only run their AI.
A BFS failure parks the NPC for ~5 turns (stuck counter 200..0xcd). State 3 with no list converts
to 6/7 and returns from the whole function (original quirk). Floor changes: 4/5 path from the
ladder nearest the target to the target, then `place_npc` on that ladder; 6/7 path to the nearest
ladder (0xc8 up / 0xc9 down, stairs 0xc4..0xc7 also accepted) and teleport to the target when
standing on it; 8 teleports immediately.

`npc_move_relative_party` reports contact (distance 1, AI > 3): AI 4/5 → 't' only when the NPC
has a dialogue; 6/7 → 'a'. TOWN's `handle_npc_contact` turns 'a' into an attack / Begone vermin
and 't' into guard dialogue / arrest.

## Port notes
- Kept quirks: `step_dxdy` clamps test the wrong coordinate; BFS bounds allow x or y == 0x20;
  `npc_move_relative_party` scans "directions" 5..7 reading the frame past its 4-entry array
  (modelled explicitly); the early `return` from `move_npcs` in the state-3/no-list case.
- Signed-vs-unsigned floor compares follow the listing (CMP byte + JLE/JGE = signed i8).
- `find_nearest_ladder` returns the BFS result although Ghidra typed it void.
