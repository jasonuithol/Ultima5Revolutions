# SHOPPES.OVL — shop conversations, part 1

Overlay in the 1000:a290–1000:b9b4 range (33 Ghidra entries = 25 functions + 8 fragments/tails of
the sell browser). No code-space data tables: every table lives in DATA.OVL (DGROUP). Rust:
`rust/src/ovl/shoppes.rs`; records `analysis/ovl_SHOPPES.jsonl`. SHOPPES2/SHOPPES3 hold the
remaining shop types (inn, tavern, ship seller, ...) and reuse the helpers below through vectors.

## Entry points and callers
| vector | target | name | called by |
|---|---|---|---|
| 1778:0842 | b542 | `arms_shop(member)` | TALK `talk_to_shopkeeper` type 0 |
| 1778:087e | aa4e | `horse_shop(member)` | TALK type 2 |
| 1778:084e | a9ee | `reagent_shop(member)` | TALK type 4 |
| 1778:07fa | a732 | `guild_shop(member)` | TALK type 5 |
| 1778:082a | b788 | `healer_shop(member)` | TALK type 6 (member passed, unused; RET 2) |
| 1778:0866 | a446 | `print_greeting()` | SHOPPES2 e84c/ec9c, SHOPPES3 ea94, local |
| 1778:0872 | a492 | `print_farewell(bought)` | SHOPPES2/3, local |
| 1778:07e2 | a510 | `ask_yes_no()` | SHOPPES3 e252/e48e |
| (direct) | a2b6/a40a/a42a | `print_shop_text`, `print_shoppe_dat`, `shadowlord_cheat_gold` | SHOPPES2/3 and local |

Calls out: ZSTATS `prev_owned` / `next_owned` / `print_item_line` (vectors 1778:0806/0812/081e),
CAST2 `in_mani_corp_resurrect`, ROOT text/input/status helpers, `get_map_tile`, `find_object_at`,
`refresh_map_view`, `add_u8_capped`/`add_i16_capped`/`sub_u8_floor`/`sub_i16_floor`,
`print_gold_line`, `select_party_member_no_zero`, `set_color`/`fill_rect_xor`/`play_sweep_noise`,
`__aFlmul`/`__aFuldiv`.

## Text machinery
* `print_shop_text(p)` expands into 0xb7fa and prints: bytes >= 0x80 are .TLK compressed words
  (pointer table 0x23ea + 2·byte, leading space, trailing space before a plain char); macros
  `#` shop name (0xaafc), `$` shopkeeper (0xaafe), `&` item (0xab00), `*` location (0xac62),
  `%` price 0xb118, `^` quantity 0xb11a, `@` morning/afternoon/evening (g_hour < 12 / < 18).
  Literal runs are copied by temporarily NUL-terminating the source in place.
* `print_shoppe_dat(off)` loads 0x5dc bytes of SHOPPE.DAT from file offset `off` into 0xb21e
  (g_ool_buffer) and expands the first string. Several strings are addressed *inside* a loaded
  block: 0xb6e2 (= block 0x1a67 + 0x4c4, reagent "empty purse" insult) and the sell offers
  0x3cbe[8] (file offsets in block 0xbc0, converted with − 0x59a2 ≡ + (0xb21e − 0xbc0)).
* Random greeting/farewell tables (SHOPPE.DAT offsets, [type][4]): 0x3b2a greetings, 0x3b6a
  "nothing bought", 0x3baa "thanks". `print_farewell` prints only for 0 / 1; −1 (refusal, sell
  session) prints nothing.

## Prices
Common buy adjustment (`apply_intelligence_adjustment`): price += price·(100 − 3·INT)/100 with a
signed 32-bit multiply and an **unsigned** 32-bit divide (INT > 33 gives the original's garbage
low word; kept). Sell offer (`arms_sell_item`): INT·price (16-bit) ·3 /100 + 1. After every
payment `shadowlord_cheat_gold`: when g_save_unk_3b2 (TOWN's g_shadowlord_present) == 0
(Faulinei) a further 1..64 gold vanishes. Tables (DGROUP): 0x3bea guild [slot][4] (keys ×3,
gems ×4, torches ×5), 0x3a32 reagent prices / 0x3a5a quantities [slot][8] (0 = not stocked),
0x3c30 horse [slot], 0x3a82 arms base prices i16[0x30] (0 = not tradeable), 0x3ae2 arms stock
[slot][8] 0xff-terminated, 0x3d86 heal / 0x3d8e cure (u8) / 0x3d96 resurrect (u16) per healer
slot. Healer: Minoc (location 5) cures/heals free; Skara Brae (7) waives fees ≤ 100 for the poor.

## Control flow
* Guild/reagents: greeting → Y/N → menu loop (letters buy; Esc/space/Enter leave; result −1 on
  a failed payment ends the menu) → farewell.
* Stables: needs a free g_monster_table slot and a neighbouring cell (0x3c38/0x3c40 offsets)
  with no object and tile 0x44/0x45/5; the horse is object tile 0x10 at that cell.
* Arms buy: `arms_buy_menu` lists stock (long name 0x17f6 if < 13 chars, else 0x1962) and calls
  `arms_buy_item` (arrows/quarrels fill to 99, others +1).
* Arms sell: `arms_sell_browser` — 4-row list in text window 1 (frame glyphs 0x10–0x17, title
  "Arms"), `draw_sell_list(&cursor,&top)` (ZSTATS helpers, 0xfd highlight, more-indicator
  0x18/0x19/0x12 at (6,6)), keys 1/3 up, 2/4 down, PgUp/PgDn ×4, Home/End, Enter/space sell,
  Esc quit; Ghidra split the key handlers into fragments b34c/b41a/b436/b47a/b4c6/b4ce and
  the tails b4f6 (`finish_sell_browser`) / b532 (`print_nothing_to_sell`), all folded back in.
* Healer: Y/N → C/H/R loop with `select_patient` (0 for a lone member), `ask_healer_payment`,
  `healing_light_effect` (XOR flashes at (0xb7,0xb7) + sweep noises); resurrect calls CAST2
  `in_mani_corp_resurrect(0xff, member)` then sets HP = max HP.

## Port notes
* `draw_sell_list` takes `&mut i16` for the two browser locals instead of DGROUP pointers;
  its return value is uninitialised in the original when no row matches (0 here).
* Up-moves with cursor ≠ top store `prev_owned`'s result even when −1 (original quirk).
* `healer_shop` gained the member parameter (RET 2) and TALK's caller now passes it.
* b4c6 is listed as COMBAT_b4c6 / vector 1778:0602 in the catalog: that belongs to COMBAT's
  code at the same address, not to this overlay.
