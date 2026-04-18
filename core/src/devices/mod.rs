use std::{
    cell::RefCell,
    collections::VecDeque,
    rc::Rc,
    sync::{Arc, Mutex},
};

use crate::disk::cdrom::CdromBackend;

pub mod adlib;
pub mod clock;
pub mod dma;
pub mod floppy_disk_controller;
pub mod game_port;
pub mod hard_disk_controller;
pub mod keyboard_controller;
pub(crate) mod nuked_opl3;
pub mod parallel_port;
pub mod parallel_port_loopback;
pub mod pc_speaker;
pub mod pic;
pub mod pit;
pub mod printer;
pub mod rtc;
pub mod serial_loopback;
pub mod serial_mouse;
pub mod sound_blaster;
pub use sound_blaster::SoundBlaster;
pub mod uart;

/// Shared ring buffer used by native audio consumer threads (e.g., Rodio).
///
/// `Adlib::consumer()` returns a clone of this handle before the `Adlib` is
/// boxed into `Box<dyn SoundCard>`. The consumer calls `pop_samples()` from
/// the audio thread; the emulator calls `Adlib::tick()` on the main thread.
/// Thread safety is provided by the inner `Arc<Mutex<_>>`.
#[derive(Clone)]
pub struct PcmRingBuffer {
    inner: Arc<Mutex<PcmRingBufferInner>>,
    pub sample_rate: u32,
    /// When true, holds the last written sample on underrun (correct for Direct DAC).
    /// When false, fills underrun slots with 0.0 silence (correct for OPL).
    hold_on_underrun: bool,
}

struct PcmRingBufferInner {
    samples: VecDeque<f32>,
    capacity: usize,
    last_sample: f32,
}

impl PcmRingBuffer {
    pub fn new(capacity: usize, sample_rate: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PcmRingBufferInner {
                samples: VecDeque::with_capacity(capacity),
                capacity,
                last_sample: 0.0,
            })),
            sample_rate,
            hold_on_underrun: false,
        }
    }

    /// Like `new` but holds the last sample on underrun instead of silence.
    /// Use for Direct DAC output where the DAC holds voltage between software writes.
    pub fn new_with_hold(capacity: usize, sample_rate: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PcmRingBufferInner {
                samples: VecDeque::with_capacity(capacity),
                capacity,
                last_sample: 0.0,
            })),
            sample_rate,
            hold_on_underrun: true,
        }
    }

    pub fn push_sample(&self, sample: f32) {
        let mut guard = self.inner.lock().unwrap();
        if guard.samples.len() >= guard.capacity {
            guard.samples.pop_front();
        }
        guard.last_sample = sample;
        guard.samples.push_back(sample);
    }

    pub fn clear(&self) {
        let mut guard = self.inner.lock().unwrap();
        guard.samples.clear();
        guard.last_sample = 0.0;
    }

    pub fn available(&self) -> usize {
        self.inner.lock().unwrap().samples.len()
    }

    /// Drain up to `buf.len()` samples into `buf`.
    /// On underrun: fills with 0.0 silence normally, or holds the last sample
    /// when `hold_on_underrun` is set. Acquires the lock exactly once.
    /// Returns the number of real samples written.
    pub fn drain_into(&self, buf: &mut [f32]) -> usize {
        let mut guard = self.inner.lock().unwrap();
        let available = guard.samples.len().min(buf.len());
        if available == 0 && !buf.is_empty() {
            log::trace!("PCM buffer underrun: needed {} samples, had 0", buf.len());
        }
        for slot in buf[..available].iter_mut() {
            *slot = guard.samples.pop_front().unwrap();
        }
        let fill = if self.hold_on_underrun {
            guard.last_sample
        } else {
            0.0
        };
        buf[available..].fill(fill);
        available
    }
}

/// Trait for sound card devices that need regular cycle-accurate advancement.
///
/// The bus calls `advance_to_cycle` on every `increment_cycle_count`, giving
/// the sound card a steady timing stream regardless of IO port activity.
pub trait SoundCard {
    fn advance_to_cycle(&mut self, cycle_count: u32);
    fn next_sample_cycle(&self) -> u32;
    /// Called by the Bus after each IO write to drain a pending DMA request assertion
    /// or deassert. Returns `Some((global_channel, asserted))` when the device wants
    /// to change DREQ state on a DMA channel; `None` if no change.
    fn take_dreq_request(&mut self) -> Option<(u8, bool)> {
        None
    }
}

pub type SoundCardRef = Rc<RefCell<dyn SoundCard>>;

/// Trait for CD-ROM controller devices.
///
/// Mirrors the `SoundCard` trait pattern — `Bus` and `PIC` hold a `CdromControllerRef`
/// and never name the concrete type. Future interfaces (ATAPI, Mitsumi, etc.) implement
/// this trait and slot in without touching `Bus` or `PIC`.
pub trait CdromController {
    fn load_disc(&mut self, disc: Box<dyn CdromBackend>);
    fn eject_disc(&mut self);
    /// Called by the PIC to drain a pending IRQ. Returns `true` once per interrupt.
    fn take_pending_irq(&mut self) -> bool;
    /// The PIC1 IRQ line this device raises (e.g. 5 for the default SB CD interface).
    fn irq_line(&self) -> u8;
}

pub type CdromControllerRef = Rc<RefCell<dyn CdromController>>;

/// Which sound card to emulate. Parsed from the `--sound-card` / `sound_card` config option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundCardType {
    /// No sound card (default).
    None,
    /// AdLib Music Synthesizer Card (Yamaha OPL2, ports 0x388–0x389).
    AdLib,
    /// Sound Blaster 16 (DSP + OPL3 + mixer + MPU-401 + CD-ROM interface).
    SoundBlaster16,
}

impl SoundCardType {
    /// Parse from a string (case-insensitive). Unknown values map to `None`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().trim() {
            "adlib" | "adl" => Some(SoundCardType::AdLib),
            "sb16" | "sb" | "soundblaster" | "sound-blaster" => Some(SoundCardType::SoundBlaster16),
            "none" => Some(SoundCardType::None),
            _ => None,
        }
    }
}
