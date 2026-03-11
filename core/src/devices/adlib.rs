use std::{any::Any, cell::RefCell};

use crate::{
    Device,
    devices::{
        PcmRingBuffer,
        nuked_opl3::{self, Opl3Chip},
    },
};

/// Target audio output sample rate for AdLib (OPL2) output.
/// Shared by both the ring buffer and the Rodio/Web Audio backends.
pub const ADLIB_SAMPLE_RATE: u32 = 44100;

/// Ring buffer capacity: 500 ms of audio.
const DEFAULT_CAPACITY: usize = ADLIB_SAMPLE_RATE as usize / 2;

/// Number of samples accumulated locally before flushing to the shared ring buffer.
const FLUSH_SIZE: usize = 128;

struct AdlibInner {
    chip: Opl3Chip,
    pending_address: u8,
    timer1_value: u8,
    timer2_value: u8,
    timer_control: u8,
    timer1_counter: u32,
    timer2_counter: u32,
    pub status: u8,
    cycle_acc: u64,
    last_cycle_count: u32,
    pending_flush: Vec<f32>,
    overflow_count: u64,
    samples_since_log: u64,
}

/// AdLib Music Synthesizer Card (Yamaha OPL2 FM synthesis).
///
/// Implements the `Device` trait; owns the Nuked OPL3 chip (running in OPL2
/// compatibility mode) and an internal PCM ring buffer.
///
/// `io_write_u8` drives all sample generation: whenever the emulator writes an
/// OPL register, elapsed CPU cycles since the previous write are converted to
/// audio samples and pushed to the ring buffer.
///
/// `io_read_u8` also advances the chip (via interior mutability) so that the
/// timer-detection sequence works correctly even when the delay consists only
/// of CPU reads.
///
/// For native platforms, call `consumer()` **before** adding the device to the
/// computer, then pass the returned handle to `RodioAdlib`.
pub struct Adlib {
    inner: RefCell<AdlibInner>,
    cpu_freq: u64,
    timer1_cycles_per_tick: u32,
    timer2_cycles_per_tick: u32,
    consumer: PcmRingBuffer,
}

impl Adlib {
    pub fn new(cpu_freq: u64) -> Self {
        let mut chip = Opl3Chip::default();
        nuked_opl3::reset(&mut chip, ADLIB_SAMPLE_RATE);
        let timer1_cycles_per_tick = (80e-6 * cpu_freq as f64).round() as u32;
        let timer2_cycles_per_tick = (320e-6 * cpu_freq as f64).round() as u32;
        Self {
            inner: RefCell::new(AdlibInner {
                chip,
                pending_address: 0,
                timer1_value: 0,
                timer2_value: 0,
                timer_control: 0,
                timer1_counter: 0,
                timer2_counter: 0,
                status: 0,
                cycle_acc: 0,
                last_cycle_count: 0,
                pending_flush: Vec::with_capacity(FLUSH_SIZE * 2),
                overflow_count: 0,
                samples_since_log: 0,
            }),
            cpu_freq,
            timer1_cycles_per_tick,
            timer2_cycles_per_tick,
            consumer: PcmRingBuffer::new(DEFAULT_CAPACITY, ADLIB_SAMPLE_RATE),
        }
    }

    /// Return a handle to the shared ring buffer for consumer threads.
    ///
    /// Clone this **before** adding the `Adlib` to the computer and pass it
    /// to the audio backend (e.g., `RodioAdlib`).
    pub fn consumer(&self) -> PcmRingBuffer {
        self.consumer.clone()
    }

