use crate::tests::{assert_screen, create_computer, run_test};
use crate::video::VideoCardType;

/// INT 10h / AH=0Eh - Teletype Output
/// Writes characters to screen advancing cursor, verifies cursor position after write.
#[test_log::test]
pub(crate) fn teletype_output() {
    let name = "cpu/bios/int10/teletype_output";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
    });
}

/// INT 10h / AH=02h - Set Cursor Position
/// INT 10h / AH=03h - Get Cursor Position
/// Sets cursor to multiple positions and reads them back to verify.
#[test_log::test]
pub(crate) fn set_get_cursor_position() {
    let name = "cpu/bios/int10/set_get_cursor_position";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
    });
}

/// INT 10h / AH=0Fh - Get Current Video Mode
/// Verifies default mode is 3 (80x25 color text) and that switching modes
/// is reflected in subsequent queries.
#[test_log::test]
pub(crate) fn get_video_mode() {
    run_test(
        "cpu/bios/int10/get_video_mode",
        create_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// INT 10h / AH=09h - Write Character and Attribute at Cursor
/// INT 10h / AH=08h - Read Character and Attribute at Cursor
/// Writes a character+attribute pair and reads it back to verify both fields.
#[test_log::test]
pub(crate) fn write_read_char_attr() {
    let name = "cpu/bios/int10/write_read_char_attr";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
    });
}

/// INT 10h / AH=01h - CGA underline cursor (start=6, end=7)
/// Verifies that a CGA-style cursor positioned at scan lines 6-7 of 8 is
/// scaled correctly to the bottom of a 16-scanline VGA character cell.
#[test_log::test]
pub(crate) fn cursor_cga_underline() {
    let name = "cpu/bios/int10/cursor_cga_underline";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
    });
}

/// INT 10h / AH=01h - Set Cursor Shape
/// Sets cursor start/end scan lines and verifies via AH=03h (get cursor position
/// returns shape in CH/CL).
#[test_log::test]
pub(crate) fn set_cursor_shape() {
    let name = "cpu/bios/int10/set_cursor_shape";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
    });
}

/// INT 10h / AH=06h - Scroll Up Window
/// Writes a character to row 1, scrolls up by 1 line, verifies the character
/// is now visible at row 0.
#[test_log::test]
pub(crate) fn scroll_up() {
    let name = "cpu/bios/int10/scroll_up";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
    });
}

/// INT 10h / AH=07h - Scroll Down Window
/// Writes a character to row 23, scrolls down by 1 line, verifies the character
/// is now visible at row 24.
#[test_log::test]
pub(crate) fn scroll_down() {
    let name = "cpu/bios/int10/scroll_down";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
    });
}

/// INT 10h / AH=0Ah - Write Character Only at Cursor (no attribute)
/// Writes a character+attribute with AH=09h, then overwrites the character
/// with AH=0Ah and verifies the attribute is preserved while the character changes.
#[test_log::test]
pub(crate) fn write_char() {
    let name = "cpu/bios/int10/write_char";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
    });
}

/// INT 10h / AH=0Bh - Set Color Palette
/// Selects CGA palette 1 (cyan/magenta/white) and writes "CMW" with matching
/// attributes, then selects palette 0 (green/red/yellow) and writes "GRY".
/// Reads each character back to verify the attribute round-trips correctly.
/// Also sets the border color and confirms video mode remains 3 throughout.
#[test_log::test]
pub(crate) fn set_color_palette() {
    let name = "cpu/bios/int10/set_color_palette";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
    });
}

/// INT 10h / AH=11h - Character Generator (AL=30h: Return Font Information)
/// Queries 8x16 font info (BH=6) and verifies CX=16 bytes/char, DX=24 (rows-1).
/// Queries 8x8 font info (BH=3) and verifies CX=8 bytes/char, DX=24.
#[test_log::test]
pub(crate) fn character_generator() {
    run_test(
        "cpu/bios/int10/character_generator",
        make_computer!(video_card_type: VideoCardType::VGA),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// INT 10h / AH=15h - Return Physical Display Parameters
/// Verifies BH returns rows-1 (24 for a 25-row screen) after a mode 3 set.
#[test_log::test]
pub(crate) fn return_physical_display_params() {
    run_test(
        "cpu/bios/int10/return_physical_display_params",
        make_computer!(video_card_type: VideoCardType::VGA),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// INT 10h / AH=1Ah - Display Combination Code
/// AL=0 reads DCC: verifies AL=1Ah (supported) and BL is non-zero.
/// AL=1 writes the codes back and reads again to confirm round-trip.
#[test_log::test]
pub(crate) fn display_combination_code() {
    run_test(
        "cpu/bios/int10/display_combination_code",
        make_computer!(video_card_type: VideoCardType::VGA),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// INT 10h / AH=FEh - Get Video Buffer
/// Sets ES=0xB800, calls the function, and verifies ES is unchanged
/// and DI=0 (no virtual buffer redirection active).
#[test_log::test]
pub(crate) fn get_video_buffer() {
    run_test(
        "cpu/bios/int10/get_video_buffer",
        create_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}
