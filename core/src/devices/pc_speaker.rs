pub trait PcSpeaker {
    fn enable(&mut self, freq: f32);
    fn disable(&mut self);
}

pub struct NullPcSpeaker {}

impl NullPcSpeaker {
    pub fn new() -> Self {
        Self {}
    }
}

impl PcSpeaker for NullPcSpeaker {
    fn enable(&mut self, _freq: f32) {}

    fn disable(&mut self) {}
}
