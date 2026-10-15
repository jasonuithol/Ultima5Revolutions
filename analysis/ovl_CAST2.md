# CAST2.OVL — spell effects part 2, shrine scene, save prompt

Level-3 overlay (1000:e1e0–f399) sharing the address range with CAST/ZSTATS/… Translated in
`rust/src/ovl/cast2.rs`; records in `analysis/ovl_CAST2.jsonl`.

## What it implements
1. **Spell helpers used by CAST** (all vector targets):
   | vec | addr | name | role |
   |---|---|---|---|
   | 0986 | e1e0 | `casting_flash(circle)` | flash the view cell + two sweep noises from DATA.OVL tables 0x4af6/0x4b08/0x4b1a/0x4b2c; circle>=9 = item sound |
   | 09aa | e27e | `pick_target_member` | "On who: " + `select_party_member_no_zero`, echoes name / "None!" |
   | 090e | e2be | `read_spell_name` | rune-letter entry (table 0x1b7a[letter] = g_rune_syllable_ptrs), sort, match g_spell_code_ptrs → index / -1 / -2 (CAST cast+use, CMDS mix) |
   | 0962 | e4e6 | `ask_direction` | "Direction-" → g_dx/g_dy, code 1..4 / 0 Pass |
   | 0956 | e5a2 | `heal_member` | Mani |
   | 09b6 | e5ea | `rel_hur_change_wind(kind, dir)` | Rel Hur (tail = e63e/e648 fragments) |
   | 097a | e64c | `wis_an_ylem_xray_view` | full-light view for 20 frames |
   | 0992 | e6a2 | `kal_xen_corp_summon_daemon(from_item)` | add_combatant type 0x26, Int check for the spell |
   | 0776 | e7c0 | `in_mani_corp_resurrect(mode, member)` | also BLCKTHRN (mode 0xff, silent) and SHOPPES healer |
   | 093e | e8cc | `in_wis_print_position` | rune-letter coordinates (also sextant) |
   | 096e | e948 | `in_ex_por_unlock_door` | 0x97→0xb8, 0x98→0xba (also skull key) |
   | 0926 | e99c | `an_grav_dispel_field(flash)` | dungeon cell ahead / combat field slot |
   | 0782 | eaca | `set_light_spell_turns` | |
   | 0932 | ead8 | `start_timed_effect(circle, turns, icon)` | g_status_icon/g_protection_turns |
2. **Shrine cut-scene** `visit_shrine` (f056, vec 02ea; MAINOUT/TOWN after `enter_moongate`, MAINOUT for
   the Codex tile 0x11) with helpers `place_codex_urns` (eaf4), `shrine_quest_dialogue` (eb46+ec1a),
   `codex_shrine_dialogue` (ef04), `shrine_step_pause` (f044).
3. **Quit & Save** `save_game_prompt` (f2de, vec 0a2e; ROOT dispatch_command 'Q').

## Control flow of the shrine scene
save g_location→0xbd15, location=0xff; back up the monster table to 0xa9fc (tiles cleared); load the
11x11 map from MISCMAPS.DAT (0x160 Codex / 0xb0 shrine, 11 rows × 16) into g_view_objects and copy into
g_light_map (stride 32); MISCMSG.DAT (file 0x3ab, 2000 bytes) into 0xb21e; urns (tile 3) for party
records whose byte +0x1f == 0x7f; the party figure (slot 0, tile 0x1c) walks from (5,10) up 4 (shrine)
or 7 (Codex) rows with footstep pauses; dialogue; walk back; restore table + location; pass_time(16);
rebuild_light_map.

Virtue shrine dialogue: shrine index from g_shrine_x/y (0x1f6e/0x1f76; 6 if no match); the answer must
contain the 4-letter stem 0x4b3e[i] and the mantra 0x1f5e[i] three times. Three outcomes on
g_shrine_quests (ordained, 0x58cc) / g_shrine_quests_done (0x58ce): ordain (quest text
0xb21e+0x4b5e[i], 7-note tune 0x4be6..0x4c1e), donate N×100 gp for +N karma ("ALAKAZAM"), or complete
(clear bit, +3 karma, +1 str/dex/int per 0x4b7e/0x4b86/0x4b8e, Humility +3 more). Codex dialogue
marks the lowest ordained quest done, prints 0xb21e+0x4b6e[i], and at 0xff shows the Codex wisdom.

## Data
- DGROUP: 0x1b7a word table indexed by ASCII letter (0x1bfc = 'A' → "AN"…"ZU"); 0x1c30 g_spell_code_ptrs
  (48 alphabetised codes); 0x24d6/0x24de dx/dy per dungeon facing; 0x595a g_dungeon_map; 0x4b95/0x4b9e/
  0x4bc2 urn layouts; 0xbd08 13-byte answer buffer; 0xbd15 saved location; 0xb21e MISCMSG/OOL buffer.
- Files: MISCMAPS.DAT, MISCMSG.DAT, UNDER.OOL, BRIT.OOL, SAVED.GAM, SAVED.OOL.
- Fragments: e63e/e648 (tail of e5ea) and ec1a (tail of eb46) exist only because ZSTATS.OVL has vector
  targets at those addresses; translated as no-ops, logic folded into the owning function.

## Port notes
- `play_sweep_noise` "string pointer" arguments are numbers (0x2710+4000·circle, 10000, 5000).
- Quest/Codex text table values are MISCMSG buffer offsets, printed as value − 0x4de2 (= +0xb21e).
- `in_mani_corp_resurrect` uses 32-bit exp·karma/100; the level is 1 + ⌊log2(exp/100)⌋+… loop.
