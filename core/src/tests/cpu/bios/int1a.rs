use crate::cpu::CpuType;
use crate::tests::run_test;

#[test_log::test]
pub(crate) fn rtc_date() {
    run_test(
        "cpu/bios/int1a/rtc_date",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| computer.run(),
    );
}

#[test_log::test]
pub(crate) fn rtc_time() {
    run_test(
        "cpu/bios/int1a/rtc_time",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| computer.run(),
    );
}

#[test_log::test]
pub(crate) fn rtc_set() {
    run_test(
        "cpu/bios/int1a/rtc_set",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| computer.run(),
    );
}

#[test_log::test]
pub(crate) fn tick_count() {
    run_test(
        "cpu/bios/int1a/tick_count",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| computer.run(),
    );
}
