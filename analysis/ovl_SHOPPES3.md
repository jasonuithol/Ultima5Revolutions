# SHOPPES3.OVL — shop conversations, part 3 (the inn)

Overlay in the 1000:e1e0–1000:ebb4 range, 8 functions, no code-space data. Rust:
`rust/src/ovl/shoppes3.rs`; records `analysis/ovl_SHOPPES3.jsonl`. Uses SHOPPES' text
machinery (`print_shop_text`, `print_shoppe_dat`, `print_greeting`, `print_farewell`,
`ask_yes_no`, `shadowlord_cheat_gold`) and TOWN's `toggle_night_tiles`, `init_town_npcs`,
`raise_alarm` through vectors.

## Entry point
| vector | target | name | called by |
|---|---|---|---|
| 1778:0a22 | ea94 | `inn_shop(member)` | TALK `talk_to_shopkeeper` type 7 |

Vectors 1778:0566/0986/099e list e1e0 but belong to FONT/CAST2/COMSUBS code at that address.

## Data
* TALK context: 0xb114 inn slot (0..5: the inn's index in the type-7 town list), 0xaafe
  shopkeeper name ('$'), 0xb118 price ('%').
* DGROUP inn tables: 0x4d7e nightly rate per member [slot] (2,3,2,3,2,3; ×10 = monthly rate),
  0x4dc4 rooms [slot] (3,4,3,2,2,2), 0x4e6e SHOPPE.DAT room descriptions [slot], 0x4e7a/0x4e80
  bed x/y [slot]. Strings at 0x4d84–0x50bd.
* Party records 0x55a8 (16 × 32): byte 0x09 gender (0x0b/0x0c), 0x0a class, 0x0b status,
  0x0e INT, 0x0f MP, 0x10 HP, 0x12 max HP, 0x17 months at inn, 0x1f inn location (settlement
  number of the inn where the member stays; 0 = travelling). Guests are records
  `party_count..15` with byte 0x1f == g_location; `count_guests_here` scans all 16.
* Prices: price·(100 − 3·INT)/100 added with a signed 32-bit multiply and a **signed**
  `__aFldiv` (SHOPPES/SHOPPES2 divide unsigned). Rest: rate × party count (price sign-extended);
  leave/pick-up: rate × 10 (high word 0); pick-up multiplies by months (0 counts as 1).

## Control flow (`inn_shop`)
Clears the protection spell and status icon → greeting → Y/N (space = N) → "Pick up, Leave
or Rest?" → key:
* **R** `inn_rest(result, gender, INT)`: room check (guests < rooms, else "no room available",
  −2), room text, Y/N, pay or "Highwaymen! … OUT!". Sleeping: close the open door
  (`set_town_map_tile(door_y, door_x, door_tile)`), party to the bed cell, 'G' → 'S', 12 ×
  pass_time(5), then pass_time(9) per frame until 06:00 with regeneration ticks and the
  day/night toggle at 20:00/05:00, "Morning!", NPCs re-placed; HP = max, MP = INT (A/M) or
  INT/2 (B), poisoned members die, sleepers wake; party x += 1. Returns −1 (ends the visit).
* **L** `inn_leave_companion(result, gender, INT)`: room check, lone Avatar refused
  (SHOPPE.DAT 0x26e8), `select_party_member_no_zero` (Avatar refuses, dead refused with
  0x2723), monthly rate, Y/N; on yes the record is tagged (0x1f = location, 0x17 = 0), the
  following records shift down and it is stored as record 15; party count −1; result 1.
  `g_active_member` is cleared/decremented to keep pointing at the same person.
* **P** `inn_pick_up_companion(result, INT)`: full party / no guests refused; one guest is
  taken directly, otherwise the GUEST REGISTER browser (window 1, frame glyphs 0x10–0x17,
  XOR bar from y = 0x28 in 8-pixel rows, keys 1/3 up, 2/4 down via `prev_guest`/`next_guest`,
  Enter/space accept, Esc "No one"). Price = months × monthly rate; unpaid → 0x275a + alarm
  (−1). The record moves into slot `party_count` (records in between shift up), inn byte
  cleared, HP/MP restored or death by poison; result 1.
* space ends; "Is there anything more I can do for thee?" loops back to the Y/N.
`print_farewell(result)`: 0 nothing bought, 1 thanks, −1 silent.

## Port notes
* Ghidra shows 0x26e8/0x2723/0x275a as DGROUP strings; they are SHOPPE.DAT offsets passed to
  `print_shoppe_dat`.
* The two 32-byte record copies (`local_22`/`local_24`) are plain Rust arrays (never passed
  to callees). Record shifting is done with `copy_record` in the original order and direction.
* `get_cursor_row` is called with a stray argument in the C; the Rust wrapper takes none.
