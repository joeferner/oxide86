use crate::{
    tests::{create_computer, run_assert_screen_key_press_run_test},
    video::VideoCardType,
};

/// Prints all 256 ASCII characters (0x00-0xFF) using the 8x16 VGA font
/// in text mode 3 (80x25), displayed in a 16x16 grid.
#[test_log::test]
pub(crate) fn print_chars_8x16() {
    run_assert_screen_key_press_run_test(
        "video/print_chars_8x16",
        make_computer!(video_card_type: VideoCardType::VGA),
    );
}

/// Prints all 256 ASCII characters (0x00-0xFF) using the 8x8 CGA font
/// in CGA graphics mode 04h (320x200 4-color), displayed in a 16x16 grid.
#[test_log::test]
pub(crate) fn print_chars_8x8() {
    run_assert_screen_key_press_run_test("video/print_chars_8x8", create_computer());
}

#[test_log::test]
pub(crate) fn mode_13h_vga_320x200x256() {
    run_assert_screen_key_press_run_test(
        "video/mode_13h_vga_320x200x256",
        make_computer!(video_card_type: VideoCardType::VGA),
    );
}

#[test_log::test]
pub(crate) fn mode_06h_cga_640x200x2() {
    run_assert_screen_key_press_run_test("video/mode_06h_cga_640x200x2", create_computer());
}

#[test_log::test]
pub(crate) fn mode_04h_cga_320x200x4() {
    run_assert_screen_key_press_run_test("video/mode_04h_cga_320x200x4", create_computer());
}

#[test_log::test]
pub(crate) fn mode_0dh_ega_320x200x16() {
    run_assert_screen_key_press_run_test(
        "video/mode_0dh_ega_320x200x16",
        make_computer!(video_card_type: VideoCardType::EGA),
    );
}

/// Tests INT 10h/AH=11h/AL=30h font info queries and direct EGA VRAM glyph rendering,
/// mirroring the technique SimCity uses for menu text (BH=02h ROM 8x14 path).
#[test_log::test]
pub(crate) fn font_direct_render() {
    run_assert_screen_key_press_run_test(
        "video/font_direct_render",
        make_computer!(video_card_type: VideoCardType::EGA),
    );
}

#[test_log::test]
pub(crate) fn print_chars_8x14() {
    run_assert_screen_key_press_run_test(
        "video/print_chars_8x14",
        make_computer!(video_card_type: VideoCardType::EGA),
    );
}

#[test_log::test]
pub(crate) fn mode_10h_ega_640x350x16() {
    run_assert_screen_key_press_run_test(
        "video/mode_10h_ega_640x350x16",
        make_computer!(video_card_type: VideoCardType::EGA),
    );
}
