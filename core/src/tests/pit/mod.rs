use crate::tests::run_test;

#[test_log::test]
pub fn check_timer_tick() {
    run_test("pit/check_timer_tick");
}
