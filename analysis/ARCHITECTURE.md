# Ultima V (PC, 1988) binary architecture — findings

Game dir: `/home/jason/Projects/Ultima 5/` (GOG release; original DOS files, run via DOSBox).
Compiler: Microsoft C 5.x (small model, near code/data, `__cdecl`), linked with Phoenix PLINK86 overlay linker.
Toolchain for this project: Ghidra 12.1.4 headless (`~/.local/opt/ghidra_12.1.4_PUBLIC`, JDK at `~/.local/opt/jdk21`), scripts in `ghidra/scripts/`, exports in `ghidra/export/<MODULE>/`, merged decompilation in `decomp/<MODULE>.c` and `decomp/catalog.json` (built by `tools/merge_exports.py`).

## Memory layout (Ghidra load base = segment 0x1000; DOS loads at PSP+0x10, everything relative)

All code lives in ONE 64 KB code segment (CS = 0x1000 in Ghidra). Near calls everywhere.

| Ghidra address        | Contents |
|-----------------------|----------|
| 1000:0000–1000:72df   | ROOT code (ULTIMA.EXE): C runtime + core engine (361 functions). |
| 172e:0000–172e:03df   | PLINK86 overlay manager (assembly). `plink_ovl_load` at 172e:02ec. |
| 176c:0000–176c:00bf   | overlay manager data. |
| 1778:0000–1778:017f   | overlay descriptor table (24 entries × 16 bytes). |
| 1778:0188–            | overlay filename strings, then 164 call **vectors** (see below). |
| 181d:0000–181d:051f   | PLINK startup stub (only in ROOT program; overwritten by overlays at run time). |
| 1000:81d0–1000:f63f   | **overlay area** (`.OVL` files are loaded here, 4 levels, see table). |
| 1f64:0000–1f64:bd3f   | **DGROUP** = `DATA.OVL` minus its 16-byte header (file bytes 0x10.. land at DGROUP 0x0000). DS = 0x1f64 for all C code. **DGROUP offset X = DATA.OVL file offset X+0x10**; wiki DATA.OVL offsets are FILE offsets, so subtract 0x10 to get the `DAT_1f64_xxxx` address. |
| 1f64:bd50–1f64:c53f   | BSS + stack (stack top at 1f64:c540). |
| 2bb8:0000–            | C heap (malloc). |

### Overlay file headers / relocations
An `.OVL` whose descriptor has N>0 relocation records starts with N×4 bytes of (u16 offset, u16 segment) records padded to a 16-byte boundary; the code/data image follows. Only INTRO.OVL (2 records → 16-byte header) and DATA.OVL (3 records → 16-byte header) have one. The manager adds the load segment to each patched word. (Earlier Ghidra runs did not strip these headers; fixed on 2026-10-13.)

## Overlays (`.OVL`)

Loaded at fixed addresses by the PLINK86 manager (offsets relative to code base 0x1000, segment form):

| idx | name | load seg (Ghidra) | level | bytes |
|----|------|------|----|----|
| 0 | TOWN | 181d | 1 | 6256 |
| 1 | MAINOUT | 181d | 1 | 7344 |
| 2 | DUNGEON | 181d | 1 | 8016 |
| 3 | INTRO | 181d | 1 | 8384 (+16 bytes empty reloc) |
| 4 | FLAMES | 1a29 | 2 | 32 |
| 5 | NPC | 1a29 | 2 | 4912 |
| 6 | COMBAT | 1a29 | 2 | 7408 |
| 7 | BLCKTHRN | 1a29 | 2 | 3184 |
| 8 | LOOKOBJ | 1a29 | 2 | 4560 |
| 9 | DNGLOOK | 1a29 | 2 | 5040 |
| 10 | OUTSUBS | 1a29 | 2 | 2464 |
| 11 | SHOPPES | 1a29 | 2 | 5936 |
| 12 | ENDGAME | 1a29 | 2 | 2800 |
| 13 | SJOG | 1bf8 | 3 | 8800 |
| 14 | CMDS | 1bf8 | 3 | 7440 |
| 15 | CAST | 1bf8 | 3 | 8560 |
| 16 | TALK | 1bf8 | 3 | 4880 |
| 17 | CAST2 | 1e1e | 4 | 4544 |
| 18 | ZSTATS | 1e1e | 4 | 4880 |
| 19 | COMSUBS | 1e1e | 4 | 5216 |
| 20 | SHOPPES2 | 1e1e | 4 | 2848 |
| 21 | SHOPPES3 | 1e1e | 4 | 2528 |
| 22 | FONT | 1e1e | 4 | 3744 |
| 23 | DATA | 1f64 | data | DGROUP (resident) |

