use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Target audio output sample rate for AdLib (OPL2) output.
/// Shared by both the ring buffer and the Rodio/Web Audio backends.
pub const ADLIB_SAMPLE_RATE: u32 = 44100;

/// Shared PCM sample ring buffer between Computer (producer) and
/// the platform audio backend (consumer).
///
/// Samples are f32 in range -1.0..1.0, mono at ADLIB_SAMPLE_RATE Hz.
/// Clones share the same underlying buffer via Arc.
#[derive(Clone)]
pub struct AdlibRingBuffer {
    inner: Arc<Mutex<VecDeque<f32>>>,
    capacity: usize,
}

impl AdlibRingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    /// Push samples into the buffer. Drops the oldest samples if the
    /// buffer is full (prevents unbounded growth on underrun).
    pub fn push_samples(&self, samples: &[f32]) {
        let mut buf = self.inner.lock().unwrap();
        for &s in samples {
            if buf.len() >= self.capacity {
                buf.pop_front();
            }
            buf.push_back(s);
        }
    }

    /// Pop up to `count` samples. Returns zeros for any samples not yet
    /// available (underrun padding).
    pub fn pop_samples(&self, count: usize) -> Vec<f32> {
        let mut buf = self.inner.lock().unwrap();
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(buf.pop_front().unwrap_or(0.0));
        }
        out
    }

    pub fn available(&self) -> usize {
        self.inner.lock().unwrap().len()
    }
}
