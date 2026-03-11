use oxide86_core::devices::PcmRingBuffer;
use rodio::Source;
use std::{num::NonZero, time::Duration};

/// Rodio audio source that drains samples from the AdLib ring buffer.
///
/// Construct one from the `PcmRingBuffer` returned by `Adlib::consumer()`,
/// then append it to the mixer sink so the audio thread pulls OPL2 output
/// continuously.
pub(crate) struct RodioSoundCard {
    buffer: PcmRingBuffer,
}

impl RodioSoundCard {
    pub(crate) fn new(buffer: PcmRingBuffer) -> Self {
        Self { buffer }
    }
}

impl Iterator for RodioSoundCard {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let mut sample = [0.0f32; 1];
        self.buffer.drain_into(&mut sample);
        Some(sample[0])
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
