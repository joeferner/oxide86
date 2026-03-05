use std::sync::{Arc, RwLock};

use crate::tests::{mock_com_device::MockComDevice, run_test_with_interaction};

#[test_log::test]
pub fn uart_hello_world() {
    run_test_with_interaction("uart/uart_hello_world", |computer| {
        let mut mock = MockComDevice::new(3);
        mock.add_response("hello", "ok");

        let test_device = Arc::new(RwLock::new(mock));

        computer.set_com_port_device(1, Some(test_device.clone()));
        computer.run();

        assert!(
            test_device.read().unwrap().was_received("hello"),
            "Computer never sent 'hello'"
        );
    });
}
