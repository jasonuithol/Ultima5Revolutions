# SHOPPES2.OVL — shop conversations, part 2 (tavern, shipwright)

Overlay in the 1000:e1e0–1000:ecfc range, 15 functions, no code-space data (every table lives
in DATA.OVL). Rust: `rust/src/ovl/shoppes2.rs`; records `analysis/ovl_SHOPPES2.jsonl`.
Shares SHOPPES' text machinery (`print_shop_text`, `print_shoppe_dat`, `print_greeting`,
`print_farewell`, `shadowlord_cheat_gold`) through vectors 1778:07ee/0836/085a/0866/0872.

## Entry points and callers
| vector | target | name | called by |
|---|---|---|---|
| 1778:09fe | e84c | `tavern_shop(member)` | TALK `talk_to_shopkeeper` type 1 |
| 1778:0a0a | ec9c | `shipwright_shop(member)` | TALK type 3 |

Vectors 1778:0566/0986/099e/09aa list e1e0/e27e but belong to FONT/CAST2/COMSUBS code at the
same addresses. `member` is the talking party member (TALK's `g_active_member` choice); his
intelligence (0x55b6 + 32·member) prices rations and vessels.

## TALK-provided context (DGROUP)
0xb114 shop slot (index of the town within the shop type's town list), 0xaafe shopkeeper name
('$'), 0xab00 person name ('&'), 0xac62 location name ('*'), 0xb118 price ('%'), 0xbcf8 line
input. Prices: `adjust_price_for_intelligence` = SHOPPES' formula (price += price·(100−3·INT)/100,
signed 32-bit multiply, UNSIGNED divide).

## Tavern (`tavern_shop`)
State: 0xbd16 style, 0xbd18 bought, 0xbd1a living members served, 0xbd1c count word, 0xbd1e
meal tile, 0xbd20 rounds served (never reset — persists across visits).
Tables: 0x4d4c style per slot (0,0,0,2,3,1,0,2,0); per-style key letters in four 6-byte rows
0x4c1e "MCBF" meal, 0x4c24 "AWRS" drink, 0x4c2a "R  P" rations (space = none), 0x4c30 "CTHA"
rumour (these rows are what globals.txt calls `g_zstats_key_grid`); 0x4c36 meal price per
head [slot]; 0x4c48 wine prices a–f (18,192,79,30,275,98); 0x4c54 rations price [slot];
0x4c66 seven rations pitches (SHOPPE.DAT offsets); 0x4d56/0x4d5e menu and "anything else"
texts [style]; rumours: 0x4c74 26 keyword pointers, 0x4ca8 person names, 0x4cdc location
index → 0x4cf6 13 location names, 0x4d10 prices, 0x4d44 four reply templates.
Flow: greeting → Y/N → menu text → key loop. Meal: `tally_living_members(price)` then
`serve_round_or_meal` (food += living count, table tile 0x95 above/below the party becomes
0x9b/0x9a). Drink: after three rounds "haven't ye had enough?" ('N' → drunk 25 moves, karma−1);
wine taverns ('W') show the list (space abandons it: SHOPPE.DAT 0x1413, result 2); others
serve a 1-gold-per-head round. Rations: packs of 25 food, "table scraps" when broke and food
< 3. Rumours (only after a purchase): keyword search of the typed line (`str_find_ci`, hit
must start the line or follow a space), "Fair 'nuff?", pay, reply with '&'/'*'.
Result 1 from any handler = "CAN'T PAY" → return without farewell; otherwise "Anything else
for thee?" → 'Y' re-shows the follow-up text, 'N' → `print_farewell(bought)`.

## Shipwright (`shipwright_shop`)
State: 0xbd22 kicked out, 0xbd24 bought. Tables: 0x4d66 frigate / 0x4d6e skiff prices
[slot], 0x4d76/0x4d7a dock x/y [slot] (Jhelom, Minoc, East Britanny, Buccaneer's Den).
`shipwright_menu`: 'F' frigate — only when no vessel is owned (`g_bought_vessel` & 0xc0 == 0):
sets 0x6605 = 0x82; owners get the "dock space is limited … 87,000 gold" joke, whose 'Y'
checks for 10000 gold and always throws the party out (0xbd22 = 1, no farewell). 'S' skiff —
no vessel: 0x40; skiff only: refused (no charge); frigate: 0x6605 += 1 (skiff aboard).
`shipwright_deliver(member, pay, announce)` records g_bought_ship_x/y from the dock table,
pays, and asks "Will there be anything else?" (true = done). Gender test compares the
0x0b/0x0c byte with 'F' (original bug: always "sir").

## Port notes
* `order_meal` (e3b2) is declared void by Ghidra but returns `serve_round_or_meal`'s AX; the
  caller tests it. Ghidra also dropped `add_i16_capped`'s third argument (`&g_food`) and
  `print_string(0x9b16)`'s argument in e2bc — taken from the listing.
* `shipwright_menu` recurses on the "dock space" 'N' answer, as the original does.
* `get_map_tile` is called with Ghidra order (y, x).
