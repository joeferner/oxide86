use crate::audio::opl2::Opl2;
use crate::audio::{PcmRingBuffer, SoundCard};

/// Target audio output sample rate for AdLib (OPL2) output.
/// Shared by both the ring buffer and the Rodio/Web Audio backends.
pub const ADLIB_SAMPLE_RATE: u32 = 44100;

const DEFAULT_CAPACITY: usize = ADLIB_SAMPLE_RATE as usize / 10; // 100 ms

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
}

impl Adlib {
    pub fn new() -> Self {
        Self {
            opl2: Opl2::new(),
            consumer: PcmRingBuffer::new(DEFAULT_CAPACITY),
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
        let mut samples = Vec::new();
        self.opl2.generate_samples(cpu_cycles, &mut samples);
        let mut buf = self.consumer.inner.lock().unwrap();
        for s in samples {
            if buf.len() >= self.consumer.capacity {
                buf.pop_front();
            }
            buf.push_back(s);
        }
    }

    fn pop_samples(&mut self, count: usize) -> Vec<f32> {
        self.consumer.pop_samples(count)
    }

    fn reset(&mut self) {
        self.opl2 = Opl2::new();
        self.consumer.inner.lock().unwrap().clear();
    }
}
