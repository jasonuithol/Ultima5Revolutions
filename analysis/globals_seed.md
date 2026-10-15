# DGROUP global map — seed (`globals_seed.jsonl`)

One JSON object per line: `off` (DGROUP offset, DS=0x1f64), `name`, `type`, `size` (bytes), `meaning`,
`source` (`wiki` = Ultima Codex DATA.OVL/SAVED.GAM pages, `code` = ROOT analysis in `symbols.json`,
`file` = read directly from DATA.OVL), `confidence`, optional `alias` (the `symbols.json` name when it differs).
~665 entries. **DGROUP offset = DATA.OVL file offset − 0x10** (16-byte relocation header); every wiki offset was
converted and the string tables were re-counted against the real file.
**Save offset S = DGROUP 0x55a6 + S** (`save_file_region("SAVED.GAM", 0x55a6, 0x1060)`).

## Region map

| DGROUP | DATA.OVL file | Contents |
|---|---|---|
| 0000–0042 | 0010–0052 | CRT null-pointer guard (MS C copyright string; checksum 0x55) |
| 0042–128a | 0052–129a | NUL-separated string tables: armour, weapons, rings, 44 creature names, 48 capitalised creature names, scroll codes, special items, "(0".."(7", shards, quest items, 28 short equipment names, potions, reagents, 48 spell names, 11 class names, 5 health texts, 24 rune syllables, 48 spell syllable codes, 27 town names, 8 dungeons, 8 virtues, 8 mantras, 46 shop names, 47 shopkeepers, 118 .TLK compressed words |
| 128a–13a6 | 129a–13b6 | file-name table (see below) |
| 13a6–1a58 | 13b6–1a68 | numeric tables: UI colour words 13ae–13ba, `g_monster_stats[48][8]` 13bd, monster flags 153c, item attack/defence/range 15fc/1634/1664, `g_monster_alt_type` 16d4, combat start positions 1704–1731, 20 extra equipment names 174c, **one 305-entry name-pointer array 17f6–1a58** (equipment 0..47, creature 48..95, creature caps 96..143, scrolls, special items, short names, potions, reagents, spells, classes) |
| 1a58–1e1a | 1a68–1e2a | wiki "unknown 0x3c2": health-text pointers, per-creature bytes, `g_anim_scripts` 1b18, `g_anim_script_index` 1bc8, rune pointers 1bfc, spell-code pointers 1c30, two 48-byte per-spell tables, a 21-word ramp, a 256-byte per-tile class table 1d1a |
| 1e1a–1f7e | 1e2a–1f8e | map-file start indexes, **location name pointers[40]** 1e3a, location x/y[40] 1e8a/1eb2, moon phases 1eda, virtue/mantra pointers 1f4e, shrine x/y 1f6e/1f76 |
| 1f7e–21ca | 1f8e–21da | `g_light_beam_pattern` (512 B) + beam variables 217e–2186 (DATA.OVL bytes used as RAM), 67 unknown bytes |
| 21ca–244a | 21da–245a | shop tables: shop-name pointers [8 trades][16] 21ca, keeper-name pointers 22ca, towns per trade 23ca |
| 244a–25ea | 245a–25fa | small unknown byte tables, `g_view_dirty` 24e6, **.TLK word pointers[129]** 24e8 |
| 25ea–265a | 25fa–266a | `g_raw_file_names[30]`, TOWNE/DWELLING/CASTLE/KEEP.DAT names + pointers |
| 265a–2de8 | 266a–2df8 | MAINOUT / TOWN / DUNGEON message strings with embedded binary blocks (28c5 zero-filled overlay variables, 2bc0 dungeon-view geometry) |
| 2de8–3105 | 2df8–3115 | dungeon sign tables, energy-field geometry, fountain/cave animation pixels, moongate text, STORY.DAT layout tables (21 screens) |
| 310c–3654 | 311c–3664 | INTRO strings (menu, fonts, Ultima IV transfer, SAVED/INIT file names) |
| 3654–3a32 | 3664–3a42 | stat-label pointers, **NPC walk bitmaps 3676** (8 × 32 B), 0/1 table, sign-frame pointers, Britannia chunk map 3876, file-name strings |
| 3a32–3b2a | 3a42–3b3a | reagent prices/stock [5][8], item prices u16[48], armoury stock [9][8] |
| 3b2a–3e78 | 3b3a–3e88 | SHOPPE.DAT offset tables, shop-string pointer tables, END.DAT layout tables (6 screens), number-word pointers |
| 3e78–4124 | 3e88–4134 | hidden searchable objects: tile/quality/location/z/x/y, 6 × 0x72 |
| 4124–50ea | 4134–50fa | CMDS/CAST/USE/SHRINE/INN/TALK strings with pointer tables (4a9f: 191 ZSTATS/TALK string pointers), words of power 4502, docks 4d76, inn tables 4e6e–4e86 |
| 50ea–5273 | 50fa–5283 | proportional font widths 50ea, space width 5154, creation question offsets 517e, creation picture x/y 51fc/5204, CRT variables |
| 5273–55a6 | 5283–55b6 | MS C runtime + graphics library + text-window/LZW/timing state (all named from code; see `symbols.json`), EGA palette 52df, driver names 5330–5348, scan-code tables 540e/5416, floppy prompt 545f, movement tables 54d4/54f4/5534, status strings |
| **55a6–6606** | 55b6–6616 | **SAVED.GAM image** (0x1060 B): party records 55a8, inventory 57c0, flags/clock/location 5874–58a7, dungeon map 595a, NPC flags/schedules/states/move lists 5b5a–65be, dungeon bytes 6602–6605 |
| 6606–6a08 | 6616–6a18 | RAM: word 6606, `g_map` 6608 (32×32 or 4 overworld chunks) |
| 6a08–6acc | 6a18–6adc | `g_tune_pos`, pow10, `g_missile_pass_bitmap`, tune notes/sequence, dawn light table, opaque/light-source tiles, `g_view_dist_sq` 6aa8 |
| 6acc–6ada | 6adc–6aea | CRT hooks (zeros) |
| 6ada–a524 | 6aea–a534 | overlay string blocks (OUTSUBS, NPC, COMBAT, BLCKTHRN, LOOKOBJ, DNGLOOK, SHOPPES, ENDGAME, TALK, ZSTATS, CAST, tavern gossip), wishing-well words 7242, sign frame rows 7388, sign text 742a, ROOT command names a110, CRT run-time error table a452 |
| a524–bd40 | a534–bd50 | **BSS** (zero in the file, zeroed by crt0): disk state a9bd–a9ce, `g_monster_table_backup` a9fc, view buffers ab02/ac64, `g_light_map` ad14, `g_tile_anim_map` b11e, `g_ool_buffer` b21e, `g_combat_roster` ba14, dungeon RAM bytes bb14–bb18 |
| bd40– | — | end of image; stack (crt0 sets SP=0xbd3e per the ROOT analysis; ARCHITECTURE.md says stack top 0xc540) |

