use crate::{
    tests::{assert_screen, create_computer, run_test},
    video::VideoCardType,
};

/// Prints all 256 ASCII characters (0x00-0xFF) using the 8x16 VGA font
/// in text mode 3 (80x25), displayed in a 16x16 grid.
#[test_log::test]
pub(crate) fn print_chars_8x16() {
    let name = "video/print_chars_8x16";
    run_test(
        name,
        make_computer!(video_card_type: VideoCardType::VGA),
        |computer, video_buffer| {
            computer.run();
            assert_screen(name, video_buffer);
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

/// Prints all 256 ASCII characters (0x00-0xFF) using the 8x8 CGA font
/// in CGA graphics mode 04h (320x200 4-color), displayed in a 16x16 grid.
#[test_log::test]
pub(crate) fn print_chars_8x8() {
    let name = "video/print_chars_8x8";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
        computer.push_key_press(0x1C /* Enter */);
        computer.run();
    });
}

#[test_log::test]
pub(crate) fn mode_13h_vga_320x200x256() {
    let name = "video/mode_13h_vga_320x200x256";
    run_test(
        name,
        make_computer!(video_card_type: VideoCardType::VGA),
        |computer, video_buffer| {
            computer.run();
            assert_screen(name, video_buffer);
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn mode_06h_cga_640x200x2() {
    let name = "video/mode_06h_cga_640x200x2";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
        computer.push_key_press(0x1C /* Enter */);
        computer.run();
    });
}

#[test_log::test]
pub(crate) fn mode_04h_cga_320x200x4() {
    let name = "video/mode_04h_cga_320x200x4";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
        computer.push_key_press(0x1C /* Enter */);
        computer.run();
    });
}

#[test_log::test]
pub(crate) fn mode_0dh_ega_320x200x16() {
    let name = "video/mode_0dh_ega_320x200x16";
    run_test(
        name,
        make_computer!(video_card_type: VideoCardType::EGA),
        |computer, video_buffer| {
            computer.run();
            assert_screen(name, video_buffer);
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn mode_10h_ega_640x350x16() {
    let name = "video/mode_10h_ega_640x350x16";
    run_test(
        name,
        make_computer!(video_card_type: VideoCardType::EGA),
        |computer, video_buffer| {
            computer.run();
            assert_screen(name, video_buffer);
            computer.push_key_press(0x1C /* Enter */);
            computer.run();
        },
    );
}
