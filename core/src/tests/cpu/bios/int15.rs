use crate::cpu::CpuType;
use crate::tests::run_test;

#[test_log::test]
pub(crate) fn get_extended_memory() {
    run_test(
        "cpu/bios/int15/get_extended_memory",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| computer.run(),
    );
}

#[test_log::test]
pub(crate) fn get_system_config() {
    run_test(
        "cpu/bios/int15/get_system_config",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| computer.run(),
    );
}

#[test_log::test]
pub(crate) fn unsupported_function() {
    run_test(
        "cpu/bios/int15/unsupported_function",
        make_computer!(cpu_type: CpuType::I8086),
        |computer, _video_buffer| computer.run(),
    );
}

/// On an 8086 the A20 gate service (AH=24h) is not available — all subfunctions
/// must return CF=1, AH=86h.
#[test_log::test]
pub(crate) fn a20_8086() {
    run_test(
        "cpu/bios/int15/a20_8086",
        make_computer!(cpu_type: CpuType::I8086),
        |computer, _video_buffer| computer.run(),
    );
}

/// On a 286 the A20 gate service (AH=24h) must enable/disable the gate and
/// the change must be visible in actual memory addressing (wrap-around test).
#[test_log::test]
pub(crate) fn a20_286() {
    run_test(
        "cpu/bios/int15/a20_286",
        make_computer!(cpu_type: CpuType::I80286),
        |computer, _video_buffer| computer.run(),
    );
}
