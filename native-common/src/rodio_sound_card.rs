use oxide86_core::devices::PcmRingBuffer;
use rodio::Source;
use std::{num::NonZero, time::Duration};

/// Number of samples pre-fetched from the ring buffer at a time.
///
/// The mutex is acquired once per batch, reducing lock contention from
/// ~44100 times/second down to ~86 times/second at 44100 Hz.
const BATCH_SIZE: usize = 512;

/// Rodio audio source that drains samples from the AdLib ring buffer.
///
/// Construct one from the `PcmRingBuffer` returned by `Adlib::consumer()`,
/// then append it to the mixer sink so the audio thread pulls OPL2 output
/// continuously.
pub(crate) struct RodioSoundCard {
    buffer: PcmRingBuffer,
    batch: Box<[f32; BATCH_SIZE]>,
    batch_pos: usize,
    log_samples: u32,
    log_nonzero: u32,
    underrun_count: u32,
}

impl RodioSoundCard {
    pub(crate) fn new(buffer: PcmRingBuffer) -> Self {
        Self {
            buffer,
            batch: Box::new([0.0f32; BATCH_SIZE]),
            batch_pos: BATCH_SIZE, // start exhausted so first next() triggers a refill
            log_samples: 0,
            log_nonzero: 0,
            underrun_count: 0,
        }
    }
}

impl Iterator for RodioSoundCard {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.batch_pos >= BATCH_SIZE {
            let available = self.buffer.drain_into(self.batch.as_mut());
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

        if self.log_samples >= self.buffer.sample_rate {
            if self.underrun_count > 0 && self.log_nonzero > 0 {
                log::warn!(
                    "[PCM] underrun — {} silence samples in the last ~1s (ring buffer ran dry)",
                    self.underrun_count
                );
            }
            log::debug!(
                "[PCM] {}/{} samples non-zero ({:.1}%), ring buffer available: {}",
                self.log_nonzero,
                self.log_samples,
                100.0 * self.log_nonzero as f32 / self.log_samples as f32,
                self.buffer.available(),
            );
            self.log_samples = 0;
            self.log_nonzero = 0;
            self.underrun_count = 0;
        }

        Some(sample)
    }
}

impl Source for RodioSoundCard {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::new(1).unwrap()
    }

    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(self.buffer.sample_rate).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
