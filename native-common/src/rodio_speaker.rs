use emu86_core::SpeakerOutput;
use rodio::stream::OutputStream;
use rodio::{Sink, Source};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Infinite square wave generator for PC speaker emulation
struct SquareWave {
    frequency: Arc<Mutex<f32>>,
    sample_rate: u32,
    phase: f32,
}

impl Iterator for SquareWave {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let freq = *self.frequency.lock().unwrap();
        if freq <= 0.0 {
            return Some(0.0);
        }

        let phase_increment = freq / (self.sample_rate as f32);
        self.phase = (self.phase + phase_increment) % 1.0;

        // Square wave: 30% volume to avoid distortion
        Some(if self.phase < 0.5 { 0.3 } else { -0.3 })
    }
}

impl Source for SquareWave {
    fn current_span_len(&self) -> Option<usize> {
        None // Infinite stream
    }

    fn channels(&self) -> u16 {
        1 // Mono
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None // Infinite
    }
}

/// Rodio-based PC speaker implementation.
///
/// Connects to a shared [`OutputStream`] mixer; the caller must keep
/// the stream alive for the duration of playback.
pub struct RodioSpeaker {
    sink: Sink,
    frequency: Arc<Mutex<f32>>,
    enabled: bool,
    /// Cached frequency to avoid unnecessary mutex locks
    last_frequency: f32,
}

impl RodioSpeaker {
    /// Create a new `RodioSpeaker` connected to `stream`'s mixer.
    pub fn new(stream: &OutputStream) -> Self {
        let sink = Sink::connect_new(stream.mixer());

        let frequency = Arc::new(Mutex::new(0.0));
        let wave = SquareWave {
            frequency: frequency.clone(),
            sample_rate: 48000,
            phase: 0.0,
        };

        sink.append(wave);
        sink.pause(); // Start paused

        Self {
            sink,
            frequency,
            enabled: false,
            last_frequency: 0.0,
        }
    }
}

impl SpeakerOutput for RodioSpeaker {
    fn set_frequency(&mut self, enabled: bool, frequency: f32) {
        // Only update frequency if it changed (avoid unnecessary mutex lock)
        if (frequency - self.last_frequency).abs() > 0.1 {
            *self.frequency.lock().unwrap() = frequency;
            self.last_frequency = frequency;
        }

        // Update playback state if changed
        if enabled != self.enabled {
            if enabled {
                log::trace!("RodioSpeaker: Starting playback at {:.2} Hz", frequency);
                self.sink.play();
            } else {
                log::trace!("RodioSpeaker: Pausing playback");
                self.sink.pause();
            }
            self.enabled = enabled;
        }
    }

    fn update(&mut self) {
        // Rodio handles buffering automatically - no action needed
    }
}
