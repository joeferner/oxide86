use crate::cpu::CpuType;
use crate::tests::run_test_configured;

#[test_log::test]
pub fn rtc_date() {
    run_test_configured(
        "cpu/bios/int1a/rtc_date",
        make_computer!(cpu_type: CpuType::I80286),
        |c| c.run(),
    );
}

#[test_log::test]
pub fn rtc_time() {
    run_test_configured(
        "cpu/bios/int1a/rtc_time",
        make_computer!(cpu_type: CpuType::I80286),
        |c| c.run(),
    );
}

#[test_log::test]
pub fn rtc_set() {
    run_test_configured(
        "cpu/bios/int1a/rtc_set",
        make_computer!(cpu_type: CpuType::I80286),
        |c| c.run(),
    );
}

#[test_log::test]
pub fn tick_count() {
    run_test_configured(
        "cpu/bios/int1a/tick_count",
        make_computer!(cpu_type: CpuType::I80286),
        |c| c.run(),
    );
}
