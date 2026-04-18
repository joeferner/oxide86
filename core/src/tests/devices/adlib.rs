use crate::{
    devices::adlib::Adlib,
    tests::{TEST_OFFSET, TEST_SEGMENT, load_program_data, run_test},
};

fn create_adlib_computer() -> (
    crate::computer::Computer,
    std::sync::Arc<std::sync::RwLock<crate::video::VideoBuffer>>,
) {
    make_computer!()
}

/// Standard AdLib detection via Timer 1: status must be 0xC0 after firing.
#[test_log::test]
pub(crate) fn detect_adlib() {
    run_test(
        "devices/adlib/detect_adlib",
        create_adlib_computer(),
        |computer, _video_buffer| {
            computer.add_sound_card(Adlib::new(8_000_000));
            computer.run();
        },
    );
}

/// Timer 2 fires correctly: status must be 0xA0 (bits 7 and 5) after overflow.
#[test_log::test]
pub(crate) fn adlib_timer2() {
    run_test(
        "devices/adlib/adlib_timer2",
        create_adlib_computer(),
        |computer, _video_buffer| {
            computer.add_sound_card(Adlib::new(8_000_000));
            computer.run();
        },
    );
}

/// Writing 0x80 to register 0x04 clears the status flags.
#[test_log::test]
pub(crate) fn adlib_status_clear() {
    run_test(
        "devices/adlib/adlib_status_clear",
        create_adlib_computer(),
        |computer, _video_buffer| {
            computer.add_sound_card(Adlib::new(8_000_000));
            computer.run();
        },
    );
}

/// Playing an OPL2 voice produces non-zero PCM samples in the ring buffer.
#[test_log::test]
pub(crate) fn tone_produces_samples() {
    let program_data = load_program_data("devices/adlib/adlib_play_tone");
    let (mut computer, _video_buffer) = create_adlib_computer();

    let adlib = Adlib::new(8_000_000);
    let consumer = adlib.consumer();
    computer.add_sound_card(adlib);

    computer
        .load_program(&program_data, TEST_SEGMENT, TEST_OFFSET)
        .unwrap();
    computer.run();
    assert_eq!(Some(0), computer.get_exit_code());

    let available = consumer.available();
    assert!(
        available > 0,
        "ring buffer must contain samples after playing a tone"
    );

    let mut samples = vec![0.0f32; available];
    consumer.drain_into(&mut samples);
    assert!(
        samples.iter().any(|&s| s != 0.0),
        "ring buffer must contain non-zero samples (OPL voice should produce sound)"
    );
}
