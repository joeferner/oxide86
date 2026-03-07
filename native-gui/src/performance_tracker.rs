use std::time::Instant;

pub(crate) struct PerformanceTracker {
    last_update_time: Instant,
    last_cycle_count: u64,
    current_mhz: f64,
    update_interval_ms: u64,
}

impl PerformanceTracker {
    pub(crate) fn new() -> Self {
        Self {
            last_update_time: Instant::now(),
            last_cycle_count: 0,
            current_mhz: 0.0,
            update_interval_ms: 200,
        }
    }

    pub(crate) fn update(&mut self, current_cycles: u64) {
        let now = Instant::now();
        let elapsed_ms = now.duration_since(self.last_update_time).as_millis() as u64;

        if elapsed_ms >= self.update_interval_ms {
            let cycle_delta = current_cycles.saturating_sub(self.last_cycle_count);
            let time_delta_ms = elapsed_ms as f64;

            // Calculate instantaneous MHz: cycles / milliseconds / 1000
            let instant_mhz = (cycle_delta as f64) / time_delta_ms / 1000.0;

            // Exponential moving average for smoothing
            if self.current_mhz == 0.0 {
                self.current_mhz = instant_mhz;
            } else {
                self.current_mhz = 0.7 * self.current_mhz + 0.3 * instant_mhz;
            }

            self.last_update_time = now;
            self.last_cycle_count = current_cycles;
        }
    }

    pub(crate) fn get_mhz(&self) -> f64 {
        self.current_mhz
    }
}
