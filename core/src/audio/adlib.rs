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
}

impl Adlib {
    pub fn new() -> Self {
        Self {
            opl2: Opl2::new(),
            consumer: PcmRingBuffer::new(DEFAULT_CAPACITY),
            samples_scratch: Vec::new(),
        }
    }

    /// Return a handle to the shared ring buffer for consumer threads.
    ///
    /// Clone this **before** boxing the `Adlib` and pass it to the audio
    /// backend (e.g., `RodioAdlib`).
    pub fn consumer(&self) -> PcmRingBuffer {
        self.consumer.clone()
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
        // Reuse the scratch buffer to avoid a heap allocation on every instruction.
        self.samples_scratch.clear();
        self.opl2
            .generate_samples(cpu_cycles, &mut self.samples_scratch);

        if self.samples_scratch.is_empty() {
            return;
        }

        let mut buf = self.consumer.inner.lock().unwrap();
        for &s in &self.samples_scratch {
            if buf.len() >= self.consumer.capacity {
                // Buffer full: discard the oldest sample so the newest is kept.
                // For a sustained tone this is inaudible; it only matters at
                // note transitions where the emulator is running faster than real-time.
                buf.pop_front();
            }
            buf.push_back(s);
        }
    }

    fn reset(&mut self) {
        self.opl2 = Opl2::new();
        self.consumer.inner.lock().unwrap().clear();
    }
}
