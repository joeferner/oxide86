use crate::tests::run_test;

fn create_rtc_computer() -> (
    crate::computer::Computer,
    std::sync::Arc<std::sync::RwLock<crate::video::VideoBuffer>>,
) {
    make_computer!(cpu_type: crate::cpu::CpuType::I80286)
}

#[test_log::test]
pub(crate) fn detect_rtc() {
    run_test(
        "devices/rtc/detect_rtc",
        create_rtc_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn alarm_interrupt() {
    run_test(
        "devices/rtc/alarm_interrupt",
        create_rtc_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn alarm_int4a() {
    run_test(
        "devices/rtc/alarm_int4a",
        create_rtc_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}
