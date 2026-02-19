use emu86_core::audio::PcmRingBuffer;
use emu86_core::audio::adlib::ADLIB_SAMPLE_RATE;
use rodio::stream::OutputStream;
use rodio::{Sink, Source};
use std::time::Duration;

/// Rodio audio source that pulls PCM samples from a PCM ring buffer.
struct PcmSource {
    consumer: PcmRingBuffer,
    /// Samples pulled since last log
    log_samples: u32,
    /// Non-zero samples in the current log window
    log_nonzero: u32,
}

impl Iterator for PcmSource {
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
                "[PCM] Rodio: {}/{} samples non-zero ({:.1}%), ring buffer available: {}",
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

impl Source for PcmSource {
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

/// Rodio-based PCM audio output.
///
/// Holds a Rodio [`Sink`] that continuously drains the PCM ring buffer.
/// Connects to a shared [`OutputStream`] mixer; the caller must keep
/// the stream alive for the duration of playback.
pub struct RodioPcm {
    _sink: Sink,
}

impl RodioPcm {
    /// Create a new PCM audio output connected to `stream`'s mixer.
    pub fn new(consumer: PcmRingBuffer, stream: &OutputStream) -> Self {
        let sink = Sink::connect_new(stream.mixer());
        sink.append(PcmSource {
            consumer,
            log_samples: 0,
            log_nonzero: 0,
        });
        // Sink starts playing immediately (no pause needed — silence is just 0.0 samples)
        Self { _sink: sink }
    }
}
