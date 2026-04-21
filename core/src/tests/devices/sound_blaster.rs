use crate::{
    devices::{SoundBlaster, SoundBlasterModel},
    tests::{TEST_OFFSET, TEST_SEGMENT, load_program_data, run_test},
};

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
            computer.add_sound_blaster(SoundBlaster::new(SoundBlasterModel::Sb16, 8_000_000));
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
            computer.add_sound_blaster(SoundBlaster::new(SoundBlasterModel::Sb16, 8_000_000));
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
            computer.add_sound_blaster(SoundBlaster::new(SoundBlasterModel::Sb16, 8_000_000));
            computer.run();
        },
    );
}

/// OPL timer detection works at the SB base port (0x220/0x221).
#[test_log::test]
pub(crate) fn opl_detect_via_sb_port() {
    run_test(
        "devices/sound_blaster/opl_detect",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(SoundBlasterModel::Sb16, 8_000_000));
            computer.run();
        },
    );
}

/// AdLib-compat ports (0x388/0x389) still work when SB16 is the active card.
#[test_log::test]
pub(crate) fn opl_adlib_compat() {
    run_test(
        "devices/sound_blaster/opl_adlib_compat",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(SoundBlasterModel::Sb16, 8_000_000));
            computer.run();
        },
    );
}

/// Mixer register write/read round-trips correctly; IRQ config register persists.
#[test_log::test]
pub(crate) fn mixer_readwrite() {
    run_test(
        "devices/sound_blaster/mixer_readwrite",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(SoundBlasterModel::Sb16, 8_000_000));
            computer.run();
        },
    );
}

/// IRQ5 fires after single-cycle 8-bit DMA block completes.
#[test_log::test]
pub(crate) fn dsp_pcm_irq_fires() {
    run_test(
        "devices/sound_blaster/dsp_pcm_single",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(SoundBlasterModel::Sb16, 8_000_000));
            computer.run();
        },
    );
}

/// 8-bit unsigned PCM DMA transfer pushes non-zero samples to the ring buffer.
#[test_log::test]
pub(crate) fn dsp_pcm_samples_in_ring_buffer() {
    let program_data = load_program_data("devices/sound_blaster/dsp_pcm_samples");
    let (mut computer, _video_buffer) = create_sb_computer();
    let sb = SoundBlaster::new(SoundBlasterModel::Sb16, 8_000_000);
    let pcm_consumer = sb.pcm_consumer();
    computer.add_sound_blaster(sb);
    computer
        .load_program(&program_data, TEST_SEGMENT, TEST_OFFSET)
        .unwrap();
    computer.run();
    assert_eq!(Some(0), computer.get_exit_code());
    let available = pcm_consumer.available();
    assert!(
        available > 0,
        "ring buffer must contain samples after PCM DMA"
    );
    let mut samples = vec![0.0f32; available];
    pcm_consumer.drain_into(&mut samples);
    assert!(
        samples.iter().any(|&s| s != 0.0),
        "ring buffer must contain non-zero samples"
    );
}

/// MPU-401 reset returns 0xFE ACK; entering UART mode also returns 0xFE.
#[test_log::test]
pub(crate) fn mpu_reset_and_uart_mode() {
    run_test(
        "devices/sound_blaster/mpu_reset",
        create_sb_computer(),
        |computer, _video_buffer| {
            computer.add_sound_blaster(SoundBlaster::new(SoundBlasterModel::Sb16, 8_000_000));
            computer.run();
        },
    );
}

/// Playing an OPL voice via the SB port produces non-zero PCM samples.
#[test_log::test]
pub(crate) fn opl_tone_produces_samples() {
    let program_data = load_program_data("devices/sound_blaster/opl_play_tone");
    let (mut computer, _video_buffer) = create_sb_computer();
    let sb = SoundBlaster::new(SoundBlasterModel::Sb16, 8_000_000);
    let opl_consumer = sb.opl_consumer();
    computer.add_sound_blaster(sb);
    computer
        .load_program(&program_data, TEST_SEGMENT, TEST_OFFSET)
        .unwrap();
    computer.run();
    assert_eq!(Some(0), computer.get_exit_code());
    let available = opl_consumer.available();
    assert!(
        available > 0,
        "ring buffer must contain samples after OPL tone"
    );
    let mut samples = vec![0.0f32; available];
    opl_consumer.drain_into(&mut samples);
    assert!(
        samples.iter().any(|&s| s != 0.0),
        "ring buffer must contain non-zero samples"
    );
}
