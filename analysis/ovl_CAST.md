# CAST.OVL — Cast and Use item

Level-3 overlay (8560 bytes, loaded at 1bf8 with SJOG/CMDS/TALK). 35 functions, 1000:bf80–1000:e0e2.
Translated to `rust/src/ovl/cast.rs`; records in `analysis/ovl_CAST.jsonl`.

## Entry points

| vector | addr | function | callers |
|---|---|---|---|
| 1778:06da | cd3a | `cast_spell_command()` | ROOT `dispatch_command` 'C' (returns 1 = turn used, 0 after a Vas Rel Por), COMBAT a8ce (combat 'C') |
| 1778:06c2 | d712 | `use_item_command()` | ROOT `dispatch_command` 'U', COMBAT (combat Use) |

Everything else is local. The three "ROOT 1778" helpers the C shows are really CAST2 vectors:
`FUN_1778_0986(circle)` = CAST2 e1e0 (casting flash/sound per circle), `FUN_1778_09aa()` = CAST2
e27e (pick a party member, negative = cancel), `FUN_1778_090e()` = CAST2 e2be (read the spell by
syllables, returns the index 0..0x2f, -1 none, -2 not a spell). CAST2 e4e6 asks a direction
(sets g_dx/g_dy, 0 = cancel), eaca(n) light-turns, ead8(kind, turns, icon) status effects,
e5a2 heal, e5ea wind change, e64c x-ray, e6a2 summon daemon, e7c0 resurrect, e8cc position,
e948 magic unlock, e99c dispel field. ZSTATS supplies the Items list (snapshot_items_page,
next_owned, draw_list_frame, ready_browser).

## Cast flow (`cast_spell_command`)

1. g_save_unk_2e9 (0x588f) = 1, attack-is-spell flag (0x5890) = 1; caster = `select_target_member`.
2. "Spell name:" → CAST2 e2be → spell index in DATA.OVL order (In Lor 0 … An Tym 0x2f);
   circle = index/6 + 1, stored in 0x588f.
3. Context byte `g_spell_unk_1c90[spell]` (DGROUP 0x1c90, file 0x1ca0): bit 8 overworld, 4
   town/castle/dwelling (loc < 0x21), 2 dungeon, 1 combat (loc ≥ 0x80). Lord British's castle
   (0x12) without the crown and location 0x1d absorb all magic ("Absorbed!").
4. `g_mixed_spells[spell]` (0x57f0) must be > 0 and is decremented; MP (record +0xf) ≥ circle
   (else "M.P. too low!"/Failed) and is reduced; level (record +0x16) ≥ circle else Failed.
5. The per-spell routine runs. Result 1 → "Success!", 0 → "Failed!" + tone, else silent.

| idx | spell | routine |
|---|---|---|
| 0/12 | In Lor / Vas Lor | CAST2 eaca(100 / 255) + flash |
| 1/13/37 | Grav Por / Vas Flam / Xen Corp | `cast_missile_spell` 0x30/0x31/0x32 via COMSUBS attack_with |
| 2,3,4,27 | An Zu, An Nox, Mani, Vas Mani | member picked by e27e; status / HP changes |
| 5 | An Ylem | `cast_an_ylem_dissolve`: rock/wall tiles at the target → 0x44 |
| 6 | An Sanct | `cast_an_sanct_unlock`: doors 0xb9/0xbb → −1, chests (object byte +5 bit 7, dungeon cell 0x4x → 0x7x) |
| 7/41 | An Xen Corp / In Quas Corp | undead / all monsters flee (entry +0 = 1, flags |= 2) |
| 10/24 | Kal Xen / In Bet Xen | `add_combatant` type 0x14/0x15/0x16/0x22 or 4 × 0x1f, allied (flags bit 0) |
| 14,15,16,20 | fields | `cast_field_spell`: dungeon cell ahead = (cell & 8) \| 0x82/0x81/0x80/0x83, or combat weapon 0x35/0x33/0x34/0x36 |
| 17 | In Por | blink along the chosen direction to the last grass tile; combat: random cell |
| 21/22 | Uus/Des Por | DUNGEON dungeon_klimb_ladder(1, ∓1) then exit_dungeon |
| 25/26 | An Ex Por / In Ex Por | doors → 0x97/0x98 ; CAST2 e948 |
| 28/40/44/45 | In Zu / In Nox Hur / In Vas Grav Corp / In Flam Hur | `cast_wind_spell(colour, 1/2/4/3)` |
| 30 | In Vas Por Ylem | earthquake: enemy entries with dex ≤ roll take 1..20 |
| 34/35/38 | An Xen Ex / Rel Xen Bet / In Quas Xen | crosshair (COMSUBS aim_crosshair range 15): charm (flags ^= 1), shrink to type 0x14, clone |
| 36 | Sanct Lor | caster anim tile 0x1d, flags \|= 0x10 |
| 46 | Vas Rel Por | "To phase:" '1'..'8' → ROOT moongate_travel; success returns 0 from the command |
| 47 | An Tym | status 'T' for 10 turns unless a Shadowlord (tile 0xfc) is present |

