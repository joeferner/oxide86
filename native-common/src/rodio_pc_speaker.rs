use oxide86_core::devices::pc_speaker::PcSpeaker;
use rodio::{MixerDeviceSink, Player, source::SquareWave};

pub(crate) struct RodioPcSpeaker {
    player: Player,
}

impl RodioPcSpeaker {
    pub(crate) fn new(sink: &MixerDeviceSink) -> Self {
        let player = Player::connect_new(sink.mixer());

        Self { player }
    }
}

impl PcSpeaker for RodioPcSpeaker {
    fn enable(&mut self, freq: f32) {
        self.player.stop();
        self.player.append(SquareWave::new(freq));
        self.player.play();
    }

    fn disable(&mut self) {
        self.player.pause();
    }
}
