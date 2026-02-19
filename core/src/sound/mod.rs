//! Platform-independent sound card abstraction.
//!
//! Provides the `SoundCard` trait implemented by concrete cards such as `Adlib`.
//! The `IoDevice` holds a `Box<dyn SoundCard>` and routes I/O ports to it.

pub mod adlib;
pub mod opl2;

/// Platform-independent interface for a sound card emulation.
///
/// Implementations handle I/O port reads/writes, produce PCM samples via
/// `tick()`, and expose samples for consumption via `pop_samples()`.
pub trait SoundCard: Send {
    /// Handle an I/O port write directed at this card.
    fn write_port(&mut self, port: u16, value: u8);
    /// Handle an I/O port read directed at this card.
    fn read_port(&mut self, port: u16) -> u8;
    /// List of (start, end) inclusive port ranges this card owns.
    fn port_ranges(&self) -> &[(u16, u16)];
    /// Advance the card by `cpu_cycles` CPU cycles, generating audio samples
    /// internally. Default implementation is a no-op.
    fn tick(&mut self, _cpu_cycles: u64) {}
    /// Pop up to `count` samples from the internal buffer.
    /// Returns zeros for any samples not yet available (underrun padding).
    /// Default implementation always returns silence.
    fn pop_samples(&mut self, count: usize) -> Vec<f32> {
        vec![0.0; count]
    }
    /// Reset the sound card to its power-on state.
    /// Default implementation is a no-op.
    fn reset(&mut self) {}
}

/// No-op sound card used when no card is selected.
pub struct NullSoundCard;

impl SoundCard for NullSoundCard {
    fn write_port(&mut self, _port: u16, _value: u8) {}
    fn read_port(&mut self, _port: u16) -> u8 {
        0xFF
    }
    fn port_ranges(&self) -> &[(u16, u16)] {
        &[]
    }
}

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
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().trim() {
            "adlib" | "adl" => SoundCardType::AdLib,
            _ => SoundCardType::None,
        }
    }
}
