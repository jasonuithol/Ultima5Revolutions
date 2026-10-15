# ROOT part 1 (1000:02f4 – 1000:1eac) — overview

119 Ghidra functions; 32 of them are artifacts (see "INTRO +0x10 offset" below). Records: `analysis/root_part1.jsonl`.

## Two corrections to ARCHITECTURE.md that the port must get right

1. **DATA.OVL has a 16-byte header.** DGROUP offset X corresponds to DATA.OVL *file* offset X+0x10
   (verified three ways: the CRT null-pointer checksum of DGROUP[0..0x42] is 0x55 only for file bytes
   0x10..0x51; the wiki's "scancodes at 0x541E" are used by the code at DGROUP 0x540e; the driver-name
   table referenced at DGROUP 0x5328 is at file 0x5338). So when loading DATA.OVL into the emulated
   DGROUP, skip the first 16 bytes. Wiki `wiki_DATA_OVL.txt` offsets are file offsets (= DGROUP + 0x10).
2. **The graphics driver is register-called, not C-callable.** Every `(*DAT_1f64_5350)(...)` in ROOT is
   preceded by assembly that loads AX/BX/CX/DX/SI/DI (and sometimes the carry flag as an extra mode bit)
   from the stack arguments and then `CALLF [5350]`. The ROOT wrapper functions (0a70–1140) use the
   **Pascal** convention (args pushed left to right, callee `RET n`), so Ghidra's cdecl-style call sites
   show the arguments **reversed** (e.g. `FUN_0aa6(199,0x13f,0,0)` is `fill_rect(0,0,319,199)`). The
   JSONL protos give source order and the register mapping.

## INTRO +0x10 offset artifacts

Every call from INTRO.OVL into ROOT resolves 16 bytes past the real target (INTRO's own 16-byte header
was presumably not stripped when it was imported). Ghidra therefore created a bogus function at
`real+0x10` for each of them: 0888, 08a2, 0a80, 0ab6, 0b20, 0bbe, 0bf4, 0c32, 0d5c, 0d82, 0f56, 0f7e,
0fa0, 0fbe, 0fec, 1004, 103e, 1168, 16b6, 16ca, 1860, 1ba4, 1c02, 1c32, 1cae, 1cda, 1cfe, 1d6e, 1e48
(and more in later parts: 2066, 208e, 210a, 217c, 252e, 25e8, 4c3a, 4cde, 638e). Two artifacts hide real
functions that Ghidra never listed: **1000:0e94 `load_video_driver`** (Ghidra "0ea4") and
**1000:0f2a `driver_init_screen`** (Ghidra "0f3a"); both are documented under the artifact addresses.
15a5 is a third artifact (mid-function in 1588, created by an uninitialised far pointer).
When analysing INTRO, subtract 0x10 from every ROOT call target.

## Address ranges / subsystems

| Range | Subsystem |
|---|---|
| 02f4–082a | MS C 5.x runtime: `exit`, vector restore, `__aFuldiv/__aFlmul/__aFldiv/__aFlshl`, `memchr`, `itoa`, run-time error messages (table at DGROUP 0xa452), `_nullcheck`. Nothing here matters to the port except that `exit(n)` ends the program. |
| 0878–0a6e | Graphics library core (assembly, register params): save/restore BIOS mode, driver init (entry 1 with BX=&config 0x52ba), rectangle normalise/clamp, Cohen–Sutherland clipper (window 0x52d0..0x52d6, enabled by 0x52c4). |
| 0a70–1140 | Pascal-convention drawing API over the driver: set_color(15), fill_rect(21, CF=XOR), copy_rect_between_pages(8), draw_line/line_to (16/17/19/20), plot_pixel, draw_hline, draw_vline, set_draw_page(5), set_clip_window, copy_page(9), draw_image(25/33), draw_image_masked(26), draw_tile(27), draw_effect_frame(32), tile_transition_effect(34 CF=1), rect_effect(34 CF=0), load_image_file(22), load_picture_file(24)/free_picture(30), driver_set_speed(37), free_far_block. |
| 0de0–0f2a | Video setup: detect_video_adapter (dead code), load_video_driver (picks CGA/EGA/T1K/HER.DRV by flags 0x52ba/0x52ef/0x52f1/0x52f3, loads it with the generic loader, stores segment in 0x5352), driver_init_screen (entry 2 → scratch segment 0x5354). |
| 1158–1226 | init_system: INT 24h/23h handlers, text windows, attribute reset, CPU speed calibration (INT 1Ch hook, INT 12h). |
| 124a–1674 | File layer: fatal_out_of_memory, load_file (raw or LZW), lzw_decompress, is_raw_file_name, dos_alloc_top, read_file_into_seg, dos_read, get_file_size, file_exists. |
| 16a6–1a3e | Text output: put_char (window-relative 8×8 glyph output through driver 31, with control codes), prepare_glyph, print_string (word wrap, centring), print_number. |
| 1b16–1eac | Input and windows: flush keyboard, poll_key_with_cursor, select/define window, set_cursor_pos, select_font/load_font, set_text_color, read_key, wait_key, read_line, set_default_drive. |

