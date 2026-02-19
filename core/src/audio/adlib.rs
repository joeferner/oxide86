use crate::audio::opl2::Opl2;
use crate::audio::{PcmRingBuffer, SoundCard};

/// Target audio output sample rate for AdLib (OPL2) output.
/// Shared by both the ring buffer and the Rodio/Web Audio backends.
pub const ADLIB_SAMPLE_RATE: u32 = 44100;

/// Ring buffer capacity: 500 ms of audio.
///
/// A larger buffer gives the emulator more slack to run slightly ahead of
/// real-time without overflowing, and gives the audio thread more samples
/// to draw from when the emulator is briefly stalled (e.g. during INT calls).
const DEFAULT_CAPACITY: usize = ADLIB_SAMPLE_RATE as usize / 2; // 500 ms

/// Number of samples accumulated locally before flushing to the shared ring buffer.
///
/// The ring buffer mutex is acquired once per flush instead of once per sample,
/// reducing emulator-thread lock acquisitions from ~44,100/sec to ~689/sec.
/// At 44,100 Hz, 64 samples = ~1.45 ms of latency — imperceptible.
const FLUSH_SIZE: usize = 64;

/// AdLib Music Synthesizer Card (Yamaha OPL2 FM synthesis).
///
/// Implements `SoundCard`: owns the OPL2 chip and an internal PCM ring buffer.
/// The emulator calls `tick(cpu_cycles)` every instruction to advance the chip
/// and accumulate samples; the audio backend calls `pop_samples()` to drain them.
///
/// For native platforms, obtain a sharable `AdlibConsumer` via `consumer()`
/// **before** boxing the `Adlib` into `Box<dyn SoundCard>`.
pub struct Adlib {
    opl2: Opl2,
    consumer: PcmRingBuffer,
    /// Reusable scratch buffer for OPL2 sample generation (avoids per-tick allocation).
    samples_scratch: Vec<f32>,
    /// Samples accumulated locally, flushed to the ring buffer every FLUSH_SIZE samples.
    pending_flush: Vec<f32>,
    /// Number of samples dropped due to ring buffer overflow since last log.
    overflow_count: u64,
    /// Total samples flushed since last overflow/underrun log (used for rate limiting).
    samples_since_log: u64,
}

impl Adlib {
    pub fn new() -> Self {
        Self {
            opl2: Opl2::new(),
            consumer: PcmRingBuffer::new(DEFAULT_CAPACITY),
            samples_scratch: Vec::new(),
            pending_flush: Vec::with_capacity(FLUSH_SIZE * 2),
            overflow_count: 0,
            samples_since_log: 0,
        }
    }

    /// Return a handle to the shared ring buffer for consumer threads.
    ///
    /// Clone this **before** boxing the `Adlib` and pass it to the audio
    /// backend (e.g., `RodioAdlib`).
    pub fn consumer(&self) -> PcmRingBuffer {
        self.consumer.clone()
    }

    /// Flush all pending samples to the shared ring buffer (one mutex acquisition).
    fn flush_pending(&mut self) {
        if self.pending_flush.is_empty() {
            return;
        }

        let mut buf = self.consumer.inner.lock().unwrap();
        for &s in &self.pending_flush {
            if buf.len() >= self.consumer.capacity {
                buf.pop_front();
                self.overflow_count += 1;
            }
            buf.push_back(s);
        }
        drop(buf);

        // Rate-limited overflow log: report once per ~1 second of audio.
        self.samples_since_log += self.pending_flush.len() as u64;
        if self.samples_since_log >= ADLIB_SAMPLE_RATE as u64 {
            if self.overflow_count > 0 {
                log::debug!(
                    "[AdLib] Ring buffer overflow: {} samples dropped in the last ~1s \
                     (emulator running faster than real-time)",
                    self.overflow_count
                );
                self.overflow_count = 0;
            }
            self.samples_since_log = 0;
        }

        self.pending_flush.clear();
    }
}

impl Default for Adlib {
    fn default() -> Self {
        Self::new()
    }
}

impl SoundCard for Adlib {
    fn write_port(&mut self, port: u16, value: u8) {
        match port {
            0x388 => self.opl2.write_address(value),
            0x389 => self.opl2.write_data(value),
            _ => {}
        }
    }

    fn read_port(&mut self, port: u16) -> u8 {
        match port {
            0x388 | 0x389 => self.opl2.read_status(),
            _ => 0xFF,
        }
    }

    fn port_ranges(&self) -> &[(u16, u16)] {
        &[(0x388, 0x389)]
    }

    fn tick(&mut self, cpu_cycles: u64) {
        // Generate samples into scratch buffer (no mutex needed).
        self.samples_scratch.clear();
        self.opl2
            .generate_samples(cpu_cycles, &mut self.samples_scratch);

        if self.samples_scratch.is_empty() {
            return;
        }

        // Accumulate locally — flush to the shared ring buffer only when
        // we have FLUSH_SIZE samples, reducing mutex acquisitions from
        // ~44,100/sec (one per sample) to ~689/sec (one per 64 samples).
        self.pending_flush.extend_from_slice(&self.samples_scratch);
        if self.pending_flush.len() >= FLUSH_SIZE {
            self.flush_pending();
        }
    }

    fn pop_samples(&mut self, count: usize) -> Vec<f32> {
        // Flush any pending samples before popping (used by WASM path).
        self.flush_pending();
        self.consumer.pop_samples(count)
    }

    fn reset(&mut self) {
        self.pending_flush.clear();
        self.opl2 = Opl2::new();
        self.consumer.inner.lock().unwrap().clear();
    }
}
