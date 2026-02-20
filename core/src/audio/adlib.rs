use crate::audio::nuked_opl3::{self, Opl3Chip};
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
/// reducing emulator-thread lock acquisitions from ~44,100/sec to ~244/sec.
/// At 44,100 Hz, 128 samples = ~0.725 ms of latency — imperceptible.
const FLUSH_SIZE: usize = 128;

/// AdLib Music Synthesizer Card (Yamaha OPL2 FM synthesis).
///
/// Implements `SoundCard`: owns the Nuked OPL3 chip (running in OPL2 compatibility
/// mode, `chip.newm = 0`) and an internal PCM ring buffer. Timer state for registers
/// 0x02–0x04 is managed here rather than inside the chip, matching the AdLib hardware
/// design where the timer logic sits on the card, not the YM3812 chip itself.
///
/// The emulator calls `tick(cpu_cycles)` every instruction to advance the chip
/// and accumulate samples; the audio backend calls `pop_samples()` to drain them.
///
/// For native platforms, obtain a sharable `AdlibConsumer` via `consumer()`
/// **before** boxing the `Adlib` into `Box<dyn SoundCard>`.
pub struct Adlib {
    chip: Opl3Chip,
    // Timer state (moved from the old hand-rolled Opl2; not part of opl3.c)
    pending_address: u8,
    timer1_value: u8,
    timer2_value: u8,
    timer_control: u8,
    timer1_counter: u32,
    timer2_counter: u32,
    pub status: u8,
    // CPU-cycles → output-samples rate-conversion accumulator
    cycle_acc: u64,
    cpu_freq: u64,
    timer1_cycles_per_tick: u32,
    timer2_cycles_per_tick: u32,
    // Ring buffer (unchanged from previous implementation)
    consumer: PcmRingBuffer,
    pending_flush: Vec<f32>,
    overflow_count: u64,
    samples_since_log: u64,
}

