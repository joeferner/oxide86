use oxide86_core::devices::pc_speaker::PcSpeaker;
use rodio::{MixerDeviceSink, Source};
use std::{
    num::NonZero,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::Duration,
};

const SAMPLE_RATE: u32 = 44100;
const AMPLITUDE: f32 = 0.25;

/// Continuous square-wave source whose frequency can be changed atomically.
///
/// Frequency is stored as f32 bits in the atomic. Zero means silent.
struct ContinuousSquareWave {
    freq_bits: Arc<AtomicU32>,
    phase: f32,
}

impl ContinuousSquareWave {
    fn new(freq_bits: Arc<AtomicU32>) -> Self {
        Self {
            freq_bits,
            phase: 0.0,
        }
    }
}

impl Iterator for ContinuousSquareWave {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let freq = f32::from_bits(self.freq_bits.load(Ordering::Relaxed));
        if freq <= 0.0 {
            self.phase = 0.0;
            return Some(0.0);
        }

        let sample = if self.phase < 0.5 { AMPLITUDE } else { -AMPLITUDE };
        self.phase += freq / SAMPLE_RATE as f32;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        Some(sample)
    }
}

impl Source for ContinuousSquareWave {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> NonZero<u16> {
        NonZero::new(1).unwrap()
    }

    fn sample_rate(&self) -> NonZero<u32> {
        NonZero::new(SAMPLE_RATE).unwrap()
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

pub(crate) struct RodioPcSpeaker {
    freq_bits: Arc<AtomicU32>,
}

impl RodioPcSpeaker {
    pub(crate) fn new(sink: &MixerDeviceSink) -> Self {
        let freq_bits = Arc::new(AtomicU32::new(0));
        sink.mixer().add(ContinuousSquareWave::new(Arc::clone(&freq_bits)));
        Self { freq_bits }
    }
}

impl PcSpeaker for RodioPcSpeaker {
    fn enable(&mut self, freq: f32) {
        log::debug!("enable {freq}Hz");
        self.freq_bits.store(freq.to_bits(), Ordering::Relaxed);
    }

    fn disable(&mut self) {
        log::debug!("disable");
        self.freq_bits.store(0, Ordering::Relaxed);
    }
}
