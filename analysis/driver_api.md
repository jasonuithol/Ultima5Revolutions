# EGA.DRV — the Ultima V graphics/sound driver, documented for a Rust reimplementation

Source: `ghidra/export/DRV_EGA/EGA_DRV_listing.asm` (raw binary loaded at 3000:0000; the listing
prints `MOV Sreg,reg` operands reversed, e.g. `8EC0 "MOV AX,ES"` is really `MOV ES,AX`). Per-entry
records (one JSON per entry, all 38) are in `analysis/driver_api.jsonl`; this file is the narrative.
Cross-checked against CGA.DRV/HER.DRV/T1K.DRV where noted, and against the real data files
(decompressed TILES.16, ITEMS.16, MON0.16, DNG1.16, TITLE.BIT, WD.BIT, PROPORT.PCS, ULTIMA.16).

## 0. Calling convention and driver-internal state

* The file begins with a 38-entry jump table (3-byte `JMP rel16` each). ROOT calls entry N with
  `MOV word ptr [0x5350],N*3 ; CALLF [0x5350]` after loading registers; DS = DGROUP (0x1f64) on entry.
  Inputs are AX/BX/CX/DX/SI/DI/ES and sometimes CF (a mode bit). Nothing is passed on the stack.
* Driver state lives in its own code segment (CS-relative). The important cells:

| CS offset | meaning |
|---|---|
| 0x072..0x201 | row table: `row[y] = y*40` (y = 0..199), built on the first entry-1 call. Index 200 reads 0x202! |
| 0x202 | page-1 segment (0 until entry 2; then 0xA800) |
| 0x204 | XOR-mode flag for the current call |
| 0x206 | tile-set segment registered by entry 24 (TILES.16) |
| 0x208..0x23a | scratch for lines/scrolls/blits |
| 0x23c | near pointer to the config block (DGROUP 0x52ba), set by entry 1 |
| 0x23e[8] | `7F BF DF EF F7 FB FD FE` = NOT(pixel bit) for x&7 |
| 0x246[8] | `80 40 20 10 08 04 02 01` = pixel bit for x&7 |
| 0x24e[4] | `01 02 04 08` plane bits |
| 0x252[8] | `00 01 03 07 0F 1F 3F 7F` right-edge masks (k low bits) |
| 0x25a[8] | `00 80 C0 E0 F0 F8 FC FE` left-edge masks (k high bits) |
| 0x272..0x311 | 160-byte row scratch (4 planes × 40 bytes) |
| 0x312 | current colour (entry 15) |
| 0x314[256] | bit-reverse table; after init `t[i] = (bitrev8(i) << 1) | 1` |
| 0x1c1e | CPU speed factor (entry 37); the unit of every busy-wait delay |
| 0x1f94, 0x27a5, 0x28c2 | three private 16-bit LCG seeds (initial 0x7664): `s = (((s + 0x9248) ror 3) ^ 0x9248) + 0x11` |
| 0x253d | "dissolve makes sound" flag, initially 1, cleared forever by the first glyph draw (entry 31) |
| 0x27a7 / 0x27a9 / 0x27ab | noise parameters (max-frequency/2, duration units, step) |
| 0x2541 / 0x2543 | LFSR state / step for entry 34 |

* Standard epilogue (almost every entry): `GC reg 3 := 0` (normal ALU), `SEQ map mask := 0x0F`,
  `GC bit mask := 0xFF`. GC reg 4 (read map) and reg 5 (write mode, always 0) are left as they are.
* Hardware used: ports 0x3C4/0x3C5 (sequencer: map mask), 0x3CE/0x3CF (graphics controller: regs 3
  function, 4 read map select, 8 bit mask), INT 10h (mode/palette/page), INT 21h (alloc/free, direct
  console input), INT 16h AH=1 (key pending), PIT channel 2 via port 0x42 and the speaker gate 0x61.
  No interrupt handlers are installed by the driver; no INT 1Ch / INT 9 hooks.

## 1. Init, mode set, shutdown; the config block

