# ZSTATS.OVL — Z-stats and Ready

Level-4 overlay (load segment 1e1e, 4880 bytes, 1000:e1e0–1000:f4ee). 22 Ghidra entries = 19
functions + 3 fragments (ecea and ed12 are pieces of the `zstats_command` page loop — FONT's
vector 1778:054e lands on ecea in FONT.OVL; ee32 is the `return 0xff` tail of `hands_state` —
COMSUBS' vector 1778:094a lands there in COMSUBS.OVL). No code-space data. Rust:
`rust/src/ovl/zstats.rs`; records `analysis/ovl_ZSTATS.jsonl`.

## Entry points and callers
| vector | target | name | called by |
|---|---|---|---|
| 1778:06b6 | ec1a | `zstats_command()` | ROOT dispatch_command 'Z', COMBAT party_member_turn |
| 1778:06ce | f476 | `ready_command()` | ROOT dispatch_command 'R', COMBAT party_member_turn |
| 1778:09da | e63e | `draw_list_frame(rows)` | local (SHOPPES-era vector) |
| 1778:0806 | e74c | `prev_owned(member, counts, n, start)` | SHOPPES af10/b34c/b436, local |
| 1778:0812 | e784 | `next_owned(member, counts, n, start)` | SHOPPES af10/b1f4/b34c/b41a, local |
| 1778:081e | e7c2 | `print_item_line(marker, names, counts, idx)` | SHOPPES af10, local |
| 1778:09e6 | eb7a | `snapshot_items_page()` | local |
| 1778:09f2 | f10e | `ready_browser(mode, member, first)` | local |

Internal: e1e0 `select_stats_member`, e262 `show_stats_page`, e458 `print_equipped_slot`, e488
`show_arms_page`, e57c `show_party_equipment_page`, e6f8 `is_equipped`, e8c8 `inventory_page`,
edb6 `finish_stats_screen`, edce `print_ready_refusal`, edea `hands_state`, ee3c `ready_item`.
Calls out: ROOT only (`select_party_member`, `str_index_of`, text windows/fonts/cursor,
`print_number`, status-window drawing helpers 0x4daa–0x4f3c, `remove_equipped_item`,
`rand_range`, `play_tone_sweep`, `read_command_key`).

## Control flow
* **Z-stats** (`zstats_command`): member prompt (`select_stats_member(1)`; '0' → page 12; in
  combat the active member is implied), "\nStatus: ", `snapshot_items_page`, then a page loop
  in text window 1 over the status area. Page index: 2·member = stats page, 2·member+1 = arms
  page, 12 = party Equipment, 13 Reagents, 14 Spells, 15 Items, 16 Armaments. Keys 1/3 previous,
  2/4 next (wrapping 0 ↔ 16 and party_count·2−1 ↔ 12), '0' page 12, '1'..'6' member page,
  space/Esc → `finish_stats_screen` ("Done"). List pages run `inventory_page`, which returns the
  key that ended it (space, Esc, 1, 2, digits); inside it 3/4 scroll by one, PgUp/PgDn (0xd5/
  0xd6) by seven, Home/End (0xd3/0xd4) jump; the status icon (0x18/0x19/0x12) shows where more
  entries are.
* **Ready** (`ready_command`): `select_stats_member(0)`, first owned/equipped item via
  `next_owned(member, g_inventory, 0x30, -1)` (empty-handed message otherwise), "Item: ", list
  frame, `ready_browser('R', member, first)`: a cursor list (0xfd highlight) with the same keys;
  Enter/space → `ready_item(sel, member)`. `ready_item` unequips an equipped item (count back
  into g_inventory, max 99) or equips it after checks: ammunition counters (0x57db for 0x1a/0x24,
  0x57dd for 0x1c), item class table 0x1a7e (ring 2, amulet 4, one-handed 0x20 via
  `hands_state`, two-handed 0x30, armour 0x40, helm 0x80), no armour change in combat while
  enemies remain, and carried weight (0x1aae sum over the six slots + item) ≤ strength. Rings
  0x2a/0x2c vanish 1/16 of the time (return 1 ends the browser); 0x2a (invisibility) in combat
  swaps the member's combat tile to 0x1d and back to 0x1ade[class].
  The browser returns after Esc ("Done") or a vanished ring; `ready_command` restores the status
  window.

## Data structures
* Party records 0x55a8 (32 bytes): +9 gender glyph (0x55b1), +0xa class (0x55b2, index into
  "AMBFDTPRS"), +0xb status (index into "GPDSC"), +0xc str, +0xd dex, +0xe int, +0xf mp,
  +0x10 hp, +0x12 max hp, +0x14 exp, +0x16 level, +0x19..+0x1e helmet/armour/left/right/ring/
  amulet. Inventory counts 0x57c0 (0x30), reagents 0x5850 (8), mixed spells 0x57f0 (0x30),
  scrolls 0x5820, potions 0x5828, quest items 0x57b0..0x57bf, g_party_count 0x585b.
* Items page scratch 0xb9ee (0x26 bytes, built by `snapshot_items_page`; 0xff = owned flag for
  moonstones/plans/boolean items, counts print "--" for 0 and nothing for −1).
* DATA.OVL tables: class names 0x1a44 and indents 0x1a58, status names 0x1a6a, short item
  names 0x1962, item name tables 0x1916 (items) / 0x19b2 (alternate '!' names) / 0x19d2
  (reagents) / 0x19e2 (spells), item class 0x1a7e, weight 0x1aae, class combat tile 0x1ade,
  equipped glyph 0x1ae8. Strings 0x96b4–0x9998.
* Text windows: window 1 redefined to (x 1..0x26/0x27, y 1..0x18 or rows+1) for the pages;
  window 2 is the message window used to read keys. Control chars: 0xff clear, 0xfc/0xfb centre
  on/off, 0xfe, 0xfd highlight, 0x10–0x17 frame glyphs; font 1 holds the item glyphs.

## Port notes
* `ready_item`: an item whose class byte is none of the known values is written through the
  stale loop pointer (record +0x1f) in the original; Rust reproduces that (`TODO(verify)`).
* `ready_browser` leaves the selection uninitialised until a row matches the cursor (Rust 0).
* `select_stats_member` is the same address as FONT/CAST2/COMSUBS e1e0 in other overlays; the
  catalog's VEC callers for it belong to those overlays.