## DOS / BIOS services used

INT 21h: 09h print `$` string (fatal only), 0Eh select disk, 19h get disk, 25h/35h set/get vector
(INT 00h restore at exit; INT 1Ch during calibration; INT 23h/24h installed once), 30h DOS version,
3Dh open (AL=2), 3Eh close, 3Fh read, 40h write (stderr, CRT only), 42h lseek (size), 48h/49h
alloc/free, 58h set allocation strategy, 4Ch terminate, 06h direct console input (keys).
INT 10h: 00h set mode, 0Fh get mode, 12h/BL=10h EGA info. INT 16h: 01h key available, 02h shift
flags. INT 12h memory size. Port I/O 3BAh (Hercules detect, dead code).

Interrupt handlers installed: INT 1Ch at 1000:1214 (temporary, increments 0x535a), INT 24h at 1000:1233
(sets 0x535c=1, returns "ignore"), INT 23h at 1000:1247 (IRET; Ctrl-Break ignored). No permanent timer
hook in this chunk: delays are busy loops scaled by `g_cpu_speed` (0x5356).

## Key data structures (DGROUP offsets)

* **Graphics config block 0x52ba** (passed to the driver): 52ba force-CGA flag; 52bc/52be cell w/h (8,8 =
  map view origin / text cell); 52c0/52c2 tile w/h (16,16); 52c4 clip enable; 52c8 adapter type
  (0 CGA,1 EGA,2 Tandy,3 Hercules); 52ca machine class; 52cc/52ce pen x,y; 52d0..52d6 clip window;
  52d8 draw page (written by driver); 52da current colour (-1 none); 52ef/52f1/52f3 EGA/Tandy/Herc flags.
* **Driver far pointer 0x5350/0x5352**; **0x5354 scratch segment** from driver entry 2 (LZW dictionary).
* **Text windows 0x535e**: 4 × {x0,y0,x1,y1,curx,cury,attr,flags} bytes; 0x5386 current index, 0x539a
  pointer; 0x5388 current font, 0x539c[4] font segments, 0x5398 current font segment; 0x53aa/53ab fg/bg;
  0x53a4/53a6/53a8 underline/centred/inverse; 0x538e cursor-advance enable; 0x5390/0x5392/0x540c cursor
  glyph base/frames/current frame; 0x538a key-from-keypad; 0x538c flush-after-key.
* **Error path**: 0x535c last DOS error, 0x5394/0x5396 far error hook (ROOT 2320/2322/25ca); 0x541e
  current drive.
* **LZW state 0x53b4–0x53d5** (see record 135a); masks 0x53ac; "Out of Memory" string 0x53e6 whose tail
  0x53ea is reused as the glyph backup buffer.
* **Tables**: 0x5306/0x5314 widths/heights of the 7 stacked title strips; 0x5322 "Tandy"; 0x5328 driver
  file-name pointers; 0x5400 powers of ten; 0x540e/0x5416 scan-code translation; 0x25ea list of
  uncompressed file names; CRT tables 0x5273, 0x528e, 0x52b0–0x52b6, 0xa452.
* **Timing**: 0x5356 CPU speed factor, 0x5358 memory KB, 0x535a tick counter.

## Things the Rust port must reproduce

* `load_file`: raw-vs-LZW decision by file name list; LZW (9–12 bit, LSB-first, 0x100 clear, 0x101 end,
  KwKwK case) must be bit-exact; result is a segment-addressed block.
* The image files loaded through driver entries 22/24 are converted *in place* to the driver's planar
  format; the port's driver replacement should instead decode the original `.16` layout directly.
* put_char semantics (control codes 0xfb–0xff, bit-7 stripping, wrap at window right edge, 8-pixel scroll
  via driver 13) and print_string's wrapping/centring rules — all game text goes through them.
* read_key mapping (F-keys → 0xc9.., arrows → 1..4, home/end/pgup/pgdn → 0xd3..0xd6, keypad flag).
* Pascal argument order for every wrapper listed above (Ghidra call sites are reversed).
