use oxide86_core::devices::pc_speaker::PcSpeaker;
use rodio::{MixerDeviceSink, Player, source::SquareWave};

pub(crate) struct RodioPcSpeaker {
    player: Player,
    current_freq: Option<f32>,
}

impl RodioPcSpeaker {
    pub(crate) fn new(sink: &MixerDeviceSink) -> Self {
        let player = Player::connect_new(sink.mixer());

        Self {
            player,
            current_freq: None,
        }
    }
}

impl PcSpeaker for RodioPcSpeaker {
    fn enable(&mut self, freq: f32) {
        if self.current_freq != Some(freq) {
            log::debug!("enable {freq}Hz");
            self.current_freq = Some(freq);
            self.player.stop();
            self.player.append(SquareWave::new(freq));
            self.player.play();
        }
    }

    fn disable(&mut self) {
        if self.current_freq.is_some() {
            log::debug!("disable");
            self.current_freq = None;
            self.player.pause();
        }
    }
}
