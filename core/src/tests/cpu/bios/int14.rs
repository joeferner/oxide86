use std::sync::{Arc, RwLock};

use crate::cpu::CpuType;
use crate::tests::mock_com_device::MockComDevice;
use crate::tests::run_test_configured;

#[test_log::test]
pub fn com1_read_write() {
    run_test_configured(
        "cpu/bios/int14/com1_read_write",
        make_computer!(cpu_type: CpuType::I8086),
        |computer| {
            let mut mock = MockComDevice::new(3);
            mock.add_response("8", "6");

            let test_device = Arc::new(RwLock::new(mock));

            computer.set_com_port_device(1, Some(test_device.clone()));
            computer.run();

            assert!(
                test_device.read().unwrap().was_received("8"),
                "Computer never sent '8'"
            );
        },
    );
}
