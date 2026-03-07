use crate::tests::{create_computer, run_test};

#[test_log::test]
pub(crate) fn check_timer_tick() {
    run_test(
        "devices/pit/check_timer_tick",
        create_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}
