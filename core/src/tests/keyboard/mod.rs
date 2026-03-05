use crate::tests::run_test_with_interaction;

#[test_log::test]
pub(crate) fn check_keystroke_int16() {
    run_test_with_interaction("keyboard/check_keystroke_int16", |computer| {
        computer.push_key_press(0x18 /* 'o' */);
        computer.run();
    });
}
