# Briefing for overlay analysis agents (read fully before starting)

You are helping decompile Ultima V (PC/DOS, 1988, Microsoft C 5.x small model, compiled with the
PASCAL calling convention) into Rust. Ghidra produced decompiled C; your job is the SEMANTIC ANALYSIS
of one or more overlay modules. Do not write Rust.

## Workflow (results must be saved incrementally)
Append results to `analysis/ovl_<NAME>.jsonl` (one file per overlay, all paths relative to
/home/jason/Projects/Ultima5Revolutions) — ONE JSON object per line, written as soon as you finish each
function (batches of 3-5 per write via a Python helper or heredoc). If the file already exists when
you start, read it and skip addresses already done. Never wait until the end to write. When an overlay
is finished write `analysis/ovl_<NAME>.md` (overview, see below) before starting the next overlay.

## Essential facts
- Read `analysis/ARCHITECTURE.md` (memory layout, overlays, call vectors, driver convention).
- DGROUP offset X = DATA.OVL FILE offset X+0x10. `reference/wiki_DATA_OVL.txt` gives FILE offsets:
  subtract 0x10 to get the `DAT_1f64_xxxx` address.
- ALL game functions use the Pascal convention (callee cleans with `RET n`). Ghidra assumed cdecl, so
  `param_1` in a decompiled definition is the LAST source argument and every call site shows arguments
  in REVERSED source order. Definitions and call sites are consistently reversed, so the semantics are
  right; give the TRUE source order in your proto and say which Ghidra param_N each is.
- The ROOT module is fully analysed: `analysis/root_part1.md` … `root_part4.md` (overviews) and
  `analysis/symbols.json` (every ROOT function: address → name/proto/purpose; every named global:
  offset → name/type/meaning). USE THESE NAMES when describing calls into ROOT (FUN_1000_xxxx) and
  REUSE the global names (g_...) for already-named offsets. Quick lookup:
  `python3 -c "import json;s=json.load(open('analysis/symbols.json'));print(s['functions']['1000:1850'])"`
  or grep `decomp_named/ROOT.c` (ROOT decompilation with names applied, purpose comments included).
- Save game is loaded at DGROUP 0x55a6 (SAVED.GAM offset S = DGROUP 0x55a6+S): party records 0x55a8
  (16 × 32 bytes), location 0x5893, x/y 0x5896/0x5897, monster/object table 0x5c5a (32 × 8 bytes),
  etc. Map `reference/wiki_SAVED_GAM.txt` fields to DGROUP offsets as you meet them.
- Calls to `vec_X_yyyy` are calls into overlay X's function `X_yyyy` (in `decomp/X.c`); calls from an
  overlay into ROOT are direct (FUN_1000_xxxx). `decomp/catalog.json` has callers/callees for every
  function (id "MODULE:seg:off").
- `(*DAT_1f64_5350)(...)` after `DAT_1f64_5350 = N` = graphics-driver jump-table entry N/3
  (register-called; `analysis/driver_api.md` documents every entry if it exists yet; the ROOT wrapper
  functions in root_part1 are what overlays normally call).
- Directions: 1=W 2=E 3=N 4=S. Text goes through ROOT put_char/print_string (control codes 0xfb–0xff).
- Disassembly with bytes: `ghidra/export/<NAME>/ULTIMA_EXE_listing.asm` (the overlay's code is in the
  1000:81d0–1000:f63f range of that file). Use it when the C is unclear (e.g. INT 21h AH values).
- Data formats: `reference/wiki_files_part1.txt`, `wiki_files_part2.txt` (maps .DAT, .NPC schedules,
  .TLK conversations, .CBT combat maps, .OOL object lists, SIGNS/LOOK2/SHOPPE/MISCMAPS/STORY .DAT …).
  Data tables inside the overlay itself (e.g. INTRO.OVL) appear as `DAT_1000_xxxx` / arrays in code
  space; describe their layout.
- Ghidra string symbols like `s_You_see__darkness__1f64_752e` are string literals in DGROUP.

## Record format (one per line in ovl_<NAME>.jsonl)
{"addr":"1000:93c0","name":"proposed_snake_case_name","proto":"ret_type name(type param, ...)" (C-style,
 TRUE source order, meaningful parameter names, best-guess types; 16-bit ints),
 "libc":null,
 "purpose":"3-10 sentences: what it does, in enough detail that someone could reimplement it from the
 decompiled code plus this text; side effects, return meaning, quirks; for big functions describe the
 major branches and the meaning of magic numbers (tile ids, location codes, item ids, string tables)",
 "globals":{"0x587c":{"name":"g_name","type":"u8|i8|u16|i16|u8[n]|ptr|...","meaning":"..."}}
 (every DGROUP global the function touches; reuse already-named offsets; new offsets get new g_ names),
 "calls_driver":[N,...], "confidence":"high|medium|low", "notes":"anything odd"}

## Overview file ovl_<NAME>.md (1 page)
What the overlay implements, its entry points (the functions called through vectors) and who calls
them, the main control flow (loops, state machines), key data structures (DGROUP offsets, heap
buffers, file formats consumed), and anything the Rust port must get right.

Naming rules: snake_case; function names describe behaviour; global names start with g_; keep hex
offsets as keys. Be faithful to the code — do not invent game semantics you cannot support from the
code, strings, call graph or reference docs; mark low confidence instead. Work through EVERY function
of the overlay in address order; don't skip. When done, reply with a brief summary (under 200 words).
