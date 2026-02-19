use emu86_core::audio::adlib::{ADLIB_SAMPLE_RATE, AdlibConsumer};
use rodio::stream::OutputStream;
use rodio::{OutputStreamBuilder, Sink, Source};
use std::time::Duration;
use anyhow::{Context, Result};

/// Rodio audio source that pulls PCM samples from an AdLib consumer handle.
struct AdlibSource {
    consumer: AdlibConsumer,
    /// Samples pulled since last log
    log_samples: u32,
    /// Non-zero samples in the current log window
    log_nonzero: u32,
}

impl Iterator for AdlibSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        // pop_samples already pads with 0.0 on underrun
        let sample = self.consumer.pop_samples(1).remove(0);

        self.log_samples += 1;
        if sample.abs() > 1e-6 {
            self.log_nonzero += 1;
        }

        // Log once per second (44100 samples)
        if self.log_samples >= 44100 {
            log::debug!(
                "[AdLib] Rodio: {}/{} samples non-zero ({:.1}%), ring buffer available: {}",
                self.log_nonzero,
                self.log_samples,
                100.0 * self.log_nonzero as f32 / self.log_samples as f32,
                self.consumer.available(),
            );
            self.log_samples = 0;
            self.log_nonzero = 0;
        }

        Some(sample)
    }
}

impl Source for AdlibSource {
    fn current_span_len(&self) -> Option<usize> {
        None // Infinite stream
    }

    fn channels(&self) -> u16 {
        1 // Mono
    }

    fn sample_rate(&self) -> u32 {
        ADLIB_SAMPLE_RATE
    }

    fn total_duration(&self) -> Option<Duration> {
        None // Infinite
    }
}

/// Rodio-based AdLib (OPL2) audio output.
///
/// Holds a Rodio Sink that continuously drains the AdLib ring buffer.
/// The PC speaker (`RodioSpeaker`) and `RodioAdlib` open separate Rodio
/// output streams; the OS mixer blends them automatically.
pub struct RodioAdlib {
    _stream: OutputStream,
    _sink: Sink,
}

impl RodioAdlib {
    /// Create a new AdLib audio output connected to `consumer`.
    /// Returns an error if the audio device is unavailable.
    pub fn new(consumer: AdlibConsumer) -> Result<Self> {
        let stream = OutputStreamBuilder::open_default_stream()
            .context("AdLib audio device unavailable")?;

        let sink = Sink::connect_new(stream.mixer());
        sink.append(AdlibSource { consumer, log_samples: 0, log_nonzero: 0 });
        // Sink starts playing immediately (no pause needed — silence is just 0.0 samples)

        Ok(Self {
            _stream: stream,
            _sink: sink,
        })
    }
}
