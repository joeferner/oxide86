use crate::tests::{assert_screen, create_computer, run_test};

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
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
        computer.push_key_press(0x1C /* Enter */);
        computer.run();
    });
}
