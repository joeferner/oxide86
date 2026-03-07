use std::time::{Duration, Instant};

/// Tracks CPU cycle throttling against wall-clock time to emulate a target clock speed.
///
/// Usage patterns:
/// - Event-driven (GUI): call `target_cycles()` each frame and run the CPU until it reaches
///   that value, capped by a per-frame maximum to keep the UI responsive.
/// - Polling (CLI): after each batch call `sync()` to sleep if running ahead of real time.
pub struct CpuThrottle {
    clock_speed: u32,
    start: Instant,
    base_cycles: u64,
}

impl CpuThrottle {
    pub fn new(clock_speed: u32, current_cycles: u64) -> Self {
        Self {
            clock_speed,
            start: Instant::now(),
            base_cycles: current_cycles,
        }
    }

    /// Reset the reference point to `current_cycles` now.
    /// Call this after a pause or other interruption to avoid burst catch-up.
    pub fn reset(&mut self, current_cycles: u64) {
        self.start = Instant::now();
        self.base_cycles = current_cycles;
    }

    /// Returns the cycle count we should have reached by now given elapsed wall time.
    pub fn target_cycles(&self) -> u64 {
        let elapsed = self.start.elapsed().as_secs_f64();
        self.base_cycles + (elapsed * self.clock_speed as f64) as u64
    }

    /// Sleep if the emulator is running ahead of real time, or reset the reference if it
    /// has fallen behind by more than `lag_threshold` (e.g. after returning from a menu).
    pub fn sync(&mut self, current_cycles: u64, lag_threshold: Duration) {
        if self.clock_speed == 0 {
            return;
        }
        let target = self.target_cycles();
        if current_cycles > target {
            let excess = current_cycles - target;
            let sleep_dur = Duration::from_secs_f64(excess as f64 / self.clock_speed as f64);
            std::thread::sleep(sleep_dur);
        } else {
            let lag_cycles = target - current_cycles;
            let lag = Duration::from_secs_f64(lag_cycles as f64 / self.clock_speed as f64);
            if lag > lag_threshold {
                self.reset(current_cycles);
            }
        }
    }
}
