//! Platform layer: replaces DOS/BIOS/the EGA.DRV driver with a framebuffer window, key queue,
//! wall-clock timer and a software PC speaker.
pub mod video;
pub mod input;
pub mod sound;
pub mod timer;

use minifb::{Window, WindowOptions};
use std::time::{Duration, Instant};

pub struct Platform {
    pub window: Option<Window>,
    pub video: video::Video,
    pub keys: input::KeyQueue,
    pub speaker: sound::Speaker,
    pub timer: timer::Timer,
    pub scale: usize,
    rgb: Vec<u32>,
    last_present: Instant,
    last_poll: Instant,
    pub quit_requested: bool,
    /// Headless mode (no window): used for automated tests/screenshots.
    pub headless: bool,
    /// Scripted input for automated runs: (time since start in ms, BIOS key code).
    pub script: Vec<(u64, u16)>,
    pub start: Instant,
    /// Automated run: write a screenshot here and exit after `auto_exit_ms`.
    pub auto_exit_ms: Option<u64>,
    pub shot_path: Option<String>,
    shots_taken: u32,
}

impl Platform {
    pub fn new(headless: bool) -> Platform {
        let scale = 3;
        let window = if headless { None } else {
            let mut w = Window::new("Ultima V", video::SCREEN_W * scale, video::SCREEN_H * scale, WindowOptions::default()).expect("window");
            w.set_target_fps(0); // we pace presentation ourselves
            Some(w)
        };
        let mut pf = Platform { window, video: video::Video::new(), keys: input::KeyQueue::new(), speaker: sound::Speaker::new(),
            timer: timer::Timer::new(), scale, rgb: Vec::new(), last_present: Instant::now(), last_poll: Instant::now(), quit_requested: false, headless,
            script: Vec::new(), start: Instant::now(), auto_exit_ms: None, shot_path: None, shots_taken: 0 };
        // U5_KEYS="1000:J,2500:ENTER,..." = scripted keys at ms offsets; U5_EXIT_MS, U5_SHOT=path.ppm
        if let Ok(spec) = std::env::var("U5_KEYS") {
            for item in spec.split(',') {
                if let Some((t, k)) = item.split_once(':') {
                    if let (Ok(t), Some(code)) = (t.trim().parse::<u64>(), input::parse_key_name(k.trim())) { pf.script.push((t, code)); }
                }
            }
            pf.script.sort();
        }
        pf.auto_exit_ms = std::env::var("U5_EXIT_MS").ok().and_then(|v| v.parse().ok());
        pf.shot_path = std::env::var("U5_SHOT").ok();
        pf
    }
    fn run_script(&mut self) {
        let now = self.start.elapsed().as_millis() as u64;
        while let Some(&(t, k)) = self.script.first() {
            if t > now { break; }
            self.keys.queue.push_back(k);
            self.script.remove(0);
        }
        if let Some(ms) = self.auto_exit_ms {
            if now >= ms {
                if let Some(p) = self.shot_path.clone() { let _ = self.screenshot_ppm(&p); eprintln!("[u5] screenshot {p}"); }
                std::process::exit(0);
            }
        }
        if let (Some(p), Some(ms)) = (self.shot_path.clone(), self.auto_exit_ms) {
            // periodic shots every quarter of the run for debugging: path with suffix
            let q = (now * 4 / ms.max(1)) as u32;
            if q > self.shots_taken && q < 4 {
                self.shots_taken = q;
                let _ = self.screenshot_ppm(&p.replace(".ppm", &format!("_{q}.ppm")));
            }
        }
    }
    /// Present page 0 if dirty and process window events. Call from every wait/poll point.
    pub fn pump(&mut self) {
        self.run_script();
        if let Some(w) = self.window.as_mut() {
            if !w.is_open() {
                // The window's close button quits the game (like killing the original; use the
                // in-game Quit & Save command to save first).
                self.quit_requested = true;
                self.speaker.tone_off();
                std::process::exit(0);
            }
            // Present at most ~60 times a second: the game pumps on every key poll, including
            // inside pixel-by-pixel drawing loops, and a present must never block those.
            let since = self.last_present.elapsed();
            if (self.video.dirty && since >= Duration::from_millis(15)) || since > Duration::from_millis(100) {
                self.video.render_rgb(&mut self.rgb, self.scale);
                let _ = w.update_with_buffer(&self.rgb, video::SCREEN_W * self.scale, video::SCREEN_H * self.scale);
                self.video.dirty = false;
                self.last_present = Instant::now();
            } else if self.last_poll.elapsed() >= Duration::from_millis(4) {
                w.update();
            } else {
                return;
            }
            self.last_poll = Instant::now();
            self.keys.poll(w);
        }
    }
    /// Present only if enough time passed (cheap to call from drawing loops).
    pub fn maybe_present(&mut self) {
        if self.last_present.elapsed() > Duration::from_millis(16) { self.pump(); }
    }
    /// Sleep for `ms` milliseconds while keeping the window alive.
    pub fn sleep_ms(&mut self, ms: u64) { self.sleep_us(ms * 1000); }
    /// Sleep for `us` microseconds while keeping the window alive.
    pub fn sleep_us(&mut self, us: u64) {
        let end = Instant::now() + Duration::from_micros(us);
        loop {
            self.pump();
            let now = Instant::now();
            if now >= end { break; }
            std::thread::sleep((end - now).min(Duration::from_millis(8)));
        }
    }
    /// BIOS INT 16h AH=1 equivalent (also pumps the window).
    pub fn key_available(&mut self) -> Option<u16> { self.pump(); self.keys.available() }
    /// Blocking key read (INT 16h AH=0), pumping the window and calling `idle` every ~16 ms.
    pub fn wait_key<F: FnMut(&mut Platform)>(&mut self, mut idle: F) -> u16 {
        loop {
            self.pump();
            if let Some(k) = self.keys.take() { return k; }
            if self.quit_requested { return 0x011b; }
            idle(self);
            std::thread::sleep(Duration::from_millis(8));
        }
    }
    /// Save page 0 as a PPM file (for automated screenshots).
    pub fn screenshot_ppm(&self, path: &str) -> std::io::Result<()> {
        let mut out = format!("P6\n{} {}\n255\n", video::SCREEN_W, video::SCREEN_H).into_bytes();
        for &c in &self.video.pages[0] {
            let rgb = video::EGA_PALETTE[(c & 15) as usize];
            out.push((rgb >> 16) as u8); out.push((rgb >> 8) as u8); out.push(rgb as u8);
        }
        std::fs::write(path, out)
    }
}
