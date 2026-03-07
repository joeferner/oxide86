use crate::tests::{create_computer, run_test};

mod bios;

#[test_log::test]
pub(crate) fn op8086() {
    run_test(
        "cpu/op8086",
        create_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn irq_chain() {
    run_test(
        "cpu/irq_chain",
        create_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}
