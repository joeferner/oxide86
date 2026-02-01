use crate::SpeakerOutput;
use rodio::stream::OutputStream;
use rodio::{OutputStreamBuilder, Sink, Source};
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

/// Rodio-based PC speaker implementation
pub struct RodioSpeaker {
    _stream: OutputStream,
    sink: Sink,
    frequency: Arc<Mutex<f32>>,
    enabled: bool,
}

impl RodioSpeaker {
    /// Create a new RodioSpeaker
    ///
    /// Returns an error if audio device is unavailable or initialization fails
    pub fn new() -> Result<Self, String> {
        let stream_handle = OutputStreamBuilder::open_default_stream()
            .map_err(|e| format!("Audio device unavailable: {}", e))?;

        let sink = Sink::connect_new(stream_handle.mixer());

        let frequency = Arc::new(Mutex::new(0.0));
        let wave = SquareWave {
            frequency: frequency.clone(),
            sample_rate: 48000,
            phase: 0.0,
        };

        sink.append(wave);
        sink.pause(); // Start paused

        Ok(Self {
            _stream: stream_handle,
            sink,
            frequency,
            enabled: false,
        })
    }
}

impl SpeakerOutput for RodioSpeaker {
    fn set_frequency(&mut self, enabled: bool, frequency: f32) {
        // Update frequency
        *self.frequency.lock().unwrap() = frequency;

        // Update playback state if changed
        if enabled != self.enabled {
            if enabled {
                self.sink.play();
            } else {
                self.sink.pause();
            }
            self.enabled = enabled;
        }
    }

    fn update(&mut self) {
        // Rodio handles buffering automatically - no action needed
    }
}
