use crate::tests::run_test_with_interaction;

#[test_log::test]
pub(crate) fn check_keystroke_int16() {
    run_test_with_interaction("keyboard/check_keystroke_int16", |computer| {
        computer.push_key_press(0x18 /* 'o' */);
        computer.run();
    });
}

/// Tests a custom INT 09h handler that mirrors the IO.SYS pattern from MS-DOS 4.01.
///
/// Verifies:
/// - INT 15h AH=4Fh returns CF=1 (key NOT intercepted, pass to BDA)
/// - BDA ring buffer stores [ascii_code][scan_code] (ascii at lower address)
///
/// See: ai-analysis/msdos4-keyboard-interrupt-handling.md
#[test_log::test]
pub(crate) fn custom_int09_keyboard_intercept() {
    run_test_with_interaction("keyboard/custom_int09_keyboard_intercept", |computer| {
        computer.push_key_press(0x1C /* Enter */);
        computer.run();
    });
}
