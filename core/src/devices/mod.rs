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
pub mod rtc;
pub mod serial_loopback;
pub mod serial_mouse;
pub mod sound_blaster_cdrom;
pub use sound_blaster_cdrom::SoundBlasterCdrom;
pub mod uart;

/// Shared ring buffer used by native audio consumer threads (e.g., Rodio).
///
/// `Adlib::consumer()` returns a clone of this handle before the `Adlib` is
/// boxed into `Box<dyn SoundCard>`. The consumer calls `pop_samples()` from
/// the audio thread; the emulator calls `Adlib::tick()` on the main thread.
/// Thread safety is provided by the inner `Arc<Mutex<_>>`.
#[derive(Clone)]
pub struct PcmRingBuffer {
    inner: Arc<Mutex<VecDeque<f32>>>,
    capacity: usize,
    pub sample_rate: u32,
}

impl PcmRingBuffer {
    pub fn new(capacity: usize, sample_rate: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
            sample_rate,
        }
    }

    pub fn available(&self) -> usize {
        self.inner.lock().unwrap().len()
    }

    /// Drain up to `buf.len()` samples into `buf`, padding with 0.0 on underrun.
    /// Acquires the lock exactly once. Returns the number of real samples written
    /// (the rest of the slice is zero-filled).
    pub fn drain_into(&self, buf: &mut [f32]) -> usize {
        let mut guard = self.inner.lock().unwrap();
        let available = guard.len().min(buf.len());
        if available == 0 && !buf.is_empty() {
            log::trace!("PCM buffer underrun: needed {} samples, had 0", buf.len());
        }
        for slot in buf[..available].iter_mut() {
            *slot = guard.pop_front().unwrap();
        }
        buf[available..].fill(0.0);
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
}

impl SoundCardType {
    /// Parse from a string (case-insensitive). Unknown values map to `None`.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().trim() {
            "adlib" | "adl" => Some(SoundCardType::AdLib),
            "none" => Some(SoundCardType::None),
            _ => None,
        }
    }
}
