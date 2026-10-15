# TALK.OVL — NPC conversations

Level-3 overlay (load segment 1bf8, 4880 bytes, 1000:bf80–1000:d28e), 39 functions, no
code-space data tables. Rust: `rust/src/ovl/talk.rs`. Records: `analysis/ovl_TALK.jsonl`.

## Entry points and callers
| vector | target | name | called by |
|---|---|---|---|
| (ROOT 'T') | c39c | `cmd_talk()` | ROOT dispatch_command in settlements (non-zero → result 2 = arrest) |
| 1778:xxxx | c29e | `talk_to_npc(npc)` | TOWN `handle_npc_contact` (guard/NPC walked into the party), `cmd_talk` |

Calls out: TOWN `find_npc_by_slot`, `set_npc_killed`, `remove_npc`, `raise_alarm`,
`begone_vermin`; NPC `schedule_index_for_hour` (vector 1778:03b6, shop hours); SHOPPES b542/
aa4e/a9ee/a732/b788, SHOPPES2 e84c/ec9c, SHOPPES3 ea94 (shop services); ROOT str_find_ci,
read_text_line, put_char/print_string/print_number, select_font, get_cursor_col, add_u8_capped,
add_i16_capped, sub_u8_floor, sub_i16_floor, find_first_conscious_member, draw_status_window,
get_direction, find_object_at, get_map_tile, load_file_region, play_tone_sweep, rand_range.

## Dispatch (`talk_to_npc`)
Dialogue index = NPC state +0xa (from the .NPC file): 0 "No response"; < 0x80 → .TLK entry
(`run_conversation`); 0x81..0x88 → shop type = index − 0x81 (`talk_to_shopkeeper`, served only
when the NPC's stored schedule entry is odd AND the entry for g_hour is odd — entry 1 = working
hours; otherwise "Come see me at … when it's open!"); 0xfd "Don't hurt me!"; 0xfe Shadowlord
(`begone_vermin`); 0xff guard (`guard_challenge`: Blackthorn's palace (0x12) password "IMPE"
when wearing the badge (status icon 0x1d), Minoc (5) half the gold to charity, elsewhere a
10 gp/member tribute; refusal = 1 = arrest). A guard-tile NPC (slot tile 0x70) with an even
schedule entry or no dialogue "offers no response"; an NPC with AI 4 (guard post) is switched to
AI 1 when addressed.

## .TLK format as consumed
File = u16 count, count × {u16 npc, u16 offset}; the entry (1 KB) is loaded into g_ool_buffer
0xb21e. Sections are NUL-terminated: 0 name, 1 description, 2 greeting, 3 job, 4 bye, then
keyword k at section 2k+5 with its answer at 2k+6; a 0x90 byte ends the keyword list, after
which labels follow: 0x90 <0x91..0x9f> question NUL default-answer NUL {keyword NUL answer NUL}*.
Text bytes are ASCII | 0x80; bytes < 0x81 index g_tlk_word_ptrs (0x24e8, compressed words,
emitted with a leading space). Control bytes: 0x81 Avatar's name · 0x82 end · 0x83 pause ·
0x84 join party · 0x85 ddd pay gold · 0x86 i give item (0x41 food, 0x42 gold, 0x43 keys, 0x44
gems, 0x45 torches, 0x46 grapple, 0x47 carpet, 0x48 sextant, 0x49 spyglass, 0x4a badge, 0x4b
skull key, < 0x40 inventory) · 0x87 run the string after the next · 0x88 ask the Avatar's name
(sets the met bit) · 0x89/0x8a karma ±1 · 0x8b alarm · 0x8c L if met goto label L (0xff =
interest prompt) · 0x8d newline · 0x8e rune font toggle · 0x8f key wait · 0x91..0x9f goto label
(ask) · 0xfe K L karma ≥ K goto L · 0xff "Your interest?".

## Control flow
`run_conversation`: `greet_npc` ("You see" + description; greeting if met, else 50% "I am
called" + name) → `interest_loop` ("Your interest?": 34 built-in keywords at 0x4aa8 — NAME,
JOB/WORK, BYE, insults — then the NPC's keywords via str_find_ci on the typed line, match at
index 0 or after a space; answer = section 2k+6; "I cannot help thee with that.") → `say_bye`
(section 4) → `shadowlord_steals_item` (Shadowlord 0 present: lose an item / gold).
Labels: `ask_question_at_label` prints the question, reads "You respond:", re-asks on
NAME/JOB (built-ins return 0), BYE ends, else the label's keyword answer or default answer, then
back to the interest loop. All interpreter functions return 1 = conversation over.

## Data (DGROUP)
0xbcdc NPC index · 0xbcde script pointer · 0xbce0 opcode args · 0xbce4 pending word (16 B,
len 0x4af1, wrap at column 17 via 0x4af3) · 0xbcf4 label · 0xbcf6 keyword index · 0xbcf8 input
(15 chars) · 0x4aee pending opcode / 0x4aef arg count / 0x4af0 font mask (0xff; 0x7f runes) /
0x4af2 answering flag / 0x4af4 last char / 0x4af5 need-space · 0x4aa0 .TLK names · 0x4a9a
"IMPE" · 0x5bd6+4·location NPC-met bitmap (g_npc_met_flags) · shop selection 0xb116/0xb114/
0xaafc/0xaafe from g_shop_towns 0x23ca and the name pointer tables 0x21ca/0x22ca.

## Port notes
- `talk_to_npc` 0xfe calls TOWN `begone_vermin` without pushing its argument (the callee reads
  the caller's saved SI); the port passes the NPC index — TODO(verify).
- SHOPPES b788 (`healer_shop`) takes the member argument (RET 2) though it never reads it; the caller passes it.
- `interpret_byte`: only the 0x87 case saves/restores the script pointer (Ghidra's trailing
  assignment is an artifact). The password compare truncates the input to 4 letters.
- 0x4aef is manipulated as a 16-bit word together with 0x4af0 in places; kept bit-exact.
