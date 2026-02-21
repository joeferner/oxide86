use oxide86_core::audio::PcmRingBuffer;
use oxide86_core::audio::adlib::ADLIB_SAMPLE_RATE;
use rodio::stream::OutputStream;
use rodio::{Sink, Source};
use std::time::Duration;

/// Number of samples pre-fetched from the ring buffer at a time.
///
/// The mutex is acquired once per batch, so at 44 100 Hz the audio thread
/// locks ~86 times/second instead of 44 100 times/second.
/// At 44 100 Hz this is ~11.6 ms of look-ahead, which is imperceptible.
const BATCH_SIZE: usize = 512;

/// Rodio audio source that pulls PCM samples from a PCM ring buffer.
struct PcmSource {
    consumer: PcmRingBuffer,
    /// Pre-fetched samples — served lock-free until exhausted.
    batch: Box<[f32; BATCH_SIZE]>,
    /// Index of the next sample to serve from `batch`.
    batch_pos: usize,
    /// Samples pulled since last log
    log_samples: u32,
    /// Non-zero samples in the current log window
    log_nonzero: u32,
    /// Underrun samples (zeros from empty ring buffer) since last log
    underrun_count: u32,
}

impl PcmSource {
    fn new(consumer: PcmRingBuffer) -> Self {
        Self {
            consumer,
            batch: Box::new([0.0f32; BATCH_SIZE]),
            batch_pos: BATCH_SIZE, // start exhausted so first next() triggers a refill
            log_samples: 0,
            log_nonzero: 0,
            underrun_count: 0,
        }
    }
}

impl Iterator for PcmSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        // Refill from ring buffer when the local batch is exhausted.
        // This is the only point where the mutex is acquired.
        if self.batch_pos >= BATCH_SIZE {
            let available = self.consumer.drain_into(self.batch.as_mut());
            if available < BATCH_SIZE {
                self.underrun_count += (BATCH_SIZE - available) as u32;
            }
            self.batch_pos = 0;
        }

        let sample = self.batch[self.batch_pos];
        self.batch_pos += 1;

        self.log_samples += 1;
        if sample.abs() > 1e-6 {
            self.log_nonzero += 1;
        }

        // Log once per second (44100 samples)
        if self.log_samples >= 44100 {
            if self.underrun_count > 0 {
                log::warn!(
                    "[PCM] Rodio: underrun — {} silence samples in the last ~1s (ring buffer ran dry)",
                    self.underrun_count
                );
            }
            log::debug!(
                "[PCM] Rodio: {}/{} samples non-zero ({:.1}%), ring buffer available: {}",
                self.log_nonzero,
                self.log_samples,
                100.0 * self.log_nonzero as f32 / self.log_samples as f32,
                self.consumer.available(),
            );
            self.log_samples = 0;
            self.log_nonzero = 0;
            self.underrun_count = 0;
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
        sink.append(PcmSource::new(consumer));
        // Sink starts playing immediately (no pause needed — silence is just 0.0 samples)
        Self { _sink: sink }
    }
}