Entry 1 `init(AX=set_mode, BX=&config)` stores the config pointer. If `set_mode != 0`:
INT 10h AX=000Dh (320×200×16, planar, A000:0000, 40 bytes/row, 8000 bytes per plane), INT 10h
AX=1002h with ES:DX = config+0x24 (17-byte palette block), INT 10h AH=05 AL=0 (display page 0).
On the first call it builds the row table and the bit-reverse table. The palette block in DATA.OVL
(DGROUP 0x52de = file 0x52ee) is `00 01 02 03 04 05 06 07 38 39 3A 3B 3C 3D 3E 3F` + overscan `00`:
the **default EGA 16-colour palette** (0 black, 1 blue, 2 green, 3 cyan, 4 red, 5 magenta, 6 brown,
7 light grey, 8 dark grey, 9 light blue, 10 light green, 11 light cyan, 12 light red, 13 light
magenta, 14 yellow, 15 white). Colour values everywhere in the driver are 4-bit indices; bit k of a
colour lives in plane k.

Config block fields the driver touches (DGROUP address = 0x52ba + offset):

| offset | DGROUP | use |
|---|---|---|
| +0x0a | 0x52c4 | clip enable: nonzero → entry 25 clips rows to +0x1a..+0x1c instead of 0..199 |
| +0x1a | 0x52d4 | clip ymin (entry 25 only) |
| +0x1c | 0x52d6 | clip ymax (entry 25 only) |
| +0x1e | 0x52d8 | **draw page** (0/1), written by entry 5, read by 13,16,19,20,21,25,27,31,32(CF=1) |
| +0x24 | 0x52de | 17 palette bytes sent with INT 10h/1002h |

Everything else in the block (cell/tile sizes, pen, clip x) is ROOT-only; the driver receives the
cell/tile sizes as registers (CX/DX/SI/DI) when relevant.

Entry 2 `init_page1()` sets up the off-screen page (EGA: VRAM at A800, cleared; CGA: 16 KB DOS block)
and returns its segment; ROOT keeps it in 0x5354 and uses it as the LZW dictionary while loading
files — i.e. **page 1 is garbage after any file load**. Entry 4 frees it (no-op on EGA), entry 30
frees the tile set. There is no shutdown entry: ROOT's `restore_video_mode` (INT 10h AH=0) restores
the text mode. Entry 0 returns 200 in all four drivers (unused).

## 2. Screen model

* **Page 0** = the visible screen, 320×200, 16 colours. All coordinates are pixels, origin top-left,
  x 0..319, y 0..199, unless an entry says "cells" (8×8, used by text and by the map-view origin) or
  "map cells" (tiles: pixel = (cell_w + 16·col, cell_h + 16·row) with cell_w = cell_h = 8 passed in
  CX/DX, so the 11×11 map view is pixels 8..183 × 8..183).
* **Page 1** = a second 320×200 buffer. On EGA it is VRAM A800:0000..7CFF laid out *plane-separated*
  (plane p at p·8000 + y·40 + x/8, replicated to all 4 hardware planes). The port only needs "a second
  framebuffer"; nothing in the game addresses page 1 memory directly except through the driver.
* **Draw page** (entry 5) selects the target for: scroll (13), pixel (16), hline (19), vline (20),
  fill (21), image blit (25/33), tile (27), glyph (31), effect frame (32 CF=1).
  Fixed targets regardless of draw page: line (17) and get_pixel (12) → page 0; 1-bpp white blit (26)
  → page 1; dissolve (34 CF=0) copies page 1 → page 0; tile dissolve step (34 CF=1) → page 0;
  title effects (35, 37) read page 1 and write page 0.
* Page transfers: 8 (rect, page1→page0 only — the page0→page1 direction is a no-op in EGA.DRV!),
  9 (whole page, either direction), 10 (swap, unused), 34 CF=0 (dissolve page1→page0).
* No driver entry clips horizontally; vertical clipping only in 25 and 16. ROOT clips lines/pixels
  with its own Cohen–Sutherland code before calling. Out-of-range writes in the original land in
  invisible VRAM; the port should just bounds-check.

## 3. Data formats

### 3.1 TILES.16 (entry 24 → tile segment)
On disk: LZW (4-byte size + stream). Decompressed: 65536 bytes = 512 tiles × 128 bytes, no header;
tile row = 16 pixels = 8 bytes packed 4 bpp (high nibble = left pixel). Entry 24 converts the whole
segment in place, every 4-byte group (8 pixels) → `[p0 p1 p2 p3]` plane bytes (MSB = leftmost pixel,
plane k = colour bit k). **Converted tile layout**: tile t at t·0x80, row r at +r·8 =
`p0L p1L p2L p3L p0R p1R p2R p3R` (L = pixels 0–7, R = pixels 8–15). Entries 27, 32, 34(CF=1), 36 all
address tiles this way (e.g. "row 14 of tile 0xc8" = offset 0xc8·0x80 + 14·8). Entry 36 command 4 is
the only code that converts back to packed form.

