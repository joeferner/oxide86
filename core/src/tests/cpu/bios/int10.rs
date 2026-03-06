use crate::tests::run_test;

/// INT 10h / AH=0Eh - Teletype Output
/// Writes characters to screen advancing cursor, verifies cursor position after write.
#[test_log::test]
pub(crate) fn teletype_output() {
    run_test("cpu/bios/int10/teletype_output");
}

/// INT 10h / AH=02h - Set Cursor Position
/// INT 10h / AH=03h - Get Cursor Position
/// Sets cursor to multiple positions and reads them back to verify.
#[test_log::test]
pub(crate) fn set_get_cursor_position() {
    run_test("cpu/bios/int10/set_get_cursor_position");
}

/// INT 10h / AH=0Fh - Get Current Video Mode
/// Verifies default mode is 3 (80x25 color text) and that switching modes
/// is reflected in subsequent queries.
#[test_log::test]
pub(crate) fn get_video_mode() {
    run_test("cpu/bios/int10/get_video_mode");
}

/// INT 10h / AH=09h - Write Character and Attribute at Cursor
/// INT 10h / AH=08h - Read Character and Attribute at Cursor
/// Writes a character+attribute pair and reads it back to verify both fields.
#[test_log::test]
pub(crate) fn write_read_char_attr() {
    run_test("cpu/bios/int10/write_read_char_attr");
}

/// INT 10h / AH=01h - Set Cursor Shape
/// Sets cursor start/end scan lines and verifies via AH=03h (get cursor position
/// returns shape in CH/CL).
#[test_log::test]
pub(crate) fn set_cursor_shape() {
    run_test("cpu/bios/int10/set_cursor_shape");
}

/// INT 10h / AH=06h - Scroll Up Window
/// Writes a character to row 1, scrolls up by 1 line, verifies the character
/// is now visible at row 0.
#[test_log::test]
pub(crate) fn scroll_up() {
    run_test("cpu/bios/int10/scroll_up");
}

/// INT 10h / AH=0Ah - Write Character Only at Cursor (no attribute)
/// Writes a character+attribute with AH=09h, then overwrites the character
/// with AH=0Ah and verifies the attribute is preserved while the character changes.
#[test_log::test]
pub(crate) fn write_char() {
    run_test("cpu/bios/int10/write_char");
}
