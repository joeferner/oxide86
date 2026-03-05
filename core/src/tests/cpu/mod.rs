use crate::tests::run_test;

mod bios;

#[test_log::test]
pub fn op8086() {
    run_test("cpu/op8086");
}

#[test_log::test]
pub fn irq_chain() {
    run_test("cpu/irq_chain");
}
