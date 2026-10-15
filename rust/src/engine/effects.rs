//! Driver title effects (entries 35 and 37), reimplemented per analysis/driver_api.md.
use crate::game::Game;
use crate::g;

fn speed_ms(gm: &Game, units: f64) -> u64 { (units * gm.g16(g::CPU_SPEED) as f64 * crate::engine::util::iter_us(gm) / 1000.0) as u64 }

/// Entry 35 CF=0: copy the next 49-row fire frame band from page 1 to page 0 rows 65..113.
pub fn title_fire_frame(gm: &mut Game) {
    let k = gm.pf.video.fire_frame as i32;
    gm.pf.video.fire_frame = ((k + 1) % 4) as u8;
    for r in 0..49 { for x in 0..320 {
        let v = gm.pf.video.get_on(1, x, 50 * k + r);
        gm.pf.video.pages[0][(65 + r) as usize * 320 + x as usize] = v;
    } }
    gm.pf.video.dirty = true;
}
/// Entry 35 CF=1: the burning-title dissolve. `mask_seg` = segment of the decompressed WD.BIT. Returns true if aborted.
pub fn title_fire_dissolve(gm: &mut Game, mask_seg: u16) -> bool {
    let mask = ((mask_seg as u32) << 4) + 8; // 49 rows of 36 bytes
    let saved = gm.pf.video.pages[1].clone();
    gm.pf.video.pages[1].iter_mut().for_each(|p| *p = 0);
    let slow = gm.g16(g::CPU_SPEED) < 250;
    let per = if slow { 256 } else { 128 };
    // CS:0x28c2 is the driver's crackle LCG seed (init 0x7664, advanced once per frame); the
    // recording of the original shows the crackles thinning out over each pass.
    let mut aborted = false;
    'passes: for pass in 0..2 {
        let mut crackle: i32 = 400;
        let mut v: u32 = 1; let mut count = 0u32;
        loop {
            let y = v / 288; let x = v % 288;
            if y < 49 {
                let mbit = gm.m.u8(mask + y * 36 + x / 8) & (0x80 >> (x & 7)) != 0;
                if pass == 1 || !mbit {
                    for k in 0..4 { let (px, py) = ((x + 16) as usize, (y + 50 * k) as usize); gm.pf.video.pages[1][py * 320 + px] = saved[py * 320 + px]; }
                }
                count += 1;
                if count % per == 0 {
                    title_fire_frame(gm);
                    crackle -= 3;
                    let fs = &mut gm.pf.video.fire_seed;
                    *fs = (fs.wrapping_add(0x9248).rotate_right(3) ^ 0x9248).wrapping_add(0x11);
                    if ((*fs & 0x1ff) as i32) < crackle {
                        // Crackle (noise routine FUN_27af): 25 random pitches in [100,1500] Hz, each
                        // held speed>>4 iterations, then the speaker is gated off for the 45-unit wait.
                        if gm.g16(g::SOUND_ENABLED) != 0 {
                            let iu = crate::engine::util::iter_us(gm);
                            let click_us = ((gm.g16(g::CPU_SPEED) >> 4) as f64 + 15.0) * iu;
                            let mut sy = crate::platform::sound::Synth::new();
                            for _ in 0..25 {
                                let seed = &mut gm.pf.video.noise_seed; // CS:0x27a5, persists across calls
                                *seed = (seed.wrapping_add(0x9248).rotate_right(3) ^ 0x9248).wrapping_add(0x11);
                                sy.tone((100 + *seed as u32 % 1401) as f64, click_us);
                            }
                            gm.pf.speaker.play_samples(&sy.samples);
                        }
                        gm.pf.sleep_ms(speed_ms(gm, 45.0).max(1));
                    } else { gm.pf.sleep_ms(speed_ms(gm, 50.0).max(1)); }
                    if gm.pf.key_available().is_some() || gm.pf.quit_requested { aborted = true; break 'passes; }
                }
            }
            let c = v & 1; v >>= 1; if c != 0 { v ^= 0x3500; }
            if v == 1 { break; }
        }
    }
    gm.pf.video.pages[1].copy_from_slice(&saved);
    gm.pf.pump();
    aborted
}
/// Entry 37: store the speed unit and play the title zoom from the 7 TITLE.BIT strips on page 1.
/// The per-stage script is read from the EGA.DRV file (CS:0x1e8c..0x1f94). Returns true if aborted.
pub fn title_zoom(gm: &mut Game) -> bool {
    gm.pf.video.speed = gm.g16(g::CPU_SPEED);
    let drv = match gm.read_file("EGA.DRV") { Some(d) => d, None => return false };
    if drv.len() < 0x1f94 { return false; }
    let script = &drv[0x1e8c..0x1f94];
    let stages = script[0] as usize;
    let mut p = 1usize;
    let heights = [3usize, 7, 11, 20, 32, 45, 61];
    let full = (118usize, 46usize, 61usize);
    let mut aborted = false;
    'outer: for stage in 0..stages {
        let (a, b, n, l) = (script[p] as usize, script[p + 1] as usize, script[p + 2] as usize, script[p + 3] as usize);
        let rows = &script[p + 4..p + 4 + l];
        p += 4 + l;
        let mirrored = stage & 1 == 1;
        let mut visible = vec![false; n.max(heights.get(stage).copied().unwrap_or(n))];
        // show phase: read the table backwards
        let mut groups: Vec<Vec<u8>> = Vec::new(); let mut cur = Vec::new();
        for &r in rows { if r == 0xff { groups.push(std::mem::take(&mut cur)); } else { cur.push(r); } }
        if !cur.is_empty() { groups.push(cur); }
        for grp in groups.iter().rev() {
            for &r in grp.iter().rev() { if (r as usize) < visible.len() { visible[r as usize] = true; } }
            if render_pass(gm, a, b, n, &visible, mirrored) { aborted = true; break 'outer; }
        }
        if stage + 1 == stages { break; }
        for grp in groups.iter() {
            for &r in grp.iter() { if (r as usize) < visible.len() { visible[r as usize] = false; } }
            if render_pass(gm, a, b, n, &visible, mirrored) { aborted = true; break 'outer; }
        }
    }
    if aborted {
        let visible = vec![true; full.2];
        let _ = render_pass_nokey(gm, full.0, full.1, full.2, &visible, false);
    }
    gm.pf.pump();
    aborted
}
fn render_pass(gm: &mut Game, a: usize, b: usize, n: usize, visible: &[bool], mirrored: bool) -> bool {
    let ms = speed_ms(gm, 12.0);
    gm.pf.sleep_ms(ms.max(1));
    render_pass_nokey(gm, a, b, n, visible, mirrored);
    gm.pf.key_available().is_some() || gm.pf.quit_requested
}
fn render_pass_nokey(gm: &mut Game, a: usize, b: usize, n: usize, visible: &[bool], mirrored: bool) {
    let shown: Vec<usize> = (0..n).filter(|&r| visible.get(r).copied().unwrap_or(false)).collect();
    let hidden = n - shown.len();
    let clear = hidden / 2;
    for i in 0..n {
        let dest = if mirrored { b + n - 1 - i } else { b + i };
        if dest >= 200 { continue; }
        let src = if i >= clear && i - clear < shown.len() { Some(a + shown[i - clear]) } else { None };
        for x in 0..320usize {
            let v = match src { Some(sr) if sr < 200 => { let c = gm.pf.video.pages[1][sr * 320 + x]; (c & 1) | (c & 8) } _ => 0 };
            gm.pf.video.pages[0][dest * 320 + x] = v;
        }
    }
    gm.pf.video.dirty = true;
}
