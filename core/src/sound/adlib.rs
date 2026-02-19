use crate::sound::SoundCard;
use crate::sound::opl2::Opl2;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Target audio output sample rate for AdLib (OPL2) output.
/// Shared by both the ring buffer and the Rodio/Web Audio backends.
pub const ADLIB_SAMPLE_RATE: u32 = 44100;

const DEFAULT_CAPACITY: usize = ADLIB_SAMPLE_RATE as usize / 10; // 100 ms

/// Shared ring buffer used by native audio consumer threads (e.g., Rodio).
///
/// `Adlib::consumer()` returns a clone of this handle before the `Adlib` is
/// boxed into `Box<dyn SoundCard>`. The consumer calls `pop_samples()` from
/// the audio thread; the emulator calls `Adlib::tick()` on the main thread.
/// Thread safety is provided by the inner `Arc<Mutex<_>>`.
#[derive(Clone)]
pub struct AdlibConsumer {
    inner: Arc<Mutex<VecDeque<f32>>>,
    capacity: usize,
}

impl AdlibConsumer {
    /// Pop up to `count` samples from the shared buffer.
    /// Returns zeros on underrun.
    pub fn pop_samples(&self, count: usize) -> Vec<f32> {
        let mut buf = self.inner.lock().unwrap();
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(buf.pop_front().unwrap_or(0.0));
        }
        out
    }

    pub fn available(&self) -> usize {
        self.inner.lock().unwrap().len()
    }
}

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
    consumer: AdlibConsumer,
}

impl Adlib {
    pub fn new() -> Self {
        Self {
            opl2: Opl2::new(),
            consumer: AdlibConsumer {
                inner: Arc::new(Mutex::new(VecDeque::with_capacity(DEFAULT_CAPACITY))),
                capacity: DEFAULT_CAPACITY,
            },
        }
    }

    /// Return a handle to the shared ring buffer for consumer threads.
    ///
    /// Clone this **before** boxing the `Adlib` and pass it to the audio
    /// backend (e.g., `RodioAdlib`).
    pub fn consumer(&self) -> AdlibConsumer {
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