### 3.2 Multi-image .16 files (entry 22 → used by 25/33)
Header: `u16 count`, then `count` × 4 bytes. Standard files (DNG*, STORY*, TEXT, END*, ENDSC, CREATE,
ULTIMA, STARTSC) have u32 offsets (high word 0). ITEMS.16 / MON0–7.16 have `(u16 image_offset,
u16 mask_offset)` pairs; the driver reads every file as pairs, and a nonzero second word means "mask".
Image = `u16 w, u16 h`, h rows of `bpr = ((w+7)/8)*4` bytes. Packed (on disk): 2 px/byte, high nibble
first, row padded to 8 pixels. **Converted (entry 22, in place, same size)**: each row becomes four
consecutive planes of `bpp = bpr/4 = ceil(w/8)` bytes: plane 0, plane 1, plane 2, plane 3, MSB = left.
Mask image (ITEMS/MON): `u16 w, u16 h`, then h rows of ceil(w/8) bytes, 1 bpp, MSB = left,
**1 = transparent (keep destination), 0 = draw source**. Masks are not converted. (The wiki's
"(width+height+7)/8" mask size is wrong; verified: ITEMS.16 image 0 is 40×80, mask 80×5 bytes.)
Verified dimensions: DNG1.16 has 28 images (24×164, 32×164, 16×164, ...); ULTIMA.16 = 319×61 title +
four 288×49 fire frames; STARTSC.16 = 16×137, 288×137, 16×137; TEXT.16 six 32-row strips.

### 3.3 1-bpp files with u16 offsets (entry 26, entry 35)
`u16 count`, `u16 offset[count]`, image = `u16 w, u16 h`, h rows of `w>>3` bytes (w multiple of 8),
MSB = left, 1 = white pixel, 0 = transparent. TITLE.BIT = 10 images (the 7 title strips 24×3, 40×7,
72×11, 112×20, 152×32, 216×45, 280×61 used by `draw_stacked_pictures`, plus 104×33, 16×15, 112×33);
BRITISH.BIT = one 272×62 image; PROPORT.PCS = 91 glyphs (w ≤ 8, h = 8, one byte per row; index =
char − 0x20); WD.BIT = one 288×49 image (uncompressed; data at byte 8, 36 bytes/row) = the mask for
entry 35.

### 3.4 Fonts (entry 31)
IBM.CH / RUNES.CH: 8 bytes per glyph, one byte per row, MSB = left pixel; ROOT passes ES:DI pointing
at the (possibly temporarily inverted/underlined) glyph. Hercules fonts are 24 bytes/glyph (HER.DRV only).

## 4. Drawing primitives — exact semantics

Colour = `CS:0x312` (entry 15, 0..15). "XOR" = CF=1 on entry: `pixel ^= colour` (planes whose colour
bit is 0 are untouched); otherwise `pixel := colour`.

