# Translation brief: decompiled C → Rust (read fully before translating anything)

Project: Ultima V (DOS, MS C 5.x small model, Pascal calling convention) → Rust crate `rust/`.
Goal: a FAITHFUL translation. Preserve every behaviour and quirk; do not "improve" game logic.

## Where things are
- `decomp_named/<MODULE>.c`: the decompiled C with analyst names applied and a purpose comment above
  every function (ROOT, TOWN, MAINOUT, …). Use this as the source text.
- `analysis/symbols.json`: functions (addr → name/proto/purpose/globals) and globals (offset → name/type/meaning).
  `rust/src/g.rs`: generated `pub const NAME: u16 = 0x....;` for every named global (regenerate with
  `python3 tools/gen_globals.py` if symbols change). `decomp_named/globals.txt` lists them.
- `analysis/fn_index.tsv`: every original function → Rust module and name.
- `rust/src/engine/rest.rs` (ROOT) and `rust/src/ovl/<name>.rs`: generated STUBS with the right
  signatures; replace each `unimplemented!(...)` body with the translation. Keep signatures unless a
  type is clearly wrong (then fix every caller too). The crate must build after your work: run
  `source ~/.local/opt/u5env.sh && cd rust && cargo build --release`.
- Hand-written foundation (already done, use it): `engine/gfx.rs` (driver wrappers), `engine/text.rs`
  (put_char/print_string/print_number/windows/fonts), `engine/input.rs` (read_key/wait_key/read_line/
  read_command_key), `engine/files.rs` (load_file/load_file_region/save_file_region/select_disk…),
  `engine/util.rs` (rand_range/wait_ticks/delay_loop/beep/tones/animate_wait/to_upper). `game.rs` = Game.

## The memory model (this is what makes translation mechanical)
- `Game` (`gm`) owns `gm.m: Mem`, the whole emulated address space. DATA.OVL (= DGROUP, DS 0x1f64),
  the EXE image and every overlay's bytes are loaded at their original addresses at start-up, so every
  table the code reads — even ones inside code segments (`DAT_1000_xxxx`) — is present.
- DGROUP globals: `gm.g8(g::NAME)`, `gm.gi8`, `gm.g16`, `gm.gi16`, `gm.g32`, `gm.set_g8/..`. Offsets are
  `u16` DGROUP offsets. Unnamed globals `DAT_1f64_1234` → `gm.g16(0x1234)` (add a comment).
  Arrays/structs: `gm.g8(g::PARTY + i as u16 * 0x20 + 0x0b)`. Pointer-to-global `&g_x` → `g::X` (u16).
- NEAR pointers (`char *`, `int *`, `undefined *`, any 16-bit pointer) are `u16` DGROUP offsets. Deref:
  `gm.g8(p)`, `gm.g16(p + 2)`. Pointer arithmetic is plain u16 arithmetic (`p.wrapping_add(..)`).
- FAR pointers / segments: a "segment" value (e.g. a heap block from `load_file`, `g_font_segs[n]`,
  `g_map_seg`) is a `u16` paragraph; linear address = `(seg as u32) << 4`; access with `gm.m.u8(lin)`,
  `gm.m.u16(lin)`, `gm.m.set_u8(..)`. `CONCAT22(seg, off)` in Ghidra = far pointer → `Mem::lin(seg, off)`.
- Local arrays whose address is passed to callees (e.g. `char local_42[40]` given to print_string or
  read_line) → `gm.with_buf(40, |gm, buf| { ... })` (`buf` is a u16 DGROUP offset in scratch memory).
  Plain scalar locals are ordinary Rust `let mut`.
- String literals `s_Thou_dost_see_1f64_752c` → `g::...` if named, else `0x752c` (u16 offset) with a
  comment of the text. `s_..._1f64_752c + 4` → `0x752c + 4`. NEVER copy game text into Rust strings.
- Heap: `gm.alloc_seg(bytes) -> Option<u16>`, `gm.free_seg(seg)`. DOS alloc/free calls map to these.

## Calling convention / parameter order (critical)
The binary is Pascal convention but Ghidra decompiled it as cdecl, so in the C text the parameter
list of a definition AND the argument list of every call are both in REVERSED source order — and
therefore CONSISTENT with each other. Rule: the Rust signature keeps Ghidra's `param_1, param_2, …`
order (the stubs already do), and every call site is translated with the arguments in the SAME order
as the C text. Do not reorder anything. (Parameter names in the stubs carry the true meaning.)
Vector calls `vec_TOWN_93c0(...)` / `town::name(...)` → `ovl::town::name(gm, ...)`.
Register-called assembly helpers (protos like `/* AX=x, BX=y */`) are already folded into the hand-
written foundation; if you meet one elsewhere, reimplement its effect inline.

## Types
`int`→`i16`, `uint`/`undefined2`→`u16`, `char`→`i8` (signed!), `byte`/`undefined1`→`u8`, `long`→`i32`,
`ulong`/`undefined4`→`u32`, `bool`→`bool`, pointers→`u16`. Use wrapping ops (`wrapping_add`,
`wrapping_sub`, `wrapping_mul`, `as` casts) wherever the original 16-bit/8-bit arithmetic could wrap;
Rust panics on overflow in debug builds and we want identical behaviour. Comparisons: match the
signedness Ghidra shows (`(int)`, `(uint)`, `(byte)`, `(char)` casts matter). `x >> 1` on signed = `>>`
on `i16`; `CONCAT11(a,b)` = `(a as u16) << 8 | b as u16`; `CARRY2`/`SBORROW2` = overflow checks.

## Control flow
Translate `goto`/labels with loops+flags or `'label: loop {}` blocks faithfully. `do {} while` →
`loop { ...; if !cond { break } }`. Switch → `match`. Keep the order of side effects exactly.
`exit(n)` → `std::process::exit(n as i32)`. DOS/BIOS interrupts (`swi(0x21)` …) in game code: file ops
→ `engine::files`, key polls → `gm.pf.key_available()` / `gm.pf.keys.take()`, timer → `gm.pf.timer`,
speaker ports → `engine::util::start_tone/stop_tone`.

## Driver calls
`g_drv_entry_off = N; (*…)(…)` inside game code = graphics-driver entry N/3. Map to the matching
`gm.pf.video.*` primitive (see `analysis/driver_api.md`) or the ROOT wrapper in `engine/gfx.rs`.
Present frames with `gm.pf.maybe_present()` inside long drawing loops; waits already pump the window.

## Unknowns
If a callee is a stub, just call it (it compiles). If something is genuinely ambiguous, keep the
literal translation and add `// TODO(verify): …`. Never silently drop code. Keep the original
address in a doc comment on every function (`/// orig 1000:1850`).