## File-name table and how it is indexed

`g_file_names` (DGROUP 0x128a, file 0x129a, 0x11c bytes) holds 32 NUL-terminated names in this order:
PROPORT.PCS, BRITISH.BIT, TITLE.BIT, TILES.16, ITEMS.16, DNG1.16, DNG2.16, DNG3.16, MON0.16 … MON7.16,
CREATE.16, TEXT.16, STORY1.16 … STORY6.16, END1.16, END2.16, ULTIMA.16, STARTSC.16, ENDSC.16, "" (empty, 0x1392),
BRIT.DAT (0x1393), UNDER.DAT (0x139c).

It is not indexed by position. `g_raw_file_names` (DGROUP 0x25ea, file 0x25fa, 30 words) is an array of near
pointers to the first 29 names plus a final pointer to the empty string, which terminates the scan; `load_file`
compares the requested name case-insensitively with each and treats a match as "raw, not LZW". INTRO patches the
".16" suffixes to ".4" in place for CGA/Hercules. BRIT.DAT/UNDER.DAT at the end are referenced directly by
address (`g_str_brit_dat2`/`g_str_under_dat`). The settlement map files use a separate 4-entry pointer table
`g_map_dat_name_ptrs` (0x2652 → TOWNE/DWELLING/CASTLE/KEEP.DAT at 0x2626) indexed by map type, with the
per-file map start indexes at 0x1e1a–0x1e3a.