| entry | name | registers | semantics |
|---|---|---|---|
| 15 | set_color | AL | current colour |
| 16 | plot_pixel | AX=x, BX=y, CF | clipped to the screen (CF=1 returned if outside); draw page |
| 17 | draw_line | AX,BX,CX,DX, CF | Bresenham, inclusive endpoints, **page 0 always**; see jsonl for the exact error-term loop (`err=2·minor−major; err<0 → major step only, err+=2·minor; else both, err+=2·minor−2·major`; dx==dy counts as shallow) |
| 19 | draw_hline | AX=x1, BX=y, CX=x2, CF | inclusive span, any x order; draw page |
| 20 | draw_vline | AX=x, BX=y1, DX=y2, CF | rows y1..y2 (y1≤y2); draw page |
| 21 | fill_rect | AX,BX,CX,DX, CF | rows y1..y2, cols min..max; draw page. Quirk: XOR on page 1 degenerates to an opaque fill (hardware ALU XOR cancels the software XOR) |
| 13 | scroll_rect | AX,BX,CX,DX, SI=±rows | move the rect's contents up (SI<0) or down by |SI| rows, then fill the vacated rows with **colour 0**; draw page. Fast path when AX==192: fixed region x 192..319 / rows 96.., up 8, no clear, colour-mangling MOVSB (pixels with bit 3 set → 15, else 0) — this is what the message window scroll uses; white-on-black text survives it |
| 31 | draw_glyph | AX=cell x, BX=cell y, DL=fg, DH=bg, ES:DI=glyph | opaque 8×8 at (8x, 8y); bit 1 → fg, 0 → bg; draw page; clears the dissolve-sound flag |
| 27 | draw_tile | AL=col, AH=row, BX=tile, CX=cell_w, DX=cell_h, SI=DI=16 | opaque 16×16 at ((CX&~7)+16col, DX+16row); draw page; nothing if SI/DI≠16 |
| 25 | draw_image | AX=seg, BX=index, SI=x (x&~7), DI=y, CX=flags | see §4.1 |
| 33 | draw_image_mirrored | as 25, CX|=2 | horizontal mirror (page 1 only) |
| 26 | draw_bitmap_white | AX=seg, BX=index, SI=x, DI=y | 1-bpp sprite, set bits → colour 15, **page 1 always**, x not byte-aligned, no clipping |
| 12 | get_pixel | AX=x, BX=y → AX | page 0 colour (unused) |
| 8 | copy_rect_page1_to_page0 | AX,BX,CX,DX, CF=1 | pixel-exact rect copy page1→page0; CF=0 (page0→page1) does nothing |
| 9 | copy_page | AX=src, BX=dst | full copy (1→0 or 0→1) |
| 10 | swap_pages | (0,1) or (1,0) | exchange full pages (unused) |

### 4.1 Image blit (entries 25/33) details
Row clipping: rows outside `[0,199]` (or `[clip_ymin, clip_ymax]` when config+0x0a ≠ 0) are skipped
(source rows advanced accordingly). x is used as `x>>3` only. Target = draw page.
* page 0: opaque copy of `bpp` bytes per plane row; flags and mask ignored (a masked image must not be
  drawn to page 0 — the mask path assumes page-1 addressing).
* page 1, plain: copy; if `w % 8 ≠ 0` the last byte keeps the destination pixels beyond `w`.
* page 1 + mask: `dest = (dest & m) | (src & ~m)` per plane, m = mask row byte.
* page 1 + mirror (flag bit 1): bytes bit-reversed and written right-to-left; the result is mirrored
  **and shifted one pixel to the left** (source pixel px → dest x0 + 8·bpp − 2 − px); mirrored data is
  ORed into the destination (not replacing) and bit 0 of the byte to the left of the image is
  cleared/set from the source's last pixel. With a mask: `dest &= mirror(m); dest |= mirror(src) & ~mirror(m)`.
* flag bit 0 (vertical flip): row step −40 starting at row y+h, so the image occupies rows y+1..y+h
  reversed; y+h must be ≤ 199 (index 200 of the row table is the page-1 segment word).
Only DUNGEON uses flags/masks/mirror (walls and ITEMS/MON sprites composed on page 1, then copied).

### 4.2 Effects and transitions
* **34 CF=0 dissolve_rect(x1,y1,x2,y2)**: copies the rect from page 1 to page 0 one pixel at a time in
  LFSR order: `N=w·h`, `b = bitlen(N−1)`, taps = `{2:0x0003,3:0x0006,4:0x000C,5:0x0014,6:0x0030,
  7:0x0060,8:0x00B8,9:0x0110,10:0x0240,11:0x0500,12:0x0CA0,13:0x1B00,14:0x3500,15:0x6000,16:0xB400}[b]`,
  `v=1; loop { x=v%w; y=v/w; if y<h copy pixel; c=v&1; v>>=1; if c v^=taps } until v==1; copy (x1,y1)`.
  Sound/abort only while `CS:0x253d≠0` (before the first glyph ever drawn): every 2nd pixel a noise
  click (§6) and a key poll (INT 16h AH=1) that aborts the dissolve. No delay otherwise — speed is
  whatever the CPU does.
* **34 CF=1 tile_dissolve_step(col,row,tile,step)**: draws one pixel of the tile at the map cell on
  page 0: step 0 → (0,0); step 1 resets the 8-bit LFSR to 1; step≥1: `v=state; pixel (v>>4, v&15);
  state = (v>>1) ^ (v&1 ? 0xB8 : 0)`. ROOT calls it for step 0..255 with key polls every 8 steps.