Combat-roster entry (8 bytes at 0xba14): +0 state, +2 flags (0x80 party, 0x40 monster, 0x20 out,
0x10 invisible, 0x02 fleeing, 0x01 allied), +3 member/type, +4 monster-table slot, +5 bit 7 =
"hit by this wind", +6 x, +7 y. `is_spell_immune_combatant` (type byte 0x2f/0x0e/0x0f) guards
the sleep/charm/fear/shrink effects.

## Wind spells

`cast_wind_spell` → `animate_wind_cone(dir, ri, colour, ys, xs)` → `plot_wind_pixel`. 21 particles
start at the caster's cell edge; energies are a copy of `g_unk_1cf0` (21 words, 0x1cf0) in BSS
0xa9d0; each pass every live particle takes rand 0..15 pixel steps (2-px dot, random tone),
drifting sideways when its energy runs out. On odd pixel rows the cell is looked up
(`pixel_to_view_cell`, `view_tile_missile_passable`); new cells (g_view_tiles byte 0, marked 0xff)
are appended to the caller's 64-entry x/y lists starting at index 1 (index 0 is uninitialised in
the original; the port uses −1). The caller then hits the first live roster entry on each cell
once (bit 7 of +5), applies sleep / poison / rand 0..30 / 99 damage (experience to the caster's
record +0x14) and clears the bits afterwards.

## Use item (`use_item_command`)

Items page 0xb9ee (0x26 entries, built by ZSTATS): 0..7 scrolls (`use_scroll`, g_scrolls 0x5820),
8..15 potions (`use_potion`, g_potions 0x5828; 1/16 chance of orange, 1/16 of a random potion),
0x10 carpet, 0x11 skull key, 0x12 amulet (icon 0x0e), 0x13 crown (0x1c), 0x14 sceptre (clears
force fields 0x70..0x7f around the party, else CAST2 e99c), 0x15..0x1c moonstones
(`bury_moonstone`: g_moonstone_x/y/loc/z at 0x5830/0x5838/0x5840/0x5848), 0x1d..0x1f shards
(`use_gem_shard`), 0x20 spyglass (LOOKOBJ look_telescope at night), 0x21 HMS Cape plans
(g_hms_cape_plans |= 0x80 aboard a ship), 0x22 sextant (CAST2 e8cc at night outdoors), 0x23
pocket watch, 0x24 black badge (icon 0x1d, 0xff turns), 0x25 sandalwood box.
`remove_status_effect_if(icon)` toggles amulet/crown/badge off when already active.

Gem shards: the party must stand at `g_shard_sites` (0x4882: x[3], y[3], location[3], z[3])
with the Shadowlord (object 0xfc) one cell north and `g_save_unk_325` == shard; the NPC is removed
(TOWN find_npc_by_slot/remove_npc), `g_shadowlord_loc[shard]` = 0xff, the shard is consumed
(0x57b6+shard) and byte 0x5bca |= 2/4/8 (0x4892).

## Strings

All text is in DGROUP 0x4592..0x4a99 (`g_str_cast`): "POOF!" 0x459a, "Disarmed!" 0x45a1, …
"Spell name:" 0x4603, "Success!" 0x4656, "Failed!" 0x4660, scroll/potion names 0x466a..0x474c,
moonstone 0x4768.., shards 0x4794..0x4874, items 0x489f..0x4a84.

## Port notes

* Early exits of `cast_spell_command` ("None!", "No effect!", "Absorbed!", "Not here!", "None
  mixed!") return whatever AX the last print/sound routine left; the port returns 1 (turn used).
  `use_item_command`'s return is likewise a leftover register (ignored by both callers).
* `cast_in_quas_xen_clone` returns an uninitialised local when no free slot/entry exists (port: 0)
  and loops forever until a free cell is found.
* Byte comparisons (party z, shard sites) are plain unsigned compares in the asm even where Ghidra
  shows char/byte mixes.
* `animate_wind_cone` takes the two 64-entry stack arrays as `&mut [i16; 64]`.
