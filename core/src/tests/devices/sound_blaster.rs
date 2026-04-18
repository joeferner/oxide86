use crate::{devices::SoundBlaster, tests::run_test};

fn create_sb_computer() -> (
    crate::computer::Computer,
    std::sync::Arc<std::sync::RwLock<crate::video::VideoBuffer>>,
) {
    make_computer!()
}

/// CD-ROM NOP command returns [0xAA, 0x55] signature via the unified SoundBlaster device.
#[test_log::test]
pub(crate) fn cdrom_nop_via_unified_card() {
    run_test(
        "devices/sound_blaster/cdrom_nop",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(8_000_000));
            computer.run();
        },
    );
}

/// DSP reset handshake returns 0xAA; version query returns 0x04/0x05.
#[test_log::test]
pub(crate) fn dsp_reset_and_version() {
    run_test(
        "devices/sound_blaster/dsp_reset",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(8_000_000));
            computer.run();
        },
    );
}

/// Speaker on (0xD1), off (0xD3), and status query (0xD8) round-trip correctly.
#[test_log::test]
pub(crate) fn dsp_speaker_control() {
    run_test(
        "devices/sound_blaster/dsp_speaker",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(8_000_000));
            computer.run();
        },
    );
}
