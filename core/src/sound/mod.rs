//! Platform-independent sound card abstraction.
//!
//! Provides the `SoundCard` trait for future sound cards (AdLib, Sound Blaster, etc.).
//! The OPL2 engine lives in `crate::opl2` and is embedded directly in `IoDevice`;
//! this module supplies the extensibility layer used when parsing CLI/config options.

pub mod adlib;
pub mod opl2;

/// Platform-independent interface for a sound card emulation.
///
/// Implementations handle I/O port reads/writes and produce PCM samples.
/// Samples are mono f32 in the range -1.0..1.0 at `crate::audio::ADLIB_SAMPLE_RATE`.
pub trait SoundCard: Send {
    /// Handle an I/O port write directed at this card.
    fn write_port(&mut self, port: u16, value: u8);
    /// Handle an I/O port read directed at this card.
    fn read_port(&mut self, port: u16) -> u8;
    /// List of (start, end) inclusive port ranges this card owns.
    fn port_ranges(&self) -> &[(u16, u16)];
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
