# Rust port plan and translation conventions (DRAFT v1)

## Principle: logic in Rust, data stays in the user's game files, memory stays faithful

The original is 16-bit C with pervasive pointer arithmetic, memory-image save files, far pointers to
heap buffers, and data tables inside code segments. To make the translation mechanical and correct we
model the original address space explicitly:

* `Mem` — an emulated flat memory (1 MiB `Vec<u8>`), linear addressing. At startup we load into it, at
  the ORIGINAL addresses: the ULTIMA.EXE load module (base 0x10000, for data tables inside code segments),
  every `.OVL` file at its load address (same reason — overlay code is never executed, only its tables read),
  and `DATA.OVL` at DGROUP (0x1f640). Nothing from the game is embedded in the Rust binary.
* Near pointers (DS-relative, 16-bit) are passed around as `u16` offsets into DGROUP; far pointers as
  `u32` linear addresses. Helper accessors: `m.u8(lin)`, `m.u16(lin)`, `m.set_u8(..)`, `m.ds(off) -> lin`
  (= 0x1f640 + off), `m.cstr(lin)`, `m.copy(dst, src, n)`.
* Globals are accessed through named offset constants generated from the symbol table (`analysis/symbols.json`):
  `g::AVATAR_X` etc. (`pub const AVATAR_X: u16 = 0x....`), with typed helpers: `gm.g_u8(g::AVATAR_X)`,
  `gm.set_g_u8(..)`, `gm.g_i16(..)`. Arrays/structs: `gm.ds(g::PARTY) + i*32 + 0x0b`.
* The C heap (`malloc`/`_fmalloc`) is a simple allocator over the emulated memory above 0x2bb80, so
  pointers returned behave like the original's (paragraph aligned for far allocations).
* C locals whose address escapes (buffers passed to callees) are allocated on an emulated stack region
  (`gm.stack_alloc(n)` / scoped `gm.with_frame(n, |gm, addr| ..)`); plain scalar locals are Rust locals.
* Integer types: C `int`→`i16`, `unsigned`→`u16`, `char`→`i8` (signed!), `unsigned char`→`u8`, `long`→`i32`.
  Use wrapping arithmetic (`wrapping_add`, `as` casts) where the original relied on 16-bit overflow.
* Every game function becomes `pub fn name(gm: &mut Game, args..) -> ret` in a module mirroring the
  original module (`engine/` for ROOT split by subsystem; `ovl/town.rs`, `ovl/cmds.rs`, ...). Overlay
  loading disappears: a call through `vec_TOWN_93c0` is just `ovl::town::fn_name(gm, ..)`.
* `Game` holds: `mem: Mem`, platform handles (video framebuffer, key queue, timer, sound), the heap
  allocator state, and nothing else game-specific (all game state lives in `mem` exactly like the original).

## Platform layer (replaces DOS/BIOS/EGA.DRV)

Implemented in `platform/` as a Rust `Driver` with the 38 entry points documented in `analysis/driver_api.md`
plus DOS services used by the C runtime (file open/read/write/seek/close on the game dir, time, keyboard
`int 16h` poll/wait, timer ticks). Video is a 320x200 16-colour framebuffer in a minifb window
(scaled). Keyboard waits pump the window so the UI stays responsive; timer-interrupt driven logic
(if the game hooks INT 8/1C) is called from the pump at 18.2 Hz.

## Process

1. Symbol table: merge `analysis/root_part*.json`, `globals_seed.json`, overlay analyses →
   `analysis/symbols.json` (functions: addr→name/proto/module; globals: offset→name/type).
2. Rename pass: produce `decomp_named/*.c` where FUN_/DAT_ identifiers are replaced by the chosen names
   (plus a header comment per function with purpose text) — the input for translators.
3. Translate module by module (agents in parallel), compile, then run against the original data and
   compare behaviour/screens with DOSBox (`tools/dosbox/capture.sh`).

## Facts established by the root analysis (see root_part1..4.md)

* Whole engine compiled with the Pascal convention (`RET n`). Ghidra assumed cdecl, so in the
  decompiled C `param_1` is the LAST source argument and call sites list arguments in reverse
  source order — but definitions and call sites are CONSISTENT with each other, so translating the
  decompiled C as-is is semantically correct. Use the analysts' protos (true order) for naming.
* Save game = raw copy of DGROUP 0x55a6..0x6606 (0x1060 bytes); party records 0x55a8 (16×32 bytes),
  location 0x5893, monster/object table 0x5c5a (32×8). SAVED.OOL buffer 0xb21e. All RAM-only tables
  must stay at their offsets (view buffer 0xab02, combat roster 0xba14, ...).
* RNG `rand_range(lo,hi)` (1000:2092): seed s at 0x5420: s = (((s + 0x9248) ror16 3) ^ 0x9248) + 0x11;
  result (s & 0x7fff) % (hi-lo+1) + lo. Must be bit-exact (102 callers).
* Frame pacing: `wait_ticks(n)` (18.2 Hz via INT 1Ch counter); map refresh runs inside the key wait
  (`read_command_key` → `refresh_map_view` every poll) — the platform key-wait must pump the window
  and call the refresh callback.
* Sound: PIT channel 2 tones (`start_tone`/`stop_tone`/`beep`, random-tone noises, software-PWM sweep)
  gated by 0xa9ce; delays are independent of sound. Port: generate square waves into an audio sink.
* Text: 4 text windows (0x535e, 8-byte records, 8×8 cells), `put_char` control codes 0xfb–0xff,
  `print_string` word-wrap/centring; cursor animation glyphs 5..8.
* Driver (EGA.DRV) is register-called from ROOT wrappers 1000:0a70–1140; the Rust `Driver` trait
  should expose the WRAPPER-level operations (fill_rect, draw_tile, draw_image, copy_page, scroll,
  glyph output, mode set, ...) with the wrappers' semantics, not the raw register entries.
* Directions: 1=W 2=E 3=N 4=S; dx/dy scratch at 0x5876/0x5878.