One overlay per level is resident at a time; overlays of different levels can coexist. Calls INTO an overlay always go through a 12-byte **vector** in segment 1778:
`CALLF plink_ovl_load ; dw overlay#+1 ; JMPF 1000:target`. The manager loads the overlay from disk if needed (evicting whatever occupied that level's region) then jumps to the target. Vectors are named `vec_<OVL>_<off>` (e.g. `vec_TOWN_93c0` → function `TOWN_93c0` at 1000:93c0 inside TOWN.OVL). In the merged decompilation a call to `vec_X_y` means "call overlay function X_y". Overlay functions call ROOT functions directly (near call) and other overlays via vectors. `decomp/catalog.json` has every function: id = `MODULE:seg:off`, name, size, callers, callees (vector calls resolved to the target function id), prototype.

## Graphics/sound driver (`EGA.DRV`, `CGA.DRV`, `HER.DRV`, `T1K.DRV`)

Raw binary loaded into a malloc'ed paragraph-aligned segment; far pointer at DGROUP 1f64:5350 (offset) / 1f64:5352 (segment). The file starts with a 38-entry jump table (3 bytes each, `jmp rel16`). Root calls driver function N as:
`mov word ptr [0x5350], N*3 ; callf [0x5350]` — so in the decompilation `(*DAT_1f64_5350)(args)` right after `DAT_1f64_5350 = 0x3c` means driver function 0x3c/3 = 20. **The driver is register-called** (AX/BX/CX/DX/SI/DI and sometimes the carry flag), not C-callable: each ROOT wrapper (1000:0a70–1140) loads registers from its stack arguments. The ROOT wrappers themselves use the **Pascal** convention (`RET n`, args pushed left→right), so Ghidra's cdecl-style call sites show their arguments in REVERSED order. See `analysis/root_part1.jsonl` protos for the true order and register mapping. Ghidra programs `U5_DRV_EGA` etc. (exports in `ghidra/export/DRV_EGA/`) name entries `drv_NN_offset`. The driver also contains the PC-speaker sound code.

Another far pointer at 1f64:5394/5396 is a root callback hook set to `1000:2320` / `1000:2322` / `1000:25ca` before certain calls (used by the overlay manager when an overlay file can't be opened, and by root code).

## Data files

LZW-compressed graphics (`*.16` EGA, `*.4` CGA, `*.BIT`), maps (`BRIT.DAT`, `UNDER.DAT`, `TOWNE/DWELLING/CASTLE/KEEP.DAT`, `DUNGEON.DAT`), combat maps (`BRIT.CBT`, `DUNGEON.CBT`), NPC schedules (`*.NPC`), conversations (`*.TLK`), save game (`SAVED.GAM`, `INIT.GAM`, `*.OOL`), misc (`DATA.OVL` tables/strings, `LOOK2.DAT`, `SIGNS.DAT`, `SHOPPE.DAT`, `MISCMAPS.DAT`, `STORY.DAT`, `END.DAT`, `ENDMSG.DAT`, `MISCMSG.DAT`, `QUESTION.DAT`, `KARMA.DAT`, `BRITISH.PTH`, fonts `IBM.CH/RUNES.CH/*.HCS`, `PROPORT.PCS`). Formats documented in `reference/wiki_*.txt` (Ultima Codex "Ultima V internal formats").

## Rust port conventions (see rust/ when it exists)

- DGROUP is modelled as a byte array (`Dgroup`), initialised from DATA.OVL, with typed accessors; every global keeps its original offset so that save/load (memory-image files) and index arithmetic behave identically.
- Platform layer replaces DOS/BIOS/driver: video framebuffer (320×200×16 colours, minifb window), keyboard, timer ticks, PC-speaker sound.