impl Adlib {
    pub fn new(cpu_freq: u64) -> Self {
        let mut chip = Opl3Chip::default();
        nuked_opl3::reset(&mut chip, ADLIB_SAMPLE_RATE);
        // Timer 1: 80 µs period; Timer 2: 320 µs period.
        let timer1_cycles_per_tick = (80e-6 * cpu_freq as f64).round() as u32;
        let timer2_cycles_per_tick = (320e-6 * cpu_freq as f64).round() as u32;
        Self {
            chip,
            pending_address: 0,
            timer1_value: 0,
            timer2_value: 0,
            timer_control: 0,
            timer1_counter: 0,
            timer2_counter: 0,
            status: 0,
            cycle_acc: 0,
            cpu_freq,
            timer1_cycles_per_tick,
            timer2_cycles_per_tick,
            consumer: PcmRingBuffer::new(DEFAULT_CAPACITY),
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

    /// Handle a write to OPL register 0x04 (Timer Control).
    ///
    /// Ported verbatim from `Opl2::handle_timer_control` (the previous hand-rolled
    /// OPL2 emulator). Timer protocol logic is not part of the Nuked OPL3 C source
    /// (opl3.c); it belongs to the AdLib card circuitry.
    fn handle_timer_control(&mut self, value: u8) {
        if value & 0x80 != 0 {
            // Bit 7 = IRQ reset: clear all status flags.
            self.status = 0;
            return;
        }
        self.timer_control = value;
        // Restart counters when the corresponding start bits are set.
        if value & 0x01 != 0 {
            self.timer1_counter = 0;
        }
        if value & 0x02 != 0 {
            self.timer2_counter = 0;
        }
    }

    /// Advance both hardware timers by `cpu_cycles`.
    ///
    /// Programs detect the AdLib card by:
    ///   1. Writing known values to timer registers 0x02 / 0x03
    ///   2. Starting timers via register 0x04
    ///   3. Reading status port 0x388 and checking bits 6/5 for overflow
    ///
    /// Ported verbatim from `Opl2::advance_timers` (the previous hand-rolled
    /// OPL2 emulator). Timer protocol logic is not part of the Nuked OPL3 C source
    /// (opl3.c); it belongs to the AdLib card circuitry.
    fn advance_timers(&mut self, cpu_cycles: u64) {
        let cycles = cpu_cycles as u32;

        // Timer 1: bit 0 of timer_control starts it; bit 6 masks the status flag.
        if self.timer_control & 0x01 != 0 {
            self.timer1_counter += cycles;
            let ticks = (256 - self.timer1_value as u32).max(1);
            let threshold = ticks * self.timer1_cycles_per_tick;
            if self.timer1_counter >= threshold {
                self.timer1_counter = 0;
                if self.timer_control & 0x40 == 0 {
                    // Not masked → set status bits.
                    self.status |= 0xC0; // bit 7 (IRQ) + bit 6 (Timer 1 expired)
                }
            }
        }

        // Timer 2: bit 1 starts it; bit 5 masks the status flag.
        if self.timer_control & 0x02 != 0 {
            self.timer2_counter += cycles;
            let ticks = (256 - self.timer2_value as u32).max(1);
            let threshold = ticks * self.timer2_cycles_per_tick;
            if self.timer2_counter >= threshold {
                self.timer2_counter = 0;
                if self.timer_control & 0x20 == 0 {
                    self.status |= 0xA0; // bit 7 (IRQ) + bit 5 (Timer 2 expired)
                }
            }
        }
    }
}

impl SoundCard for Adlib {
    fn write_port(&mut self, port: u16, value: u8) {
        match port {
            0x388 => self.pending_address = value,
            0x389 => {
                let addr = self.pending_address;
                match addr {
                    // Timer registers are intercepted here; they are not forwarded
                    // to the Nuked OPL3 chip (opl3.c has no timer handling).
                    0x02 => self.timer1_value = value,
                    0x03 => self.timer2_value = value,
                    0x04 => self.handle_timer_control(value),
                    _ => nuked_opl3::write_reg(&mut self.chip, addr as u16, value),
                }
            }
            _ => {}
        }
    }

    fn read_port(&mut self, port: u16) -> u8 {
        match port {
            0x388 | 0x389 => self.status,
            _ => 0xFF,
        }
    }

    fn port_ranges(&self) -> &[(u16, u16)] {
        &[(0x388, 0x389)]
    }

    fn tick(&mut self, cpu_cycles: u64) {
        self.advance_timers(cpu_cycles);

        // Convert CPU cycles to OPL output samples using an integer accumulator
        // to avoid per-tick floating-point division.
        self.cycle_acc += cpu_cycles * ADLIB_SAMPLE_RATE as u64;
        let n_out = self.cycle_acc / self.cpu_freq;
        self.cycle_acc %= self.cpu_freq;

        for _ in 0..n_out {
            // nuked_opl3::generate_resampled fills buf[0]=left, buf[1]=right.
            let mut buf = [0i16; 2];
            nuked_opl3::generate_resampled(&mut self.chip, &mut buf);
            // Mix to mono: AdLib is a mono card.
            let mono = (buf[0] as i32 + buf[1] as i32) / 2;
            self.pending_flush.push(mono as f32 / 32768.0);
        }

        if self.pending_flush.len() >= FLUSH_SIZE {
            self.flush_pending();
        }
    }

    fn pop_samples(&mut self, count: usize) -> Vec<f32> {
        // Flush any pending samples before popping (used by WASM path).
        self.flush_pending();
        // Return only the samples actually available — no zero-padding.
        // Zero-padding would inject periodic silence into the audio stream,
        // causing audible warbling (periodic amplitude modulation at ~57 Hz).
        // The AudioWorklet handles true underruns by zero-filling individual
        // 128-sample quanta when its own buffer empties.
        let mut buf = self.consumer.inner.lock().unwrap();
        let available = buf.len().min(count);
        let mut out = Vec::with_capacity(available);
        for _ in 0..available {
            out.push(buf.pop_front().unwrap());
        }
        out
    }

    fn reset(&mut self) {
        self.pending_flush.clear();
        nuked_opl3::reset(&mut self.chip, ADLIB_SAMPLE_RATE);
        self.pending_address = 0;
        self.timer1_value = 0;
        self.timer2_value = 0;
        self.timer_control = 0;
        self.timer1_counter = 0;
        self.timer2_counter = 0;
        self.status = 0;
        self.cycle_acc = 0;
        self.consumer.inner.lock().unwrap().clear();
    }
}
