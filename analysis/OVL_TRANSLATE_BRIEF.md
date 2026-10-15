# Overlay analysis + translation brief (one agent per overlay group)

You translate one or more Ultima V overlay modules (`.OVL`) from decompiled C into Rust, naming and
documenting as you go. Read these first, fully:
1. `analysis/TRANSLATION_BRIEF.md` — the memory model, pointer/parameter/type rules. Mandatory.
2. `analysis/OVL_AGENT_BRIEF.md` — background facts about overlays (Pascal convention, DGROUP = DATA.OVL
   minus 0x10, vectors, resources). Ignore its "deliverables" section; the deliverables are below.
3. `analysis/ARCHITECTURE.md`, and the ROOT overviews `analysis/root_part1..4.md` as needed.

All paths relative to /home/jason/Projects/Ultima5Revolutions.

## Inputs for overlay NAME
- `decomp_named/NAME.c` — decompiled C with ROOT names applied (purpose comments only exist for ROOT
  functions; overlay functions are still `NAME_xxxx`). ROOT callees are documented in
  `analysis/symbols.json` and already translated in `rust/src/engine/*.rs` (grep `pub fn name`).
- `rust/src/ovl/name.rs` — generated stubs `fn_1000_xxxx(gm, ...)` with Ghidra-order signatures.
- `ghidra/export/NAME/ULTIMA_EXE_listing.asm` — disassembly if the C is unclear.
- `decomp/catalog.json` — callers/callees of every function (id "NAME:1000:xxxx").

## Deliverables
A. `analysis/ovl_NAME.jsonl` — one record per function (same format as the ROOT records):
   {"addr":"1000:93c0","name":"snake_case","proto":"ret name(args in TRUE source order)","purpose":"3-10
   sentences","globals":{"0x....":{"name":"g_...","type":"...","meaning":"..."}},"calls_driver":[],
   "confidence":"high|medium|low","notes":"..."} — append as you go (this is also your progress log).
B. The translated Rust module `rust/src/ovl/name.rs`: every stub body replaced; functions renamed from
   `fn_1000_xxxx` to your snake_case name. RENAMING RULE (important): stub names are only unique WITHIN a
   module — different overlays share the same address range, so `fn_1000_93c0` exists in both TOWN and
   MAINOUT. Rename (a) the definition and unqualified uses inside your own file, and (b) qualified callers
   everywhere else: `grep -rl 'ovl::town::fn_1000_93c0' rust/src | xargs sed -i 's/ovl::town::fn_1000_93c0\b/ovl::town::enter_town/g'`.
   Never do an unqualified crate-wide replace. Keep the `/// orig 1000:xxxx` doc comment on each function
   and add the purpose as doc text. Change the file's first line to `//! NAME.OVL (translated)` when done.
C. `analysis/ovl_NAME.md` — one page: what the overlay implements, entry points (vector targets) and
   who calls them, control flow, data structures (DGROUP offsets, buffers, files consumed), port notes.

## Rules
- Work function by function in address order; after every 3-5 functions build:
  `source ~/.local/opt/u5env.sh && cd rust && cargo build --release 2>&1 | grep -E "^error" -A6 | head -40`
  and fix errors before continuing. If restarted, continue from the first remaining `unimplemented!`.
- Faithful translation: every branch, every side effect, every magic number. Never summarise a big
  function; never "fix" game logic. Add `// TODO(verify):` where the C is ambiguous.
- Data tables inside the overlay (`DAT_1000_xxxx` / arrays in code space) are already in emulated memory
  at their original linear address: read them with `gm.m.u8(0x1xxxx)` / `gm.m.u16(..)` — never copy
  the bytes into Rust source. DGROUP data: `gm.g8(off)` etc., `g::` constants where named.
- Game text is never embedded in Rust: print with DGROUP offsets (`text::print_string(gm, 0x7530)`).
- Stub signatures may be wrong (parameter count/type): fix them and all callers. Return types: if a
  caller uses the result, the function must return it.
- Calls into other overlays appear as `other::fn_1000_xxxx(...)` → `ovl::other::fn_1000_xxxx(gm, ...)`
  (or its new name if already translated — check the file).
- Stay within your assigned overlays; do not edit other overlay files except for renames/signature
  fixes of functions you own.
When done: crate builds, deliverables A/B/C written. Reply with a summary under 200 words.