* **32 CF=1 draw_effect_frame(frame,col,row,BL)**: composes into tile slot 0x116 the base tile (5, or
  0x44 when BL==0xFF) with its last `frame` rows replaced by the first `frame` rows of tile 0xDC, draws
  it with entry 27 (draw page), restores slot 0x116. ROOT runs frame 15→1 with delays.
* **32 CF=0 animate_tiles()**: per-frame in-place mutation of tile bitmaps (noise into sprites
  0x1E8–0x1EB, water tiles 1,2,3,0x8F rotate rows down by one, shoreline tiles 0x60–0x6F / 0x34–0x37 /
  0xE4–0xE7 / 0x108 / 0x1B4 recomposed through plane-3 masks of 0x70–0x7F / 0xD0–0xD3 / 0x100, fire
  tiles 0xB0–0xB3, 0xBC–0xBF, 0xDE XORed with masked noise, random row swaps in tiles 0x12, 0x14, 0x15,
  0x3E, 0x121, 0x123, 0x12D, 0x12F). The full recipe is in the jsonl record; it only needs to be
  reproduced if the animated water/fire should look identical.
* **35 CF=0 title_fire_frame()**: blits 49 rows of page 1 starting at row 50·k (k cycles 0..3) to
  page 0 rows 65..113. **35 CF=1 title_fire_dissolve(AX=WD.BIT seg)**: saves page 1 (32000-byte DOS
  block), clears it, re-reveals pixels (x+16, y+50k for all four frame bands) in 14-bit LFSR order
  (taps 0x3500, `y=v/288, x=v%288, y<49`), first pass where the WD.BIT mask bit is 0, second pass all;
  every 128 pixels (256 if speed<250) it shows the next fire frame, maybe crackles (§6) and waits
  45–50 × speed iterations; a pending key aborts; finally page 1 is restored and the block freed.
* **37 set_speed_and_play_title_zoom(AX=speed)**: stores the speed unit, then animates the 7 TITLE.BIT
  strips (already in page 1 at rows 0,3,10,21,41,73,118, horizontally centred) onto page 0 in bands
  of N rows at rows 75,72,71,66,60,53,46 (centred on ~row 76), writing only planes 0 and 3 (→ colour 9,
  light blue, on black). Script at CS:0x1e8c, per stage `A,B,N,L` + L bytes (row indices, 0xFF = end of
  group → render pass; show phase reads the table backwards, hide phase forwards; even stages drawn
  top-down from B, odd stages bottom-up from B+N i.e. vertically mirrored; render pass = delay 12×speed,
  clear ⌊hidden/2⌋ rows, draw visible rows packed, clear the rest). Key (INT 21h AH=6) aborts: final
  strip drawn at once, AX=1. Script bytes (0x1e8c..0x1f93):
```
07 00 4b 03 0b ff ff ff 01 ff ff ff ff 00 02 ff | 03 48 07 0f ff ff 01 05 ff ff 02 04 ff ff 03 ff 00 06 ff
0a 47 0b 13 ff ff 02 08 ff 03 07 ff 01 09 ff 04 06 ff 05 ff 00 0a ff
15 42 14 1c ff 04 0f ff 01 07 0c 12 ff 05 0e ff 02 08 0b 11 ff 03 06 0d 10 ff 09 0a ff 00 13 ff
29 3c 20 28 ff 07 18 ff 02 0c 13 1d ff 03 08 0d 12 17 1c ff 01 06 0b 14 19 1e ff 04 09 0e 11 16 1b ff 05 0a 0f 10 15 1a ff 00 1f ff
49 35 2d 35 ff 04 0b 12 1a 21 28 ff 01 08 0f 13 24 2b ff 06 0d 14 18 1f 26 ff 03 0a 11 16 1b 22 29 ff 02 05 09 0c 10 13 19 1c 20 23 27 2a ff 07 0e 15 17 1e 25 ff 00 2c ff
76 2e 3d 45 ff 1c 17 12 0d 08 03 20 25 2a 2f 34 39 ff 1a 15 10 0b 06 01 22 27 2c 31 36 3b ff 1d 18 13 0e 09 04 1f 24 29 2e 33 38 ff 1b 16 11 0c 07 02 21 26 2b 30 35 3a ff 19 0f 05 23 2d 37 ff 1e 28 32 14 0a ff 00 3c ff
```
  (the leading `07` is the stage count; each stage = `A B N L` + L bytes.)