The same pointer-table idiom is used everywhere else: `g_name_ptrs` 0x17f6 (305 words: item names by item id
0..47, creature names by type 0..47, caps names, scrolls, special items, short names, potions, reagents, spells,
classes), `g_location_name_ptrs` 0x1e3a (40 words, index = location − 1, NULL for the five locations that have no
caps name), `g_tlk_word_ptrs` 0x24e8 (129 words, .TLK byte 0x80+n → entry n), the shop/keeper name tables 0x21ca /
0x22ca ([trade][slot], the slot being the town's column in `g_shop_towns` 0x23ca), and the dungeon sign texts
0x2e10. All pointer values are DGROUP offsets, i.e. the wiki's "add 0x10" to get a file offset.

## Discrepancies found

* **Wiki offsets off by one or three.** Shard names span 0x29 bytes (wiki 0x28). The NPC walk bitmap is at
  file 0x3686 (DGROUP 0x3676), not 0x3683: the 8 stat-label pointers at 0x3676–0x3686 end there, and the data
  is 8 bitmaps of 32 bytes, not one byte per tile. The "dungeon sprite indexes at 0x4513" fall one byte inside
  the words-of-power pointer table 0x4512; the real 8-byte tile list is at file 0x4522 (DGROUP 0x4512).
  Fountain image-7 pixels are listed at 0x2fe2 but lie at 0x2f02. The wiki's second "0x2e62 unknown" overlaps
  its own energy-field tables; the unknown block actually starts at 0x2e72.
* **Counts.** 48 capitalised creature names (wiki 44), 28 short names (29), 24 rune syllables (26), 27 town
  names (26), 46 shop names, 47 keepers, 118 .TLK words, 6 inn beds (wiki 5).
* **"Index" tables that are strings.** Wiki "weapon strings (add 0x10)" at 0x175c is 20 plain strings (Chain
  Coif … Ankh); the "armor index" 0x1806 and "text index" 0x187a are one contiguous pointer array.
* **The 13 + 22 location-name words are one 40-entry array** with 5 NULLs between them, matching the 40-entry
  x/y coordinate tables.
* **wiki 0x6ab8 "day/night time stamps, 6×6 grid"** is the code's `g_view_dist_sq` (squared view distance per
  quadrant cell, compared with the light level) — same table, different description. Wiki 0x6adc (zeros) =
  CRT hook area 0x6acc; the md's "initialiser tables 0x6ada" is actually the last row of `g_view_dist_sq`.
* **Code names vs file.** `g_monster_stats` 0x13bd (48×8) runs to 0x153d but the wiki monster-flag words start
  at 0x153c — one of the two boundaries is wrong (flags kept, low confidence). `g_moon_phase_table` 0x1ed8 is the
  1-based indexing base; the data is at 0x1eda. `g_str_out_of_memory` is named at 0x53e6 but the string starts
  at 0x53d6 (0x53e6 is its "\r\n$" tail — looks like a file/DGROUP shift slip). `g_flag_ega/tandy/herc`
  (0x52ef/0x52f1/0x52f3) coincide with the tail of the 16-byte EGA palette 0x52df. `g_raw_file_names` is
  described as NULL-pointer terminated but actually ends with a pointer to an empty string.
* **Save game.** Wiki and code agree on every field; 0x2D0/0x2D2 (active monsters/PCs) are reused by ROOT as
  `g_dx/g_dy` scratch outside combat (kept as alias). 0x1060 onward is RAM only (`g_map` at 0x6608).
* **UltimatePyve `data_ovl.py`** copies the wiki offsets (including the wrong ones) and additionally names the
  shop-text strings at 0x7836/0x7bf2/0x7dc4/0x7e44/0x7f58/0x7fc2/0x7ff8/0x8028/0x8046 — all confirmed here
  (DGROUP 0x7826 … 0x8036).
* **Stack.** ROOT part 4 says crt0 sets SP = 0xbd3e and zeroes BSS 0xa524–0xbd3f; ARCHITECTURE.md says
  BSS+stack 0xbd50–0xc53f with the top at 0xc540. Reconcile before fixing the Rust `Dgroup` size.

Entries marked `confidence: low` are unnamed binary blocks (`g_unk_XXXX`) whose contents were only eyeballed, or
wiki claims that could not be verified from the file; `medium` marks pointer tables whose targets were resolved
but whose consumer in code has not been identified.
