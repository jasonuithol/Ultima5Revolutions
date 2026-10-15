# FLAMES.OVL — 32-byte "free the level-2 region" overlay

Level-2 overlay (load segment 1a29 = 1000:a290, 32 bytes, shares its region with NPC/COMBAT/
BLCKTHRN/LOOKOBJ/DNGLOOK/OUTSUBS/SHOPPES/ENDGAME). Two functions, no data tables, no DGROUP
access. Rust: `rust/src/ovl/flames.rs`. Records: `analysis/ovl_FLAMES.jsonl`.

| addr | name | behaviour |
|---|---|---|
| 1000:a290 | `flames_entry` | `PUSH BP,SI,DI,DS; CALL a29e; POP…; RET` — returns AX from a29e |
| 1000:a29e | `flames_noop` | `POP AX; PUSH AX; RET` — AX = its own return address 0xa299 |

## Entry point and caller
Vector **1778:057e** (`ovlnum` 5 = FLAMES) → `FLAMES_a290`, called only by **FONT_f032**:
`if (g_unk_520c == 0) g_unk_520c = vec_FLAMES_a290();` followed by a copy of 0x1800 words from
DS:0 to that pointer (and FONT_f05b presumably copies them back). The trick: calling into the
32-byte FLAMES overlay makes the PLINK manager evict whatever level-2 overlay is resident, so the
level-2 region from 1000:a299 on (linear 0x1a299, ~28 KB up to the level-3 base 1bf8:0000) is
guaranteed free and the returned offset is the start of a scratch buffer for the View/character
creation screens. The vectors 1778:02f6 (NPC), 05f6 (COMBAT) and 07b2 (DNGLOOK) that the catalog
lists as callers target the same address inside other overlays.

## Port notes
* `flames_entry` returns `FLAMES_SCRATCH_OFF = 0xa299`; FONT's translation should store it in
  g_unk_520c (0x520c) and copy through `gm.m` at linear `0x10000 + off` — that memory is the
  emulated overlay area and is otherwise unused by the port (overlay bytes are only loaded for
  their data tables; FONT and all level-2 overlays' code never executes from it).
* ROOT's `fn_1778_057e` stub in `engine/rest.rs` is the vector; FONT should call
  `ovl::flames::flames_entry` directly.
