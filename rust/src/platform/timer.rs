//! DOS timer tick emulation (18.2 Hz BIOS tick).
use std::time::Instant;

pub struct Timer {
    start: Instant,
}
impl Timer {
    pub fn new() -> Timer { Timer { start: Instant::now() } }
    /// Ticks since start at 18.2065 Hz.
    pub fn ticks(&self) -> u32 {
        let secs = self.start.elapsed().as_secs_f64();
        (secs * 18.2065) as u32
    }
}
