/// Platform-independent speaker output trait for PC speaker emulation.
///
/// Implementations provide platform-specific audio output (e.g., Rodio for native,
/// Web Audio API for WASM). The speaker produces square wave tones at specified frequencies.
pub trait SpeakerOutput: Send {
    /// Set the speaker frequency and enable/disable state.
    ///
    /// # Parameters
    /// - `enabled`: Whether the speaker should be producing sound
    /// - `frequency`: Frequency in Hz (typically 100-10000 Hz range for PC speaker)
    ///
    /// When `enabled` is false, the speaker should stop producing sound regardless of frequency.
    fn set_frequency(&mut self, enabled: bool, frequency: f32);

    /// Update the speaker output (for platforms that need periodic updates).
    ///
    /// Called periodically to allow the speaker implementation to perform any
    /// necessary updates or buffering. For asynchronous implementations (like Rodio),
    /// this may be a no-op.
    fn update(&mut self);
}

/// Null implementation of SpeakerOutput that produces no sound.
///
/// Used as a fallback when audio is unavailable or for headless environments.
pub struct NullSpeaker;

impl SpeakerOutput for NullSpeaker {
    fn set_frequency(&mut self, _enabled: bool, _frequency: f32) {
        // No-op: null speaker produces no sound
    }

    fn update(&mut self) {
        // No-op: nothing to update
    }
}