    /// Advance timers and generate samples up to `cycle_count`.
    ///
    /// Takes `&self` so it can be called from both `io_read_u8` (immutable) and
    /// `io_write_u8` (mutable). Interior mutability is provided by `RefCell`.
    fn advance_to_cycle(&self, cycle_count: u32) {
        let mut g = self.inner.borrow_mut();
        let elapsed = cycle_count.wrapping_sub(g.last_cycle_count) as u64;
        g.last_cycle_count = cycle_count;

        // Advance hardware timers.
        if elapsed > 0 {
            let cycles = elapsed as u32;
            if g.timer_control & 0x01 != 0 {
                g.timer1_counter += cycles;
                let ticks = (256 - g.timer1_value as u32).max(1);
                let threshold = ticks * self.timer1_cycles_per_tick;
                if g.timer1_counter >= threshold {
                    g.timer1_counter = 0;
                    if g.timer_control & 0x40 == 0 {
                        g.status |= 0xC0;
                    }
                }
            }
            if g.timer_control & 0x02 != 0 {
                g.timer2_counter += cycles;
                let ticks = (256 - g.timer2_value as u32).max(1);
                let threshold = ticks * self.timer2_cycles_per_tick;
                if g.timer2_counter >= threshold {
                    g.timer2_counter = 0;
                    if g.timer_control & 0x20 == 0 {
                        g.status |= 0xA0;
                    }
                }
            }
        }

        // Generate OPL samples for the elapsed cycles.
        g.cycle_acc += elapsed * ADLIB_SAMPLE_RATE as u64;
        let n_out = g.cycle_acc / self.cpu_freq;
        g.cycle_acc %= self.cpu_freq;

        for _ in 0..n_out {
            let mut buf = [0i16; 2];
            nuked_opl3::generate_resampled(&mut g.chip, &mut buf);
            let mono = (buf[0] as i32 + buf[1] as i32) / 2;
            g.pending_flush.push(mono as f32 / 32768.0);
        }

        if g.pending_flush.len() >= FLUSH_SIZE {
            Self::flush_pending_inner(&mut g, &self.consumer);
        }
    }

    fn flush_pending_inner(g: &mut AdlibInner, consumer: &PcmRingBuffer) {
        if g.pending_flush.is_empty() {
            return;
        }
        let mut buf = consumer.inner.lock().unwrap();
        for &s in &g.pending_flush {
            if buf.len() >= consumer.capacity {
                buf.pop_front();
                g.overflow_count += 1;
            }
            buf.push_back(s);
        }
        drop(buf);

        g.samples_since_log += g.pending_flush.len() as u64;
        if g.samples_since_log >= ADLIB_SAMPLE_RATE as u64 {
            if g.overflow_count > 0 {
                log::debug!(
                    "[AdLib] Ring buffer overflow: {} samples dropped in the last ~1s",
                    g.overflow_count
                );
                g.overflow_count = 0;
            }
            g.samples_since_log = 0;
        }
        g.pending_flush.clear();
    }
}

impl Device for Adlib {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn io_read_u8(&self, port: u16, cycle_count: u32) -> Option<u8> {
        match port {
            0x388 | 0x389 => {
                self.advance_to_cycle(cycle_count);
                Some(self.inner.borrow().status)
            }
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8, cycle_count: u32) -> bool {
        match port {
            0x388 => {
                self.advance_to_cycle(cycle_count);
                self.inner.borrow_mut().pending_address = val;
                true
            }
            0x389 => {
                self.advance_to_cycle(cycle_count);
                let addr = self.inner.borrow().pending_address;
                let mut g = self.inner.borrow_mut();
                match addr {
                    0x02 => g.timer1_value = val,
                    0x03 => g.timer2_value = val,
                    0x04 => {
                        if val & 0x80 != 0 {
                            g.status = 0;
                        } else {
                            g.timer_control = val;
                            if val & 0x01 != 0 {
                                g.timer1_counter = 0;
                            }
                            if val & 0x02 != 0 {
                                g.timer2_counter = 0;
                            }
                        }
                    }
                    _ => nuked_opl3::write_reg(&mut g.chip, addr as u16, val),
                }
                true
            }
            _ => false,
        }
    }

    fn memory_read_u8(&self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn reset(&mut self) {
        let mut g = self.inner.borrow_mut();
        g.pending_flush.clear();
        nuked_opl3::reset(&mut g.chip, ADLIB_SAMPLE_RATE);
        g.pending_address = 0;
        g.timer1_value = 0;
        g.timer2_value = 0;
        g.timer_control = 0;
        g.timer1_counter = 0;
        g.timer2_counter = 0;
        g.status = 0;
        g.cycle_acc = 0;
        g.last_cycle_count = 0;
        drop(g);
        self.consumer.inner.lock().unwrap().clear();
    }
}
