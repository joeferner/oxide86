use emu86_core::audio::adlib::{ADLIB_SAMPLE_RATE, AdlibConsumer};
use rodio::stream::OutputStream;
use rodio::{OutputStreamBuilder, Sink, Source};
use std::time::Duration;

/// Rodio audio source that pulls PCM samples from an AdLib consumer handle.
struct AdlibSource {
    consumer: AdlibConsumer,
}

impl Iterator for AdlibSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        // pop_samples already pads with 0.0 on underrun
        Some(self.consumer.pop_samples(1).remove(0))
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
    pub fn new(consumer: AdlibConsumer) -> Result<Self, String> {
        let stream = OutputStreamBuilder::open_default_stream()
            .map_err(|e| format!("AdLib audio device unavailable: {}", e))?;

        let sink = Sink::connect_new(stream.mixer());
        sink.append(AdlibSource { consumer });
        // Sink starts playing immediately (no pause needed — silence is just 0.0 samples)

        Ok(Self {
            _stream: stream,
            _sink: sink,
        })
    }
}
