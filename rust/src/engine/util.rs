//! ROOT utilities: RNG, timing, PC-speaker sound, string helpers.
use crate::game::Game;
use crate::g;

/// Nominal CPU speed factor: what calibrate_timer measures under the GOG DOSBox profile
/// (cycles=5000: ~3 instructions per calibration-loop iteration → ~91,000 iterations per 55 ms tick
/// → 91000*18/750).
pub const NOMINAL_CPU_SPEED: u16 = 2200;
/// Microseconds per iteration of the engine's `DEC mem / JNZ` busy loops at that speed: speed
/// iterations take 0.88 ms (the calibration loop is 1.5x the cost of the delay loop).
pub fn iter_us(gm: &Game) -> f64 { 880.0 / gm.g16(g::CPU_SPEED).max(1) as f64 }

/// U5_CPU_SPEED overrides the measured CPU speed factor (bigger = shorter busy-wait delays and
/// sound effects; the original's 18.2 Hz frame ticks are unaffected).
pub fn calibrate_timer(gm: &mut Game) {
    let speed = std::env::var("U5_CPU_SPEED").ok().and_then(|v| v.parse::<u16>().ok()).unwrap_or(NOMINAL_CPU_SPEED);
    gm.set_g16(g::CPU_SPEED, speed.max(0x100)); gm.set_g16(g::MEM_KB, 640);
}
pub fn init_system(gm: &mut Game) {
    crate::engine::text::init_text_windows(gm);
    crate::engine::text::reset_text_attrs(gm);
    calibrate_timer(gm);
}
/// Sleep for `iters` busy-loop iterations of the original.
pub fn delay_iterations(gm: &mut Game, iters: f64) {
    let us = iters * iter_us(gm);
    if us >= 200.0 { gm.pf.sleep_us(us as u64); } else { gm.pf.maybe_present(); }
}
pub fn get_time_hash(_gm: &mut Game) -> u16 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
    let secs = now.as_secs(); let hund = (now.subsec_millis() / 10) as u16;
    let (h, m, s) = (((secs / 3600) % 24) as u16, ((secs / 60) % 60) as u16, (secs % 60) as u16);
    (((h << 1) << 8 | (m << 2)).wrapping_add((s << 3) << 8 | hund) ^ 0x91eb) & 0xfff
}
pub fn set_random_seed(gm: &mut Game, seed: u16) { gm.set_g16(g::RAND_SEED, seed); }
/// rand_range(lo,hi) — Ghidra order (hi,lo). Bit-exact game RNG.
pub fn rand_range(gm: &mut Game, hi: i16, lo: i16) -> i16 {
    let s = gm.g16(g::RAND_SEED);
    let s = (s.wrapping_add(0x9248).rotate_right(3) ^ 0x9248).wrapping_add(0x11);
    gm.set_g16(g::RAND_SEED, s);
    let span = (hi as i32 - lo as i32 + 1) as u16;
    if span == 0 { return lo; }
    (((s & 0x7fff) % span) as i16).wrapping_add(lo)
}
/// delay_loop(speed_index,count) — Ghidra order (count,speed_index): count × g_cpu_speed iterations.
pub fn delay_loop(gm: &mut Game, count: i16, _speed_index: i16) {
    let speed = gm.g16(g::CPU_SPEED) as f64;
    delay_iterations(gm, (count as u16) as f64 * speed);
}
/// wait_ticks(ticks): frame timer, 18.2 Hz ticks.
pub fn wait_ticks(gm: &mut Game, ticks: i16) {
    if ticks == 1 && gm.g16(g::CPU_SPEED) <= 0xf0 { return; }
    gm.pf.sleep_ms((ticks.max(0) as u64) * 55);
}
pub fn start_tone(gm: &mut Game, freq: u16) { if gm.g16(g::SOUND_ENABLED) != 0 { gm.pf.speaker.tone_on(freq.max(19) as u32); } }
pub fn stop_tone(gm: &mut Game) -> u8 { gm.pf.speaker.tone_off(); 0 }
/// Play a synthesised waveform and block for its duration (the original blocks while it plays).
fn play_synth(gm: &mut Game, synth: &crate::platform::sound::Synth) {
    let ms = synth.duration_ms();
    if gm.g16(g::SOUND_ENABLED) != 0 { gm.pf.speaker.play_samples(&synth.samples); }
    gm.pf.sleep_us((ms * 1000.0) as u64);
}
/// beep(freq,duration) — Ghidra order (duration,freq): tone for duration × g_cpu_speed iterations.
pub fn beep(gm: &mut Game, duration: i16, freq: u16) {
    let mut sy = crate::platform::sound::Synth::new();
    let us = (duration as u16) as f64 * gm.g16(g::CPU_SPEED) as f64 * iter_us(gm);
    sy.tone(freq.max(19) as f64, us);
    play_synth(gm, &sy);
}
/// play_random_tones(step,total,max_freq) — Ghidra order (max_freq,total,step): burst of random-pitch
/// tones, each lasting step × (g_cpu_speed >> 4) loop iterations (plus ~30 instructions of overhead).
pub fn play_random_tones(gm: &mut Game, max_freq: i16, total: i16, step: i16) {
    gm.set_gi16(g::NOISE_ELAPSED, 0);
    let per_iters = (gm.g16(g::CPU_SPEED) >> 4) as f64;
    let iu = iter_us(gm);
    let mut sy = crate::platform::sound::Synth::new();
    loop {
        let s = gm.g16(g::NOISE_SEED);
        let s = (s.wrapping_add(0x9248).rotate_right(3) ^ 0x9248).wrapping_add(0x11);
        gm.set_g16(g::NOISE_SEED, s);
        let span = (max_freq as i32 - 99).max(1) as u16;
        let freq = 100 + (s % span) as u32;
        sy.tone(freq as f64, (step.max(1) as f64 * per_iters + 15.0) * iu);
        let e = gm.gi16(g::NOISE_ELAPSED).wrapping_add(step); gm.set_gi16(g::NOISE_ELAPSED, e);
        if e >= total || step <= 0 { break; }
    }
    play_synth(gm, &sy);
}
/// play_sweep_noise(phase_step,hold,count,threshold,threshold_step) — Ghidra order
/// (threshold_step,threshold,count,hold,phase_step). Software PWM: each iteration opens the speaker
/// gate when phase > threshold, then busy-waits hold × (g_cpu_speed/24) iterations.
pub fn play_sweep_noise(gm: &mut Game, threshold_step: u16, mut threshold: u16, count: u16, hold: i16, phase_step: u16) {
    let speed = gm.g16(g::CPU_SPEED);
    let unit = if speed < 100 { 0 } else { speed / 24 };
    gm.set_g16(g::SND_DELAY_UNIT, unit);
    let iu = iter_us(gm);
    let per_us = ((hold as u16) as f64 * unit as f64 + 12.0) * iu; // loop body ≈ 12 instructions
    let mut phase: u16 = 0;
    let mut sy = crate::platform::sound::Synth::new();
    for _ in 0..count {
        phase = phase.wrapping_add(phase_step);
        sy.level(phase > threshold, per_us);
        threshold = threshold.wrapping_add(threshold_step);
    }
    play_synth(gm, &sy);
}
/// Linear frequency glide (ROOT 43ae): start_tone(freq); delay_loop(step); freq += delta*step/total.
pub fn tone_sweep(gm: &mut Game, total: i16, step: i16, end_freq: i16, start_freq: i16) {
    let delta = end_freq.wrapping_sub(start_freq);
    let mut freq = start_freq;
    let seg_us = (step as u16) as f64 * gm.g16(g::CPU_SPEED) as f64 * iter_us(gm);
    let mut sy = crate::platform::sound::Synth::new();
    let mut i: i16 = 0;
    while i < total {
        sy.tone((freq as u16).max(19) as f64, seg_us);
        freq = freq.wrapping_add(delta.wrapping_mul(step).wrapping_div(total.max(1)));
        i = i.wrapping_add(step.max(1));
    }
    play_synth(gm, &sy);
}
pub fn to_upper(c: i16) -> i16 { if (b'a' as i16..=b'z' as i16).contains(&c) { c - 0x20 } else { c } }
pub fn str_len(gm: &Game, s: u32) -> i16 { gm.m.strlen(s) as i16 }
/// animate_wait(frames): pause while keeping the map animating.
pub fn animate_wait(gm: &mut Game, frames: i16) {
    if gm.g8(g::MAP_UPDATE_ENABLED) != 0 && frames > 0 {
        for _ in 0..frames { crate::engine::view::refresh_map_view(gm); wait_ticks(gm, 1); }
    }
}
