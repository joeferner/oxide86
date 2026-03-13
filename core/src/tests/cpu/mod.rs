use crate::cpu::CpuType;
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
pub(crate) fn op286() {
    run_test(
        "cpu/op286",
        make_computer!(cpu_type: CpuType::I80286),
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

/// Run the CPU detection program as an 8086.
/// Expected: SP push quirk detected, bits 12-15 confirmed 0xF000 → exit 0x00.
#[test_log::test]
pub(crate) fn cpu_detect_8086() {
    let program_data = super::load_program_data("cpu/cpu_detect");
    let (mut computer, _video_buffer) = make_computer!();
    computer
        .load_program(&program_data, super::TEST_SEGMENT, super::TEST_OFFSET)
        .unwrap();
    computer.run();
    assert_eq!(
        Some(0x00),
        computer.get_exit_code(),
        "expected 8086 detection (exit 0x00)"
    );
}

/// Run the same program as a 286.
/// Expected: no SP quirk, IOPL not settable, bits 12-15 confirmed 0x0000 → exit 0x01.
#[test_log::test]
pub(crate) fn cpu_detect_286() {
    let program_data = super::load_program_data("cpu/cpu_detect");
    let (mut computer, _video_buffer) = make_computer!(cpu_type: CpuType::I80286);
    computer
        .load_program(&program_data, super::TEST_SEGMENT, super::TEST_OFFSET)
        .unwrap();
    computer.run();
    assert_eq!(
        Some(0x01),
        computer.get_exit_code(),
        "expected 286 detection (exit 0x01)"
    );
}