* **36 tile_patch(AX=cmd[,BL=hour,BH=minute])**: 0/1 save+replace / restore row 14 of tiles 0xC8 and
  0x117 (with row 0); 2 clock hands in tiles 0xFA/0xFB (ring positions around (8,3), hour mod 12 and
  minute/5, white; ring cleared in planes 0,1,3 so it falls back to red); 3 swap colour bits 1↔2 in a
  list of terrain tiles (involution); 4 irreversible recolour of 22 tiles with the map
  `{1→5,2→4,3→4,4→2,5→1,6→2,9→12,10→12,11→12,12→10,13→9}` (ENDGAME).

## 5. Palette / colour handling
Fixed default EGA palette (see §1). No palette animation anywhere; the only "colour effects" are the
tile-set manipulations of entry 36 and the plane-mask tricks of entries 37 (planes 0+3 → colour 9)
and 13's fast path. The port can hard-code the 16 standard EGA RGB values.

## 6. Sound, timing, keyboard inside the driver
* Delay unit = `CS:0x1c1e` (= ROOT's `g_cpu_speed`, "loop iterations per 55 ms tick × 18 / 750"),
  set only by entry 37 (first call = title screen); a busy loop of `n × speed` iterations ≈ n × (55 ms
  × 750/18 / iterations-per-tick)… in practice ROOT calibrates so that `speed` iterations ≈ 4 ms
  (55 ms·18/750 = 1.32 ms per "750 iterations"; treat `speed` as "about 1.3 ms × 3"). The port should
  simply use wall-clock delays: entry 37 render pass ≈ 12 units, entry 35 frame ≈ 45–50 units, noise
  click ≈ 1/16 unit.
* Noise routine FUN_27af (used by 34 CF=0 and 35): speaker gate on (`port 61h |= 3`); repeat
  `0x27a9` times: `f = 100 + rand % (0x27a7/2 − 100 + 1)` Hz, PIT ch.2 reload `1193182/f` (lo,hi to
  port 42h), wait `speed>>4` iterations; then the caller gates the speaker off (`&= ~3`). Dissolve:
  `0x27a7` starts at 240 and increments per click, `0x27a9 = 1`; title: `0x27a7 = 3000, 0x27a9 = 25`.
  Nothing else in the driver makes sound; the game's music/SFX are elsewhere (ROOT/overlays).
* Keyboard: entry 34 CF=0 and 35 poll INT 16h AH=1 (key pending → abort, key left in the buffer);
  entry 37 polls INT 21h AH=06h DL=FF (consumes the key, and a second byte for extended keys) and
  returns AX=1 when aborted.
* No timer interrupt is used by the driver.

## 7. Recommended Rust `Driver` trait

```rust
pub type Color = u8;                       // 0..15, default EGA palette
pub enum Page { Front = 0, Back = 1 }

pub struct TileSet { pub tiles: Vec<[u8; 256]> }   // 512 tiles, pixel-per-byte (converted from TILES.16)
pub struct Image  { pub w: u16, pub h: u16, pub pixels: Vec<u8>, pub mask: Option<Vec<u8>> } // mask: 1 = transparent
pub struct Bitmap { pub w: u16, pub h: u16, pub bits: Vec<u8> }   // 1 bpp rows of w/8 bytes, MSB left
pub struct ImageFile { pub images: Vec<Option<Image>> }           // index = entry 25 BX
pub struct BitmapFile { pub images: Vec<Bitmap> }                 // TITLE.BIT / PROPORT.PCS / BRITISH.BIT / WD.BIT

pub trait Driver {
    // entries 1, 2, 5
    fn init(&mut self);                                   // mode + palette; both pages black
    fn set_draw_page(&mut self, page: Page);
    // 15, 16, 17, 19, 20, 21, 13
    fn set_color(&mut self, c: Color);
    fn plot_pixel(&mut self, x: i16, y: i16, xor: bool);  // ignores points off-screen
    fn draw_line(&mut self, x1: i16, y1: i16, x2: i16, y2: i16, xor: bool);  // always Front page
    fn draw_hline(&mut self, x1: i16, y: i16, x2: i16, xor: bool);
    fn draw_vline(&mut self, x: i16, y1: i16, y2: i16, xor: bool);
    fn fill_rect(&mut self, x1: i16, y1: i16, x2: i16, y2: i16, xor: bool);   // inclusive
    fn scroll_rect(&mut self, x1: i16, y1: i16, x2: i16, y2: i16, rows: i16); // <0 up, >0 down; vacated rows := 0
    // 8, 9
    fn copy_rect_back_to_front(&mut self, x1: i16, y1: i16, x2: i16, y2: i16);
    fn copy_page(&mut self, src: Page, dst: Page);
    // 31, 27, 25/33, 26
    fn draw_glyph(&mut self, cell_x: u8, cell_y: u8, fg: Color, bg: Color, glyph: &[u8; 8]);
    fn draw_tile(&mut self, col: u8, row: u8, tile: u16, cell_w: u8, cell_h: u8);   // uses the registered tile set
    fn draw_image(&mut self, file: &ImageFile, index: usize, x: i16, y: i16, vflip: bool, mirror: bool);
    fn draw_bitmap_white(&mut self, file: &BitmapFile, index: usize, x: i16, y: i16); // always Back page
    // 24, 30, 36, 32 CF=0
    fn load_tileset(&mut self, tiles: TileSet);          // entry 24 (decode TILES.16 directly)
    fn free_tileset(&mut self);
    fn tile_patch(&mut self, cmd: u8, hour: u8, minute: u8);
    fn animate_tiles(&mut self);
    // effects: 32 CF=1, 34, 35, 37.  `poll_key` is the abort test; `delay(units)` = n × speed.
    fn draw_effect_frame(&mut self, frame: u8, col: u8, row: u8, dungeon: bool, cell_w: u8, cell_h: u8);
    fn dissolve_rect(&mut self, x1: i16, y1: i16, x2: i16, y2: i16);                    // Back → Front, LFSR order
    fn tile_dissolve_step(&mut self, col: u8, row: u8, tile: u16, step: u8, cell_w: u8, cell_h: u8);
    fn title_fire_frame(&mut self);
    fn title_fire_dissolve(&mut self, mask: &Bitmap) -> bool;                            // true = aborted by key
    fn title_zoom(&mut self, speed: u16) -> bool;                                        // entry 37
}
```
Mapping notes for the ROOT wrappers (register → argument): `set_color(AL)`, `fill_rect(AX,BX,CX,DX,
CF)`, `copy_rect_between_pages(SI=src,DI=dst,AX..DX)` → only (1,0) does anything, `draw_image(AX=seg,
BX=index,SI=x,DI=y,CX=mode)` → `vflip = mode&1`, `mirror = mode&2 || entry 33`, `draw_tile(AX=row<<8|col,
BX=tile,CX,DX,SI,DI)`, `draw_effect_frame(AX=frame,SI=col,DI=row,BL=g_input_mode,CX,DX)`, `tile_transition_
effect` → `tile_dissolve_step` for DI = 0..255, `driver_set_speed(AX)` → `title_zoom`, put_char →
`draw_glyph` + `scroll_rect(x1,y1,x2,y2,-8)` + `fill_rect` (clear) with `set_color`.
Entries 22 and 24 become file decoders in the port (keep the packed data; decode to pixel-per-byte).
Entries 0, 3, 4, 6, 7, 10, 11, 12, 14, 18, 23, 28, 29 need no implementation (stubs or unused).

## 8. Open points / things to verify in DOSBox
1. Entry 13 fast path (AX=192): whether non-white message text is really whitened on scroll (hardware
   analysis says yes: MOVSB under map mask 0Fh writes plane-3 data to all planes).
2. Entry 37's odd stages are drawn bottom-up from the band end (vertically mirrored) and in colour 9 —
   confirm the look of the title zoom before polishing.
3. Entry 25 vertical flip starting at row y+h (one row lower than expected) — check how DUNGEON uses it.
4. Entry 8 CF=0 being a no-op: callers that pass src=0,dst=1 (COMSUBS f4be, DUNGEON 85a6/9db0, INTRO,
   LOOKOBJ b38c, ROOT 3072) should be checked for which direction they really use.
5. `g_cpu_speed` to milliseconds: calibrate the delay unit against DOSBox rather than trusting the formula.
