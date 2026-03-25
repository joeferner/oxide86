use std::sync::{Arc, RwLock};

use crate::tests::{create_computer, devices::mock_com_device::MockComDevice, run_test};

#[test_log::test]
pub(crate) fn uart_loopback() {
    run_test(
        "devices/uart/uart_loopback",
        create_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn uart_hello_world() {
    run_test(
        "devices/uart/uart_hello_world",
        create_computer(),
        |computer, _video_buffer| {
            let mut mock = MockComDevice::new(3);
            mock.add_response("hello", "ok");

            let test_device = Arc::new(RwLock::new(mock));

            computer.set_com_port_device(1, Some(test_device.clone()));
            computer.run();

            assert!(
                test_device.read().unwrap().was_received("hello"),
                "Computer never sent 'hello'"
            );
        },
    );
}
