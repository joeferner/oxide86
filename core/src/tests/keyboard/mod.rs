use crate::tests::run_test_with_interaction;

#[test_log::test]
pub(crate) fn check_keystroke_int16() {
    run_test_with_interaction("keyboard/check_keystroke_int16", |computer| {
        computer.push_key_press(0x18 /* 'o' */);
        computer.run();
    });
}

/// Tests that ALT+key combinations work correctly when a custom INT 09h handler
/// reads port 0x60 directly (clearing OBF) before chaining to the old BIOS handler.
///
/// Regression test for: process_key_presses loading the next queued scan code
/// between the custom handler's port 0x60 read and the chained BIOS handler's
/// port 0x60 read, causing the BDA ALT flag to never be set.
#[test_log::test]
pub(crate) fn custom_int09_alt_key() {
    run_test_with_interaction("keyboard/custom_int09_alt_key", |computer| {
        // First run: program installs the custom INT 09h handler, enables
        // interrupts, then blocks on INT 16h AH=00h waiting for a key.
        computer.run();
        // Push keys now — the custom handler is already installed, so IRQ1
        // goes through int09_handler (reads port 0x60, chains to BIOS).
        computer.push_key_press(0x38); // ALT press
        computer.push_key_press(0x21); // F press
        computer.push_key_press(0xA1); // F release (0x80 | 0x21)
        // Second run: resumes from INT 16h, processes keys via custom handler.
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
