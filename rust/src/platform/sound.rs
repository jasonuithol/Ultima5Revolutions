//! PC speaker emulation. Two sources feed one mono 22050 Hz stream on a background thread:
//! a queue of pre-synthesised sample buffers (sample-accurate effects: random-tone bursts,
//! software-PWM sweeps, glides, beeps) and, when the queue is empty, a continuous square wave at
//! `freq` Hz (for code that calls start_tone/stop_tone around its own delays).
//! Output goes through `paplay` (PipeWire's PulseAudio shim), `aplay` or `pw-play` via stdin.
use std::collections::VecDeque;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

pub const RATE: u32 = 22050;
pub const AMP: i16 = 6000;

pub struct Speaker {
    freq: Arc<AtomicU32>,
    queue: Arc<Mutex<VecDeque<i16>>>,
    _thread: Option<std::thread::JoinHandle<()>>,
}

impl Speaker {
    pub fn new() -> Speaker {
        let freq = Arc::new(AtomicU32::new(0));
        let queue: Arc<Mutex<VecDeque<i16>>> = Arc::new(Mutex::new(VecDeque::new()));
        let (f2, q2) = (freq.clone(), queue.clone());
        let thread = std::thread::spawn(move || {
            let spawn = |cmd: &str, args: &[&str]| Command::new(cmd).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok();
            let mut child = spawn("paplay", &["--raw", "--rate=22050", "--format=s16le", "--channels=1", "--latency-msec=30"])
                .or_else(|| spawn("aplay", &["-q", "-t", "raw", "-f", "S16_LE", "-r", "22050", "-c", "1"]))
                .or_else(|| spawn("pw-play", &["--rate", "22050", "--format", "s16", "--channels", "1", "-"]));
            let mut stdin = match child.as_mut().and_then(|c| c.stdin.take()) { Some(s) => s, None => return };
            let mut phase: f64 = 0.0;
            let chunk = RATE as usize / 100; // 10 ms
            let mut buf = vec![0u8; 2 * chunk];
            // Pace generation on the wall clock: stay at most LEAD ahead of real time so that
            // queued effects start promptly instead of behind a backlog of pre-generated audio.
            let start = std::time::Instant::now();
            let lead = std::time::Duration::from_millis(40);
            let mut written: u64 = 0;
            loop {
                let due = start + std::time::Duration::from_millis(written * 10);
                let now = std::time::Instant::now();
                if due > now + lead { std::thread::sleep(due - now - lead); }
                written += 1;
                let f = f2.load(Ordering::Relaxed);
                let mut q = q2.lock().unwrap();
                for i in 0..chunk {
                    let s: i16 = if let Some(v) = q.pop_front() { v } else if f == 0 { 0 } else {
                        phase += f as f64 / RATE as f64;
                        if phase >= 1.0 { phase -= 1.0; }
                        if phase < 0.5 { AMP } else { -AMP }
                    };
                    buf[2 * i] = (s as u16 & 0xff) as u8;
                    buf[2 * i + 1] = (s as u16 >> 8) as u8;
                }
                drop(q);
                if stdin.write_all(&buf).is_err() { return; }
            }
        });
        Speaker { freq, queue, _thread: Some(thread) }
    }
    pub fn tone_on(&self, hz: u32) { self.freq.store(hz, Ordering::Relaxed); }
    pub fn tone_off(&self) { self.freq.store(0, Ordering::Relaxed); }
    /// Queue a pre-synthesised waveform (played before/instead of the continuous tone).
    pub fn play_samples(&self, samples: &[i16]) {
        if let Ok(path) = std::env::var("U5_DUMP_AUDIO") {
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                let mut bytes = Vec::with_capacity(samples.len() * 2 + 4);
                bytes.extend_from_slice(&(samples.len() as u32).to_le_bytes());
                for s in samples { bytes.extend_from_slice(&s.to_le_bytes()); }
                let _ = f.write_all(&bytes);
            }
        }
        self.queue.lock().unwrap().extend(samples.iter().copied());
    }
    pub fn divisor_to_hz(div: u16) -> u32 { if div == 0 { 18 } else { 1_193_182 / div as u32 } }
}

/// Helper to build speaker waveforms sample by sample with fractional-sample timing.
pub struct Synth {
    pub samples: Vec<i16>,
    acc: f64,      // fractional samples carried over
    phase: f64,
}
impl Synth {
    pub fn new() -> Synth { Synth { samples: Vec::new(), acc: 0.0, phase: 0.0 } }
    /// Square wave at `hz` for `us` microseconds (hz = 0 → silence).
    pub fn tone(&mut self, hz: f64, us: f64) {
        let n = us * RATE as f64 / 1_000_000.0 + self.acc;
        let count = n.floor() as usize; self.acc = n - count as f64;
        for _ in 0..count {
            if hz <= 0.0 { self.samples.push(0); continue; }
            self.phase += hz / RATE as f64;
            if self.phase >= 1.0 { self.phase -= 1.0; }
            self.samples.push(if self.phase < 0.5 { AMP } else { -AMP });
        }
    }
    /// Constant level (speaker gate held on/off) for `us` microseconds.
    pub fn level(&mut self, on: bool, us: f64) {
        let n = us * RATE as f64 / 1_000_000.0 + self.acc;
        let count = n.floor() as usize; self.acc = n - count as f64;
        for _ in 0..count { self.samples.push(if on { AMP } else { -AMP }); }
    }
    pub fn duration_ms(&self) -> f64 { self.samples.len() as f64 * 1000.0 / RATE as f64 }
}
