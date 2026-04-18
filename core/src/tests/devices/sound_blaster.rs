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
