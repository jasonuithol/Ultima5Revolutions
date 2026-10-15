# Ultima V Revolutions — a Rust decompilation of Ultima V (PC, 1988)

A faithful Rust port of the DOS *Ultima V: Warriors of Destiny* engine, produced by decompiling the
original executable and its 23 overlays (Ghidra) and translating every function. The port contains
game LOGIC only: all text, maps, graphics, fonts and saves are read at run time from your own copy of
the game (the GOG release), which is never modified except for the save files the game writes itself
(SAVED.GAM, *.OOL), exactly like the original.

## Running

    source ~/.local/opt/u5env.sh          # Rust toolchain (rustup) on PATH
    cd rust && cargo build --release
    U5_DIR="/path/to/Ultima 5" ./target/release/ultima5   # your own (GOG) copy of the game

Environment variables:
- `U5_DIR=/path/to/game`  game directory (the original DOS files: ULTIMA.EXE, *.OVL, *.DAT, *.16, …)
- `U5_VIDEO=E|C|T|H`      video driver flavour as the original's command-line letter (default E = EGA)
- `U5_HEADLESS=1`         no window (for scripted tests), with
  `U5_KEYS="ms:KEY,ms:KEY,..."` scripted keys (chars, ENTER, ESC, SPACE, UP/DOWN/LEFT/RIGHT, F1..),
  `U5_EXIT_MS=n` exit after n ms, `U5_SHOT=file.ppm` screenshot at exit (plus _1.._3 quarter shots)
- `U5_TRACE_KEYS=1`       log every key the game reads
- `U5_CPU_SPEED=n`        CPU speed factor used for busy-wait delays and speaker effects (default 2200 ≈ the GOG DOSBox profile; larger = faster)

Window: 320x200 EGA framebuffer scaled 3x. Keys are the original's: letters for commands, arrows or
keypad for movement, Enter/Space/Esc in menus. Sound is the PC speaker emulated through PipeWire
(`pw-play`), falling back to `paplay`/`aplay`.

## Layout

- `rust/src/engine/`  the ROOT module of ULTIMA.EXE (gfx wrappers, text windows, input, files,
  utilities, effects, and `rest.rs` with the rest of the engine incl. `main_entry`).
- `rust/src/ovl/`     one module per overlay (town, mainout, dungeon, combat, cast, talk, shoppes, ...).
- `rust/src/platform/` framebuffer window (minifb), EGA.DRV reimplementation (`video.rs`), keyboard,
  timer, speaker.
- `rust/src/mem.rs`, `loader.rs`, `g.rs`  emulated memory (DGROUP at its original address, overlay
  tables at theirs), file loader, generated global-offset constants.
- `analysis/`  the reverse-engineering record: ARCHITECTURE.md, per-function records (`*.jsonl`),
  per-module overviews (`*.md`), driver API, symbol table, translation briefs.
- `ghidra/scripts/` + `ghidra/run_*.sh` the Ghidra pipeline; its outputs (`ghidra/export`, `decomp/`,
  `decomp_named/`) are derived from the original binaries and are not part of the repository —
  run the pipeline against your own copy to regenerate them (see analysis/ARCHITECTURE.md).
- `tools/`  merge/symbol/stub generators and the DOSBox comparison harness (`tools/dosbox/`).

## Requirements and legal note

You need your own copy of the original PC game (e.g. the GOG release). This repository holds a
reimplementation of the program logic and the notes made while reverse-engineering it; it contains
no game data, text, graphics or binaries. Ultima V is a trademark/copyright of its owners.

## Conventions worth knowing

- Rust parameter order equals the Ghidra decompiled order, which is the REVERSE of the original source
  order (the game was compiled with the Pascal convention). Doc comments give the original address.
- Near pointers are `u16` DGROUP offsets, far pointers `u32` linear addresses; globals go through
  `gm.g8/g16/set_g8/...` with constants from `g.rs`.
- `// TODO(verify):` marks places where the original relied on uninitialised data or where the
  decompilation was ambiguous; behaviour was translated literally.
