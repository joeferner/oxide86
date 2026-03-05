use crate::cpu::CpuType;
use crate::tests::run_test_configured;

#[test_log::test]
pub(crate) fn get_extended_memory() {
    run_test_configured(
        "cpu/bios/int15/get_extended_memory",
        make_computer!(cpu_type: CpuType::I80286),
        |c| c.run(),
    );
}

#[test_log::test]
pub(crate) fn get_system_config() {
    run_test_configured(
        "cpu/bios/int15/get_system_config",
        make_computer!(cpu_type: CpuType::I80286),
        |c| c.run(),
    );
}

#[test_log::test]
pub(crate) fn unsupported_function() {
    run_test_configured(
        "cpu/bios/int15/unsupported_function",
        make_computer!(cpu_type: CpuType::I8086),
        |c| c.run(),
    );
}
